// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use crate::check_unstable;
use crate::symbol::NativeType;
use crate::FfiPermissions;
use crate::FfiState;
use crate::ForeignFunction;
use crate::PendingFfiAsyncWork;
use crate::LOCAL_ISOLATE_POINTER;
use crate::MAX_SAFE_INTEGER;
use crate::MIN_SAFE_INTEGER;
use deno_core::error::AnyError;
use deno_core::futures::channel::mpsc;
use deno_core::op;
use deno_core::serde_v8;
use deno_core::v8;
use deno_core::CancelFuture;
use deno_core::CancelHandle;
use deno_core::OpState;
use deno_core::Resource;
use deno_core::ResourceId;
use libffi::middle::Cif;
use serde::Deserialize;
use std::borrow::Cow;
use std::cell::RefCell;
use std::ffi::c_void;
use std::future::Future;
use std::future::IntoFuture;
use std::pin::Pin;
use std::ptr;
use std::ptr::NonNull;
use std::rc::Rc;
use std::sync::mpsc::sync_channel;
use std::task::Poll;
use std::task::Waker;
#[derive(Clone)]
pub struct PtrSymbol {
  pub cif: libffi::middle::Cif,
  pub ptr: libffi::middle::CodePtr,
}

impl PtrSymbol {
  pub fn new(fn_ptr: usize, def: &ForeignFunction) -> Self {
    let ptr = libffi::middle::CodePtr::from_ptr(fn_ptr as _);
    let cif = libffi::middle::Cif::new(
      def
        .parameters
        .clone()
        .into_iter()
        .map(libffi::middle::Type::from),
      def.result.clone().into(),
    );

    Self { cif, ptr }
  }
}

#[allow(clippy::non_send_fields_in_send_ty)]
// SAFETY: unsafe trait must have unsafe implementation
unsafe impl Send for PtrSymbol {}
// SAFETY: unsafe trait must have unsafe implementation
unsafe impl Sync for PtrSymbol {}

struct UnsafeCallbackResource {
  cancel: Rc<CancelHandle>,
  // Closure is never directly touched, but it keeps the C callback alive
  // until `close()` method is called.
  #[allow(dead_code)]
  closure: libffi::middle::Closure<'static>,
  info: *mut CallbackInfo,
}

impl Resource for UnsafeCallbackResource {
  fn name(&self) -> Cow<str> {
    "unsafecallback".into()
  }

  fn close(self: Rc<Self>) {
    self.cancel.cancel();
    // SAFETY: This drops the closure and the callback info associated with it.
    // Any retained function pointers to the closure become dangling pointers.
    // It is up to the user to know that it is safe to call the `close()` on the
    // UnsafeCallback instance.
    unsafe {
      let info = Box::from_raw(self.info);
      let isolate = info.isolate.as_mut().unwrap();
      let _ = v8::Global::from_raw(isolate, info.callback);
      let _ = v8::Global::from_raw(isolate, info.context);
    }
  }
}

struct CallbackInfo {
  pub parameters: Vec<NativeType>,
  pub result: NativeType,
  pub async_work_sender: mpsc::UnboundedSender<PendingFfiAsyncWork>,
  pub callback: NonNull<v8::Function>,
  pub context: NonNull<v8::Context>,
  pub isolate: *mut v8::Isolate,
  pub waker: Option<Waker>,
}

