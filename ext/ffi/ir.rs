// Copyright 2018-2025 the Deno authors. MIT license.

use std::ffi::c_void;
use std::ptr;

use deno_core::v8;
use libffi::middle::Arg;

use crate::symbol::NativeType;

#[derive(Debug, thiserror::Error, deno_error::JsError)]
#[class(type)]
pub enum IRError {
  #[error("Invalid FFI u8 type, expected boolean")]
  InvalidU8ExpectedBoolean,
  #[error("Invalid FFI u8 type, expected unsigned integer")]
  InvalidU8ExpectedUnsignedInteger,
  #[error("Invalid FFI i8 type, expected integer")]
  InvalidI8,
  #[error("Invalid FFI u16 type, expected unsigned integer")]
  InvalidU16,
  #[error("Invalid FFI i16 type, expected integer")]
  InvalidI16,
  #[error("Invalid FFI u32 type, expected unsigned integer")]
  InvalidU32,
  #[error("Invalid FFI i32 type, expected integer")]
  InvalidI32,
  #[error("Invalid FFI u64 type, expected unsigned integer")]
  InvalidU64,
  #[error("Invalid FFI i64 type, expected integer")]
  InvalidI64,
  #[error("Invalid FFI usize type, expected unsigned integer")]
  InvalidUsize,
  #[error("Invalid FFI isize type, expected integer")]
  InvalidIsize,
  #[error("Invalid FFI f32 type, expected number")]
  InvalidF32,
  #[error("Invalid FFI f64 type, expected number")]
  InvalidF64,
  #[error("Invalid FFI pointer type, expected null, or External")]
  InvalidPointerType,
  #[error(
    "Invalid FFI buffer type, expected null, ArrayBuffer, or ArrayBufferView"
  )]
  InvalidBufferType,
  #[error("Invalid FFI ArrayBufferView, expected data in the buffer")]
  InvalidArrayBufferView,
  #[error("Invalid FFI ArrayBuffer, expected data in buffer")]
  InvalidArrayBuffer,
  #[error("Invalid FFI struct type, expected ArrayBuffer, or ArrayBufferView")]
  InvalidStructType,
  #[error("Invalid FFI function type, expected null, or External")]
  InvalidFunctionType,
}

pub struct OutBuffer(pub *mut u8);

// SAFETY: OutBuffer is allocated by us in 00_ffi.js and is guaranteed to be
// only used for the purpose of writing return value of structs.
unsafe impl Send for OutBuffer {}
// SAFETY: See above
unsafe impl Sync for OutBuffer {}

pub fn out_buffer_as_ptr(
  scope: &mut v8::HandleScope,
  out_buffer: Option<v8::Local<v8::TypedArray>>,
) -> Option<OutBuffer> {
  match out_buffer {
    Some(out_buffer) => {
      let ab = out_buffer.buffer(scope).unwrap();
      ab.data()
        .map(|non_null| OutBuffer(non_null.as_ptr() as *mut u8))
    }
    None => None,
  }
}

/// Intermediate format for easy translation from NativeType + V8 value
/// to libffi argument types.
#[repr(C)]
pub union NativeValue {
  pub void_value: (),
  pub bool_value: bool,
  pub u8_value: u8,
  pub i8_value: i8,
  pub u16_value: u16,
  pub i16_value: i16,
  pub u32_value: u32,
  pub i32_value: i32,
  pub u64_value: u64,
  pub i64_value: i64,
  pub usize_value: usize,
  pub isize_value: isize,
  pub f32_value: f32,
  pub f64_value: f64,
  pub pointer: *mut c_void,
}

