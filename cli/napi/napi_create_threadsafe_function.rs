use deno_core::futures::channel::mpsc;
use deno_core::napi::*;
use std::sync::mpsc::channel;
use std::mem::forget;

pub struct TsFn {
  pub maybe_func: Option<v8::Global<v8::Function>>,
  pub maybe_call_js_cb: Option<napi_threadsafe_function_call_js>,
  pub context: *mut c_void,
  pub thread_counter: usize,
  sender: mpsc::UnboundedSender<PendingNapiAsyncWork>,
  // Must not be used from outside the js thread!
  isolate_ptr: *mut v8::OwnedIsolate,
  tsfn_sender: mpsc::UnboundedSender<ThreadSafeFunctionStatus>,
}

impl TsFn {
  pub fn aquire(&mut self) -> Result {
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
      let isolate_ptr = self.isolate_ptr;
      let sender = self.sender.clone();
      let tsfn_sender = self.tsfn_sender.clone();
      let call = Box::new(move |scope: &mut v8::HandleScope| {
        let func = js_func.unwrap();
        let func: v8::Local<v8::Value> = func.open(scope).to_object(scope).unwrap().into();
        let mut env = Env::new(isolate_ptr, scope, sender, tsfn_sender);
                
        unsafe {
          call_js_cb(
            &mut env as *mut _ as *mut c_void,
            transmute::<v8::Local<v8::Value>, napi_value>(func),
            context,
            data,
          )
        };

        // TODO: Reciever may be dropped
        tx.send(());
      });
      self.sender.unbounded_send(call);
    } else if let Some(js_func) = js_func {
      let call = Box::new(move |scope: &mut v8::HandleScope| {
        let func = js_func.open(scope);
        // TODO: Reciever may be dropped
        tx.send(());
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
  let maybe_func = func
    .as_mut()
    .map(|func| {
      let value =
        unsafe { transmute::<napi_value, v8::Local<v8::Value>>(func) };
      let func = v8::Local::<v8::Function>::try_from(value)
        .map_err(|_| Error::FunctionExpected)?;
      Ok(v8::Global::new(env.scope, func))
    })
    .transpose()?;
  let tsfn = TsFn {
    maybe_func,
    maybe_call_js_cb,
    context,
    thread_counter: initial_thread_count,
    sender: env.async_work_sender.clone(),
    tsfn_sender: env.threadsafe_function_sender.clone(),
    // We need to pass the isolate pointer
    // when calling the tsfn on the main thread.
    isolate_ptr: env.isolate_ptr,
  };

  env
    .threadsafe_function_sender
    .unbounded_send(ThreadSafeFunctionStatus::Alive)
    .map_err(|_| Error::GenericFailure)?;
  *result = transmute::<Box<TsFn>, _>(Box::new(tsfn));

  Ok(())
}
