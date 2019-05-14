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
fn get_import_map_value(specifier: &str, base_url: &str) -> Option<String> {
  match resolve_module_spec(specifier, base_url) {
    Ok(url) => Some(url),
    Err(_) => None
  }
}

#[allow(dead_code)]
pub struct ImportMap {
  base_url: String,
  modules: HashMap<String, Vec<String>>,
}

#[allow(dead_code)]
impl ImportMap {
  // TODO(bartlomieju): add reading JSON string
  // TODO: add proper error handling: https://github.com/WICG/import-maps/issues/100
  pub fn new(base_url: &str) -> Self {
    let mut modules = HashMap::new();

    // TODO: to be removed
    modules.insert(
      get_import_map_key("bar", base_url.clone()).unwrap(),
      vec![
        get_import_map_value("./bar.ts", base_url.clone()).unwrap(),
      ],
    );
    modules.insert(
      get_import_map_key("./foo", base_url.clone()).unwrap(),
      vec![
          get_import_map_value("./test/foo.ts", base_url.clone()).unwrap(),
        ],
    );

    ImportMap {
      base_url: base_url.to_string(),
      modules,
    }
  }

  pub fn resolve(&self, specifier: &str, referrer: &str) -> Option<String> {
    println!("current map: {:?} {:?}", self.base_url, self.modules);
    let key = get_import_map_key(specifier, referrer).unwrap();

    if !self.modules.contains_key(&key) {
      println!("Import map: no match for {:?}", key);
      return None;
    }

    match self.modules.get(&key) {
      Some(matches) => {
        match matches.first() {
          Some(url) => {
            println!("Import map: found match for {:?}, maps to {:?}", key, url);
            Some(url.to_string())
          },
          // we don't want to have entries in import map that are empty vectors
          None => unreachable!(),
        }
      },
      None => unreachable!(),
    }
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

  #[test]
  fn test_import_map() {
    let import_map = ImportMap::new("file:///");

    let url1 = import_map.resolve("bar", "file:///script.ts");
    assert_eq!(url1, Some("file:///bar.ts".to_string()));

    let url2 = import_map.resolve("./foo", "file:///script.ts");
    assert_eq!(url2, Some("file:///test/foo.ts".to_string()));

    let url2 = import_map.resolve("./foo", "file:///dev/script2.ts");
    assert_eq!(url2, Some("file:///test/foo.ts".to_string()));

    let url3 = import_map.resolve("./not_mapped_module.ts", "file:///script.ts");
    assert_eq!(url3, None);
  }
}
