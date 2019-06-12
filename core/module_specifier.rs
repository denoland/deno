use std::fmt;
use url::Url;

#[derive(Debug, Clone, PartialEq)]
/// Resolved module specifier
pub struct ModuleSpecifier(Url);

impl ModuleSpecifier {
  pub fn to_url(&self) -> Url {
    self.0.clone()
  }

  /// Resolves module using this algorithm:
  /// https://html.spec.whatwg.org/multipage/webappapis.html#resolve-a-module-specifier
  pub fn resolve(
    specifier: &str,
    base: &str,
  ) -> Result<ModuleSpecifier, url::ParseError> {
    // 1. Apply the URL parser to specifier. If the result is not failure, return
    //    the result.
    // let specifier = parse_local_or_remote(specifier)?.to_string();
    if let Ok(url) = Url::parse(specifier) {
      return Ok(ModuleSpecifier(url));
    }

    // 2. If specifier does not start with the character U+002F SOLIDUS (/), the
    //    two-character sequence U+002E FULL STOP, U+002F SOLIDUS (./), or the
    //    three-character sequence U+002E FULL STOP, U+002E FULL STOP, U+002F
    //    SOLIDUS (../), return failure.
    if !specifier.starts_with('/')
      && !specifier.starts_with("./")
      && !specifier.starts_with("../")
    {
      // TODO This is (probably) not the correct error to return here.
      // TODO: This error is very not-user-friendly
      return Err(url::ParseError::RelativeUrlWithCannotBeABaseBase);
    }

    // 3. Return the result of applying the URL parser to specifier with base URL
    //    as the base URL.
    let base_url = Url::parse(base)?;
    let u = base_url.join(&specifier)?;
    Ok(ModuleSpecifier(u))
  }

  /// Takes a string representing a path or URL to a module, but of the type
  /// passed through the command-line interface for the main module. This is
  /// slightly different than specifiers used in import statements: "foo.js" for
  /// example is allowed here, whereas in import statements a leading "./" is
  /// required ("./foo.js"). This function is aware of the current working
  /// directory and returns an absolute URL.
  pub fn resolve_root(
    root_specifier: &str,
  ) -> Result<ModuleSpecifier, url::ParseError> {
    if let Ok(url) = Url::parse(root_specifier) {
      Ok(ModuleSpecifier(url))
    } else {
      let cwd = std::env::current_dir().unwrap();
      let base = Url::from_directory_path(cwd).unwrap();
      let url = base.join(root_specifier)?;
      Ok(ModuleSpecifier(url))
    }
  }
}

impl fmt::Display for ModuleSpecifier {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    self.0.fmt(f)
  }
}

impl From<Url> for ModuleSpecifier {
  fn from(url: Url) -> Self {
    ModuleSpecifier(url)
  }
}

impl PartialEq<String> for ModuleSpecifier {
  fn eq(&self, other: &String) -> bool {
    &self.to_string() == other
  }
}
