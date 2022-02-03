use deno_core::napi::*;

thread_local! {
  static NODE_VERSION: napi_node_version = {
    let release = std::ffi::CString::new("Deno N-API").unwrap();
    let release = release.as_ptr();
    std::mem::forget(release);
    napi_node_version {
      major: 17,
      minor: 4,
      patch: 0,
      release: release,
    }
  }
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
