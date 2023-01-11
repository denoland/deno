// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use crate::symbol::NativeType;
use crate::MAX_SAFE_INTEGER;
use crate::MIN_SAFE_INTEGER;
use deno_core::error::type_error;
use deno_core::error::AnyError;
use deno_core::serde_json::Value;
use deno_core::serde_v8;
use deno_core::v8;
use libffi::middle::Arg;
use std::ffi::c_void;
use std::ptr;

pub struct OutBuffer(pub *mut u8, pub usize);

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
      let len = ab.byte_length();
      ab.data()
        .map(|non_null| OutBuffer(non_null.as_ptr() as *mut u8, len))
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
  pub unsafe fn to_value(&self, native_type: NativeType) -> Value {
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
      NativeType::Struct(_) => {
        // Return value is written to out_buffer
        Value::Null
      }
    }
  }

  // SAFETY: native_type must correspond to the type of value represented by the union field
  #[inline]
  pub unsafe fn to_v8<'scope>(
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
      NativeType::Struct(_) => {
        let local_value: v8::Local<v8::Value> = v8::null(scope).into();
        local_value.into()
      }
    }
  }
}

// SAFETY: unsafe trait must have unsafe implementation
unsafe impl Send for NativeValue {}

#[inline]
pub fn ffi_parse_bool_arg(
  arg: v8::Local<v8::Value>,
) -> Result<NativeValue, AnyError> {
  let bool_value = v8::Local::<v8::Boolean>::try_from(arg)
    .map_err(|_| type_error("Invalid FFI u8 type, expected boolean"))?
    .is_true();
  Ok(NativeValue { bool_value })
}

#[inline]
pub fn ffi_parse_u8_arg(
  arg: v8::Local<v8::Value>,
) -> Result<NativeValue, AnyError> {
  let u8_value = v8::Local::<v8::Uint32>::try_from(arg)
    .map_err(|_| type_error("Invalid FFI u8 type, expected unsigned integer"))?
    .value() as u8;
  Ok(NativeValue { u8_value })
}

#[inline]
pub fn ffi_parse_i8_arg(
  arg: v8::Local<v8::Value>,
) -> Result<NativeValue, AnyError> {
  let i8_value = v8::Local::<v8::Int32>::try_from(arg)
    .map_err(|_| type_error("Invalid FFI i8 type, expected integer"))?
    .value() as i8;
  Ok(NativeValue { i8_value })
}

#[inline]
pub fn ffi_parse_u16_arg(
  arg: v8::Local<v8::Value>,
) -> Result<NativeValue, AnyError> {
  let u16_value = v8::Local::<v8::Uint32>::try_from(arg)
    .map_err(|_| type_error("Invalid FFI u16 type, expected unsigned integer"))?
    .value() as u16;
  Ok(NativeValue { u16_value })
}

#[inline]
pub fn ffi_parse_i16_arg(
  arg: v8::Local<v8::Value>,
) -> Result<NativeValue, AnyError> {
  let i16_value = v8::Local::<v8::Int32>::try_from(arg)
    .map_err(|_| type_error("Invalid FFI i16 type, expected integer"))?
    .value() as i16;
  Ok(NativeValue { i16_value })
}

#[inline]
pub fn ffi_parse_u32_arg(
  arg: v8::Local<v8::Value>,
) -> Result<NativeValue, AnyError> {
  let u32_value = v8::Local::<v8::Uint32>::try_from(arg)
    .map_err(|_| type_error("Invalid FFI u32 type, expected unsigned integer"))?
    .value();
  Ok(NativeValue { u32_value })
}

#[inline]
pub fn ffi_parse_i32_arg(
  arg: v8::Local<v8::Value>,
) -> Result<NativeValue, AnyError> {
  let i32_value = v8::Local::<v8::Int32>::try_from(arg)
    .map_err(|_| type_error("Invalid FFI i32 type, expected integer"))?
    .value();
  Ok(NativeValue { i32_value })
}

#[inline]
pub fn ffi_parse_u64_arg(
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
pub fn ffi_parse_i64_arg(
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
    value.integer_value(scope).unwrap()
  } else {
    return Err(type_error("Invalid FFI i64 type, expected integer"));
  };
  Ok(NativeValue { i64_value })
}

#[inline]
pub fn ffi_parse_usize_arg(
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
pub fn ffi_parse_isize_arg(
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
pub fn ffi_parse_f32_arg(
  arg: v8::Local<v8::Value>,
) -> Result<NativeValue, AnyError> {
  let f32_value = v8::Local::<v8::Number>::try_from(arg)
    .map_err(|_| type_error("Invalid FFI f32 type, expected number"))?
    .value() as f32;
  Ok(NativeValue { f32_value })
}

#[inline]
pub fn ffi_parse_f64_arg(
  arg: v8::Local<v8::Value>,
) -> Result<NativeValue, AnyError> {
  let f64_value = v8::Local::<v8::Number>::try_from(arg)
    .map_err(|_| type_error("Invalid FFI f64 type, expected number"))?
    .value();
  Ok(NativeValue { f64_value })
}

#[inline]
pub fn ffi_parse_pointer_arg(
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
pub fn ffi_parse_buffer_arg(
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
pub fn ffi_parse_struct_arg(
  scope: &mut v8::HandleScope,
  arg: v8::Local<v8::Value>,
) -> Result<NativeValue, AnyError> {
  // Order of checking:
  // 1. ArrayBuffer: Fairly common and not supported by Fast API, optimise this case.
  // 2. ArrayBufferView: Common and supported by Fast API

  let pointer = if let Ok(value) = v8::Local::<v8::ArrayBuffer>::try_from(arg) {
    if let Some(non_null) = value.data() {
      non_null.as_ptr()
    } else {
      return Err(type_error(
        "Invalid FFI ArrayBuffer, expected data in buffer",
      ));
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
      return Err(type_error(
        "Invalid FFI ArrayBufferView, expected data in buffer",
      ));
    }
  } else {
    return Err(type_error(
      "Invalid FFI struct type, expected ArrayBuffer, or ArrayBufferView",
    ));
  };
  Ok(NativeValue { pointer })
}

#[inline]
pub fn ffi_parse_function_arg(
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

pub fn ffi_parse_args<'scope>(
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
