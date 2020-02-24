// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use crate::es_isolate::EsIsolate;
use crate::isolate::encode_message_as_json;
use crate::isolate::handle_exception;
use crate::isolate::Isolate;
use crate::isolate::ZeroCopyBuf;

use rusty_v8 as v8;
use v8::MapFnTo;

use std::convert::TryFrom;
use std::option::Option;

lazy_static! {
  pub static ref EXTERNAL_REFERENCES: v8::ExternalReferences =
    v8::ExternalReferences::new(&[
      v8::ExternalReference {
        function: print.map_fn_to()
      },
      v8::ExternalReference {
        function: recv.map_fn_to()
      },
      v8::ExternalReference {
        function: send.map_fn_to()
      },
      v8::ExternalReference {
        function: eval_context.map_fn_to()
      },
      v8::ExternalReference {
        function: error_to_json.map_fn_to()
      },
      v8::ExternalReference {
        getter: shared_getter.map_fn_to()
      },
      v8::ExternalReference {
        message: message_callback
      },
      v8::ExternalReference {
        function: queue_microtask.map_fn_to()
      },
    ]);
}

pub fn script_origin<'a>(
  s: &mut impl v8::ToLocal<'a>,
  resource_name: v8::Local<'a, v8::String>,
) -> v8::ScriptOrigin<'a> {
  let resource_line_offset = v8::Integer::new(s, 0);
  let resource_column_offset = v8::Integer::new(s, 0);
  let resource_is_shared_cross_origin = v8::Boolean::new(s, false);
  let script_id = v8::Integer::new(s, 123);
  let source_map_url = v8::String::new(s, "source_map_url").unwrap();
  let resource_is_opaque = v8::Boolean::new(s, true);
  let is_wasm = v8::Boolean::new(s, false);
  let is_module = v8::Boolean::new(s, false);
  v8::ScriptOrigin::new(
    resource_name.into(),
    resource_line_offset,
    resource_column_offset,
    resource_is_shared_cross_origin,
    script_id,
    source_map_url.into(),
    resource_is_opaque,
    is_wasm,
    is_module,
  )
}

pub fn module_origin<'a>(
  s: &mut impl v8::ToLocal<'a>,
  resource_name: v8::Local<'a, v8::String>,
) -> v8::ScriptOrigin<'a> {
  let resource_line_offset = v8::Integer::new(s, 0);
  let resource_column_offset = v8::Integer::new(s, 0);
  let resource_is_shared_cross_origin = v8::Boolean::new(s, false);
  let script_id = v8::Integer::new(s, 123);
  let source_map_url = v8::String::new(s, "source_map_url").unwrap();
  let resource_is_opaque = v8::Boolean::new(s, true);
  let is_wasm = v8::Boolean::new(s, false);
  let is_module = v8::Boolean::new(s, true);
  v8::ScriptOrigin::new(
    resource_name.into(),
    resource_line_offset,
    resource_column_offset,
    resource_is_shared_cross_origin,
    script_id,
    source_map_url.into(),
    resource_is_opaque,
    is_wasm,
    is_module,
  )
}

