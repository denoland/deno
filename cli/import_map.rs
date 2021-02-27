// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use deno_core::serde::Serialize;
use deno_core::serde_json;
use deno_core::serde_json::Map;
use deno_core::serde_json::Value;
use deno_core::url::Url;
use indexmap::IndexMap;
use std::cmp::Ordering;
use std::collections::HashSet;
use std::error::Error;
use std::fmt;

#[derive(Debug)]
pub struct ImportMapError(String);

impl fmt::Display for ImportMapError {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    f.pad(&self.0)
  }
}

impl Error for ImportMapError {}

// https://url.spec.whatwg.org/#special-scheme
const SPECIAL_PROTOCOLS: &[&str] =
  &["ftp", "file", "http", "https", "ws", "wss"];
fn is_special(url: &Url) -> bool {
  SPECIAL_PROTOCOLS.contains(&url.scheme())
}

type SpecifierMap = IndexMap<String, Option<Url>>;
type ScopesMap = IndexMap<String, SpecifierMap>;

#[derive(Debug, Clone, Serialize)]
pub struct ImportMap {
  #[serde(skip)]
  base_url: String,

  imports: SpecifierMap,
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
        return Err(ImportMapError(
          "Unable to parse import map JSON".to_string(),
        ));
      }
    };

    match v {
      Value::Object(_) => {}
      _ => {
        return Err(ImportMapError(
          "Import map JSON must be an object".to_string(),
        ));
      }
    }

    let normalized_imports = match &v.get("imports") {
      Some(imports_map) => {
        if !imports_map.is_object() {
          return Err(ImportMapError(
            "Import map's 'imports' must be an object".to_string(),
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
          return Err(ImportMapError(
            "Import map's 'scopes' must be an object".to_string(),
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
      eprintln!("Invalid top-level key \"{}\". Only \"imports\" and \"scopes\" can be present.", key);
    }

    let import_map = ImportMap {
      base_url: base_url.to_string(),
      imports: normalized_imports,
      scopes: normalized_scopes,
    };

    Ok(import_map)
  }

  fn try_url_like_specifier(specifier: &str, base: &str) -> Option<Url> {
    if specifier.starts_with('/')
      || specifier.starts_with("./")
      || specifier.starts_with("../")
    {
      if let Ok(base_url) = Url::parse(base) {
        if let Ok(url) = base_url.join(specifier) {
          return Some(url);
        }
      }
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
      eprintln!("Invalid empty string specifier.");
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
          eprintln!("Invalid address {:#?} for the specifier key \"{}\". Addresses must be strings.", value, specifier_key);
          normalized_map.insert(normalized_specifier_key, None);
          continue;
        }
      };

      let address_url =
        match ImportMap::try_url_like_specifier(&potential_address, base_url) {
          Some(url) => url,
          None => {
            eprintln!(
              "Invalid address \"{}\" for the specifier key \"{}\".",
              potential_address, specifier_key
            );
            normalized_map.insert(normalized_specifier_key, None);
            continue;
          }
        };

      let address_url_string = address_url.to_string();
      if specifier_key.ends_with('/') && !address_url_string.ends_with('/') {
        eprintln!(
          "Invalid target address {:?} for package specifier {:?}. \
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
        return Err(ImportMapError(format!(
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
            eprintln!(
              "Invalid scope \"{}\" (parsed against base URL \"{}\").",
              scope_prefix, base_url
            );
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

  fn resolve_scopes_match(
    scopes: &ScopesMap,
    normalized_specifier: &str,
    as_url: Option<&Url>,
    referrer: &str,
  ) -> Result<Option<Url>, ImportMapError> {
    // exact-match
    if let Some(scope_imports) = scopes.get(referrer) {
      let scope_match = ImportMap::resolve_imports_match(
        scope_imports,
        normalized_specifier,
        as_url,
      )?;
      // Return only if there was actual match (not None).
      if scope_match.is_some() {
        return Ok(scope_match);
      }
    }

    for (normalized_scope_key, scope_imports) in scopes.iter() {
      if normalized_scope_key.ends_with('/')
        && referrer.starts_with(normalized_scope_key)
      {
        let scope_match = ImportMap::resolve_imports_match(
          scope_imports,
          normalized_specifier,
          as_url,
        )?;
        // Return only if there was actual match (not None).
        if scope_match.is_some() {
          return Ok(scope_match);
        }
      }
    }

    Ok(None)
  }

  fn resolve_imports_match(
    specifier_map: &SpecifierMap,
    normalized_specifier: &str,
    as_url: Option<&Url>,
  ) -> Result<Option<Url>, ImportMapError> {
    // exact-match
    if let Some(maybe_address) = specifier_map.get(normalized_specifier) {
      if let Some(address) = maybe_address {
        return Ok(Some(address.clone()));
      } else {
        return Err(ImportMapError(format!(
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
        return Err(ImportMapError(format!(
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
          return Err(ImportMapError(format!(
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
        return Err(ImportMapError(format!(
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
  ) -> Result<Option<Url>, ImportMapError> {
    let as_url: Option<Url> =
      ImportMap::try_url_like_specifier(specifier, referrer);
    let normalized_specifier = if let Some(url) = as_url.as_ref() {
      url.to_string()
    } else {
      specifier.to_string()
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

    // The specifier was able to be turned into a URL, but wasn't remapped into anything.
    if as_url.is_some() {
      return Ok(as_url);
    }

    Err(ImportMapError(format!(
      "Unmapped bare specifier {:?}",
      specifier
    )))
  }
}

#[cfg(test)]
mod tests {

  use super::*;
  use deno_core::resolve_import;
  use std::path::Path;
  use std::path::PathBuf;
  use walkdir::WalkDir;

  #[derive(Debug)]
  enum TestKind {
    Resolution {
      given_specifier: String,
      expected_specifier: Option<String>,
      base_url: String,
    },
    Parse {
      expected_import_map: Value,
    },
  }

  #[derive(Debug)]
  struct ImportMapTestCase {
    name: String,
    import_map: String,
    import_map_base_url: String,
    kind: TestKind,
  }

  fn load_import_map_wpt_tests() -> Vec<String> {
    let mut found_test_files = vec![];
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let import_map_wpt_path =
      repo_root.join("test_util/wpt/import-maps/data-driven/resources");
    eprintln!("import map wpt path {:#?}", import_map_wpt_path);
    for entry in WalkDir::new(import_map_wpt_path)
      .contents_first(true)
      .into_iter()
      .filter_entry(|e| {
        eprintln!("entry {:#?}", e);
        if let Some(ext) = e.path().extension() {
          return ext.to_string_lossy() == "json";
        }
        false
      })
      .filter_map(|e| match e {
        Ok(e) => Some(e),
        _ => None,
      })
      .map(|e| PathBuf::from(e.path()))
    {
      found_test_files.push(entry);
    }

    let mut file_contents = vec![];

    for file in found_test_files {
      let content = std::fs::read_to_string(file).unwrap();
      file_contents.push(content);
    }

    file_contents
  }

  fn parse_import_map_tests(test_str: &str) -> Vec<ImportMapTestCase> {
    let json_file: serde_json::Value = serde_json::from_str(test_str).unwrap();
    let maybe_name = json_file
      .get("name")
      .map(|s| s.as_str().unwrap().to_string());
    return parse_test_object(&json_file, maybe_name, None, None, None, None);

    fn parse_test_object(
      test_obj: &Value,
      maybe_name_prefix: Option<String>,
      maybe_import_map: Option<String>,
      maybe_base_url: Option<String>,
      maybe_import_map_base_url: Option<String>,
      maybe_expected_import_map: Option<Value>,
    ) -> Vec<ImportMapTestCase> {
      let maybe_import_map_base_url =
        if let Some(base_url) = test_obj.get("importMapBaseURL") {
          Some(base_url.as_str().unwrap().to_string())
        } else {
          maybe_import_map_base_url
        };

      let maybe_base_url = if let Some(base_url) = test_obj.get("baseURL") {
        Some(base_url.as_str().unwrap().to_string())
      } else {
        maybe_base_url
      };

      let maybe_expected_import_map =
        if let Some(im) = test_obj.get("expectedParsedImportMap") {
          Some(im.to_owned())
        } else {
          maybe_expected_import_map
        };

      let maybe_import_map = if let Some(import_map) = test_obj.get("importMap")
      {
        Some(if import_map.is_string() {
          import_map.as_str().unwrap().to_string()
        } else {
          serde_json::to_string(import_map).unwrap()
        })
      } else {
        maybe_import_map
      };

      if let Some(nested_tests) = test_obj.get("tests") {
        let nested_tests_obj = nested_tests.as_object().unwrap();
        let mut collected = vec![];
        for (name, test_obj) in nested_tests_obj {
          let nested_name = if let Some(ref name_prefix) = maybe_name_prefix {
            format!("{}: {}", name_prefix, name)
          } else {
            name.to_string()
          };
          let parsed_nested_tests = parse_test_object(
            test_obj,
            Some(nested_name),
            maybe_import_map.clone(),
            maybe_base_url.clone(),
            maybe_import_map_base_url.clone(),
            maybe_expected_import_map.clone(),
          );
          collected.extend(parsed_nested_tests)
        }
        return collected;
      }

      let mut collected_cases = vec![];
      if let Some(results) = test_obj.get("expectedResults") {
        let expected_results = results.as_object().unwrap();
        for (given, expected) in expected_results {
          let name = if let Some(ref name_prefix) = maybe_name_prefix {
            format!("{}: {}", name_prefix, given)
          } else {
            given.to_string()
          };
          let given_specifier = given.to_string();
          let expected_specifier = expected.as_str().map(|str| str.to_string());

          let test_case = ImportMapTestCase {
            name,
            import_map_base_url: maybe_import_map_base_url.clone().unwrap(),
            import_map: maybe_import_map.clone().unwrap(),
            kind: TestKind::Resolution {
              given_specifier,
              expected_specifier,
              base_url: maybe_base_url.clone().unwrap(),
            },
          };

          collected_cases.push(test_case);
        }
      } else if let Some(expected_import_map) = maybe_expected_import_map {
        let test_case = ImportMapTestCase {
          name: maybe_name_prefix.unwrap(),
          import_map_base_url: maybe_import_map_base_url.unwrap(),
          import_map: maybe_import_map.unwrap(),
          kind: TestKind::Parse {
            expected_import_map,
          },
        };

        collected_cases.push(test_case);
      } else {
        eprintln!("unreachable {:#?}", test_obj);
        unreachable!();
      }

      collected_cases
    }
  }

  fn run_import_map_test_cases(tests: Vec<ImportMapTestCase>) {
    for test in tests {
      match &test.kind {
        TestKind::Resolution {
          given_specifier,
          expected_specifier,
          base_url,
        } => {
          let import_map =
            ImportMap::from_json(&test.import_map_base_url, &test.import_map)
              .unwrap();
          let maybe_resolved = import_map
            .resolve(&given_specifier, &base_url)
            .ok()
            .map(|maybe_resolved| {
              if let Some(specifier) = maybe_resolved {
                specifier.to_string()
              } else {
                resolve_import(&given_specifier, &base_url)
                  .unwrap()
                  .to_string()
              }
            });
          assert_eq!(expected_specifier, &maybe_resolved, "{}", test.name);
        }
        TestKind::Parse {
          expected_import_map,
        } => {
          if matches!(expected_import_map, Value::Null) {
            assert!(ImportMap::from_json(
              &test.import_map_base_url,
              &test.import_map
            )
            .is_err());
          } else {
            let import_map =
              ImportMap::from_json(&test.import_map_base_url, &test.import_map)
                .unwrap();
            let import_map_value = serde_json::to_value(import_map).unwrap();
            assert_eq!(expected_import_map, &import_map_value, "{}", test.name);
          }
        }
      }
    }
  }

  #[test]
  fn wpt() {
    let test_file_contents = load_import_map_wpt_tests();
    eprintln!("Found test files {}", test_file_contents.len());

    for test_file in test_file_contents {
      let tests = parse_import_map_tests(&test_file);
      run_import_map_test_cases(tests);
    }
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
}
