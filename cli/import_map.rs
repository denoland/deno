use crate::worker::resolve_module_spec;
use indexmap::IndexMap;
use serde_json::Map;
use serde_json::Value;
use std::cmp::Ordering;
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

type SpecifierMap = IndexMap<String, Vec<String>>;
type ScopesMap = IndexMap<String, SpecifierMap>;

#[allow(dead_code)]
#[derive(Debug)]
pub struct ImportMap {
  base_url: String,
  imports: SpecifierMap,
  scopes: ScopesMap,
}

#[allow(dead_code)]
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
        ImportMap::normalize_specifier_map(imports_map, base_url)
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
        match ImportMap::normalize_scope_map(scope_map, base_url) {
          Ok(scopes_map) => scopes_map,
          Err(err) => return Err(err),
        }
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

  // TODO: add proper error handling: https://github.com/WICG/import-maps/issues/100
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

  /// Addreses returned from this method are proper URLs
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

      if let Ok(address_url) = resolve_module_spec(potential_address, base_url)
      {
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
    }

    normalized_addresses
  }

  fn normalize_specifier_map(
    json_map: &Map<String, Value>,
    base_url: &str,
  ) -> SpecifierMap {
    let mut normalized_map: SpecifierMap = SpecifierMap::new();

    // order is preserved because of "preserve_order" feature of "serde_json"
    for (specifier_key, value) in json_map.iter() {
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

      debug!(
        "normalized specifier {:?}; {:?}",
        normalized_specifier_key, normalized_address_array
      );
      normalized_map.insert(normalized_specifier_key, normalized_address_array);
    }

    // now sort in longest and alphabetical order
    normalized_map.sort_by(|k1, _v1, k2, _v2| {
      if k1.len() > k2.len() {
        return Ordering::Less;
      } else if k2.len() > k1.len() {
        return Ordering::Greater;
      }

      k2.cmp(k1)
    });

    normalized_map
  }

  // TODO(bartlomieju): factor our SpecifierMap type (IndexMap<String, Vec<String>)?
  fn normalize_scope_map(
    scope_map: &Map<String, Value>,
    base_url: &str,
  ) -> Result<ScopesMap, ImportMapError> {
    // using IndexMap to preserve order during iteration
    let mut normalized_map: ScopesMap = ScopesMap::new();

    // order is preserved because of "preserve_order" feature of "serde_json"
    for (scope_prefix, potential_specifier_map) in scope_map.iter() {
      if !potential_specifier_map.is_object() {
        return Err(ImportMapError::new(&format!(
          "The value for the \"{:?}\" scope prefix must be an object",
          scope_prefix
        )));
      }

      let potential_specifier_map =
        potential_specifier_map.as_object().unwrap();

      let scope_prefix_url =
        match Url::parse(base_url).unwrap().join(scope_prefix) {
          Ok(url) => url.to_string(),
          _ => continue,
        };

      // TODO: handle bad "fetch_schemes"
      //      if (!hasFetchScheme(scopePrefixURL)) {
      //        console.warn(`Invalid scope "${scopePrefixURL}". Scope URLs must have a fetch scheme.`);
      //        continue;
      //      }

      let norm_map =
        ImportMap::normalize_specifier_map(potential_specifier_map, base_url);
      println!("normalized scope map {:?} {:?}", scope_prefix_url, norm_map);

      normalized_map.insert(scope_prefix_url, norm_map);
    }

    // now sort in longest and alphabetical order
    normalized_map.sort_by(|k1, _v1, k2, _v2| {
      if k1.len() > k2.len() {
        return Ordering::Less;
      } else if k2.len() > k1.len() {
        return Ordering::Greater;
      }

      k2.cmp(k1)
    });

    Ok(normalized_map)
  }

  // TODO: I get a feeling that we should be able to
  // return only Option
  pub fn resolve_scopes_match(
    scopes: &ScopesMap,
    normalized_specifier: &String,
    referrer: &String,
  ) -> Result<Option<String>, ImportMapError> {
    // exact-match
    println!(
      "resolve_scopes_match {:?}\n {:?} \n{:?}",
      normalized_specifier, referrer, scopes
    );
    if let Some(scope_imports) = scopes.get(referrer) {
      println!(
        "\n\nscope_imports {:?} {:?}\n\n",
        normalized_specifier, scope_imports
      );
      if let Ok(scope_match) =
        ImportMap::resolve_imports_match(scope_imports, normalized_specifier)
      {
        println!(
          "exact scope match {:?} {:?}",
          normalized_specifier, scope_match
        );
        return Ok(scope_match);
      }
    }

    for (normalized_scope_key, scope_imports) in scopes.iter() {
      println!(
        "\n\nfoo scope_imports {:?} {:?} {:?}\n\n",
        referrer, normalized_scope_key, scope_imports
      );
      if normalized_scope_key.ends_with('/')
        && referrer.starts_with(normalized_scope_key)
      {
        println!("\n\nfound scope match trying to find scope import match {:?} {:?} {:?}\n\n", referrer, normalized_scope_key, scope_imports);
        if let Ok(scope_match) =
          ImportMap::resolve_imports_match(scope_imports, normalized_specifier)
        {
          println!(
            "prefix scope match {:?} {:?}",
            normalized_scope_key, scope_match
          );
          return Ok(scope_match);
        }
      }
    }

    Ok(None)
  }

  // https://github.com/WICG/import-maps/issues/73#issuecomment-439327758
  // for some more optimized candidate implementations.
  pub fn resolve_imports_match(
    imports: &SpecifierMap,
    normalized_specifier: &String,
  ) -> Result<Option<String>, ImportMapError> {
    // exact-match
    if let Some(address_vec) = imports.get(normalized_specifier) {
      if address_vec.is_empty() {
        println!("match with empty array {:?}", normalized_specifier);
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
        // TODO(bartlomieju): ensure that it's a valid URL
        return Ok(Some(address.to_string()));
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

          if let Ok(base_url) = Url::parse(address) {
            if let Ok(url) = base_url.join(after_prefix) {
              let resolved_url = url.to_string();
              debug!("Specifier {:?} was mapped to {:?} (via prefix specifier key {:?}).", normalized_specifier, resolved_url, address);
              return Ok(Some(resolved_url));
            }
          }

          unreachable!();
        // TODO: implement built-in module notice here
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
  ) -> Result<Option<String>, ImportMapError> {
    println!("resolve! {:?}; {:?};", specifier, referrer);
    let resolved_url: Option<String> =
      match resolve_module_spec(specifier, referrer) {
        Ok(url) => Some(url.clone()),
        Err(_) => None,
      };
    let normalized_specifier = match &resolved_url {
      Some(url) => url.clone(),
      None => specifier.to_string(),
    };

    println!(
      "resolve normalized! {:?}; {:?};",
      normalized_specifier, referrer
    );
    // TODO: referrer should be parsed URL?
    let scopes_match = match ImportMap::resolve_scopes_match(
      &self.scopes,
      &normalized_specifier,
      &referrer.to_string(),
    ) {
      Ok(m) => m,
      Err(e) => return Err(e),
    };
    // match found in scopes map
    if scopes_match.is_some() {
      return Ok(scopes_match);
    }

    let imports_match = match ImportMap::resolve_imports_match(
      &self.imports,
      &normalized_specifier,
    ) {
      Ok(m) => m,
      Err(e) => return Err(e),
    };

    // match found in import map
    if imports_match.is_some() {
      return Ok(imports_match);
    }

    // no match in import map but we got resolvable URL
    if resolved_url.is_some() {
      // TODO(bartlomieju): verify `resolved_url` scheme in (http://, https://, file://)
      return Ok(resolved_url);
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

  #[test]
  fn import_map_parsing() {
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
      assert!(
        ImportMap::from_json(
          base_url,
          &format!("{{\"imports\": {}}}", non_object),
        ).is_err()
      );
    }

    // invalid schema: 'scopes' is non-object
    for non_object in non_object_strings.to_vec() {
      assert!(
        ImportMap::from_json(
          base_url,
          &format!("{{\"scopes\": {}}}", non_object),
        ).is_err()
      );
    }
  }

  #[test]
  fn import_map_from_json() {
    let json_map = json!({
      "imports": {
        "foo": "https://example.com/1",
        "bar": ["https://example.com/2"],
        "fizz": null
      }
    });
    let result =
      ImportMap::from_json("https://deno.land", &json_map.to_string());
    assert!(result.is_ok());
  }

  fn get_empty_import_map() -> ImportMap {
    ImportMap {
      base_url: "https://example.com/app/main.ts".to_string(),
      imports: IndexMap::new(),
      scopes: IndexMap::new(),
    }
  }

  #[test]
  fn empty_import_map_relative_specifiers() {
    let referrer_url = "https://example.com/js/script.ts";
    let import_map = get_empty_import_map();

    // should resolve ./ specifiers as URLs
    assert_eq!(
      import_map.resolve("./foo", referrer_url).unwrap(),
      Some("https://example.com/js/foo".to_string())
    );
    assert_eq!(
      import_map.resolve("./foo/bar", referrer_url).unwrap(),
      Some("https://example.com/js/foo/bar".to_string())
    );
    assert_eq!(
      import_map.resolve("./foo/../bar", referrer_url).unwrap(),
      Some("https://example.com/js/bar".to_string())
    );
    assert_eq!(
      import_map.resolve("./foo/../../bar", referrer_url).unwrap(),
      Some("https://example.com/bar".to_string())
    );

    // should resolve ../ specifiers as URLs
    assert_eq!(
      import_map.resolve("../foo", referrer_url).unwrap(),
      Some("https://example.com/foo".to_string())
    );
    assert_eq!(
      import_map.resolve("../foo/bar", referrer_url).unwrap(),
      Some("https://example.com/foo/bar".to_string())
    );
    assert_eq!(
      import_map
        .resolve("../../../foo/bar", referrer_url)
        .unwrap(),
      Some("https://example.com/foo/bar".to_string())
    );
  }

  #[test]
  fn empty_import_map_abolute_specifiers() {
    let referrer_url = "https://example.com/js/script.ts";
    let import_map = get_empty_import_map();

    // should resolve / specifiers as URLs
    assert_eq!(
      import_map.resolve("/foo", referrer_url).unwrap(),
      Some("https://example.com/foo".to_string())
    );
    assert_eq!(
      import_map.resolve("/foo/bar", referrer_url).unwrap(),
      Some("https://example.com/foo/bar".to_string())
    );
    assert_eq!(
      import_map.resolve("../../foo/bar", referrer_url).unwrap(),
      Some("https://example.com/foo/bar".to_string())
    );
    assert_eq!(
      import_map.resolve("/../foo/../bar", referrer_url).unwrap(),
      Some("https://example.com/bar".to_string())
    );

    // should parse absolute fetch-scheme URLs
    assert_eq!(
      import_map.resolve("about:good", referrer_url).unwrap(),
      Some("about:good".to_string())
    );
    assert_eq!(
      import_map
        .resolve("https://example.net", referrer_url)
        .unwrap(),
      Some("https://example.net/".to_string())
    );
    assert_eq!(
      import_map
        .resolve("https://ex%41mple.com/", referrer_url)
        .unwrap(),
      Some("https://example.com/".to_string())
    );
    assert_eq!(
      import_map
        .resolve("https:example.org", referrer_url)
        .unwrap(),
      Some("https://example.org/".to_string())
    );
    assert_eq!(
      import_map
        .resolve("https://///example.com///", referrer_url)
        .unwrap(),
      Some("https://example.com///".to_string())
    );
  }

  #[test]
  fn bad_specifiers() {
    let referrer_url = "https://example.com/js/script.ts";
    let import_map = get_empty_import_map();

    // TODO(bartlomieju): enable these tests
    // should fail for absolute non-fetch-scheme URLs
    // {
    // assert_eq!(import_map.resolve("mailto:bad", referrer_url), None);
    // assert_eq!(import_map.resolve("import:bad", referrer_url), None);
    // assert_eq!(import_map.resolve("javascript:bad", referrer_url), None);
    // assert_eq!(import_map.resolve("wss:bad", referrer_url), None);
    // }

    // should fail for string not parseable as absolute URLs and not starting with ./, ../ or /
    assert!(import_map.resolve("foo", referrer_url).is_err());
    assert!(import_map.resolve("\\foo", referrer_url).is_err());
    assert!(import_map.resolve(":foo", referrer_url).is_err());
    assert!(import_map.resolve("@foo", referrer_url).is_err());
    assert!(import_map.resolve("%2E/foo", referrer_url).is_err());
    assert!(import_map.resolve("%2E%2Efoo", referrer_url).is_err());
    assert!(import_map.resolve(".%2Efoo", referrer_url).is_err());
    assert!(
      import_map
        .resolve("https://ex ample.org", referrer_url)
        .is_err()
    );
    assert!(
      import_map
        .resolve("https://example.org:deno", referrer_url)
        .is_err()
    );
    assert!(
      import_map
        .resolve("https://[example.org]", referrer_url)
        .is_err()
    );
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

    assert!(import_map.resolve("moment", referrer_url).is_err());
    assert!(import_map.resolve("lodash", referrer_url).is_err());
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
    assert_eq!(
      import_map.resolve("moment", referrer_url).unwrap(),
      Some("https://example.com/deps/moment/src/moment.js".to_string())
    );
    assert_eq!(
      import_map.resolve("lodash-dot", referrer_url).unwrap(),
      Some("https://example.com/app/deps/lodash-es/lodash.js".to_string())
    );
    assert_eq!(
      import_map.resolve("lodash-dotdot", referrer_url).unwrap(),
      Some("https://example.com/deps/lodash-es/lodash.js".to_string())
    );

    // should work for package submodules
    assert_eq!(
      import_map.resolve("moment/foo", referrer_url).unwrap(),
      Some("https://example.com/deps/moment/src/foo".to_string())
    );
    assert_eq!(
      import_map.resolve("lodash-dot/foo", referrer_url).unwrap(),
      Some("https://example.com/app/deps/lodash-es/foo".to_string())
    );
    assert_eq!(
      import_map
        .resolve("lodash-dotdot/foo", referrer_url)
        .unwrap(),
      Some("https://example.com/deps/lodash-es/foo".to_string())
    );

    // should work for package names that end in a slash
    assert_eq!(
      import_map.resolve("moment/", referrer_url).unwrap(),
      Some("https://example.com/deps/moment/src/".to_string())
    );

    // should fail for package modules that are not declared
    assert!(import_map.resolve("underscore/", referrer_url).is_err());
    assert!(import_map.resolve("underscore/foo", referrer_url).is_err());

    // should fail for package submodules that map to nowhere
    assert!(import_map.resolve("nowhere/foo", referrer_url).is_err());
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
    assert_eq!(
      import_map
        .resolve("package/withslash", referrer_url)
        .unwrap(),
      Some("https://example.com/deps/package-with-slash/index.mjs".to_string())
    );

    // should work when the specifier has punctuation
    assert_eq!(
      import_map.resolve(".", referrer_url).unwrap(),
      Some("https://example.com/lib/dot.mjs".to_string())
    );
    assert_eq!(
      import_map.resolve("..", referrer_url).unwrap(),
      Some("https://example.com/lib/dotdot.mjs".to_string())
    );
    assert_eq!(
      import_map.resolve("..\\\\", referrer_url).unwrap(),
      Some("https://example.com/lib/dotdotbackslash.mjs".to_string())
    );
    assert_eq!(
      import_map.resolve("%2E", referrer_url).unwrap(),
      Some("https://example.com/lib/percent2e.mjs".to_string())
    );
    assert_eq!(
      import_map.resolve("%2F", referrer_url).unwrap(),
      Some("https://example.com/lib/percent2f.mjs".to_string())
    );

    // should fail for attempting to get a submodule of something not declared with a trailing slash
    assert!(
      import_map
        .resolve("not-a-package/foo", referrer_url)
        .is_err()
    );
  }

  #[test]
  fn url_like_specifier() {
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
    assert_eq!(
      import_map
        .resolve("https://example.com/lib/foo.mjs", referrer_url)
        .unwrap(),
      Some("https://example.com/app/more/bar.mjs".to_string())
    );
    assert_eq!(
      import_map
        .resolve("https://///example.com/lib/foo.mjs", referrer_url)
        .unwrap(),
      Some("https://example.com/app/more/bar.mjs".to_string())
    );
    assert_eq!(
      import_map.resolve("/lib/foo.mjs", referrer_url).unwrap(),
      Some("https://example.com/app/more/bar.mjs".to_string())
    );
    assert_eq!(
      import_map
        .resolve("https://example.com/app/dotrelative/foo.mjs", referrer_url)
        .unwrap(),
      Some("https://example.com/lib/dot.mjs".to_string())
    );
    assert_eq!(
      import_map
        .resolve("../app/dotrelative/foo.mjs", referrer_url)
        .unwrap(),
      Some("https://example.com/lib/dot.mjs".to_string())
    );
    assert_eq!(
      import_map
        .resolve("https://example.com/dotdotrelative/foo.mjs", referrer_url)
        .unwrap(),
      Some("https://example.com/lib/dotdot.mjs".to_string())
    );
    assert_eq!(
      import_map
        .resolve("../dotdotrelative/foo.mjs", referrer_url)
        .unwrap(),
      Some("https://example.com/lib/dotdot.mjs".to_string())
    );

    // should fail for URLs that remap to empty arrays
    assert!(
      import_map
        .resolve("https://example.com/lib/no.mjs", referrer_url)
        .is_err()
    );
    assert!(import_map.resolve("/lib/no.mjs", referrer_url).is_err());
    assert!(import_map.resolve("../lib/no.mjs", referrer_url).is_err());
    assert!(
      import_map
        .resolve("https://example.com/app/dotrelative/no.mjs", referrer_url)
        .is_err()
    );
    assert!(
      import_map
        .resolve("/app/dotrelative/no.mjs", referrer_url)
        .is_err()
    );
    assert!(
      import_map
        .resolve("../app/dotrelative/no.mjs", referrer_url)
        .is_err()
    );

    // should remap URLs that are just composed from / and .
    assert_eq!(
      import_map
        .resolve("https://example.com/", referrer_url)
        .unwrap(),
      Some("https://example.com/lib/slash-only/".to_string())
    );
    assert_eq!(
      import_map.resolve("/", referrer_url).unwrap(),
      Some("https://example.com/lib/slash-only/".to_string())
    );
    assert_eq!(
      import_map.resolve("../", referrer_url).unwrap(),
      Some("https://example.com/lib/slash-only/".to_string())
    );
    assert_eq!(
      import_map
        .resolve("https://example.com/app/", referrer_url)
        .unwrap(),
      Some("https://example.com/lib/dotslash-only/".to_string())
    );
    assert_eq!(
      import_map.resolve("/app/", referrer_url).unwrap(),
      Some("https://example.com/lib/dotslash-only/".to_string())
    );
    assert_eq!(
      import_map.resolve("../app/", referrer_url).unwrap(),
      Some("https://example.com/lib/dotslash-only/".to_string())
    );

    // should remap URLs that are prefix-matched by keys with trailing slashes
    assert_eq!(
      import_map.resolve("/test/foo.mjs", referrer_url).unwrap(),
      Some("https://example.com/lib/url-trailing-slash/foo.mjs".to_string())
    );
    assert_eq!(
      import_map
        .resolve("https://example.com/app/test/foo.mjs", referrer_url)
        .unwrap(),
      Some(
        "https://example.com/lib/url-trailing-slash-dot/foo.mjs".to_string()
      )
    );

    // should use the last entry's address when URL-like specifiers parse to the same absolute URL
    //
    // NOTE: this works properly because of "preserve_order" feature flag to "serde_json" crate
    assert_eq!(
      import_map.resolve("/test", referrer_url).unwrap(),
      Some("https://example.com/lib/test2.mjs".to_string())
    );
  }

  #[test]
  fn overlapping_entities_with_trailing_slashes() {
    let base_url = "https://example.com/app/main.ts";
    let referrer_url = "https://example.com/js/script.ts";

    // should favor the most-specific key (no empty arrays)
    {
      let json_map = json!({
        "imports": {
          "a": "/1",
          "a/": "/2/",
          "a/b": "/3",
          "a/b/": "/4/"
        }
      });
      let import_map =
        ImportMap::from_json(base_url, &json_map.to_string()).unwrap();

      assert_eq!(
        import_map.resolve("a", referrer_url).unwrap(),
        Some("https://example.com/1".to_string())
      );
      assert_eq!(
        import_map.resolve("a/", referrer_url).unwrap(),
        Some("https://example.com/2/".to_string())
      );
      assert_eq!(
        import_map.resolve("a/b", referrer_url).unwrap(),
        Some("https://example.com/3".to_string())
      );
      assert_eq!(
        import_map.resolve("a/b/", referrer_url).unwrap(),
        Some("https://example.com/4/".to_string())
      );
      assert_eq!(
        import_map.resolve("a/b/c", referrer_url).unwrap(),
        Some("https://example.com/4/c".to_string())
      );
    }

    // should favor the most-specific key when empty arrays are involved for less-specific keys
    {
      let json_map = json!({
        "imports": {
          "a": [],
          "a/": [],
          "a/b": "/3",
          "a/b/": "/4/"
        }
      });
      let import_map =
        ImportMap::from_json(base_url, &json_map.to_string()).unwrap();

      assert!(import_map.resolve("a", referrer_url).is_err());
      assert!(import_map.resolve("a/", referrer_url).is_err());
      assert!(import_map.resolve("a/x", referrer_url).is_err());
      assert_eq!(
        import_map.resolve("a/b", referrer_url).unwrap(),
        Some("https://example.com/3".to_string())
      );
      assert_eq!(
        import_map.resolve("a/b/", referrer_url).unwrap(),
        Some("https://example.com/4/".to_string())
      );
      assert_eq!(
        import_map.resolve("a/b/c", referrer_url).unwrap(),
        Some("https://example.com/4/c".to_string())
      );
      assert!(import_map.resolve("a/x/c", referrer_url).is_err());
    }
  }

  #[test]
  fn scopes_map_to_empty_array() {
    let base_url = "https://example.com/app/main.ts";
    let referrer_url = "https://example.com/js";

    let json_map = json!({
      "scopes": {
        "/js/": {
          "moment": "null",
          "lodash": []
        }
      }
    });
    let import_map =
      ImportMap::from_json(base_url, &json_map.to_string()).unwrap();

    // should remap to other URLs
    assert!(import_map.resolve("moment", referrer_url).is_err());
    assert!(import_map.resolve("lodash", referrer_url).is_err());
  }

  #[test]
  fn scopes_2() {
    let base_url = "https://example.com/app/main.ts";

    let json_map = json!({
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
    });
    let import_map =
      ImportMap::from_json(base_url, &json_map.to_string()).unwrap();

    // should match correctly when both are in the map
    let js_non_dir = "https://example.com/js";
    let js_in_dir = "https://example.com/js/app.mjs";
    let with_js_prefix = "https://example.com/jsiscool";

    assert_eq!(
      import_map.resolve("moment", js_non_dir).unwrap(),
      Some("https://example.com/only-triggered-by-exact/moment".to_string())
    );
    assert_eq!(
      import_map.resolve("moment/foo", js_non_dir).unwrap(),
      Some(
        "https://example.com/only-triggered-by-exact/moment/foo".to_string()
      )
    );
    assert_eq!(
      import_map.resolve("moment", js_in_dir).unwrap(),
      Some("https://example.com/triggered-by-any-subpath/moment".to_string())
    );
    assert_eq!(
      import_map.resolve("moment/foo", js_in_dir).unwrap(),
      Some(
        "https://example.com/triggered-by-any-subpath/moment/foo".to_string()
      )
    );
    assert!(import_map.resolve("moment", with_js_prefix).is_err());
    assert!(import_map.resolve("moment/foo", with_js_prefix).is_err());
  }

  #[test]
  fn scopes_3() {
    let base_url = "https://example.com/app/main.ts";

    let json_map = json!({
      "scopes": {
        "/js": {
          "moment": "/only-triggered-by-exact/moment",
          "moment/": "/only-triggered-by-exact/moment/"
        }
      }
    });
    let import_map =
      ImportMap::from_json(base_url, &json_map.to_string()).unwrap();

    // should match correctly when only an exact match is in the map
    let js_non_dir = "https://example.com/js";
    let js_in_dir = "https://example.com/js/app.mjs";
    let with_js_prefix = "https://example.com/jsiscool";

    assert_eq!(
      import_map.resolve("moment", js_non_dir).unwrap(),
      Some("https://example.com/only-triggered-by-exact/moment".to_string())
    );
    assert_eq!(
      import_map.resolve("moment/foo", js_non_dir).unwrap(),
      Some(
        "https://example.com/only-triggered-by-exact/moment/foo".to_string()
      )
    );
    assert!(import_map.resolve("moment", js_in_dir).is_err());
    assert!(import_map.resolve("moment/foo", js_in_dir).is_err());
    assert!(import_map.resolve("moment", with_js_prefix).is_err());
    assert!(import_map.resolve("moment/foo", with_js_prefix).is_err());
  }

  #[test]
  fn scopes_4() {
    let base_url = "https://example.com/app/main.ts";

    let json_map = json!({
      "scopes": {
        "/js/": {
          "moment": "/triggered-by-any-subpath/moment",
          "moment/": "/triggered-by-any-subpath/moment/"
        }
      }
    });
    let import_map =
      ImportMap::from_json(base_url, &json_map.to_string()).unwrap();

    // should match correctly when only a prefix match is in the map
    let js_non_dir = "https://example.com/js";
    let js_in_dir = "https://example.com/js/app.mjs";
    let with_js_prefix = "https://example.com/jsiscool";

    assert!(import_map.resolve("moment", js_non_dir).is_err());
    assert!(import_map.resolve("moment/foo", js_non_dir).is_err());
    assert_eq!(
      import_map.resolve("moment", js_in_dir).unwrap(),
      Some("https://example.com/triggered-by-any-subpath/moment".to_string())
    );
    assert_eq!(
      import_map.resolve("moment/foo", js_in_dir).unwrap(),
      Some(
        "https://example.com/triggered-by-any-subpath/moment/foo".to_string()
      )
    );
    assert!(import_map.resolve("moment", with_js_prefix).is_err());
    assert!(import_map.resolve("moment/foo", with_js_prefix).is_err());
  }

  #[test]
  fn scopes_package_like() {
    let base_url = "https://example.com/app/main.ts";

    let json_map = json!({
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
    });
    let import_map =
      ImportMap::from_json(base_url, &json_map.to_string()).unwrap();

    // should match correctly when only a prefix match is in the map
    let js_non_dir = "https://example.com/js";
    let js_in_dir = "https://example.com/js/app.mjs";
    let with_js_prefix = "https://example.com/jsiscool";
    let top_level = "https://example.com/app.mjs";

    // should resolve scoped'
    assert_eq!(
      import_map.resolve("lodash-dot", js_in_dir).unwrap(),
      Some(
        "https://example.com/app/node_modules_2/lodash-es/lodash.js"
          .to_string()
      )
    );
    assert_eq!(
      import_map.resolve("lodash-dotdot", js_in_dir).unwrap(),
      Some(
        "https://example.com/node_modules_2/lodash-es/lodash.js".to_string()
      )
    );
    assert_eq!(
      import_map.resolve("lodash-dot/foo", js_in_dir).unwrap(),
      Some("https://example.com/app/node_modules_2/lodash-es/foo".to_string())
    );
    assert_eq!(
      import_map.resolve("lodash-dotdot/foo", js_in_dir).unwrap(),
      Some("https://example.com/node_modules_2/lodash-es/foo".to_string())
    );

    // should apply best scope match'
    assert_eq!(
      import_map.resolve("moment", top_level).unwrap(),
      Some(
        "https://example.com/node_modules_3/moment/src/moment.js".to_string()
      )
    );
    assert_eq!(
      import_map.resolve("moment", js_in_dir).unwrap(),
      Some(
        "https://example.com/node_modules_3/moment/src/moment.js".to_string()
      )
    );
    assert_eq!(
      import_map.resolve("vue", js_in_dir).unwrap(),
      Some(
        "https://example.com/node_modules_3/vue/dist/vue.runtime.esm.js"
          .to_string()
      )
    );

    // should fallback to "imports"
    assert_eq!(
      import_map.resolve("moment/foo", top_level).unwrap(),
      Some("https://example.com/node_modules/moment/src/foo".to_string())
    );
    assert_eq!(
      import_map.resolve("moment/foo", js_in_dir).unwrap(),
      Some("https://example.com/node_modules/moment/src/foo".to_string())
    );
    assert_eq!(
      import_map.resolve("lodash-dot", top_level).unwrap(),
      Some(
        "https://example.com/app/node_modules/lodash-es/lodash.js".to_string()
      )
    );
    assert_eq!(
      import_map.resolve("lodash-dotdot", top_level).unwrap(),
      Some("https://example.com/node_modules/lodash-es/lodash.js".to_string())
    );
    assert_eq!(
      import_map.resolve("lodash-dot/foo", top_level).unwrap(),
      Some("https://example.com/app/node_modules/lodash-es/foo".to_string())
    );
    assert_eq!(
      import_map.resolve("lodash-dotdot/foo", top_level).unwrap(),
      Some("https://example.com/node_modules/lodash-es/foo".to_string())
    );

    // should still fail for package-like specifiers that are not declared'
    assert!(import_map.resolve("underscore/", js_in_dir).is_err());
    assert!(import_map.resolve("underscore/foo", js_in_dir).is_err());
  }
}
