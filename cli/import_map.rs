// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use deno_core::serde::Serialize;
use deno_core::serde_json;
use deno_core::serde_json::Map;
use deno_core::serde_json::Value;
use deno_core::url::Url;
use deno_core::ModuleSpecifier;
use indexmap::IndexMap;
use std::cmp::Ordering;
use std::collections::HashSet;
use std::error::Error;
use std::fmt;

#[derive(Debug)]
pub struct ImportMapError {
  pub msg: String,
}

impl ImportMapError {
  pub fn new(msg: &str) -> Self {
    ImportMapError {
      msg: msg.to_string(),
    }
  }
}

impl fmt::Display for ImportMapError {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    f.pad(&self.msg)
  }
}

impl Error for ImportMapError {}

// https://url.spec.whatwg.org/#special-scheme
const SPECIAL_PROTOCOLS: &[&str] =
  &["ftp", "file", "http", "https", "ws", "wss"];
fn is_special(url: &Url) -> bool {
  SPECIAL_PROTOCOLS.contains(&url.scheme())
}

type SpecifierMap = IndexMap<String, Option<ModuleSpecifier>>;
type ScopesMap = IndexMap<String, SpecifierMap>;

#[derive(Debug, Clone, Serialize)]
pub struct ImportMap {
  #[serde(skip)]
  base_url: String,

  #[serde(with = "indexmap::serde_seq")]
  imports: SpecifierMap,

  #[serde(with = "indexmap::serde_seq")]
  scopes: ScopesMap,
}

impl ImportMap {
  pub fn from_json(
    base_url: &str,
    json_string: &str,
  ) -> Result<Self, ImportMapError> {
    let v: Value = match serde_json::from_str(json_string) {
      Ok(v) => v,
      Err(_) => {
        return Err(ImportMapError::new("Unable to parse import map JSON"));
      }
    };

    match v {
      Value::Object(_) => {}
      _ => {
        return Err(ImportMapError::new("Import map JSON must be an object"));
      }
    }

    let normalized_imports = match &v.get("imports") {
      Some(imports_map) => {
        if !imports_map.is_object() {
          return Err(ImportMapError::new(
            "Import map's 'imports' must be an object",
          ));
        }

        let imports_map = imports_map.as_object().unwrap();
        ImportMap::parse_specifier_map(imports_map, base_url)
      }
      None => IndexMap::new(),
    };

    let normalized_scopes = match &v.get("scopes") {
      Some(scope_map) => {
        if !scope_map.is_object() {
          return Err(ImportMapError::new(
            "Import map's 'scopes' must be an object",
          ));
        }

        let scope_map = scope_map.as_object().unwrap();
        ImportMap::parse_scope_map(scope_map, base_url)?
      }
      None => IndexMap::new(),
    };

    let mut keys: HashSet<String> = v
      .as_object()
      .unwrap()
      .keys()
      .map(|k| k.to_string())
      .collect();
    keys.remove("imports");
    keys.remove("scopes");
    for key in keys {
      eprintln!("Invalid top-level key \"{:?}\". Only \"imports\" and \"scopes\" can be present.", key);
    }

    let import_map = ImportMap {
      base_url: base_url.to_string(),
      imports: normalized_imports,
      scopes: normalized_scopes,
    };

    Ok(import_map)
  }

  fn try_url_like_specifier(specifier: &str, base: &str) -> Option<Url> {
    // this should never fail
    if specifier.starts_with('/')
      || specifier.starts_with("./")
      || specifier.starts_with("../")
    {
      let base_url = Url::parse(base).unwrap();
      let url = base_url.join(specifier).unwrap();
      return Some(url);
    }

    if let Ok(url) = Url::parse(specifier) {
      return Some(url);
    }

    None
  }

  /// Parse provided key as import map specifier.
  ///
  /// Specifiers must be valid URLs (eg. "https://deno.land/x/std/testing/asserts.ts")
  /// or "bare" specifiers (eg. "moment").
  fn normalize_specifier_key(
    specifier_key: &str,
    base_url: &str,
  ) -> Option<String> {
    // ignore empty keys
    if specifier_key.is_empty() {
      // TODO: add diagnostic?
      return None;
    }

    if let Some(url) =
      ImportMap::try_url_like_specifier(specifier_key, base_url)
    {
      return Some(url.to_string());
    }

    // "bare" specifier
    Some(specifier_key.to_string())
  }

