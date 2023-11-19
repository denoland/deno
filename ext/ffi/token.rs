// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use deno_core::error::range_error;
use deno_core::error::type_error;
use deno_core::error::AnyError;
use deno_core::external;
use deno_core::op2;
use deno_core::v8;
use deno_core::ExternalPointer;
use deno_core::OpState;
use std::cell::RefCell;
use std::ffi::c_char;
use std::ffi::c_void;
use std::ffi::CStr;
use std::future::Future;
use std::path::PathBuf;
use std::ptr::{self};
use std::rc::Rc;

use crate::call::call_ptr;
use crate::call::call_ptr_nonblocking;
use crate::call::FfiValue;
use crate::callback::unsafe_callback_create;
use crate::callback::RegisterCallbackArgs;
use crate::check_unstable;
use crate::dlfcn::ForeignFunction;
use crate::repr::noop_deleter_callback;
use crate::FfiPermissions;

thread_local! {
  static FFI_TOKEN_TABLE: RefCell<Vec<Option<*const c_void>>> = RefCell::new(vec![]);
}

struct FfiToken {
  index: usize,
}

external!(FfiToken, "FFI token");

#[inline(always)]
fn check_token(token: *const c_void) {
  let &FfiToken {
    index
  } =
  // SAFETY: We do not use the reference at all. We deref only for the purpose of
  // making sure that the token pointer we got contained an FFI token and was not
  // some other pointer instead. If another pointer was passed, then crashing is okay
  // as it means that someone attempted to spoof FFI tokens.
    unsafe { ExternalPointer::<FfiToken>::from_raw(token).unsafely_deref() };
  FFI_TOKEN_TABLE.with(|cell| {
    let v = cell.borrow();
    assert_eq!(v[index], Some(token));
  })
}

#[op2(fast)]
pub fn op_ffi_create_token<FP>(
  state: &mut OpState,
  #[string] path: String,
) -> Result<*const c_void, AnyError>
where
  FP: FfiPermissions + 'static,
{
  check_unstable(state, "Deno.createFfiToken");
  let permissions = state.borrow_mut::<FP>();
  permissions.check_partial(Some(&PathBuf::from(&path)))?;

  let token = FFI_TOKEN_TABLE.with(|cell| {
    let mut v = cell.borrow_mut();
    let index = v.len();
    let token: *const c_void =
      ExternalPointer::new(FfiToken { index }).into_raw();
    v.push(Some(token));
    token
  });

  Ok(token)
}

#[op2(fast)]
pub fn op_ffi_token_close(token: *const c_void) {
  let FfiToken {
    index
  } =
    // SAFETY: User initiated close of the token.
    unsafe { ExternalPointer::<FfiToken>::from_raw(token).unsafely_take() };
  FFI_TOKEN_TABLE.with(|cell| {
    let mut v = cell.borrow_mut();
    v[index] = None;
  });
}

#[op2(fast)]
pub fn op_ffi_token_ptr_create(
  state: &mut OpState,
  token: *const c_void,
  #[bigint] ptr_number: usize,
) -> Result<*mut c_void, AnyError> {
  check_unstable(state, "TokenizedPointer#create");
  check_token(token);

  Ok(ptr_number as *mut c_void)
}

#[op2(fast)]
pub fn op_ffi_token_ptr_equals(
  state: &mut OpState,
  token: *const c_void,
  a: *const c_void,
  b: *const c_void,
) -> bool {
  check_unstable(state, "TokenizedPointer#equals");
  check_token(token);

  a == b
}

#[op2(fast)]
pub fn op_ffi_token_ptr_of(
  state: &mut OpState,
  token: *const c_void,
  #[anybuffer] buf: *const u8,
) -> Result<*mut c_void, AnyError> {
  check_unstable(state, "TokenizedPointer#of");
  check_token(token);

  Ok(buf as *mut c_void)
}

