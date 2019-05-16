use std::collections::HashMap;
use url::Url;
use crate::worker::resolve_module_spec;

#[derive(PartialEq, Debug)]
enum ParsedSpecifierType {
  Bare,
  Url,
  Invalid,
}

#[allow(dead_code)]
#[derive(Default, Debug)]
struct ParsedSpecifier {
  bare_specifier: Option<String>,
  url: Option<Url>,
}

// TODO: make it more rusty
// TODO: make sure we actually need this struct
#[allow(dead_code)]
impl ParsedSpecifier {
  pub fn new(
    specifier: &str,
    base: &str,
  ) -> Self {
    // TODO: this method is very similar to `resolve_module_spec`
    // 1. Apply the URL parser to specifier. If the result is not failure, return
    //    the result.
    if let Ok(specifier_url) = Url::parse(specifier) {
      return ParsedSpecifier::from(specifier_url);
    }

    // 2. If specifier does not start with the character U+002F SOLIDUS (/), the
    //    two-character sequence U+002E FULL STOP, U+002F SOLIDUS (./), or the
    //    three-character sequence U+002E FULL STOP, U+002E FULL STOP, U+002F
    //    SOLIDUS (../), return failure.
    if !specifier.starts_with('/')
      && !specifier.starts_with("./")
      && !specifier.starts_with("../")
    {
      if specifier.is_empty() {
        return ParsedSpecifier::default();
      }

      return ParsedSpecifier::from(specifier.to_string());
    }

    // 3. Return the result of applying the URL parser to specifier with base URL
    //    as the base URL.
    if let Ok(base_url) = Url::parse(base) {
      if let Ok(url) = base_url.join(&specifier) {
        return ParsedSpecifier::from(url);
      }
    }

    return ParsedSpecifier::default();
  }

  pub fn get_type(&self) -> ParsedSpecifierType {
    if self.url.is_some() {
      return ParsedSpecifierType::Url;
    }

    if self.bare_specifier.is_some() {
      return ParsedSpecifierType::Bare;
    }

    ParsedSpecifierType::Invalid
  }

  pub fn is_valid(&self) -> bool {
    match self.get_type() {
      ParsedSpecifierType::Invalid => false,
      _ => true,
    }
  }

  pub fn get_import_map_key(&self) -> Option<String> {
    if self.url.is_some() {
      return Some(self.url.clone().unwrap().to_string());
    }

    if self.bare_specifier.is_some() {
      return self.bare_specifier.clone();
    }

    None
  }

  pub fn get_url(&self) -> Option<Url> {
    if self.url.is_some() {
      return self.url.clone();
    }

    None
  }
}

impl From<Url> for ParsedSpecifier {
  fn from(url: Url) -> Self {
    ParsedSpecifier {
      url: Some(url),
      ..ParsedSpecifier::default()
    }
  }
}

impl From<String> for ParsedSpecifier {
  fn from(bare_specifier: String) -> Self {
    ParsedSpecifier {
      bare_specifier: Some(bare_specifier),
      ..ParsedSpecifier::default()
    }
  }
}

// TODO: this should return Result?
/// This is only place where we allow for bare imports (eg. `import query from "jquery";`);
#[allow(dead_code)]
fn get_import_map_key(specifier: &str, base_url: &str) -> Option<String> {
  if specifier.is_empty() {
    return None;
  }

  let parsed_specifier = ParsedSpecifier::new(specifier, base_url);
  let map_key = parsed_specifier.get_import_map_key();
  map_key
}

// TODO: this should return Result?
/// Only resolvable URLs are allowed as values to import map
#[allow(dead_code)]
fn get_import_map_value(specifier: &str, base_url: &str) -> Option<String> {
  match resolve_module_spec(specifier, base_url) {
    Ok(url) => Some(url),
    Err(_) => None
  }
}

#[allow(dead_code)]
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

  fn resolve_imports_match(&self, normalized_specifier: String) -> Option<String> {
    for (specifier_key, address_vec) in self.modules.iter() {
      // exact-match
      if normalized_specifier == specifier_key.to_string() {
        if address_vec.is_empty() {
          println!("Specifier {:?} was mapped to no addresses.", normalized_specifier);
          return None;
        } else if address_vec.len() == 1 {
          let address = address_vec.first().unwrap();
          println!("Specifier {:?} was mapped to {:?}.", normalized_specifier, address);
          // TODO(bartlomieju): ensure that it's a valid URL
          return Some(address.to_string());
        } else {
          println!("Multi-address mappings are not yet supported");
          return None;
        }
      }

      // package-prefix match
      if specifier_key.ends_with("/") && normalized_specifier.starts_with(specifier_key) {
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
            },
            Err(_) => {
              println!("Specifier {:?} was mapped via prefix specifier key {:?}, but is not resolvable.", normalized_specifier, address);
              return None;
            },
          };
        } else {
          println!("Multi-address mappings are not yet supported");
          return None;
        }
      }
    }

    println!("Specifier {:?} was not mapped in import map.", normalized_specifier);
    return None;
  }

  /// Currently we support two types of specifiers: URL (http://, https://, file://)
  /// and "bare" (moment, jquery, lodash)
  pub fn resolve(&self, specifier: &str, referrer: &str) -> Option<String> {
    let resolved_url: Option<String> = match resolve_module_spec(specifier, referrer) {
      Ok(url) => Some(url.clone()),
      Err(_) => None,
    };
    let normalized_specifier = match &resolved_url {
      Some(url) => url.clone(),
      None => specifier.to_string(),
    };

    // TODO: handle scopes

    let imports_match = self.resolve_imports_match(normalized_specifier.clone());

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

  #[test]
  fn test_parsed_specifier() {
    let spec1 = &ParsedSpecifier::new("./foo.ts", "file:///");
    assert!(spec1.is_valid());
    assert_eq!(
      spec1.get_type(),
      ParsedSpecifierType::Url,
    );
    assert_eq!(
      spec1.get_import_map_key(),
      Some("file:///foo.ts".to_string()),
    );

    let spec2 = &ParsedSpecifier::new("foo", "http://deno.land");
    assert!(spec2.is_valid());
    assert_eq!(
      spec2.get_type(),
      ParsedSpecifierType::Bare,
    );
    assert_eq!(
      spec2.get_import_map_key(),
      Some("foo".to_string()),
    );
    assert_eq!(
      spec2.get_url(),
      None,
    );

    let invalid_spec = &ParsedSpecifier::new("", "http://deno.land");
    assert!(!invalid_spec.is_valid());
    assert_eq!(
      invalid_spec.get_type(),
      ParsedSpecifierType::Invalid,
    );
    assert_eq!(
      invalid_spec.get_import_map_key(),
      None,
    );
    assert_eq!(
      invalid_spec.get_url(),
      None,
    );

    let invalid_base = &ParsedSpecifier::new("", ".");
    assert!(!invalid_base.is_valid());
    assert_eq!(
      invalid_base.get_type(),
      ParsedSpecifierType::Invalid,
    );
    assert_eq!(
      invalid_base.get_import_map_key(),
      None,
    );
    assert_eq!(
      invalid_base.get_url(),
      None,
    );
  }

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
    assert_eq!(import_map.resolve("https://ex ample.org", referrer_url), None);
    assert_eq!(import_map.resolve("https://example.org:deno", referrer_url), None);
    assert_eq!(import_map.resolve("https://[example.org]", referrer_url), None);
  }
}
