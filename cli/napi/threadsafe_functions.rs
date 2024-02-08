// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use deno_core::futures::channel::mpsc;
use deno_core::V8CrossThreadTaskSpawner;
use deno_runtime::deno_napi::*;
use once_cell::sync::Lazy;
use std::mem::forget;
use std::ptr::NonNull;
use std::sync::atomic::AtomicUsize;
use std::sync::Arc;

#[repr(transparent)]
pub struct SendPtr<T>(pub *const T);

unsafe impl<T> Send for SendPtr<T> {}
unsafe impl<T> Sync for SendPtr<T> {}

static TS_FN_ID_COUNTER: Lazy<AtomicUsize> = Lazy::new(|| AtomicUsize::new(0));

pub struct TsFn {
  pub id: usize,
  pub env: *mut Env,
  pub maybe_func: Option<v8::Global<v8::Function>>,
  pub maybe_call_js_cb: Option<napi_threadsafe_function_call_js>,
  pub context: *mut c_void,
  pub thread_counter: usize,
  pub ref_counter: Arc<AtomicUsize>,
  finalizer: Option<napi_finalize>,
  finalizer_data: *mut c_void,
  sender: V8CrossThreadTaskSpawner,
  tsfn_sender: mpsc::UnboundedSender<ThreadSafeFunctionStatus>,
}

impl Drop for TsFn {
  fn drop(&mut self) {
    let env = unsafe { self.env.as_mut().unwrap() };
    env.remove_threadsafe_function_ref_counter(self.id);
    if let Some(finalizer) = self.finalizer {
      unsafe {
        (finalizer)(self.env as _, self.finalizer_data, ptr::null_mut());
      }
    }
  }
}

impl TsFn {
  pub fn acquire(&mut self) -> napi_status {
    self.thread_counter += 1;
    napi_ok
  }

  pub fn release(mut self) -> napi_status {
    self.thread_counter -= 1;
    if self.thread_counter == 0 {
      if self
        .tsfn_sender
        .unbounded_send(ThreadSafeFunctionStatus::Dead)
        .is_err()
      {
        return napi_generic_failure;
      }
      drop(self);
    } else {
      forget(self);
    }
    napi_ok
  }

  pub fn ref_(&mut self) -> napi_status {
    self
      .ref_counter
      .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    napi_ok
  }

  pub fn unref(&mut self) -> napi_status {
    let _ = self.ref_counter.fetch_update(
      std::sync::atomic::Ordering::SeqCst,
      std::sync::atomic::Ordering::SeqCst,
      |x| {
        if x == 0 {
          None
        } else {
          Some(x - 1)
        }
      },
    );

    napi_ok
  }

  pub fn call(&self, data: *mut c_void, is_blocking: bool) {
    let js_func = self.maybe_func.clone();

    let env = SendPtr(self.env);
    let context = SendPtr(self.context);
    let data = SendPtr(data);

    #[inline(always)]
    fn spawn(
      sender: &V8CrossThreadTaskSpawner,
      is_blocking: bool,
      f: impl FnOnce(&mut v8::HandleScope) + Send + 'static,
    ) {
      if is_blocking {
        sender.spawn_blocking(f);
      } else {
        sender.spawn(f);
      }
    }

    if let Some(call_js_cb) = self.maybe_call_js_cb {
      if let Some(func) = js_func {
        let func = SendPtr(func.into_raw().as_ptr());
        #[inline(always)]
        fn call(
          scope: &mut v8::HandleScope,
          call_js_cb: napi_threadsafe_function_call_js,
          func: SendPtr<v8::Function>,
          env: SendPtr<Env>,
          context: SendPtr<c_void>,
          data: SendPtr<c_void>,
        ) {
          // SAFETY: This is a valid global from above
          let func: v8::Global<v8::Function> = unsafe {
            v8::Global::<v8::Function>::from_raw(
              scope,
              NonNull::new_unchecked(func.0 as _),
            )
          };
          let func: v8::Local<v8::Value> =
            func.open(scope).to_object(scope).unwrap().into();
          // SAFETY: env is valid for the duration of the callback.
          //         data lifetime is users responsibility.
          unsafe {
            call_js_cb(env.0 as _, func.into(), context.0 as _, data.0 as _)
          }
        }
        spawn(&self.sender, is_blocking, move |scope| {
          call(scope, call_js_cb, func, env, context, data);
        });
      } else {
        #[inline(always)]
        fn call(
          call_js_cb: napi_threadsafe_function_call_js,
          env: SendPtr<Env>,
          context: SendPtr<c_void>,
          data: SendPtr<c_void>,
        ) {
          // SAFETY: env is valid for the duration of the callback.
          //         data lifetime is users responsibility.
          unsafe {
            call_js_cb(
              env.0 as _,
              std::mem::zeroed(),
              context.0 as _,
              data.0 as _,
            )
          }
        }
        spawn(&self.sender, is_blocking, move |_| {
          call(call_js_cb, env, context, data);
        });
      }
    } else {
      spawn(&self.sender, is_blocking, |_| {
        // TODO: func.call
      });
    };
  }
}

