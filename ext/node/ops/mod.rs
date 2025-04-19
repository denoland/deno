// Copyright 2018-2025 the Deno authors. MIT license.

pub mod blocklist;
pub mod buffer;
pub mod crypto;
pub mod dns;
pub mod fs;
pub mod handle_wrap;
pub mod http;
pub mod http2;
pub mod idna;
pub mod inspector;
pub mod ipc;
pub mod os;
pub mod perf_hooks;
pub mod process;
pub mod require;
pub mod sqlite;
pub mod stream_wrap;
pub mod tls;
pub mod util;
pub mod v8;
pub mod vm;
pub mod winerror;
pub mod worker_threads;
pub mod zlib;

#[cfg(test)]
async fn js_test(ext: deno_core::Extension, source_code: &'static str) {
  use std::future::poll_fn;

  use deno_core::JsRuntime;
  use deno_core::RuntimeOptions;

  let mut runtime = JsRuntime::new(RuntimeOptions {
    extensions: vec![ext],
    ..Default::default()
  });
  runtime
    .execute_script("file://_wrap_test.js", source_code)
    .unwrap();

  let _ =
    poll_fn(move |cx| runtime.poll_event_loop(cx, Default::default())).await;
}
