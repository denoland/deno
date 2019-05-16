use crate::worker::resolve_module_spec;
use serde_json::Map;
use serde_json::Value;
use std::collections::HashMap;
use url::Url;

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

    if let Ok(url) = resolve_module_spec(&specifier_key, base_url) {
      return Some(url.clone());
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
          if specifier_key.ends_with('/') && !address_url.ends_with('/') {
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
      if normalized_specifier == specifier_key {
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
      if specifier_key.ends_with('/')
        && normalized_specifier.starts_with(specifier_key)
      {
        if address_vec.is_empty() {
          println!("Specifier {:?} was mapped to no addresses (via prefix specifier key {:?}).", normalized_specifier, specifier_key);
          return None;
        } else if address_vec.len() == 1 {
          let address = address_vec.first().unwrap();
          let after_prefix = &normalized_specifier[specifier_key.len()..];

          if let Ok(base_url) = Url::parse(address) {
            if let Ok(url) = base_url.join(after_prefix) {
              let resolved_url = url.to_string();
              println!("Specifier {:?} was mapped to {:?} (via prefix specifier key {:?}).", normalized_specifier, resolved_url, address);
              return Some(resolved_url);
            }
          }

          // TODO(bartlomieju): can this happen?
          println!("Specifier {:?} was mapped via prefix specifier key {:?}, but is not resolvable.", normalized_specifier, address);
          return None;
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

    None
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
    None
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn parsing_import_map() {
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

  // TODO(bartlomieju): port tests from:
  //  https://github.com/WICG/import-maps/blob/master/reference-implementation/__tests__/resolving.js
  #[test]
  fn empty_import_map() {
    let base_url = "https://example.com/app/main.ts";
    let referrer_url = "https://example.com/js/script.ts";
    let import_map = ImportMap::new(base_url);

    // should resolve ./ specifiers as URLs
    {
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
    }

    // should resolve ../ specifiers as URLs
    {
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
    }

    // should resolve / specifiers as URLs
    {
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
    }

    // should parse absolute fetch-scheme URLs
    {
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
    }

    // TODO(bartlomieju): enable these tests
    // should fail for absolute non-fetch-scheme URLs
    // {
    // assert_eq!(import_map.resolve("mailto:bad", referrer_url), None);
    // assert_eq!(import_map.resolve("import:bad", referrer_url), None);
    // assert_eq!(import_map.resolve("javascript:bad", referrer_url), None);
    // assert_eq!(import_map.resolve("wss:bad", referrer_url), None);
    // }

    // should fail for string not parseable as absolute URLs and not starting with ./, ../ or /
    {
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
  }

  #[test]
  fn mapped_imports() {
    let base_url = "https://example.com/app/main.ts";
    let referrer_url = "https://example.com/js/script.ts";

    // should fail when mapping is to an empty array
    let json_map = json!({
      "imports": {
        "moment": null,
        "lodash": []
      }
    });
    let import_map =
      ImportMap::from_json(base_url, &json_map.to_string()).unwrap();

    assert_eq!(import_map.resolve("moment", referrer_url), None);
    assert_eq!(import_map.resolve("lodash", referrer_url), None);
  }

  #[test]
  fn package_like_modules() {
    let base_url = "https://example.com/app/main.ts";
    let referrer_url = "https://example.com/js/script.ts";

    let json_map = json!({
      "imports": {
        "moment": "/deps/moment/src/moment.js",
        "moment/": "/deps/moment/src/",
        "lodash-dot": "./deps/lodash-es/lodash.js",
        "lodash-dot/": "./deps/lodash-es/",
        "lodash-dotdot": "../deps/lodash-es/lodash.js",
        "lodash-dotdot/": "../deps/lodash-es/",
        "nowhere/": []
      }
    });
    let import_map =
      ImportMap::from_json(base_url, &json_map.to_string()).unwrap();

    // should work for package main modules
    {
      assert_eq!(
        import_map.resolve("moment", referrer_url),
        Some("https://example.com/deps/moment/src/moment.js".to_string())
      );
      assert_eq!(
        import_map.resolve("lodash-dot", referrer_url),
        Some("https://example.com/app/deps/lodash-es/lodash.js".to_string())
      );
      assert_eq!(
        import_map.resolve("lodash-dotdot", referrer_url),
        Some("https://example.com/deps/lodash-es/lodash.js".to_string())
      );
    }

    // should work for package submodules
    {
      assert_eq!(
        import_map.resolve("moment/foo", referrer_url),
        Some("https://example.com/deps/moment/src/foo".to_string())
      );
      assert_eq!(
        import_map.resolve("lodash-dot/foo", referrer_url),
        Some("https://example.com/app/deps/lodash-es/foo".to_string())
      );
      assert_eq!(
        import_map.resolve("lodash-dotdot/foo", referrer_url),
        Some("https://example.com/deps/lodash-es/foo".to_string())
      );
    }

    // should work for package names that end in a slash
    {
      assert_eq!(
        import_map.resolve("moment/", referrer_url),
        Some("https://example.com/deps/moment/src/".to_string())
      );
    }

    // should fail for package modules that are not declared
    {
      assert_eq!(import_map.resolve("underscore/", referrer_url), None);
      assert_eq!(import_map.resolve("underscore/foo", referrer_url), None);
    }

    // should fail for package submodules that map to nowhere
    {
      assert_eq!(import_map.resolve("nowhere/foo", referrer_url), None);
    }
  }

  #[test]
  fn tricky_specifiers() {
    let base_url = "https://example.com/app/main.ts";
    let referrer_url = "https://example.com/js/script.ts";

    let json_map = json!({
      "imports": {
        "package/withslash": "/deps/package-with-slash/index.mjs",
        "not-a-package": "/lib/not-a-package.mjs",
        ".": "/lib/dot.mjs",
        "..": "/lib/dotdot.mjs",
        "..\\\\": "/lib/dotdotbackslash.mjs",
        "%2E": "/lib/percent2e.mjs",
        "%2F": "/lib/percent2f.mjs"
      }
    });
    let import_map =
      ImportMap::from_json(base_url, &json_map.to_string()).unwrap();

    // should work for explicitly-mapped specifiers that happen to have a slash
    {
      assert_eq!(
        import_map.resolve("package/withslash", referrer_url),
        Some(
          "https://example.com/deps/package-with-slash/index.mjs".to_string()
        )
      );
    }

    // should work when the specifier has punctuation
    {
      assert_eq!(
        import_map.resolve(".", referrer_url),
        Some("https://example.com/lib/dot.mjs".to_string())
      );
      assert_eq!(
        import_map.resolve("..", referrer_url),
        Some("https://example.com/lib/dotdot.mjs".to_string())
      );
      assert_eq!(
        import_map.resolve("..\\\\", referrer_url),
        Some("https://example.com/lib/dotdotbackslash.mjs".to_string())
      );
      assert_eq!(
        import_map.resolve("%2E", referrer_url),
        Some("https://example.com/lib/percent2e.mjs".to_string())
      );
      assert_eq!(
        import_map.resolve("%2F", referrer_url),
        Some("https://example.com/lib/percent2f.mjs".to_string())
      );
    }

    // should fail for attempting to get a submodule of something not declared with a trailing slash
    {
      assert_eq!(import_map.resolve("not-a-package/foo", referrer_url), None);
    }
  }

  #[test]
  fn url_like_specifier() {
    // TODO(bartlomieju): in order for this test to pass we need to implement longest-match in map
    //  ie. iterate over self.modules sorted by longest keys
    return;

    let base_url = "https://example.com/app/main.ts";
    let referrer_url = "https://example.com/js/script.ts";

    let json_map = json!({
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
    });
    let import_map =
      ImportMap::from_json(base_url, &json_map.to_string()).unwrap();

    // should remap to other URLs
    {
      assert_eq!(
        import_map.resolve("https://example.com/lib/foo.mjs", referrer_url),
        Some("https://example.com/app/more/bar.mjs".to_string())
      );
      assert_eq!(
        import_map.resolve("https://///example.com/lib/foo.mjs", referrer_url),
        Some("https://example.com/app/more/bar.mjs".to_string())
      );
      assert_eq!(
        import_map.resolve("/lib/foo.mjs", referrer_url),
        Some("https://example.com/app/more/bar.mjs".to_string())
      );
      assert_eq!(
        import_map
          .resolve("https://example.com/app/dotrelative/foo.mjs", referrer_url),
        Some("https://example.com/lib/dot.mjs".to_string())
      );
      assert_eq!(
        import_map.resolve("../app/dotrelative/foo.mjs", referrer_url),
        Some("https://example.com/lib/dot.mjs".to_string())
      );
      assert_eq!(
        import_map
          .resolve("https://example.com/dotdotrelative/foo.mjs", referrer_url),
        Some("https://example.com/lib/dotdot.mjs".to_string())
      );
      assert_eq!(
        import_map.resolve("../dotdotrelative/foo.mjs", referrer_url),
        Some("https://example.com/lib/dotdot.mjs".to_string())
      );
    }
  }
}