pub fn initialize_context<'s>(
  scope: &mut impl v8::ToLocal<'s>,
) -> v8::Local<'s, v8::Context> {
  let mut hs = v8::EscapableHandleScope::new(scope);
  let scope = hs.enter();

  let context = v8::Context::new(scope);
  let global = context.global(scope);

  let mut cs = v8::ContextScope::new(scope, context);
  let scope = cs.enter();

  let deno_val = v8::Object::new(scope);
  global.set(
    context,
    v8::String::new(scope, "Deno").unwrap().into(),
    deno_val.into(),
  );

  let mut core_val = v8::Object::new(scope);
  deno_val.set(
    context,
    v8::String::new(scope, "core").unwrap().into(),
    core_val.into(),
  );

  let mut print_tmpl = v8::FunctionTemplate::new(scope, print);
  let print_val = print_tmpl.get_function(scope, context).unwrap();
  core_val.set(
    context,
    v8::String::new(scope, "print").unwrap().into(),
    print_val.into(),
  );

  let mut recv_tmpl = v8::FunctionTemplate::new(scope, recv);
  let recv_val = recv_tmpl.get_function(scope, context).unwrap();
  core_val.set(
    context,
    v8::String::new(scope, "recv").unwrap().into(),
    recv_val.into(),
  );

  let mut send_tmpl = v8::FunctionTemplate::new(scope, send);
  let send_val = send_tmpl.get_function(scope, context).unwrap();
  core_val.set(
    context,
    v8::String::new(scope, "send").unwrap().into(),
    send_val.into(),
  );

  let mut eval_context_tmpl = v8::FunctionTemplate::new(scope, eval_context);
  let eval_context_val =
    eval_context_tmpl.get_function(scope, context).unwrap();
  core_val.set(
    context,
    v8::String::new(scope, "evalContext").unwrap().into(),
    eval_context_val.into(),
  );

  let mut error_to_json_tmpl = v8::FunctionTemplate::new(scope, error_to_json);
  let error_to_json_val =
    error_to_json_tmpl.get_function(scope, context).unwrap();
  core_val.set(
    context,
    v8::String::new(scope, "errorToJSON").unwrap().into(),
    error_to_json_val.into(),
  );

  core_val.set_accessor(
    context,
    v8::String::new(scope, "shared").unwrap().into(),
    shared_getter,
  );

  // Direct bindings on `window`.
  let mut queue_microtask_tmpl =
    v8::FunctionTemplate::new(scope, queue_microtask);
  let queue_microtask_val =
    queue_microtask_tmpl.get_function(scope, context).unwrap();
  global.set(
    context,
    v8::String::new(scope, "queueMicrotask").unwrap().into(),
    queue_microtask_val.into(),
  );

  scope.escape(context)
}

pub fn boxed_slice_to_uint8array<'sc>(
  scope: &mut impl v8::ToLocal<'sc>,
  buf: Box<[u8]>,
) -> v8::Local<'sc, v8::Uint8Array> {
  assert!(!buf.is_empty());
  let buf_len = buf.len();
  let backing_store = v8::ArrayBuffer::new_backing_store_from_boxed_slice(buf);
  let mut backing_store_shared = backing_store.make_shared();
  let ab =
    v8::ArrayBuffer::with_backing_store(scope, &mut backing_store_shared);
  v8::Uint8Array::new(ab, 0, buf_len).expect("Failed to create UintArray8")
}

pub extern "C" fn host_import_module_dynamically_callback(
  context: v8::Local<v8::Context>,
  referrer: v8::Local<v8::ScriptOrModule>,
  specifier: v8::Local<v8::String>,
) -> *mut v8::Promise {
  let mut cbs = v8::CallbackScope::new_escapable(context);
  let mut hs = v8::EscapableHandleScope::new(cbs.enter());
  let scope = hs.enter();
  let isolate = scope.isolate();
  let deno_isolate: &mut EsIsolate =
    unsafe { &mut *(isolate.get_data(1) as *mut EsIsolate) };

  // NOTE(bartlomieju): will crash for non-UTF-8 specifier
  let specifier_str = specifier
    .to_string(scope)
    .unwrap()
    .to_rust_string_lossy(scope);
  let referrer_name = referrer.get_resource_name();
  let referrer_name_str = referrer_name
    .to_string(scope)
    .unwrap()
    .to_rust_string_lossy(scope);

  // TODO(ry) I'm not sure what HostDefinedOptions is for or if we're ever going
  // to use it. For now we check that it is not used. This check may need to be
  // changed in the future.
  let host_defined_options = referrer.get_host_defined_options();
  assert_eq!(host_defined_options.length(), 0);

  let mut resolver = v8::PromiseResolver::new(scope, context).unwrap();
  let promise = resolver.get_promise(scope);

  let mut resolver_handle = v8::Global::new();
  resolver_handle.set(scope, resolver);

  let import_id = deno_isolate.next_dyn_import_id;
  deno_isolate.next_dyn_import_id += 1;
  deno_isolate
    .dyn_import_map
    .insert(import_id, resolver_handle);

  deno_isolate.dyn_import_cb(&specifier_str, &referrer_name_str, import_id);

  &mut *scope.escape(promise)
}

