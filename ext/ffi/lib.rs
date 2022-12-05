// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use core::ptr::NonNull;
use deno_core::anyhow::anyhow;
use deno_core::error::type_error;
use deno_core::error::AnyError;
use deno_core::futures::channel::mpsc;
use deno_core::futures::Future;
use deno_core::include_js_files;
use deno_core::op;
use deno_core::serde_json::Value;
use deno_core::serde_v8;
use deno_core::v8;
use deno_core::CancelFuture;
use deno_core::CancelHandle;
use deno_core::Extension;
use deno_core::OpState;
use deno_core::Resource;
use deno_core::ResourceId;

use libffi::middle::Arg;
use libffi::middle::Cif;
use serde::Deserialize;
use std::borrow::Cow;
use std::cell::RefCell;
use std::ffi::c_void;
use std::future::IntoFuture;
use std::mem::size_of;
use std::os::raw::c_char;
use std::os::raw::c_short;
use std::path::Path;
use std::pin::Pin;
use std::ptr;
use std::rc::Rc;
use std::sync::mpsc::sync_channel;
use std::task::Poll;
use std::task::Waker;

mod dlfcn;
mod turbocall;
mod repr;
mod symbol;

use dlfcn::{op_ffi_load, DynamicLibraryResource, ForeignFunction};
use repr::*;
use symbol::{NativeType, Symbol};

#[cfg(not(target_pointer_width = "64"))]
compile_error!("platform not supported");

const _: () = {
  assert!(size_of::<c_char>() == 1);
  assert!(size_of::<c_short>() == 2);
  assert!(size_of::<*const ()>() == 8);
};

thread_local! {
  static LOCAL_ISOLATE_POINTER: RefCell<*const v8::Isolate> = RefCell::new(ptr::null());
}

const MAX_SAFE_INTEGER: isize = 9007199254740991;
const MIN_SAFE_INTEGER: isize = -9007199254740991;

pub struct Unstable(pub bool);

fn check_unstable(state: &OpState, api_name: &str) {
  let unstable = state.borrow::<Unstable>();

  if !unstable.0 {
    eprintln!(
      "Unstable API '{}'. The --unstable flag must be provided.",
      api_name
    );
    std::process::exit(70);
  }
}

pub fn check_unstable2(state: &Rc<RefCell<OpState>>, api_name: &str) {
  let state = state.borrow();
  check_unstable(&state, api_name)
}

pub trait FfiPermissions {
  fn check(&mut self, path: Option<&Path>) -> Result<(), AnyError>;
}

#[derive(Clone)]
struct PtrSymbol {
  cif: libffi::middle::Cif,
  ptr: libffi::middle::CodePtr,
}

impl PtrSymbol {
  fn new(fn_ptr: usize, def: &ForeignFunction) -> Self {
    let ptr = libffi::middle::CodePtr::from_ptr(fn_ptr as _);
    let cif = libffi::middle::Cif::new(
      def
        .parameters
        .clone()
        .into_iter()
        .map(libffi::middle::Type::from),
      def.result.into(),
    );

    Self { cif, ptr }
  }
}

#[allow(clippy::non_send_fields_in_send_ty)]
// SAFETY: unsafe trait must have unsafe implementation
unsafe impl Send for PtrSymbol {}
// SAFETY: unsafe trait must have unsafe implementation
unsafe impl Sync for PtrSymbol {}

type PendingFfiAsyncWork = Box<dyn FnOnce()>;

struct FfiState {
  async_work_sender: mpsc::UnboundedSender<PendingFfiAsyncWork>,
  async_work_receiver: mpsc::UnboundedReceiver<PendingFfiAsyncWork>,
}

pub fn init<P: FfiPermissions + 'static>(unstable: bool) -> Extension {
  Extension::builder()
    .js(include_js_files!(
      prefix "deno:ext/ffi",
      "00_ffi.js",
    ))
    .ops(vec![
      op_ffi_load::decl::<P>(),
      op_ffi_get_static::decl(),
      op_ffi_call_nonblocking::decl(),
      op_ffi_call_ptr::decl::<P>(),
      op_ffi_call_ptr_nonblocking::decl::<P>(),
      op_ffi_ptr_of::decl::<P>(),
      op_ffi_get_buf::decl::<P>(),
      op_ffi_buf_copy_into::decl::<P>(),
      op_ffi_cstr_read::decl::<P>(),
      op_ffi_read_bool::decl::<P>(),
      op_ffi_read_u8::decl::<P>(),
      op_ffi_read_i8::decl::<P>(),
      op_ffi_read_u16::decl::<P>(),
      op_ffi_read_i16::decl::<P>(),
      op_ffi_read_u32::decl::<P>(),
      op_ffi_read_i32::decl::<P>(),
      op_ffi_read_u64::decl::<P>(),
      op_ffi_read_i64::decl::<P>(),
      op_ffi_read_f32::decl::<P>(),
      op_ffi_read_f64::decl::<P>(),
      op_ffi_unsafe_callback_create::decl::<P>(),
      op_ffi_unsafe_callback_ref::decl(),
      op_ffi_unsafe_callback_unref::decl(),
    ])
    .event_loop_middleware(|op_state_rc, _cx| {
      // FFI callbacks coming in from other threads will call in and get queued.
      let mut maybe_scheduling = false;

      let mut work_items: Vec<PendingFfiAsyncWork> = vec![];

      {
        let mut op_state = op_state_rc.borrow_mut();
        let ffi_state = op_state.borrow_mut::<FfiState>();

        while let Ok(Some(async_work_fut)) =
          ffi_state.async_work_receiver.try_next()
        {
          // Move received items to a temporary vector so that we can drop the `op_state` borrow before we do the work.
          work_items.push(async_work_fut);
          maybe_scheduling = true;
        }

        drop(op_state);
      }
      while let Some(async_work_fut) = work_items.pop() {
        async_work_fut();
      }

      maybe_scheduling
    })
    .state(move |state| {
      // Stolen from deno_webgpu, is there a better option?
      state.put(Unstable(unstable));

      let (async_work_sender, async_work_receiver) =
        mpsc::unbounded::<PendingFfiAsyncWork>();

      state.put(FfiState {
        async_work_receiver,
        async_work_sender,
      });

      Ok(())
    })
    .build()
}

