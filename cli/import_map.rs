use std::collections::HashMap;
use url::Url;

#[allow(dead_code)]
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

#[allow(dead_code)]
impl ParsedSpecifier {
  pub fn new(
    specifier: &str,
    base: &str,
  ) -> Self {
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
    self.get_type() != ParsedSpecifierType::Invalid
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

#[allow(dead_code)]
struct ImportMap {
  modules: HashMap<String, Vec<Url>>,
}

#[allow(dead_code)]
impl ImportMap {
  pub fn new() -> Self {
    ImportMap {
      modules: HashMap::new(),
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_parsed_specifier() {
    let spec1 = &ParsedSpecifier::new("./foo.ts", ".");
    assert!(spec1.is_valid());
    assert_eq!(
      spec1.get_type(),
      ParsedSpecifierType::Url,
    );
    assert_eq!(
      spec1.get_import_map_key(),
      Some("./foo.ts".to_string()),
    );

    let spec2 = &ParsedSpecifier::new("foo", ".");
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

    let invalid_spec = &ParsedSpecifier::new("", ".");
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
  }

//  #[test]
//  fn test_remaps() {
//    let mut import_map = ImportMap::new();
//  }
}
