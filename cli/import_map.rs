use crate::worker::resolve_module_spec;
use serde_json::Map;
use serde_json::Value;
use std::collections::HashMap;

#[derive(Debug)]
pub struct ImportMapError {
  msg: String,
}

impl ImportMapError {
  pub fn new(msg: &str) -> Self {
    ImportMapError {
      msg: msg.to_string(),
    }
  }
}

#[allow(dead_code)]
#[derive(Debug)]
pub struct ImportMap {
  base_url: String,
  pub modules: HashMap<String, Vec<String>>,
}

#[allow(dead_code)]
impl ImportMap {
  // TODO(bartlomieju): add reading JSON string
  // TODO: add proper error handling: https://github.com/WICG/import-maps/issues/100
  pub fn new(base_url: &str) -> Self {
    ImportMap {
      base_url: base_url.to_string(),
      modules: HashMap::new(),
    }
  }

  fn normalize_specifier_key(
    specifier_key: &str,
    base_url: &str,
  ) -> Option<String> {
    // ignore empty keys
    if specifier_key.is_empty() {
      return None;
    }

    match resolve_module_spec(&specifier_key, base_url) {
      Ok(url) => {
        return Some(url.clone());
      }
      _ => {}
    }

    // "bare" specifier
    Some(specifier_key.to_string())
  }

  fn normalize_addresses(
    specifier_key: &str,
    base_url: &str,
    address_value: Value,
  ) -> Vec<String> {
    let potential_addresses: Vec<Value> = match address_value {
      Value::String(_) => vec![address_value],
      Value::Array(address_array) => address_array,
      Value::Null => vec![],
      _ => vec![],
    };

    let mut normalized_addresses: Vec<String> = vec![];

    for address in potential_addresses.iter() {
      let potential_address = match address {
        Value::String(address) => address,
        _ => continue,
      };
      match resolve_module_spec(potential_address, base_url) {
        Ok(address_url) => {
          if specifier_key.ends_with("/") && !address_url.ends_with("/") {
            println!(
              "Invalid target address `{:?}` for package specifier `{:?}`.\
               Package address targets must end with `/`.",
              address_url, specifier_key
            );
            continue;
          }

          normalized_addresses.push(address_url);
        }
        _ => continue,
      }
    }

    normalized_addresses
  }

  fn normalize_specifier_map(
    imports_map: &Map<String, Value>,
    base_url: &str,
  ) -> HashMap<String, Vec<String>> {
    let mut normalized_map: HashMap<String, Vec<String>> = HashMap::new();

    for (specifier_key, value) in imports_map.iter() {
      let normalized_specifier_key =
        match ImportMap::normalize_specifier_key(specifier_key, base_url) {
          Some(s) => s,
          None => continue,
        };

      let normalized_address_array = ImportMap::normalize_addresses(
        &normalized_specifier_key,
        base_url,
        value.clone(),
      );

      normalized_map.insert(normalized_specifier_key, normalized_address_array);
    }

    normalized_map
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

    let normalized_imports = match &v["imports"] {
      Value::Object(imports_map) => {
        ImportMap::normalize_specifier_map(imports_map, base_url)
      }
      _ => {
        return Err(ImportMapError::new(
          "Import map's 'imports' must be an object",
        ));
      }
    };

    // TODO(bartlomieju): handle scopes

    let import_map = ImportMap {
      base_url: base_url.to_string(),
      modules: normalized_imports,
    };

    Ok(import_map)
  }

  // TODO(bartlomieju): this method can definitely be optimized
  // https://github.com/WICG/import-maps/issues/73#issuecomment-439327758
  // for some candidate implementations.

