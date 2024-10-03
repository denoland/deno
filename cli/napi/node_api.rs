// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

#![deny(unsafe_op_in_unsafe_fn)]

use super::util::get_array_buffer_ptr;
use super::util::make_external_backing_store;
use super::util::napi_clear_last_error;
use super::util::napi_set_last_error;
use super::util::SendPtr;
use crate::check_arg;
use crate::check_env;
use deno_core::parking_lot::Condvar;
use deno_core::parking_lot::Mutex;
use deno_core::V8CrossThreadTaskSpawner;
use deno_runtime::deno_napi::*;
use napi_sym::napi_sym;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicU8;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::sync::Arc;

#[napi_sym]
fn napi_module_register(module: *const NapiModule) -> napi_status {
  MODULE_TO_REGISTER.with(|cell| {
    let mut slot = cell.borrow_mut();
    let prev = slot.replace(module);
    assert!(prev.is_none());
  });
  napi_ok
}

#[napi_sym]
fn napi_add_env_cleanup_hook(
  env: *mut Env,
  fun: Option<napi_cleanup_hook>,
  arg: *mut c_void,
) -> napi_status {
  let env = check_env!(env);
  check_arg!(env, fun);

  let fun = fun.unwrap();

  env.add_cleanup_hook(fun, arg);

  napi_ok
}

#[napi_sym]
fn napi_remove_env_cleanup_hook(
  env: *mut Env,
  fun: Option<napi_cleanup_hook>,
  arg: *mut c_void,
) -> napi_status {
  let env = check_env!(env);
  check_arg!(env, fun);

  let fun = fun.unwrap();

  env.remove_cleanup_hook(fun, arg);

  napi_ok
}

struct AsyncCleanupHandle {
  env: *mut Env,
  hook: napi_async_cleanup_hook,
  data: *mut c_void,
}

unsafe extern "C" fn async_cleanup_handler(arg: *mut c_void) {
  unsafe {
    let handle = Box::<AsyncCleanupHandle>::from_raw(arg as _);
    (handle.hook)(arg, handle.data);
  }
}

#[napi_sym]
fn napi_add_async_cleanup_hook(
  env: *mut Env,
  hook: Option<napi_async_cleanup_hook>,
  arg: *mut c_void,
  remove_handle: *mut napi_async_cleanup_hook_handle,
) -> napi_status {
  let env = check_env!(env);
  check_arg!(env, hook);

  let hook = hook.unwrap();

  let handle = Box::into_raw(Box::new(AsyncCleanupHandle {
    env,
    hook,
    data: arg,
  })) as *mut c_void;

  env.add_cleanup_hook(async_cleanup_handler, handle);

  if !remove_handle.is_null() {
    unsafe {
      *remove_handle = handle;
    }
  }

  napi_clear_last_error(env)
}

#[napi_sym]
fn napi_remove_async_cleanup_hook(
  remove_handle: napi_async_cleanup_hook_handle,
) -> napi_status {
  if remove_handle.is_null() {
    return napi_invalid_arg;
  }

  let handle =
    unsafe { Box::<AsyncCleanupHandle>::from_raw(remove_handle as _) };

  let env = unsafe { &mut *handle.env };

  env.remove_cleanup_hook(async_cleanup_handler, remove_handle);

  napi_ok
}

#[napi_sym]
fn napi_fatal_exception(env: &mut Env, err: napi_value) -> napi_status {
  check_arg!(env, err);

  let report_error = v8::Local::new(&mut env.scope(), &env.report_error);

  let this = v8::undefined(&mut env.scope());
  if report_error
    .call(&mut env.scope(), this.into(), &[err.unwrap()])
    .is_none()
  {
    return napi_generic_failure;
  }

  napi_ok
}

#[napi_sym]
#[allow(clippy::print_stderr)]
fn napi_fatal_error(
  location: *const c_char,
  location_len: usize,
  message: *const c_char,
  message_len: usize,
) -> napi_status {
  let location = if location.is_null() {
    None
  } else {
    unsafe {
      Some(if location_len == NAPI_AUTO_LENGTH {
        std::ffi::CStr::from_ptr(location).to_str().unwrap()
      } else {
        let slice = std::slice::from_raw_parts(
          location as *const u8,
          location_len as usize,
        );
        std::str::from_utf8(slice).unwrap()
      })
    }
  };

  let message = if message_len == NAPI_AUTO_LENGTH {
    unsafe { std::ffi::CStr::from_ptr(message).to_str().unwrap() }
  } else {
    let slice = unsafe {
      std::slice::from_raw_parts(message as *const u8, message_len as usize)
    };
    std::str::from_utf8(slice).unwrap()
  };

  if let Some(location) = location {
    eprintln!("NODE API FATAL ERROR: {} {}", location, message);
  } else {
    eprintln!("NODE API FATAL ERROR: {}", message);
  }

  std::process::abort();
}