#[op2(fast)]
pub fn op_ffi_token_ptr_of_exact(
  state: &mut OpState,
  token: *const c_void,
  buf: v8::Local<v8::ArrayBufferView>,
) -> Result<*mut c_void, AnyError> {
  check_unstable(state, "TokenizedPointer#of");
  check_token(token);

  let Some(buf) = buf.get_backing_store() else {
    return Ok(0 as _);
  };
  let Some(buf) = buf.data() else {
    return Ok(0 as _);
  };
  Ok(buf.as_ptr() as _)
}

#[op2(fast)]
pub fn op_ffi_token_ptr_offset(
  state: &mut OpState,
  token: *const c_void,
  ptr: *mut c_void,
  #[number] offset: isize,
) -> Result<*mut c_void, AnyError> {
  check_unstable(state, "TokenizedPointer#offset");
  check_token(token);

  if ptr.is_null() {
    return Err(type_error("Invalid pointer to offset, pointer is null"));
  }

  // TODO(mmastrac): Create a RawPointer that can safely do pointer math.

  // SAFETY: Using `ptr.offset` is *actually unsafe* and has generated UB, but our FFI code relies on this working so we're going to
  // try and ask the compiler to be less undefined here by using `ptr.wrapping_offset`.
  Ok(ptr.wrapping_offset(offset))
}

#[op2(fast)]
#[bigint]
pub fn op_ffi_token_ptr_value(
  state: &mut OpState,
  token: *const c_void,
  ptr: *mut c_void,
) -> usize {
  check_unstable(state, "TokenizedPointer#value");
  check_token(token);

  ptr as usize
}

#[op2(fast)]
pub fn op_ffi_token_read_bool(
  state: &mut OpState,
  token: *const c_void,
  ptr: *mut c_void,
  offset: i32,
) -> Result<bool, AnyError> {
  check_unstable(state, "TokenizedPointerView#getBool");
  check_token(token);

  if ptr.is_null() {
    return Err(type_error("Invalid bool pointer, pointer is null"));
  }

  // SAFETY: ptr and offset are user provided.
  Ok(unsafe {
    ptr::read_unaligned::<bool>(ptr.offset(offset as isize) as *const bool)
  })
}

#[op2(fast)]
pub fn op_ffi_token_read_u8(
  state: &mut OpState,
  token: *const c_void,
  ptr: *mut c_void,
  offset: i32,
) -> Result<u8, AnyError> {
  check_unstable(state, "TokenizedPointerView#getUint8");
  check_token(token);

  if ptr.is_null() {
    return Err(type_error("Invalid u8 pointer, pointer is null"));
  }

  // SAFETY: ptr and offset are user provided.
  Ok(unsafe {
    ptr::read_unaligned::<u8>(ptr.offset(offset as isize) as *const u8)
  })
}

#[op2(fast)]
pub fn op_ffi_token_read_i8(
  state: &mut OpState,
  token: *const c_void,
  ptr: *mut c_void,
  offset: i32,
) -> Result<i8, AnyError> {
  check_unstable(state, "TokenizedPointerView#getInt8");
  check_token(token);

  if ptr.is_null() {
    return Err(type_error("Invalid i8 pointer, pointer is null"));
  }

  // SAFETY: ptr and offset are user provided.
  Ok(unsafe {
    ptr::read_unaligned::<i8>(ptr.offset(offset as isize) as *const i8)
  })
}

#[op2(fast)]
pub fn op_ffi_token_read_u16(
  state: &mut OpState,
  token: *const c_void,
  ptr: *mut c_void,
  offset: i32,
) -> Result<u16, AnyError> {
  check_unstable(state, "TokenizedPointerView#getUint16");
  check_token(token);

  if ptr.is_null() {
    return Err(type_error("Invalid u16 pointer, pointer is null"));
  }

  // SAFETY: ptr and offset are user provided.
  Ok(unsafe {
    ptr::read_unaligned::<u16>(ptr.offset(offset as isize) as *const u16)
  })
}

#[op2(fast)]
pub fn op_ffi_token_read_i16(
  state: &mut OpState,
  token: *const c_void,
  ptr: *mut c_void,
  offset: i32,
) -> Result<i16, AnyError> {
  check_unstable(state, "TokenizedPointerView#getInt16");
  check_token(token);

  if ptr.is_null() {
    return Err(type_error("Invalid i16 pointer, pointer is null"));
  }

  // SAFETY: ptr and offset are user provided.
  Ok(unsafe {
    ptr::read_unaligned::<i16>(ptr.offset(offset as isize) as *const i16)
  })
}

