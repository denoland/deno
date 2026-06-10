// Copyright 2018-2026 the Deno authors. MIT license.

use deno_core::op2;
use deno_core::v8;
use deno_core::v8::ExternalReference;
use deno_core::v8::MapFnTo;

const CONSTANTS: &[&str] = &[
  "kInit",
  "kBefore",
  "kAfter",
  "kDestroy",
  "kPromiseResolve",
  "kTotals",
  "kCheck",
  "kExecutionAsyncId",
  "kTriggerAsyncId",
  "kAsyncIdCounter",
  "kDefaultTriggerAsyncId",
  "kUsesExecutionAsyncResource",
  "kStackLength",
];

const UID_FIELDS: &[&str] = &[
  "kExecutionAsyncId",
  "kTriggerAsyncId",
  "kDefaultTriggerAsyncId",
  "kUidFieldsCount",
];

const PROVIDER_TYPES: &[&str] = &[
  "NONE",
  "DIRHANDLE",
  "DNSCHANNEL",
  "ELDHISTOGRAM",
  "FILEHANDLE",
  "FILEHANDLECLOSEREQ",
  "FIXEDSIZEBLOBCOPY",
  "FSEVENTWRAP",
  "FSREQCALLBACK",
  "FSREQPROMISE",
  "GETADDRINFOREQWRAP",
  "GETNAMEINFOREQWRAP",
  "HEAPSNAPSHOT",
  "HTTP2SESSION",
  "HTTP2STREAM",
  "HTTP2PING",
  "HTTP2SETTINGS",
  "HTTPINCOMINGMESSAGE",
  "HTTPCLIENTREQUEST",
  "JSSTREAM",
  "JSUDPWRAP",
  "MESSAGEPORT",
  "PIPECONNECTWRAP",
  "PIPESERVERWRAP",
  "PIPEWRAP",
  "PROCESSWRAP",
  "PROMISE",
  "QUERYWRAP",
  "SHUTDOWNWRAP",
  "SIGNALWRAP",
  "STATWATCHER",
  "STREAMPIPE",
  "TCPCONNECTWRAP",
  "TCPSERVERWRAP",
  "TCPWRAP",
  "TLSWRAP",
  "TTYWRAP",
  "UDPSENDWRAP",
  "UDPWRAP",
  "SIGINTWATCHDOG",
  "WORKER",
  "WORKERHEAPSNAPSHOT",
  "WRITEWRAP",
  "ZLIB",
];

fn set_value(
  scope: &mut v8::PinScope,
  obj: v8::Local<v8::Object>,
  name: &str,
  value: v8::Local<v8::Value>,
) {
  let key = v8::String::new(scope, name).unwrap();
  obj.set(scope, key.into(), value);
}

fn set_number(
  scope: &mut v8::PinScope,
  obj: v8::Local<v8::Object>,
  name: &str,
  value: u32,
) {
  let value = v8::Integer::new_from_unsigned(scope, value).into();
  set_value(scope, obj, name, value);
}

fn create_enum_object<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  names: &[&str],
) -> v8::Local<'s, v8::Object> {
  let obj = v8::Object::new(scope);
  for (index, name) in names.iter().enumerate() {
    set_number(scope, obj, name, index as u32);
    let key = v8::Integer::new_from_unsigned(scope, index as u32);
    let value = v8::String::new(scope, name).unwrap();
    obj.set(scope, key.into(), value.into());
  }
  obj
}

fn create_uint32_array<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  length: usize,
) -> v8::Local<'s, v8::Uint32Array> {
  let array_buffer = v8::ArrayBuffer::new(scope, length * 4);
  v8::Uint32Array::new(scope, array_buffer, 0, length).unwrap()
}

fn create_float64_array<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  length: usize,
) -> v8::Local<'s, v8::Float64Array> {
  let array_buffer = v8::ArrayBuffer::new(scope, length * 8);
  v8::Float64Array::new(scope, array_buffer, 0, length).unwrap()
}

fn register_destroy_hook_callback(
  _scope: &mut v8::PinScope,
  _args: v8::FunctionCallbackArguments,
  _rv: v8::ReturnValue,
) {
}

pub(crate) fn external_references() -> [ExternalReference; 1] {
  [ExternalReference {
    function: register_destroy_hook_callback.map_fn_to(),
  }]
}

#[op2]
pub fn op_node_internal_binding_async_wrap<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  async_wrap: v8::Local<'s, v8::Value>,
  new_async_id: v8::Local<'s, v8::Value>,
) -> v8::Local<'s, v8::Object> {
  let obj = v8::Object::new(scope);

  let async_hook_fields = create_uint32_array(scope, CONSTANTS.len() * 2);
  set_value(scope, obj, "async_hook_fields", async_hook_fields.into());

  let async_id_fields = create_float64_array(scope, UID_FIELDS.len() * 2);
  let default_trigger_async_id = v8::Number::new(scope, -1.0);
  async_id_fields.set_index(scope, 2, default_trigger_async_id.into());
  set_value(scope, obj, "asyncIdFields", async_id_fields.into());

  set_value(scope, obj, "AsyncWrap", async_wrap);

  let register_destroy_hook =
    v8::FunctionTemplate::new(scope, register_destroy_hook_callback)
      .get_function(scope)
      .unwrap();
  set_value(
    scope,
    obj,
    "registerDestroyHook",
    register_destroy_hook.into(),
  );

  set_value(scope, obj, "newAsyncId", new_async_id);

  let constants = create_enum_object(scope, CONSTANTS);
  set_value(scope, obj, "constants", constants.into());
  let uid_fields = create_enum_object(scope, UID_FIELDS);
  set_value(scope, obj, "UidFields", uid_fields.into());
  let provider_type = create_enum_object(scope, PROVIDER_TYPES);
  set_value(scope, obj, "providerType", provider_type.into());
  obj
}