  /// Convert provided JSON map to valid SpecifierMap.
  ///
  /// From specification:
  /// - order of iteration must be retained
  /// - SpecifierMap's keys are sorted in longest and alphabetic order
  fn parse_specifier_map(
    json_map: &Map<String, Value>,
    base_url: &str,
  ) -> SpecifierMap {
    let mut normalized_map: SpecifierMap = SpecifierMap::new();

    // Order is preserved because of "preserve_order" feature of "serde_json".
    for (specifier_key, value) in json_map.iter() {
      let normalized_specifier_key =
        match ImportMap::normalize_specifier_key(specifier_key, base_url) {
          Some(s) => s,
          None => continue,
        };

      let potential_address = match value {
        Value::String(address) => address.to_string(),
        _ => {
          // TODO: add diagnostic
          normalized_map.insert(normalized_specifier_key, None);
          continue;
        }
      };

      let address_url =
        match ImportMap::try_url_like_specifier(&potential_address, base_url) {
          Some(url) => url,
          None => {
            // TODO: add diagnostic
            normalized_map.insert(normalized_specifier_key, None);
            continue;
          }
        };

      let address_url_string = address_url.to_string();
      if specifier_key.ends_with('/') && !address_url_string.ends_with('/') {
        // TODO: make it a diagnostic
        eprintln!(
          "Invalid target address {:?} for package specifier {:?}.\
            Package address targets must end with \"/\".",
          address_url_string, specifier_key
        );
        normalized_map.insert(normalized_specifier_key, None);
        continue;
      }

      normalized_map.insert(normalized_specifier_key, Some(address_url));
    }

    // Sort in longest and alphabetical order.
    normalized_map.sort_by(|k1, _v1, k2, _v2| match k1.cmp(&k2) {
      Ordering::Greater => Ordering::Less,
      Ordering::Less => Ordering::Greater,
      // JSON guarantees that there can't be duplicate keys
      Ordering::Equal => unreachable!(),
    });

    normalized_map
  }

  /// Convert provided JSON map to valid ScopeMap.
  ///
  /// From specification:
  /// - order of iteration must be retained
  /// - ScopeMap's keys are sorted in longest and alphabetic order
  fn parse_scope_map(
    scope_map: &Map<String, Value>,
    base_url: &str,
  ) -> Result<ScopesMap, ImportMapError> {
    let mut normalized_map: ScopesMap = ScopesMap::new();

    // Order is preserved because of "preserve_order" feature of "serde_json".
    for (scope_prefix, potential_specifier_map) in scope_map.iter() {
      if !potential_specifier_map.is_object() {
        return Err(ImportMapError::new(&format!(
          "The value for the {:?} scope prefix must be an object",
          scope_prefix
        )));
      }

      let potential_specifier_map =
        potential_specifier_map.as_object().unwrap();

      let scope_prefix_url =
        match Url::parse(base_url).unwrap().join(scope_prefix) {
          Ok(url) => url.to_string(),
          _ => {
            // TODO: add diagnostic
            continue;
          }
        };

      let norm_map =
        ImportMap::parse_specifier_map(potential_specifier_map, base_url);

      normalized_map.insert(scope_prefix_url, norm_map);
    }

    // Sort in longest and alphabetical order.
    normalized_map.sort_by(|k1, _v1, k2, _v2| match k1.cmp(&k2) {
      Ordering::Greater => Ordering::Less,
      Ordering::Less => Ordering::Greater,
      // JSON guarantees that there can't be duplicate keys
      Ordering::Equal => unreachable!(),
    });

    Ok(normalized_map)
  }