#[op2(fast)]
pub fn op_ffi_token_read_u32(
  state: &mut OpState,
  token: *const c_void,
  ptr: *mut c_void,
  offset: i32,
) -> Result<u32, AnyError> {
  check_unstable(state, "TokenizedPointerView#getUint32");
  check_token(token);

  if ptr.is_null() {
    return Err(type_error("Invalid u32 pointer, pointer is null"));
  }

  // SAFETY: ptr and offset are user provided.
  Ok(unsafe {
    ptr::read_unaligned::<u32>(ptr.offset(offset as isize) as *const u32)
  })
}

#[op2(fast)]
pub fn op_ffi_token_read_i32(
  state: &mut OpState,
  token: *const c_void,
  ptr: *mut c_void,
  offset: i32,
) -> Result<i32, AnyError> {
  check_unstable(state, "TokenizedPointerView#getInt32");
  check_token(token);

  if ptr.is_null() {
    return Err(type_error("Invalid i32 pointer, pointer is null"));
  }

  // SAFETY: ptr and offset are user provided.
  Ok(unsafe {
    ptr::read_unaligned::<i32>(ptr.offset(offset as isize) as *const i32)
  })
}

#[op2(fast)]
#[bigint]
pub fn op_ffi_token_read_u64(
  state: &mut OpState,
  token: *const c_void,
  ptr: *mut c_void,
  offset: i32,
) -> Result<u64, AnyError> {
  check_unstable(state, "TokenizedPointerView#getUint64");
  check_token(token);

  if ptr.is_null() {
    return Err(type_error("Invalid u64 pointer, pointer is null"));
  }

  // SAFETY: ptr and offset are user provided.
  Ok(unsafe {
    ptr::read_unaligned::<u64>(ptr.offset(offset as isize) as *const u64)
  })
}

#[op2(fast)]
#[bigint]
pub fn op_ffi_token_read_i64(
  state: &mut OpState,
  token: *const c_void,
  ptr: *mut c_void,
  offset: i32,
) -> Result<i64, AnyError> {
  check_unstable(state, "TokenizedPointerView#getInt64");
  check_token(token);

  if ptr.is_null() {
    return Err(type_error("Invalid i64 pointer, pointer is null"));
  }

  // SAFETY: ptr and offset are user provided.
  Ok(unsafe {
    ptr::read_unaligned::<i64>(ptr.offset(offset as isize) as *const i64)
  })
}

#[op2(fast)]
pub fn op_ffi_token_read_f32(
  state: &mut OpState,
  token: *const c_void,
  ptr: *mut c_void,
  offset: i32,
) -> Result<f32, AnyError> {
  check_unstable(state, "TokenizedPointerView#getFloat32");
  check_token(token);

  if ptr.is_null() {
    return Err(type_error("Invalid f32 pointer, pointer is null"));
  }

  // SAFETY: ptr and offset are user provided.
  Ok(unsafe {
    ptr::read_unaligned::<f32>(ptr.offset(offset as isize) as *const f32)
  })
}

#[op2(fast)]
pub fn op_ffi_token_read_f64(
  state: &mut OpState,
  token: *const c_void,
  ptr: *mut c_void,
  offset: i32,
) -> Result<f64, AnyError> {
  check_unstable(state, "TokenizedPointerView#getFloat64");
  check_token(token);

  if ptr.is_null() {
    return Err(type_error("Invalid f64 pointer, pointer is null"));
  }

  // SAFETY: ptr and offset are user provided.
  Ok(unsafe {
    ptr::read_unaligned::<f64>(ptr.offset(offset as isize) as *const f64)
  })
}

