// Copyright 2018-2026 the Deno authors. MIT license.

use deno_core::op2;
use deno_core::v8;

fn set_i32(
  scope: &mut v8::PinScope,
  obj: v8::Local<v8::Object>,
  name: &str,
  value: i32,
) {
  let key = v8::String::new(scope, name).unwrap();
  let value = v8::Integer::new(scope, value);
  obj.set(scope, key.into(), value.into());
}

fn set_str(
  scope: &mut v8::PinScope,
  obj: v8::Local<v8::Object>,
  name: &str,
  value: &str,
) {
  let key = v8::String::new(scope, name).unwrap();
  let value = v8::String::new(scope, value).unwrap();
  obj.set(scope, key.into(), value.into());
}

fn set_bool(
  scope: &mut v8::PinScope,
  obj: v8::Local<v8::Object>,
  name: &str,
  value: bool,
) {
  let key = v8::String::new(scope, name).unwrap();
  let value = v8::Boolean::new(scope, value);
  obj.set(scope, key.into(), value.into());
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

fn core_object<'s>(
  scope: &mut v8::PinScope<'s, '_>,
) -> v8::Local<'s, v8::Object> {
  let context = scope.get_current_context();
  let global = context.global(scope);
  let deno_key = v8::String::new(scope, "Deno").unwrap();
  let core_key = v8::String::new(scope, "core").unwrap();
  let deno = global.get(scope, deno_key.into()).unwrap();
  let deno = v8::Local::<v8::Object>::try_from(deno).unwrap();
  let core = deno.get(scope, core_key.into()).unwrap();
  v8::Local::<v8::Object>::try_from(core).unwrap()
}

fn core_ops<'s>(scope: &mut v8::PinScope<'s, '_>) -> v8::Local<'s, v8::Object> {
  let core = core_object(scope);
  let ops_key = v8::String::new(scope, "ops").unwrap();
  let ops = core.get(scope, ops_key.into()).unwrap();
  v8::Local::<v8::Object>::try_from(ops).unwrap()
}

fn get_op<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  name: &str,
) -> v8::Local<'s, v8::Value> {
  let ops = core_ops(scope);
  let key = v8::String::new(scope, name).unwrap();
  ops.get(scope, key.into()).unwrap()
}

fn set_op_alias(
  scope: &mut v8::PinScope,
  obj: v8::Local<v8::Object>,
  export_name: &str,
  op_name: &str,
) {
  let op = get_op(scope, op_name);
  let key = v8::String::new(scope, export_name).unwrap();
  obj.set(scope, key.into(), op);
}

fn set_core_alias(
  scope: &mut v8::PinScope,
  obj: v8::Local<v8::Object>,
  export_name: &str,
) {
  let core = core_object(scope);
  let key = v8::String::new(scope, export_name).unwrap();
  let value = core.get(scope, key.into()).unwrap();
  obj.set(scope, key.into(), value);
}

#[op2]
pub fn op_node_internal_binding_encodings<'s>(
  scope: &mut v8::PinScope<'s, '_>,
) -> v8::Local<'s, v8::Object> {
  let obj = v8::Object::new(scope);
  for (name, value) in [
    ("ASCII", 0),
    ("UTF8", 1),
    ("BASE64", 2),
    ("UCS2", 3),
    ("BINARY", 4),
    ("HEX", 5),
    ("BUFFER", 6),
    ("BASE64URL", 7),
    ("LATIN1", 4),
  ] {
    set_i32(scope, obj, name, value);
  }
  for (value, name) in [
    ("0", "ASCII"),
    ("1", "UTF8"),
    ("2", "BASE64"),
    ("3", "UCS2"),
    ("4", "LATIN1"),
    ("5", "HEX"),
    ("6", "BUFFER"),
    ("7", "BASE64URL"),
  ] {
    set_str(scope, obj, value, name);
  }
  obj
}

#[op2]
#[string]
pub fn op_node_ares_strerror(#[smi] code: i32) -> &'static str {
  const ERROR_TEXT: &[&str] = &[
    "Successful completion",
    "DNS server returned answer with no data",
    "DNS server claims query was misformatted",
    "DNS server returned general failure",
    "Domain name not found",
    "DNS server does not implement requested operation",
    "DNS server refused query",
    "Misformatted DNS query",
    "Misformatted domain name",
    "Unsupported address family",
    "Misformatted DNS reply",
    "Could not contact DNS servers",
    "Timeout while contacting DNS servers",
    "End of file",
    "Error reading file",
    "Out of memory",
    "Channel is being destroyed",
    "Misformatted string",
    "Illegal flags specified",
    "Given hostname is not numeric",
    "Illegal hints flags specified",
    "c-ares library initialization not yet performed",
    "Error loading iphlpapi.dll",
    "Could not find GetNetworkParams function",
    "DNS query cancelled",
  ];
  ERROR_TEXT.get(code as usize).copied().unwrap_or("unknown")
}

