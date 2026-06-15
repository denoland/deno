// Copyright 2018-2026 the Deno authors. MIT license.

use deno_core::op2;
use deno_core::v8;
use deno_core::v8::ExternalReference;
use deno_core::v8::MapFnTo;

const UNHEX_TABLE: [i8; 256] = {
  let mut table = [-1; 256];
  let mut i = 0;
  while i < 10 {
    table[b'0' as usize + i] = i as i8;
    i += 1;
  }
  i = 0;
  while i < 6 {
    table[b'A' as usize + i] = (10 + i) as i8;
    table[b'a' as usize + i] = (10 + i) as i8;
    i += 1;
  }
  table
};

fn throw_type_error(scope: &mut v8::PinScope, message: &str) {
  let message = v8::String::new(scope, message).unwrap();
  let exception = v8::Exception::type_error(scope, message);
  scope.throw_exception(exception);
}

fn throw_error(scope: &mut v8::PinScope, message: &str) {
  let message = v8::String::new(scope, message).unwrap();
  let exception = v8::Exception::error(scope, message);
  scope.throw_exception(exception);
}

fn set_value(
  scope: &mut v8::PinScope,
  obj: v8::Local<v8::Object>,
  name: &str,
  value: v8::Local<v8::Value>,
) {
  let key = v8::String::new(scope, name).unwrap();
  obj.set(scope, key.into(), value);
}

fn set_function(
  scope: &mut v8::PinScope,
  obj: v8::Local<v8::Object>,
  name: &str,
  function: v8::Local<v8::Function>,
) {
  set_value(scope, obj, name, function.into());
}

fn arg_string(
  scope: &mut v8::PinScope,
  args: &v8::FunctionCallbackArguments,
) -> Option<String> {
  let value = args.get(0);
  let value = value.to_string(scope)?;
  Some(value.to_rust_string_lossy(scope))
}

fn uint8_array<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  bytes: Vec<u8>,
) -> v8::Local<'s, v8::Uint8Array> {
  let len = bytes.len();
  let store = v8::ArrayBuffer::new_backing_store_from_vec(bytes).make_shared();
  let array_buffer = v8::ArrayBuffer::with_backing_store(scope, &store);
  v8::Uint8Array::new(scope, array_buffer, 0, len).unwrap()
}

fn int8_array<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  bytes: Vec<u8>,
) -> v8::Local<'s, v8::Int8Array> {
  let len = bytes.len();
  let store = v8::ArrayBuffer::new_backing_store_from_vec(bytes).make_shared();
  let array_buffer = v8::ArrayBuffer::with_backing_store(scope, &store);
  v8::Int8Array::new(scope, array_buffer, 0, len).unwrap()
}

pub(crate) fn simdutf_base64_decode(input: &[u8]) -> Option<Vec<u8>> {
  use v8::simdutf;

  let max_len = simdutf::maximal_binary_length_from_base64(input);
  let mut output = vec![0; max_len];
  // SAFETY: `output` is allocated to simdutf's reported maximum decoded length.
  let result = unsafe {
    simdutf::base64_to_binary(
      input,
      &mut output,
      simdutf::Base64Options::Default,
      simdutf::LastChunkHandling::Loose,
    )
  };
  if result.is_ok() {
    output.truncate(result.count);
    Some(output)
  } else {
    None
  }
}

pub(crate) fn base64_clean(input: &str) -> String {
  let trimmed = if let Some(eq_index) = input.find('=') {
    input[..eq_index].trim_start()
  } else {
    input.trim()
  };
  let mut cleaned = String::with_capacity(trimmed.len() + 2);
  for ch in trimmed.chars() {
    if matches!(ch, '+' | '/' | '-' | '_') || ch.is_ascii_alphanumeric() {
      cleaned.push(ch);
    }
  }
  let len = cleaned.len();
  if len < 2 {
    return String::new();
  }
  match len % 4 {
    0 => cleaned,
    1 => {
      cleaned.pop();
      cleaned
    }
    2 => {
      cleaned.push_str("==");
      cleaned
    }
    3 => {
      cleaned.push('=');
      cleaned
    }
    _ => unreachable!(),
  }
}