#[napi_sym]
fn napi_open_callback_scope(
  env: *mut Env,
  _resource_object: napi_value,
  _context: napi_value,
  result: *mut napi_callback_scope,
) -> napi_status {
  let env = check_env!(env);
  check_arg!(env, result);

  // we open scope automatically when it's needed
  unsafe {
    *result = std::ptr::null_mut();
  }

  napi_clear_last_error(env)
}

#[napi_sym]
fn napi_close_callback_scope(
  env: *mut Env,
  scope: napi_callback_scope,
) -> napi_status {
  let env = check_env!(env);
  // we close scope automatically when it's needed
  assert!(scope.is_null());
  napi_clear_last_error(env)
}

// NOTE: we don't support "async_hooks::AsyncContext" so these APIs are noops.
#[napi_sym]
fn napi_async_init(
  env: *mut Env,
  _async_resource: napi_value,
  _async_resource_name: napi_value,
  result: *mut napi_async_context,
) -> napi_status {
  let env = check_env!(env);
  unsafe {
    *result = ptr::null_mut();
  }
  napi_clear_last_error(env)
}

#[napi_sym]
fn napi_async_destroy(
  env: *mut Env,
  async_context: napi_async_context,
) -> napi_status {
  let env = check_env!(env);
  assert!(async_context.is_null());
  napi_clear_last_error(env)
}

#[napi_sym]
fn napi_make_callback<'s>(
  env: &'s mut Env,
  _async_context: napi_async_context,
  recv: napi_value,
  func: napi_value,
  argc: usize,
  argv: *const napi_value<'s>,
  result: *mut napi_value<'s>,
) -> napi_status {
  check_arg!(env, recv);
  if argc > 0 {
    check_arg!(env, argv);
  }

  let Some(recv) = recv.and_then(|v| v.to_object(&mut env.scope())) else {
    return napi_object_expected;
  };

  let Some(func) =
    func.and_then(|v| v8::Local::<v8::Function>::try_from(v).ok())
  else {
    return napi_function_expected;
  };

  let args = if argc > 0 {
    unsafe {
      std::slice::from_raw_parts(argv as *mut v8::Local<v8::Value>, argc)
    }
  } else {
    &[]
  };

  // TODO: async_context

  let Some(v) = func.call(&mut env.scope(), recv.into(), args) else {
    return napi_generic_failure;
  };

  unsafe {
    *result = v.into();
  }

  napi_ok
}

#[napi_sym]
fn napi_create_buffer<'s>(
  env: &'s mut Env,
  length: usize,
  data: *mut *mut c_void,
  result: *mut napi_value<'s>,
) -> napi_status {
  check_arg!(env, result);

  let ab = v8::ArrayBuffer::new(&mut env.scope(), length);

  let buffer_constructor =
    v8::Local::new(&mut env.scope(), &env.buffer_constructor);
  let Some(buffer) =
    buffer_constructor.new_instance(&mut env.scope(), &[ab.into()])
  else {
    return napi_generic_failure;
  };

  if !data.is_null() {
    unsafe {
      *data = get_array_buffer_ptr(ab);
    }
  }

  unsafe {
    *result = buffer.into();
  }

  napi_ok
}

#[napi_sym]
fn napi_create_external_buffer<'s>(
  env: &'s mut Env,
  length: usize,
  data: *mut c_void,
  finalize_cb: napi_finalize,
  finalize_hint: *mut c_void,
  result: *mut napi_value<'s>,
) -> napi_status {
  check_arg!(env, result);

  let store = make_external_backing_store(
    env,
    data,
    length,
    ptr::null_mut(),
    finalize_cb,
    finalize_hint,
  );

  let ab =
    v8::ArrayBuffer::with_backing_store(&mut env.scope(), &store.make_shared());

  let buffer_constructor =
    v8::Local::new(&mut env.scope(), &env.buffer_constructor);
  let Some(buffer) =
    buffer_constructor.new_instance(&mut env.scope(), &[ab.into()])
  else {
    return napi_generic_failure;
  };

  unsafe {
    *result = buffer.into();
  }

  napi_ok
}

