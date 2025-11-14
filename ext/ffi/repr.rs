// Copyright 2018-2025 the Deno authors. MIT license.

use std::ffi::CStr;
use std::ffi::c_char;
use std::ffi::c_void;
use std::ptr;

use deno_core::OpState;
use deno_core::op2;
use deno_core::v8;
use deno_permissions::PermissionsContainer;

#[derive(Debug, thiserror::Error, deno_error::JsError)]
#[class(type)]
pub enum ReprError {
  #[error("Invalid pointer to offset, pointer is null")]
  InvalidOffset,
  #[error("Invalid ArrayBuffer pointer, pointer is null")]
  InvalidArrayBuffer,
  #[error("Destination length is smaller than source length")]
  DestinationLengthTooShort,
  #[error("Invalid CString pointer, pointer is null")]
  InvalidCString,
  #[error("Invalid CString pointer, string exceeds max length")]
  CStringTooLong,
  #[error("Invalid bool pointer, pointer is null")]
  InvalidBool,
  #[error("Invalid u8 pointer, pointer is null")]
  InvalidU8,
  #[error("Invalid i8 pointer, pointer is null")]
  InvalidI8,
  #[error("Invalid u16 pointer, pointer is null")]
  InvalidU16,
  #[error("Invalid i16 pointer, pointer is null")]
  InvalidI16,
  #[error("Invalid u32 pointer, pointer is null")]
  InvalidU32,
  #[error("Invalid i32 pointer, pointer is null")]
  InvalidI32,
  #[error("Invalid u64 pointer, pointer is null")]
  InvalidU64,
  #[error("Invalid i64 pointer, pointer is null")]
  InvalidI64,
  #[error("Invalid f32 pointer, pointer is null")]
  InvalidF32,
  #[error("Invalid f64 pointer, pointer is null")]
  InvalidF64,
  #[error("Invalid pointer pointer, pointer is null")]
  InvalidPointer,
  #[class(inherit)]
  #[error(transparent)]
  Permission(#[from] deno_permissions::PermissionCheckError),
}

#[op2(fast, stack_trace)]
pub fn op_ffi_ptr_create(
  state: &mut OpState,
  #[bigint] ptr_number: usize,
) -> Result<*mut c_void, ReprError> {
  let permissions = state.borrow_mut::<PermissionsContainer>();
  permissions.check_ffi_partial_no_path()?;

  Ok(ptr_number as *mut c_void)
}

#[op2(fast, stack_trace)]
pub fn op_ffi_ptr_equals(
  state: &mut OpState,
  a: *const c_void,
  b: *const c_void,
) -> Result<bool, ReprError> {
  let permissions = state.borrow_mut::<PermissionsContainer>();
  permissions.check_ffi_partial_no_path()?;

  Ok(a == b)
}

#[op2(stack_trace)]
pub fn op_ffi_ptr_of(
  state: &mut OpState,
  #[anybuffer] buf: *const u8,
) -> Result<*mut c_void, ReprError> {
  let permissions = state.borrow_mut::<PermissionsContainer>();
  permissions.check_ffi_partial_no_path()?;

  Ok(buf as *mut c_void)
}

#[op2(fast, stack_trace)]
pub fn op_ffi_ptr_of_exact(
  state: &mut OpState,
  buf: v8::Local<v8::ArrayBufferView>,
) -> Result<*mut c_void, ReprError> {
  let permissions = state.borrow_mut::<PermissionsContainer>();
  permissions.check_ffi_partial_no_path()?;

  let Some(buf) = buf.get_backing_store() else {
    return Ok(0 as _);
  };
  let Some(buf) = buf.data() else {
    return Ok(0 as _);
  };
  Ok(buf.as_ptr() as _)
}

#[op2(fast, stack_trace)]
pub fn op_ffi_ptr_offset(
  state: &mut OpState,
  ptr: *mut c_void,
  #[number] offset: isize,
) -> Result<*mut c_void, ReprError> {
  let permissions = state.borrow_mut::<PermissionsContainer>();
  permissions.check_ffi_partial_no_path()?;

  if ptr.is_null() {
    return Err(ReprError::InvalidOffset);
  }

  // TODO(mmastrac): Create a RawPointer that can safely do pointer math.

  // SAFETY: Using `ptr.offset` is *actually unsafe* and has generated UB, but our FFI code relies on this working so we're going to
  // try and ask the compiler to be less undefined here by using `ptr.wrapping_offset`.
  Ok(ptr.wrapping_offset(offset))
}

unsafe extern "C" fn noop_deleter_callback(
  _data: *mut c_void,
  _byte_length: usize,
  _deleter_data: *mut c_void,
) {
}

#[op2(fast, stack_trace)]
#[bigint]
pub fn op_ffi_ptr_value(
  state: &mut OpState,
  ptr: *mut c_void,
) -> Result<usize, ReprError> {
  let permissions = state.borrow_mut::<PermissionsContainer>();
  permissions.check_ffi_partial_no_path()?;

  Ok(ptr as usize)
}

#[op2(stack_trace)]
pub fn op_ffi_get_buf<'scope>(
  scope: &mut v8::PinScope<'scope, '_>,
  state: &mut OpState,
  ptr: *mut c_void,
  #[number] offset: isize,
  #[number] len: usize,
) -> Result<v8::Local<'scope, v8::ArrayBuffer>, ReprError> {
  let permissions = state.borrow_mut::<PermissionsContainer>();
  permissions.check_ffi_partial_no_path()?;

  if ptr.is_null() {
    return Err(ReprError::InvalidArrayBuffer);
  }

  // SAFETY: Trust the user to have provided a real pointer, offset, and a valid matching size to it. Since this is a foreign pointer, we should not do any deletion.
  let backing_store = unsafe {
    v8::ArrayBuffer::new_backing_store_from_ptr(
      ptr.offset(offset),
      len,
      noop_deleter_callback,
      ptr::null_mut(),
    )
  }
  .make_shared();
  let array_buffer = v8::ArrayBuffer::with_backing_store(scope, &backing_store);
  Ok(array_buffer)
}

