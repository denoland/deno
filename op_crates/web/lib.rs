// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use deno_core::error::uri_error;
use deno_core::error::AnyError;
use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use deno_core::JsRuntime;
use deno_core::ZeroCopyBuf;
use idna::domain_to_ascii;
use idna::domain_to_ascii_strict;
use serde::Deserialize;
use std::path::PathBuf;

pub fn op_domain_to_ascii(
  _state: &mut deno_core::OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  #[derive(Deserialize)]
  #[serde(rename_all = "camelCase")]
  struct DomainToAscii {
    domain: String,
    be_strict: bool,
  }

  let args: DomainToAscii = serde_json::from_value(args)?;
  if args.be_strict {
    domain_to_ascii_strict(args.domain.as_str())
  } else {
    domain_to_ascii(args.domain.as_str())
  }
  .map_err(|err| {
    let message = format!("Invalid IDNA encoded domain name: {:?}", err);
    uri_error(message)
  })
  .map(|domain| json!(domain))
}

/// Load and execute the javascript code.
pub fn init(isolate: &mut JsRuntime) {
  let files = vec![
    (
      "deno:op_crates/web/00_dom_exception.js",
      include_str!("00_dom_exception.js"),
    ),
    (
      "deno:op_crates/web/01_event.js",
      include_str!("01_event.js"),
    ),
    (
      "deno:op_crates/web/02_abort_signal.js",
      include_str!("02_abort_signal.js"),
    ),
    (
      "deno:op_crates/web/03_global_interfaces.js",
      include_str!("03_global_interfaces.js"),
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
    isolate.execute(url, source_code).unwrap();
  }
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
        assert!(error_string.contains("deno:op_crates/web/01_event.js"));
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
