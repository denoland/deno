// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use deno_core::error::custom_error;
use deno_core::error::AnyError;
use deno_core::serde_json::Value;
use deno_core::url::Position;
use deno_core::url::Url;
use deno_core::ModuleSpecifier;
use lsp_server::Notification;
use serde::de::DeserializeOwned;
use std::error::Error;
use std::fmt;

// TODO(@kitsonk) support actually supporting cancellation requests from the
// client.

pub struct Canceled {
  _private: (),
}

impl Canceled {
  #[allow(unused)]
  pub fn new() -> Self {
    Self { _private: () }
  }

  #[allow(unused)]
  pub fn throw() -> ! {
    std::panic::resume_unwind(Box::new(Canceled::new()))
  }
}

impl fmt::Display for Canceled {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    write!(f, "cancelled")
  }
}

impl fmt::Debug for Canceled {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    write!(f, "Canceled")
  }
}

impl Error for Canceled {}

pub fn from_json<T: DeserializeOwned>(
  what: &'static str,
  json: Value,
) -> Result<T, AnyError> {
  let response = T::deserialize(&json).map_err(|err| {
    custom_error(
      "DeserializeFailed",
      format!("Failed to deserialize {}: {}; {}", what, err, json),
    )
  })?;
  Ok(response)
}

pub fn is_canceled(e: &(dyn Error + 'static)) -> bool {
  e.downcast_ref::<Canceled>().is_some()
}

pub fn notification_is<N: lsp_types::notification::Notification>(
  notification: &Notification,
) -> bool {
  notification.method == N::METHOD
}

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
  fn test_normalize_url() {
    let fixture = Url::parse("deno:///https/deno.land/x/mod.ts").unwrap();
    let actual = normalize_url(fixture);
    assert_eq!(
      actual,
      ModuleSpecifier::resolve_url("https://deno.land/x/mod.ts").unwrap()
    );
  }
}