#[napi_sym]
fn napi_create_buffer_copy<'s>(
  env: &'s mut Env,
  length: usize,
  data: *mut c_void,
  result_data: *mut *mut c_void,
  result: *mut napi_value<'s>,
) -> napi_status {
  check_arg!(env, result);

  let ab = v8::ArrayBuffer::new(&mut env.scope(), length);

  let buffer_constructor =
    v8::Local::new(&mut env.scope(), &env.buffer_constructor);
  let Some(buffer) =
    buffer_constructor.new_instance(&mut env.scope(), &[ab.into()])
  else {
    return napi_generic_failure;
  };

  let ptr = get_array_buffer_ptr(ab);
  unsafe {
    std::ptr::copy(data, ptr, length);
  }

  if !result_data.is_null() {
    unsafe {
      *result_data = ptr;
    }
  }

  unsafe {
    *result = buffer.into();
  }

  napi_ok
}

#[napi_sym]
fn napi_is_buffer(
  env: *mut Env,
  value: napi_value,
  result: *mut bool,
) -> napi_status {
  let env = check_env!(env);
  check_arg!(env, value);
  check_arg!(env, result);

  let buffer_constructor =
    v8::Local::new(&mut env.scope(), &env.buffer_constructor);

  let Some(is_buffer) = value
    .unwrap()
    .instance_of(&mut env.scope(), buffer_constructor.into())
  else {
    return napi_set_last_error(env, napi_generic_failure);
  };

  unsafe {
    *result = is_buffer;
  }

  napi_clear_last_error(env)
}

#[napi_sym]
fn napi_get_buffer_info(
  env: *mut Env,
  value: napi_value,
  data: *mut *mut c_void,
  length: *mut usize,
) -> napi_status {
  let env = check_env!(env);
  check_arg!(env, value);

  // NB: Any TypedArray instance seems to be accepted by this function
  // in Node.js.
  let Some(ta) =
    value.and_then(|v| v8::Local::<v8::TypedArray>::try_from(v).ok())
  else {
    return napi_set_last_error(env, napi_invalid_arg);
  };

  if !data.is_null() {
    unsafe {
      *data = ta.data();
    }
  }

  if !length.is_null() {
    unsafe {
      *length = ta.byte_length();
    }
  }

  napi_clear_last_error(env)
}

#[napi_sym]
fn napi_get_node_version(
  env: *mut Env,
  result: *mut *const napi_node_version,
) -> napi_status {
  let env = check_env!(env);
  check_arg!(env, result);

  const NODE_VERSION: napi_node_version = napi_node_version {
    major: 20,
    minor: 11,
    patch: 1,
    release: c"Deno".as_ptr(),
  };

  unsafe {
    *result = &NODE_VERSION as *const napi_node_version;
  }

  napi_clear_last_error(env)
}

struct AsyncWork {
  state: AtomicU8,
  env: *mut Env,
  _async_resource: v8::Global<v8::Object>,
  _async_resource_name: String,
  execute: napi_async_execute_callback,
  complete: Option<napi_async_complete_callback>,
  data: *mut c_void,
}

impl AsyncWork {
  const IDLE: u8 = 0;
  const QUEUED: u8 = 1;
  const RUNNING: u8 = 2;
}

#[napi_sym]
fn napi_create_async_work(
  env: *mut Env,
  async_resource: napi_value,
  async_resource_name: napi_value,
  execute: Option<napi_async_execute_callback>,
  complete: Option<napi_async_complete_callback>,
  data: *mut c_void,
  result: *mut napi_async_work,
) -> napi_status {
  let env_ptr = env;
  let env = check_env!(env);
  check_arg!(env, execute);
  check_arg!(env, result);

  let resource = if let Some(v) = *async_resource {
    let Some(resource) = v.to_object(&mut env.scope()) else {
      return napi_set_last_error(env, napi_object_expected);
    };
    resource
  } else {
    v8::Object::new(&mut env.scope())
  };

  let Some(resource_name) =
    async_resource_name.and_then(|v| v.to_string(&mut env.scope()))
  else {
    return napi_set_last_error(env, napi_string_expected);
  };

  let resource_name = resource_name.to_rust_string_lossy(&mut env.scope());

  let work = Box::new(AsyncWork {
    state: AtomicU8::new(AsyncWork::IDLE),
    env: env_ptr,
    _async_resource: v8::Global::new(&mut env.scope(), resource),
    _async_resource_name: resource_name,
    execute: execute.unwrap(),
    complete,
    data,
  });

  unsafe {
    *result = Box::into_raw(work) as _;
  }

  napi_clear_last_error(env)
}

