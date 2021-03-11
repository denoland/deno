// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use deno_core::error::generic_error;
use deno_core::error::type_error;
use deno_core::error::uri_error;
use deno_core::error::AnyError;
use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use deno_core::url::form_urlencoded;
use deno_core::url::quirks;
use deno_core::url::Url;
use deno_core::JsRuntime;
use deno_core::ZeroCopyBuf;
use serde::Deserialize;
use serde::Serialize;
use std::panic::catch_unwind;
use std::path::PathBuf;

/// Load and execute the javascript code.
pub fn init(isolate: &mut JsRuntime) {
  let files = vec![
    (
      "deno:op_crates/web/00_webidl.js",
      include_str!("00_webidl.js"),
    ),
    (
      "deno:op_crates/web/01_dom_exception.js",
      include_str!("01_dom_exception.js"),
    ),
    (
      "deno:op_crates/web/02_event.js",
      include_str!("02_event.js"),
    ),
    (
      "deno:op_crates/web/03_abort_signal.js",
      include_str!("03_abort_signal.js"),
    ),
    (
      "deno:op_crates/web/04_global_interfaces.js",
      include_str!("04_global_interfaces.js"),
    ),
    (
      "deno:op_crates/web/08_text_encoding.js",
      include_str!("08_text_encoding.js"),
    ),
    ("deno:op_crates/web/11_url.js", include_str!("11_url.js")),
    (
      "deno:op_crates/web/12_location.js",
      include_str!("12_location.js"),
    ),
    (
      "deno:op_crates/web/21_filereader.js",
      include_str!("21_filereader.js"),
    ),
  ];
  for (url, source_code) in files {
    isolate.execute_static(url, source_code).unwrap();
  }
}

/// Parse `UrlParseArgs::href` with an optional `UrlParseArgs::base_href`, or an
/// optional part to "set" after parsing. Return `UrlParts`.
pub fn op_parse_url(
  _state: &mut deno_core::OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  #[derive(Deserialize)]
  #[serde(rename_all = "camelCase")]
  struct UrlParseArgs {
    href: String,
    base_href: Option<String>,
    // If one of the following are present, this is a setter call. Apply the
    // proper `Url::set_*()` method after (re)parsing `href`.
    set_hash: Option<String>,
    set_host: Option<String>,
    set_hostname: Option<String>,
    set_password: Option<String>,
    set_pathname: Option<String>,
    set_port: Option<String>,
    set_protocol: Option<String>,
    set_search: Option<String>,
    set_username: Option<String>,
  }
  let args: UrlParseArgs = serde_json::from_value(args)?;
  let base_url = args
    .base_href
    .as_ref()
    .map(|b| Url::parse(b).map_err(|_| type_error("Invalid base URL")))
    .transpose()?;
  let mut url = Url::options()
    .base_url(base_url.as_ref())
    .parse(&args.href)
    .map_err(|_| type_error("Invalid URL"))?;

  if let Some(hash) = args.set_hash.as_ref() {
    quirks::set_hash(&mut url, hash);
  } else if let Some(host) = args.set_host.as_ref() {
    quirks::set_host(&mut url, host).map_err(|_| uri_error("Invalid host"))?;
  } else if let Some(hostname) = args.set_hostname.as_ref() {
    quirks::set_hostname(&mut url, hostname)
      .map_err(|_| uri_error("Invalid hostname"))?;
  } else if let Some(password) = args.set_password.as_ref() {
    quirks::set_password(&mut url, password)
      .map_err(|_| uri_error("Invalid password"))?;
  } else if let Some(pathname) = args.set_pathname.as_ref() {
    quirks::set_pathname(&mut url, pathname);
  } else if let Some(port) = args.set_port.as_ref() {
    quirks::set_port(&mut url, port).map_err(|_| uri_error("Invalid port"))?;
  } else if let Some(protocol) = args.set_protocol.as_ref() {
    quirks::set_protocol(&mut url, protocol)
      .map_err(|_| uri_error("Invalid protocol"))?;
  } else if let Some(search) = args.set_search.as_ref() {
    quirks::set_search(&mut url, search);
  } else if let Some(username) = args.set_username.as_ref() {
    quirks::set_username(&mut url, username)
      .map_err(|_| uri_error("Invalid username"))?;
  }

  #[derive(Serialize)]
  struct UrlParts<'a> {
    href: &'a str,
    hash: &'a str,
    host: &'a str,
    hostname: &'a str,
    origin: &'a str,
    password: &'a str,
    pathname: &'a str,
    port: &'a str,
    protocol: &'a str,
    search: &'a str,
    username: &'a str,
  }
  // TODO(nayeemrmn): Panic that occurs in rust-url for the `non-spec:`
  // url-constructor wpt tests: https://github.com/servo/rust-url/issues/670.
  let username = catch_unwind(|| quirks::username(&url)).map_err(|_| {
    generic_error(format!(
      "Internal error while parsing \"{}\"{}, \
       see https://github.com/servo/rust-url/issues/670",
      args.href,
      args
        .base_href
        .map(|b| format!(" against \"{}\"", b))
        .unwrap_or_default()
    ))
  })?;
  Ok(json!(UrlParts {
    href: quirks::href(&url),
    hash: quirks::hash(&url),
    host: quirks::host(&url),
    hostname: quirks::hostname(&url),
    origin: &quirks::origin(&url),
    password: quirks::password(&url),
    pathname: quirks::pathname(&url),
    port: quirks::port(&url),
    protocol: quirks::protocol(&url),
    search: quirks::search(&url),
    username,
  }))
}

