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
  permissions.check(Some(&PathBuf::from(&path)))?;

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
  let resource = state.resource_table.get::<FfiTokenResource>(rid)?;
  if !std::ptr::eq(resource.ptr, key) {
    return Err(type_error("Attempted to use invalid FFI token"));
  }
  Ok(())
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