/// Intermediate format for easy translation from NativeType + V8 value
/// to libffi argument types.
#[repr(C)]
union NativeValue {
  void_value: (),
  bool_value: bool,
  u8_value: u8,
  i8_value: i8,
  u16_value: u16,
  i16_value: i16,
  u32_value: u32,
  i32_value: i32,
  u64_value: u64,
  i64_value: i64,
  usize_value: usize,
  isize_value: isize,
  f32_value: f32,
  f64_value: f64,
  pointer: *mut c_void,
}

impl NativeValue {
  unsafe fn as_arg(&self, native_type: NativeType) -> Arg {
    match native_type {
      NativeType::Void => unreachable!(),
      NativeType::Bool => Arg::new(&self.bool_value),
      NativeType::U8 => Arg::new(&self.u8_value),
      NativeType::I8 => Arg::new(&self.i8_value),
      NativeType::U16 => Arg::new(&self.u16_value),
      NativeType::I16 => Arg::new(&self.i16_value),
      NativeType::U32 => Arg::new(&self.u32_value),
      NativeType::I32 => Arg::new(&self.i32_value),
      NativeType::U64 => Arg::new(&self.u64_value),
      NativeType::I64 => Arg::new(&self.i64_value),
      NativeType::USize => Arg::new(&self.usize_value),
      NativeType::ISize => Arg::new(&self.isize_value),
      NativeType::F32 => Arg::new(&self.f32_value),
      NativeType::F64 => Arg::new(&self.f64_value),
      NativeType::Pointer | NativeType::Buffer | NativeType::Function => {
        Arg::new(&self.pointer)
      }
    }
  }

  // SAFETY: native_type must correspond to the type of value represented by the union field
  unsafe fn to_value(&self, native_type: NativeType) -> Value {
    match native_type {
      NativeType::Void => Value::Null,
      NativeType::Bool => Value::from(self.bool_value),
      NativeType::U8 => Value::from(self.u8_value),
      NativeType::I8 => Value::from(self.i8_value),
      NativeType::U16 => Value::from(self.u16_value),
      NativeType::I16 => Value::from(self.i16_value),
      NativeType::U32 => Value::from(self.u32_value),
      NativeType::I32 => Value::from(self.i32_value),
      NativeType::U64 => Value::from(self.u64_value),
      NativeType::I64 => Value::from(self.i64_value),
      NativeType::USize => Value::from(self.usize_value),
      NativeType::ISize => Value::from(self.isize_value),
      NativeType::F32 => Value::from(self.f32_value),
      NativeType::F64 => Value::from(self.f64_value),
      NativeType::Pointer | NativeType::Function | NativeType::Buffer => {
        Value::from(self.pointer as usize)
      }
    }
  }

  // SAFETY: native_type must correspond to the type of value represented by the union field
  #[inline]
  unsafe fn to_v8<'scope>(
    &self,
    scope: &mut v8::HandleScope<'scope>,
    native_type: NativeType,
  ) -> serde_v8::Value<'scope> {
    match native_type {
      NativeType::Void => {
        let local_value: v8::Local<v8::Value> = v8::undefined(scope).into();
        local_value.into()
      }
      NativeType::Bool => {
        let local_value: v8::Local<v8::Value> =
          v8::Boolean::new(scope, self.bool_value).into();
        local_value.into()
      }
      NativeType::U8 => {
        let local_value: v8::Local<v8::Value> =
          v8::Integer::new_from_unsigned(scope, self.u8_value as u32).into();
        local_value.into()
      }
      NativeType::I8 => {
        let local_value: v8::Local<v8::Value> =
          v8::Integer::new(scope, self.i8_value as i32).into();
        local_value.into()
      }
      NativeType::U16 => {
        let local_value: v8::Local<v8::Value> =
          v8::Integer::new_from_unsigned(scope, self.u16_value as u32).into();
        local_value.into()
      }
      NativeType::I16 => {
        let local_value: v8::Local<v8::Value> =
          v8::Integer::new(scope, self.i16_value as i32).into();
        local_value.into()
      }
      NativeType::U32 => {
        let local_value: v8::Local<v8::Value> =
          v8::Integer::new_from_unsigned(scope, self.u32_value).into();
        local_value.into()
      }
      NativeType::I32 => {
        let local_value: v8::Local<v8::Value> =
          v8::Integer::new(scope, self.i32_value).into();
        local_value.into()
      }
      NativeType::U64 => {
        let value = self.u64_value;
        let local_value: v8::Local<v8::Value> =
          if value > MAX_SAFE_INTEGER as u64 {
            v8::BigInt::new_from_u64(scope, value).into()
          } else {
            v8::Number::new(scope, value as f64).into()
          };
        local_value.into()
      }
      NativeType::I64 => {
        let value = self.i64_value;
        let local_value: v8::Local<v8::Value> =
          if value > MAX_SAFE_INTEGER as i64 || value < MIN_SAFE_INTEGER as i64
          {
            v8::BigInt::new_from_i64(scope, self.i64_value).into()
          } else {
            v8::Number::new(scope, value as f64).into()
          };
        local_value.into()
      }
      NativeType::USize => {
        let value = self.usize_value;
        let local_value: v8::Local<v8::Value> =
          if value > MAX_SAFE_INTEGER as usize {
            v8::BigInt::new_from_u64(scope, value as u64).into()
          } else {
            v8::Number::new(scope, value as f64).into()
          };
        local_value.into()
      }
      NativeType::ISize => {
        let value = self.isize_value;
        let local_value: v8::Local<v8::Value> =
          if !(MIN_SAFE_INTEGER..=MAX_SAFE_INTEGER).contains(&value) {
            v8::BigInt::new_from_i64(scope, self.isize_value as i64).into()
          } else {
            v8::Number::new(scope, value as f64).into()
          };
        local_value.into()
      }
      NativeType::F32 => {
        let local_value: v8::Local<v8::Value> =
          v8::Number::new(scope, self.f32_value as f64).into();
        local_value.into()
      }
      NativeType::F64 => {
        let local_value: v8::Local<v8::Value> =
          v8::Number::new(scope, self.f64_value).into();
        local_value.into()
      }
      NativeType::Pointer | NativeType::Buffer | NativeType::Function => {
        let value = self.pointer as u64;
        let local_value: v8::Local<v8::Value> =
          if value > MAX_SAFE_INTEGER as u64 {
            v8::BigInt::new_from_u64(scope, value).into()
          } else {
            v8::Number::new(scope, value as f64).into()
          };
        local_value.into()
      }
    }
  }
}

