use std::env;
use std::ffi::CString;
use std::os::raw::c_char;
use std::path::PathBuf;


/// Place this in your build.rs to link the libdeno static library
pub fn build_setup(libdeno_path: Option<PathBuf>) {
    let libdeno_path = match libdeno_path {
        Some(libdeno_path) => libdeno_path,
        None => PathBuf::from(env::var("LIBDENO_PATH").expect("Cannot find libdeno")),
    };

    println!("cargo:rustc-link-search=native={}", libdeno_path.to_str().expect("Cannot convert LIBDENO_PATH to string"));
    println!("cargo:rustc-link-arg=-Wl,--allow-multiple-definition");

    println!("cargo:rustc-link-lib=static=deno");

    println!("cargo:rustc-link-lib=dylib=pthread");
    println!("cargo:rustc-link-lib=dylib=dl");
    println!("cargo:rustc-link-lib=dylib=m");
    
    println!("cargo:rustc-link-lib=dylib=stdc++");
}

pub fn deno_embedded_eval(code: &str) -> i32 {
    let c_code = CString::new(code).expect("CString conversion failed");

    unsafe extern "C" {
        fn deno_embedded_eval(code: *const c_char) -> i32;
    }

    unsafe {
        deno_embedded_eval(c_code.as_ptr())
    }
}
