// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
use rusty_v8 as v8;

use serde::{Deserialize, Serialize};

use serde_v8::utils::{js_exec, v8_do};
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
  v8_do(|| {
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
  })
}

#[test]
fn magic_buffer() {
  v8_do(|| {
    // Init isolate
    let isolate = &mut v8::Isolate::new(v8::CreateParams::default());
    let handle_scope = &mut v8::HandleScope::new(isolate);
    let context = v8::Context::new(handle_scope);
    let scope = &mut v8::ContextScope::new(handle_scope, context);
    let global = context.global(scope);

    // Simple buffer
    let v8_array = js_exec(scope, "new Uint8Array([1,2,3,4,5])");
    let zbuf: serde_v8::Buffer = serde_v8::from_v8(scope, v8_array).unwrap();
    assert_eq!(&*zbuf, &[1, 2, 3, 4, 5]);

    // Multi buffers
    let v8_arrays =
      js_exec(scope, "[new Uint8Array([1,2]), new Uint8Array([3,4,5])]");
    let (z1, z2): (serde_v8::Buffer, serde_v8::Buffer) =
      serde_v8::from_v8(scope, v8_arrays).unwrap();
    assert_eq!(&*z1, &[1, 2]);
    assert_eq!(&*z2, &[3, 4, 5]);

    // Wrapped in option, like our current op-ABI
    let v8_array = js_exec(scope, "new Uint8Array([1,2,3,4,5])");
    let zbuf: Option<serde_v8::Buffer> =
      serde_v8::from_v8(scope, v8_array).unwrap();
    assert_eq!(&*zbuf.unwrap(), &[1, 2, 3, 4, 5]);

    // Observe mutation in JS
    let v8_array = js_exec(scope, "new Uint8Array([1,2,3,4,5])");
    let mut zbuf: serde_v8::Buffer =
      serde_v8::from_v8(scope, v8_array).unwrap();
    let key = serde_v8::to_v8(scope, "t1").unwrap();
    global.set(scope, key, v8_array);
    (&mut *zbuf)[2] = 42;
    let eq = js_exec(scope, "t1[2] === 42");
    assert!(eq.is_true());

    // Serialization
    let buf: Vec<u8> = vec![1, 2, 3, 99, 5];
    let zbuf: serde_v8::Buffer = buf.into();
    let v8_value = serde_v8::to_v8(scope, zbuf).unwrap();
    let key = serde_v8::to_v8(scope, "t2").unwrap();
    global.set(scope, key, v8_value);
    let eq = js_exec(scope, "t2[3] === 99");
    assert!(eq.is_true());

    // Composite Serialization
    #[derive(serde::Serialize)]
    struct Wrapper {
      a: serde_v8::Buffer,
      b: serde_v8::Buffer,
    }
    let buf1: Vec<u8> = vec![1, 2, 33, 4, 5];
    let buf2: Vec<u8> = vec![5, 4, 3, 2, 11];
    let wrapped = Wrapper {
      a: buf1.into(),
      b: buf2.into(),
    };
    let v8_value = serde_v8::to_v8(scope, wrapped).unwrap();
    let key = serde_v8::to_v8(scope, "t3").unwrap();
    global.set(scope, key, v8_value);
    let eq = js_exec(scope, "t3.a[2] === 33");
    assert!(eq.is_true());
    let eq = js_exec(scope, "t3.b[4] === 11");
    assert!(eq.is_true());
  })
}
