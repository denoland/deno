// Copyright 2018-2026 the Deno authors. MIT license.

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

use crate::ExtNodeSys;

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

/// Returns the cgroup-constrained memory limit, or 0 if unconstrained.
/// This matches Node.js `process.constrainedMemory()` semantics.
#[op2(fast)]
#[number]
pub fn op_node_process_constrained_memory<TSys: ExtNodeSys + 'static>(
  state: &mut OpState,
) -> u64 {
  #[cfg(any(target_os = "android", target_os = "linux"))]
  {
    let sys = state.borrow::<TSys>();
    cgroup::cgroup_memory_limit(sys).unwrap_or(0)
  }
  #[cfg(not(any(target_os = "android", target_os = "linux")))]
  {
    let _ = state;
    0
  }
}

#[cfg(any(target_os = "android", target_os = "linux"))]
pub mod cgroup {
  pub enum CgroupVersion<'a> {
    V1 { cgroup_relpath: &'a str },
    V2 { cgroup_relpath: &'a str },
    None,
  }

  pub fn parse_self_cgroup(self_cgroup_content: &str) -> CgroupVersion<'_> {
    let mut cgroup_version = CgroupVersion::None;

    for line in self_cgroup_content.lines() {
      let split = line.split(":").collect::<Vec<_>>();

      match &split[..] {
        // cgroup v1 memory controller — takes priority, break immediately
        [_, "memory", cgroup_v1_relpath] => {
          cgroup_version = CgroupVersion::V1 {
            cgroup_relpath: cgroup_v1_relpath
              .strip_prefix("/")
              .unwrap_or(cgroup_v1_relpath),
          };
          break;
        }
        // cgroup v2 (but keep looking for v1 memory in hybrid mode)
        ["0", "", cgroup_v2_relpath] => {
          cgroup_version = CgroupVersion::V2 {
            cgroup_relpath: cgroup_v2_relpath
              .strip_prefix("/")
              .unwrap_or(cgroup_v2_relpath),
          };
        }
        _ => {}
      }
    }

    cgroup_version
  }

  /// Read the cgroup memory limit from the filesystem.
  /// Returns `None` if cgroup info cannot be read or parsed.
  pub fn cgroup_memory_limit<TSys: sys_traits::FsRead>(
    sys: &TSys,
  ) -> Option<u64> {
    let self_cgroup = sys.fs_read_to_string("/proc/self/cgroup").ok()?;

    match parse_self_cgroup(&self_cgroup) {
      CgroupVersion::V1 { cgroup_relpath } => {
        let limit_path = std::path::Path::new("/sys/fs/cgroup/memory")
          .join(cgroup_relpath)
          .join("memory.limit_in_bytes");
        sys
          .fs_read_to_string(limit_path)
          .ok()
          .and_then(|s| s.trim().parse::<u64>().ok())
      }
      CgroupVersion::V2 { cgroup_relpath } => {
        let limit_path = std::path::Path::new("/sys/fs/cgroup")
          .join(cgroup_relpath)
          .join("memory.max");
        sys
          .fs_read_to_string(limit_path)
          .ok()
          .and_then(|s| s.trim().parse::<u64>().ok())
      }
      CgroupVersion::None => None,
    }
  }

  #[cfg(test)]
  mod tests {
    use super::*;

    #[test]
    fn test_parse_self_cgroup_v2() {
      let self_cgroup = "0::/user.slice/user-1000.slice/session-3.scope";
      let cgroup_version = parse_self_cgroup(self_cgroup);
      assert!(matches!(
        cgroup_version,
        CgroupVersion::V2 { cgroup_relpath } if cgroup_relpath == "user.slice/user-1000.slice/session-3.scope"
      ));
    }

    #[test]
    fn test_parse_self_cgroup_hybrid() {
      let self_cgroup = r#"12:rdma:/
11:blkio:/user.slice
10:devices:/user.slice
9:cpu,cpuacct:/user.slice
8:pids:/user.slice/user-1000.slice/session-3.scope
7:memory:/user.slice/user-1000.slice/session-3.scope
6:perf_event:/
5:freezer:/
4:net_cls,net_prio:/
3:hugetlb:/
2:cpuset:/
1:name=systemd:/user.slice/user-1000.slice/session-3.scope
0::/user.slice/user-1000.slice/session-3.scope
"#;
      let cgroup_version = parse_self_cgroup(self_cgroup);
      assert!(matches!(
        cgroup_version,
        CgroupVersion::V1 { cgroup_relpath } if cgroup_relpath == "user.slice/user-1000.slice/session-3.scope"
      ));
    }

    #[test]
    fn test_parse_self_cgroup_v1() {
      let self_cgroup = r#"11:hugetlb:/
10:pids:/user.slice/user-1000.slice
9:perf_event:/
8:devices:/user.slice
7:net_cls,net_prio:/
6:memory:/
5:blkio:/
4:cpuset:/
3:cpu,cpuacct:/
2:freezer:/
1:name=systemd:/user.slice/user-1000.slice/session-2.scope
"#;
      let cgroup_version = parse_self_cgroup(self_cgroup);
      assert!(matches!(
        cgroup_version,
        CgroupVersion::V1 { cgroup_relpath } if cgroup_relpath.is_empty()
      ));
    }
  }
}
