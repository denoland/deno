use std::ffi::CStr;
use std::os::raw::c_char;

#[no_mangle]
pub extern "C" fn print_something() {
  println!("something");
}

#[no_mangle]
pub unsafe extern "C" fn print_string(ptr: *const c_char) {
  let cstr = CStr::from_ptr(ptr);
  let name = cstr.to_str().unwrap();
  println!("{}", name);
}

#[no_mangle]
pub unsafe extern "C" fn print_buffer(ptr: *const u8, len: usize) {
  let buf = std::slice::from_raw_parts(ptr, len);
  println!("{:?}", buf);
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
