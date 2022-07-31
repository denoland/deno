// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

#![allow(clippy::undocumented_unsafe_blocks)]

use std::os::raw::c_void;
use std::thread::sleep;
use std::time::Duration;

static BUFFER: [u8; 8] = [1, 2, 3, 4, 5, 6, 7, 8];

#[no_mangle]
pub extern "C" fn print_something() {
  println!("something");
}

/// # Safety
///
/// The pointer to the buffer must be valid and initalized, and the length must
/// not be longer than the buffer's allocation.
#[no_mangle]
pub unsafe extern "C" fn print_buffer(ptr: *const u8, len: usize) {
  let buf = std::slice::from_raw_parts(ptr, len);
  println!("{:?}", buf);
}

/// # Safety
///
/// The pointer to the buffer must be valid and initalized, and the length must
/// not be longer than the buffer's allocation.
#[no_mangle]
pub unsafe extern "C" fn print_buffer2(
  ptr1: *const u8,
  len1: usize,
  ptr2: *const u8,
  len2: usize,
) {
  let buf1 = std::slice::from_raw_parts(ptr1, len1);
  let buf2 = std::slice::from_raw_parts(ptr2, len2);
  println!("{:?} {:?}", buf1, buf2);
}

#[no_mangle]
pub extern "C" fn return_buffer() -> *const u8 {
  BUFFER.as_ptr()
}

#[no_mangle]
pub extern "C" fn is_null_ptr(ptr: *const u8) -> u8 {
  ptr.is_null() as u8
}

#[no_mangle]
pub extern "C" fn add_u32(a: u32, b: u32) -> u32 {
  a + b
}

#[no_mangle]
pub extern "C" fn add_i32(a: i32, b: i32) -> i32 {
  a + b
}

#[no_mangle]
pub extern "C" fn add_u64(a: u64, b: u64) -> u64 {
  a + b
}

#[no_mangle]
pub extern "C" fn add_i64(a: i64, b: i64) -> i64 {
  a + b
}

#[no_mangle]
pub extern "C" fn add_usize(a: usize, b: usize) -> usize {
  a + b
}

#[no_mangle]
pub extern "C" fn add_usize_fast(a: usize, b: usize) -> u32 {
  (a + b) as u32
}

#[no_mangle]
pub extern "C" fn add_isize(a: isize, b: isize) -> isize {
  a + b
}

#[no_mangle]
pub extern "C" fn add_f32(a: f32, b: f32) -> f32 {
  a + b
}

#[no_mangle]
pub extern "C" fn add_f64(a: f64, b: f64) -> f64 {
  a + b
}

#[no_mangle]
unsafe extern "C" fn hash(ptr: *const u8, length: u32) -> u32 {
  let buf = std::slice::from_raw_parts(ptr, length as usize);
  let mut hash: u32 = 0;
  for byte in buf {
    hash = hash.wrapping_mul(0x10001000).wrapping_add(*byte as u32);
  }
  hash
}

#[no_mangle]
pub extern "C" fn sleep_blocking(ms: u64) {
  let duration = Duration::from_millis(ms);
  sleep(duration);
}

/// # Safety
///
/// The pointer to the buffer must be valid and initalized, and the length must
/// not be longer than the buffer's allocation.
#[no_mangle]
pub unsafe extern "C" fn fill_buffer(value: u8, buf: *mut u8, len: usize) {
  let buf = std::slice::from_raw_parts_mut(buf, len);
  for itm in buf.iter_mut() {
    *itm = value;
  }
}

/// # Safety
///
/// The pointer to the buffer must be valid and initalized, and the length must
/// not be longer than the buffer's allocation.
#[no_mangle]
pub unsafe extern "C" fn nonblocking_buffer(ptr: *const u8, len: usize) {
  let buf = std::slice::from_raw_parts(ptr, len);
  assert_eq!(buf, vec![1, 2, 3, 4, 5, 6, 7, 8]);
}

#[no_mangle]
pub extern "C" fn get_add_u32_ptr() -> *const c_void {
  add_u32 as *const c_void
}

#[no_mangle]
pub extern "C" fn get_sleep_blocking_ptr() -> *const c_void {
  sleep_blocking as *const c_void
}

#[no_mangle]
pub extern "C" fn call_fn_ptr(func: Option<extern "C" fn()>) {
  if func.is_none() {
    return;
  }
  let func = func.unwrap();
  func();
}

#[no_mangle]
pub extern "C" fn call_fn_ptr_many_parameters(
  func: Option<
    extern "C" fn(u8, i8, u16, i16, u32, i32, u64, i64, f32, f64, *const u8),
  >,
) {
  if func.is_none() {
    return;
  }
  let func = func.unwrap();
  func(1, -1, 2, -2, 3, -3, 4, -4, 0.5, -0.5, BUFFER.as_ptr());
}

#[no_mangle]
pub extern "C" fn call_fn_ptr_return_u8(func: Option<extern "C" fn() -> u8>) {
  if func.is_none() {
    return;
  }
  let func = func.unwrap();
  println!("u8: {}", func());
}

#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[no_mangle]
pub extern "C" fn call_fn_ptr_return_buffer(
  func: Option<extern "C" fn() -> *const u8>,
) {
  if func.is_none() {
    return;
  }
  let func = func.unwrap();
  let ptr = func();
  let buf = unsafe { std::slice::from_raw_parts(ptr, 8) };
  println!("buf: {:?}", buf);
}