#[op2(stack_trace)]
pub fn op_ffi_buf_copy_into(
  state: &mut OpState,
  src: *mut c_void,
  #[number] offset: isize,
  #[anybuffer] dst: &mut [u8],
  #[number] len: usize,
) -> Result<(), ReprError> {
  let permissions = state.borrow_mut::<PermissionsContainer>();
  permissions.check_ffi_partial_no_path()?;

  if src.is_null() {
    Err(ReprError::InvalidArrayBuffer)
  } else if dst.len() < len {
    Err(ReprError::DestinationLengthTooShort)
  } else {
    let src = src as *const c_void;

    // SAFETY: src and offset are user defined.
    // dest is properly aligned and is valid for writes of len * size_of::<T>() bytes.
    unsafe {
      ptr::copy::<u8>(src.offset(offset) as *const u8, dst.as_mut_ptr(), len)
    };
    Ok(())
  }
}

#[op2(stack_trace)]
pub fn op_ffi_cstr_read<'scope>(
  scope: &mut v8::PinScope<'scope, '_>,
  state: &mut OpState,
  ptr: *mut c_void,
  #[number] offset: isize,
) -> Result<v8::Local<'scope, v8::String>, ReprError> {
  let permissions = state.borrow_mut::<PermissionsContainer>();
  permissions.check_ffi_partial_no_path()?;

  if ptr.is_null() {
    return Err(ReprError::InvalidCString);
  }

  let cstr =
  // SAFETY: Pointer and offset are user provided.
    unsafe { CStr::from_ptr(ptr.offset(offset) as *const c_char) }.to_bytes();
  #[allow(clippy::unnecessary_lazy_evaluations)]
  let value = v8::String::new_from_utf8(scope, cstr, v8::NewStringType::Normal)
    .ok_or_else(|| ReprError::CStringTooLong)?;
  Ok(value)
}

