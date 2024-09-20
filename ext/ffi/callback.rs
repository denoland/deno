// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use crate::symbol::NativeType;
use crate::FfiPermissions;
use crate::ForeignFunction;
use deno_core::error::AnyError;
use deno_core::op2;
use deno_core::v8;
use deno_core::v8::TryCatch;
use deno_core::CancelFuture;
use deno_core::CancelHandle;
use deno_core::OpState;
use deno_core::Resource;
use deno_core::ResourceId;
use deno_core::V8CrossThreadTaskSpawner;
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
use std::sync::atomic;
use std::sync::atomic::AtomicU32;
use std::task::Poll;

static THREAD_ID_COUNTER: AtomicU32 = AtomicU32::new(1);

thread_local! {
  static LOCAL_THREAD_ID: RefCell<u32> = const { RefCell::new(0) };
}

#[derive(Clone)]
pub struct PtrSymbol {
  pub cif: libffi::middle::Cif,
  pub ptr: libffi::middle::CodePtr,
}

impl PtrSymbol {
  pub fn new(
    fn_ptr: *mut c_void,
    def: &ForeignFunction,
  ) -> Result<Self, AnyError> {
    let ptr = libffi::middle::CodePtr::from_ptr(fn_ptr as _);
    let cif = libffi::middle::Cif::new(
      def
        .parameters
        .clone()
        .into_iter()
        .map(libffi::middle::Type::try_from)
        .collect::<Result<Vec<_>, _>>()?,
      def.result.clone().try_into()?,
    );

    Ok(Self { cif, ptr })
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
  }
}

struct CallbackInfo {
  pub async_work_sender: V8CrossThreadTaskSpawner,
  pub callback: NonNull<v8::Function>,
  pub context: NonNull<v8::Context>,
  pub parameters: Box<[NativeType]>,
  pub result: NativeType,
  pub thread_id: u32,
}

impl Future for CallbackInfo {
  type Output = ();
  fn poll(
    self: Pin<&mut Self>,
    _cx: &mut std::task::Context<'_>,
  ) -> std::task::Poll<Self::Output> {
    // The future for the CallbackInfo never resolves: It can only be canceled.
    Poll::Pending
  }
}

struct TaskArgs {
  cif: NonNull<libffi::low::ffi_cif>,
  result: NonNull<c_void>,
  args: *const *const c_void,
  info: NonNull<CallbackInfo>,
}

// SAFETY: we know these are valid Send-safe pointers as they are for FFI
unsafe impl Send for TaskArgs {}

impl TaskArgs {
  fn run(&mut self, scope: &mut v8::HandleScope) {
    // SAFETY: making a call using Send-safe pointers turned back into references. We know the
    // lifetime of these will last because we block on the result of the spawn call.
    unsafe {
      do_ffi_callback(
        scope,
        self.cif.as_ref(),
        self.info.as_ref(),
        self.result.as_mut(),
        self.args,
      )
    }
  }
}

unsafe extern "C" fn deno_ffi_callback(
  cif: &libffi::low::ffi_cif,
  result: &mut c_void,
  args: *const *const c_void,
  info: &CallbackInfo,
) {
  LOCAL_THREAD_ID.with(|s| {
    if *s.borrow() == info.thread_id {
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
      let context: NonNull<v8::Context> = info.context;
      let context = std::mem::transmute::<
        NonNull<v8::Context>,
        v8::Local<v8::Context>,
      >(context);
      let mut cb_scope = v8::CallbackScope::new(context);
      let scope = &mut v8::HandleScope::new(&mut cb_scope);

      do_ffi_callback(scope, cif, info, result, args);
    } else {
      let async_work_sender = &info.async_work_sender;

      let mut args = TaskArgs {
        cif: NonNull::from(cif),
        result: NonNull::from(result),
        args,
        info: NonNull::from(info),
      };

      async_work_sender.spawn_blocking(move |scope| {
        // We don't have a lot of choice here, so just print an unhandled exception message
        let tc_scope = &mut TryCatch::new(scope);
        args.run(tc_scope);
        if tc_scope.exception().is_some() {
          log::error!("Illegal unhandled exception in nonblocking callback");
        }
      });
    }
  });
}

