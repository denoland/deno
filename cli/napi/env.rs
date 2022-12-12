// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use deno_runtime::deno_napi::*;
use std::os::raw::c_char;

/// # Safety
///
/// It's an N-API symbol
#[no_mangle]
pub unsafe extern "C" fn napi_fatal_error(
  location: *const c_char,
  location_len: isize,
  message: *const c_char,
  message_len: isize,
) -> ! {
  let location = if location.is_null() {
    None
  } else {
    Some(if location_len < 0 {
      std::ffi::CStr::from_ptr(location).to_str().unwrap()
    } else {
      let slice = std::slice::from_raw_parts(
        location as *const u8,
        location_len as usize,
      );
      std::str::from_utf8(slice).unwrap()
    })
  };
  let message = if message_len < 0 {
    std::ffi::CStr::from_ptr(message).to_str().unwrap()
  } else {
    let slice =
      std::slice::from_raw_parts(message as *const u8, message_len as usize);
    std::str::from_utf8(slice).unwrap()
  };
  panic!(
    "Fatal exception triggered by napi_fatal_error!\nLocation: {:?}\n{}",
    location, message
  );
}

// napi-3

#[napi_sym::napi_sym]
fn napi_fatal_exception(env: *mut Env, value: napi_value) -> Result {
  let env: &mut Env = env.as_mut().ok_or(Error::InvalidArg)?;
  let value = transmute::<napi_value, v8::Local<v8::Value>>(value);
  let error = value.to_rust_string_lossy(&mut env.scope());
  panic!(
    "Fatal exception triggered by napi_fatal_exception!\n{}",
    error
  );
}

// TODO: properly implement
#[napi_sym::napi_sym]
fn napi_add_env_cleanup_hook(
  _env: *mut Env,
  _hook: extern "C" fn(*const c_void),
  _data: *const c_void,
) -> Result {
  log::info!("napi_add_env_cleanup_hook is currently not supported");
  Ok(())
}

#[napi_sym::napi_sym]
fn napi_remove_env_cleanup_hook(
  _env: *mut Env,
  _hook: extern "C" fn(*const c_void),
  _data: *const c_void,
) -> Result {
  log::info!("napi_remove_env_cleanup_hook is currently not supported");
  Ok(())
}

#[napi_sym::napi_sym]
fn napi_open_callback_scope(
  _env: *mut Env,
  _resource_object: napi_value,
  _context: napi_value,
  _result: *mut napi_callback_scope,
) -> Result {
  // we open scope automatically when it's needed
  Ok(())
}

#[napi_sym::napi_sym]
fn napi_close_callback_scope(
  _env: *mut Env,
  _scope: napi_callback_scope,
) -> Result {
  // we close scope automatically when it's needed
  Ok(())
}

#[napi_sym::napi_sym]
fn node_api_get_module_file_name(
  env: *mut Env,
  result: *mut *const c_char,
) -> Result {
  let env: &mut Env = env.as_mut().ok_or(Error::InvalidArg)?;

  let shared = env.shared();
  *result = shared.filename;
  Ok(())
}

#[napi_sym::napi_sym]
fn napi_module_register(module: *const NapiModule) -> Result {
  MODULE.with(|cell| {
    let mut slot = cell.borrow_mut();
    slot.replace(module);
  });
  Ok(())
}

#[napi_sym::napi_sym]
fn napi_get_uv_event_loop(_env: *mut Env, uv_loop: *mut *mut ()) -> Result {
  // Don't error out because addons may pass this to
  // our libuv _polyfills_.
  *uv_loop = std::ptr::null_mut();
  Ok(())
}

const NODE_VERSION: napi_node_version = napi_node_version {
  major: 17,
  minor: 4,
  patch: 0,
  release: "Deno\0".as_ptr() as *const c_char,
};

#[napi_sym::napi_sym]
fn napi_get_node_version(
  env: *mut Env,
  result: *mut *const napi_node_version,
) -> Result {
  let _: &mut Env = env.as_mut().ok_or(Error::InvalidArg)?;
  crate::check_arg!(result);

  *result = &NODE_VERSION as *const napi_node_version;
  Ok(())
}