#[op2(fast, stack_trace)]
pub fn op_ffi_read_bool(
  state: &mut OpState,
  ptr: *mut c_void,
  #[number] offset: isize,
) -> Result<bool, ReprError> {
  let permissions = state.borrow_mut::<PermissionsContainer>();
  permissions.check_ffi_partial_no_path()?;

  if ptr.is_null() {
    return Err(ReprError::InvalidBool);
  }

  // SAFETY: ptr and offset are user provided.
  Ok(unsafe { ptr::read_unaligned::<bool>(ptr.offset(offset) as *const bool) })
}

#[op2(fast, stack_trace)]
pub fn op_ffi_read_u8(
  state: &mut OpState,
  ptr: *mut c_void,
  #[number] offset: isize,
) -> Result<u32, ReprError> {
  let permissions = state.borrow_mut::<PermissionsContainer>();
  permissions.check_ffi_partial_no_path()?;

  if ptr.is_null() {
    return Err(ReprError::InvalidU8);
  }

  // SAFETY: ptr and offset are user provided.
  Ok(unsafe {
    ptr::read_unaligned::<u8>(ptr.offset(offset) as *const u8) as u32
  })
}

#[op2(fast, stack_trace)]
pub fn op_ffi_read_i8(
  state: &mut OpState,
  ptr: *mut c_void,
  #[number] offset: isize,
) -> Result<i32, ReprError> {
  let permissions = state.borrow_mut::<PermissionsContainer>();
  permissions.check_ffi_partial_no_path()?;

  if ptr.is_null() {
    return Err(ReprError::InvalidI8);
  }

  // SAFETY: ptr and offset are user provided.
  Ok(unsafe {
    ptr::read_unaligned::<i8>(ptr.offset(offset) as *const i8) as i32
  })
}

#[op2(fast, stack_trace)]
pub fn op_ffi_read_u16(
  state: &mut OpState,
  ptr: *mut c_void,
  #[number] offset: isize,
) -> Result<u32, ReprError> {
  let permissions = state.borrow_mut::<PermissionsContainer>();
  permissions.check_ffi_partial_no_path()?;

  if ptr.is_null() {
    return Err(ReprError::InvalidU16);
  }

  // SAFETY: ptr and offset are user provided.
  Ok(unsafe {
    ptr::read_unaligned::<u16>(ptr.offset(offset) as *const u16) as u32
  })
}

#[op2(fast, stack_trace)]
pub fn op_ffi_read_i16(
  state: &mut OpState,
  ptr: *mut c_void,
  #[number] offset: isize,
) -> Result<i32, ReprError> {
  let permissions = state.borrow_mut::<PermissionsContainer>();
  permissions.check_ffi_partial_no_path()?;

  if ptr.is_null() {
    return Err(ReprError::InvalidI16);
  }

  // SAFETY: ptr and offset are user provided.
  Ok(unsafe {
    ptr::read_unaligned::<i16>(ptr.offset(offset) as *const i16) as i32
  })
}

#[op2(fast, stack_trace)]
pub fn op_ffi_read_u32(
  state: &mut OpState,
  ptr: *mut c_void,
  #[number] offset: isize,
) -> Result<u32, ReprError> {
  let permissions = state.borrow_mut::<PermissionsContainer>();
  permissions.check_ffi_partial_no_path()?;

  if ptr.is_null() {
    return Err(ReprError::InvalidU32);
  }

  // SAFETY: ptr and offset are user provided.
  Ok(unsafe { ptr::read_unaligned::<u32>(ptr.offset(offset) as *const u32) })
}

