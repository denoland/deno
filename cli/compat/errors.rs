// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use deno_core::error::generic_error;
use deno_core::error::AnyError;
use deno_core::url::Url;

pub(crate) fn err_unsupported_esm_url_scheme(url: &Url) -> AnyError {
  let mut msg =
    "[ERR_UNSUPPORTED_ESM_URL_SCHEME] Only file and data URLS are supported by the default ESM loader"
      .to_string();

  if cfg!(window) && url.scheme().len() == 2 {
    msg = format!(
      "{}. On Windows, absolute path must be valid file:// URLs",
      msg
    );
  }

  msg = format!("{}. Received protocol '{}'", msg, url.scheme());
  generic_error(msg)
}
