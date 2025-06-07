// Copyright 2018-2025 the Deno authors. MIT license.

use deno_core::op2;
use deno_core::OpState;
use deno_permissions::PermissionsContainer;

#[cfg(unix)]
fn kill(pid: i32, sig: i32) -> i32 {
  // SAFETY: FFI call to libc
  if unsafe { libc::kill(pid, sig) } < 0 {
    std::io::Error::last_os_error().raw_os_error().unwrap()
  } else {
    0
  }
}

#[cfg(not(unix))]
fn kill(pid: i32, _sig: i32) -> i32 {
  match deno_subprocess_windows::process_kill(pid, _sig) {
    Ok(_) => 0,
    Err(e) => e.as_uv_error(),
  }
}

#[op2(fast, stack_trace)]
pub fn op_node_process_kill(
  state: &mut OpState,
  #[smi] pid: i32,
  #[smi] sig: i32,
) -> Result<i32, deno_permissions::PermissionCheckError> {
  state
    .borrow_mut::<PermissionsContainer>()
    .check_run_all("process.kill")?;
  Ok(kill(pid, sig))
}

#[op2(fast)]
pub fn op_process_abort() {
  std::process::abort();
}
