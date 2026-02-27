// Copyright 2018-2025 the Deno authors. MIT license.

use url::ParseError;
use url::Url;

/// Error indicating the reason resolving a module specifier failed.
#[derive(
  Clone, Debug, Eq, PartialEq, thiserror::Error, deno_error::JsError,
)]
#[class(uri)]
pub enum ModuleResolutionError {
  #[error("invalid URL: {0}")]
  InvalidUrl(#[source] ParseError),
  #[error("invalid base URL for relative import: {0}")]
  InvalidBaseUrl(#[source] ParseError),
  #[error(
    "Relative import path \"{specifier}\" not prefixed with / or ./ or ../{}",
    .maybe_referrer.as_ref().map_or(String::new(), |referrer| format!(" from \"{referrer}\""))
  )]
  ImportPrefixMissing {
    specifier: String,
    maybe_referrer: Option<String>,
  },
}

use ModuleResolutionError::*;

/// Resolved module specifier
pub type ModuleSpecifier = Url;

/// Resolves module using this algorithm:
/// <https://html.spec.whatwg.org/multipage/webappapis.html#resolve-a-module-specifier>
pub fn resolve_import(
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
      let maybe_referrer = if base.is_empty() {
        None
      } else {
        Some(base.to_string())
      };
      return Err(ImportPrefixMissing {
        specifier: specifier.to_string(),
        maybe_referrer,
      });
    }

    // 3. Return the result of applying the URL parser to specifier with base
    //    URL as the base URL.
    Err(ParseError::RelativeUrlWithoutBase) => {
      let base = Url::parse(base).map_err(InvalidBaseUrl)?;
      base.join(specifier).map_err(InvalidUrl)?
    }

    // If parsing the specifier as a URL failed for a different reason than
    // it being relative, always return the original error. We don't want to
    // return `ImportPrefixMissing` or `InvalidBaseUrl` if the real
    // problem lies somewhere else.
    Err(err) => return Err(InvalidUrl(err)),
  };

  Ok(url)
}

/// Converts a string representing an absolute URL into a ModuleSpecifier.
pub fn resolve_url(
  url_str: &str,
) -> Result<ModuleSpecifier, ModuleResolutionError> {
  Url::parse(url_str).map_err(ModuleResolutionError::InvalidUrl)
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::serde_json::from_value;
  use crate::serde_json::json;

  #[test]
  fn test_resolve_import() {
    let tests = vec![
      (
        "./005_more_imports.ts",
        "http://deno.land/core/tests/006_url_imports.ts",
        "http://deno.land/core/tests/005_more_imports.ts",
      ),
      (
        "../005_more_imports.ts",
        "http://deno.land/core/tests/006_url_imports.ts",
        "http://deno.land/core/005_more_imports.ts",
      ),
      (
        "http://deno.land/core/tests/005_more_imports.ts",
        "http://deno.land/core/tests/006_url_imports.ts",
        "http://deno.land/core/tests/005_more_imports.ts",
      ),
      (
        "data:text/javascript,export default 'grapes';",
        "http://deno.land/core/tests/006_url_imports.ts",
        "data:text/javascript,export default 'grapes';",
      ),
      (
        "blob:https://whatwg.org/d0360e2f-caee-469f-9a2f-87d5b0456f6f",
        "http://deno.land/core/tests/006_url_imports.ts",
        "blob:https://whatwg.org/d0360e2f-caee-469f-9a2f-87d5b0456f6f",
      ),
      (
        "javascript:export default 'artichokes';",
        "http://deno.land/core/tests/006_url_imports.ts",
        "javascript:export default 'artichokes';",
      ),
      (
        "data:text/plain,export default 'kale';",
        "http://deno.land/core/tests/006_url_imports.ts",
        "data:text/plain,export default 'kale';",
      ),
      (
        "/dev/core/tests/005_more_imports.ts",
        "file:///home/yeti",
        "file:///dev/core/tests/005_more_imports.ts",
      ),
      (
        "//zombo.com/1999.ts",
        "https://cherry.dev/its/a/thing",
        "https://zombo.com/1999.ts",
      ),
      (
        "http://deno.land/this/url/is/valid",
        "base is clearly not a valid url",
        "http://deno.land/this/url/is/valid",
      ),
      (
        "//server/some/dir/file",
        "file:///home/yeti/deno",
        "file://server/some/dir/file",
      ),
      // This test is disabled because the url crate does not follow the spec,
      // dropping the server part from the final result.
      // (
      //   "/another/path/at/the/same/server",
      //   "file://server/some/dir/file",
      //   "file://server/another/path/at/the/same/server",
      // ),
    ];

    for (specifier, base, expected_url) in tests {
      let url = resolve_import(specifier, base).unwrap().to_string();
      assert_eq!(url, expected_url);
    }
  }

  #[test]
  fn test_resolve_import_error() {
    use ModuleResolutionError::*;
    use url::ParseError::*;

    let tests = vec![
      (
        "awesome.ts",
        "<unknown>",
        ImportPrefixMissing {
          specifier: "awesome.ts".to_string(),
          maybe_referrer: Some("<unknown>".to_string()),
        },
      ),
      (
        "005_more_imports.ts",
        "http://deno.land/core/tests/006_url_imports.ts",
        ImportPrefixMissing {
          specifier: "005_more_imports.ts".to_string(),
          maybe_referrer: Some(
            "http://deno.land/core/tests/006_url_imports.ts".to_string(),
          ),
        },
      ),
      (
        ".tomato",
        "http://deno.land/core/tests/006_url_imports.ts",
        ImportPrefixMissing {
          specifier: ".tomato".to_string(),
          maybe_referrer: Some(
            "http://deno.land/core/tests/006_url_imports.ts".to_string(),
          ),
        },
      ),
      (
        "..zucchini.mjs",
        "http://deno.land/core/tests/006_url_imports.ts",
        ImportPrefixMissing {
          specifier: "..zucchini.mjs".to_string(),
          maybe_referrer: Some(
            "http://deno.land/core/tests/006_url_imports.ts".to_string(),
          ),
        },
      ),
      (
        r".\yam.es",
        "http://deno.land/core/tests/006_url_imports.ts",
        ImportPrefixMissing {
          specifier: r".\yam.es".to_string(),
          maybe_referrer: Some(
            "http://deno.land/core/tests/006_url_imports.ts".to_string(),
          ),
        },
      ),
      (
        r"..\yam.es",
        "http://deno.land/core/tests/006_url_imports.ts",
        ImportPrefixMissing {
          specifier: r"..\yam.es".to_string(),
          maybe_referrer: Some(
            "http://deno.land/core/tests/006_url_imports.ts".to_string(),
          ),
        },
      ),
      (
        "https://eggplant:b/c",
        "http://deno.land/core/tests/006_url_imports.ts",
        InvalidUrl(InvalidPort),
      ),
      (
        "https://eggplant@/c",
        "http://deno.land/core/tests/006_url_imports.ts",
        InvalidUrl(EmptyHost),
      ),
      (
        "./foo.ts",
        "/relative/base/url",
        InvalidBaseUrl(RelativeUrlWithoutBase),
      ),
    ];

    for (specifier, base, expected_err) in tests {
      let err = resolve_import(specifier, base).unwrap_err();
      assert_eq!(err, expected_err);
    }
  }

  #[test]
  fn test_deserialize_module_specifier() {
    let actual: ModuleSpecifier =
      from_value(json!("http://deno.land/x/mod.ts")).unwrap();
    let expected = resolve_url("http://deno.land/x/mod.ts").unwrap();
    assert_eq!(actual, expected);
  }
}
