// Full deno desktop app for iOS: links the denort_desktop runtime (Deno + V8)
// + the laufey WKWebView backend, driven by UIApplicationMain.
extern crate denort; // denort_desktop's lib; provides laufey_runtime_init/start

extern "C" {
  fn laufey_ios_main() -> std::os::raw::c_int;
}

fn main() {
  unsafe {
    laufey_ios_main();
  }
}
