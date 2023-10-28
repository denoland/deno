// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use std::borrow::Cow;
use std::ffi::c_void;
use std::path::PathBuf;
use std::ptr::NonNull;
use std::ptr::{self};

use deno_core::error::type_error;
use deno_core::error::AnyError;
use deno_core::op;
use deno_core::serde_v8::ExternalPointer;
use deno_core::v8;
use deno_core::OpState;
use deno_core::Resource;
use deno_core::ResourceId;

use crate::check_unstable;
use crate::FfiPermissions;

pub struct FfiTokenResource {
  ptr: *const c_void,
}

impl Resource for FfiTokenResource {
  fn name(&self) -> Cow<str> {
    "ffitoken".into()
  }
}

impl FfiTokenResource {
  fn check(&self, key: *mut c_void) -> Result<(), AnyError> {
    if !std::ptr::eq(self.ptr, key) {
      Err(type_error("Attempted to use invalid FFI token"))
    } else {
      Ok(())
    }
  }
}

#[op]
pub fn op_ffi_create_token<FP>(
  state: &mut OpState,
  path: String,
) -> Result<(ResourceId, ExternalPointer), AnyError>
where
  FP: FfiPermissions + 'static,
{
  check_unstable(state, "Deno.createFfiToken");
  let permissions = state.borrow_mut::<FP>();
  permissions.check_partial(Some(&PathBuf::from(&path)))?;

  let ptr: *mut c_void = NonNull::<c_void>::dangling().as_ptr();
  let resource = FfiTokenResource { ptr };
  let rid = state.resource_table.add(resource);

  Ok((rid, ptr.into()))
}

#[inline(always)]
fn check_token(
  state: &mut OpState,
  rid: ResourceId,
  key: *mut c_void,
) -> Result<(), AnyError> {
  state
    .resource_table
    .get::<FfiTokenResource>(rid)?
    .check(key)
}

#[op(fast)]
pub fn op_ffi_token_ptr_create(
  state: &mut OpState,
  rid: ResourceId,
  key: *mut c_void,
  ptr_number: usize,
) -> Result<*mut c_void, AnyError> {
  check_token(state, rid, key)?;
  Ok(ptr_number as *mut c_void)
}

#[op(fast)]
pub fn op_ffi_token_ptr_equals(
  state: &mut OpState,
  rid: ResourceId,
  key: *mut c_void,
) -> Result<(), AnyError> {
  check_token(state, rid, key)?;
  Ok(())
}

#[op(fast)]
pub fn op_ffi_token_ptr_of(
  state: &mut OpState,
  rid: ResourceId,
  key: *mut c_void,
) -> Result<(), AnyError> {
  check_token(state, rid, key)?;
  Ok(())
}

#[op(fast)]
pub fn op_ffi_token_ptr_offset(
  state: &mut OpState,
  rid: ResourceId,
  key: *mut c_void,
) -> Result<(), AnyError> {
  check_token(state, rid, key)?;
  Ok(())
}

#[op(fast)]
pub fn op_ffi_token_read_u8(
  state: &mut OpState,
  rid: ResourceId,
  key: *mut c_void,
  ptr: *mut c_void,
  offset: isize,
) -> Result<u32, AnyError> {
  check_token(state, rid, key)?;

  if ptr.is_null() {
    return Err(type_error("Invalid u8 pointer, pointer is null"));
  }

  // SAFETY: ptr and offset are user provided.
  Ok(unsafe {
    ptr::read_unaligned::<u8>(ptr.offset(offset) as *const u8) as u32
  })
}

#[op(fast)]
pub fn op_ffi_token_read_bool(
  state: &mut OpState,
  rid: ResourceId,
  key: *mut c_void,
  ptr: *mut c_void,
  offset: isize,
) -> Result<bool, AnyError> {
  check_token(state, rid, key)?;

  if ptr.is_null() {
    return Err(type_error("Invalid u8 pointer, pointer is null"));
  }

  // SAFETY: ptr and offset are user provided.
  Ok(unsafe {
    ptr::read_unaligned::<bool>(ptr.offset(offset) as *const bool) as bool
  })
}

#[op(fast)]
pub fn op_ffi_token_read_i8(
  state: &mut OpState,
  rid: ResourceId,
  key: *mut c_void,
  ptr: *mut c_void,
  offset: isize,
) -> Result<i32, AnyError> {
  check_token(state, rid, key)?;

  if ptr.is_null() {
    return Err(type_error("Invalid u8 pointer, pointer is null"));
  }

  // SAFETY: ptr and offset are user provided.
  Ok(unsafe {
    ptr::read_unaligned::<i8>(ptr.offset(offset) as *const i8) as i32
  })
}

