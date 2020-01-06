// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

#![allow(mutable_transmutes)]
#![allow(clippy::transmute_ptr_to_ptr)]

use crate::isolate::Isolate;
use crate::libdeno::deno_buf;
use crate::libdeno::script_origin;
use crate::libdeno::PinnedBuf;

use rusty_v8 as v8;
use v8::InIsolate;

use libc::c_char;
use libc::c_void;
use std::convert::TryFrom;
use std::ffi::CString;
use std::option::Option;

pub extern "C" fn host_import_module_dynamically_callback(
  context: v8::Local<v8::Context>,
  referrer: v8::Local<v8::ScriptOrModule>,
  specifier: v8::Local<v8::String>,
) -> *mut v8::Promise {
  let mut cbs = v8::CallbackScope::new(context);
  let mut hs = v8::EscapableHandleScope::new(cbs.enter());
  let scope = hs.enter();
  let isolate = scope.isolate();
  let deno_isolate: &mut Isolate =
    unsafe { &mut *(isolate.get_data(0) as *mut Isolate) };

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

  let import_id = deno_isolate.next_dyn_import_id_;
  deno_isolate.next_dyn_import_id_ += 1;
  deno_isolate
    .dyn_import_map_
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
  let deno_isolate: &mut Isolate =
    unsafe { &mut *(isolate.get_data(0) as *mut Isolate) };

  let id = module.get_identity_hash();
  assert_ne!(id, 0);

  let info = deno_isolate.get_module_info(id).expect("Module not found");

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
  let mut message: v8::Local<v8::Message> =
    unsafe { std::mem::transmute(message) };
  let isolate = message.get_isolate();
  let deno_isolate: &mut Isolate =
    unsafe { &mut *(isolate.get_data(0) as *mut Isolate) };
  let mut locker = v8::Locker::new(isolate);
  let mut hs = v8::HandleScope::new(&mut locker);
  let scope = hs.enter();
  assert!(!deno_isolate.context_.is_empty());
  let context = deno_isolate.context_.get(scope).unwrap();

  // TerminateExecution was called
  if isolate.is_execution_terminating() {
    let u = v8::new_undefined(scope);
    deno_isolate.handle_exception(scope, context, u.into());
    return;
  }

  let json_str = deno_isolate.encode_message_as_json(scope, context, message);
  deno_isolate.last_exception_ = Some(json_str);
}

pub extern "C" fn promise_reject_callback(msg: v8::PromiseRejectMessage) {
  #[allow(mutable_transmutes)]
  let mut msg: v8::PromiseRejectMessage = unsafe { std::mem::transmute(msg) };
  let isolate = msg.isolate();
  let deno_isolate: &mut Isolate =
    unsafe { &mut *(isolate.get_data(0) as *mut Isolate) };
  let mut locker = v8::Locker::new(isolate);
  assert!(!deno_isolate.context_.is_empty());
  let mut hs = v8::HandleScope::new(&mut locker);
  let scope = hs.enter();
  let mut context = deno_isolate.context_.get(scope).unwrap();
  context.enter();

  let promise = msg.get_promise();
  let promise_id = promise.get_identity_hash();

  match msg.get_event() {
    v8::PromiseRejectEvent::PromiseRejectWithNoHandler => {
      let error = msg.get_value();
      let mut error_global = v8::Global::<v8::Value>::new();
      error_global.set(scope, error);
      deno_isolate
        .pending_promise_map_
        .insert(promise_id, error_global);
    }
    v8::PromiseRejectEvent::PromiseHandlerAddedAfterReject => {
      if let Some(mut handle) =
        deno_isolate.pending_promise_map_.remove(&promise_id)
      {
        handle.reset(scope);
      }
    }
    v8::PromiseRejectEvent::PromiseRejectAfterResolved => {}
    v8::PromiseRejectEvent::PromiseResolveAfterResolved => {
      // Should not warn. See #1272
    }
  };

  context.exit();
}