// SAFETY: unsafe trait must have unsafe implementation
unsafe impl Send for NativeValue {}

#[inline]
fn ffi_parse_bool_arg(
  arg: v8::Local<v8::Value>,
) -> Result<NativeValue, AnyError> {
  let bool_value = v8::Local::<v8::Boolean>::try_from(arg)
    .map_err(|_| type_error("Invalid FFI u8 type, expected boolean"))?
    .is_true();
  Ok(NativeValue { bool_value })
}

#[inline]
fn ffi_parse_u8_arg(
  arg: v8::Local<v8::Value>,
) -> Result<NativeValue, AnyError> {
  let u8_value = v8::Local::<v8::Uint32>::try_from(arg)
    .map_err(|_| type_error("Invalid FFI u8 type, expected unsigned integer"))?
    .value() as u8;
  Ok(NativeValue { u8_value })
}

#[inline]
fn ffi_parse_i8_arg(
  arg: v8::Local<v8::Value>,
) -> Result<NativeValue, AnyError> {
  let i8_value = v8::Local::<v8::Int32>::try_from(arg)
    .map_err(|_| type_error("Invalid FFI i8 type, expected integer"))?
    .value() as i8;
  Ok(NativeValue { i8_value })
}

#[inline]
fn ffi_parse_u16_arg(
  arg: v8::Local<v8::Value>,
) -> Result<NativeValue, AnyError> {
  let u16_value = v8::Local::<v8::Uint32>::try_from(arg)
    .map_err(|_| type_error("Invalid FFI u16 type, expected unsigned integer"))?
    .value() as u16;
  Ok(NativeValue { u16_value })
}

#[inline]
fn ffi_parse_i16_arg(
  arg: v8::Local<v8::Value>,
) -> Result<NativeValue, AnyError> {
  let i16_value = v8::Local::<v8::Int32>::try_from(arg)
    .map_err(|_| type_error("Invalid FFI i16 type, expected integer"))?
    .value() as i16;
  Ok(NativeValue { i16_value })
}

#[inline]
fn ffi_parse_u32_arg(
  arg: v8::Local<v8::Value>,
) -> Result<NativeValue, AnyError> {
  let u32_value = v8::Local::<v8::Uint32>::try_from(arg)
    .map_err(|_| type_error("Invalid FFI u32 type, expected unsigned integer"))?
    .value() as u32;
  Ok(NativeValue { u32_value })
}

#[inline]
fn ffi_parse_i32_arg(
  arg: v8::Local<v8::Value>,
) -> Result<NativeValue, AnyError> {
  let i32_value = v8::Local::<v8::Int32>::try_from(arg)
    .map_err(|_| type_error("Invalid FFI i32 type, expected integer"))?
    .value() as i32;
  Ok(NativeValue { i32_value })
}

#[inline]
fn ffi_parse_u64_arg(
  scope: &mut v8::HandleScope,
  arg: v8::Local<v8::Value>,
) -> Result<NativeValue, AnyError> {
  // Order of checking:
  // 1. BigInt: Uncommon and not supported by Fast API, so optimise slow call for this case.
  // 2. Number: Common, supported by Fast API, so let that be the optimal case.
  let u64_value: u64 = if let Ok(value) = v8::Local::<v8::BigInt>::try_from(arg)
  {
    value.u64_value().0
  } else if let Ok(value) = v8::Local::<v8::Number>::try_from(arg) {
    value.integer_value(scope).unwrap() as u64
  } else {
    return Err(type_error(
      "Invalid FFI u64 type, expected unsigned integer",
    ));
  };
  Ok(NativeValue { u64_value })
}