unsafe fn do_ffi_callback(
  scope: &mut v8::HandleScope,
  cif: &libffi::low::ffi_cif,
  info: &CallbackInfo,
  result: &mut c_void,
  args: *const *const c_void,
) {
  let callback: NonNull<v8::Function> = info.callback;
  let func = std::mem::transmute::<
    NonNull<v8::Function>,
    v8::Local<v8::Function>,
  >(callback);
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
        v8::BigInt::new_from_i64(scope, result).into()
      }
      NativeType::U64 | NativeType::USize => {
        let result = *((*val) as *const u64);
        v8::BigInt::new_from_u64(scope, result).into()
      }
      NativeType::Pointer | NativeType::Buffer | NativeType::Function => {
        let result = *((*val) as *const *mut c_void);
        if result.is_null() {
          v8::null(scope).into()
        } else {
          v8::External::new(scope, result).into()
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
    NativeType::Buffer => {
      let pointer: *mut u8 = if let Ok(value) =
        v8::Local::<v8::ArrayBufferView>::try_from(value)
      {
        let byte_offset = value.byte_offset();
        let pointer = value
          .buffer(scope)
          .expect("Unable to deserialize result parameter.")
          .data();
        if let Some(non_null) = pointer {
          // SAFETY: Pointer is non-null, and V8 guarantees that the byte_offset
          // is within the buffer backing store.
          unsafe { non_null.as_ptr().add(byte_offset) as *mut u8 }
        } else {
          ptr::null_mut()
        }
      } else if let Ok(value) = v8::Local::<v8::ArrayBuffer>::try_from(value) {
        let pointer = value.data();
        if let Some(non_null) = pointer {
          non_null.as_ptr() as *mut u8
        } else {
          ptr::null_mut()
        }
      } else {
        ptr::null_mut()
      };
      *(result as *mut *mut u8) = pointer;
    }
    NativeType::Pointer | NativeType::Function => {
      let pointer: *mut c_void =
        if let Ok(external) = v8::Local::<v8::External>::try_from(value) {
          external.value()
        } else {
          // TODO(@aapoalas): Start throwing errors into JS about invalid callback return values.
          ptr::null_mut()
        };
      *(result as *mut *mut c_void) = pointer;
    }
    NativeType::I8 => {
      let value = if let Ok(value) = v8::Local::<v8::Int32>::try_from(value) {
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
      let value = if let Ok(value) = v8::Local::<v8::Uint32>::try_from(value) {
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
      let value = if let Ok(value) = v8::Local::<v8::Int32>::try_from(value) {
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
      let value = if let Ok(value) = v8::Local::<v8::Uint32>::try_from(value) {
        value.value() as u16
      } else {
        // Fallthrough, essentially UB.
        value
          .uint32_value(scope)
          .expect("Unable to deserialize result parameter.") as u16
      };
      *(result as *mut u16) = value;
    }
    NativeType::I32 => {
      let value = if let Ok(value) = v8::Local::<v8::Int32>::try_from(value) {
        value.value()
      } else {
        // Fallthrough, essentially UB.
        value
          .int32_value(scope)
          .expect("Unable to deserialize result parameter.")
      };
      *(result as *mut i32) = value;
    }
    NativeType::U32 => {
      let value = if let Ok(value) = v8::Local::<v8::Uint32>::try_from(value) {
        value.value()
      } else {
        // Fallthrough, essentially UB.
        value
          .uint32_value(scope)
          .expect("Unable to deserialize result parameter.")
      };
      *(result as *mut u32) = value;
    }
    NativeType::I64 | NativeType::ISize => {
      if let Ok(value) = v8::Local::<v8::BigInt>::try_from(value) {
        *(result as *mut i64) = value.i64_value().0;
      } else if let Ok(value) = v8::Local::<v8::Int32>::try_from(value) {
        *(result as *mut i64) = value.value() as i64;
      } else if let Ok(value) = v8::Local::<v8::Number>::try_from(value) {
        *(result as *mut i64) = value.value() as i64;
      } else {
        *(result as *mut i64) = value
          .integer_value(scope)
          .expect("Unable to deserialize result parameter.");
      }
    }
    NativeType::U64 | NativeType::USize => {
      if let Ok(value) = v8::Local::<v8::BigInt>::try_from(value) {
        *(result as *mut u64) = value.u64_value().0;
      } else if let Ok(value) = v8::Local::<v8::Uint32>::try_from(value) {
        *(result as *mut u64) = value.value() as u64;
      } else if let Ok(value) = v8::Local::<v8::Number>::try_from(value) {
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
  };
}

#[op2(async)]
pub fn op_ffi_unsafe_callback_ref(
  state: Rc<RefCell<OpState>>,
  #[smi] rid: ResourceId,
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

#[derive(Deserialize)]
pub struct RegisterCallbackArgs {
  parameters: Vec<NativeType>,
  result: NativeType,
}

#[op2]
pub fn op_ffi_unsafe_callback_create<FP, 'scope>(
  state: &mut OpState,
  scope: &mut v8::HandleScope<'scope>,
  #[serde] args: RegisterCallbackArgs,
  cb: v8::Local<v8::Function>,
) -> Result<v8::Local<'scope, v8::Value>, AnyError>
where
  FP: FfiPermissions + 'static,
{
  let permissions = state.borrow_mut::<FP>();
  permissions.check_partial_no_path()?;

  let thread_id: u32 = LOCAL_THREAD_ID.with(|s| {
    let value = *s.borrow();
    if value == 0 {
      let res = THREAD_ID_COUNTER.fetch_add(1, atomic::Ordering::SeqCst);
      s.replace(res);
      res
    } else {
      value
    }
  });

  if thread_id == 0 {
    panic!("Isolate ID counter overflowed u32");
  }

  let async_work_sender = state.borrow::<V8CrossThreadTaskSpawner>().clone();

  let callback = v8::Global::new(scope, cb).into_raw();
  let current_context = scope.get_current_context();
  let context = v8::Global::new(scope, current_context).into_raw();

  let info: *mut CallbackInfo = Box::leak(Box::new(CallbackInfo {
    async_work_sender,
    callback,
    context,
    parameters: args.parameters.clone().into(),
    result: args.result.clone(),
    thread_id,
  }));
  let cif = Cif::new(
    args
      .parameters
      .into_iter()
      .map(libffi::middle::Type::try_from)
      .collect::<Result<Vec<_>, _>>()?,
    libffi::middle::Type::try_from(args.result)?,
  );

  // SAFETY: CallbackInfo is leaked, is not null and stays valid as long as the callback exists.
  let closure = libffi::middle::Closure::new(cif, deno_ffi_callback, unsafe {
    info.as_ref().unwrap()
  });
  let ptr = *closure.code_ptr() as *mut c_void;
  let resource = UnsafeCallbackResource {
    cancel: CancelHandle::new_rc(),
    closure,
    info,
  };
  let rid = state.resource_table.add(resource);

  let rid_local = v8::Integer::new_from_unsigned(scope, rid);
  let ptr_local: v8::Local<v8::Value> = v8::External::new(scope, ptr).into();
  let array = v8::Array::new(scope, 2);
  array.set_index(scope, 0, rid_local.into());
  array.set_index(scope, 1, ptr_local);
  let array_value: v8::Local<v8::Value> = array.into();

  Ok(array_value)
}

#[op2(fast)]
pub fn op_ffi_unsafe_callback_close(
  state: &mut OpState,
  scope: &mut v8::HandleScope,
  #[smi] rid: ResourceId,
) -> Result<(), AnyError> {
  // SAFETY: This drops the closure and the callback info associated with it.
  // Any retained function pointers to the closure become dangling pointers.
  // It is up to the user to know that it is safe to call the `close()` on the
  // UnsafeCallback instance.
  unsafe {
    let callback_resource =
      state.resource_table.take::<UnsafeCallbackResource>(rid)?;
    let info = Box::from_raw(callback_resource.info);
    let _ = v8::Global::from_raw(scope, info.callback);
    let _ = v8::Global::from_raw(scope, info.context);
    callback_resource.close();
  }
  Ok(())
}