impl Future for CallbackInfo {
  type Output = ();
  fn poll(
    mut self: Pin<&mut Self>,
    cx: &mut std::task::Context<'_>,
  ) -> std::task::Poll<Self::Output> {
    // Always replace the waker to make sure it's bound to the proper Future.
    self.waker.replace(cx.waker().clone());
    // The future for the CallbackInfo never resolves: It can only be canceled.
    Poll::Pending
  }
}
unsafe extern "C" fn deno_ffi_callback(
  cif: &libffi::low::ffi_cif,
  result: &mut c_void,
  args: *const *const c_void,
  info: &CallbackInfo,
) {
  LOCAL_ISOLATE_POINTER.with(|s| {
    if ptr::eq(*s.borrow(), info.isolate) {
      // Own isolate thread, okay to call directly
      do_ffi_callback(cif, info, result, args);
    } else {
      let async_work_sender = &info.async_work_sender;
      // SAFETY: Safe as this function blocks until `do_ffi_callback` completes and a response message is received.
      let cif: &'static libffi::low::ffi_cif = std::mem::transmute(cif);
      let result: &'static mut c_void = std::mem::transmute(result);
      let info: &'static CallbackInfo = std::mem::transmute(info);
      let (response_sender, response_receiver) = sync_channel::<()>(0);
      let fut = Box::new(move || {
        do_ffi_callback(cif, info, result, args);
        response_sender.send(()).unwrap();
      });
      async_work_sender.unbounded_send(fut).unwrap();
      if let Some(waker) = info.waker.as_ref() {
        // Make sure event loop wakes up to receive our message before we start waiting for a response.
        waker.wake_by_ref();
      }
      response_receiver.recv().unwrap();
    }
  });
}