#[op(fast)]
pub fn op_ffi_token_read_u16(
  state: &mut OpState,
  rid: ResourceId,
  key: *mut c_void,
  ptr: *mut c_void,
  offset: isize,
) -> Result<u32, AnyError> {
  check_token(state, rid, key)?;

  if ptr.is_null() {
    return Err(type_error("Invalid u8 pointer, pointer is null"));
  }

  // SAFETY: ptr and offset are user provided.
  Ok(unsafe {
    ptr::read_unaligned::<u16>(ptr.offset(offset) as *const u16) as u32
  })
}

#[op(fast)]
pub fn op_ffi_token_read_i16(
  state: &mut OpState,
  rid: ResourceId,
  key: *mut c_void,
  ptr: *mut c_void,
  offset: isize,
) -> Result<i32, AnyError> {
  check_token(state, rid, key)?;

  if ptr.is_null() {
    return Err(type_error("Invalid u8 pointer, pointer is null"));
  }

  // SAFETY: ptr and offset are user provided.
  Ok(unsafe {
    ptr::read_unaligned::<i16>(ptr.offset(offset) as *const i16) as i32
  })
}

#[op(fast)]
pub fn op_ffi_token_read_u32(
  state: &mut OpState,
  rid: ResourceId,
  key: *mut c_void,
  ptr: *mut c_void,
  offset: isize,
) -> Result<u32, AnyError> {
  check_token(state, rid, key)?;

  if ptr.is_null() {
    return Err(type_error("Invalid u8 pointer, pointer is null"));
  }

  // SAFETY: ptr and offset are user provided.
  Ok(unsafe {
    ptr::read_unaligned::<u32>(ptr.offset(offset) as *const u32) as u32
  })
}

#[op(fast)]
pub fn op_ffi_token_read_i32(
  state: &mut OpState,
  rid: ResourceId,
  key: *mut c_void,
  ptr: *mut c_void,
  offset: isize,
) -> Result<i32, AnyError> {
  check_token(state, rid, key)?;

  if ptr.is_null() {
    return Err(type_error("Invalid u8 pointer, pointer is null"));
  }

  // SAFETY: ptr and offset are user provided.
  Ok(unsafe {
    ptr::read_unaligned::<i32>(ptr.offset(offset) as *const i32) as i32
  })
}

#[op(fast)]
pub fn op_ffi_token_read_u64(
  state: &mut OpState,
  rid: ResourceId,
  key: *mut c_void,
  ptr: *mut c_void,
  offset: isize,
) -> Result<u64, AnyError> {
  check_token(state, rid, key)?;

  if ptr.is_null() {
    return Err(type_error("Invalid u8 pointer, pointer is null"));
  }

  // SAFETY: ptr and offset are user provided.
  Ok(unsafe {
    ptr::read_unaligned::<u64>(ptr.offset(offset) as *const u64) as u64
  })
}

#[op(fast)]
pub fn op_ffi_token_read_i64(
  state: &mut OpState,
  rid: ResourceId,
  key: *mut c_void,
  ptr: *mut c_void,
  offset: isize,
) -> Result<i64, AnyError> {
  check_token(state, rid, key)?;

  if ptr.is_null() {
    return Err(type_error("Invalid u8 pointer, pointer is null"));
  }

  // SAFETY: ptr and offset are user provided.
  Ok(unsafe {
    ptr::read_unaligned::<i64>(ptr.offset(offset) as *const i64) as i64
  })
}

#[op(fast)]
pub fn op_ffi_token_read_f32(
  state: &mut OpState,
  rid: ResourceId,
  key: *mut c_void,
  ptr: *mut c_void,
  offset: isize,
) -> Result<f64, AnyError> {
  check_token(state, rid, key)?;

  if ptr.is_null() {
    return Err(type_error("Invalid u8 pointer, pointer is null"));
  }

  // SAFETY: ptr and offset are user provided.
  Ok(unsafe {
    ptr::read_unaligned::<f32>(ptr.offset(offset) as *const f32) as f64
  })
}

#[op(fast)]
pub fn op_ffi_token_read_f64(
  state: &mut OpState,
  rid: ResourceId,
  key: *mut c_void,
  ptr: *mut c_void,
  offset: isize,
) -> Result<f64, AnyError> {
  check_token(state, rid, key)?;

  if ptr.is_null() {
    return Err(type_error("Invalid u8 pointer, pointer is null"));
  }

  // SAFETY: ptr and offset are user provided.
  Ok(unsafe {
    ptr::read_unaligned::<f64>(ptr.offset(offset) as *const f64) as f64
  })
}

#[op(fast)]
pub fn op_ffi_token_read_ptr(
  state: &mut OpState,
  rid: ResourceId,
  key: *mut c_void,
  ptr: *mut c_void,
  offset: isize,
) -> Result<*mut c_void, AnyError> {
  check_token(state, rid, key)?;

  if ptr.is_null() {
    return Err(type_error("Invalid u8 pointer, pointer is null"));
  }

  // SAFETY: ptr and offset are user provided.
  Ok(unsafe {
    ptr::read_unaligned::<*mut c_void>(ptr.offset(offset) as *const *mut c_void)
      as *mut c_void
  })
}

