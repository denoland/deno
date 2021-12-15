// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use std::thread::sleep;
use std::time::Duration;

static BUFFER: [u8; 8] = [1, 2, 3, 4, 5, 6, 7, 8];

#[no_mangle]
pub extern "C" fn print_something() {
  println!("something");
}

#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[no_mangle]
pub extern "C" fn print_buffer(ptr: *const u8, len: usize) {
  let buf = unsafe { std::slice::from_raw_parts(ptr, len) };
  println!("{:?}", buf);
}

#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[no_mangle]
pub extern "C" fn print_buffer2(
  ptr1: *const u8,
  len1: usize,
  ptr2: *const u8,
  len2: usize,
) {
  let buf1 = unsafe { std::slice::from_raw_parts(ptr1, len1) };
  let buf2 = unsafe { std::slice::from_raw_parts(ptr2, len2) };
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
pub extern "C" fn sleep_blocking(ms: u64) {
  let duration = Duration::from_millis(ms);
  sleep(duration);
}

#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[no_mangle]
pub extern "C" fn fill_buffer(value: u8, buf: *mut u8, len: usize) {
  let buf = unsafe { std::slice::from_raw_parts_mut(buf, len) };
  for itm in buf.iter_mut() {
    *itm = value;
  }
}

#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[no_mangle]
pub extern "C" fn nonblocking_buffer(ptr: *const u8, len: usize) {
  let buf = unsafe { std::slice::from_raw_parts(ptr, len) };
  assert_eq!(buf, vec![1, 2, 3, 4, 5, 6, 7, 8]);
}