impl NativeValue {
  pub unsafe fn as_arg(&self, native_type: &NativeType) -> Arg {
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
      NativeType::Struct(_) => Arg::new(&*self.pointer),
    }
  }

  // SAFETY: native_type must correspond to the type of value represented by the union field
  #[inline]
  pub unsafe fn to_v8<'scope>(
    &self,
    scope: &mut v8::HandleScope<'scope>,
    native_type: NativeType,
  ) -> v8::Local<'scope, v8::Value> {
    match native_type {
      NativeType::Void => v8::undefined(scope).into(),
      NativeType::Bool => v8::Boolean::new(scope, self.bool_value).into(),
      NativeType::U8 => {
        v8::Integer::new_from_unsigned(scope, self.u8_value as u32).into()
      }
      NativeType::I8 => v8::Integer::new(scope, self.i8_value as i32).into(),
      NativeType::U16 => {
        v8::Integer::new_from_unsigned(scope, self.u16_value as u32).into()
      }
      NativeType::I16 => v8::Integer::new(scope, self.i16_value as i32).into(),
      NativeType::U32 => {
        v8::Integer::new_from_unsigned(scope, self.u32_value).into()
      }
      NativeType::I32 => v8::Integer::new(scope, self.i32_value).into(),
      NativeType::U64 => v8::BigInt::new_from_u64(scope, self.u64_value).into(),
      NativeType::I64 => v8::BigInt::new_from_i64(scope, self.i64_value).into(),
      NativeType::USize => {
        v8::BigInt::new_from_u64(scope, self.usize_value as u64).into()
      }
      NativeType::ISize => {
        v8::BigInt::new_from_i64(scope, self.isize_value as i64).into()
      }
      NativeType::F32 => v8::Number::new(scope, self.f32_value as f64).into(),
      NativeType::F64 => v8::Number::new(scope, self.f64_value).into(),
      NativeType::Pointer | NativeType::Buffer | NativeType::Function => {
        let local_value: v8::Local<v8::Value> = if self.pointer.is_null() {
          v8::null(scope).into()
        } else {
          v8::External::new(scope, self.pointer).into()
        };
        local_value
      }
      NativeType::Struct(_) => v8::null(scope).into(),
    }
  }
}

// SAFETY: unsafe trait must have unsafe implementation
unsafe impl Send for NativeValue {}

#[inline]
pub fn ffi_parse_bool_arg(
  arg: v8::Local<v8::Value>,
) -> Result<NativeValue, IRError> {
  let bool_value = v8::Local::<v8::Boolean>::try_from(arg)
    .map_err(|_| IRError::InvalidU8ExpectedBoolean)?
    .is_true();
  Ok(NativeValue { bool_value })
}

#[inline]
pub fn ffi_parse_u8_arg(
  arg: v8::Local<v8::Value>,
) -> Result<NativeValue, IRError> {
  let u8_value = v8::Local::<v8::Uint32>::try_from(arg)
    .map_err(|_| IRError::InvalidU8ExpectedUnsignedInteger)?
    .value() as u8;
  Ok(NativeValue { u8_value })
}

#[inline]
pub fn ffi_parse_i8_arg(
  arg: v8::Local<v8::Value>,
) -> Result<NativeValue, IRError> {
  let i8_value = v8::Local::<v8::Int32>::try_from(arg)
    .map_err(|_| IRError::InvalidI8)?
    .value() as i8;
  Ok(NativeValue { i8_value })
}

#[inline]
pub fn ffi_parse_u16_arg(
  arg: v8::Local<v8::Value>,
) -> Result<NativeValue, IRError> {
  let u16_value = v8::Local::<v8::Uint32>::try_from(arg)
    .map_err(|_| IRError::InvalidU16)?
    .value() as u16;
  Ok(NativeValue { u16_value })
}

#[inline]
pub fn ffi_parse_i16_arg(
  arg: v8::Local<v8::Value>,
) -> Result<NativeValue, IRError> {
  let i16_value = v8::Local::<v8::Int32>::try_from(arg)
    .map_err(|_| IRError::InvalidI16)?
    .value() as i16;
  Ok(NativeValue { i16_value })
}

#[inline]
pub fn ffi_parse_u32_arg(
  arg: v8::Local<v8::Value>,
) -> Result<NativeValue, IRError> {
  let u32_value = v8::Local::<v8::Uint32>::try_from(arg)
    .map_err(|_| IRError::InvalidU32)?
    .value();
  Ok(NativeValue { u32_value })
}

#[inline]
pub fn ffi_parse_i32_arg(
  arg: v8::Local<v8::Value>,
) -> Result<NativeValue, IRError> {
  let i32_value = v8::Local::<v8::Int32>::try_from(arg)
    .map_err(|_| IRError::InvalidI32)?
    .value();
  Ok(NativeValue { i32_value })
}

