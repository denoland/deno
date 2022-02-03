use deno_core::napi::*;
use std::cell::RefCell;
use std::thread_local;

thread_local! {
  pub static MODULE: RefCell<Option<*const NapiModule>> = RefCell::new(None);
}

type napi_addon_register_func =
  extern "C" fn(env: napi_env, exports: napi_value) -> napi_value;

#[repr(C)]
#[derive(Debug, Clone)]
pub struct NapiModule {
  pub nm_version: i32,
  pub nm_flags: u32,
  nm_filename: *const c_char,
  pub nm_register_func: napi_addon_register_func,
  nm_modname: *const c_char,
  nm_priv: *mut c_void,
  reserved: [*mut c_void; 4],
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
