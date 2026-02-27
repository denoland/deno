// Copyright 2018-2025 the Deno authors. MIT license.

use std::sync::Once;

pub fn js_exec<'s, 'i>(
  scope: &mut v8::PinScope<'s, 'i>,
  src: &str,
) -> v8::Local<'s, v8::Value> {
  let code = v8::String::new(scope, src).unwrap();
  let script = v8::Script::compile(scope, code, None).unwrap();
  script.run(scope).unwrap()
}

pub fn v8_init() {
  let platform = v8::new_unprotected_default_platform(0, false).make_shared();
  v8::V8::initialize_platform(platform);
  v8::V8::initialize();
}

pub fn v8_shutdown() {
  // SAFETY: this is safe, because all isolates have been shut down already.
  unsafe {
    v8::V8::dispose();
  }
  v8::V8::dispose_platform();
}

pub fn v8_do(f: impl FnOnce()) {
  static V8_INIT: Once = Once::new();
  V8_INIT.call_once(|| {
    v8_init();
  });
  f();
  // v8_shutdown();
}