#[inline]
pub fn ffi_parse_u64_arg(
  scope: &mut v8::HandleScope,
  arg: v8::Local<v8::Value>,
) -> Result<NativeValue, IRError> {
  // Order of checking:
  // 1. BigInt: Uncommon and not supported by Fast API, so optimise slow call for this case.
  // 2. Number: Common, supported by Fast API, so let that be the optimal case.
  let u64_value: u64 = if let Ok(value) = v8::Local::<v8::BigInt>::try_from(arg)
  {
    value.u64_value().0
  } else if let Ok(value) = v8::Local::<v8::Number>::try_from(arg) {
    value.integer_value(scope).unwrap() as u64
  } else {
    return Err(IRError::InvalidU64);
  };
  Ok(NativeValue { u64_value })
}

#[inline]
pub fn ffi_parse_i64_arg(
  scope: &mut v8::HandleScope,
  arg: v8::Local<v8::Value>,
) -> Result<NativeValue, IRError> {
  // Order of checking:
  // 1. BigInt: Uncommon and not supported by Fast API, so optimise slow call for this case.
  // 2. Number: Common, supported by Fast API, so let that be the optimal case.
  let i64_value: i64 = if let Ok(value) = v8::Local::<v8::BigInt>::try_from(arg)
  {
    value.i64_value().0
  } else if let Ok(value) = v8::Local::<v8::Number>::try_from(arg) {
    value.integer_value(scope).unwrap()
  } else {
    return Err(IRError::InvalidI64);
  };
  Ok(NativeValue { i64_value })
}

#[inline]
pub fn ffi_parse_usize_arg(
  scope: &mut v8::HandleScope,
  arg: v8::Local<v8::Value>,
) -> Result<NativeValue, IRError> {
  // Order of checking:
  // 1. BigInt: Uncommon and not supported by Fast API, so optimise slow call for this case.
  // 2. Number: Common, supported by Fast API, so let that be the optimal case.
  let usize_value: usize =
    if let Ok(value) = v8::Local::<v8::BigInt>::try_from(arg) {
      value.u64_value().0 as usize
    } else if let Ok(value) = v8::Local::<v8::Number>::try_from(arg) {
      value.integer_value(scope).unwrap() as usize
    } else {
      return Err(IRError::InvalidUsize);
    };
  Ok(NativeValue { usize_value })
}

#[inline]
pub fn ffi_parse_isize_arg(
  scope: &mut v8::HandleScope,
  arg: v8::Local<v8::Value>,
) -> Result<NativeValue, IRError> {
  // Order of checking:
  // 1. BigInt: Uncommon and not supported by Fast API, so optimise slow call for this case.
  // 2. Number: Common, supported by Fast API, so let that be the optimal case.
  let isize_value: isize =
    if let Ok(value) = v8::Local::<v8::BigInt>::try_from(arg) {
      value.i64_value().0 as isize
    } else if let Ok(value) = v8::Local::<v8::Number>::try_from(arg) {
      value.integer_value(scope).unwrap() as isize
    } else {
      return Err(IRError::InvalidIsize);
    };
  Ok(NativeValue { isize_value })
}

#[inline]
pub fn ffi_parse_f32_arg(
  arg: v8::Local<v8::Value>,
) -> Result<NativeValue, IRError> {
  let f32_value = v8::Local::<v8::Number>::try_from(arg)
    .map_err(|_| IRError::InvalidF32)?
    .value() as f32;
  Ok(NativeValue { f32_value })
}

#[inline]
pub fn ffi_parse_f64_arg(
  arg: v8::Local<v8::Value>,
) -> Result<NativeValue, IRError> {
  let f64_value = v8::Local::<v8::Number>::try_from(arg)
    .map_err(|_| IRError::InvalidF64)?
    .value();
  Ok(NativeValue { f64_value })
}

#[inline]
pub fn ffi_parse_pointer_arg(
  _scope: &mut v8::HandleScope,
  arg: v8::Local<v8::Value>,
) -> Result<NativeValue, IRError> {
  let pointer = if let Ok(value) = v8::Local::<v8::External>::try_from(arg) {
    value.value()
  } else if arg.is_null() {
    ptr::null_mut()
  } else {
    return Err(IRError::InvalidPointerType);
  };
  Ok(NativeValue { pointer })
}