static mut STORED_FUNCTION: Option<extern "C" fn()> = None;
static mut STORED_FUNCTION_2: Option<extern "C" fn(u8) -> u8> = None;

#[no_mangle]
pub extern "C" fn store_function(func: Option<extern "C" fn()>) {
  unsafe { STORED_FUNCTION = func };
  if func.is_none() {
    println!("STORED_FUNCTION cleared");
  }
}

#[no_mangle]
pub extern "C" fn store_function_2(func: Option<extern "C" fn(u8) -> u8>) {
  unsafe { STORED_FUNCTION_2 = func };
  if func.is_none() {
    println!("STORED_FUNCTION_2 cleared");
  }
}

#[no_mangle]
pub extern "C" fn call_stored_function() {
  unsafe {
    if STORED_FUNCTION.is_none() {
      return;
    }
    STORED_FUNCTION.unwrap()();
  }
}

#[no_mangle]
pub extern "C" fn call_stored_function_2(arg: u8) {
  unsafe {
    if STORED_FUNCTION_2.is_none() {
      return;
    }
    println!("{}", STORED_FUNCTION_2.unwrap()(arg));
  }
}

#[no_mangle]
pub extern "C" fn call_stored_function_thread_safe() {
  std::thread::spawn(move || {
    std::thread::sleep(std::time::Duration::from_millis(1500));
    unsafe {
      if STORED_FUNCTION.is_none() {
        return;
      }
      STORED_FUNCTION.unwrap()();
    }
  });
}

#[no_mangle]
pub extern "C" fn call_stored_function_thread_safe_and_log() {
  std::thread::spawn(move || {
    std::thread::sleep(std::time::Duration::from_millis(1500));
    unsafe {
      if STORED_FUNCTION.is_none() {
        return;
      }
      STORED_FUNCTION.unwrap()();
      println!("STORED_FUNCTION called");
    }
  });
}

// FFI performance helper functions
#[no_mangle]
pub extern "C" fn nop() {}

#[no_mangle]
pub extern "C" fn nop_u8(_a: u8) {}

#[no_mangle]
pub extern "C" fn nop_i8(_a: i8) {}

#[no_mangle]
pub extern "C" fn nop_u16(_a: u16) {}

#[no_mangle]
pub extern "C" fn nop_i16(_a: i16) {}

#[no_mangle]
pub extern "C" fn nop_u32(_a: u32) {}

#[no_mangle]
pub extern "C" fn nop_i32(_a: i32) {}

#[no_mangle]
pub extern "C" fn nop_u64(_a: u64) {}

#[no_mangle]
pub extern "C" fn nop_i64(_a: i64) {}

#[no_mangle]
pub extern "C" fn nop_usize(_a: usize) {}

#[no_mangle]
pub extern "C" fn nop_isize(_a: isize) {}

#[no_mangle]
pub extern "C" fn nop_f32(_a: f32) {}

#[no_mangle]
pub extern "C" fn nop_f64(_a: f64) {}

#[no_mangle]
pub extern "C" fn nop_buffer(_buffer: *mut [u8; 8]) {}

#[no_mangle]
pub extern "C" fn return_u8() -> u8 {
  255
}

#[no_mangle]
pub extern "C" fn return_i8() -> i8 {
  -128
}

#[no_mangle]
pub extern "C" fn return_u16() -> u16 {
  65535
}

#[no_mangle]
pub extern "C" fn return_i16() -> i16 {
  -32768
}

#[no_mangle]
pub extern "C" fn return_u32() -> u32 {
  4294967295
}

#[no_mangle]
pub extern "C" fn return_i32() -> i32 {
  -2147483648
}

#[no_mangle]
pub extern "C" fn return_u64() -> u64 {
  18446744073709551615
}

#[no_mangle]
pub extern "C" fn return_i64() -> i64 {
  -9223372036854775808
}

#[no_mangle]
pub extern "C" fn return_usize() -> usize {
  18446744073709551615
}

#[no_mangle]
pub extern "C" fn return_isize() -> isize {
  -9223372036854775808
}

#[no_mangle]
pub extern "C" fn return_f32() -> f32 {
  #[allow(clippy::excessive_precision)]
  0.20298023223876953125
}

#[no_mangle]
pub extern "C" fn return_f64() -> f64 {
  1e-10
}

// Parameters iteration

#[no_mangle]
pub extern "C" fn nop_many_parameters(
  _: u8,
  _: i8,
  _: u16,
  _: i16,
  _: u32,
  _: i32,
  _: u64,
  _: i64,
  _: usize,
  _: isize,
  _: f32,
  _: f64,
  _: *mut [u8; 8],
  _: u8,
  _: i8,
  _: u16,
  _: i16,
  _: u32,
  _: i32,
  _: u64,
  _: i64,
  _: usize,
  _: isize,
  _: f32,
  _: f64,
  _: *mut [u8; 8],
) {
}

// Statics
#[no_mangle]
pub static static_u32: u32 = 42;

#[no_mangle]
pub static static_i64: i64 = -1242464576485;

#[repr(C)]
pub struct Structure {
  _data: u32,
}

#[no_mangle]
pub static mut static_ptr: Structure = Structure { _data: 42 };

static STRING: &str = "Hello, world!\0";

#[no_mangle]
extern "C" fn ffi_string() -> *const u8 {
  STRING.as_ptr()
}