#[op2(fast, stack_trace)]
pub fn op_ffi_read_i32(
  state: &mut OpState,
  ptr: *mut c_void,
  #[number] offset: isize,
) -> Result<i32, ReprError> {
  let permissions = state.borrow_mut::<PermissionsContainer>();
  permissions.check_ffi_partial_no_path()?;

  if ptr.is_null() {
    return Err(ReprError::InvalidI32);
  }

  // SAFETY: ptr and offset are user provided.
  Ok(unsafe { ptr::read_unaligned::<i32>(ptr.offset(offset) as *const i32) })
}

#[op2(fast, stack_trace)]
#[bigint]
pub fn op_ffi_read_u64(
  state: &mut OpState,
  ptr: *mut c_void,
  // Note: The representation of 64-bit integers is function-wide. We cannot
  // choose to take this parameter as a number while returning a bigint.
  #[bigint] offset: isize,
) -> Result<u64, ReprError> {
  let permissions = state.borrow_mut::<PermissionsContainer>();
  permissions.check_ffi_partial_no_path()?;

  if ptr.is_null() {
    return Err(ReprError::InvalidU64);
  }

  let value =
  // SAFETY: ptr and offset are user provided.
    unsafe { ptr::read_unaligned::<u64>(ptr.offset(offset) as *const u64) };

  Ok(value)
}

#[op2(fast, stack_trace)]
#[bigint]
pub fn op_ffi_read_i64(
  state: &mut OpState,
  ptr: *mut c_void,
  // Note: The representation of 64-bit integers is function-wide. We cannot
  // choose to take this parameter as a number while returning a bigint.
  #[bigint] offset: isize,
) -> Result<i64, ReprError> {
  let permissions = state.borrow_mut::<PermissionsContainer>();
  permissions.check_ffi_partial_no_path()?;

  if ptr.is_null() {
    return Err(ReprError::InvalidI64);
  }

  let value =
  // SAFETY: ptr and offset are user provided.
    unsafe { ptr::read_unaligned::<i64>(ptr.offset(offset) as *const i64) };
  // SAFETY: Length and alignment of out slice were asserted to be correct.
  Ok(value)
}

#[op2(fast, stack_trace)]
pub fn op_ffi_read_f32(
  state: &mut OpState,
  ptr: *mut c_void,
  #[number] offset: isize,
) -> Result<f32, ReprError> {
  let permissions = state.borrow_mut::<PermissionsContainer>();
  permissions.check_ffi_partial_no_path()?;

  if ptr.is_null() {
    return Err(ReprError::InvalidF32);
  }

  // SAFETY: ptr and offset are user provided.
  Ok(unsafe { ptr::read_unaligned::<f32>(ptr.offset(offset) as *const f32) })
}

#[op2(fast, stack_trace)]
pub fn op_ffi_read_f64(
  state: &mut OpState,
  ptr: *mut c_void,
  #[number] offset: isize,
) -> Result<f64, ReprError> {
  let permissions = state.borrow_mut::<PermissionsContainer>();
  permissions.check_ffi_partial_no_path()?;

  if ptr.is_null() {
    return Err(ReprError::InvalidF64);
  }

  // SAFETY: ptr and offset are user provided.
  Ok(unsafe { ptr::read_unaligned::<f64>(ptr.offset(offset) as *const f64) })
}

#[op2(fast, stack_trace)]
pub fn op_ffi_read_ptr(
  state: &mut OpState,
  ptr: *mut c_void,
  #[number] offset: isize,
) -> Result<*mut c_void, ReprError> {
  let permissions = state.borrow_mut::<PermissionsContainer>();
  permissions.check_ffi_partial_no_path()?;

  if ptr.is_null() {
    return Err(ReprError::InvalidPointer);
  }

  // SAFETY: ptr and offset are user provided.
  Ok(unsafe {
    ptr::read_unaligned::<*mut c_void>(ptr.offset(offset) as *const *mut c_void)
  })
}