#[op2(fast)]
pub fn op_ffi_token_read_ptr(
  state: &mut OpState,
  token: *const c_void,
  ptr: *mut c_void,
  offset: i32,
) -> Result<*mut c_void, AnyError> {
  check_unstable(state, "TokenizedPointerView#getPointer");
  check_token(token);

  if ptr.is_null() {
    return Err(type_error("Invalid source pointer, pointer is null"));
  }

  // SAFETY: ptr and offset are user provided.
  Ok(unsafe {
    ptr::read_unaligned::<*mut c_void>(
      ptr.offset(offset as isize) as *const *mut c_void
    )
  })
}

#[op2(fast)]
pub fn op_ffi_token_write_bool(
  state: &mut OpState,
  token: *const c_void,
  ptr: *mut c_void,
  offset: i32,
  value: bool,
) -> Result<(), AnyError> {
  check_unstable(state, "TokenizedPointerView#setBool");
  check_token(token);

  if ptr.is_null() {
    return Err(type_error("Invalid bool pointer, pointer is null"));
  }
  // SAFETY: ptr and offset are user provided.
  unsafe {
    ptr::write_unaligned::<bool>(
      ptr.offset(offset as isize) as *mut bool,
      value,
    )
  }
  Ok(())
}

#[op2(fast)]
pub fn op_ffi_token_write_u8(
  state: &mut OpState,
  token: *const c_void,
  ptr: *mut c_void,
  offset: i32,
  value: u8,
) -> Result<(), AnyError> {
  check_unstable(state, "TokenizedPointerView#setUint8");
  check_token(token);

  if ptr.is_null() {
    return Err(type_error("Invalid u8 pointer, pointer is null"));
  }
  // SAFETY: ptr and offset are user provided.
  unsafe {
    ptr::write_unaligned::<u8>(ptr.offset(offset as isize) as *mut u8, value)
  }
  Ok(())
}

#[op2(fast)]
pub fn op_ffi_token_write_i8(
  state: &mut OpState,
  token: *const c_void,
  ptr: *mut c_void,
  offset: i32,
  value: i8,
) -> Result<(), AnyError> {
  check_unstable(state, "TokenizedPointerView#setInt8");
  check_token(token);

  if ptr.is_null() {
    return Err(type_error("Invalid i8 pointer, pointer is null"));
  }
  // SAFETY: ptr and offset are user provided.
  unsafe {
    ptr::write_unaligned::<i8>(ptr.offset(offset as isize) as *mut i8, value)
  }
  Ok(())
}

#[op2(fast)]
pub fn op_ffi_token_write_u16(
  state: &mut OpState,
  token: *const c_void,
  ptr: *mut c_void,
  offset: i32,
  value: u16,
) -> Result<(), AnyError> {
  check_unstable(state, "TokenizedPointerView#setUint16");
  check_token(token);

  if ptr.is_null() {
    return Err(type_error("Invalid u16 pointer, pointer is null"));
  }
  // SAFETY: ptr and offset are user provided.
  unsafe {
    ptr::write_unaligned::<u16>(ptr.offset(offset as isize) as *mut u16, value)
  }
  Ok(())
}

#[op2(fast)]
pub fn op_ffi_token_write_i16(
  state: &mut OpState,
  token: *const c_void,
  ptr: *mut c_void,
  offset: i32,
  value: i16,
) -> Result<(), AnyError> {
  check_unstable(state, "TokenizedPointerView#setInt16");
  check_token(token);

  if ptr.is_null() {
    return Err(type_error("Invalid i16 pointer, pointer is null"));
  }

  // SAFETY: ptr and offset are user provided.
  unsafe {
    ptr::write_unaligned::<i16>(ptr.offset(offset as isize) as *mut i16, value)
  }
  Ok(())
}

#[op2(fast)]
pub fn op_ffi_token_write_u32(
  state: &mut OpState,
  token: *const c_void,
  ptr: *mut c_void,
  offset: i32,
  value: u32,
) -> Result<(), AnyError> {
  check_unstable(state, "TokenizedPointerView#setUint32");
  check_token(token);

  if ptr.is_null() {
    return Err(type_error("Invalid u32 pointer, pointer is null"));
  }

  // SAFETY: ptr and offset are user provided.
  unsafe {
    ptr::write_unaligned::<u32>(ptr.offset(offset as isize) as *mut u32, value)
  }
  Ok(())
}

