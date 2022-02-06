use deno_core::napi::*;
use std::cell::RefCell;

#[napi_sym::napi_sym]
fn napi_module_register(module: *const NapiModule) -> Result {
  MODULE.with(|cell| {
    let mut slot = cell.borrow_mut();
    assert!(slot.is_none());
    slot.replace(module);
  });
  Ok(())
}