pub extern "C" fn host_initialize_import_meta_object_callback(
  context: v8::Local<v8::Context>,
  module: v8::Local<v8::Module>,
  meta: v8::Local<v8::Object>,
) {
  let mut cbs = v8::CallbackScope::new(context);
  let mut hs = v8::HandleScope::new(cbs.enter());
  let scope = hs.enter();
  let isolate = scope.isolate();
  let deno_isolate: &mut EsIsolate =
    unsafe { &mut *(isolate.get_data(1) as *mut EsIsolate) };

  let id = module.get_identity_hash();
  assert_ne!(id, 0);

  let info = deno_isolate.modules.get_info(id).expect("Module not found");

  meta.create_data_property(
    context,
    v8::String::new(scope, "url").unwrap().into(),
    v8::String::new(scope, &info.name).unwrap().into(),
  );
  meta.create_data_property(
    context,
    v8::String::new(scope, "main").unwrap().into(),
    v8::Boolean::new(scope, info.main).into(),
  );
}

pub extern "C" fn message_callback(
  message: v8::Local<v8::Message>,
  _exception: v8::Local<v8::Value>,
) {
  let mut cbs = v8::CallbackScope::new(message);
  let mut hs = v8::HandleScope::new(cbs.enter());
  let scope = hs.enter();

  let deno_isolate: &mut Isolate =
    unsafe { &mut *(scope.isolate().get_data(0) as *mut Isolate) };

  // TerminateExecution was called
  // TODO(piscisaureus): rusty_v8 should implement the
  // `is_execution_terminating()` method on struct `Isolate` also.
  if scope
    .isolate()
    .thread_safe_handle()
    .is_execution_terminating()
  {
    let undefined = v8::undefined(scope).into();
    handle_exception(scope, undefined, &mut deno_isolate.last_exception);
    return;
  }

  let json_str = encode_message_as_json(scope, message);
  deno_isolate.last_exception = Some(json_str);
}

pub extern "C" fn promise_reject_callback(message: v8::PromiseRejectMessage) {
  let mut cbs = v8::CallbackScope::new(&message);
  let mut hs = v8::HandleScope::new(cbs.enter());
  let scope = hs.enter();

  let deno_isolate: &mut Isolate =
    unsafe { &mut *(scope.isolate().get_data(0) as *mut Isolate) };

  let context = deno_isolate.global_context.get(scope).unwrap();
  let mut cs = v8::ContextScope::new(scope, context);
  let scope = cs.enter();

  let promise = message.get_promise();
  let promise_id = promise.get_identity_hash();

  match message.get_event() {
    v8::PromiseRejectEvent::PromiseRejectWithNoHandler => {
      let error = message.get_value();
      let mut error_global = v8::Global::<v8::Value>::new();
      error_global.set(scope, error);
      deno_isolate
        .pending_promise_exceptions
        .insert(promise_id, error_global);
    }
    v8::PromiseRejectEvent::PromiseHandlerAddedAfterReject => {
      if let Some(mut handle) =
        deno_isolate.pending_promise_exceptions.remove(&promise_id)
      {
        handle.reset(scope);
      }
    }
    v8::PromiseRejectEvent::PromiseRejectAfterResolved => {}
    v8::PromiseRejectEvent::PromiseResolveAfterResolved => {
      // Should not warn. See #1272
    }
  };
}