#[op2(fast)]
pub fn op_ffi_token_write_i32(
  state: &mut OpState,
  token: *const c_void,
  ptr: *mut c_void,
  offset: i32,
  value: i32,
) -> Result<(), AnyError> {
  check_unstable(state, "TokenizedPointerView#setInt32");
  check_token(token);

  if ptr.is_null() {
    return Err(type_error("Invalid i32 pointer, pointer is null"));
  }

  // SAFETY: ptr and offset are user provided.
  unsafe {
    ptr::write_unaligned::<i32>(ptr.offset(offset as isize) as *mut i32, value)
  }
  Ok(())
}

#[op2(fast)]
#[bigint]
pub fn op_ffi_token_write_u64(
  state: &mut OpState,
  token: *const c_void,
  ptr: *mut c_void,
  offset: i32,
  #[bigint] value: u64,
) -> Result<(), AnyError> {
  check_unstable(state, "TokenizedPointerView#setBigUint64");
  check_token(token);

  if ptr.is_null() {
    return Err(type_error("Invalid u64 pointer, pointer is null"));
  }

  // SAFETY: ptr and offset are user provided.
  unsafe {
    ptr::write_unaligned::<u64>(ptr.offset(offset as isize) as *mut u64, value)
  }
  Ok(())
}

#[op2(fast)]
#[bigint]
pub fn op_ffi_token_write_i64(
  state: &mut OpState,
  token: *const c_void,
  ptr: *mut c_void,
  offset: i32,
  #[bigint] value: i64,
) -> Result<(), AnyError> {
  check_unstable(state, "TokenizedPointerView#setBigInt64");
  check_token(token);

  if ptr.is_null() {
    return Err(type_error("Invalid i64 pointer, pointer is null"));
  }

  // SAFETY: ptr and offset are user provided.
  unsafe {
    ptr::write_unaligned::<i64>(ptr.offset(offset as isize) as *mut i64, value)
  }
  Ok(())
}

#[op2(fast)]
pub fn op_ffi_token_write_f32(
  state: &mut OpState,
  token: *const c_void,
  ptr: *mut c_void,
  offset: i32,
  value: f32,
) -> Result<(), AnyError> {
  check_unstable(state, "TokenizedPointerView#setFloat32");
  check_token(token);

  if ptr.is_null() {
    return Err(type_error("Invalid f32 pointer, pointer is null"));
  }

  // SAFETY: ptr and offset are user provided.
  unsafe {
    ptr::write_unaligned::<f32>(ptr.offset(offset as isize) as *mut f32, value)
  }
  Ok(())
}

#[op2(fast)]
pub fn op_ffi_token_write_f64(
  state: &mut OpState,
  token: *const c_void,
  ptr: *mut c_void,
  offset: i32,
  value: f64,
) -> Result<(), AnyError> {
  check_unstable(state, "TokenizedPointerView#setFloat64");
  check_token(token);

  if ptr.is_null() {
    return Err(type_error("Invalid f64 pointer, pointer is null"));
  }

  // SAFETY: ptr and offset are user provided.
  unsafe {
    ptr::write_unaligned::<f64>(ptr.offset(offset as isize) as *mut f64, value)
  }
  Ok(())
}

#[op2(fast)]
pub fn op_ffi_token_write_ptr(
  state: &mut OpState,
  token: *const c_void,
  ptr: *mut c_void,
  offset: i32,
  value: *mut c_void,
) -> Result<(), AnyError> {
  check_unstable(state, "TokenizedPointerView#setPointer");
  check_token(token);

  if ptr.is_null() {
    return Err(type_error("Invalid target pointer, pointer is null"));
  }

  // SAFETY: ptr and offset are user provided.
  unsafe {
    ptr::write_unaligned::<*mut c_void>(
      ptr.offset(offset as isize) as *mut *mut c_void,
      value,
    )
  }
  Ok(())
}

