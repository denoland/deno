use std::fmt;
use std::path::PathBuf;
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
    // TODO: it should firstly try to resolve from path (specifier may be absolute filepath)

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
  pub fn resolve_from_cwd(
    specifier: &str,
  ) -> Result<ModuleSpecifier, url::ParseError> {
    if let Ok(module_specifier) = ModuleSpecifier::resolve_absolute(specifier) {
      return Ok(module_specifier);
    }

    // fallback to relative file path
    // HACK: `Url::from_directory_path` is used here because it normalizes the path.
    // Joining `/dev/deno/" with "./tests" using `PathBuf` yields `/deno/dev/./tests/`.
    // On the other hand joining `/dev/deno/" with "./tests" using `Url` yields "/dev/deno/tests"
    // - and that's what we want.
    // There exists similar method on `PathBuf` - `PathBuf.canonicalize`, but the problem
    // is `canonicalize` resolves symlinks and we don't want that.
    // We just want o normalize the path...
    let specifier_path = PathBuf::from(specifier);
    let cwd = std::env::current_dir().unwrap();
    let path = cwd.join(specifier_path);
    let url =
      Url::from_file_path(path).expect("PathBuf should be parseable URL");
    Ok(ModuleSpecifier(url))
  }

  /// Takes a string representing a path or URL to a module - must be absolute file path
  /// or remote URL
  pub fn resolve_absolute(
    specifier: &str,
  ) -> Result<ModuleSpecifier, url::ParseError> {
    // first check if specifier is an absolute path
    let path = PathBuf::from(specifier);

    if let Ok(url) = Url::from_file_path(path) {
      return Ok(ModuleSpecifier(url));
    }

    // now check if it's resolvable url
    let url = Url::parse(specifier)?;
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

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_resolve_from_cwd() {
    if cfg!(target_os = "windows") {
      let expected_url = "file:///C:/deno/tests/006_url_imports.ts";

      eprintln!("{:?}", Url::parse(r"C:/deno/tests/006_url_imports.ts"));
      eprintln!("{:?}", Url::parse(r"C:\deno\tests\006_url_imports.ts"));

      assert_eq!(
        ModuleSpecifier::resolve_from_cwd(r"C:/deno/tests/006_url_imports.ts")
          .unwrap()
          .to_string(),
        expected_url,
      );
      assert_eq!(
        ModuleSpecifier::resolve_from_cwd(r"C:\deno\tests\006_url_imports.ts")
          .unwrap()
          .to_string(),
        expected_url,
      );
      assert_eq!(
        ModuleSpecifier::resolve_from_cwd(r"/deno/tests/006_url_imports.ts")
          .unwrap()
          .to_string(),
        expected_url
      );
    } else {
      assert_eq!(
        ModuleSpecifier::resolve_from_cwd("/deno/tests/006_url_imports.ts")
          .unwrap()
          .to_string(),
        "file:///deno/tests/006_url_imports.ts"
      );
    }

    // Assuming cwd is the deno repo root.
    let cwd = std::env::current_dir().unwrap();
    let cwd_string = String::from(cwd.to_str().unwrap()) + "/";

    if cfg!(target_os = "windows") {
      let expected_url = format!(
        "file:///{}{}",
        cwd_string.as_str(),
        "tests/006_url_imports.ts"
      ).replace("\\", "/");

      assert_eq!(
        ModuleSpecifier::resolve_from_cwd(r"./tests/006_url_imports.ts")
          .unwrap()
          .to_string(),
        expected_url
      );
      assert_eq!(
        ModuleSpecifier::resolve_from_cwd(r".\tests\006_url_imports.ts")
          .unwrap()
          .to_string(),
        expected_url
      );
      assert_eq!(
        ModuleSpecifier::resolve_from_cwd(r"\tests\006_url_imports.ts")
          .unwrap()
          .to_string(),
        expected_url
      );
    } else {
      assert_eq!(
        ModuleSpecifier::resolve_from_cwd("./tests/006_url_imports.ts")
          .unwrap()
          .to_string(),
        format!(
          "file://{}{}",
          cwd_string.as_str(),
          "tests/006_url_imports.ts"
        ),
      );
    }

    assert_eq!(
      ModuleSpecifier::resolve_from_cwd(
        "http://deno.land/core/tests/006_url_imports.ts"
      ).unwrap()
      .to_string(),
      "http://deno.land/core/tests/006_url_imports.ts",
    );
    assert_eq!(
      ModuleSpecifier::resolve_from_cwd(
        "https://deno.land/core/tests/006_url_imports.ts"
      ).unwrap()
      .to_string(),
      "https://deno.land/core/tests/006_url_imports.ts",
    );
  }

  #[test]
  fn test_resolve() {
    assert_eq!(
      ModuleSpecifier::resolve(
        "./005_more_imports.ts",
        "http://deno.land/core/tests/006_url_imports.ts",
      ).unwrap()
      .to_string(),
      "http://deno.land/core/tests/005_more_imports.ts"
    );

    // TODO(bartlomieju): add more test cases
  }

  #[test]
  fn test_resolve_bad_specifier() {
    assert!(
      ModuleSpecifier::resolve(
        "005_more_imports.ts",
        "http://deno.land/core/tests/006_url_imports.ts",
      ).is_err()
    );
    // TODO(bartlomieju): add more test cases
  }
}