unsafe fn do_ffi_callback(
  cif: &libffi::low::ffi_cif,
  info: &CallbackInfo,
  result: &mut c_void,
  args: *const *const c_void,
) {
  let callback: NonNull<v8::Function> = info.callback;
  let context: NonNull<v8::Context> = info.context;
  let isolate: *mut v8::Isolate = info.isolate;
  let isolate = &mut *isolate;
  let callback = v8::Global::from_raw(isolate, callback);
  let context = std::mem::transmute::<
    NonNull<v8::Context>,
    v8::Local<v8::Context>,
  >(context);
  // Call from main thread. If this callback is being triggered due to a
  // function call coming from Deno itself, then this callback will build
  // ontop of that stack.
  // If this callback is being triggered outside of Deno (for example from a
  // signal handler) then this will either create an empty new stack if
  // Deno currently has nothing running and is waiting for promises to resolve,
  // or will (very incorrectly) build ontop of whatever stack exists.
  // The callback will even be called through from a `while (true)` liveloop, but
  // it somehow cannot change the values that the loop sees, even if they both
  // refer the same `let bool_value`.
  let mut cb_scope = v8::CallbackScope::new(context);
  let scope = &mut v8::HandleScope::new(&mut cb_scope);
  let func = callback.open(scope);
  let result = result as *mut c_void;
  let vals: &[*const c_void] =
    std::slice::from_raw_parts(args, info.parameters.len());
  let arg_types = std::slice::from_raw_parts(cif.arg_types, cif.nargs as usize);

  let mut params: Vec<v8::Local<v8::Value>> = vec![];
  for ((index, native_type), val) in
    info.parameters.iter().enumerate().zip(vals)
  {
    let value: v8::Local<v8::Value> = match native_type {
      NativeType::Bool => {
        let value = *((*val) as *const bool);
        v8::Boolean::new(scope, value).into()
      }
      NativeType::F32 => {
        let value = *((*val) as *const f32);
        v8::Number::new(scope, value as f64).into()
      }
      NativeType::F64 => {
        let value = *((*val) as *const f64);
        v8::Number::new(scope, value).into()
      }
      NativeType::I8 => {
        let value = *((*val) as *const i8);
        v8::Integer::new(scope, value as i32).into()
      }
      NativeType::U8 => {
        let value = *((*val) as *const u8);
        v8::Integer::new_from_unsigned(scope, value as u32).into()
      }
      NativeType::I16 => {
        let value = *((*val) as *const i16);
        v8::Integer::new(scope, value as i32).into()
      }
      NativeType::U16 => {
        let value = *((*val) as *const u16);
        v8::Integer::new_from_unsigned(scope, value as u32).into()
      }
      NativeType::I32 => {
        let value = *((*val) as *const i32);
        v8::Integer::new(scope, value).into()
      }
      NativeType::U32 => {
        let value = *((*val) as *const u32);
        v8::Integer::new_from_unsigned(scope, value).into()
      }
      NativeType::I64 | NativeType::ISize => {
        let result = *((*val) as *const i64);
        if result > MAX_SAFE_INTEGER as i64 || result < MIN_SAFE_INTEGER as i64
        {
          v8::BigInt::new_from_i64(scope, result).into()
        } else {
          v8::Number::new(scope, result as f64).into()
        }
      }
      NativeType::U64 | NativeType::USize => {
        let result = *((*val) as *const u64);
        if result > MAX_SAFE_INTEGER as u64 {
          v8::BigInt::new_from_u64(scope, result).into()
        } else {
          v8::Number::new(scope, result as f64).into()
        }
      }
      NativeType::Pointer | NativeType::Buffer | NativeType::Function => {
        let result = *((*val) as *const usize);
        if result > MAX_SAFE_INTEGER as usize {
          v8::BigInt::new_from_u64(scope, result as u64).into()
        } else {
          v8::Number::new(scope, result as f64).into()
        }
      }
      NativeType::Struct(_) => {
        let size = arg_types[index].as_ref().unwrap().size;
        let ptr = (*val) as *const u8;
        let slice = std::slice::from_raw_parts(ptr, size);
        let boxed = Box::from(slice);
        let store = v8::ArrayBuffer::new_backing_store_from_boxed_slice(boxed);
        let ab =
          v8::ArrayBuffer::with_backing_store(scope, &store.make_shared());
        let local_value: v8::Local<v8::Value> =
          v8::Uint8Array::new(scope, ab, 0, ab.byte_length())
            .unwrap()
            .into();
        local_value
      }
      NativeType::Void => unreachable!(),
    };
    params.push(value);
  }

  let recv = v8::undefined(scope);
  let call_result = func.call(scope, recv.into(), &params);
  std::mem::forget(callback);

  if call_result.is_none() {
    // JS function threw an exception. Set the return value to zero and return.
    // The exception continue propagating up the call chain when the event loop
    // resumes.
    match info.result {
      NativeType::Bool => {
        *(result as *mut bool) = false;
      }
      NativeType::U32 | NativeType::I32 => {
        // zero is equal for signed and unsigned alike
        *(result as *mut u32) = 0;
      }
      NativeType::F32 => {
        *(result as *mut f32) = 0.0;
      }
      NativeType::F64 => {
        *(result as *mut f64) = 0.0;
      }
      NativeType::U8 | NativeType::I8 => {
        // zero is equal for signed and unsigned alike
        *(result as *mut u8) = 0;
      }
      NativeType::U16 | NativeType::I16 => {
        // zero is equal for signed and unsigned alike
        *(result as *mut u16) = 0;
      }
      NativeType::Pointer
      | NativeType::Buffer
      | NativeType::Function
      | NativeType::U64
      | NativeType::I64 => {
        *(result as *mut usize) = 0;
      }
      NativeType::Void => {
        // nop
      }
      _ => {
        unreachable!();
      }
    };

    return;
  }
  let value = call_result.unwrap();

  match info.result {
    NativeType::Bool => {
      let value = if let Ok(value) = v8::Local::<v8::Boolean>::try_from(value) {
        value.is_true()
      } else {
        value.boolean_value(scope)
      };
      *(result as *mut bool) = value;
    }
    NativeType::I32 => {
      let value = if let Ok(value) = v8::Local::<v8::Integer>::try_from(value) {
        value.value() as i32
      } else {
        // Fallthrough, probably UB.
        value
          .int32_value(scope)
          .expect("Unable to deserialize result parameter.")
      };
      *(result as *mut i32) = value;
    }
    NativeType::F32 => {
      let value = if let Ok(value) = v8::Local::<v8::Number>::try_from(value) {
        value.value() as f32
      } else {
        // Fallthrough, probably UB.
        value
          .number_value(scope)
          .expect("Unable to deserialize result parameter.") as f32
      };
      *(result as *mut f32) = value;
    }
    NativeType::F64 => {
      let value = if let Ok(value) = v8::Local::<v8::Number>::try_from(value) {
        value.value()
      } else {
        // Fallthrough, probably UB.
        value
          .number_value(scope)
          .expect("Unable to deserialize result parameter.")
      };
      *(result as *mut f64) = value;
    }
    NativeType::Pointer | NativeType::Buffer | NativeType::Function => {
      let pointer = if let Ok(value) =
        v8::Local::<v8::ArrayBufferView>::try_from(value)
      {
        let byte_offset = value.byte_offset();
        let backing_store = value
          .buffer(scope)
          .expect("Unable to deserialize result parameter.")
          .get_backing_store();
        &backing_store[byte_offset..] as *const _ as *const u8
      } else if let Ok(value) = v8::Local::<v8::BigInt>::try_from(value) {
        value.u64_value().0 as usize as *const u8
      } else if let Ok(value) = v8::Local::<v8::ArrayBuffer>::try_from(value) {
        let backing_store = value.get_backing_store();
        &backing_store[..] as *const _ as *const u8
      } else if let Ok(value) = v8::Local::<v8::Integer>::try_from(value) {
        value.value() as usize as *const u8
      } else if value.is_null() {
        ptr::null()
      } else {
        // Fallthrough: Probably someone returned a number but this could
        // also be eg. a string. This is essentially UB.
        value
          .integer_value(scope)
          .expect("Unable to deserialize result parameter.") as usize
          as *const u8
      };
      *(result as *mut *const u8) = pointer;
    }
    NativeType::I8 => {
      let value = if let Ok(value) = v8::Local::<v8::Integer>::try_from(value) {
        value.value() as i8
      } else {
        // Fallthrough, essentially UB.
        value
          .int32_value(scope)
          .expect("Unable to deserialize result parameter.") as i8
      };
      *(result as *mut i8) = value;
    }
    NativeType::U8 => {
      let value = if let Ok(value) = v8::Local::<v8::Integer>::try_from(value) {
        value.value() as u8
      } else {
        // Fallthrough, essentially UB.
        value
          .uint32_value(scope)
          .expect("Unable to deserialize result parameter.") as u8
      };
      *(result as *mut u8) = value;
    }
    NativeType::I16 => {
      let value = if let Ok(value) = v8::Local::<v8::Integer>::try_from(value) {
        value.value() as i16
      } else {
        // Fallthrough, essentially UB.
        value
          .int32_value(scope)
          .expect("Unable to deserialize result parameter.") as i16
      };
      *(result as *mut i16) = value;
    }
    NativeType::U16 => {
      let value = if let Ok(value) = v8::Local::<v8::Integer>::try_from(value) {
        value.value() as u16
      } else {
        // Fallthrough, essentially UB.
        value
          .uint32_value(scope)
          .expect("Unable to deserialize result parameter.") as u16
      };
      *(result as *mut u16) = value;
    }
    NativeType::U32 => {
      let value = if let Ok(value) = v8::Local::<v8::Integer>::try_from(value) {
        value.value() as u32
      } else {
        // Fallthrough, essentially UB.
        value
          .uint32_value(scope)
          .expect("Unable to deserialize result parameter.")
      };
      *(result as *mut u32) = value;
    }
    NativeType::I64 => {
      if let Ok(value) = v8::Local::<v8::BigInt>::try_from(value) {
        *(result as *mut i64) = value.i64_value().0;
      } else if let Ok(value) = v8::Local::<v8::Integer>::try_from(value) {
        *(result as *mut i64) = value.value();
      } else {
        *(result as *mut i64) = value
          .integer_value(scope)
          .expect("Unable to deserialize result parameter.");
      }
    }
    NativeType::U64 => {
      if let Ok(value) = v8::Local::<v8::BigInt>::try_from(value) {
        *(result as *mut u64) = value.u64_value().0;
      } else if let Ok(value) = v8::Local::<v8::Integer>::try_from(value) {
        *(result as *mut u64) = value.value() as u64;
      } else {
        *(result as *mut u64) = value
          .integer_value(scope)
          .expect("Unable to deserialize result parameter.")
          as u64;
      }
    }
    NativeType::Struct(_) => {
      let size;
      let pointer = if let Ok(value) =
        v8::Local::<v8::ArrayBufferView>::try_from(value)
      {
        let byte_offset = value.byte_offset();
        let ab = value
          .buffer(scope)
          .expect("Unable to deserialize result parameter.");
        size = value.byte_length();
        ab.data()
          .expect("Unable to deserialize result parameter.")
          .as_ptr()
          .add(byte_offset)
      } else if let Ok(value) = v8::Local::<v8::ArrayBuffer>::try_from(value) {
        size = value.byte_length();
        value
          .data()
          .expect("Unable to deserialize result parameter.")
          .as_ptr()
      } else {
        panic!("Unable to deserialize result parameter.");
      };
      std::ptr::copy_nonoverlapping(
        pointer as *mut u8,
        result as *mut u8,
        std::cmp::min(size, (*cif.rtype).size),
      );
    }
    NativeType::Void => {
      // nop
    }
    _ => {
      unreachable!();
    }
  };
}