fn print(
  scope: v8::FunctionCallbackScope,
  args: v8::FunctionCallbackArguments,
  _rv: v8::ReturnValue,
) {
  let arg_len = args.length();
  assert!(arg_len >= 0 && arg_len <= 2);

  let obj = args.get(0);
  let is_err_arg = args.get(1);

  let mut hs = v8::HandleScope::new(scope);
  let scope = hs.enter();

  let mut is_err = false;
  if arg_len == 2 {
    let int_val = is_err_arg
      .integer_value(scope)
      .expect("Unable to convert to integer");
    is_err = int_val != 0;
  };
  let mut try_catch = v8::TryCatch::new(scope);
  let _tc = try_catch.enter();
  let str_ = match obj.to_string(scope) {
    Some(s) => s,
    None => v8::String::new(scope, "").unwrap(),
  };
  if is_err {
    eprint!("{}", str_.to_rust_string_lossy(scope));
  } else {
    print!("{}", str_.to_rust_string_lossy(scope));
  }
}

fn recv(
  scope: v8::FunctionCallbackScope,
  args: v8::FunctionCallbackArguments,
  _rv: v8::ReturnValue,
) {
  let deno_isolate: &mut Isolate =
    unsafe { &mut *(scope.isolate().get_data(0) as *mut Isolate) };

  if !deno_isolate.js_recv_cb.is_empty() {
    let msg = v8::String::new(scope, "Deno.core.recv already called.").unwrap();
    scope.isolate().throw_exception(msg.into());
    return;
  }

  let recv_fn = v8::Local::<v8::Function>::try_from(args.get(0)).unwrap();
  deno_isolate.js_recv_cb.set(scope, recv_fn);
}

fn send(
  scope: v8::FunctionCallbackScope,
  args: v8::FunctionCallbackArguments,
  mut rv: v8::ReturnValue,
) {
  let deno_isolate: &mut Isolate =
    unsafe { &mut *(scope.isolate().get_data(0) as *mut Isolate) };
  assert!(!deno_isolate.global_context.is_empty());

  let r = v8::Local::<v8::Uint32>::try_from(args.get(0));

  if let Err(err) = r {
    let s = format!("bad op id {}", err);
    let msg = v8::String::new(scope, &s).unwrap();
    scope.isolate().throw_exception(msg.into());
    return;
  }

  let op_id = r.unwrap().value() as u32;

  let control = match v8::Local::<v8::ArrayBufferView>::try_from(args.get(1)) {
    Ok(view) => {
      let byte_offset = view.byte_offset();
      let byte_length = view.byte_length();
      let backing_store = view.buffer().unwrap().get_backing_store();
      let buf = unsafe { &**backing_store.get() };
      &buf[byte_offset..byte_offset + byte_length]
    }
    Err(..) => &[],
  };

  let zero_copy: Option<ZeroCopyBuf> =
    v8::Local::<v8::ArrayBufferView>::try_from(args.get(2))
      .map(ZeroCopyBuf::new)
      .ok();

  // If response is empty then it's either async op or exception was thrown
  let maybe_response =
    deno_isolate.dispatch_op(scope, op_id, control, zero_copy);

  if let Some(response) = maybe_response {
    // Synchronous response.
    // Note op_id is not passed back in the case of synchronous response.
    let (_op_id, buf) = response;

    if !buf.is_empty() {
      let ui8 = boxed_slice_to_uint8array(scope, buf);
      rv.set(ui8.into())
    }
  }
}