pub fn op_parse_url_search_params(
  _state: &mut deno_core::OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let search: String = serde_json::from_value(args)?;
  let search_params: Vec<_> = form_urlencoded::parse(search.as_bytes())
    .into_iter()
    .collect();
  Ok(json!(search_params))
}

pub fn op_stringify_url_search_params(
  _state: &mut deno_core::OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let search_params: Vec<(String, String)> = serde_json::from_value(args)?;
  let search = form_urlencoded::Serializer::new(String::new())
    .extend_pairs(search_params)
    .finish();
  Ok(json!(search))
}

pub fn get_declaration() -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("lib.deno_web.d.ts")
}

#[cfg(test)]
mod tests {
  use deno_core::JsRuntime;
  use futures::future::lazy;
  use futures::task::Context;
  use futures::task::Poll;

  fn run_in_task<F>(f: F)
  where
    F: FnOnce(&mut Context) + Send + 'static,
  {
    futures::executor::block_on(lazy(move |cx| f(cx)));
  }

  fn setup() -> JsRuntime {
    let mut isolate = JsRuntime::new(Default::default());
    crate::init(&mut isolate);
    isolate
  }

  #[test]
  fn test_abort_controller() {
    run_in_task(|mut cx| {
      let mut isolate = setup();
      isolate
        .execute(
          "abort_controller_test.js",
          include_str!("abort_controller_test.js"),
        )
        .unwrap();
      if let Poll::Ready(Err(_)) = isolate.poll_event_loop(&mut cx) {
        unreachable!();
      }
    });
  }

  #[test]
  fn test_event() {
    run_in_task(|mut cx| {
      let mut isolate = setup();
      isolate
        .execute("event_test.js", include_str!("event_test.js"))
        .unwrap();
      if let Poll::Ready(Err(_)) = isolate.poll_event_loop(&mut cx) {
        unreachable!();
      }
    });
  }

  #[test]
  fn test_event_error() {
    run_in_task(|mut cx| {
      let mut isolate = setup();
      let result = isolate.execute("foo.js", "new Event()");
      if let Err(error) = result {
        let error_string = error.to_string();
        // Test that the script specifier is a URL: `deno:<repo-relative path>`.
        assert!(error_string.contains("deno:op_crates/web/02_event.js"));
        assert!(error_string.contains("TypeError"));
      } else {
        unreachable!();
      }
      if let Poll::Ready(Err(_)) = isolate.poll_event_loop(&mut cx) {
        unreachable!();
      }
    });
  }

  #[test]
  fn test_event_target() {
    run_in_task(|mut cx| {
      let mut isolate = setup();
      isolate
        .execute("event_target_test.js", include_str!("event_target_test.js"))
        .unwrap();
      if let Poll::Ready(Err(_)) = isolate.poll_event_loop(&mut cx) {
        unreachable!();
      }
    });
  }
}
