// Copyright 2018-2025 the Deno authors. MIT license.

// This test performs initialization similar to napi-rs.
// https://github.com/napi-rs/napi-rs/commit/a5a04a4e545f268769cc78e2bd6c45af4336aac3

use std::ffi::c_char;
use std::ffi::c_void;
use std::ptr;

use napi_sys as sys;

macro_rules! check_status_or_panic {
  ($code:expr, $msg:expr) => {{
    let c = $code;
    match c {
      sys::Status::napi_ok => {}
      _ => panic!($msg),
    }
  }};
}

fn create_custom_gc(env: sys::napi_env) {
  let mut custom_gc_fn = ptr::null_mut();
  check_status_or_panic!(
    unsafe {
      sys::napi_create_function(
        env,
        "custom_gc".as_ptr() as *const c_char,
        9,
        Some(empty),
        ptr::null_mut(),
        &mut custom_gc_fn,
      )
    },
    "Create Custom GC Function in napi_register_module_v1 failed"
  );
  let mut async_resource_name = ptr::null_mut();
  check_status_or_panic!(
    unsafe {
      sys::napi_create_string_utf8(
        env,
        "CustomGC".as_ptr() as *const c_char,
        8,
        &mut async_resource_name,
      )
    },
    "Create async resource string in napi_register_module_v1 napi_register_module_v1"
  );
  let mut custom_gc_tsfn = ptr::null_mut();
  let context = Box::into_raw(Box::new(0)) as *mut c_void;
  check_status_or_panic!(
    unsafe {
      sys::napi_create_threadsafe_function(
        env,
        custom_gc_fn,
        ptr::null_mut(),
        async_resource_name,
        0,
        1,
        ptr::null_mut(),
        Some(custom_gc_finalize),
        context,
        Some(custom_gc),
        &mut custom_gc_tsfn,
      )
    },
    "Create Custom GC ThreadsafeFunction in napi_register_module_v1 failed"
  );
  check_status_or_panic!(
    unsafe { sys::napi_unref_threadsafe_function(env, custom_gc_tsfn) },
    "Unref Custom GC ThreadsafeFunction in napi_register_module_v1 failed"
  );
}

unsafe extern "C" fn empty(
  _env: sys::napi_env,
  _info: sys::napi_callback_info,
) -> sys::napi_value {
  ptr::null_mut()
}

unsafe extern "C" fn custom_gc_finalize(
  _env: sys::napi_env,
  _finalize_data: *mut c_void,
  finalize_hint: *mut c_void,
) {
  unsafe {
    let _ = Box::from_raw(finalize_hint as *mut i32);
  }
}

extern "C" fn custom_gc(
  env: sys::napi_env,
  _js_callback: sys::napi_value,
  _context: *mut c_void,
  data: *mut c_void,
) {
  let mut ref_count = 0;
  check_status_or_panic!(
    unsafe {
      sys::napi_reference_unref(env, data as sys::napi_ref, &mut ref_count)
    },
    "Failed to unref Buffer reference in Custom GC"
  );
  check_status_or_panic!(
    unsafe { sys::napi_delete_reference(env, data as sys::napi_ref) },
    "Failed to delete Buffer reference in Custom GC"
  );
}

pub fn init(env: sys::napi_env, _exports: sys::napi_value) {
  create_custom_gc(env);
}