#[napi_sym::napi_sym]
fn napi_create_threadsafe_function(
  env: *mut Env,
  func: napi_value,
  _async_resource: napi_value,
  _async_resource_name: napi_value,
  _max_queue_size: usize,
  initial_thread_count: usize,
  thread_finalize_data: *mut c_void,
  thread_finalize_cb: Option<napi_finalize>,
  context: *mut c_void,
  maybe_call_js_cb: Option<napi_threadsafe_function_call_js>,
  result: *mut napi_threadsafe_function,
) -> napi_status {
  let Some(env_ref) = env.as_mut() else {
    return napi_generic_failure;
  };
  if initial_thread_count == 0 {
    return napi_invalid_arg;
  }

  let mut maybe_func = None;

  if let Some(value) = *func {
    let Ok(func) = v8::Local::<v8::Function>::try_from(value) else {
      return napi_function_expected;
    };
    maybe_func = Some(v8::Global::new(&mut env_ref.scope(), func));
  }

  let id = TS_FN_ID_COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);

  let tsfn = TsFn {
    id,
    maybe_func,
    maybe_call_js_cb,
    context,
    thread_counter: initial_thread_count,
    sender: env_ref.async_work_sender.clone(),
    finalizer: thread_finalize_cb,
    finalizer_data: thread_finalize_data,
    tsfn_sender: env_ref.threadsafe_function_sender.clone(),
    ref_counter: Arc::new(AtomicUsize::new(1)),
    env,
  };

  env_ref
    .add_threadsafe_function_ref_counter(tsfn.id, tsfn.ref_counter.clone());

  if env_ref
    .threadsafe_function_sender
    .unbounded_send(ThreadSafeFunctionStatus::Alive)
    .is_err()
  {
    return napi_generic_failure;
  }
  *result = transmute::<Box<TsFn>, _>(Box::new(tsfn));

  napi_ok
}

#[napi_sym::napi_sym]
fn napi_acquire_threadsafe_function(
  tsfn: napi_threadsafe_function,
  _mode: napi_threadsafe_function_release_mode,
) -> napi_status {
  let tsfn: &mut TsFn = &mut *(tsfn as *mut TsFn);
  tsfn.acquire()
}

#[napi_sym::napi_sym]
fn napi_unref_threadsafe_function(
  _env: &mut Env,
  tsfn: napi_threadsafe_function,
) -> napi_status {
  let tsfn: &mut TsFn = &mut *(tsfn as *mut TsFn);
  tsfn.unref()
}

/// Maybe called from any thread.
#[napi_sym::napi_sym]
pub fn napi_get_threadsafe_function_context(
  func: napi_threadsafe_function,
  result: *mut *const c_void,
) -> napi_status {
  let tsfn: &TsFn = &*(func as *const TsFn);
  *result = tsfn.context;
  napi_ok
}

#[napi_sym::napi_sym]
fn napi_call_threadsafe_function(
  func: napi_threadsafe_function,
  data: *mut c_void,
  is_blocking: napi_threadsafe_function_call_mode,
) -> napi_status {
  let tsfn: &TsFn = &*(func as *const TsFn);
  tsfn.call(data, is_blocking != 0);
  napi_ok
}

#[napi_sym::napi_sym]
fn napi_ref_threadsafe_function(
  _env: &mut Env,
  func: napi_threadsafe_function,
) -> napi_status {
  let tsfn: &mut TsFn = &mut *(func as *mut TsFn);
  tsfn.ref_()
}

#[napi_sym::napi_sym]
fn napi_release_threadsafe_function(
  tsfn: napi_threadsafe_function,
  _mode: napi_threadsafe_function_release_mode,
) -> napi_status {
  let tsfn: Box<TsFn> = Box::from_raw(tsfn as *mut TsFn);
  tsfn.release()
}
