use deno_core::futures::channel::mpsc;
use deno_core::napi::*;
use std::sync::mpsc::channel;

pub struct TsFn {
  pub js_func: v8::Global<v8::Function>,
  pub maybe_call_js_cb: Option<napi_threadsafe_function_call_js>,
  pub context: *mut c_void,
  pub thread_counter: usize,
  sender: mpsc::UnboundedSender<PendingNapiAsyncWork>,
  // Must not be used from outside the js thread!
  isolate_ptr: *mut v8::OwnedIsolate,
}

impl TsFn {
  pub fn aquire(&mut self) -> Result {
    self.thread_counter += 1;
    Ok(())
  }

  pub fn release(mut self) -> Result {
    self.thread_counter -= 1;
    if self.thread_counter == 0 {
      drop(self);
    }
    Ok(())
  }

  pub fn call(&self, data: *mut c_void, is_blocking: bool) {
    let js_func = self.js_func.clone();
    let (tx, rx) = channel();
    if let Some(call_js_cb) = self.maybe_call_js_cb {
      let context = self.context;
      let isolate_ptr = self.isolate_ptr;
      let sender = self.sender.clone();
      let call = Box::new(move |scope: &mut v8::HandleScope| {
        let func = js_func.open(scope).to_object(scope).unwrap();
        let mut env = Env::new(isolate_ptr, scope, sender);

        unsafe {
          call_js_cb(
            &mut env as *mut _ as *mut c_void,
            transmute::<v8::Local<v8::Value>, napi_value>(func.into()),
            context,
            data,
          )
        };

        // TODO: Reciever may be dropped
        tx.send(()).unwrap();
      });
      self.sender.unbounded_send(call);
    } else {
      let call = Box::new(move |scope: &mut v8::HandleScope| {
        let func = js_func.open(scope);
        // TODO: Reciever may be dropped
        tx.send(()).unwrap();
      });
      self.sender.unbounded_send(call);
    }

    if is_blocking {
      rx.recv().unwrap();
    }
  }
}

#[napi_sym::napi_sym]
fn napi_create_threadsafe_function(
  env: &mut Env,
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
  if initial_thread_count <= 0 {
    return Err(Error::InvalidArg);
  }
  let value = unsafe { transmute::<napi_value, v8::Local<v8::Value>>(func) };
  let func = v8::Local::<v8::Function>::try_from(value)
    .map_err(|_| Error::FunctionExpected)?;
  let js_func = v8::Global::new(env.scope, func);
  let tsfn = TsFn {
    js_func,
    maybe_call_js_cb,
    context,
    thread_counter: initial_thread_count,
    sender: env.async_work_sender.clone(),
    // We need to pass the isolate pointer
    // when calling the tsfn on the main thread.
    isolate_ptr: env.isolate_ptr,
  };

  *result = transmute::<Box<TsFn>, _>(Box::new(tsfn));

  Ok(())
}