#[napi_sym]
fn napi_delete_async_work(env: *mut Env, work: napi_async_work) -> napi_status {
  let env = check_env!(env);
  check_arg!(env, work);

  drop(unsafe { Box::<AsyncWork>::from_raw(work as _) });

  napi_clear_last_error(env)
}

#[napi_sym]
fn napi_get_uv_event_loop(
  env_ptr: *mut Env,
  uv_loop: *mut *mut (),
) -> napi_status {
  let env = check_env!(env_ptr);
  check_arg!(env, uv_loop);
  unsafe {
    *uv_loop = env_ptr.cast();
  }
  0
}

#[napi_sym]
fn napi_queue_async_work(env: *mut Env, work: napi_async_work) -> napi_status {
  let env = check_env!(env);
  check_arg!(env, work);

  let work = unsafe { &*(work as *mut AsyncWork) };

  let result =
    work
      .state
      .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |state| {
        // allow queue if idle or if running, but not if already queued.
        if state == AsyncWork::IDLE || state == AsyncWork::RUNNING {
          Some(AsyncWork::QUEUED)
        } else {
          None
        }
      });

  if result.is_err() {
    return napi_clear_last_error(env);
  }

  let work = SendPtr(work);

  env.add_async_work(move || {
    let work = work.take();
    let work = unsafe { &*work };

    let state = work.state.compare_exchange(
      AsyncWork::QUEUED,
      AsyncWork::RUNNING,
      Ordering::SeqCst,
      Ordering::SeqCst,
    );

    if state.is_ok() {
      unsafe {
        (work.execute)(work.env as _, work.data);
      }

      // reset back to idle if its still marked as running
      let _ = work.state.compare_exchange(
        AsyncWork::RUNNING,
        AsyncWork::IDLE,
        Ordering::SeqCst,
        Ordering::Relaxed,
      );
    }

    if let Some(complete) = work.complete {
      let status = if state.is_ok() {
        napi_ok
      } else if state == Err(AsyncWork::IDLE) {
        napi_cancelled
      } else {
        napi_generic_failure
      };

      unsafe {
        complete(work.env as _, status, work.data);
      }
    }

    // `complete` probably deletes this `work`, so don't use it here.
  });

  napi_clear_last_error(env)
}

#[napi_sym]
fn napi_cancel_async_work(env: *mut Env, work: napi_async_work) -> napi_status {
  let env = check_env!(env);
  check_arg!(env, work);

  let work = unsafe { &*(work as *mut AsyncWork) };

  let _ = work.state.compare_exchange(
    AsyncWork::QUEUED,
    AsyncWork::IDLE,
    Ordering::SeqCst,
    Ordering::Relaxed,
  );

  napi_clear_last_error(env)
}

extern "C" fn default_call_js_cb(
  env: napi_env,
  js_callback: napi_value,
  _context: *mut c_void,
  _data: *mut c_void,
) {
  if let Some(js_callback) = *js_callback {
    if let Ok(js_callback) = v8::Local::<v8::Function>::try_from(js_callback) {
      let env = unsafe { &mut *(env as *mut Env) };
      let scope = &mut env.scope();
      let recv = v8::undefined(scope);
      js_callback.call(scope, recv.into(), &[]);
    }
  }
}

struct TsFn {
  env: *mut Env,
  func: Option<v8::Global<v8::Function>>,
  max_queue_size: usize,
  queue_size: Mutex<usize>,
  queue_cond: Condvar,
  thread_count: AtomicUsize,
  thread_finalize_data: *mut c_void,
  thread_finalize_cb: Option<napi_finalize>,
  context: *mut c_void,
  call_js_cb: napi_threadsafe_function_call_js,
  _resource: v8::Global<v8::Object>,
  _resource_name: String,
  is_closing: AtomicBool,
  is_closed: Arc<AtomicBool>,
  sender: V8CrossThreadTaskSpawner,
  is_ref: AtomicBool,
}