#[op(fast)]
pub fn op_ffi_token_write_bool(
  state: &mut OpState,
  rid: ResourceId,
  key: *mut c_void,
  ptr: *mut c_void,
  offset: isize,
  value: bool,
) -> Result<(), AnyError> {
  check_token(state, rid, key)?;
  Ok(())
}

#[op(fast)]
pub fn op_ffi_token_write_u8(
  state: &mut OpState,
  rid: ResourceId,
  key: *mut c_void,
  ptr: *mut c_void,
  offset: isize,
  value: u8,
) -> Result<(), AnyError> {
  check_token(state, rid, key)?;
  Ok(())
}

#[op(fast)]
pub fn op_ffi_token_write_i8(
  state: &mut OpState,
  rid: ResourceId,
  key: *mut c_void,
  ptr: *mut c_void,
  offset: isize,
  value: i8,
) -> Result<(), AnyError> {
  check_token(state, rid, key)?;
  Ok(())
}

#[op(fast)]
pub fn op_ffi_token_write_u16(
  state: &mut OpState,
  rid: ResourceId,
  key: *mut c_void,
  ptr: *mut c_void,
  offset: isize,
  value: u16,
) -> Result<(), AnyError> {
  check_token(state, rid, key)?;
  Ok(())
}

#[op(fast)]
pub fn op_ffi_token_write_i16(
  state: &mut OpState,
  rid: ResourceId,
  key: *mut c_void,
  ptr: *mut c_void,
  offset: isize,
  value: i16,
) -> Result<(), AnyError> {
  check_token(state, rid, key)?;
  Ok(())
}

#[op(fast)]
pub fn op_ffi_token_write_u32(
  state: &mut OpState,
  rid: ResourceId,
  key: *mut c_void,
  ptr: *mut c_void,
  offset: isize,
  value: u32,
) -> Result<(), AnyError> {
  check_token(state, rid, key)?;
  Ok(())
}

#[op(fast)]
pub fn op_ffi_token_write_i32(
  state: &mut OpState,
  rid: ResourceId,
  key: *mut c_void,
  ptr: *mut c_void,
  offset: isize,
  value: i32,
) -> Result<(), AnyError> {
  check_token(state, rid, key)?;
  Ok(())
}

#[op(fast)]
pub fn op_ffi_token_write_f32(
  state: &mut OpState,
  rid: ResourceId,
  key: *mut c_void,
  ptr: *mut c_void,
  offset: isize,
  value: f32,
) -> Result<(), AnyError> {
  check_token(state, rid, key)?;
  Ok(())
}

#[op(fast)]
pub fn op_ffi_token_write_f64(
  state: &mut OpState,
  rid: ResourceId,
  key: *mut c_void,
  ptr: *mut c_void,
  offset: isize,
  value: f64,
) -> Result<(), AnyError> {
  check_token(state, rid, key)?;
  Ok(())
}

#[op(fast)]
pub fn op_ffi_token_write_ptr(
  state: &mut OpState,
  rid: ResourceId,
  key: *mut c_void,
  ptr: *mut c_void,
  offset: isize,
  value: *mut c_void,
) -> Result<(), AnyError> {
  check_token(state, rid, key)?;
  Ok(())
}

#[op(fast)]
pub fn op_ffi_token_cstr_read(
  state: &mut OpState,
  rid: ResourceId,
  key: *mut c_void,
) -> Result<(), AnyError> {
  check_token(state, rid, key)?;
  Ok(())
}

#[op(fast)]
pub fn op_ffi_token_get_buf(
  state: &mut OpState,
  rid: ResourceId,
  key: *mut c_void,
) -> Result<(), AnyError> {
  check_token(state, rid, key)?;
  Ok(())
}

#[op(fast)]
pub fn op_ffi_token_buf_copy_into(
  state: &mut OpState,
  rid: ResourceId,
  key: *mut c_void,
) -> Result<(), AnyError> {
  check_token(state, rid, key)?;
  Ok(())
}

#[op(fast)]
pub fn op_ffi_token_unsafe_callback_create(
  state: &mut OpState,
  rid: ResourceId,
  key: *mut c_void,
) -> Result<(), AnyError> {
  check_token(state, rid, key)?;
  Ok(())
}

#[op(fast)]
pub fn op_ffi_token_unsafe_callback_close(
  state: &mut OpState,
  rid: ResourceId,
  key: *mut c_void,
) -> Result<(), AnyError> {
  check_token(state, rid, key)?;
  Ok(())
}

#[op(fast)]
pub fn op_ffi_token_call_ptr_nonblocking(
  state: &mut OpState,
  rid: ResourceId,
  key: *mut c_void,
) -> Result<(), AnyError> {
  check_token(state, rid, key)?;
  Ok(())
}

#[op(fast)]
pub fn op_ffi_token_call_ptr(
  state: &mut OpState,
  rid: ResourceId,
  key: *mut c_void,
) -> Result<(), AnyError> {
  check_token(state, rid, key)?;
  Ok(())
}
