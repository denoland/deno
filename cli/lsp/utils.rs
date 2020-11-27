// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use deno_core::error::custom_error;
use deno_core::error::AnyError;
use deno_core::serde_json::Value;
use lsp_server::Notification;
use serde::de::DeserializeOwned;
use std::error::Error;
use std::fmt;

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
