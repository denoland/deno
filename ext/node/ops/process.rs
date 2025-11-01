// Copyright 2018-2025 the Deno authors. MIT license.

use deno_core::OpState;
use deno_core::op2;
use deno_permissions::PermissionCheckError;
use deno_permissions::PermissionsContainer;
#[cfg(unix)]
use nix::unistd::Gid;
#[cfg(unix)]
use nix::unistd::Group;

use crate::NodePermissions;

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum ProcessError {
  #[class(inherit)]
  #[error(transparent)]
  Permission(
    #[from]
    #[inherit]
    PermissionCheckError,
  ),
  #[class(generic)]
  #[error("{0} identifier does not exist: {1}")]
  #[property("code" = "ERR_UNKNOWN_CREDENTIAL")]
  UnknownCredential(String, String),
  #[class(inherit)]
  #[error(transparent)]
  Io(#[from] std::io::Error),
}

#[cfg(unix)]
impl From<nix::Error> for ProcessError {
  fn from(err: nix::Error) -> Self {
    ProcessError::Io(std::io::Error::from_raw_os_error(err as i32))
  }
}

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

#[derive(serde::Deserialize)]
#[serde(untagged)]
enum Id {
  Number(u32),
  Name(String),
}

#[cfg(not(any(target_os = "android", target_os = "windows")))]
fn get_group_id(name: &str) -> Result<Gid, ProcessError> {
  let group = Group::from_name(name)?;

  if let Some(group) = group {
    Ok(group.gid)
  } else {
    Err(ProcessError::UnknownCredential(
      "Group".to_string(),
      name.to_string(),
    ))
  }
}

#[cfg(not(any(target_os = "android", target_os = "windows")))]
#[op2(stack_trace)]
pub fn op_node_process_setegid<P>(
  state: &mut OpState,
  #[serde] id: Id,
) -> Result<(), ProcessError>
where
  P: NodePermissions + 'static,
{
  {
    let permissions = state.borrow_mut::<P>();
    permissions.check_sys("setegid", "node:process.setegid")?;
  }

  let gid = match id {
    Id::Number(id) => Gid::from_raw(id),
    Id::Name(name) => get_group_id(&name)?,
  };

  nix::unistd::setegid(gid)?;

  Ok(())
}

#[cfg(any(target_os = "android", target_os = "windows"))]
#[op2(stack_trace)]
pub fn op_node_process_setegid<P>(
  state: &mut OpState,
  #[string] id: &str,
) -> Result<(), ProcessError>
where
  P: NodePermissions + 'static,
{
  unimplemented!()
}
