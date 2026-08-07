// Copyright 2018-2026 the Deno authors. MIT license.

// Full deno desktop app for iOS: links the denort_desktop runtime (Deno + V8)
// + the laufey WKWebView backend, driven by UIApplicationMain.
extern crate denort; // denort_desktop's lib; provides laufey_runtime_init/start

extern "C" {
  fn laufey_ios_main() -> std::os::raw::c_int;
}

fn main() {
  // SAFETY: `laufey_ios_main` is the C entrypoint exported by the linked
  // denort_desktop/laufey backend. It takes no arguments and drives the iOS
  // `UIApplicationMain` run loop; calling it once from `main` is its intended
  // FFI usage.
  unsafe {
    laufey_ios_main();
  }
}
