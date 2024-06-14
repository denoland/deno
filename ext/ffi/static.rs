// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use crate::dlfcn::DynamicLibraryResource;
use crate::symbol::NativeType;
use deno_core::error::type_error;
use deno_core::error::AnyError;
use deno_core::op2;
use deno_core::v8;
use deno_core::OpState;
use deno_core::ResourceId;
use std::ptr;

#[op2]
pub fn op_ffi_get_static<'scope>(
  scope: &mut v8::HandleScope<'scope>,
  state: &mut OpState,
  #[smi] rid: ResourceId,
  #[string] name: String,
  #[serde] static_type: NativeType,
  optional: bool,
) -> Result<v8::Local<'scope, v8::Value>, AnyError> {
  let resource = state.resource_table.get::<DynamicLibraryResource>(rid)?;

  let data_ptr = match resource.get_static(name) {
    Ok(data_ptr) => Ok(data_ptr),
    Err(err) => {
      if optional {
        let null: v8::Local<v8::Value> = v8::null(scope).into();
        return Ok(null);
      } else {
        Err(err)
      }
    }
  }?;

  Ok(match static_type {
    NativeType::Void => {
      return Err(type_error("Invalid FFI static type 'void'"));
    }
    NativeType::Bool => {
      // SAFETY: ptr is user provided
      let result = unsafe { ptr::read_unaligned(data_ptr as *const bool) };
      let boolean: v8::Local<v8::Value> =
        v8::Boolean::new(scope, result).into();
      boolean
    }
    NativeType::U8 => {
      // SAFETY: ptr is user provided
      let result = unsafe { ptr::read_unaligned(data_ptr as *const u8) };
      let number: v8::Local<v8::Value> =
        v8::Integer::new_from_unsigned(scope, result as u32).into();
      number
    }
    NativeType::I8 => {
      // SAFETY: ptr is user provided
      let result = unsafe { ptr::read_unaligned(data_ptr as *const i8) };
      let number: v8::Local<v8::Value> =
        v8::Integer::new(scope, result as i32).into();
      number
    }
    NativeType::U16 => {
      // SAFETY: ptr is user provided
      let result = unsafe { ptr::read_unaligned(data_ptr as *const u16) };
      let number: v8::Local<v8::Value> =
        v8::Integer::new_from_unsigned(scope, result as u32).into();
      number
    }
    NativeType::I16 => {
      // SAFETY: ptr is user provided
      let result = unsafe { ptr::read_unaligned(data_ptr as *const i16) };
      let number: v8::Local<v8::Value> =
        v8::Integer::new(scope, result as i32).into();
      number
    }
    NativeType::U32 => {
      // SAFETY: ptr is user provided
      let result = unsafe { ptr::read_unaligned(data_ptr as *const u32) };
      let number: v8::Local<v8::Value> =
        v8::Integer::new_from_unsigned(scope, result).into();
      number
    }
    NativeType::I32 => {
      // SAFETY: ptr is user provided
      let result = unsafe { ptr::read_unaligned(data_ptr as *const i32) };
      let number: v8::Local<v8::Value> = v8::Integer::new(scope, result).into();
      number
    }
    NativeType::U64 => {
      // SAFETY: ptr is user provided
      let result = unsafe { ptr::read_unaligned(data_ptr as *const u64) };
      let integer: v8::Local<v8::Value> =
        v8::BigInt::new_from_u64(scope, result).into();
      integer
    }
    NativeType::I64 => {
      // SAFETY: ptr is user provided
      let result = unsafe { ptr::read_unaligned(data_ptr as *const i64) };
      let integer: v8::Local<v8::Value> =
        v8::BigInt::new_from_i64(scope, result).into();
      integer
    }
    NativeType::USize => {
      // SAFETY: ptr is user provided
      let result = unsafe { ptr::read_unaligned(data_ptr as *const usize) };
      let integer: v8::Local<v8::Value> =
        v8::BigInt::new_from_u64(scope, result as u64).into();
      integer
    }
    NativeType::ISize => {
      // SAFETY: ptr is user provided
      let result = unsafe { ptr::read_unaligned(data_ptr as *const isize) };
      let integer: v8::Local<v8::Value> =
        v8::BigInt::new_from_i64(scope, result as i64).into();
      integer
    }
    NativeType::F32 => {
      // SAFETY: ptr is user provided
      let result = unsafe { ptr::read_unaligned(data_ptr as *const f32) };
      let number: v8::Local<v8::Value> =
        v8::Number::new(scope, result as f64).into();
      number
    }
    NativeType::F64 => {
      // SAFETY: ptr is user provided
      let result = unsafe { ptr::read_unaligned(data_ptr as *const f64) };
      let number: v8::Local<v8::Value> = v8::Number::new(scope, result).into();
      number
    }
    NativeType::Pointer | NativeType::Function | NativeType::Buffer => {
      let external: v8::Local<v8::Value> =
        v8::External::new(scope, data_ptr).into();
      external
    }
    NativeType::Struct(_) => {
      return Err(type_error("Invalid FFI static type 'struct'"));
    }
  })
}