#[op2]
pub fn op_ffi_token_cstr_read<'scope>(
  scope: &mut v8::HandleScope<'scope>,
  state: &mut OpState,
  token: *const c_void,
  ptr: *mut c_void,
  #[number] offset: isize,
) -> Result<v8::Local<'scope, v8::String>, AnyError> {
  check_unstable(state, "TokenizedPointerView#getCString");
  check_token(token);

  if ptr.is_null() {
    return Err(type_error("Invalid CString pointer, pointer is null"));
  }

  let cstr =
  // SAFETY: Pointer and offset are user provided.
    unsafe { CStr::from_ptr(ptr.offset(offset) as *const c_char) }.to_bytes();
  let value = v8::String::new_from_utf8(scope, cstr, v8::NewStringType::Normal)
    .ok_or_else(|| {
      type_error("Invalid CString pointer, string exceeds max length")
    })?;
  Ok(value)
}

#[op2]
pub fn op_ffi_token_get_buf<'scope>(
  scope: &mut v8::HandleScope<'scope>,
  state: &mut OpState,
  token: *const c_void,
  ptr: *mut c_void,
  #[number] offset: isize,
  #[number] len: usize,
) -> Result<v8::Local<'scope, v8::ArrayBuffer>, AnyError> {
  check_unstable(state, "TokenizedPointerView#getArrayBuffer");
  check_token(token);

  if ptr.is_null() {
    return Err(type_error("Invalid ArrayBuffer pointer, pointer is null"));
  }

  // SAFETY: Trust the user to have provided a real pointer, offset, and a valid matching size to it. Since this is a foreign pointer, we should not do any deletion.
  let backing_store = unsafe {
    v8::ArrayBuffer::new_backing_store_from_ptr(
      ptr.offset(offset),
      len,
      noop_deleter_callback,
      std::ptr::null_mut(),
    )
  }
  .make_shared();
  let array_buffer = v8::ArrayBuffer::with_backing_store(scope, &backing_store);
  Ok(array_buffer)
}

#[op2(fast)]
pub fn op_ffi_token_buf_copy_into(
  state: &mut OpState,
  token: *const c_void,
  src: *mut c_void,
  #[number] offset: isize,
  #[anybuffer] dst: &mut [u8],
  #[number] len: usize,
) -> Result<(), AnyError> {
  check_unstable(state, "TokenizedPointerView#copyInto");
  check_token(token);

  if src.is_null() {
    Err(type_error("Invalid ArrayBuffer pointer, pointer is null"))
  } else if dst.len() < len {
    Err(range_error(
      "Destination length is smaller than source length",
    ))
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

#[op2]
pub fn op_ffi_token_unsafe_callback_create<'scope>(
  scope: &mut v8::HandleScope<'scope>,
  state: &mut OpState,
  token: *const c_void,
  #[serde] args: RegisterCallbackArgs,
  cb: v8::Local<v8::Function>,
) -> Result<v8::Local<'scope, v8::Value>, AnyError> {
  check_unstable(state, "TokenizedCallback");
  check_token(token);

  unsafe_callback_create(state, scope, args, cb)
}

#[op2(async)]
#[serde]
pub fn op_ffi_token_call_ptr_nonblocking(
  scope: &mut v8::HandleScope,
  state: Rc<RefCell<OpState>>,
  token: *const c_void,
  pointer: *mut c_void,
  #[serde] def: ForeignFunction,
  parameters: v8::Local<v8::Array>,
  out_buffer: Option<v8::Local<v8::TypedArray>>,
) -> Result<impl Future<Output = Result<FfiValue, AnyError>>, AnyError> {
  check_unstable(&state.borrow(), "TokenizedFnPointer#call");
  check_token(token);

  call_ptr_nonblocking(scope, pointer, def, parameters, out_buffer)
}

#[op2]
#[serde]
pub fn op_ffi_token_call_ptr(
  scope: &mut v8::HandleScope,
  state: Rc<RefCell<OpState>>,
  token: *const c_void,
  pointer: *mut c_void,
  #[serde] def: ForeignFunction,
  parameters: v8::Local<v8::Array>,
  out_buffer: Option<v8::Local<v8::TypedArray>>,
) -> Result<FfiValue, AnyError> {
  check_unstable(&state.borrow(), "TokenizedFnPointer#call");
  check_token(token);

  call_ptr(scope, pointer, def, parameters, out_buffer)
}