#[inline]
fn ffi_parse_i64_arg(
  scope: &mut v8::HandleScope,
  arg: v8::Local<v8::Value>,
) -> Result<NativeValue, AnyError> {
  // Order of checking:
  // 1. BigInt: Uncommon and not supported by Fast API, so optimise slow call for this case.
  // 2. Number: Common, supported by Fast API, so let that be the optimal case.
  let i64_value: i64 = if let Ok(value) = v8::Local::<v8::BigInt>::try_from(arg)
  {
    value.i64_value().0
  } else if let Ok(value) = v8::Local::<v8::Number>::try_from(arg) {
    value.integer_value(scope).unwrap() as i64
  } else {
    return Err(type_error("Invalid FFI i64 type, expected integer"));
  };
  Ok(NativeValue { i64_value })
}

#[inline]
fn ffi_parse_usize_arg(
  scope: &mut v8::HandleScope,
  arg: v8::Local<v8::Value>,
) -> Result<NativeValue, AnyError> {
  // Order of checking:
  // 1. BigInt: Uncommon and not supported by Fast API, so optimise slow call for this case.
  // 2. Number: Common, supported by Fast API, so let that be the optimal case.
  let usize_value: usize =
    if let Ok(value) = v8::Local::<v8::BigInt>::try_from(arg) {
      value.u64_value().0 as usize
    } else if let Ok(value) = v8::Local::<v8::Number>::try_from(arg) {
      value.integer_value(scope).unwrap() as usize
    } else {
      return Err(type_error("Invalid FFI usize type, expected integer"));
    };
  Ok(NativeValue { usize_value })
}

#[inline]
fn ffi_parse_isize_arg(
  scope: &mut v8::HandleScope,
  arg: v8::Local<v8::Value>,
) -> Result<NativeValue, AnyError> {
  // Order of checking:
  // 1. BigInt: Uncommon and not supported by Fast API, so optimise slow call for this case.
  // 2. Number: Common, supported by Fast API, so let that be the optimal case.
  let isize_value: isize =
    if let Ok(value) = v8::Local::<v8::BigInt>::try_from(arg) {
      value.i64_value().0 as isize
    } else if let Ok(value) = v8::Local::<v8::Number>::try_from(arg) {
      value.integer_value(scope).unwrap() as isize
    } else {
      return Err(type_error("Invalid FFI isize type, expected integer"));
    };
  Ok(NativeValue { isize_value })
}

#[inline]
fn ffi_parse_f32_arg(
  arg: v8::Local<v8::Value>,
) -> Result<NativeValue, AnyError> {
  let f32_value = v8::Local::<v8::Number>::try_from(arg)
    .map_err(|_| type_error("Invalid FFI f32 type, expected number"))?
    .value() as f32;
  Ok(NativeValue { f32_value })
}

#[inline]
fn ffi_parse_f64_arg(
  arg: v8::Local<v8::Value>,
) -> Result<NativeValue, AnyError> {
  let f64_value = v8::Local::<v8::Number>::try_from(arg)
    .map_err(|_| type_error("Invalid FFI f64 type, expected number"))?
    .value() as f64;
  Ok(NativeValue { f64_value })
}

#[inline]
fn ffi_parse_pointer_arg(
  scope: &mut v8::HandleScope,
  arg: v8::Local<v8::Value>,
) -> Result<NativeValue, AnyError> {
  // Order of checking:
  // 1. BigInt: Uncommon and not supported by Fast API, optimise this case.
  // 2. Number: Common and supported by Fast API.
  // 3. Null: Very uncommon / can be represented by a 0.
  let pointer = if let Ok(value) = v8::Local::<v8::BigInt>::try_from(arg) {
    value.u64_value().0 as usize as *mut c_void
  } else if let Ok(value) = v8::Local::<v8::Number>::try_from(arg) {
    value.integer_value(scope).unwrap() as usize as *mut c_void
  } else if arg.is_null() {
    ptr::null_mut()
  } else {
    return Err(type_error(
      "Invalid FFI pointer type, expected null, integer or BigInt",
    ));
  };
  Ok(NativeValue { pointer })
}

#[inline]
fn ffi_parse_buffer_arg(
  scope: &mut v8::HandleScope,
  arg: v8::Local<v8::Value>,
) -> Result<NativeValue, AnyError> {
  // Order of checking:
  // 1. ArrayBuffer: Fairly common and not supported by Fast API, optimise this case.
  // 2. ArrayBufferView: Common and supported by Fast API
  // 5. Null: Very uncommon / can be represented by a 0.

  let pointer = if let Ok(value) = v8::Local::<v8::ArrayBuffer>::try_from(arg) {
    if let Some(non_null) = value.data() {
      non_null.as_ptr()
    } else {
      ptr::null_mut()
    }
  } else if let Ok(value) = v8::Local::<v8::ArrayBufferView>::try_from(arg) {
    let byte_offset = value.byte_offset();
    let pointer = value
      .buffer(scope)
      .ok_or_else(|| {
        type_error("Invalid FFI ArrayBufferView, expected data in the buffer")
      })?
      .data();
    if let Some(non_null) = pointer {
      // SAFETY: Pointer is non-null, and V8 guarantees that the byte_offset
      // is within the buffer backing store.
      unsafe { non_null.as_ptr().add(byte_offset) }
    } else {
      ptr::null_mut()
    }
  } else if arg.is_null() {
    ptr::null_mut()
  } else {
    return Err(type_error(
      "Invalid FFI buffer type, expected null, ArrayBuffer, or ArrayBufferView",
    ));
  };
  Ok(NativeValue { pointer })
}

