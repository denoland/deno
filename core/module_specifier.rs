use std::fmt;
use std::path::Component;
use std::path::Path;
use std::path::PathBuf;
use std::path::Prefix;
use url::Url;

/// Ensure that Windows path has disk prefix (eg. C:, D:).
///
/// Currently we don't allow verbatim, UNC and device NS paths.
fn ensure_valid_prefix(path: &Path) -> Result<(), url::ParseError> {
  if cfg!(target_os = "windows") {
    match path.components().next().unwrap() {
      Component::Prefix(prefix_component) => {
        match prefix_component.kind() {
          Prefix::Disk(_) => {}
          // TODO(bartlomieju) this is not proper error to return
          _ => return Err(url::ParseError::RelativeUrlWithCannotBeABaseBase),
        }
      }
      _ => unreachable!(), // TODO: should handle this branch?
    }
  }

  Ok(())
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
  ) -> Result<ModuleSpecifier, url::ParseError> {
    // 1. Apply the URL parser to specifier. If the result is not failure, return
    //    the result.
    if let Ok(module_specifier) = ModuleSpecifier::resolve_absolute(specifier) {
      return Ok(module_specifier);
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
    // TODO: factor out as `normalize_path` method - it is used in /cli/deno_dir.rs and should
    //  be applied to resolve https://github.com/denoland/deno/issues/1798
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
    ensure_valid_prefix(&path)?;
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

    if let Ok(url) = Url::from_file_path(path.clone()) {
      ensure_valid_prefix(&path)?;
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
    // Assuming cwd is the deno repo root.
    let cwd = std::env::current_dir().unwrap();
    let cwd_string = String::from(cwd.to_str().unwrap()) + "/";

    if cfg!(target_os = "windows") {
      let expected_url = "file:///C:/deno/tests/006_url_imports.ts";

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
        format!(
          "file:///{}",
          cwd.join("/deno/tests/006_url_imports.ts").to_str().unwrap(),
        ).replace("\\", "/")
      );
      assert_eq!(
        ModuleSpecifier::resolve_from_cwd(r"\tests\006_url_imports.ts")
          .unwrap()
          .to_string(),
        format!(
          "file:///{}",
          cwd.join(r"\tests\006_url_imports.ts").to_str().unwrap(),
        ).replace("\\", "/")
      );

      // non-disk paths
      let invalid_paths = vec![
        r"\\server\share",
        r"//server/share",
        r"\\.\c:\foo\bar.txt",
        r"//./c:/foo/bar.txt",
        r"\\?\c:\foo\bar",
        r"\??\something\something",
        r"d:foo\bar.txt",
      ];

      for invalid_path in invalid_paths {
        println!("{}", invalid_path);
        assert!(ModuleSpecifier::resolve_from_cwd(invalid_path).is_err());
      }
    } else {
      assert_eq!(
        ModuleSpecifier::resolve_from_cwd("/deno/tests/006_url_imports.ts")
          .unwrap()
          .to_string(),
        "file:///deno/tests/006_url_imports.ts"
      );
    }

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

    assert_eq!(
      ModuleSpecifier::resolve(
        "../005_more_imports.ts",
        "http://deno.land/core/tests/006_url_imports.ts",
      ).unwrap()
      .to_string(),
      "http://deno.land/core/005_more_imports.ts"
    );

    // absolute URL
    assert_eq!(
      ModuleSpecifier::resolve(
        "http://deno.land/core/tests/005_more_imports.ts",
        "http://deno.land/core/tests/006_url_imports.ts",
      ).unwrap()
      .to_string(),
      "http://deno.land/core/tests/005_more_imports.ts",
    );

    // absolute path
    if cfg!(target_os = "windows") {
      assert_eq!(
        ModuleSpecifier::resolve(
          r"C:/deno/tests/005_more_imports.ts",
          r"C:/deno/tests/006_url_imports.ts",
        ).unwrap()
        .to_string(),
        "file:///C:/deno/tests/005_more_imports.ts",
      );
      assert_eq!(
        ModuleSpecifier::resolve(
          r"C:\deno\tests\005_more_imports.ts",
          r"C:/deno/tests/006_url_imports.ts",
        ).unwrap()
        .to_string(),
        "file:///C:/deno/tests/005_more_imports.ts",
      );
    } else {
      assert_eq!(
        ModuleSpecifier::resolve(
          "/dev/core/tests/005_more_imports.ts",
          "/dev/core/tests/006_url_imports.ts",
        ).unwrap()
        .to_string(),
        "file:///dev/core/tests/005_more_imports.ts",
      );
    }

    assert_eq!(
      ModuleSpecifier::resolve(
        "data:text/javascript,export default 'grapes';",
        "http://deno.land/core/tests/006_url_imports.ts",
      ).unwrap()
      .to_string(),
      "data:text/javascript,export default 'grapes';",
    );

    assert_eq!(
      ModuleSpecifier::resolve(
        "blob:https://whatwg.org/d0360e2f-caee-469f-9a2f-87d5b0456f6f",
        "http://deno.land/core/tests/006_url_imports.ts",
      ).unwrap()
      .to_string(),
      "blob:https://whatwg.org/d0360e2f-caee-469f-9a2f-87d5b0456f6f",
    );

    assert_eq!(
      ModuleSpecifier::resolve(
        "javascript:export default 'artichokes';",
        "http://deno.land/core/tests/006_url_imports.ts",
      ).unwrap()
      .to_string(),
      "javascript:export default 'artichokes';",
    );

    assert_eq!(
      ModuleSpecifier::resolve(
        "data:text/plain,export default 'kale';",
        "http://deno.land/core/tests/006_url_imports.ts",
      ).unwrap()
      .to_string(),
      "data:text/plain,export default 'kale';",
    );
  }

  #[test]
  fn test_resolve_bad_specifier() {
    assert_eq!(
      ModuleSpecifier::resolve(
        "005_more_imports.ts",
        "http://deno.land/core/tests/006_url_imports.ts",
      ).unwrap_err(),
      url::ParseError::RelativeUrlWithCannotBeABaseBase
    );
    assert_eq!(
      ModuleSpecifier::resolve(
        "https://eggplant:b/c",
        "http://deno.land/core/tests/006_url_imports.ts",
      ).unwrap_err(),
      url::ParseError::RelativeUrlWithCannotBeABaseBase
    );
    assert_eq!(
      ModuleSpecifier::resolve(
        ".tomato",
        "http://deno.land/core/tests/006_url_imports.ts",
      ).unwrap_err(),
      url::ParseError::RelativeUrlWithCannotBeABaseBase
    );
    assert_eq!(
      ModuleSpecifier::resolve(
        "..zucchini.mjs",
        "http://deno.land/core/tests/006_url_imports.ts",
      ).unwrap_err(),
      url::ParseError::RelativeUrlWithCannotBeABaseBase
    );
    assert_eq!(
      ModuleSpecifier::resolve(
        r".\yam.es",
        "http://deno.land/core/tests/006_url_imports.ts",
      ).unwrap_err(),
      url::ParseError::RelativeUrlWithCannotBeABaseBase
    );
  }
}