  // TODO(bartlomieju):
  // "most-specific wins", i.e. when there are multiple matching keys,
  // choose the longest.
  // https://github.com/WICG/import-maps/issues/102
  fn resolve_imports_match(
    &self,
    normalized_specifier: String,
  ) -> Option<String> {
    for (specifier_key, address_vec) in self.modules.iter() {
      // exact-match
      if normalized_specifier == specifier_key.to_string() {
        if address_vec.is_empty() {
          println!(
            "Specifier {:?} was mapped to no addresses.",
            normalized_specifier
          );
          return None;
        } else if address_vec.len() == 1 {
          let address = address_vec.first().unwrap();
          println!(
            "Specifier {:?} was mapped to {:?}.",
            normalized_specifier, address
          );
          // TODO(bartlomieju): ensure that it's a valid URL
          return Some(address.to_string());
        } else {
          println!("Multi-address mappings are not yet supported");
          return None;
        }
      }

      // package-prefix match
      if specifier_key.ends_with("/")
        && normalized_specifier.starts_with(specifier_key)
      {
        if address_vec.is_empty() {
          println!("Specifier {:?} was mapped to no addresses (via prefix specifier key {:?}).", normalized_specifier, specifier_key);
          return None;
        } else if address_vec.len() == 1 {
          let address = address_vec.first().unwrap();
          let after_prefix = &normalized_specifier[specifier_key.len()..];

          match resolve_module_spec(after_prefix, &address) {
            Ok(resolved_url) => {
              let resolved_url = resolved_url.to_string();
              println!("Specifier {:?} was mapped to {:?} (via prefix specifier key {:?}).", normalized_specifier, resolved_url, address);
              return Some(resolved_url);
            }
            Err(_) => {
              println!("Specifier {:?} was mapped via prefix specifier key {:?}, but is not resolvable.", normalized_specifier, address);
              return None;
            }
          };
        } else {
          println!("Multi-address mappings are not yet supported");
          return None;
        }
      }
    }

    println!(
      "Specifier {:?} was not mapped in import map.",
      normalized_specifier
    );
    return None;
  }