#[inline]
fn ffi_parse_function_arg(
  scope: &mut v8::HandleScope,
  arg: v8::Local<v8::Value>,
) -> Result<NativeValue, AnyError> {
  // Order of checking:
  // 1. BigInt: Uncommon and not supported by Fast API, optimise this case.
  // 2. Number: Common and supported by Fast API, optimise this case as second.
  // 3. Null: Very uncommon / can be represented by a 0.
  let pointer = if let Ok(value) = v8::Local::<v8::BigInt>::try_from(arg) {
    value.u64_value().0 as usize as *mut c_void
  } else if let Ok(value) = v8::Local::<v8::Number>::try_from(arg) {
    value.integer_value(scope).unwrap() as usize as *mut c_void
  } else if arg.is_null() {
    ptr::null_mut()
  } else {
    return Err(type_error(
      "Invalid FFI function type, expected null, integer, or BigInt",
    ));
  };
  Ok(NativeValue { pointer })
}

fn ffi_parse_args<'scope>(
  scope: &mut v8::HandleScope<'scope>,
  args: serde_v8::Value<'scope>,
  parameter_types: &[NativeType],
) -> Result<Vec<NativeValue>, AnyError>
where
  'scope: 'scope,
{
  if parameter_types.is_empty() {
    return Ok(vec![]);
  }

  let args = v8::Local::<v8::Array>::try_from(args.v8_value)
    .map_err(|_| type_error("Invalid FFI parameters, expected Array"))?;
  let mut ffi_args: Vec<NativeValue> =
    Vec::with_capacity(parameter_types.len());

  for (index, native_type) in parameter_types.iter().enumerate() {
    let value = args.get_index(scope, index as u32).unwrap();
    match native_type {
      NativeType::Bool => {
        ffi_args.push(ffi_parse_bool_arg(value)?);
      }
      NativeType::U8 => {
        ffi_args.push(ffi_parse_u8_arg(value)?);
      }
      NativeType::I8 => {
        ffi_args.push(ffi_parse_i8_arg(value)?);
      }
      NativeType::U16 => {
        ffi_args.push(ffi_parse_u16_arg(value)?);
      }
      NativeType::I16 => {
        ffi_args.push(ffi_parse_i16_arg(value)?);
      }
      NativeType::U32 => {
        ffi_args.push(ffi_parse_u32_arg(value)?);
      }
      NativeType::I32 => {
        ffi_args.push(ffi_parse_i32_arg(value)?);
      }
      NativeType::U64 => {
        ffi_args.push(ffi_parse_u64_arg(scope, value)?);
      }
      NativeType::I64 => {
        ffi_args.push(ffi_parse_i64_arg(scope, value)?);
      }
      NativeType::USize => {
        ffi_args.push(ffi_parse_usize_arg(scope, value)?);
      }
      NativeType::ISize => {
        ffi_args.push(ffi_parse_isize_arg(scope, value)?);
      }
      NativeType::F32 => {
        ffi_args.push(ffi_parse_f32_arg(value)?);
      }
      NativeType::F64 => {
        ffi_args.push(ffi_parse_f64_arg(value)?);
      }
      NativeType::Buffer => {
        ffi_args.push(ffi_parse_buffer_arg(scope, value)?);
      }
      NativeType::Pointer => {
        ffi_args.push(ffi_parse_pointer_arg(scope, value)?);
      }
      NativeType::Function => {
        ffi_args.push(ffi_parse_function_arg(scope, value)?);
      }
      NativeType::Void => {
        unreachable!();
      }
    }
  }

  Ok(ffi_args)
}