#[op2]
pub fn op_node_internal_binding_ares<'s>(
  scope: &mut v8::PinScope<'s, '_>,
) -> v8::Local<'s, v8::Object> {
  let obj = v8::Object::new(scope);
  for (name, value) in [
    ("ARES_AI_CANONNAME", 1 << 0),
    ("ARES_AI_NUMERICHOST", 1 << 1),
    ("ARES_AI_PASSIVE", 1 << 2),
    ("ARES_AI_NUMERICSERV", 1 << 3),
    ("AI_V4MAPPED", 1 << 4),
    ("AI_ALL", 1 << 5),
    ("AI_ADDRCONFIG", 1 << 6),
    ("ARES_AI_NOSORT", 1 << 7),
    ("ARES_AI_ENVHOSTS", 1 << 8),
  ] {
    set_i32(scope, obj, name, value);
  }

  set_op_alias(scope, obj, "ares_strerror", "op_node_ares_strerror");
  obj
}

#[op2]
pub fn op_node_internal_binding_string_decoder<'s>(
  scope: &mut v8::PinScope<'s, '_>,
) -> v8::Local<'s, v8::Object> {
  let obj = v8::Object::new(scope);
  let encodings = v8::Array::new(scope, 8);
  for (index, encoding) in [
    (0, "ascii"),
    (1, "utf8"),
    (2, "base64"),
    (3, "utf16le"),
    (4, "latin1"),
    (5, "hex"),
    (6, "buffer"),
    (7, "base64url"),
  ] {
    let value = v8::String::new(scope, encoding).unwrap();
    encodings.set_index(scope, index, value.into());
  }
  set_value(scope, obj, "encodings", encodings.into());
  let default_obj = v8::Object::new(scope);
  set_value(scope, default_obj, "encodings", encodings.into());
  set_value(scope, obj, "default", default_obj.into());
  obj
}

#[op2]
pub fn op_node_internal_binding_symbols<'s>(
  scope: &mut v8::PinScope<'s, '_>,
) -> v8::Local<'s, v8::Object> {
  let obj = v8::Object::new(scope);
  let async_id_description = v8::String::new(scope, "asyncIdSymbol").unwrap();
  let async_id = v8::Symbol::new(scope, Some(async_id_description));
  set_value(scope, obj, "asyncIdSymbol", async_id.into());
  let owner_description = v8::String::new(scope, "ownerSymbol").unwrap();
  let owner = v8::Symbol::new(scope, Some(owner_description));
  set_value(scope, obj, "ownerSymbol", owner.into());
  obj
}

#[op2]
pub fn op_node_internal_binding_inspector<'s>(
  scope: &mut v8::PinScope<'s, '_>,
) -> v8::Local<'s, v8::Object> {
  let obj = v8::Object::new(scope);
  set_op_alias(scope, obj, "isEnabled", "op_inspector_enabled");
  obj
}

#[op2]
pub fn op_node_internal_binding_handle_wrap<'s>(
  scope: &mut v8::PinScope<'s, '_>,
) -> v8::Local<'s, v8::Object> {
  let obj = v8::Object::new(scope);
  set_op_alias(scope, obj, "HandleWrap", "HandleWrap");
  obj
}

#[op2]
pub fn op_node_internal_binding_tty_wrap<'s>(
  scope: &mut v8::PinScope<'s, '_>,
) -> v8::Local<'s, v8::Object> {
  let obj = v8::Object::new(scope);
  let tty = get_op(scope, "TTY");
  let tty_fn = v8::Local::<v8::Function>::try_from(tty).unwrap();
  let prototype_key = v8::String::new(scope, "prototype").unwrap();
  let prototype = tty_fn.get(scope, prototype_key.into()).unwrap();
  let prototype = v8::Local::<v8::Object>::try_from(prototype).unwrap();
  set_bool(scope, prototype, "isStreamBase", true);
  let key = v8::String::new(scope, "TTY").unwrap();
  obj.set(scope, key.into(), tty_fn.into());
  obj
}

#[op2]
pub fn op_node_internal_binding_libuv_winerror<'s>(
  scope: &mut v8::PinScope<'s, '_>,
) -> v8::Local<'s, v8::Object> {
  let obj = v8::Object::new(scope);
  set_op_alias(scope, obj, "uvTranslateSysError", "op_node_sys_to_uv_error");
  obj
}

#[op2]
pub fn op_node_internal_binding_types<'s>(
  scope: &mut v8::PinScope<'s, '_>,
) -> v8::Local<'s, v8::Object> {
  let obj = v8::Object::new(scope);
  for name in [
    "isAnyArrayBuffer",
    "isArgumentsObject",
    "isArrayBuffer",
    "isAsyncFunction",
    "isBigIntObject",
    "isBooleanObject",
    "isBoxedPrimitive",
    "isDataView",
    "isDate",
    "isGeneratorFunction",
    "isGeneratorObject",
    "isMap",
    "isMapIterator",
    "isModuleNamespaceObject",
    "isNativeError",
    "isNumberObject",
    "isPromise",
    "isProxy",
    "isRegExp",
    "isSet",
    "isSetIterator",
    "isSharedArrayBuffer",
    "isStringObject",
    "isSymbolObject",
    "isTypedArray",
    "isWeakMap",
    "isWeakSet",
  ] {
    set_core_alias(scope, obj, name);
  }
  obj
}