  /// Currently we support two types of specifiers: URL (http://, https://, file://)
  /// and "bare" (moment, jquery, lodash)
  pub fn resolve(&self, specifier: &str, referrer: &str) -> Option<String> {
    let resolved_url: Option<String> =
      match resolve_module_spec(specifier, referrer) {
        Ok(url) => Some(url.clone()),
        Err(_) => None,
      };
    let normalized_specifier = match &resolved_url {
      Some(url) => url.clone(),
      None => specifier.to_string(),
    };

    // TODO: handle scopes

    let imports_match =
      self.resolve_imports_match(normalized_specifier.clone());

    // match found in import map
    if imports_match.is_some() {
      return imports_match;
    }

    // no match in import map but we got resolvable URL
    if resolved_url.is_some() {
      // TODO(bartlomieju): verify `resolved_url` scheme in (http://, https://, file://)
      return resolved_url;
    }

    println!("Unmapped bare specifier {:?}", normalized_specifier);
    return None;
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  // TODO(bartlomieju): port tests from:
  //  https://github.com/WICG/import-maps/blob/master/reference-implementation/__tests__/resolving.js
  #[test]
  fn test_empty_import_map() {
    let base_url = "https://example.com/app/main.ts";
    let referrer_url = "https://example.com/js/script.ts";
    let import_map = ImportMap::new(base_url);

    // should resolve ./ specifiers as URLs
    assert_eq!(
      import_map.resolve("./foo", referrer_url),
      Some("https://example.com/js/foo".to_string())
    );
    assert_eq!(
      import_map.resolve("./foo/bar", referrer_url),
      Some("https://example.com/js/foo/bar".to_string())
    );
    assert_eq!(
      import_map.resolve("./foo/../bar", referrer_url),
      Some("https://example.com/js/bar".to_string())
    );
    assert_eq!(
      import_map.resolve("./foo/../../bar", referrer_url),
      Some("https://example.com/bar".to_string())
    );

    // should resolve ../ specifiers as URLs
    assert_eq!(
      import_map.resolve("../foo", referrer_url),
      Some("https://example.com/foo".to_string())
    );
    assert_eq!(
      import_map.resolve("../foo/bar", referrer_url),
      Some("https://example.com/foo/bar".to_string())
    );
    assert_eq!(
      import_map.resolve("../../../foo/bar", referrer_url),
      Some("https://example.com/foo/bar".to_string())
    );

    // should resolve / specifiers as URLs
    assert_eq!(
      import_map.resolve("/foo", referrer_url),
      Some("https://example.com/foo".to_string())
    );
    assert_eq!(
      import_map.resolve("/foo/bar", referrer_url),
      Some("https://example.com/foo/bar".to_string())
    );
    assert_eq!(
      import_map.resolve("../../foo/bar", referrer_url),
      Some("https://example.com/foo/bar".to_string())
    );
    assert_eq!(
      import_map.resolve("/../foo/../bar", referrer_url),
      Some("https://example.com/bar".to_string())
    );

    // should parse absolute fetch-scheme URLs
    assert_eq!(
      import_map.resolve("about:good", referrer_url),
      Some("about:good".to_string())
    );
    assert_eq!(
      import_map.resolve("https://example.net", referrer_url),
      Some("https://example.net/".to_string())
    );
    assert_eq!(
      import_map.resolve("https://ex%41mple.com/", referrer_url),
      Some("https://example.com/".to_string())
    );
    assert_eq!(
      import_map.resolve("https:example.org", referrer_url),
      Some("https://example.org/".to_string())
    );
    assert_eq!(
      import_map.resolve("https://///example.com///", referrer_url),
      Some("https://example.com///".to_string())
    );

    // TODO(bartlomieju): enable these tests
    // should fail for absolute non-fetch-scheme URLs
    // assert_eq!(import_map.resolve("mailto:bad", referrer_url), None);
    // assert_eq!(import_map.resolve("import:bad", referrer_url), None);
    // assert_eq!(import_map.resolve("javascript:bad", referrer_url), None);
    // assert_eq!(import_map.resolve("wss:bad", referrer_url), None);

    // should fail for string not parseable as absolute URLs and not starting with ./, ../ or /
    assert_eq!(import_map.resolve("foo", referrer_url), None);
    assert_eq!(import_map.resolve("\\foo", referrer_url), None);
    assert_eq!(import_map.resolve(":foo", referrer_url), None);
    assert_eq!(import_map.resolve("@foo", referrer_url), None);
    assert_eq!(import_map.resolve("%2E/foo", referrer_url), None);
    assert_eq!(import_map.resolve("%2E%2Efoo", referrer_url), None);
    assert_eq!(import_map.resolve(".%2Efoo", referrer_url), None);
    assert_eq!(
      import_map.resolve("https://ex ample.org", referrer_url),
      None
    );
    assert_eq!(
      import_map.resolve("https://example.org:deno", referrer_url),
      None
    );
    assert_eq!(
      import_map.resolve("https://[example.org]", referrer_url),
      None
    );
  }

  #[test]
  fn test_parsing_import_map() {
    let base_url = "https://deno.land";

    {
      // invalid JSON
      assert!(ImportMap::from_json(base_url, "{}").is_err());
      assert!(ImportMap::from_json(base_url, "null").is_err());
      assert!(ImportMap::from_json(base_url, "true").is_err());
      assert!(ImportMap::from_json(base_url, "1").is_err());
      assert!(ImportMap::from_json(base_url, "\"foo\"").is_err());
      assert!(ImportMap::from_json(base_url, "[]").is_err());
    }

    {
      // invalid schema: 'imports' is non-object
      assert!(ImportMap::from_json(base_url, "{\"imports\": null}").is_err());
      assert!(ImportMap::from_json(base_url, "{\"imports\": true}").is_err());
      assert!(ImportMap::from_json(base_url, "{\"imports\": 1}").is_err());
      assert!(ImportMap::from_json(base_url, "{\"imports\": \"foo\"").is_err());
      assert!(ImportMap::from_json(base_url, "{\"imports\": []}").is_err());
    }

    {
      let json_map = json!({
        "imports": {
          "foo": "https://example.com/1",
          "bar": ["https://example.com/2"],
          "fizz": null
        }
      });
      let result = ImportMap::from_json(base_url, &json_map.to_string());
      eprintln!("import map result {:?}", result);
      assert!(result.is_ok());
    }
  }
}