#[op]
pub fn op_ffi_unsafe_callback_ref(
  state: Rc<RefCell<OpState>>,
  rid: ResourceId,
) -> Result<impl Future<Output = Result<(), AnyError>>, AnyError> {
  let state = state.borrow();
  let callback_resource =
    state.resource_table.get::<UnsafeCallbackResource>(rid)?;

  Ok(async move {
    let info: &mut CallbackInfo =
    // SAFETY: CallbackInfo pointer stays valid as long as the resource is still alive.
      unsafe { callback_resource.info.as_mut().unwrap() };
    // Ignore cancellation rejection
    let _ = info
      .into_future()
      .or_cancel(callback_resource.cancel.clone())
      .await;
    Ok(())
  })
}

#[op(fast)]
pub fn op_ffi_unsafe_callback_unref(
  state: &mut deno_core::OpState,
  rid: u32,
) -> Result<(), AnyError> {
  state
    .resource_table
    .get::<UnsafeCallbackResource>(rid)?
    .cancel
    .cancel();
  Ok(())
}

#[derive(Deserialize)]
pub struct RegisterCallbackArgs {
  parameters: Vec<NativeType>,
  result: NativeType,
}

#[op(v8)]
pub fn op_ffi_unsafe_callback_create<FP, 'scope>(
  state: &mut deno_core::OpState,
  scope: &mut v8::HandleScope<'scope>,
  args: RegisterCallbackArgs,
  cb: serde_v8::Value<'scope>,
) -> Result<serde_v8::Value<'scope>, AnyError>
where
  FP: FfiPermissions + 'static,
{
  check_unstable(state, "Deno.UnsafeCallback");
  let permissions = state.borrow_mut::<FP>();
  permissions.check(None)?;

  let v8_value = cb.v8_value;
  let cb = v8::Local::<v8::Function>::try_from(v8_value)?;

  let isolate: *mut v8::Isolate = &mut *scope as &mut v8::Isolate;
  LOCAL_ISOLATE_POINTER.with(|s| {
    if s.borrow().is_null() {
      s.replace(isolate);
    }
  });

  let async_work_sender =
    state.borrow_mut::<FfiState>().async_work_sender.clone();
  let callback = v8::Global::new(scope, cb).into_raw();
  let current_context = scope.get_current_context();
  let context = v8::Global::new(scope, current_context).into_raw();

  let info: *mut CallbackInfo = Box::leak(Box::new(CallbackInfo {
    parameters: args.parameters.clone(),
    result: args.result.clone(),
    async_work_sender,
    callback,
    context,
    isolate,
    waker: None,
  }));
  let cif = Cif::new(
    args.parameters.into_iter().map(libffi::middle::Type::from),
    libffi::middle::Type::from(args.result),
  );

  // SAFETY: CallbackInfo is leaked, is not null and stays valid as long as the callback exists.
  let closure = libffi::middle::Closure::new(cif, deno_ffi_callback, unsafe {
    info.as_ref().unwrap()
  });
  let ptr = *closure.code_ptr() as usize;
  let resource = UnsafeCallbackResource {
    cancel: CancelHandle::new_rc(),
    closure,
    info,
  };
  let rid = state.resource_table.add(resource);

  let rid_local = v8::Integer::new_from_unsigned(scope, rid);
  let ptr_local: v8::Local<v8::Value> = if ptr > MAX_SAFE_INTEGER as usize {
    v8::BigInt::new_from_u64(scope, ptr as u64).into()
  } else {
    v8::Number::new(scope, ptr as f64).into()
  };
  let array = v8::Array::new(scope, 2);
  array.set_index(scope, 0, rid_local.into());
  array.set_index(scope, 1, ptr_local);
  let array_value: v8::Local<v8::Value> = array.into();

  Ok(array_value.into())
}