impl Drop for TsFn {
  fn drop(&mut self) {
    assert!(self
      .is_closed
      .compare_exchange(false, true, Ordering::Relaxed, Ordering::Relaxed)
      .is_ok());

    self.unref();

    if let Some(finalizer) = self.thread_finalize_cb {
      unsafe {
        (finalizer)(self.env as _, self.thread_finalize_data, ptr::null_mut());
      }
    }
  }
}

impl TsFn {
  pub fn acquire(&self) -> napi_status {
    if self.is_closing.load(Ordering::SeqCst) {
      return napi_closing;
    }
    self.thread_count.fetch_add(1, Ordering::Relaxed);
    napi_ok
  }

  pub fn release(
    tsfn: *mut TsFn,
    mode: napi_threadsafe_function_release_mode,
  ) -> napi_status {
    let tsfn = unsafe { &mut *tsfn };

    let result = tsfn.thread_count.fetch_update(
      Ordering::Relaxed,
      Ordering::Relaxed,
      |x| {
        if x == 0 {
          None
        } else {
          Some(x - 1)
        }
      },
    );

    if result.is_err() {
      return napi_invalid_arg;
    }

    if (result == Ok(1) || mode == napi_tsfn_abort)
      && tsfn
        .is_closing
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_ok()
    {
      tsfn.queue_cond.notify_all();
      let tsfnptr = SendPtr(tsfn);
      // drop must be queued in order to preserve ordering consistent
      // with Node.js and so that the finalizer runs on the main thread.
      tsfn.sender.spawn(move |_| {
        let tsfn = unsafe { Box::from_raw(tsfnptr.take() as *mut TsFn) };
        drop(tsfn);
      });
    }

    napi_ok
  }

  pub fn ref_(&self) -> napi_status {
    if self
      .is_ref
      .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
      .is_ok()
    {
      let env = unsafe { &mut *self.env };
      env.threadsafe_function_ref();
    }
    napi_ok
  }

  pub fn unref(&self) -> napi_status {
    if self
      .is_ref
      .compare_exchange(true, false, Ordering::SeqCst, Ordering::SeqCst)
      .is_ok()
    {
      let env = unsafe { &mut *self.env };
      env.threadsafe_function_unref();
    }

    napi_ok
  }

  pub fn call(
    &self,
    data: *mut c_void,
    mode: napi_threadsafe_function_call_mode,
  ) -> napi_status {
    if self.is_closing.load(Ordering::SeqCst) {
      return napi_closing;
    }

    if self.max_queue_size > 0 {
      let mut queue_size = self.queue_size.lock();
      while *queue_size >= self.max_queue_size {
        if mode == napi_tsfn_blocking {
          self.queue_cond.wait(&mut queue_size);

          if self.is_closing.load(Ordering::SeqCst) {
            return napi_closing;
          }
        } else {
          return napi_queue_full;
        }
      }
      *queue_size += 1;
    }

    let is_closed = self.is_closed.clone();
    let tsfn = SendPtr(self);
    let data = SendPtr(data);
    let context = SendPtr(self.context);
    let call_js_cb = self.call_js_cb;

    self.sender.spawn(move |scope: &mut v8::HandleScope| {
      let data = data.take();

      // if is_closed then tsfn is freed, don't read from it.
      if is_closed.load(Ordering::Relaxed) {
        unsafe {
          call_js_cb(
            std::ptr::null_mut(),
            None::<v8::Local<v8::Value>>.into(),
            context.take() as _,
            data as _,
          );
        }
      } else {
        let tsfn = tsfn.take();

        let tsfn = unsafe { &*tsfn };

        if tsfn.max_queue_size > 0 {
          let mut queue_size = tsfn.queue_size.lock();
          let size = *queue_size;
          *queue_size -= 1;
          if size == tsfn.max_queue_size {
            tsfn.queue_cond.notify_one();
          }
        }

        let func = tsfn.func.as_ref().map(|f| v8::Local::new(scope, f));

        unsafe {
          (tsfn.call_js_cb)(
            tsfn.env as _,
            func.into(),
            tsfn.context,
            data as _,
          );
        }
      }
    });

    napi_ok
  }
}

