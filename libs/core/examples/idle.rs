// Copyright 2018-2026 the Deno authors. MIT license.
//! Minimal deno_core embedder that creates a JsRuntime and idles forever.
//! Used to measure baseline V8 + deno_core RSS without any deno extensions.

use std::rc::Rc;

use deno_core::*;

fn main() {
  let mut runtime = JsRuntime::new(RuntimeOptions::default());
  // Just keep the runtime alive — drive the event loop forever.
  let local = tokio::task::LocalSet::new();
  let rt = tokio::runtime::Builder::new_current_thread()
    .enable_all()
    .build()
    .unwrap();
  local.block_on(&rt, async move {
    // Touch the runtime so V8 is fully initialized
    runtime
      .execute_script("<idle>", "globalThis.__keepalive = 1;")
      .unwrap();
    let _ = std::future::pending::<()>().await;
    drop(Rc::new(()));
  });
}
