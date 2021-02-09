// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use deno_core::error::AnyError;
use deno_core::url::Position;
use deno_core::url::Url;
use deno_core::ModuleSpecifier;

/// Normalizes a file name returned from the TypeScript compiler into a URI that
/// should be sent by the language server to the client.
pub fn normalize_file_name(file_name: &str) -> Result<Url, AnyError> {
  let specifier_str = if file_name.starts_with("file://") {
    file_name.to_string()
  } else {
    format!("deno:///{}", file_name.replacen("://", "/", 1))
  };
  Url::parse(&specifier_str).map_err(|err| err.into())
}

pub fn normalize_specifier(
  specifier: &ModuleSpecifier,
) -> Result<Url, AnyError> {
  let url = specifier.as_url();
  if url.scheme() == "file" {
    Ok(url.clone())
  } else {
    let specifier_str =
      format!("deno:///{}", url.as_str().replacen("://", "/", 1));
    Url::parse(&specifier_str).map_err(|err| err.into())
  }
}

/// Normalize URLs from the client, where "virtual" `deno:///` URLs are
/// converted into proper module specifiers.
pub fn normalize_url(url: Url) -> ModuleSpecifier {
  if url.scheme() == "deno"
    && (url.path().starts_with("/http") || url.path().starts_with("/asset"))
  {
    let specifier_str = url[Position::BeforePath..]
      .replacen("/", "", 1)
      .replacen("/", "://", 1);
    if let Ok(specifier) =
      percent_encoding::percent_decode_str(&specifier_str).decode_utf8()
    {
      if let Ok(specifier) = ModuleSpecifier::resolve_url(&specifier) {
        return specifier;
      }
    }
  }
  ModuleSpecifier::from(url)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_normalize_file_name() {
    let fixture = "https://deno.land/x/mod.ts";
    let actual = normalize_file_name(fixture).unwrap();
    let expected = Url::parse("deno:///https/deno.land/x/mod.ts").unwrap();
    assert_eq!(actual, expected);
  }

  #[test]
  fn test_normalize_specifier() {
    let fixture =
      ModuleSpecifier::resolve_url("https://deno.land/x/mod.ts").unwrap();
    let actual = normalize_specifier(&fixture).unwrap();
    let expected = Url::parse("deno:///https/deno.land/x/mod.ts").unwrap();
    assert_eq!(actual, expected);
  }

  #[test]
  fn test_normalize_url() {
    let fixture = Url::parse("deno:///https/deno.land/x/mod.ts").unwrap();
    let actual = normalize_url(fixture);
    assert_eq!(
      actual,
      ModuleSpecifier::resolve_url("https://deno.land/x/mod.ts").unwrap()
    );
  }
}
