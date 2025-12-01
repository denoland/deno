// Copyright 2018-2025 the Deno authors. MIT license.

use deno_core::OpState;
use deno_core::op2;
use deno_core::v8;
use deno_permissions::PermissionCheckError;
use deno_permissions::PermissionsContainer;
#[cfg(unix)]
use nix::unistd::Gid;
#[cfg(unix)]
use nix::unistd::Group;
#[cfg(unix)]
use nix::unistd::Uid;
#[cfg(unix)]
use nix::unistd::User;

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
  #[class(generic)]
  #[error("Operation not supported on this platform")]
  NotSupported,
  #[class(type)]
  #[error("Invalid {0} parameter")]
  InvalidParam(String),
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

#[cfg(not(any(target_os = "android", target_os = "windows")))]
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
fn serialize_id<'a>(
  scope: &mut v8::PinScope<'a, '_>,
  value: v8::Local<'a, v8::Value>,
) -> Result<Id, ProcessError> {
  if value.is_number() {
    let num = value.uint32_value(scope).unwrap();
    return Ok(Id::Number(num));
  }

  if value.is_string() {
    let name = value.to_string(scope).unwrap();
    return Ok(Id::Name(name.to_rust_string_lossy(scope)));
  }

  Err(ProcessError::InvalidParam("id".to_string()))
}

#[cfg(not(any(target_os = "android", target_os = "windows")))]
#[op2(fast, stack_trace)]
pub fn op_node_process_setegid<'a>(
  scope: &mut v8::PinScope<'a, '_>,
  state: &mut OpState,
  id: v8::Local<'a, v8::Value>,
) -> Result<(), ProcessError> {
  {
    let permissions = state.borrow_mut::<PermissionsContainer>();
    permissions.check_sys("setegid", "node:process.setegid")?;
  }

  let gid = match serialize_id(scope, id)? {
    Id::Number(number) => Gid::from_raw(number),
    Id::Name(name) => get_group_id(&name)?,
  };

  nix::unistd::setegid(gid)?;

  Ok(())
}

#[cfg(any(target_os = "android", target_os = "windows"))]
#[op2(fast, stack_trace)]
pub fn op_node_process_setegid(
  _scope: &mut v8::PinScope<'_, '_>,
  _state: &mut OpState,
  _id: v8::Local<'_, v8::Value>,
) -> Result<(), ProcessError> {
  Err(ProcessError::NotSupported)
}

#[cfg(not(any(target_os = "android", target_os = "windows")))]
fn get_user_id(name: &str) -> Result<Uid, ProcessError> {
  let user = User::from_name(name)?;

  if let Some(user) = user {
    Ok(user.uid)
  } else {
    Err(ProcessError::UnknownCredential(
      "User".to_string(),
      name.to_string(),
    ))
  }
}

#[cfg(not(any(target_os = "android", target_os = "windows")))]
#[op2(fast, stack_trace)]
pub fn op_node_process_seteuid<'a>(
  scope: &mut v8::PinScope<'a, '_>,
  state: &mut OpState,
  id: v8::Local<'a, v8::Value>,
) -> Result<(), ProcessError> {
  {
    let permissions = state.borrow_mut::<PermissionsContainer>();
    permissions.check_sys("seteuid", "node:process.seteuid")?;
  }

  let uid = match serialize_id(scope, id)? {
    Id::Number(number) => Uid::from_raw(number),
    Id::Name(name) => get_user_id(&name)?,
  };

  nix::unistd::seteuid(uid)?;

  Ok(())
}

#[cfg(any(target_os = "android", target_os = "windows"))]
#[op2(fast, stack_trace)]
pub fn op_node_process_seteuid(
  _scope: &mut v8::PinScope<'_, '_>,
  _state: &mut OpState,
  _id: v8::Local<'_, v8::Value>,
) -> Result<(), ProcessError> {
  Err(ProcessError::NotSupported)
}

#[cfg(not(any(target_os = "android", target_os = "windows")))]
#[op2(fast, stack_trace)]
pub fn op_node_process_setgid<'a>(
  scope: &mut v8::PinScope<'a, '_>,
  state: &mut OpState,
  id: v8::Local<'a, v8::Value>,
) -> Result<(), ProcessError> {
  {
    let permissions = state.borrow_mut::<PermissionsContainer>();
    permissions.check_sys("setgid", "node:process.setgid")?;
  }

  let gid = match serialize_id(scope, id)? {
    Id::Number(number) => Gid::from_raw(number),
    Id::Name(name) => get_group_id(&name)?,
  };

  nix::unistd::setgid(gid)?;

  Ok(())
}

#[cfg(any(target_os = "android", target_os = "windows"))]
#[op2(fast, stack_trace)]
pub fn op_node_process_setgid(
  _scope: &mut v8::PinScope<'_, '_>,
  _state: &mut OpState,
  _id: v8::Local<'_, v8::Value>,
) -> Result<(), ProcessError> {
  Err(ProcessError::NotSupported)
}

#[cfg(not(any(target_os = "android", target_os = "windows")))]
#[op2(fast, stack_trace)]
pub fn op_node_process_setuid<'a>(
  scope: &mut v8::PinScope<'a, '_>,
  state: &mut OpState,
  id: v8::Local<'a, v8::Value>,
) -> Result<(), ProcessError> {
  {
    let permissions = state.borrow_mut::<PermissionsContainer>();
    permissions.check_sys("setuid", "node:process.setuid")?;
  }

  let uid = match serialize_id(scope, id)? {
    Id::Number(number) => Uid::from_raw(number),
    Id::Name(name) => get_user_id(&name)?,
  };

  nix::unistd::setuid(uid)?;

  Ok(())
}

#[cfg(any(target_os = "android", target_os = "windows"))]
#[op2(fast, stack_trace)]
pub fn op_node_process_setuid(
  _scope: &mut v8::PinScope<'_, '_>,
  _state: &mut OpState,
  _id: v8::Local<'_, v8::Value>,
) -> Result<(), ProcessError> {
  Err(ProcessError::NotSupported)
}