pub(crate) fn ascii_to_bytes(str: &str) -> Vec<u8> {
  str.encode_utf16().map(|c| c as u8).collect()
}

pub(crate) fn base64_to_bytes(str: &str) -> Option<Vec<u8>> {
  simdutf_base64_decode(str.as_bytes()).or_else(|| {
    let standard = str.replace('-', "+").replace('_', "/");
    let cleaned = base64_clean(&standard);
    simdutf_base64_decode(cleaned.as_bytes())
  })
}

pub(crate) fn base64_url_to_bytes(str: &str) -> Option<Vec<u8>> {
  let cleaned = base64_clean(str);
  let standard = cleaned.replace('-', "+").replace('_', "/");
  simdutf_base64_decode(standard.as_bytes())
}

pub(crate) fn hex_to_bytes(str: &str) -> Vec<u8> {
  let code_units = str.encode_utf16().collect::<Vec<_>>();
  let length = code_units.len() >> 1;
  let mut bytes = Vec::with_capacity(length);
  for i in 0..length {
    let a = UNHEX_TABLE[(code_units[i * 2] & 0xff) as usize];
    let b = UNHEX_TABLE[(code_units[i * 2 + 1] & 0xff) as usize];
    if a == -1 || b == -1 {
      break;
    }
    bytes.push(((a << 4) | b) as u8);
  }
  bytes
}

pub(crate) fn utf16le_to_bytes(
  str: &str,
  max_length: Option<usize>,
) -> Vec<u8> {
  let length = max_length.unwrap_or_else(|| str.encode_utf16().count() * 2);
  let mut bytes = Vec::with_capacity(length);
  for code_unit in str.encode_utf16().take(length / 2) {
    bytes.push(code_unit as u8);
    bytes.push((code_unit >> 8) as u8);
  }
  bytes
}

fn ascii_to_bytes_callback(
  scope: &mut v8::PinScope,
  args: v8::FunctionCallbackArguments,
  mut rv: v8::ReturnValue,
) {
  let Some(str) = arg_string(scope, &args) else {
    throw_type_error(scope, "str must be a string");
    return;
  };
  let bytes = ascii_to_bytes(&str);
  let bytes = uint8_array(scope, bytes);
  rv.set(bytes.into());
}

fn base64_to_bytes_callback(
  scope: &mut v8::PinScope,
  args: v8::FunctionCallbackArguments,
  mut rv: v8::ReturnValue,
) {
  let Some(str) = arg_string(scope, &args) else {
    throw_type_error(scope, "str must be a string");
    return;
  };
  let bytes = base64_to_bytes(&str);
  let Some(bytes) = bytes else {
    throw_error(scope, "Failed to decode base64");
    return;
  };
  let bytes = uint8_array(scope, bytes);
  rv.set(bytes.into());
}

fn base64_url_to_bytes_callback(
  scope: &mut v8::PinScope,
  args: v8::FunctionCallbackArguments,
  mut rv: v8::ReturnValue,
) {
  let Some(str) = arg_string(scope, &args) else {
    throw_type_error(scope, "str must be a string");
    return;
  };
  let Some(bytes) = base64_url_to_bytes(&str) else {
    throw_error(scope, "Failed to decode base64url");
    return;
  };
  let bytes = uint8_array(scope, bytes);
  rv.set(bytes.into());
}

fn hex_to_bytes_callback(
  scope: &mut v8::PinScope,
  args: v8::FunctionCallbackArguments,
  mut rv: v8::ReturnValue,
) {
  let Some(str) = arg_string(scope, &args) else {
    throw_type_error(scope, "str must be a string");
    return;
  };
  let bytes = hex_to_bytes(&str);
  let bytes = uint8_array(scope, bytes);
  rv.set(bytes.into());
}

