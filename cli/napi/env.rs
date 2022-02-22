use deno_core::napi::*;

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

/// # Safety
///
/// It's an N-API symbol
#[no_mangle]
pub unsafe extern "C" fn napi_fatal_exception(
  env: napi_env,
  value: napi_value,
) -> ! {
  let mut env = &mut *(env as *mut Env);
  let value: v8::Local<v8::Value> = std::mem::transmute(value);
  let error = value.to_rust_string_lossy(env.scope);
  panic!(
    "Fatal exception triggered by napi_fatal_exception!\n{}",
    error
  );
}

// TODO: properly implement
#[napi_sym::napi_sym]
fn napi_add_env_cleanup_hook(
  env: napi_env,
  _hook: extern "C" fn(*const c_void),
  _data: *const c_void,
) -> Result {
  let mut _env = &mut *(env as *mut Env);
  Ok(())
}

// TODO: properly implement
#[napi_sym::napi_sym]
fn napi_remove_env_cleanup_hook(
  env: napi_env,
  _hook: extern "C" fn(*const c_void),
  _data: *const c_void,
) -> Result {
  let mut _env = &mut *(env as *mut Env);
  Ok(())
}

#[napi_sym::napi_sym]
fn node_api_get_module_file_name(
  env: napi_env,
  result: *mut *const c_char,
) -> Result {
  let env = &mut *(env as *mut Env);
  let shared = env.shared();
  *result = shared.filename;
  Ok(())
}

#[napi_sym::napi_sym]
fn napi_module_register(module: *const NapiModule) -> Result {
  MODULE.with(|cell| {
    let mut slot = cell.borrow_mut();
    assert!(slot.is_none());
    slot.replace(module);
  });
  Ok(())
}

#[napi_sym::napi_sym]
fn napi_get_uv_event_loop(_env: &mut Env, uv_loop: *mut *mut ()) -> Result {
  // Don't error out because addons maybe pass this to
  // our libuv _polyfills_.
  *uv_loop = std::ptr::null_mut();
  Ok(())
}
#[napi_sym::napi_sym]
fn napi_get_node_version(
  _: napi_env,
  result: *mut *const napi_node_version,
) -> Result {
  NODE_VERSION.with(|version| {
    *result = version as *const napi_node_version;
  });
  Ok(())
}
thread_local! {
  static NODE_VERSION: napi_node_version = {
    let release = std::ffi::CString::new("Deno N-API").unwrap();
    let release_ptr = release.as_ptr();
    std::mem::forget(release);
    napi_node_version {
      major: 17,
      minor: 4,
      patch: 0,
      release: release_ptr,
    }
  }
}