// A one-off synchronous FFI call.
fn ffi_call_sync<'scope>(
  scope: &mut v8::HandleScope<'scope>,
  args: v8::FunctionCallbackArguments,
  symbol: &Symbol,
) -> Result<NativeValue, AnyError>
where
  'scope: 'scope,
{
  let Symbol {
    parameter_types,
    result_type,
    cif,
    ptr: fun_ptr,
    ..
  } = symbol;
  let mut ffi_args: Vec<NativeValue> =
    Vec::with_capacity(parameter_types.len());

  for (index, native_type) in parameter_types.iter().enumerate() {
    let value = args.get(index as i32);
    match native_type {
      NativeType::Bool => {
        ffi_args.push(ffi_parse_bool_arg(value)?);
      }
      NativeType::U8 => {
        ffi_args.push(ffi_parse_u8_arg(value)?);
      }
      NativeType::I8 => {
        ffi_args.push(ffi_parse_i8_arg(value)?);
      }
      NativeType::U16 => {
        ffi_args.push(ffi_parse_u16_arg(value)?);
      }
      NativeType::I16 => {
        ffi_args.push(ffi_parse_i16_arg(value)?);
      }
      NativeType::U32 => {
        ffi_args.push(ffi_parse_u32_arg(value)?);
      }
      NativeType::I32 => {
        ffi_args.push(ffi_parse_i32_arg(value)?);
      }
      NativeType::U64 => {
        ffi_args.push(ffi_parse_u64_arg(scope, value)?);
      }
      NativeType::I64 => {
        ffi_args.push(ffi_parse_i64_arg(scope, value)?);
      }
      NativeType::USize => {
        ffi_args.push(ffi_parse_usize_arg(scope, value)?);
      }
      NativeType::ISize => {
        ffi_args.push(ffi_parse_isize_arg(scope, value)?);
      }
      NativeType::F32 => {
        ffi_args.push(ffi_parse_f32_arg(value)?);
      }
      NativeType::F64 => {
        ffi_args.push(ffi_parse_f64_arg(value)?);
      }
      NativeType::Buffer => {
        ffi_args.push(ffi_parse_buffer_arg(scope, value)?);
      }
      NativeType::Pointer => {
        ffi_args.push(ffi_parse_pointer_arg(scope, value)?);
      }
      NativeType::Function => {
        ffi_args.push(ffi_parse_function_arg(scope, value)?);
      }
      NativeType::Void => {
        unreachable!();
      }
    }
  }
  let call_args: Vec<Arg> = ffi_args.iter().map(Arg::new).collect();
  // SAFETY: types in the `Cif` match the actual calling convention and
  // types of symbol.
  unsafe {
    Ok(match result_type {
      NativeType::Void => NativeValue {
        void_value: cif.call::<()>(*fun_ptr, &call_args),
      },
      NativeType::Bool => NativeValue {
        bool_value: cif.call::<bool>(*fun_ptr, &call_args),
      },
      NativeType::U8 => NativeValue {
        u8_value: cif.call::<u8>(*fun_ptr, &call_args),
      },
      NativeType::I8 => NativeValue {
        i8_value: cif.call::<i8>(*fun_ptr, &call_args),
      },
      NativeType::U16 => NativeValue {
        u16_value: cif.call::<u16>(*fun_ptr, &call_args),
      },
      NativeType::I16 => NativeValue {
        i16_value: cif.call::<i16>(*fun_ptr, &call_args),
      },
      NativeType::U32 => NativeValue {
        u32_value: cif.call::<u32>(*fun_ptr, &call_args),
      },
      NativeType::I32 => NativeValue {
        i32_value: cif.call::<i32>(*fun_ptr, &call_args),
      },
      NativeType::U64 => NativeValue {
        u64_value: cif.call::<u64>(*fun_ptr, &call_args),
      },
      NativeType::I64 => NativeValue {
        i64_value: cif.call::<i64>(*fun_ptr, &call_args),
      },
      NativeType::USize => NativeValue {
        usize_value: cif.call::<usize>(*fun_ptr, &call_args),
      },
      NativeType::ISize => NativeValue {
        isize_value: cif.call::<isize>(*fun_ptr, &call_args),
      },
      NativeType::F32 => NativeValue {
        f32_value: cif.call::<f32>(*fun_ptr, &call_args),
      },
      NativeType::F64 => NativeValue {
        f64_value: cif.call::<f64>(*fun_ptr, &call_args),
      },
      NativeType::Pointer | NativeType::Function | NativeType::Buffer => {
        NativeValue {
          pointer: cif.call::<*mut c_void>(*fun_ptr, &call_args),
        }
      }
    })
  }
}

fn ffi_call(
  call_args: Vec<NativeValue>,
  cif: &libffi::middle::Cif,
  fun_ptr: libffi::middle::CodePtr,
  parameter_types: &[NativeType],
  result_type: NativeType,
) -> Result<NativeValue, AnyError> {
  let call_args: Vec<Arg> = call_args
    .iter()
    .enumerate()
    .map(|(index, ffi_arg)| {
      // SAFETY: the union field is initialized
      unsafe { ffi_arg.as_arg(*parameter_types.get(index).unwrap()) }
    })
    .collect();

  // SAFETY: types in the `Cif` match the actual calling convention and
  // types of symbol.
  unsafe {
    Ok(match result_type {
      NativeType::Void => NativeValue {
        void_value: cif.call::<()>(fun_ptr, &call_args),
      },
      NativeType::Bool => NativeValue {
        bool_value: cif.call::<bool>(fun_ptr, &call_args),
      },
      NativeType::U8 => NativeValue {
        u8_value: cif.call::<u8>(fun_ptr, &call_args),
      },
      NativeType::I8 => NativeValue {
        i8_value: cif.call::<i8>(fun_ptr, &call_args),
      },
      NativeType::U16 => NativeValue {
        u16_value: cif.call::<u16>(fun_ptr, &call_args),
      },
      NativeType::I16 => NativeValue {
        i16_value: cif.call::<i16>(fun_ptr, &call_args),
      },
      NativeType::U32 => NativeValue {
        u32_value: cif.call::<u32>(fun_ptr, &call_args),
      },
      NativeType::I32 => NativeValue {
        i32_value: cif.call::<i32>(fun_ptr, &call_args),
      },
      NativeType::U64 => NativeValue {
        u64_value: cif.call::<u64>(fun_ptr, &call_args),
      },
      NativeType::I64 => NativeValue {
        i64_value: cif.call::<i64>(fun_ptr, &call_args),
      },
      NativeType::USize => NativeValue {
        usize_value: cif.call::<usize>(fun_ptr, &call_args),
      },
      NativeType::ISize => NativeValue {
        isize_value: cif.call::<isize>(fun_ptr, &call_args),
      },
      NativeType::F32 => NativeValue {
        f32_value: cif.call::<f32>(fun_ptr, &call_args),
      },
      NativeType::F64 => NativeValue {
        f64_value: cif.call::<f64>(fun_ptr, &call_args),
      },
      NativeType::Pointer | NativeType::Function | NativeType::Buffer => {
        NativeValue {
          pointer: cif.call::<*mut c_void>(fun_ptr, &call_args),
        }
      }
    })
  }
}

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

