// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use deno_core::error::AnyError;
use deno_core::serde_json;
use deno_core::serde_json::Map;
use deno_core::serde_json::Value;
use deno_core::url::Url;
use deno_core::ModuleSpecifier;
use indexmap::IndexMap;
use std::cmp::Ordering;
use std::error::Error;
use std::fmt;
use std::fs;
use std::io;

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

// NOTE: here is difference between deno and reference implementation - deno currently
//  can't resolve URL with other schemes (eg. data:, about:, blob:)
const SUPPORTED_FETCH_SCHEMES: [&str; 3] = ["http", "https", "file"];

type SpecifierMap = IndexMap<String, Vec<ModuleSpecifier>>;
type ScopesMap = IndexMap<String, SpecifierMap>;

#[derive(Debug, Clone)]
pub struct ImportMap {
  base_url: String,
  imports: SpecifierMap,
  scopes: ScopesMap,
}

impl ImportMap {
  pub fn load(file_path: &str) -> Result<Self, AnyError> {
    let file_url = ModuleSpecifier::resolve_url_or_path(file_path)?.to_string();
    let resolved_path = std::env::current_dir().unwrap().join(file_path);
    debug!(
      "Attempt to load import map: {}",
      resolved_path.to_str().unwrap()
    );

    // Load the contents of import map
    let json_string = fs::read_to_string(&resolved_path).map_err(|err| {
      io::Error::new(
        io::ErrorKind::InvalidInput,
        format!(
          "Error retrieving import map file at \"{}\": {}",
          resolved_path.to_str().unwrap(),
          err.to_string()
        )
        .as_str(),
      )
    })?;
    // The URL of the import map is the base URL for its values.
    ImportMap::from_json(&file_url, &json_string).map_err(AnyError::from)
  }

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
      if SUPPORTED_FETCH_SCHEMES.contains(&url.scheme()) {
        return Some(url);
      }
    }

    None
  }

  /// Parse provided key as import map specifier.
  ///
  /// Specifiers must be valid URLs (eg. "https://deno.land/x/std/testing/asserts.ts")
  /// or "bare" specifiers (eg. "moment").
  // TODO: add proper error handling: https://github.com/WICG/import-maps/issues/100
  fn normalize_specifier_key(
    specifier_key: &str,
    base_url: &str,
  ) -> Option<String> {
    // ignore empty keys
    if specifier_key.is_empty() {
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

  /// Parse provided addresses as valid URLs.
  ///
  /// Non-valid addresses are skipped.
  fn normalize_addresses(
    specifier_key: &str,
    base_url: &str,
    potential_addresses: Vec<String>,
  ) -> Vec<ModuleSpecifier> {
    let mut normalized_addresses: Vec<ModuleSpecifier> = vec![];

    for potential_address in potential_addresses {
      let url =
        match ImportMap::try_url_like_specifier(&potential_address, base_url) {
          Some(url) => url,
          None => continue,
        };

      let url_string = url.to_string();
      if specifier_key.ends_with('/') && !url_string.ends_with('/') {
        eprintln!(
          "Invalid target address {:?} for package specifier {:?}.\
           Package address targets must end with \"/\".",
          url_string, specifier_key
        );
        continue;
      }

      normalized_addresses.push(url.into());
    }

    normalized_addresses
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

      let potential_addresses: Vec<String> = match value {
        Value::String(address) => vec![address.to_string()],
        Value::Array(address_array) => {
          let mut string_addresses: Vec<String> = vec![];

          for address in address_array {
            match address {
              Value::String(address) => {
                string_addresses.push(address.to_string())
              }
              _ => continue,
            }
          }

          string_addresses
        }
        Value::Null => vec![],
        _ => vec![],
      };

      let normalized_address_array = ImportMap::normalize_addresses(
        &normalized_specifier_key,
        base_url,
        potential_addresses,
      );

      debug!(
        "normalized specifier {:?}; {:?}",
        normalized_specifier_key, normalized_address_array
      );
      normalized_map.insert(normalized_specifier_key, normalized_address_array);
    }

    // Sort in longest and alphabetical order.
    normalized_map.sort_by(|k1, _v1, k2, _v2| match k1.cmp(&k2) {
      Ordering::Greater => Ordering::Less,
      Ordering::Less => Ordering::Greater,
      Ordering::Equal => k2.cmp(k1),
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
          Ok(url) => {
            if !SUPPORTED_FETCH_SCHEMES.contains(&url.scheme()) {
              eprintln!(
              "Invalid scope {:?}. Scope URLs must have a valid fetch scheme.",
              url.to_string()
            );
              continue;
            }
            url.to_string()
          }
          _ => continue,
        };

      let norm_map =
        ImportMap::parse_specifier_map(potential_specifier_map, base_url);

      normalized_map.insert(scope_prefix_url, norm_map);
    }

    // Sort in longest and alphabetical order.
    normalized_map.sort_by(|k1, _v1, k2, _v2| match k1.cmp(&k2) {
      Ordering::Greater => Ordering::Less,
      Ordering::Less => Ordering::Greater,
      Ordering::Equal => k2.cmp(k1),
    });

    Ok(normalized_map)
  }

  pub fn resolve_scopes_match(
    scopes: &ScopesMap,
    normalized_specifier: &str,
    referrer: &str,
  ) -> Result<Option<ModuleSpecifier>, ImportMapError> {
    // exact-match
    if let Some(scope_imports) = scopes.get(referrer) {
      if let Ok(scope_match) =
        ImportMap::resolve_imports_match(scope_imports, normalized_specifier)
      {
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
        if let Ok(scope_match) =
          ImportMap::resolve_imports_match(scope_imports, normalized_specifier)
        {
          // Return only if there was actual match (not None).
          if scope_match.is_some() {
            return Ok(scope_match);
          }
        }
      }
    }

    Ok(None)
  }

  // TODO: https://github.com/WICG/import-maps/issues/73#issuecomment-439327758
  // for some more optimized candidate implementations.
  pub fn resolve_imports_match(
    imports: &SpecifierMap,
    normalized_specifier: &str,
  ) -> Result<Option<ModuleSpecifier>, ImportMapError> {
    // exact-match
    if let Some(address_vec) = imports.get(normalized_specifier) {
      if address_vec.is_empty() {
        return Err(ImportMapError::new(&format!(
          "Specifier {:?} was mapped to no addresses.",
          normalized_specifier
        )));
      } else if address_vec.len() == 1 {
        let address = address_vec.first().unwrap();
        debug!(
          "Specifier {:?} was mapped to {:?}.",
          normalized_specifier, address
        );
        return Ok(Some(address.clone()));
      } else {
        return Err(ImportMapError::new(
          "Multi-address mappings are not yet supported",
        ));
      }
    }

    // package-prefix match
    // "most-specific wins", i.e. when there are multiple matching keys,
    // choose the longest.
    // https://github.com/WICG/import-maps/issues/102
    for (specifier_key, address_vec) in imports.iter() {
      if specifier_key.ends_with('/')
        && normalized_specifier.starts_with(specifier_key)
      {
        if address_vec.is_empty() {
          return Err(ImportMapError::new(&format!("Specifier {:?} was mapped to no addresses (via prefix specifier key {:?}).", normalized_specifier, specifier_key)));
        } else if address_vec.len() == 1 {
          let address = address_vec.first().unwrap();
          let after_prefix = &normalized_specifier[specifier_key.len()..];

          let base_url = address.as_url();
          if let Ok(url) = base_url.join(after_prefix) {
            debug!("Specifier {:?} was mapped to {:?} (via prefix specifier key {:?}).", normalized_specifier, url, address);
            return Ok(Some(ModuleSpecifier::from(url)));
          }

          unreachable!();
        } else {
          return Err(ImportMapError::new(
            "Multi-address mappings are not yet supported",
          ));
        }
      }
    }

    debug!(
      "Specifier {:?} was not mapped in import map.",
      normalized_specifier
    );

    Ok(None)
  }

  // TODO: add support for built-in modules
  /// Currently we support two types of specifiers: URL (http://, https://, file://)
  /// and "bare" (moment, jquery, lodash)
  ///
  /// Scenarios:
  ///   1. import resolved using import map -> String
  ///   2. import restricted by import map -> ImportMapError
  ///   3. import not mapped -> None
  pub fn resolve(
    &self,
    specifier: &str,
    referrer: &str,
  ) -> Result<Option<ModuleSpecifier>, ImportMapError> {
    let resolved_url: Option<Url> =
      ImportMap::try_url_like_specifier(specifier, referrer);
    let normalized_specifier = match &resolved_url {
      Some(url) => url.to_string(),
      None => specifier.to_string(),
    };

    let scopes_match = ImportMap::resolve_scopes_match(
      &self.scopes,
      &normalized_specifier,
      &referrer.to_string(),
    )?;

    // match found in scopes map
    if scopes_match.is_some() {
      return Ok(scopes_match);
    }

    let imports_match =
      ImportMap::resolve_imports_match(&self.imports, &normalized_specifier)?;

    // match found in import map
    if imports_match.is_some() {
      return Ok(imports_match);
    }

    // no match in import map but we got resolvable URL
    if let Some(resolved_url) = resolved_url {
      return Ok(Some(ModuleSpecifier::from(resolved_url)));
    }

    Err(ImportMapError::new(&format!(
      "Unmapped bare specifier {:?}",
      normalized_specifier
    )))
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use deno_core::serde_json::json;

  #[test]
  fn load_nonexistent() {
    let file_path = "nonexistent_import_map.json";
    assert!(ImportMap::load(file_path).is_err());
  }

  #[test]
  fn from_json_1() {
    let base_url = "https://deno.land";

    // empty JSON
    assert!(ImportMap::from_json(base_url, "{}").is_ok());

    let non_object_strings = vec!["null", "true", "1", "\"foo\"", "[]"];

    // invalid JSON
    for non_object in non_object_strings.to_vec() {
      assert!(ImportMap::from_json(base_url, non_object).is_err());
    }

    // invalid schema: 'imports' is non-object
    for non_object in non_object_strings.to_vec() {
      assert!(ImportMap::from_json(
        base_url,
        &format!("{{\"imports\": {}}}", non_object),
      )
      .is_err());
    }

    // invalid schema: 'scopes' is non-object
    for non_object in non_object_strings.to_vec() {
      assert!(ImportMap::from_json(
        base_url,
        &format!("{{\"scopes\": {}}}", non_object),
      )
      .is_err());
    }
  }

  #[test]
  fn from_json_2() {
    let json_map = r#"{
      "imports": {
        "foo": "https://example.com/1",
        "bar": ["https://example.com/2"],
        "fizz": null
      }
    }"#;
    let result = ImportMap::from_json("https://deno.land", json_map);
    assert!(result.is_ok());
  }

  #[test]
  fn parse_specifier_keys_relative() {
    // Should absolutize strings prefixed with ./, ../, or / into the corresponding URLs..
    let json_map = r#"{
      "imports": {
        "./foo": "/dotslash",
        "../foo": "/dotdotslash",
        "/foo": "/slash"
      }
    }"#;
    let import_map =
      ImportMap::from_json("https://base.example/path1/path2/path3", json_map)
        .unwrap();
    assert_eq!(
      import_map
        .imports
        .get("https://base.example/path1/path2/foo")
        .unwrap()[0],
      "https://base.example/dotslash".to_string()
    );
    assert_eq!(
      import_map
        .imports
        .get("https://base.example/path1/foo")
        .unwrap()[0],
      "https://base.example/dotdotslash".to_string()
    );
    assert_eq!(
      import_map.imports.get("https://base.example/foo").unwrap()[0],
      "https://base.example/slash".to_string()
    );

    // Should absolutize the literal strings ./, ../, or / with no suffix..
    let json_map = r#"{
      "imports": {
        "./": "/dotslash/",
        "../": "/dotdotslash/",
        "/": "/slash/"
      }
    }"#;
    let import_map =
      ImportMap::from_json("https://base.example/path1/path2/path3", json_map)
        .unwrap();
    assert_eq!(
      import_map
        .imports
        .get("https://base.example/path1/path2/")
        .unwrap()[0],
      "https://base.example/dotslash/".to_string()
    );
    assert_eq!(
      import_map
        .imports
        .get("https://base.example/path1/")
        .unwrap()[0],
      "https://base.example/dotdotslash/".to_string()
    );
    assert_eq!(
      import_map.imports.get("https://base.example/").unwrap()[0],
      "https://base.example/slash/".to_string()
    );

    // Should treat percent-encoded variants of ./, ../, or / as bare specifiers..
    let json_map = r#"{
      "imports": {
        "%2E/": "/dotSlash1/",
        "%2E%2E/": "/dotDotSlash1/",
        ".%2F": "/dotSlash2",
        "..%2F": "/dotDotSlash2",
        "%2F": "/slash2",
        "%2E%2F": "/dotSlash3",
        "%2E%2E%2F": "/dotDotSlash3"
      }
    }"#;
    let import_map =
      ImportMap::from_json("https://base.example/path1/path2/path3", json_map)
        .unwrap();
    assert_eq!(
      import_map.imports.get("%2E/").unwrap()[0],
      "https://base.example/dotSlash1/".to_string()
    );
    assert_eq!(
      import_map.imports.get("%2E%2E/").unwrap()[0],
      "https://base.example/dotDotSlash1/".to_string()
    );
    assert_eq!(
      import_map.imports.get(".%2F").unwrap()[0],
      "https://base.example/dotSlash2".to_string()
    );
    assert_eq!(
      import_map.imports.get("..%2F").unwrap()[0],
      "https://base.example/dotDotSlash2".to_string()
    );
    assert_eq!(
      import_map.imports.get("%2F").unwrap()[0],
      "https://base.example/slash2".to_string()
    );
    assert_eq!(
      import_map.imports.get("%2E%2F").unwrap()[0],
      "https://base.example/dotSlash3".to_string()
    );
    assert_eq!(
      import_map.imports.get("%2E%2E%2F").unwrap()[0],
      "https://base.example/dotDotSlash3".to_string()
    );
  }

  #[test]
  fn parse_specifier_keys_absolute() {
    // Should only accept absolute URL specifier keys with fetch schemes,.
    // treating others as bare specifiers.
    let json_map = r#"{
      "imports": {
        "file:///good": "/file",
        "http://good/": "/http/",
        "https://good/": "/https/",
        "about:bad": "/about",
        "blob:bad": "/blob",
        "data:bad": "/data",
        "filesystem:bad": "/filesystem",
        "ftp://bad/": "/ftp/",
        "import:bad": "/import",
        "mailto:bad": "/mailto",
        "javascript:bad": "/javascript",
        "wss:bad": "/wss"
      }
    }"#;
    let import_map =
      ImportMap::from_json("https://base.example/path1/path2/path3", json_map)
        .unwrap();
    assert_eq!(
      import_map.imports.get("http://good/").unwrap()[0],
      "https://base.example/http/".to_string()
    );
    assert_eq!(
      import_map.imports.get("https://good/").unwrap()[0],
      "https://base.example/https/".to_string()
    );
    assert_eq!(
      import_map.imports.get("file:///good").unwrap()[0],
      "https://base.example/file".to_string()
    );
    assert_eq!(
      import_map.imports.get("http://good/").unwrap()[0],
      "https://base.example/http/".to_string()
    );
    assert_eq!(
      import_map.imports.get("import:bad").unwrap()[0],
      "https://base.example/import".to_string()
    );
    assert_eq!(
      import_map.imports.get("mailto:bad").unwrap()[0],
      "https://base.example/mailto".to_string()
    );
    assert_eq!(
      import_map.imports.get("javascript:bad").unwrap()[0],
      "https://base.example/javascript".to_string()
    );
    assert_eq!(
      import_map.imports.get("wss:bad").unwrap()[0],
      "https://base.example/wss".to_string()
    );
    assert_eq!(
      import_map.imports.get("about:bad").unwrap()[0],
      "https://base.example/about".to_string()
    );
    assert_eq!(
      import_map.imports.get("blob:bad").unwrap()[0],
      "https://base.example/blob".to_string()
    );
    assert_eq!(
      import_map.imports.get("data:bad").unwrap()[0],
      "https://base.example/data".to_string()
    );

    // Should parse absolute URLs, treating unparseable ones as bare specifiers..
    let json_map = r#"{
      "imports": {
        "https://ex ample.org/": "/unparseable1/",
        "https://example.com:demo": "/unparseable2",
        "http://[www.example.com]/": "/unparseable3/",
        "https:example.org": "/invalidButParseable1/",
        "https://///example.com///": "/invalidButParseable2/",
        "https://example.net": "/prettyNormal/",
        "https://ex%41mple.com/": "/percentDecoding/",
        "https://example.com/%41": "/noPercentDecoding"
      }
    }"#;
    let import_map =
      ImportMap::from_json("https://base.example/path1/path2/path3", json_map)
        .unwrap();
    assert_eq!(
      import_map.imports.get("https://ex ample.org/").unwrap()[0],
      "https://base.example/unparseable1/".to_string()
    );
    assert_eq!(
      import_map.imports.get("https://example.com:demo").unwrap()[0],
      "https://base.example/unparseable2".to_string()
    );
    assert_eq!(
      import_map.imports.get("http://[www.example.com]/").unwrap()[0],
      "https://base.example/unparseable3/".to_string()
    );
    assert_eq!(
      import_map.imports.get("https://example.org/").unwrap()[0],
      "https://base.example/invalidButParseable1/".to_string()
    );
    assert_eq!(
      import_map.imports.get("https://example.com///").unwrap()[0],
      "https://base.example/invalidButParseable2/".to_string()
    );
    assert_eq!(
      import_map.imports.get("https://example.net/").unwrap()[0],
      "https://base.example/prettyNormal/".to_string()
    );
    assert_eq!(
      import_map.imports.get("https://example.com/").unwrap()[0],
      "https://base.example/percentDecoding/".to_string()
    );
    assert_eq!(
      import_map.imports.get("https://example.com/%41").unwrap()[0],
      "https://base.example/noPercentDecoding".to_string()
    );
  }

  #[test]
  fn parse_scope_keys_relative() {
    // Should work with no prefix..
    let json_map = r#"{
      "scopes": {
        "foo": {}
      }
    }"#;
    let import_map =
      ImportMap::from_json("https://base.example/path1/path2/path3", json_map)
        .unwrap();
    assert!(import_map
      .scopes
      .contains_key("https://base.example/path1/path2/foo"));

    // Should work with ./, ../, and / prefixes..
    let json_map = r#"{
      "scopes": {
        "./foo": {},
        "../foo": {},
        "/foo": {}
      }
    }"#;
    let import_map =
      ImportMap::from_json("https://base.example/path1/path2/path3", json_map)
        .unwrap();
    assert!(import_map
      .scopes
      .contains_key("https://base.example/path1/path2/foo"));
    assert!(import_map
      .scopes
      .contains_key("https://base.example/path1/foo"));
    assert!(import_map.scopes.contains_key("https://base.example/foo"));

    // Should work with /s, ?s, and #s..
    let json_map = r#"{
      "scopes": {
        "foo/bar?baz#qux": {}
      }
    }"#;
    let import_map =
      ImportMap::from_json("https://base.example/path1/path2/path3", json_map)
        .unwrap();
    assert!(import_map
      .scopes
      .contains_key("https://base.example/path1/path2/foo/bar?baz#qux"));

    // Should work with an empty string scope key..
    let json_map = r#"{
      "scopes": {
        "": {}
      }
    }"#;
    let import_map =
      ImportMap::from_json("https://base.example/path1/path2/path3", json_map)
        .unwrap();
    assert!(import_map
      .scopes
      .contains_key("https://base.example/path1/path2/path3"));

    // Should work with / suffixes..
    let json_map = r#"{
      "scopes": {
        "foo/": {},
        "./foo/": {},
        "../foo/": {},
        "/foo/": {},
        "/foo//": {}
      }
    }"#;
    let import_map =
      ImportMap::from_json("https://base.example/path1/path2/path3", json_map)
        .unwrap();
    assert!(import_map
      .scopes
      .contains_key("https://base.example/path1/path2/foo/"));
    assert!(import_map
      .scopes
      .contains_key("https://base.example/path1/path2/foo/"));
    assert!(import_map
      .scopes
      .contains_key("https://base.example/path1/foo/"));
    assert!(import_map.scopes.contains_key("https://base.example/foo/"));
    assert!(import_map.scopes.contains_key("https://base.example/foo//"));

    // Should deduplicate based on URL parsing rules..
    let json_map = r#"{
      "scopes": {
        "foo/\\": {},
        "foo//": {},
        "foo\\\\": {}
      }
    }"#;
    let import_map =
      ImportMap::from_json("https://base.example/path1/path2/path3", json_map)
        .unwrap();
    assert!(import_map
      .scopes
      .contains_key("https://base.example/path1/path2/foo//"));
    assert_eq!(import_map.scopes.len(), 1);
  }

  #[test]
  fn parse_scope_keys_absolute() {
    // Should only accept absolute URL scope keys with fetch schemes..
    let json_map = r#"{
      "scopes": {
        "http://good/": {},
        "https://good/": {},
        "file:///good": {},
        "about:bad": {},
        "blob:bad": {},
        "data:bad": {},
        "filesystem:bad": {},
        "ftp://bad/": {},
        "import:bad": {},
        "mailto:bad": {},
        "javascript:bad": {},
        "wss:bad": {}
      }
    }"#;
    let import_map =
      ImportMap::from_json("https://base.example/path1/path2/path3", json_map)
        .unwrap();
    assert!(import_map.scopes.contains_key("http://good/"));
    assert!(import_map.scopes.contains_key("https://good/"));
    assert!(import_map.scopes.contains_key("file:///good"));
    assert_eq!(import_map.scopes.len(), 3);

    // Should parse absolute URL scope keys, ignoring unparseable ones..
    let json_map = r#"{
      "scopes": {
        "https://ex ample.org/": {},
        "https://example.com:demo": {},
        "http://[www.example.com]/": {},
        "https:example.org": {},
        "https://///example.com///": {},
        "https://example.net": {},
        "https://ex%41mple.com/foo/": {},
        "https://example.com/%41": {}
      }
    }"#;
    let import_map =
      ImportMap::from_json("https://base.example/path1/path2/path3", json_map)
        .unwrap();
    // tricky case! remember we have a base URL
    assert!(import_map
      .scopes
      .contains_key("https://base.example/path1/path2/example.org"));
    assert!(import_map.scopes.contains_key("https://example.com///"));
    assert!(import_map.scopes.contains_key("https://example.net/"));
    assert!(import_map.scopes.contains_key("https://example.com/foo/"));
    assert!(import_map.scopes.contains_key("https://example.com/%41"));
    assert_eq!(import_map.scopes.len(), 5);
  }

  #[test]
  fn parse_addresses_relative_url_like() {
    // Should accept strings prefixed with ./, ../, or /..
    let json_map = r#"{
      "imports": {
        "dotSlash": "./foo",
        "dotDotSlash": "../foo",
        "slash": "/foo"
       }
    }"#;
    let import_map =
      ImportMap::from_json("https://base.example/path1/path2/path3", json_map)
        .unwrap();

    assert_eq!(
      import_map.imports.get("dotSlash").unwrap(),
      &vec!["https://base.example/path1/path2/foo".to_string()]
    );
    assert_eq!(
      import_map.imports.get("dotDotSlash").unwrap(),
      &vec!["https://base.example/path1/foo".to_string()]
    );
    assert_eq!(
      import_map.imports.get("slash").unwrap(),
      &vec!["https://base.example/foo".to_string()]
    );

    // Should accept the literal strings ./, ../, or / with no suffix..
    let json_map = r#"{
      "imports": {
        "dotSlash": "./",
        "dotDotSlash": "../",
        "slash": "/"
       }
    }"#;
    let import_map =
      ImportMap::from_json("https://base.example/path1/path2/path3", json_map)
        .unwrap();

    assert_eq!(
      import_map.imports.get("dotSlash").unwrap(),
      &vec!["https://base.example/path1/path2/".to_string()]
    );
    assert_eq!(
      import_map.imports.get("dotDotSlash").unwrap(),
      &vec!["https://base.example/path1/".to_string()]
    );
    assert_eq!(
      import_map.imports.get("slash").unwrap(),
      &vec!["https://base.example/".to_string()]
    );

    // Should ignore percent-encoded variants of ./, ../, or /..
    let json_map = r#"{
      "imports": {
        "dotSlash1": "%2E/",
        "dotDotSlash1": "%2E%2E/",
        "dotSlash2": ".%2F",
        "dotDotSlash2": "..%2F",
        "slash2": "%2F",
        "dotSlash3": "%2E%2F",
        "dotDotSlash3": "%2E%2E%2F"
      }
    }"#;
    let import_map =
      ImportMap::from_json("https://base.example/path1/path2/path3", json_map)
        .unwrap();

    assert!(import_map.imports.get("dotSlash1").unwrap().is_empty());
    assert!(import_map.imports.get("dotDotSlash1").unwrap().is_empty());
    assert!(import_map.imports.get("dotSlash2").unwrap().is_empty());
    assert!(import_map.imports.get("dotDotSlash2").unwrap().is_empty());
    assert!(import_map.imports.get("slash2").unwrap().is_empty());
    assert!(import_map.imports.get("dotSlash3").unwrap().is_empty());
    assert!(import_map.imports.get("dotDotSlash3").unwrap().is_empty());
  }

  #[test]
  fn parse_addresses_absolute_with_fetch_schemes() {
    // Should only accept absolute URL addresses with fetch schemes..
    let json_map = r#"{
      "imports": {
        "http": "http://good/",
        "https": "https://good/",
        "file": "file:///good",
        "about": "about:bad",
        "blob": "blob:bad",
        "data": "data:bad",
        "filesystem": "filesystem:bad",
        "ftp": "ftp://good/",
        "import": "import:bad",
        "mailto": "mailto:bad",
        "javascript": "javascript:bad",
        "wss": "wss:bad"
      }
    }"#;
    let import_map =
      ImportMap::from_json("https://base.example/path1/path2/path3", json_map)
        .unwrap();

    assert_eq!(
      import_map.imports.get("file").unwrap(),
      &vec!["file:///good".to_string()]
    );
    assert_eq!(
      import_map.imports.get("http").unwrap(),
      &vec!["http://good/".to_string()]
    );
    assert_eq!(
      import_map.imports.get("https").unwrap(),
      &vec!["https://good/".to_string()]
    );

    assert!(import_map.imports.get("about").unwrap().is_empty());
    assert!(import_map.imports.get("blob").unwrap().is_empty());
    assert!(import_map.imports.get("data").unwrap().is_empty());
    assert!(import_map.imports.get("filesystem").unwrap().is_empty());
    assert!(import_map.imports.get("ftp").unwrap().is_empty());
    assert!(import_map.imports.get("import").unwrap().is_empty());
    assert!(import_map.imports.get("mailto").unwrap().is_empty());
    assert!(import_map.imports.get("javascript").unwrap().is_empty());
    assert!(import_map.imports.get("wss").unwrap().is_empty());
  }

  #[test]
  fn parse_addresses_absolute_with_fetch_schemes_arrays() {
    // Should only accept absolute URL addresses with fetch schemes inside arrays..
    let json_map = r#"{
      "imports": {
        "http": ["http://good/"],
        "https": ["https://good/"],
        "file": ["file:///good"],
        "about": ["about:bad"],
        "blob": ["blob:bad"],
        "data": ["data:bad"],
        "filesystem": ["filesystem:bad"],
        "ftp": ["ftp://good/"],
        "import": ["import:bad"],
        "mailto": ["mailto:bad"],
        "javascript": ["javascript:bad"],
        "wss": ["wss:bad"]
      }
    }"#;
    let import_map =
      ImportMap::from_json("https://base.example/path1/path2/path3", json_map)
        .unwrap();

    assert_eq!(
      import_map.imports.get("file").unwrap(),
      &vec!["file:///good".to_string()]
    );
    assert_eq!(
      import_map.imports.get("http").unwrap(),
      &vec!["http://good/".to_string()]
    );
    assert_eq!(
      import_map.imports.get("https").unwrap(),
      &vec!["https://good/".to_string()]
    );

    assert!(import_map.imports.get("about").unwrap().is_empty());
    assert!(import_map.imports.get("blob").unwrap().is_empty());
    assert!(import_map.imports.get("data").unwrap().is_empty());
    assert!(import_map.imports.get("filesystem").unwrap().is_empty());
    assert!(import_map.imports.get("ftp").unwrap().is_empty());
    assert!(import_map.imports.get("import").unwrap().is_empty());
    assert!(import_map.imports.get("mailto").unwrap().is_empty());
    assert!(import_map.imports.get("javascript").unwrap().is_empty());
    assert!(import_map.imports.get("wss").unwrap().is_empty());
  }

  #[test]
  fn parse_addresses_unparseable() {
    // Should parse absolute URLs, ignoring unparseable ones..
    let json_map = r#"{
      "imports": {
        "unparseable1": "https://ex ample.org/",
        "unparseable2": "https://example.com:demo",
        "unparseable3": "http://[www.example.com]/",
        "invalidButParseable1": "https:example.org",
        "invalidButParseable2": "https://///example.com///",
        "prettyNormal": "https://example.net",
        "percentDecoding": "https://ex%41mple.com/",
        "noPercentDecoding": "https://example.com/%41"
      }
    }"#;
    let import_map =
      ImportMap::from_json("https://base.example/path1/path2/path3", json_map)
        .unwrap();

    assert_eq!(
      import_map.imports.get("invalidButParseable1").unwrap(),
      &vec!["https://example.org/".to_string()]
    );
    assert_eq!(
      import_map.imports.get("invalidButParseable2").unwrap(),
      &vec!["https://example.com///".to_string()]
    );
    assert_eq!(
      import_map.imports.get("prettyNormal").unwrap(),
      &vec!["https://example.net/".to_string()]
    );
    assert_eq!(
      import_map.imports.get("percentDecoding").unwrap(),
      &vec!["https://example.com/".to_string()]
    );
    assert_eq!(
      import_map.imports.get("noPercentDecoding").unwrap(),
      &vec!["https://example.com/%41".to_string()]
    );

    assert!(import_map.imports.get("unparseable1").unwrap().is_empty());
    assert!(import_map.imports.get("unparseable2").unwrap().is_empty());
    assert!(import_map.imports.get("unparseable3").unwrap().is_empty());
  }

  #[test]
  fn parse_addresses_unparseable_arrays() {
    // Should parse absolute URLs, ignoring unparseable ones inside arrays..
    let json_map = r#"{
      "imports": {
        "unparseable1": ["https://ex ample.org/"],
        "unparseable2": ["https://example.com:demo"],
        "unparseable3": ["http://[www.example.com]/"],
        "invalidButParseable1": ["https:example.org"],
        "invalidButParseable2": ["https://///example.com///"],
        "prettyNormal": ["https://example.net"],
        "percentDecoding": ["https://ex%41mple.com/"],
        "noPercentDecoding": ["https://example.com/%41"]
      }
    }"#;
    let import_map =
      ImportMap::from_json("https://base.example/path1/path2/path3", json_map)
        .unwrap();

    assert_eq!(
      import_map.imports.get("invalidButParseable1").unwrap(),
      &vec!["https://example.org/".to_string()]
    );
    assert_eq!(
      import_map.imports.get("invalidButParseable2").unwrap(),
      &vec!["https://example.com///".to_string()]
    );
    assert_eq!(
      import_map.imports.get("prettyNormal").unwrap(),
      &vec!["https://example.net/".to_string()]
    );
    assert_eq!(
      import_map.imports.get("percentDecoding").unwrap(),
      &vec!["https://example.com/".to_string()]
    );
    assert_eq!(
      import_map.imports.get("noPercentDecoding").unwrap(),
      &vec!["https://example.com/%41".to_string()]
    );

    assert!(import_map.imports.get("unparseable1").unwrap().is_empty());
    assert!(import_map.imports.get("unparseable2").unwrap().is_empty());
    assert!(import_map.imports.get("unparseable3").unwrap().is_empty());
  }

  #[test]
  fn parse_addresses_mismatched_trailing_slashes() {
    // Should parse absolute URLs, ignoring unparseable ones inside arrays..
    let json_map = r#"{
      "imports": {
        "trailer/": "/notrailer"
      }
    }"#;
    let import_map =
      ImportMap::from_json("https://base.example/path1/path2/path3", json_map)
        .unwrap();

    assert!(import_map.imports.get("trailer/").unwrap().is_empty());
    // TODO: I'd be good to assert that warning was shown
  }

  #[test]
  fn parse_addresses_mismatched_trailing_slashes_array() {
    // Should warn for a mismatch alone in an array..
    let json_map = r#"{
      "imports": {
        "trailer/": ["/notrailer"]
      }
    }"#;
    let import_map =
      ImportMap::from_json("https://base.example/path1/path2/path3", json_map)
        .unwrap();

    assert!(import_map.imports.get("trailer/").unwrap().is_empty());
    // TODO: I'd be good to assert that warning was shown
  }

  #[test]
  fn parse_addresses_mismatched_trailing_slashes_with_nonmismatched_array() {
    // Should warn for a mismatch alone in an array..
    let json_map = r#"{
      "imports": {
        "trailer/": ["/atrailer/", "/notrailer"]
      }
    }"#;
    let import_map =
      ImportMap::from_json("https://base.example/path1/path2/path3", json_map)
        .unwrap();

    assert_eq!(
      import_map.imports.get("trailer/").unwrap(),
      &vec!["https://base.example/atrailer/".to_string()]
    );
    // TODO: I'd be good to assert that warning was shown
  }

  #[test]
  fn parse_addresses_other_invalid() {
    // Should ignore unprefixed strings that are not absolute URLs.
    for bad in &["bar", "\\bar", "~bar", "#bar", "?bar"] {
      let json_map = json!({
        "imports": {
          "foo": bad
        }
      });
      let import_map = ImportMap::from_json(
        "https://base.example/path1/path2/path3",
        &json_map.to_string(),
      )
      .unwrap();

      assert!(import_map.imports.get("foo").unwrap().is_empty());
    }
  }

  fn get_empty_import_map() -> ImportMap {
    ImportMap {
      base_url: "https://example.com/app/main.ts".to_string(),
      imports: IndexMap::new(),
      scopes: IndexMap::new(),
    }
  }

  fn assert_resolve(
    result: Result<Option<ModuleSpecifier>, ImportMapError>,
    expected_url: &str,
  ) {
    let maybe_url = result
      .unwrap_or_else(|err| panic!("ImportMap::resolve failed: {:?}", err));
    let resolved_url =
      maybe_url.unwrap_or_else(|| panic!("Unexpected None resolved URL"));
    assert_eq!(resolved_url, expected_url.to_string());
  }

  #[test]
  fn resolve_unmapped_relative_specifiers() {
    let referrer_url = "https://example.com/js/script.ts";
    let import_map = get_empty_import_map();

    // Should resolve ./ specifiers as URLs.
    assert_resolve(
      import_map.resolve("./foo", referrer_url),
      "https://example.com/js/foo",
    );
    assert_resolve(
      import_map.resolve("./foo/bar", referrer_url),
      "https://example.com/js/foo/bar",
    );
    assert_resolve(
      import_map.resolve("./foo/../bar", referrer_url),
      "https://example.com/js/bar",
    );
    assert_resolve(
      import_map.resolve("./foo/../../bar", referrer_url),
      "https://example.com/bar",
    );

    // Should resolve ../ specifiers as URLs.
    assert_resolve(
      import_map.resolve("../foo", referrer_url),
      "https://example.com/foo",
    );
    assert_resolve(
      import_map.resolve("../foo/bar", referrer_url),
      "https://example.com/foo/bar",
    );
    assert_resolve(
      import_map.resolve("../../../foo/bar", referrer_url),
      "https://example.com/foo/bar",
    );
  }

  #[test]
  fn resolve_unmapped_absolute_specifiers() {
    let referrer_url = "https://example.com/js/script.ts";
    let import_map = get_empty_import_map();

    // Should resolve / specifiers as URLs.
    assert_resolve(
      import_map.resolve("/foo", referrer_url),
      "https://example.com/foo",
    );
    assert_resolve(
      import_map.resolve("/foo/bar", referrer_url),
      "https://example.com/foo/bar",
    );
    assert_resolve(
      import_map.resolve("../../foo/bar", referrer_url),
      "https://example.com/foo/bar",
    );
    assert_resolve(
      import_map.resolve("/../foo/../bar", referrer_url),
      "https://example.com/bar",
    );

    // Should parse absolute fetch-scheme URLs.
    assert_resolve(
      import_map.resolve("https://example.net", referrer_url),
      "https://example.net/",
    );
    assert_resolve(
      import_map.resolve("https://ex%41mple.com/", referrer_url),
      "https://example.com/",
    );
    assert_resolve(
      import_map.resolve("https:example.org", referrer_url),
      "https://example.org/",
    );
    assert_resolve(
      import_map.resolve("https://///example.com///", referrer_url),
      "https://example.com///",
    );
  }

  #[test]
  fn resolve_unmapped_bad_specifiers() {
    let referrer_url = "https://example.com/js/script.ts";
    let import_map = get_empty_import_map();

    // Should fail for absolute non-fetch-scheme URLs.
    assert!(import_map.resolve("about:good", referrer_url).is_err());
    assert!(import_map.resolve("mailto:bad", referrer_url).is_err());
    assert!(import_map.resolve("import:bad", referrer_url).is_err());
    assert!(import_map.resolve("javascript:bad", referrer_url).is_err());
    assert!(import_map.resolve("wss:bad", referrer_url).is_err());

    // Should fail for string not parseable as absolute URLs and not starting with ./, ../ or /.
    assert!(import_map.resolve("foo", referrer_url).is_err());
    assert!(import_map.resolve("\\foo", referrer_url).is_err());
    assert!(import_map.resolve(":foo", referrer_url).is_err());
    assert!(import_map.resolve("@foo", referrer_url).is_err());
    assert!(import_map.resolve("%2E/foo", referrer_url).is_err());
    assert!(import_map.resolve("%2E%2Efoo", referrer_url).is_err());
    assert!(import_map.resolve(".%2Efoo", referrer_url).is_err());
    assert!(import_map
      .resolve("https://ex ample.org", referrer_url)
      .is_err());
    assert!(import_map
      .resolve("https://example.org:deno", referrer_url)
      .is_err());
    assert!(import_map
      .resolve("https://[example.org]", referrer_url)
      .is_err());
  }

  #[test]
  fn resolve_imports_mapped() {
    let base_url = "https://example.com/app/main.ts";
    let referrer_url = "https://example.com/js/script.ts";

    // Should fail when mapping is to an empty array.
    let json_map = r#"{
    "imports": {
      "moment": null,
      "lodash": []
    }
  }"#;
    let import_map = ImportMap::from_json(base_url, json_map).unwrap();

    assert!(import_map.resolve("moment", referrer_url).is_err());
    assert!(import_map.resolve("lodash", referrer_url).is_err());
  }

  #[test]
  fn resolve_imports_package_like_modules() {
    let base_url = "https://example.com/app/main.ts";
    let referrer_url = "https://example.com/js/script.ts";

    let json_map = r#"{
    "imports": {
      "moment": "/deps/moment/src/moment.js",
      "moment/": "/deps/moment/src/",
      "lodash-dot": "./deps/lodash-es/lodash.js",
      "lodash-dot/": "./deps/lodash-es/",
      "lodash-dotdot": "../deps/lodash-es/lodash.js",
      "lodash-dotdot/": "../deps/lodash-es/",
      "nowhere/": []
    }
  }"#;
    let import_map = ImportMap::from_json(base_url, json_map).unwrap();

    // Should work for package main modules.
    assert_resolve(
      import_map.resolve("moment", referrer_url),
      "https://example.com/deps/moment/src/moment.js",
    );
    assert_resolve(
      import_map.resolve("lodash-dot", referrer_url),
      "https://example.com/app/deps/lodash-es/lodash.js",
    );
    assert_resolve(
      import_map.resolve("lodash-dotdot", referrer_url),
      "https://example.com/deps/lodash-es/lodash.js",
    );

    // Should work for package submodules.
    assert_resolve(
      import_map.resolve("moment/foo", referrer_url),
      "https://example.com/deps/moment/src/foo",
    );
    assert_resolve(
      import_map.resolve("lodash-dot/foo", referrer_url),
      "https://example.com/app/deps/lodash-es/foo",
    );
    assert_resolve(
      import_map.resolve("lodash-dotdot/foo", referrer_url),
      "https://example.com/deps/lodash-es/foo",
    );

    // Should work for package names that end in a slash.
    assert_resolve(
      import_map.resolve("moment/", referrer_url),
      "https://example.com/deps/moment/src/",
    );

    // Should fail for package modules that are not declared.
    assert!(import_map.resolve("underscore/", referrer_url).is_err());
    assert!(import_map.resolve("underscore/foo", referrer_url).is_err());

    // Should fail for package submodules that map to nowhere.
    assert!(import_map.resolve("nowhere/foo", referrer_url).is_err());
  }

  #[test]
  fn resolve_imports_tricky_specifiers() {
    let base_url = "https://example.com/app/main.ts";
    let referrer_url = "https://example.com/js/script.ts";

    let json_map = r#"{
    "imports": {
      "package/withslash": "/deps/package-with-slash/index.mjs",
      "not-a-package": "/lib/not-a-package.mjs",
      ".": "/lib/dot.mjs",
      "..": "/lib/dotdot.mjs",
      "..\\\\": "/lib/dotdotbackslash.mjs",
      "%2E": "/lib/percent2e.mjs",
      "%2F": "/lib/percent2f.mjs"
    }
  }"#;
    let import_map = ImportMap::from_json(base_url, json_map).unwrap();

    // Should work for explicitly-mapped specifiers that happen to have a slash.
    assert_resolve(
      import_map.resolve("package/withslash", referrer_url),
      "https://example.com/deps/package-with-slash/index.mjs",
    );

    // Should work when the specifier has punctuation.
    assert_resolve(
      import_map.resolve(".", referrer_url),
      "https://example.com/lib/dot.mjs",
    );
    assert_resolve(
      import_map.resolve("..", referrer_url),
      "https://example.com/lib/dotdot.mjs",
    );
    assert_resolve(
      import_map.resolve("..\\\\", referrer_url),
      "https://example.com/lib/dotdotbackslash.mjs",
    );
    assert_resolve(
      import_map.resolve("%2E", referrer_url),
      "https://example.com/lib/percent2e.mjs",
    );
    assert_resolve(
      import_map.resolve("%2F", referrer_url),
      "https://example.com/lib/percent2f.mjs",
    );

    // Should fail for attempting to get a submodule of something not declared with a trailing slash.
    assert!(import_map
      .resolve("not-a-package/foo", referrer_url)
      .is_err());
  }

  #[test]
  fn resolve_imports_url_like_specifier() {
    let base_url = "https://example.com/app/main.ts";
    let referrer_url = "https://example.com/js/script.ts";

    let json_map = r#"{
    "imports": {
      "/node_modules/als-polyfill/index.mjs": "std:kv-storage",
      "/lib/foo.mjs": "./more/bar.mjs",
      "./dotrelative/foo.mjs": "/lib/dot.mjs",
      "../dotdotrelative/foo.mjs": "/lib/dotdot.mjs",
      "/lib/no.mjs": null,
      "./dotrelative/no.mjs": [],
      "/": "/lib/slash-only/",
      "./": "/lib/dotslash-only/",
      "/test/": "/lib/url-trailing-slash/",
      "./test/": "/lib/url-trailing-slash-dot/",
      "/test": "/lib/test1.mjs",
      "../test": "/lib/test2.mjs"
    }
  }"#;
    let import_map = ImportMap::from_json(base_url, json_map).unwrap();

    // Should remap to other URLs.
    assert_resolve(
      import_map.resolve("https://example.com/lib/foo.mjs", referrer_url),
      "https://example.com/app/more/bar.mjs",
    );
    assert_resolve(
      import_map.resolve("https://///example.com/lib/foo.mjs", referrer_url),
      "https://example.com/app/more/bar.mjs",
    );
    assert_resolve(
      import_map.resolve("/lib/foo.mjs", referrer_url),
      "https://example.com/app/more/bar.mjs",
    );
    assert_resolve(
      import_map
        .resolve("https://example.com/app/dotrelative/foo.mjs", referrer_url),
      "https://example.com/lib/dot.mjs",
    );
    assert_resolve(
      import_map.resolve("../app/dotrelative/foo.mjs", referrer_url),
      "https://example.com/lib/dot.mjs",
    );
    assert_resolve(
      import_map
        .resolve("https://example.com/dotdotrelative/foo.mjs", referrer_url),
      "https://example.com/lib/dotdot.mjs",
    );
    assert_resolve(
      import_map.resolve("../dotdotrelative/foo.mjs", referrer_url),
      "https://example.com/lib/dotdot.mjs",
    );

    // Should fail for URLs that remap to empty arrays.
    assert!(import_map
      .resolve("https://example.com/lib/no.mjs", referrer_url)
      .is_err());
    assert!(import_map.resolve("/lib/no.mjs", referrer_url).is_err());
    assert!(import_map.resolve("../lib/no.mjs", referrer_url).is_err());
    assert!(import_map
      .resolve("https://example.com/app/dotrelative/no.mjs", referrer_url)
      .is_err());
    assert!(import_map
      .resolve("/app/dotrelative/no.mjs", referrer_url)
      .is_err());
    assert!(import_map
      .resolve("../app/dotrelative/no.mjs", referrer_url)
      .is_err());

    // Should remap URLs that are just composed from / and ..
    assert_resolve(
      import_map.resolve("https://example.com/", referrer_url),
      "https://example.com/lib/slash-only/",
    );
    assert_resolve(
      import_map.resolve("/", referrer_url),
      "https://example.com/lib/slash-only/",
    );
    assert_resolve(
      import_map.resolve("../", referrer_url),
      "https://example.com/lib/slash-only/",
    );
    assert_resolve(
      import_map.resolve("https://example.com/app/", referrer_url),
      "https://example.com/lib/dotslash-only/",
    );
    assert_resolve(
      import_map.resolve("/app/", referrer_url),
      "https://example.com/lib/dotslash-only/",
    );
    assert_resolve(
      import_map.resolve("../app/", referrer_url),
      "https://example.com/lib/dotslash-only/",
    );

    // Should remap URLs that are prefix-matched by keys with trailing slashes.
    assert_resolve(
      import_map.resolve("/test/foo.mjs", referrer_url),
      "https://example.com/lib/url-trailing-slash/foo.mjs",
    );
    assert_resolve(
      import_map.resolve("https://example.com/app/test/foo.mjs", referrer_url),
      "https://example.com/lib/url-trailing-slash-dot/foo.mjs",
    );

    // Should use the last entry's address when URL-like specifiers parse to the same absolute URL.
    //
    // NOTE: this works properly because of "preserve_order" feature flag to "serde_json" crate
    assert_resolve(
      import_map.resolve("/test", referrer_url),
      "https://example.com/lib/test2.mjs",
    );
  }

  #[test]
  fn resolve_imports_overlapping_entities_with_trailing_slashes() {
    let base_url = "https://example.com/app/main.ts";
    let referrer_url = "https://example.com/js/script.ts";

    // Should favor the most-specific key (no empty arrays).
    {
      let json_map = r#"{
      "imports": {
        "a": "/1",
        "a/": "/2/",
        "a/b": "/3",
        "a/b/": "/4/"
      }
    }"#;
      let import_map = ImportMap::from_json(base_url, json_map).unwrap();

      assert_resolve(
        import_map.resolve("a", referrer_url),
        "https://example.com/1",
      );
      assert_resolve(
        import_map.resolve("a/", referrer_url),
        "https://example.com/2/",
      );
      assert_resolve(
        import_map.resolve("a/b", referrer_url),
        "https://example.com/3",
      );
      assert_resolve(
        import_map.resolve("a/b/", referrer_url),
        "https://example.com/4/",
      );
      assert_resolve(
        import_map.resolve("a/b/c", referrer_url),
        "https://example.com/4/c",
      );
    }

    // Should favor the most-specific key when empty arrays are involved for less-specific keys.
    {
      let json_map = r#"{
      "imports": {
        "a": [],
        "a/": [],
        "a/b": "/3",
        "a/b/": "/4/"
      }
    }"#;
      let import_map = ImportMap::from_json(base_url, json_map).unwrap();

      assert!(import_map.resolve("a", referrer_url).is_err());
      assert!(import_map.resolve("a/", referrer_url).is_err());
      assert!(import_map.resolve("a/x", referrer_url).is_err());
      assert_resolve(
        import_map.resolve("a/b", referrer_url),
        "https://example.com/3",
      );
      assert_resolve(
        import_map.resolve("a/b/", referrer_url),
        "https://example.com/4/",
      );
      assert_resolve(
        import_map.resolve("a/b/c", referrer_url),
        "https://example.com/4/c",
      );
      assert!(import_map.resolve("a/x/c", referrer_url).is_err());
    }
  }

  #[test]
  fn resolve_scopes_map_to_empty_array() {
    let base_url = "https://example.com/app/main.ts";
    let referrer_url = "https://example.com/js";

    let json_map = r#"{
    "scopes": {
      "/js/": {
        "moment": "null",
        "lodash": []
      }
    }
  }"#;
    let import_map = ImportMap::from_json(base_url, json_map).unwrap();

    assert!(import_map.resolve("moment", referrer_url).is_err());
    assert!(import_map.resolve("lodash", referrer_url).is_err());
  }

  #[test]
  fn resolve_scopes_exact_vs_prefix_matching() {
    let base_url = "https://example.com/app/main.ts";

    let json_map = r#"{
    "scopes": {
      "/js": {
        "moment": "/only-triggered-by-exact/moment",
        "moment/": "/only-triggered-by-exact/moment/"
      },
      "/js/": {
        "moment": "/triggered-by-any-subpath/moment",
        "moment/": "/triggered-by-any-subpath/moment/"
      }
    }
  }"#;
    let import_map = ImportMap::from_json(base_url, json_map).unwrap();

    let js_non_dir = "https://example.com/js";
    let js_in_dir = "https://example.com/js/app.mjs";
    let with_js_prefix = "https://example.com/jsiscool";

    assert_resolve(
      import_map.resolve("moment", js_non_dir),
      "https://example.com/only-triggered-by-exact/moment",
    );
    assert_resolve(
      import_map.resolve("moment/foo", js_non_dir),
      "https://example.com/only-triggered-by-exact/moment/foo",
    );
    assert_resolve(
      import_map.resolve("moment", js_in_dir),
      "https://example.com/triggered-by-any-subpath/moment",
    );
    assert_resolve(
      import_map.resolve("moment/foo", js_in_dir),
      "https://example.com/triggered-by-any-subpath/moment/foo",
    );
    assert!(import_map.resolve("moment", with_js_prefix).is_err());
    assert!(import_map.resolve("moment/foo", with_js_prefix).is_err());
  }

  #[test]
  fn resolve_scopes_only_exact_in_map() {
    let base_url = "https://example.com/app/main.ts";

    let json_map = r#"{
    "scopes": {
      "/js": {
        "moment": "/only-triggered-by-exact/moment",
        "moment/": "/only-triggered-by-exact/moment/"
      }
    }
  }"#;
    let import_map = ImportMap::from_json(base_url, json_map).unwrap();

    // Should match correctly when only an exact match is in the map.
    let js_non_dir = "https://example.com/js";
    let js_in_dir = "https://example.com/js/app.mjs";
    let with_js_prefix = "https://example.com/jsiscool";

    assert_resolve(
      import_map.resolve("moment", js_non_dir),
      "https://example.com/only-triggered-by-exact/moment",
    );
    assert_resolve(
      import_map.resolve("moment/foo", js_non_dir),
      "https://example.com/only-triggered-by-exact/moment/foo",
    );
    assert!(import_map.resolve("moment", js_in_dir).is_err());
    assert!(import_map.resolve("moment/foo", js_in_dir).is_err());
    assert!(import_map.resolve("moment", with_js_prefix).is_err());
    assert!(import_map.resolve("moment/foo", with_js_prefix).is_err());
  }

  #[test]
  fn resolve_scopes_only_prefix_in_map() {
    let base_url = "https://example.com/app/main.ts";

    let json_map = r#"{
    "scopes": {
      "/js/": {
        "moment": "/triggered-by-any-subpath/moment",
        "moment/": "/triggered-by-any-subpath/moment/"
      }
    }
  }"#;
    let import_map = ImportMap::from_json(base_url, json_map).unwrap();

    // Should match correctly when only a prefix match is in the map.
    let js_non_dir = "https://example.com/js";
    let js_in_dir = "https://example.com/js/app.mjs";
    let with_js_prefix = "https://example.com/jsiscool";

    assert!(import_map.resolve("moment", js_non_dir).is_err());
    assert!(import_map.resolve("moment/foo", js_non_dir).is_err());
    assert_resolve(
      import_map.resolve("moment", js_in_dir),
      "https://example.com/triggered-by-any-subpath/moment",
    );
    assert_resolve(
      import_map.resolve("moment/foo", js_in_dir),
      "https://example.com/triggered-by-any-subpath/moment/foo",
    );
    assert!(import_map.resolve("moment", with_js_prefix).is_err());
    assert!(import_map.resolve("moment/foo", with_js_prefix).is_err());
  }

  #[test]
  fn resolve_scopes_package_like() {
    let base_url = "https://example.com/app/main.ts";

    let json_map = r#"{
    "imports": {
      "moment": "/node_modules/moment/src/moment.js",
      "moment/": "/node_modules/moment/src/",
      "lodash-dot": "./node_modules/lodash-es/lodash.js",
      "lodash-dot/": "./node_modules/lodash-es/",
      "lodash-dotdot": "../node_modules/lodash-es/lodash.js",
      "lodash-dotdot/": "../node_modules/lodash-es/"
    },
    "scopes": {
      "/": {
        "moment": "/node_modules_3/moment/src/moment.js",
        "vue": "/node_modules_3/vue/dist/vue.runtime.esm.js"
      },
      "/js/": {
        "lodash-dot": "./node_modules_2/lodash-es/lodash.js",
        "lodash-dot/": "./node_modules_2/lodash-es/",
        "lodash-dotdot": "../node_modules_2/lodash-es/lodash.js",
        "lodash-dotdot/": "../node_modules_2/lodash-es/"
      }
    }
  }"#;
    let import_map = ImportMap::from_json(base_url, json_map).unwrap();

    // Should match correctly when only a prefix match is in the map.
    let js_in_dir = "https://example.com/js/app.mjs";
    let top_level = "https://example.com/app.mjs";

    // Should resolve scoped.
    assert_resolve(
      import_map.resolve("lodash-dot", js_in_dir),
      "https://example.com/app/node_modules_2/lodash-es/lodash.js",
    );
    assert_resolve(
      import_map.resolve("lodash-dotdot", js_in_dir),
      "https://example.com/node_modules_2/lodash-es/lodash.js",
    );
    assert_resolve(
      import_map.resolve("lodash-dot/foo", js_in_dir),
      "https://example.com/app/node_modules_2/lodash-es/foo",
    );
    assert_resolve(
      import_map.resolve("lodash-dotdot/foo", js_in_dir),
      "https://example.com/node_modules_2/lodash-es/foo",
    );

    // Should apply best scope match.
    assert_resolve(
      import_map.resolve("moment", top_level),
      "https://example.com/node_modules_3/moment/src/moment.js",
    );
    assert_resolve(
      import_map.resolve("moment", js_in_dir),
      "https://example.com/node_modules_3/moment/src/moment.js",
    );
    assert_resolve(
      import_map.resolve("vue", js_in_dir),
      "https://example.com/node_modules_3/vue/dist/vue.runtime.esm.js",
    );

    // Should fallback to "imports".
    assert_resolve(
      import_map.resolve("moment/foo", top_level),
      "https://example.com/node_modules/moment/src/foo",
    );
    assert_resolve(
      import_map.resolve("moment/foo", js_in_dir),
      "https://example.com/node_modules/moment/src/foo",
    );
    assert_resolve(
      import_map.resolve("lodash-dot", top_level),
      "https://example.com/app/node_modules/lodash-es/lodash.js",
    );
    assert_resolve(
      import_map.resolve("lodash-dotdot", top_level),
      "https://example.com/node_modules/lodash-es/lodash.js",
    );
    assert_resolve(
      import_map.resolve("lodash-dot/foo", top_level),
      "https://example.com/app/node_modules/lodash-es/foo",
    );
    assert_resolve(
      import_map.resolve("lodash-dotdot/foo", top_level),
      "https://example.com/node_modules/lodash-es/foo",
    );

    // Should still fail for package-like specifiers that are not declared.
    assert!(import_map.resolve("underscore/", js_in_dir).is_err());
    assert!(import_map.resolve("underscore/foo", js_in_dir).is_err());
  }

  #[test]
  fn resolve_scopes_inheritance() {
    // https://github.com/WICG/import-maps#scope-inheritance
    let base_url = "https://example.com/app/main.ts";

    let json_map = r#"{
    "imports": {
      "a": "/a-1.mjs",
      "b": "/b-1.mjs",
      "c": "/c-1.mjs"
    },
    "scopes": {
      "/scope2/": {
        "a": "/a-2.mjs"
      },
      "/scope2/scope3/": {
        "b": "/b-3.mjs"
      }
    }
  }"#;
    let import_map = ImportMap::from_json(base_url, json_map).unwrap();

    let scope_1_url = "https://example.com/scope1/foo.mjs";
    let scope_2_url = "https://example.com/scope2/foo.mjs";
    let scope_3_url = "https://example.com/scope2/scope3/foo.mjs";

    // Should fall back to "imports" when none match.
    assert_resolve(
      import_map.resolve("a", scope_1_url),
      "https://example.com/a-1.mjs",
    );
    assert_resolve(
      import_map.resolve("b", scope_1_url),
      "https://example.com/b-1.mjs",
    );
    assert_resolve(
      import_map.resolve("c", scope_1_url),
      "https://example.com/c-1.mjs",
    );

    // Should use a direct scope override.
    assert_resolve(
      import_map.resolve("a", scope_2_url),
      "https://example.com/a-2.mjs",
    );
    assert_resolve(
      import_map.resolve("b", scope_2_url),
      "https://example.com/b-1.mjs",
    );
    assert_resolve(
      import_map.resolve("c", scope_2_url),
      "https://example.com/c-1.mjs",
    );

    // Should use an indirect scope override.
    assert_resolve(
      import_map.resolve("a", scope_3_url),
      "https://example.com/a-2.mjs",
    );
    assert_resolve(
      import_map.resolve("b", scope_3_url),
      "https://example.com/b-3.mjs",
    );
    assert_resolve(
      import_map.resolve("c", scope_3_url),
      "https://example.com/c-1.mjs",
    );
  }

  #[test]
  fn resolve_scopes_relative_url_keys() {
    // https://github.com/WICG/import-maps#scope-inheritance
    let base_url = "https://example.com/app/main.ts";

    let json_map = r#"{
    "imports": {
      "a": "/a-1.mjs",
      "b": "/b-1.mjs",
      "c": "/c-1.mjs"
    },
    "scopes": {
      "": {
        "a": "/a-empty-string.mjs"
      },
      "./": {
        "b": "/b-dot-slash.mjs"
      },
      "../": {
        "c": "/c-dot-dot-slash.mjs"
      }
    }
  }"#;
    let import_map = ImportMap::from_json(base_url, json_map).unwrap();
    let in_same_dir_as_map = "https://example.com/app/foo.mjs";
    let in_dir_above_map = "https://example.com/foo.mjs";

    // Should resolve an empty string scope using the import map URL.
    assert_resolve(
      import_map.resolve("a", base_url),
      "https://example.com/a-empty-string.mjs",
    );
    assert_resolve(
      import_map.resolve("a", in_same_dir_as_map),
      "https://example.com/a-1.mjs",
    );

    // Should resolve a ./ scope using the import map URL's directory.
    assert_resolve(
      import_map.resolve("b", base_url),
      "https://example.com/b-dot-slash.mjs",
    );
    assert_resolve(
      import_map.resolve("b", in_same_dir_as_map),
      "https://example.com/b-dot-slash.mjs",
    );

    // Should resolve a ../ scope using the import map URL's directory.
    assert_resolve(
      import_map.resolve("c", base_url),
      "https://example.com/c-dot-dot-slash.mjs",
    );
    assert_resolve(
      import_map.resolve("c", in_same_dir_as_map),
      "https://example.com/c-dot-dot-slash.mjs",
    );
    assert_resolve(
      import_map.resolve("c", in_dir_above_map),
      "https://example.com/c-dot-dot-slash.mjs",
    );
  }

  #[test]
  fn cant_resolve_to_built_in() {
    let base_url = "https://example.com/app/main.ts";

    let import_map = ImportMap::from_json(base_url, "{}").unwrap();

    assert!(import_map.resolve("std:blank", base_url).is_err());
  }

  #[test]
  fn resolve_builtins_remap() {
    let base_url = "https://example.com/app/main.ts";

    let json_map = r#"{
      "imports": {
        "std:blank": "./blank.mjs",
        "std:none": "./none.mjs"
      }
    }"#;
    let import_map = ImportMap::from_json(base_url, json_map).unwrap();

    assert_resolve(
      import_map.resolve("std:blank", base_url),
      "https://example.com/app/blank.mjs",
    );
    assert_resolve(
      import_map.resolve("std:none", base_url),
      "https://example.com/app/none.mjs",
    );
  }
}
