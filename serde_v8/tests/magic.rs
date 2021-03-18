use rusty_v8 as v8;
use serde_v8;

use serde::{Deserialize, Serialize};

use serde_v8::utils::{js_exec, v8_init, v8_shutdown};
use std::convert::TryFrom;

#[derive(Deserialize)]
struct MagicOp<'s> {
  pub a: u64,
  pub b: u64,
  pub c: serde_v8::Value<'s>,
  pub operator: Option<String>,
}

#[derive(Serialize)]
struct MagicContainer<'s> {
  pub magic: bool,
  pub contains: serde_v8::Value<'s>,
}

#[test]
fn magic_basic() {
  v8_init();

  {
    let isolate = &mut v8::Isolate::new(v8::CreateParams::default());
    let handle_scope = &mut v8::HandleScope::new(isolate);
    let context = v8::Context::new(handle_scope);
    let scope = &mut v8::ContextScope::new(handle_scope, context);

    // Decode
    let v = js_exec(scope, "({a: 1, b: 3, c: 'abracadabra'})");
    let mop: MagicOp = serde_v8::from_v8(scope, v).unwrap();
    // Check string
    let v8_value: v8::Local<v8::Value> = mop.c.into();
    let vs = v8::Local::<v8::String>::try_from(v8_value).unwrap();
    let s = vs.to_rust_string_lossy(scope);
    assert_eq!(s, "abracadabra");

    // Encode
    let container = MagicContainer {
      magic: true,
      contains: v.into(),
    };
    let vc = serde_v8::to_v8(scope, container).unwrap();
    // JSON stringify & check
    let json = v8::json::stringify(scope, vc).unwrap();
    let s2 = json.to_rust_string_lossy(scope);
    assert_eq!(
      s2,
      r#"{"magic":true,"contains":{"a":1,"b":3,"c":"abracadabra"}}"#
    );
  }

  v8_shutdown();
}