unsafe extern "C" fn deno_ffi_callback(
  _cif: &libffi::low::ffi_cif,
  result: &mut c_void,
  args: *const *const c_void,
  info: &CallbackInfo,
) {
  LOCAL_ISOLATE_POINTER.with(|s| {
    if ptr::eq(*s.borrow(), info.isolate) {
      // Own isolate thread, okay to call directly
      do_ffi_callback(info, result, args);
    } else {
      let async_work_sender = &info.async_work_sender;
      // SAFETY: Safe as this function blocks until `do_ffi_callback` completes and a response message is received.
      let result: &'static mut c_void = std::mem::transmute(result);
      let info: &'static CallbackInfo = std::mem::transmute(info);
      let (response_sender, response_receiver) = sync_channel::<()>(0);
      let fut = Box::new(move || {
        do_ffi_callback(info, result, args);
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
    std::slice::from_raw_parts(args, info.parameters.len() as usize);

  let mut params: Vec<v8::Local<v8::Value>> = vec![];
  for (native_type, val) in info.parameters.iter().zip(vals) {
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
          .expect("Unable to deserialize result parameter.") as i32
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
          .expect("Unable to deserialize result parameter.")
          as i64;
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
    NativeType::Void => {
      // nop
    }
    _ => {
      unreachable!();
    }
  };
}

#[derive(Deserialize)]
struct RegisterCallbackArgs {
  parameters: Vec<NativeType>,
  result: NativeType,
}

#[op(v8)]
fn op_ffi_unsafe_callback_create<FP, 'scope>(
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
    result: args.result,
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

#[op(v8)]
fn op_ffi_call_ptr<FP, 'scope>(
  scope: &mut v8::HandleScope<'scope>,
  state: Rc<RefCell<deno_core::OpState>>,
  pointer: usize,
  def: ForeignFunction,
  parameters: serde_v8::Value<'scope>,
) -> Result<serde_v8::Value<'scope>, AnyError>
where
  FP: FfiPermissions + 'static,
{
  check_unstable2(&state, "Deno.UnsafeFnPointer#call");
  {
    let mut state = state.borrow_mut();
    let permissions = state.borrow_mut::<FP>();
    permissions.check(None)?;
  };

  let symbol = PtrSymbol::new(pointer, &def);
  let call_args = ffi_parse_args(scope, parameters, &def.parameters)?;

  let result = ffi_call(
    call_args,
    &symbol.cif,
    symbol.ptr,
    &def.parameters,
    def.result,
  )?;
  // SAFETY: Same return type declared to libffi; trust user to have it right beyond that.
  let result = unsafe { result.to_v8(scope, def.result) };
  Ok(result)
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

#[op]
fn op_ffi_unsafe_callback_ref(
  state: &mut deno_core::OpState,
  rid: ResourceId,
) -> Result<impl Future<Output = Result<(), AnyError>>, AnyError> {
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
fn op_ffi_unsafe_callback_unref(
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

#[op(v8)]
fn op_ffi_call_ptr_nonblocking<'scope, FP>(
  scope: &mut v8::HandleScope<'scope>,
  state: Rc<RefCell<deno_core::OpState>>,
  pointer: usize,
  def: ForeignFunction,
  parameters: serde_v8::Value<'scope>,
) -> Result<impl Future<Output = Result<Value, AnyError>>, AnyError>
where
  FP: FfiPermissions + 'static,
{
  check_unstable2(&state, "Deno.UnsafeFnPointer#call");
  {
    let mut state = state.borrow_mut();
    let permissions = state.borrow_mut::<FP>();
    permissions.check(None)?;
  };

  let symbol = PtrSymbol::new(pointer, &def);
  let call_args = ffi_parse_args(scope, parameters, &def.parameters)?;

  let join_handle = tokio::task::spawn_blocking(move || {
    let PtrSymbol { cif, ptr } = symbol.clone();
    ffi_call(call_args, &cif, ptr, &def.parameters, def.result)
  });

  Ok(async move {
    let result = join_handle
      .await
      .map_err(|err| anyhow!("Nonblocking FFI call failed: {}", err))??;
    // SAFETY: Same return type declared to libffi; trust user to have it right beyond that.
    Ok(unsafe { result.to_value(def.result) })
  })
}

#[op(v8)]
fn op_ffi_get_static<'scope>(
  scope: &mut v8::HandleScope<'scope>,
  state: &mut deno_core::OpState,
  rid: ResourceId,
  name: String,
  static_type: NativeType,
) -> Result<serde_v8::Value<'scope>, AnyError> {
  let resource = state.resource_table.get::<DynamicLibraryResource>(rid)?;

  let data_ptr = resource.get_static(name)?;

  Ok(match static_type {
    NativeType::Void => {
      return Err(type_error("Invalid FFI static type 'void'"));
    }
    NativeType::Bool => {
      // SAFETY: ptr is user provided
      let result = unsafe { ptr::read_unaligned(data_ptr as *const bool) };
      let boolean: v8::Local<v8::Value> =
        v8::Boolean::new(scope, result).into();
      boolean.into()
    }
    NativeType::U8 => {
      // SAFETY: ptr is user provided
      let result = unsafe { ptr::read_unaligned(data_ptr as *const u8) };
      let number: v8::Local<v8::Value> =
        v8::Integer::new_from_unsigned(scope, result as u32).into();
      number.into()
    }
    NativeType::I8 => {
      // SAFETY: ptr is user provided
      let result = unsafe { ptr::read_unaligned(data_ptr as *const i8) };
      let number: v8::Local<v8::Value> =
        v8::Integer::new(scope, result as i32).into();
      number.into()
    }
    NativeType::U16 => {
      // SAFETY: ptr is user provided
      let result = unsafe { ptr::read_unaligned(data_ptr as *const u16) };
      let number: v8::Local<v8::Value> =
        v8::Integer::new_from_unsigned(scope, result as u32).into();
      number.into()
    }
    NativeType::I16 => {
      // SAFETY: ptr is user provided
      let result = unsafe { ptr::read_unaligned(data_ptr as *const i16) };
      let number: v8::Local<v8::Value> =
        v8::Integer::new(scope, result as i32).into();
      number.into()
    }
    NativeType::U32 => {
      // SAFETY: ptr is user provided
      let result = unsafe { ptr::read_unaligned(data_ptr as *const u32) };
      let number: v8::Local<v8::Value> =
        v8::Integer::new_from_unsigned(scope, result).into();
      number.into()
    }
    NativeType::I32 => {
      // SAFETY: ptr is user provided
      let result = unsafe { ptr::read_unaligned(data_ptr as *const i32) };
      let number: v8::Local<v8::Value> = v8::Integer::new(scope, result).into();
      number.into()
    }
    NativeType::U64 => {
      // SAFETY: ptr is user provided
      let result = unsafe { ptr::read_unaligned(data_ptr as *const u64) };
      let integer: v8::Local<v8::Value> = if result > MAX_SAFE_INTEGER as u64 {
        v8::BigInt::new_from_u64(scope, result).into()
      } else {
        v8::Number::new(scope, result as f64).into()
      };
      integer.into()
    }
    NativeType::I64 => {
      // SAFETY: ptr is user provided
      let result = unsafe { ptr::read_unaligned(data_ptr as *const i64) };
      let integer: v8::Local<v8::Value> = if result > MAX_SAFE_INTEGER as i64
        || result < MIN_SAFE_INTEGER as i64
      {
        v8::BigInt::new_from_i64(scope, result).into()
      } else {
        v8::Number::new(scope, result as f64).into()
      };
      integer.into()
    }
    NativeType::USize => {
      // SAFETY: ptr is user provided
      let result = unsafe { ptr::read_unaligned(data_ptr as *const usize) };
      let integer: v8::Local<v8::Value> = if result > MAX_SAFE_INTEGER as usize
      {
        v8::BigInt::new_from_u64(scope, result as u64).into()
      } else {
        v8::Number::new(scope, result as f64).into()
      };
      integer.into()
    }
    NativeType::ISize => {
      // SAFETY: ptr is user provided
      let result = unsafe { ptr::read_unaligned(data_ptr as *const isize) };
      let integer: v8::Local<v8::Value> =
        if !(MIN_SAFE_INTEGER..=MAX_SAFE_INTEGER).contains(&result) {
          v8::BigInt::new_from_i64(scope, result as i64).into()
        } else {
          v8::Number::new(scope, result as f64).into()
        };
      integer.into()
    }
    NativeType::F32 => {
      // SAFETY: ptr is user provided
      let result = unsafe { ptr::read_unaligned(data_ptr as *const f32) };
      let number: v8::Local<v8::Value> =
        v8::Number::new(scope, result as f64).into();
      number.into()
    }
    NativeType::F64 => {
      // SAFETY: ptr is user provided
      let result = unsafe { ptr::read_unaligned(data_ptr as *const f64) };
      let number: v8::Local<v8::Value> = v8::Number::new(scope, result).into();
      number.into()
    }
    NativeType::Pointer | NativeType::Function | NativeType::Buffer => {
      let result = data_ptr as u64;
      let integer: v8::Local<v8::Value> = if result > MAX_SAFE_INTEGER as u64 {
        v8::BigInt::new_from_u64(scope, result).into()
      } else {
        v8::Number::new(scope, result as f64).into()
      };
      integer.into()
    }
  })
}

/// A non-blocking FFI call.
#[op(v8)]
fn op_ffi_call_nonblocking<'scope>(
  scope: &mut v8::HandleScope<'scope>,
  state: Rc<RefCell<deno_core::OpState>>,
  rid: ResourceId,
  symbol: String,
  parameters: serde_v8::Value<'scope>,
) -> Result<impl Future<Output = Result<Value, AnyError>> + 'static, AnyError> {
  let symbol = {
    let state = state.borrow();
    let resource = state.resource_table.get::<DynamicLibraryResource>(rid)?;
    let symbols = &resource.symbols;
    *symbols
      .get(&symbol)
      .ok_or_else(|| type_error("Invalid FFI symbol name"))?
      .clone()
  };

  let call_args = ffi_parse_args(scope, parameters, &symbol.parameter_types)?;

  let result_type = symbol.result_type;
  let join_handle = tokio::task::spawn_blocking(move || {
    let Symbol {
      cif,
      ptr,
      parameter_types,
      result_type,
      ..
    } = symbol.clone();
    ffi_call(call_args, &cif, ptr, &parameter_types, result_type)
  });

  Ok(async move {
    let result = join_handle
      .await
      .map_err(|err| anyhow!("Nonblocking FFI call failed: {}", err))??;
    // SAFETY: Same return type declared to libffi; trust user to have it right beyond that.
    Ok(unsafe { result.to_value(result_type) })
  })
}
