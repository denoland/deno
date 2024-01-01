// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

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
    "Fatal exception triggered by napi_fatal_error!\nLocation: {location:?}\n{message}"
  );
}

// napi-3

#[napi_sym::napi_sym]
fn napi_fatal_exception(env: *mut Env, value: napi_value) -> napi_status {
  let Some(env) = env.as_mut() else {
    return napi_invalid_arg;
  };
  let value = transmute::<napi_value, v8::Local<v8::Value>>(value);
  let error = value.to_rust_string_lossy(&mut env.scope());
  panic!("Fatal exception triggered by napi_fatal_exception!\n{error}");
}

#[napi_sym::napi_sym]
fn napi_add_env_cleanup_hook(
  env: *mut Env,
  hook: extern "C" fn(*const c_void),
  data: *const c_void,
) -> napi_status {
  let Some(env) = env.as_mut() else {
    return napi_invalid_arg;
  };

  {
    let mut env_cleanup_hooks = env.cleanup_hooks.borrow_mut();
    if env_cleanup_hooks
      .iter()
      .any(|pair| pair.0 == hook && pair.1 == data)
    {
      panic!("Cleanup hook with this data already registered");
    }
    env_cleanup_hooks.push((hook, data));
  }
  napi_ok
}

#[napi_sym::napi_sym]
fn napi_remove_env_cleanup_hook(
  env: *mut Env,
  hook: extern "C" fn(*const c_void),
  data: *const c_void,
) -> napi_status {
  let Some(env) = env.as_mut() else {
    return napi_invalid_arg;
  };

  {
    let mut env_cleanup_hooks = env.cleanup_hooks.borrow_mut();
    // Hooks are supposed to be removed in LIFO order
    let maybe_index = env_cleanup_hooks
      .iter()
      .rposition(|&pair| pair.0 == hook && pair.1 == data);

    if let Some(index) = maybe_index {
      env_cleanup_hooks.remove(index);
    } else {
      panic!("Cleanup hook with this data not found");
    }
  }

  napi_ok
}

#[napi_sym::napi_sym]
fn napi_open_callback_scope(
  _env: *mut Env,
  _resource_object: napi_value,
  _context: napi_value,
  _result: *mut napi_callback_scope,
) -> napi_status {
  // we open scope automatically when it's needed
  napi_ok
}

#[napi_sym::napi_sym]
fn napi_close_callback_scope(
  _env: *mut Env,
  _scope: napi_callback_scope,
) -> napi_status {
  // we close scope automatically when it's needed
  napi_ok
}

#[napi_sym::napi_sym]
fn node_api_get_module_file_name(
  env: *mut Env,
  result: *mut *const c_char,
) -> napi_status {
  let Some(env) = env.as_mut() else {
    return napi_invalid_arg;
  };

  let shared = env.shared();
  *result = shared.filename;
  napi_ok
}

#[napi_sym::napi_sym]
fn napi_module_register(module: *const NapiModule) -> napi_status {
  MODULE_TO_REGISTER.with(|cell| {
    let mut slot = cell.borrow_mut();
    let prev = slot.replace(module);
    assert!(prev.is_none());
  });
  napi_ok
}

#[napi_sym::napi_sym]
fn napi_get_uv_event_loop(
  _env: *mut Env,
  uv_loop: *mut *mut (),
) -> napi_status {
  // There is no uv_loop in Deno
  *uv_loop = std::ptr::null_mut();
  napi_ok
}

const NODE_VERSION: napi_node_version = napi_node_version {
  major: 18,
  minor: 13,
  patch: 0,
  release: "Deno\0".as_ptr() as *const c_char,
};

#[napi_sym::napi_sym]
fn napi_get_node_version(
  env: *mut Env,
  result: *mut *const napi_node_version,
) -> napi_status {
  crate::check_env!(env);
  crate::check_arg!(env, result);

  *result = &NODE_VERSION as *const napi_node_version;
  napi_ok
}
