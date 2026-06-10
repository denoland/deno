// Copyright 2018-2026 the Deno authors. MIT license.

mod session;
mod stream;
mod types;

use deno_core::op2;
use deno_core::v8;
use deno_core::v8::ExternalReference;
use deno_core::v8::MapFnTo;

pub use session::Http2Session;
pub use session::op_http2_callbacks;
pub use session::op_http2_error_string;
pub use session::op_http2_http_state;
pub use stream::Http2Stream;

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

fn forward_to_receiver_method(
  scope: &mut v8::PinScope,
  args: v8::FunctionCallbackArguments,
  mut rv: v8::ReturnValue,
  name: &str,
) {
  let this = args.this();
  let key = v8::String::new(scope, name).unwrap();
  let Some(method) = this.get(scope, key.into()) else {
    return;
  };
  let Ok(method) = v8::Local::<v8::Function>::try_from(method) else {
    return;
  };
  let call_args = (0..args.length())
    .map(|index| args.get(index))
    .collect::<Vec<_>>();
  if let Some(result) = method.call(scope, this.into(), &call_args) {
    rv.set(result);
  }
}

fn http2_stream_constructor_callback(
  _scope: &mut v8::PinScope,
  _args: v8::FunctionCallbackArguments,
  _rv: v8::ReturnValue,
) {
}

fn http2_session_constructor_callback(
  _scope: &mut v8::PinScope,
  _args: v8::FunctionCallbackArguments,
  _rv: v8::ReturnValue,
) {
}

fn http2_stream_respond_callback(
  scope: &mut v8::PinScope,
  args: v8::FunctionCallbackArguments,
  rv: v8::ReturnValue,
) {
  forward_to_receiver_method(scope, args, rv, "respond");
}

fn http2_stream_push_promise_callback(
  scope: &mut v8::PinScope,
  args: v8::FunctionCallbackArguments,
  rv: v8::ReturnValue,
) {
  forward_to_receiver_method(scope, args, rv, "pushPromise");
}

fn http2_session_request_callback(
  scope: &mut v8::PinScope,
  args: v8::FunctionCallbackArguments,
  rv: v8::ReturnValue,
) {
  forward_to_receiver_method(scope, args, rv, "request");
}

fn create_constructor<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  name: &str,
  constructor: impl MapFnTo<v8::FunctionCallback>,
) -> v8::Local<'s, v8::Function> {
  let template = v8::FunctionTemplate::new(scope, constructor);
  let class_name = v8::String::new(scope, name).unwrap();
  template.set_class_name(class_name);
  template.get_function(scope).unwrap()
}

pub(crate) fn internal_binding_external_references() -> [ExternalReference; 5] {
  [
    ExternalReference {
      function: http2_stream_constructor_callback.map_fn_to(),
    },
    ExternalReference {
      function: http2_session_constructor_callback.map_fn_to(),
    },
    ExternalReference {
      function: http2_stream_respond_callback.map_fn_to(),
    },
    ExternalReference {
      function: http2_stream_push_promise_callback.map_fn_to(),
    },
    ExternalReference {
      function: http2_session_request_callback.map_fn_to(),
    },
  ]
}

#[op2]
pub fn op_node_internal_binding_http2<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  constants: v8::Local<'s, v8::Value>,
  nghttp2_error_string: v8::Local<'s, v8::Value>,
) -> v8::Local<'s, v8::Object> {
  let http2_stream =
    create_constructor(scope, "Http2Stream", http2_stream_constructor_callback);
  let prototype_key = v8::String::new(scope, "prototype").unwrap();
  let prototype = http2_stream
    .get(scope, prototype_key.into())
    .and_then(|value| v8::Local::<v8::Object>::try_from(value).ok())
    .unwrap();
  let respond = v8::FunctionTemplate::new(scope, http2_stream_respond_callback)
    .get_function(scope)
    .unwrap();
  set_function(scope, prototype, "respond", respond);
  let push_promise =
    v8::FunctionTemplate::new(scope, http2_stream_push_promise_callback)
      .get_function(scope)
      .unwrap();
  set_function(scope, prototype, "pushPromise", push_promise);

  let http2_session = create_constructor(
    scope,
    "Http2Session",
    http2_session_constructor_callback,
  );
  let prototype = http2_session
    .get(scope, prototype_key.into())
    .and_then(|value| v8::Local::<v8::Object>::try_from(value).ok())
    .unwrap();
  let request =
    v8::FunctionTemplate::new(scope, http2_session_request_callback)
      .get_function(scope)
      .unwrap();
  set_function(scope, prototype, "request", request);

  let default = v8::Object::new(scope);
  set_value(scope, default, "constants", constants);
  set_value(scope, default, "Http2Session", http2_session.into());
  set_value(scope, default, "Http2Stream", http2_stream.into());
  set_value(scope, default, "nghttp2ErrorString", nghttp2_error_string);

  let obj = v8::Object::new(scope);
  set_value(scope, obj, "constants", constants);
  set_value(scope, obj, "Http2Session", http2_session.into());
  set_value(scope, obj, "Http2Stream", http2_stream.into());
  set_value(scope, obj, "nghttp2ErrorString", nghttp2_error_string);
  set_value(scope, obj, "default", default.into());
  obj
}
