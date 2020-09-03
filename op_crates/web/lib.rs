// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use deno_core::js_check;
use deno_core::CoreIsolate;
use std::path::PathBuf;

pub fn init(isolate: &mut CoreIsolate) {
  let files = vec![
    get_path("00_dom_exception.js"),
    get_path("01_event.js"),
    get_path("02_abort_signal.js"),
    get_path("08_text_encoding.js"),
  ];
  for file in files {
    println!("cargo:rerun-if-changed={}", file.display());
    js_check(isolate.execute(
      &file.to_string_lossy(),
      &std::fs::read_to_string(&file).unwrap(),
    ));
  }
}

pub fn get_declaration() -> PathBuf {
  get_path("lib.deno_web.d.ts")
}

fn get_path(file_name: &str) -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(file_name)
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
    crate::init(&mut isolate);
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
