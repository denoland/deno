// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use deno_core::futures::channel::mpsc;
use deno_runtime::deno_napi::*;
use std::mem::forget;
use std::sync::mpsc::channel;

pub struct TsFn {
  pub env: *mut Env,
  pub maybe_func: Option<v8::Global<v8::Function>>,
  pub maybe_call_js_cb: Option<napi_threadsafe_function_call_js>,
  pub context: *mut c_void,
  pub thread_counter: usize,
  sender: mpsc::UnboundedSender<PendingNapiAsyncWork>,
  tsfn_sender: mpsc::UnboundedSender<ThreadSafeFunctionStatus>,
}

impl TsFn {
  pub fn acquire(&mut self) -> Result {
    self.thread_counter += 1;
    Ok(())
  }

  pub fn release(mut self) -> Result {
    self.thread_counter -= 1;
    if self.thread_counter == 0 {
      self
        .tsfn_sender
        .unbounded_send(ThreadSafeFunctionStatus::Dead)
        .map_err(|_| Error::GenericFailure)?;
      drop(self);
    } else {
      forget(self);
    }
    Ok(())
  }

  pub fn call(&self, data: *mut c_void, is_blocking: bool) {
    let js_func = self.maybe_func.clone();
    let (tx, rx) = channel();

    if let Some(call_js_cb) = self.maybe_call_js_cb {
      let context = self.context;
      let env = self.env;
      let call = Box::new(move || {
        let scope = &mut unsafe { (*env).scope() };
        match js_func {
          Some(func) => {
            let func: v8::Local<v8::Value> =
              func.open(scope).to_object(scope).unwrap().into();
            unsafe {
              call_js_cb(env as *mut c_void, func.into(), context, data)
            };
          }
          None => {
            unsafe {
              call_js_cb(env as *mut c_void, std::mem::zeroed(), context, data)
            };
          }
        }

        // Receiver might have been already dropped
        let _ = tx.send(());
      });
      // This call should never fail
      self.sender.unbounded_send(call).unwrap();
    } else if let Some(_js_func) = js_func {
      let call = Box::new(move || {
        // TODO: func.call
        // let func = js_func.open(scope);
        // Receiver might have been already dropped
        let _ = tx.send(());
      });
      // This call should never fail
      self.sender.unbounded_send(call).unwrap();
    }

    if is_blocking {
      rx.recv().unwrap();
    }
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
  _thread_finialize_data: *mut c_void,
  _thread_finalize_cb: napi_finalize,
  context: *mut c_void,
  maybe_call_js_cb: Option<napi_threadsafe_function_call_js>,
  result: *mut napi_threadsafe_function,
) -> Result {
  let env_ref = env.as_mut().ok_or(Error::GenericFailure)?;
  if initial_thread_count == 0 {
    return Err(Error::InvalidArg);
  }
  let maybe_func = func
    .map(|value| {
      let func = v8::Local::<v8::Function>::try_from(value)
        .map_err(|_| Error::FunctionExpected)?;
      Ok(v8::Global::new(&mut env_ref.scope(), func))
    })
    .transpose()?;

  let tsfn = TsFn {
    maybe_func,
    maybe_call_js_cb,
    context,
    thread_counter: initial_thread_count,
    sender: env_ref.async_work_sender.clone(),
    tsfn_sender: env_ref.threadsafe_function_sender.clone(),
    env,
  };

  env_ref
    .threadsafe_function_sender
    .unbounded_send(ThreadSafeFunctionStatus::Alive)
    .map_err(|_| Error::GenericFailure)?;
  *result = transmute::<Box<TsFn>, _>(Box::new(tsfn));

  Ok(())
}

#[napi_sym::napi_sym]
fn napi_acquire_threadsafe_function(
  tsfn: napi_threadsafe_function,
  _mode: napi_threadsafe_function_release_mode,
) -> Result {
  let tsfn: &mut TsFn = &mut *(tsfn as *mut TsFn);
  tsfn.acquire()?;

  Ok(())
}

#[napi_sym::napi_sym]
fn napi_unref_threadsafe_function(
  _env: &mut Env,
  tsfn: napi_threadsafe_function,
) -> Result {
  let _tsfn: &TsFn = &*(tsfn as *const TsFn);

  Ok(())
}

/// Maybe called from any thread.
#[napi_sym::napi_sym]
pub fn napi_get_threadsafe_function_context(
  func: napi_threadsafe_function,
  result: *mut *const c_void,
) -> Result {
  let tsfn: &TsFn = &*(func as *const TsFn);
  *result = tsfn.context;
  Ok(())
}

#[napi_sym::napi_sym]
fn napi_call_threadsafe_function(
  func: napi_threadsafe_function,
  data: *mut c_void,
  is_blocking: napi_threadsafe_function_call_mode,
) -> Result {
  let tsfn: &TsFn = &*(func as *const TsFn);
  tsfn.call(data, is_blocking != 0);
  Ok(())
}

#[napi_sym::napi_sym]
fn napi_ref_threadsafe_function() -> Result {
  // TODO
  Ok(())
}

#[napi_sym::napi_sym]
fn napi_release_threadsafe_function(
  tsfn: napi_threadsafe_function,
  _mode: napi_threadsafe_function_release_mode,
) -> Result {
  let tsfn: Box<TsFn> = Box::from_raw(tsfn as *mut TsFn);
  tsfn.release()?;

  Ok(())
}