  pub fn resolve_scopes_match(
    scopes: &ScopesMap,
    normalized_specifier: &str,
    as_url: Option<&Url>,
    referrer: &str,
  ) -> Result<Option<ModuleSpecifier>, ImportMapError> {
    // exact-match
    if let Some(scope_imports) = scopes.get(referrer) {
      if let Ok(scope_match) = ImportMap::resolve_imports_match(
        scope_imports,
        normalized_specifier,
        as_url,
      ) {
        // Return only if there was actual match (not None).
        if scope_match.is_some() {
          return Ok(scope_match);
        }
      }
    }

    for (normalized_scope_key, scope_imports) in scopes.iter() {
      if normalized_scope_key.ends_with('/')
        && referrer.starts_with(normalized_scope_key)
      {
        if let Ok(scope_match) = ImportMap::resolve_imports_match(
          scope_imports,
          normalized_specifier,
          as_url,
        ) {
          // Return only if there was actual match (not None).
          if scope_match.is_some() {
            return Ok(scope_match);
          }
        }
      }
    }

    Ok(None)
  }

  pub fn resolve_imports_match(
    specifier_map: &SpecifierMap,
    normalized_specifier: &str,
    as_url: Option<&Url>,
  ) -> Result<Option<ModuleSpecifier>, ImportMapError> {
    // exact-match
    if let Some(maybe_address) = specifier_map.get(normalized_specifier) {
      if let Some(address) = maybe_address {
        return Ok(Some(address.clone()));
      } else {
        return Err(ImportMapError::new(&format!(
          "Blocked by null entry for \"{:?}\"",
          normalized_specifier
        )));
      }
    }

    // Package-prefix match
    // "most-specific wins", i.e. when there are multiple matching keys,
    // choose the longest.
    for (specifier_key, maybe_address) in specifier_map.iter() {
      if !specifier_key.ends_with('/') {
        continue;
      }

      if !normalized_specifier.starts_with(specifier_key) {
        continue;
      }

      if let Some(url) = as_url {
        if !is_special(url) {
          continue;
        }
      }

      if maybe_address.is_none() {
        return Err(ImportMapError::new(&format!(
          "Blocked by null entry for \"{:?}\"",
          specifier_key
        )));
      }

      let resolution_result = maybe_address.clone().unwrap();

      // Enforced by parsing.
      assert!(resolution_result.to_string().ends_with('/'));

      let after_prefix = &normalized_specifier[specifier_key.len()..];

      let url = match resolution_result.join(after_prefix) {
        Ok(url) => url,
        Err(_) => {
          return Err(ImportMapError::new(&format!(
            "Failed to resolve the specifier \"{:?}\" as its after-prefix
            portion \"{:?}\" could not be URL-parsed relative to the URL prefix
            \"{:?}\" mapped to by the prefix \"{:?}\"",
            normalized_specifier,
            after_prefix,
            resolution_result,
            specifier_key
          )));
        }
      };

      if !url.as_str().starts_with(resolution_result.as_str()) {
        return Err(ImportMapError::new(&format!(
          "The specifier \"{:?}\" backtracks above its prefix \"{:?}\"",
          normalized_specifier, specifier_key
        )));
      }

      return Ok(Some(url));
    }

    debug!(
      "Specifier {:?} was not mapped in import map.",
      normalized_specifier
    );

    Ok(None)
  }

  pub fn resolve(
    &self,
    specifier: &str,
    referrer: &str,
  ) -> Result<Option<ModuleSpecifier>, ImportMapError> {
    let as_url: Option<Url> =
      ImportMap::try_url_like_specifier(specifier, referrer);
    let normalized_specifier = match &as_url {
      Some(url) => url.to_string(),
      None => specifier.to_string(),
    };

    let scopes_match = ImportMap::resolve_scopes_match(
      &self.scopes,
      &normalized_specifier,
      as_url.as_ref(),
      &referrer.to_string(),
    )?;

    // match found in scopes map
    if scopes_match.is_some() {
      return Ok(scopes_match);
    }

    let imports_match = ImportMap::resolve_imports_match(
      &self.imports,
      &normalized_specifier,
      as_url.as_ref(),
    )?;

    // match found in import map
    if imports_match.is_some() {
      return Ok(imports_match);
    }

    // no match in import map but we got resolvable URL
    if let Some(resolved_url) = as_url {
      return Ok(Some(resolved_url));
    }

    Err(ImportMapError::new(&format!(
      "Unmapped bare specifier {:?}",
      normalized_specifier
    )))
  }
}