fn eval_context(
  scope: v8::FunctionCallbackScope,
  args: v8::FunctionCallbackArguments,
  mut rv: v8::ReturnValue,
) {
  let deno_isolate: &mut Isolate =
    unsafe { &mut *(scope.isolate().get_data(0) as *mut Isolate) };
  assert!(!deno_isolate.global_context.is_empty());
  let context = deno_isolate.global_context.get(scope).unwrap();

  let source = match v8::Local::<v8::String>::try_from(args.get(0)) {
    Ok(s) => s,
    Err(_) => {
      let msg = v8::String::new(scope, "Invalid argument").unwrap();
      let exception = v8::Exception::type_error(scope, msg);
      scope.isolate().throw_exception(exception);
      return;
    }
  };

  let output = v8::Array::new(scope, 2);
  /*
   output[0] = result
   output[1] = ErrorInfo | null
     ErrorInfo = {
       thrown: Error | any,
       isNativeError: boolean,
       isCompileError: boolean,
     }
  */
  let mut try_catch = v8::TryCatch::new(scope);
  let tc = try_catch.enter();
  let name = v8::String::new(scope, "<unknown>").unwrap();
  let origin = script_origin(scope, name);
  let maybe_script = v8::Script::compile(scope, context, source, Some(&origin));

  if maybe_script.is_none() {
    assert!(tc.has_caught());
    let exception = tc.exception().unwrap();

    output.set(
      context,
      v8::Integer::new(scope, 0).into(),
      v8::null(scope).into(),
    );

    let errinfo_obj = v8::Object::new(scope);
    errinfo_obj.set(
      context,
      v8::String::new(scope, "isCompileError").unwrap().into(),
      v8::Boolean::new(scope, true).into(),
    );

    errinfo_obj.set(
      context,
      v8::String::new(scope, "isNativeError").unwrap().into(),
      v8::Boolean::new(scope, exception.is_native_error()).into(),
    );

    errinfo_obj.set(
      context,
      v8::String::new(scope, "thrown").unwrap().into(),
      exception,
    );

    output.set(
      context,
      v8::Integer::new(scope, 1).into(),
      errinfo_obj.into(),
    );

    rv.set(output.into());
    return;
  }

  let result = maybe_script.unwrap().run(scope, context);

  if result.is_none() {
    assert!(tc.has_caught());
    let exception = tc.exception().unwrap();

    output.set(
      context,
      v8::Integer::new(scope, 0).into(),
      v8::null(scope).into(),
    );

    let errinfo_obj = v8::Object::new(scope);
    errinfo_obj.set(
      context,
      v8::String::new(scope, "isCompileError").unwrap().into(),
      v8::Boolean::new(scope, false).into(),
    );

    let is_native_error = if exception.is_native_error() {
      v8::Boolean::new(scope, true)
    } else {
      v8::Boolean::new(scope, false)
    };

    errinfo_obj.set(
      context,
      v8::String::new(scope, "isNativeError").unwrap().into(),
      is_native_error.into(),
    );

    errinfo_obj.set(
      context,
      v8::String::new(scope, "thrown").unwrap().into(),
      exception,
    );

    output.set(
      context,
      v8::Integer::new(scope, 1).into(),
      errinfo_obj.into(),
    );

    rv.set(output.into());
    return;
  }

  output.set(context, v8::Integer::new(scope, 0).into(), result.unwrap());
  output.set(
    context,
    v8::Integer::new(scope, 1).into(),
    v8::null(scope).into(),
  );
  rv.set(output.into());
}

fn error_to_json(
  scope: v8::FunctionCallbackScope,
  args: v8::FunctionCallbackArguments,
  mut rv: v8::ReturnValue,
) {
  let deno_isolate: &mut Isolate =
    unsafe { &mut *(scope.isolate().get_data(0) as *mut Isolate) };
  let context = deno_isolate.global_context.get(scope).unwrap();

  let message = v8::Exception::create_message(scope, args.get(0));
  let json_obj = encode_message_as_object(scope, message);
  let json_string = v8::json::stringify(context, json_obj.into()).unwrap();

  rv.set(json_string.into());
}

fn queue_microtask(
  scope: v8::FunctionCallbackScope,
  args: v8::FunctionCallbackArguments,
  _rv: v8::ReturnValue,
) {
  match v8::Local::<v8::Function>::try_from(args.get(0)) {
    Ok(f) => scope.isolate().enqueue_microtask(f),
    Err(_) => {
      let msg = v8::String::new(scope, "Invalid argument").unwrap();
      let exception = v8::Exception::type_error(scope, msg);
      scope.isolate().throw_exception(exception);
    }
  };
}