pub extern "C" fn print(info: &v8::FunctionCallbackInfo) {
  let info: &mut v8::FunctionCallbackInfo =
    unsafe { std::mem::transmute(info) };

  let arg_len = info.length();
  assert!(arg_len >= 0 && arg_len <= 2);

  let obj = info.get_argument(0);
  let is_err_arg = info.get_argument(1);

  let mut hs = v8::HandleScope::new(info);
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

pub extern "C" fn recv(info: &v8::FunctionCallbackInfo) {
  #[allow(mutable_transmutes)]
  #[allow(clippy::transmute_ptr_to_ptr)]
  let info: &mut v8::FunctionCallbackInfo =
    unsafe { std::mem::transmute(info) };
  assert_eq!(info.length(), 1);
  let isolate = info.get_isolate();
  let deno_isolate: &mut Isolate =
    unsafe { &mut *(isolate.get_data(0) as *mut Isolate) };
  let mut locker = v8::Locker::new(&isolate);
  let mut hs = v8::HandleScope::new(&mut locker);
  let scope = hs.enter();

  if !deno_isolate.recv_.is_empty() {
    let msg = v8::String::new(scope, "Deno.core.recv already called.").unwrap();
    isolate.throw_exception(msg.into());
    return;
  }

  let recv_fn =
    v8::Local::<v8::Function>::try_from(info.get_argument(0)).unwrap();
  deno_isolate.recv_.set(scope, recv_fn);
}

pub extern "C" fn send(info: &v8::FunctionCallbackInfo) {
  #[allow(mutable_transmutes)]
  #[allow(clippy::transmute_ptr_to_ptr)]
  let info: &mut v8::FunctionCallbackInfo =
    unsafe { std::mem::transmute(info) };

  let mut hs = v8::HandleScope::new(info);
  let scope = hs.enter();
  let isolate = scope.isolate();
  let deno_isolate: &mut Isolate =
    unsafe { &mut *(isolate.get_data(0) as *mut Isolate) };
  assert!(!deno_isolate.context_.is_empty());

  let op_id = v8::Local::<v8::Uint32>::try_from(info.get_argument(0))
    .unwrap()
    .value() as u32;

  let control =
    v8::Local::<v8::ArrayBufferView>::try_from(info.get_argument(1))
      .map(|view| {
        let mut backing_store = view.buffer().unwrap().get_backing_store();
        let backing_store_ptr = backing_store.data() as *mut _ as *mut u8;
        let view_ptr = unsafe { backing_store_ptr.add(view.byte_offset()) };
        let view_len = view.byte_length();
        unsafe { deno_buf::from_raw_parts(view_ptr, view_len) }
      })
      .unwrap_or_else(|_| deno_buf::empty());

  let zero_copy: Option<PinnedBuf> =
    v8::Local::<v8::ArrayBufferView>::try_from(info.get_argument(2))
      .map(PinnedBuf::new)
      .ok();

  // TODO: what's the point of this again?
  // DCHECK_NULL(d->current_args_);
  // d->current_args_ = &args;
  assert!(deno_isolate.current_args_.is_null());
  deno_isolate.current_args_ = info;

  deno_isolate.pre_dispatch(op_id, control, zero_copy);

  if deno_isolate.current_args_.is_null() {
    // This indicates that deno_repond() was called already.
  } else {
    // Asynchronous.
    deno_isolate.current_args_ = std::ptr::null();
  }
}

pub extern "C" fn eval_context(info: &v8::FunctionCallbackInfo) {
  let rv = &mut info.get_return_value();

  #[allow(mutable_transmutes)]
  #[allow(clippy::transmute_ptr_to_ptr)]
  let info: &mut v8::FunctionCallbackInfo =
    unsafe { std::mem::transmute(info) };
  let arg0 = info.get_argument(0);

  let mut hs = v8::HandleScope::new(info);
  let scope = hs.enter();
  let isolate = scope.isolate();
  let deno_isolate: &mut Isolate =
    unsafe { &mut *(isolate.get_data(0) as *mut Isolate) };
  assert!(!deno_isolate.context_.is_empty());
  let context = deno_isolate.context_.get(scope).unwrap();

  let source = match v8::Local::<v8::String>::try_from(arg0) {
    Ok(s) => s,
    Err(_) => {
      let msg = v8::String::new(scope, "Invalid argument").unwrap();
      let exception = v8::type_error(scope, msg);
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
      v8::new_null(scope).into(),
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
      v8::new_null(scope).into(),
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
    v8::new_null(scope).into(),
  );
  rv.set(output.into());
}

pub extern "C" fn error_to_json(info: &v8::FunctionCallbackInfo) {
  #[allow(mutable_transmutes)]
  #[allow(clippy::transmute_ptr_to_ptr)]
  let info: &mut v8::FunctionCallbackInfo =
    unsafe { std::mem::transmute(info) };
  assert_eq!(info.length(), 1);
  // <Boilerplate>
  let isolate = info.get_isolate();
  let deno_isolate: &mut Isolate =
    unsafe { &mut *(isolate.get_data(0) as *mut Isolate) };
  let mut locker = v8::Locker::new(&isolate);
  assert!(!deno_isolate.context_.is_empty());
  let mut hs = v8::HandleScope::new(&mut locker);
  let scope = hs.enter();
  let context = deno_isolate.context_.get(scope).unwrap();
  // </Boilerplate>
  let exception = info.get_argument(0);
  let json_string =
    deno_isolate.encode_exception_as_json(scope, context, exception);
  let s = v8::String::new(scope, &json_string).unwrap();
  let mut rv = info.get_return_value();
  rv.set(s.into());
}

pub extern "C" fn queue_microtask(info: &v8::FunctionCallbackInfo) {
  #[allow(mutable_transmutes)]
  #[allow(clippy::transmute_ptr_to_ptr)]
  let info: &mut v8::FunctionCallbackInfo =
    unsafe { std::mem::transmute(info) };
  assert_eq!(info.length(), 1);
  let arg0 = info.get_argument(0);
  let isolate = info.get_isolate();
  let mut locker = v8::Locker::new(&isolate);
  let mut hs = v8::HandleScope::new(&mut locker);
  let scope = hs.enter();

  match v8::Local::<v8::Function>::try_from(arg0) {
    Ok(f) => isolate.enqueue_microtask(f),
    Err(_) => {
      let msg = v8::String::new(scope, "Invalid argument").unwrap();
      let exception = v8::type_error(scope, msg);
      isolate.throw_exception(exception);
    }
  };
}

pub extern "C" fn shared_getter(
  _name: v8::Local<v8::Name>,
  info: &v8::PropertyCallbackInfo,
) {
  let shared_ab = {
    #[allow(mutable_transmutes)]
    #[allow(clippy::transmute_ptr_to_ptr)]
    let info: &mut v8::PropertyCallbackInfo =
      unsafe { std::mem::transmute(info) };

    let mut hs = v8::EscapableHandleScope::new(info);
    let scope = hs.enter();
    let isolate = scope.isolate();
    let deno_isolate: &mut Isolate =
      unsafe { &mut *(isolate.get_data(0) as *mut Isolate) };

    if deno_isolate.shared_.data_ptr.is_null() {
      return;
    }

    // Lazily initialize the persistent external ArrayBuffer.
    if deno_isolate.shared_ab_.is_empty() {
      #[allow(mutable_transmutes)]
      #[allow(clippy::transmute_ptr_to_ptr)]
      let data_ptr: *mut u8 =
        unsafe { std::mem::transmute(deno_isolate.shared_.data_ptr) };
      let ab = unsafe {
        v8::SharedArrayBuffer::new_DEPRECATED(
          scope,
          data_ptr as *mut c_void,
          deno_isolate.shared_.data_len,
        )
      };
      deno_isolate.shared_ab_.set(scope, ab);
    }

    let shared_ab = deno_isolate.shared_ab_.get(scope).unwrap();
    scope.escape(shared_ab)
  };

  let rv = &mut info.get_return_value();
  rv.set(shared_ab.into());
}

pub fn module_resolve_callback(
  context: v8::Local<v8::Context>,
  specifier: v8::Local<v8::String>,
  referrer: v8::Local<v8::Module>,
) -> *mut v8::Module {
  let mut cbs = v8::CallbackScope::new(context);
  let cb_scope = cbs.enter();
  let isolate = cb_scope.isolate();
  let deno_isolate: &mut Isolate =
    unsafe { &mut *(isolate.get_data(0) as *mut Isolate) };

  let mut locker = v8::Locker::new(isolate);
  let mut hs = v8::EscapableHandleScope::new(&mut locker);
  let scope = hs.enter();

  let referrer_id = referrer.get_identity_hash();
  let referrer_info = deno_isolate
    .get_module_info(referrer_id)
    .expect("ModuleInfo not found");
  let len_ = referrer.get_module_requests_length();

  let specifier_str = specifier.to_rust_string_lossy(scope);

  for i in 0..len_ {
    let req = referrer.get_module_request(i);
    let req_str = req.to_rust_string_lossy(scope);

    if req_str == specifier_str {
      let resolve_cb = deno_isolate.resolve_cb_.unwrap();
      let c_str = CString::new(req_str.to_string()).unwrap();
      let c_req_str: *const c_char = c_str.as_ptr() as *const c_char;
      let id = unsafe {
        resolve_cb(deno_isolate.resolve_context_, c_req_str, referrer_id)
      };
      let maybe_info = deno_isolate.get_module_info(id);

      if maybe_info.is_none() {
        let msg = format!(
          "Cannot resolve module \"{}\" from \"{}\"",
          req_str, referrer_info.name
        );
        let msg = v8::String::new(scope, &msg).unwrap();
        isolate.throw_exception(msg.into());
        break;
      }

      let child_mod =
        maybe_info.unwrap().handle.get(scope).expect("Empty handle");
      return &mut *scope.escape(child_mod);
    }
  }

  std::ptr::null_mut()
}
