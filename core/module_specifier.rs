use std::error::Error;
use std::fmt;
use url::ParseError;
use url::Url;

/// Error indicating the reason resolving a module specifier failed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ModuleResolutionError {
  InvalidUrl(ParseError),
  InvalidBaseUrl(ParseError),
  ImportPathPrefixMissing,
}
use ModuleResolutionError::*;

impl Error for ModuleResolutionError {
  fn source(&self) -> Option<&(dyn Error + 'static)> {
    match self {
      InvalidUrl(ref err) | InvalidBaseUrl(ref err) => Some(err),
      _ => None,
    }
  }
}

impl fmt::Display for ModuleResolutionError {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    match self {
      InvalidUrl(ref err) => write!(f, "invalid URL: {}", err),
      InvalidBaseUrl(ref err) => {
        write!(f, "invalid base URL for relative import: {}", err)
      }
      ImportPathPrefixMissing => {
        write!(f, "relative import path not prefixed with / or ./ or ../")
      }
    }
  }
}

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
  ) -> Result<ModuleSpecifier, ModuleResolutionError> {
    let url = match Url::parse(specifier) {
      // 1. Apply the URL parser to specifier.
      //    If the result is not failure, return he result.
      Ok(url) => url,

      // 2. If specifier does not start with the character U+002F SOLIDUS (/),
      //    the two-character sequence U+002E FULL STOP, U+002F SOLIDUS (./),
      //    or the three-character sequence U+002E FULL STOP, U+002E FULL STOP,
      //    U+002F SOLIDUS (../), return failure.
      Err(ParseError::RelativeUrlWithoutBase)
        if !(specifier.starts_with('/')
          || specifier.starts_with("./")
          || specifier.starts_with("../")) =>
      {
        Err(ImportPathPrefixMissing)?
      }

      // 3. Return the result of applying the URL parser to specifier with base
      //    URL as the base URL.
      Err(ParseError::RelativeUrlWithoutBase) => {
        let base = Url::parse(base).map_err(InvalidBaseUrl)?;
        base.join(&specifier).map_err(InvalidUrl)?
      }

      // If parsing the specifier as a URL failed for a different reason than
      // it being relative, always return the original error. We don't want to
      // return `ImportPathPrefixMissing` or `InvalidBaseUrl` if the real
      // problem lies somewhere else.
      Err(err) => Err(InvalidUrl(err))?,
    };

    Ok(ModuleSpecifier(url))
  }

  /// Takes a string representing a path or URL to a module, but of the type
  /// passed through the command-line interface for the main module. This is
  /// slightly different than specifiers used in import statements: "foo.js" for
  /// example is allowed here, whereas in import statements a leading "./" is
  /// required ("./foo.js"). This function is aware of the current working
  /// directory and returns an absolute URL.
  pub fn resolve_root(
    root_specifier: &str,
  ) -> Result<ModuleSpecifier, ModuleResolutionError> {
    let url = match Url::parse(root_specifier) {
      Ok(url) => url,
      Err(..) => {
        let cwd = std::env::current_dir().unwrap();
        let base = Url::from_directory_path(cwd).unwrap();
        base.join(&root_specifier).map_err(InvalidUrl)?
      }
    };

    Ok(ModuleSpecifier(url))
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
