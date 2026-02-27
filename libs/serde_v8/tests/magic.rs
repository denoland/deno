// Copyright 2018-2025 the Deno authors. MIT license.

use serde::Deserialize;
use serde::Serialize;

use serde_v8::Result;
use serde_v8_utilities::js_exec;
use serde_v8_utilities::v8_do;

#[derive(Serialize, Deserialize)]
struct MagicOp {
  #[allow(unused)]
  pub a: u64,
  #[allow(unused)]
  pub b: u64,
  pub c: String,
  #[allow(unused)]
  pub operator: Option<String>,
}

#[test]
fn magic_basic() {
  v8_do(|| {
    let isolate = &mut v8::Isolate::new(v8::CreateParams::default());
    v8::scope!(handle_scope, isolate);
    let context = v8::Context::new(handle_scope, Default::default());
    let scope = &mut v8::ContextScope::new(handle_scope, context);

    // Decode
    let v = js_exec(scope, "({a: 1, b: 3, c: 'abracadabra'})");
    let mop: MagicOp = serde_v8::from_v8(scope, v).unwrap();
    assert_eq!(mop.a, 1);
    assert_eq!(mop.b, 3);
    assert_eq!(mop.c, "abracadabra");
    assert!(mop.operator.is_none());

    // Encode
    let vc = serde_v8::to_v8(scope, mop).unwrap();
    // JSON stringify & check
    let json = v8::json::stringify(scope, vc).unwrap();
    let s2 = json.to_rust_string_lossy(scope);
    assert_eq!(s2, r#"{"a":1,"b":3,"c":"abracadabra","operator":null}"#);
  })
}

#[test]
fn magic_buffer() {
  v8_do(|| {
    // Init isolate
    let isolate = &mut v8::Isolate::new(v8::CreateParams::default());
    v8::scope!(handle_scope, isolate);
    let context = v8::Context::new(handle_scope, Default::default());
    let scope = &mut v8::ContextScope::new(handle_scope, context);
    let global = context.global(scope);

    // Simple buffer
    let v8_array = js_exec(scope, "new Uint8Array([1,2,3,4,5])");
    let zbuf: serde_v8::JsBuffer = serde_v8::from_v8(scope, v8_array).unwrap();
    assert_eq!(&*zbuf, &[1, 2, 3, 4, 5]);

    // Multi buffers
    let v8_arrays =
      js_exec(scope, "[new Uint8Array([1,2]), new Uint8Array([3,4,5])]");
    let (z1, z2): (serde_v8::JsBuffer, serde_v8::JsBuffer) =
      serde_v8::from_v8(scope, v8_arrays).unwrap();
    assert_eq!(&*z1, &[1, 2]);
    assert_eq!(&*z2, &[3, 4, 5]);

    // Wrapped in option, like our current op-ABI
    let v8_array = js_exec(scope, "new Uint8Array([1,2,3,4,5])");
    let zbuf: Option<serde_v8::JsBuffer> =
      serde_v8::from_v8(scope, v8_array).unwrap();
    assert_eq!(&*zbuf.unwrap(), &[1, 2, 3, 4, 5]);

    // Observe mutation in JS
    let v8_array = js_exec(scope, "new Uint8Array([1,2,3,4,5])");
    let mut zbuf: serde_v8::JsBuffer =
      serde_v8::from_v8(scope, v8_array).unwrap();
    let key = serde_v8::to_v8(scope, "t1").unwrap();
    global.set(scope, key, v8_array);
    (&mut *zbuf)[2] = 42;
    let eq = js_exec(scope, "t1[2] === 42");
    assert!(eq.is_true());

    // Shared buffers
    let v8_array =
      js_exec(scope, "new Uint8Array(new SharedArrayBuffer([1,2,3,4,5]))");
    let zbuf: Result<serde_v8::JsBuffer> = serde_v8::from_v8(scope, v8_array);
    assert!(zbuf.is_err());

    // Serialization
    let buf: Vec<u8> = vec![1, 2, 3, 99, 5];
    let zbuf: serde_v8::ToJsBuffer = buf.into();
    let v8_value = serde_v8::to_v8(scope, zbuf).unwrap();
    let key = serde_v8::to_v8(scope, "t2").unwrap();
    global.set(scope, key, v8_value);
    let eq = js_exec(scope, "t2[3] === 99");
    assert!(eq.is_true());

    // Composite Serialization
    #[derive(serde::Serialize)]
    struct Wrapper {
      a: serde_v8::ToJsBuffer,
      b: serde_v8::ToJsBuffer,
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

#[test]
fn magic_byte_string() {
  v8_do(|| {
    // Init isolate
    let isolate = &mut v8::Isolate::new(v8::CreateParams::default());
    v8::scope!(handle_scope, isolate);
    let context = v8::Context::new(handle_scope, Default::default());
    let scope = &mut v8::ContextScope::new(handle_scope, context);
    let global = context.global(scope);

    // JS string to ByteString
    let v8_string = js_exec(scope, "'test \\0\\t\\n\\r\\x7F\\x80áþÆñ'");
    let rust_reflex: serde_v8::ByteString =
      serde_v8::from_v8(scope, v8_string).unwrap();
    assert_eq!(
      rust_reflex.as_slice(),
      b"test \0\t\n\r\x7F\x80\xE1\xFE\xC6\xF1"
    );

    // Non-Latin-1 characters
    let v8_string = js_exec(scope, "'日本語'");
    let rust_reflex: Result<serde_v8::ByteString> =
      serde_v8::from_v8(scope, v8_string);
    assert!(rust_reflex.is_err());

    // Windows-1252 characters that aren't Latin-1
    let v8_string = js_exec(scope, "'œ'");
    let rust_reflex: Result<serde_v8::ByteString> =
      serde_v8::from_v8(scope, v8_string);
    assert!(rust_reflex.is_err());

    // ByteString to JS string
    let expected = "a\x00sf:~\x7Fá\u{009C}þ\u{008A}";
    let buf: Vec<u8> = b"a\x00sf:~\x7F\xE1\x9C\xFE\x8A".as_ref().into();
    let zbuf = serde_v8::ByteString::from(buf);
    let v8_value = serde_v8::to_v8(scope, zbuf).unwrap();
    let key = serde_v8::to_v8(scope, "actual").unwrap();
    global.set(scope, key, v8_value);
    let v8_value_expected = serde_v8::to_v8(scope, expected).unwrap();
    let key_expected = serde_v8::to_v8(scope, "expected").unwrap();
    global.set(scope, key_expected, v8_value_expected);
    let eq = js_exec(scope, "actual === expected");
    assert!(eq.is_true());
  })
}

#[test]
fn magic_value() {
  use serde_v8_utilities::js_exec;
  use serde_v8_utilities::v8_do;

  struct TestLocal<'a>(v8::Local<'a, v8::Value>);
  impl<'de> serde::Deserialize<'de> for TestLocal<'_> {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
      D: serde::Deserializer<'de>,
    {
      let value = serde_v8::Value::deserialize(deserializer)?;
      let value = value.v8_value;
      Ok(Self(value))
    }
  }

  struct TestGlobal(v8::Global<v8::Value>);
  impl<'de> serde::Deserialize<'de> for TestGlobal {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
      D: serde::Deserializer<'de>,
    {
      let value = serde_v8::GlobalValue::deserialize(deserializer)?;
      let value = value.v8_value;
      Ok(Self(value))
    }
  }

  v8_do(|| {
    // Init isolate
    let isolate = &mut v8::Isolate::new(v8::CreateParams::default());
    v8::scope!(handle_scope, isolate);
    let context = v8::Context::new(handle_scope, Default::default());
    let scope = &mut v8::ContextScope::new(handle_scope, context);

    let v8_string = js_exec(scope, "'test'");
    let test: TestGlobal = serde_v8::from_v8(scope, v8_string).unwrap();
    let local = v8::Local::new(scope, test.0);
    let test = local.to_rust_string_lossy(scope);
    assert_eq!(test.as_str(), "test");

    let v8_string = js_exec(scope, "'test'");
    let test: TestLocal = serde_v8::from_v8(scope, v8_string).unwrap();
    let local = test.0;
    let test = local.to_rust_string_lossy(scope);
    assert_eq!(test.as_str(), "test");
  })
}