#[inline]
pub fn parse_buffer_arg(
  arg: v8::Local<v8::Value>,
) -> Result<*mut c_void, IRError> {
  // Order of checking:
  // 1. ArrayBuffer: Fairly common and not supported by Fast API, optimise this case.
  // 2. ArrayBufferView: Common and supported by Fast API
  // 5. Null: Very uncommon / can be represented by a 0.

  if let Ok(value) = v8::Local::<v8::ArrayBuffer>::try_from(arg) {
    Ok(value.data().map(|p| p.as_ptr()).unwrap_or(ptr::null_mut()))
  } else if let Ok(value) = v8::Local::<v8::ArrayBufferView>::try_from(arg) {
    const {
      // We don't keep `buffer` around when this function returns,
      // so assert that it will be unused.
      assert!(deno_core::v8::TYPED_ARRAY_MAX_SIZE_IN_HEAP == 0);
    }
    let mut buffer = [0; deno_core::v8::TYPED_ARRAY_MAX_SIZE_IN_HEAP];
    // SAFETY: `buffer` is unused due to above, returned pointer is not
    // dereferenced by rust code, and we keep it alive at least as long
    // as the turbocall.
    let (ptr, len) = unsafe { value.get_contents_raw_parts(&mut buffer) };
    if ptr == buffer.as_mut_ptr() {
      // Empty TypedArray instances can hit this path because their base pointer
      // isn't cleared properly: https://issues.chromium.org/issues/40643872
      debug_assert_eq!(len, 0);
      Ok(ptr::null_mut())
    } else {
      Ok(ptr as _)
    }
  } else if arg.is_null() {
    Ok(ptr::null_mut())
  } else {
    Err(IRError::InvalidBufferType)
  }
}

#[inline]
pub fn ffi_parse_buffer_arg(
  arg: v8::Local<v8::Value>,
) -> Result<NativeValue, IRError> {
  let pointer = parse_buffer_arg(arg)?;
  Ok(NativeValue { pointer })
}

#[inline]
pub fn ffi_parse_struct_arg(
  scope: &mut v8::HandleScope,
  arg: v8::Local<v8::Value>,
) -> Result<NativeValue, IRError> {
  // Order of checking:
  // 1. ArrayBuffer: Fairly common and not supported by Fast API, optimise this case.
  // 2. ArrayBufferView: Common and supported by Fast API

  let pointer = if let Ok(value) = v8::Local::<v8::ArrayBuffer>::try_from(arg) {
    if let Some(non_null) = value.data() {
      non_null.as_ptr()
    } else {
      return Err(IRError::InvalidArrayBuffer);
    }
  } else if let Ok(value) = v8::Local::<v8::ArrayBufferView>::try_from(arg) {
    let byte_offset = value.byte_offset();
    let pointer = value
      .buffer(scope)
      .ok_or(IRError::InvalidArrayBufferView)?
      .data();
    if let Some(non_null) = pointer {
      // SAFETY: Pointer is non-null, and V8 guarantees that the byte_offset
      // is within the buffer backing store.
      unsafe { non_null.as_ptr().add(byte_offset) }
    } else {
      return Err(IRError::InvalidArrayBufferView);
    }
  } else {
    return Err(IRError::InvalidStructType);
  };
  Ok(NativeValue { pointer })
}

#[inline]
pub fn ffi_parse_function_arg(
  _scope: &mut v8::HandleScope,
  arg: v8::Local<v8::Value>,
) -> Result<NativeValue, IRError> {
  let pointer = if let Ok(value) = v8::Local::<v8::External>::try_from(arg) {
    value.value()
  } else if arg.is_null() {
    ptr::null_mut()
  } else {
    return Err(IRError::InvalidFunctionType);
  };
  Ok(NativeValue { pointer })
}

pub fn ffi_parse_args<'scope>(
  scope: &mut v8::HandleScope<'scope>,
  args: v8::Local<v8::Array>,
  parameter_types: &[NativeType],
) -> Result<Vec<NativeValue>, IRError>
where
  'scope: 'scope,
{
  if parameter_types.is_empty() {
    return Ok(vec![]);
  }

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
        ffi_args.push(ffi_parse_buffer_arg(value)?);
      }
      NativeType::Struct(_) => {
        ffi_args.push(ffi_parse_struct_arg(scope, value)?);
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
