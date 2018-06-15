extern crate libc;
use libc::c_char;
use libc::c_int;
use std::ffi::CStr;
use std::ffi::CString;

#[link(name = "deno", kind = "static")]
extern "C" {
    fn deno_v8_version() -> *const c_char;
    fn deno_init();

    // Note: `deno_set_flags` actually takes `char**` as it's second argument,
    // not `const char**`, so this is technically incorrect. However it doesn't
    // actually modify the contents of the strings, so it's not unsafe.
    // TODO: use the correct function signature.
    fn deno_set_flags(argc: *mut c_int, argv: *mut *const c_char);
}

fn set_flags() {
    // Create a vector of zero terminated c strings.
    let mut argv = std::env::args()
        .map(|arg| CString::new(arg).unwrap().as_ptr())
        .collect::<Vec<_>>();
    let mut argc = argv.len() as c_int;
    unsafe {
        // pass the pointer of the vector's internal buffer to a C function
        deno_set_flags(&mut argc, argv.as_mut_ptr());
    };
}

fn main() {
    println!("Hi");
    set_flags();
    unsafe { deno_init() };
    let v = unsafe { deno_v8_version() };
    let c_str = unsafe { CStr::from_ptr(v) };
    let version = c_str.to_str().unwrap();
    println!("version: {}", version);
}