#[napi_sym]
#[allow(clippy::too_many_arguments)]
fn napi_create_threadsafe_function(
  env: *mut Env,
  func: napi_value,
  async_resource: napi_value,
  async_resource_name: napi_value,
  max_queue_size: usize,
  initial_thread_count: usize,
  thread_finalize_data: *mut c_void,
  thread_finalize_cb: Option<napi_finalize>,
  context: *mut c_void,
  call_js_cb: Option<napi_threadsafe_function_call_js>,
  result: *mut napi_threadsafe_function,
) -> napi_status {
  let env = check_env!(env);
  check_arg!(env, async_resource_name);
  if initial_thread_count == 0 {
    return napi_set_last_error(env, napi_invalid_arg);
  }
  check_arg!(env, result);

  let func = if let Some(value) = *func {
    let Ok(func) = v8::Local::<v8::Function>::try_from(value) else {
      return napi_set_last_error(env, napi_function_expected);
    };
    Some(v8::Global::new(&mut env.scope(), func))
  } else {
    check_arg!(env, call_js_cb);
    None
  };

  let resource = if let Some(v) = *async_resource {
    let Some(resource) = v.to_object(&mut env.scope()) else {
      return napi_set_last_error(env, napi_object_expected);
    };
    resource
  } else {
    v8::Object::new(&mut env.scope())
  };
  let resource = v8::Global::new(&mut env.scope(), resource);

  let Some(resource_name) =
    async_resource_name.and_then(|v| v.to_string(&mut env.scope()))
  else {
    return napi_set_last_error(env, napi_string_expected);
  };
  let resource_name = resource_name.to_rust_string_lossy(&mut env.scope());

  let mut tsfn = Box::new(TsFn {
    env,
    func,
    max_queue_size,
    queue_size: Mutex::new(0),
    queue_cond: Condvar::new(),
    thread_count: AtomicUsize::new(initial_thread_count),
    thread_finalize_data,
    thread_finalize_cb,
    context,
    call_js_cb: call_js_cb.unwrap_or(default_call_js_cb),
    _resource: resource,
    _resource_name: resource_name,
    is_closing: AtomicBool::new(false),
    is_closed: Arc::new(AtomicBool::new(false)),
    is_ref: AtomicBool::new(false),
    sender: env.async_work_sender.clone(),
  });

  tsfn.ref_();

  unsafe {
    *result = Box::into_raw(tsfn) as _;
  }

  napi_clear_last_error(env)
}

/// Maybe called from any thread.
#[napi_sym]
fn napi_get_threadsafe_function_context(
  func: napi_threadsafe_function,
  result: *mut *const c_void,
) -> napi_status {
  assert!(!func.is_null());
  let tsfn = unsafe { &*(func as *const TsFn) };
  unsafe {
    *result = tsfn.context;
  }
  napi_ok
}

#[napi_sym]
fn napi_call_threadsafe_function(
  func: napi_threadsafe_function,
  data: *mut c_void,
  is_blocking: napi_threadsafe_function_call_mode,
) -> napi_status {
  assert!(!func.is_null());
  let tsfn = unsafe { &*(func as *mut TsFn) };
  tsfn.call(data, is_blocking)
}

#[napi_sym]
fn napi_acquire_threadsafe_function(
  tsfn: napi_threadsafe_function,
) -> napi_status {
  assert!(!tsfn.is_null());
  let tsfn = unsafe { &*(tsfn as *mut TsFn) };
  tsfn.acquire()
}

#[napi_sym]
fn napi_release_threadsafe_function(
  tsfn: napi_threadsafe_function,
  mode: napi_threadsafe_function_release_mode,
) -> napi_status {
  assert!(!tsfn.is_null());
  TsFn::release(tsfn as _, mode)
}

#[napi_sym]
fn napi_unref_threadsafe_function(
  _env: &mut Env,
  func: napi_threadsafe_function,
) -> napi_status {
  assert!(!func.is_null());
  let tsfn = unsafe { &*(func as *mut TsFn) };
  tsfn.unref()
}

#[napi_sym]
fn napi_ref_threadsafe_function(
  _env: &mut Env,
  func: napi_threadsafe_function,
) -> napi_status {
  assert!(!func.is_null());
  let tsfn = unsafe { &*(func as *mut TsFn) };
  tsfn.ref_()
}

#[napi_sym]
fn node_api_get_module_file_name(
  env: *mut Env,
  result: *mut *const c_char,
) -> napi_status {
  let env = check_env!(env);
  check_arg!(env, result);

  unsafe {
    *result = env.shared().filename.as_ptr() as _;
  }

  napi_clear_last_error(env)
}