fn utf16le_to_bytes_callback(
  scope: &mut v8::PinScope,
  args: v8::FunctionCallbackArguments,
  mut rv: v8::ReturnValue,
) {
  let Some(str) = arg_string(scope, &args) else {
    throw_type_error(scope, "str must be a string");
    return;
  };
  let mut length = str.encode_utf16().count() * 2;
  let units = args.get(1);
  if !units.is_undefined() && units.boolean_value(scope) {
    let units = units.uint32_value(scope).unwrap_or(0) as usize;
    length = length.min((units >> 1) * 2);
  }
  let bytes = utf16le_to_bytes(&str, Some(length));
  let bytes = uint8_array(scope, bytes);
  rv.set(bytes.into());
}

pub(crate) fn external_references() -> [ExternalReference; 5] {
  [
    ASCII_TO_BYTES_CALLBACK.with(|callback| ExternalReference {
      function: *callback,
    }),
    BASE64_TO_BYTES_CALLBACK.with(|callback| ExternalReference {
      function: *callback,
    }),
    BASE64_URL_TO_BYTES_CALLBACK.with(|callback| ExternalReference {
      function: *callback,
    }),
    HEX_TO_BYTES_CALLBACK.with(|callback| ExternalReference {
      function: *callback,
    }),
    UTF16LE_TO_BYTES_CALLBACK.with(|callback| ExternalReference {
      function: *callback,
    }),
  ]
}

thread_local! {
  static ASCII_TO_BYTES_CALLBACK: v8::FunctionCallback = ascii_to_bytes_callback.map_fn_to();
  static BASE64_TO_BYTES_CALLBACK: v8::FunctionCallback = base64_to_bytes_callback.map_fn_to();
  static BASE64_URL_TO_BYTES_CALLBACK: v8::FunctionCallback = base64_url_to_bytes_callback.map_fn_to();
  static HEX_TO_BYTES_CALLBACK: v8::FunctionCallback = hex_to_bytes_callback.map_fn_to();
  static UTF16LE_TO_BYTES_CALLBACK: v8::FunctionCallback = utf16le_to_bytes_callback.map_fn_to();
}

fn function_from_callback<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  callback: v8::FunctionCallback,
) -> v8::Local<'s, v8::Function> {
  v8::FunctionTemplate::new_raw(scope, callback)
    .get_function(scope)
    .unwrap()
}

#[op2]
pub fn op_node_internal_binding_utils<'s>(
  scope: &mut v8::PinScope<'s, '_>,
) -> v8::Local<'s, v8::Object> {
  let obj = v8::Object::new(scope);

  let ascii_to_bytes = ASCII_TO_BYTES_CALLBACK
    .with(|callback| function_from_callback(scope, *callback));
  set_function(scope, obj, "asciiToBytes", ascii_to_bytes);

  let base64_to_bytes = BASE64_TO_BYTES_CALLBACK
    .with(|callback| function_from_callback(scope, *callback));
  set_function(scope, obj, "base64ToBytes", base64_to_bytes);

  let base64_url_to_bytes = BASE64_URL_TO_BYTES_CALLBACK
    .with(|callback| function_from_callback(scope, *callback));
  set_function(scope, obj, "base64UrlToBytes", base64_url_to_bytes);

  let hex_to_bytes = HEX_TO_BYTES_CALLBACK
    .with(|callback| function_from_callback(scope, *callback));
  set_function(scope, obj, "hexToBytes", hex_to_bytes);

  let utf16le_to_bytes = UTF16LE_TO_BYTES_CALLBACK
    .with(|callback| function_from_callback(scope, *callback));
  set_function(scope, obj, "utf16leToBytes", utf16le_to_bytes);

  let table = UNHEX_TABLE
    .iter()
    .map(|value| *value as u8)
    .collect::<Vec<_>>();
  let table = int8_array(scope, table);
  set_value(scope, obj, "unhexTable", table.into());

  obj
}