fn shared_getter(
  scope: v8::PropertyCallbackScope,
  _name: v8::Local<v8::Name>,
  _args: v8::PropertyCallbackArguments,
  mut rv: v8::ReturnValue,
) {
  let deno_isolate: &mut Isolate =
    unsafe { &mut *(scope.isolate().get_data(0) as *mut Isolate) };

  // Lazily initialize the persistent external ArrayBuffer.
  if deno_isolate.shared_ab.is_empty() {
    let ab = v8::SharedArrayBuffer::with_backing_store(
      scope,
      deno_isolate.shared.get_backing_store(),
    );
    deno_isolate.shared_ab.set(scope, ab);
  }

  let shared_ab = deno_isolate.shared_ab.get(scope).unwrap();
  rv.set(shared_ab.into());
}

pub fn module_resolve_callback<'s>(
  context: v8::Local<'s, v8::Context>,
  specifier: v8::Local<'s, v8::String>,
  referrer: v8::Local<'s, v8::Module>,
) -> Option<v8::Local<'s, v8::Module>> {
  let mut scope = v8::CallbackScope::new_escapable(context);
  let mut scope = v8::EscapableHandleScope::new(scope.enter());
  let scope = scope.enter();

  let deno_isolate: &mut EsIsolate =
    unsafe { &mut *(scope.isolate().get_data(1) as *mut EsIsolate) };

  let referrer_id = referrer.get_identity_hash();
  let referrer_name = deno_isolate
    .modules
    .get_info(referrer_id)
    .expect("ModuleInfo not found")
    .name
    .to_string();
  let len_ = referrer.get_module_requests_length();

  let specifier_str = specifier.to_rust_string_lossy(scope);

  for i in 0..len_ {
    let req = referrer.get_module_request(i);
    let req_str = req.to_rust_string_lossy(scope);

    if req_str == specifier_str {
      let id = deno_isolate.module_resolve_cb(&req_str, referrer_id);
      let maybe_info = deno_isolate.modules.get_info(id);

      if maybe_info.is_none() {
        let msg = format!(
          "Cannot resolve module \"{}\" from \"{}\"",
          req_str, referrer_name
        );
        let msg = v8::String::new(scope, &msg).unwrap();
        scope.isolate().throw_exception(msg.into());
        break;
      }

      return maybe_info
        .and_then(|i| i.handle.get(scope))
        .map(|m| scope.escape(m));
    }
  }

  None
}

