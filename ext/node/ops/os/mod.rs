// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use crate::NodePermissions;
use deno_core::op2;
use deno_core::OpState;

mod cpus;
pub mod priority;

#[derive(Debug, thiserror::Error)]
pub enum OsError {
  #[error(transparent)]
  Priority(priority::PriorityError),
  #[error(transparent)]
  Permission(#[from] deno_permissions::PermissionCheckError),
  #[error("Failed to get cpu info")]
  FailedToGetCpuInfo,
}

#[op2(fast)]
pub fn op_node_os_get_priority<P>(
  state: &mut OpState,
  pid: u32,
) -> Result<i32, OsError>
where
  P: NodePermissions + 'static,
{
  {
    let permissions = state.borrow_mut::<P>();
    permissions.check_sys("getPriority", "node:os.getPriority()")?;
  }

  priority::get_priority(pid).map_err(OsError::Priority)
}

#[op2(fast)]
pub fn op_node_os_set_priority<P>(
  state: &mut OpState,
  pid: u32,
  priority: i32,
) -> Result<(), OsError>
where
  P: NodePermissions + 'static,
{
  {
    let permissions = state.borrow_mut::<P>();
    permissions.check_sys("setPriority", "node:os.setPriority()")?;
  }

  priority::set_priority(pid, priority).map_err(OsError::Priority)
}

#[op2]
#[string]
pub fn op_node_os_username<P>(
  state: &mut OpState,
) -> Result<String, deno_core::error::AnyError>
where
  P: NodePermissions + 'static,
{
  {
    let permissions = state.borrow_mut::<P>();
    permissions.check_sys("username", "node:os.userInfo()")?;
  }

  Ok(deno_whoami::username())
}

#[op2(fast)]
pub fn op_geteuid<P>(
  state: &mut OpState,
) -> Result<u32, deno_core::error::AnyError>
where
  P: NodePermissions + 'static,
{
  {
    let permissions = state.borrow_mut::<P>();
    permissions.check_sys("uid", "node:os.geteuid()")?;
  }

  #[cfg(windows)]
  let euid = 0;
  #[cfg(unix)]
  // SAFETY: Call to libc geteuid.
  let euid = unsafe { libc::geteuid() };

  Ok(euid)
}

#[op2(fast)]
pub fn op_getegid<P>(
  state: &mut OpState,
) -> Result<u32, deno_core::error::AnyError>
where
  P: NodePermissions + 'static,
{
  {
    let permissions = state.borrow_mut::<P>();
    permissions.check_sys("getegid", "node:os.getegid()")?;
  }

  #[cfg(windows)]
  let egid = 0;
  #[cfg(unix)]
  // SAFETY: Call to libc getegid.
  let egid = unsafe { libc::getegid() };

  Ok(egid)
}

#[op2]
#[serde]
pub fn op_cpus<P>(state: &mut OpState) -> Result<Vec<cpus::CpuInfo>, OsError>
where
  P: NodePermissions + 'static,
{
  {
    let permissions = state.borrow_mut::<P>();
    permissions.check_sys("cpus", "node:os.cpus()")?;
  }

  cpus::cpu_info().ok_or(OsError::FailedToGetCpuInfo)
}

#[op2]
#[string]
pub fn op_homedir<P>(
  state: &mut OpState,
) -> Result<Option<String>, deno_core::error::AnyError>
where
  P: NodePermissions + 'static,
{
  {
    let permissions = state.borrow_mut::<P>();
    permissions.check_sys("homedir", "node:os.homedir()")?;
  }

  Ok(home::home_dir().map(|path| path.to_string_lossy().to_string()))
}
