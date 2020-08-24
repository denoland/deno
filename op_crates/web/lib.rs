// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use deno_core::crate_modules;
use std::path::PathBuf;

crate_modules!();

pub struct WebScripts {
  pub abort_signal: String,
  pub declaration: String,
  pub dom_exception: String,
  pub event: String,
  pub text_encoding: String,
}

fn get_str_path(file_name: &str) -> String {
  PathBuf::from(DENO_CRATE_PATH)
    .join(file_name)
    .to_string_lossy()
    .to_string()
}

pub fn get_scripts() -> WebScripts {
  WebScripts {
    abort_signal: get_str_path("02_abort_signal.js"),
    declaration: get_str_path("lib.deno_web.d.ts"),
    dom_exception: get_str_path("00_dom_exception.js"),
    event: get_str_path("01_event.js"),
    text_encoding: get_str_path("08_text_encoding.js"),
  }
}

#[cfg(test)]
mod tests {
  use deno_core::js_check;
  use deno_core::CoreIsolate;
  use deno_core::StartupData;
  use futures::future::lazy;
  use futures::future::FutureExt;
  use futures::task::Context;
  use futures::task::Poll;

  fn run_in_task<F>(f: F)
  where
    F: FnOnce(&mut Context) + Send + 'static,
  {
    futures::executor::block_on(lazy(move |cx| f(cx)));
  }

  fn setup() -> CoreIsolate {
    let mut isolate = CoreIsolate::new(StartupData::None, false);
    js_check(
      isolate
        .execute("00_dom_exception.js", include_str!("00_dom_exception.js")),
    );
    js_check(isolate.execute("01_event.js", include_str!("01_event.js")));
    js_check(
      isolate.execute("02_abort_signal.js", include_str!("02_abort_signal.js")),
    );
    js_check(
      isolate
        .execute("08_text_encoding.js", include_str!("08_text_encoding.js")),
    );
    isolate
  }

  #[test]
  fn test_abort_controller() {
    run_in_task(|mut cx| {
      let mut isolate = setup();
      js_check(isolate.execute(
        "abort_controller_test.js",
        include_str!("abort_controller_test.js"),
      ));
      if let Poll::Ready(Err(_)) = isolate.poll_unpin(&mut cx) {
        unreachable!();
      }
    });
  }

  #[test]
  fn test_event() {
    run_in_task(|mut cx| {
      let mut isolate = setup();
      js_check(isolate.execute("event_test.js", include_str!("event_test.js")));
      if let Poll::Ready(Err(_)) = isolate.poll_unpin(&mut cx) {
        unreachable!();
      }
    });
  }

  #[test]
  fn test_event_target() {
    run_in_task(|mut cx| {
      let mut isolate = setup();
      js_check(
        isolate.execute(
          "event_target_test.js",
          include_str!("event_target_test.js"),
        ),
      );
      if let Poll::Ready(Err(_)) = isolate.poll_unpin(&mut cx) {
        unreachable!();
      }
    });
  }

  #[test]
  fn test_text_encoding() {
    run_in_task(|mut cx| {
      let mut isolate = setup();
      js_check(isolate.execute(
        "text_encoding_test.js",
        include_str!("text_encoding_test.js"),
      ));
      if let Poll::Ready(Err(_)) = isolate.poll_unpin(&mut cx) {
        unreachable!();
      }
    });
  }
}