pub fn encode_message_as_object<'a>(
  s: &mut impl v8::ToLocal<'a>,
  message: v8::Local<v8::Message>,
) -> v8::Local<'a, v8::Object> {
  let context = s.get_current_context().unwrap();
  let json_obj = v8::Object::new(s);

  let exception_str = message.get(s);
  json_obj.set(
    context,
    v8::String::new(s, "message").unwrap().into(),
    exception_str.into(),
  );

  let script_resource_name = message
    .get_script_resource_name(s)
    .expect("Missing ScriptResourceName");
  json_obj.set(
    context,
    v8::String::new(s, "scriptResourceName").unwrap().into(),
    script_resource_name,
  );

  let source_line = message
    .get_source_line(s, context)
    .expect("Missing SourceLine");
  json_obj.set(
    context,
    v8::String::new(s, "sourceLine").unwrap().into(),
    source_line.into(),
  );

  let line_number = message
    .get_line_number(context)
    .expect("Missing LineNumber");
  json_obj.set(
    context,
    v8::String::new(s, "lineNumber").unwrap().into(),
    v8::Integer::new(s, line_number as i32).into(),
  );

  json_obj.set(
    context,
    v8::String::new(s, "startPosition").unwrap().into(),
    v8::Integer::new(s, message.get_start_position() as i32).into(),
  );

  json_obj.set(
    context,
    v8::String::new(s, "endPosition").unwrap().into(),
    v8::Integer::new(s, message.get_end_position() as i32).into(),
  );

  json_obj.set(
    context,
    v8::String::new(s, "errorLevel").unwrap().into(),
    v8::Integer::new(s, message.error_level() as i32).into(),
  );

  json_obj.set(
    context,
    v8::String::new(s, "startColumn").unwrap().into(),
    v8::Integer::new(s, message.get_start_column() as i32).into(),
  );

  json_obj.set(
    context,
    v8::String::new(s, "endColumn").unwrap().into(),
    v8::Integer::new(s, message.get_end_column() as i32).into(),
  );

  let is_shared_cross_origin =
    v8::Boolean::new(s, message.is_shared_cross_origin());

  json_obj.set(
    context,
    v8::String::new(s, "isSharedCrossOrigin").unwrap().into(),
    is_shared_cross_origin.into(),
  );

  let is_opaque = v8::Boolean::new(s, message.is_opaque());

  json_obj.set(
    context,
    v8::String::new(s, "isOpaque").unwrap().into(),
    is_opaque.into(),
  );

  let frames = if let Some(stack_trace) = message.get_stack_trace(s) {
    let count = stack_trace.get_frame_count() as i32;
    let frames = v8::Array::new(s, count);

    for i in 0..count {
      let frame = stack_trace
        .get_frame(s, i as usize)
        .expect("No frame found");
      let frame_obj = v8::Object::new(s);
      frames.set(context, v8::Integer::new(s, i).into(), frame_obj.into());
      frame_obj.set(
        context,
        v8::String::new(s, "line").unwrap().into(),
        v8::Integer::new(s, frame.get_line_number() as i32).into(),
      );
      frame_obj.set(
        context,
        v8::String::new(s, "column").unwrap().into(),
        v8::Integer::new(s, frame.get_column() as i32).into(),
      );

      if let Some(function_name) = frame.get_function_name(s) {
        frame_obj.set(
          context,
          v8::String::new(s, "functionName").unwrap().into(),
          function_name.into(),
        );
      }

      let script_name = match frame.get_script_name_or_source_url(s) {
        Some(name) => name,
        None => v8::String::new(s, "<unknown>").unwrap(),
      };
      frame_obj.set(
        context,
        v8::String::new(s, "scriptName").unwrap().into(),
        script_name.into(),
      );

      frame_obj.set(
        context,
        v8::String::new(s, "isEval").unwrap().into(),
        v8::Boolean::new(s, frame.is_eval()).into(),
      );

      frame_obj.set(
        context,
        v8::String::new(s, "isConstructor").unwrap().into(),
        v8::Boolean::new(s, frame.is_constructor()).into(),
      );

      frame_obj.set(
        context,
        v8::String::new(s, "isWasm").unwrap().into(),
        v8::Boolean::new(s, frame.is_wasm()).into(),
      );
    }

    frames
  } else {
    // No stack trace. We only have one stack frame of info..
    let frames = v8::Array::new(s, 1);
    let frame_obj = v8::Object::new(s);
    frames.set(context, v8::Integer::new(s, 0).into(), frame_obj.into());

    frame_obj.set(
      context,
      v8::String::new(s, "scriptResourceName").unwrap().into(),
      script_resource_name,
    );
    frame_obj.set(
      context,
      v8::String::new(s, "line").unwrap().into(),
      v8::Integer::new(s, line_number as i32).into(),
    );
    frame_obj.set(
      context,
      v8::String::new(s, "column").unwrap().into(),
      v8::Integer::new(s, message.get_start_column() as i32).into(),
    );

    frames
  };

  json_obj.set(
    context,
    v8::String::new(s, "frames").unwrap().into(),
    frames.into(),
  );

  json_obj
}
