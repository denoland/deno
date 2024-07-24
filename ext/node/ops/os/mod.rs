// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use crate::NodePermissions;
use deno_core::error::type_error;
use deno_core::error::AnyError;
use deno_core::op2;
use deno_core::OpState;

mod cpus;
mod priority;

#[op2(fast)]
pub fn op_node_os_get_priority<P>(
  state: &mut OpState,
  pid: u32,
) -> Result<i32, AnyError>
where
  P: NodePermissions + 'static,
{
  {
    let permissions = state.borrow_mut::<P>();
    permissions.check_sys("getPriority", "node:os.getPriority()")?;
  }

  priority::get_priority(pid)
}

#[op2(fast)]
pub fn op_node_os_set_priority<P>(
  state: &mut OpState,
  pid: u32,
  priority: i32,
) -> Result<(), AnyError>
where
  P: NodePermissions + 'static,
{
  {
    let permissions = state.borrow_mut::<P>();
    permissions.check_sys("setPriority", "node:os.setPriority()")?;
  }

  priority::set_priority(pid, priority)
}

#[derive(serde::Serialize)]
pub struct UserInfo {
  username: String,
  homedir: Option<String>,
  shell: Option<String>,
}

#[cfg(unix)]
fn get_user_info(uid: u32) -> Result<UserInfo, AnyError> {
  use std::ffi::CStr;
  use std::mem::MaybeUninit;
  let mut pw: MaybeUninit<libc::passwd> = MaybeUninit::uninit();
  let mut result: *mut libc::passwd = std::ptr::null_mut();
  // SAFETY: libc call, no invariants
  let max_buf_size = unsafe { libc::sysconf(libc::_SC_GETPW_R_SIZE_MAX) };
  let buf_size = if max_buf_size < 0 {
    // from the man page
    16_384
  } else {
    max_buf_size as usize
  };
  let mut buf = {
    let mut b = Vec::<MaybeUninit<libc::c_char>>::with_capacity(buf_size);
    // SAFETY: MaybeUninit has no initialization invariants, and len == cap
    unsafe {
      b.set_len(buf_size);
    }
    b
  };
  // SAFETY: libc call, args are correct
  let s = unsafe {
    libc::getpwuid_r(
      uid,
      pw.as_mut_ptr(),
      buf.as_mut_ptr().cast(),
      buf_size,
      std::ptr::addr_of_mut!(result),
    )
  };
  if result.is_null() {
    if s != 0 {
      return Err(std::io::Error::last_os_error().into());
    } else {
      return Err(std::io::Error::from(std::io::ErrorKind::NotFound).into());
    }
  }
  // SAFETY: pw was initialized by the call to `getpwuid_r` above
  let pw = unsafe { pw.assume_init() };
  // SAFETY: initialized above, pw alive until end of function, nul terminated
  let username = unsafe { CStr::from_ptr(pw.pw_name) };
  let homedir = unsafe { CStr::from_ptr(pw.pw_dir) };
  let shell = unsafe { CStr::from_ptr(pw.pw_shell) };
  Ok(UserInfo {
    username: username.to_string_lossy().into_owned(),
    homedir: Some(homedir.to_string_lossy().into_owned()),
    shell: Some(shell.to_string_lossy().into_owned()),
  })
}

#[cfg(windows)]
fn get_user_info(_uid: u32) {
  // use windows_sys::Win32::Foundation::HANDLE;
  // use windows_sys::Win32::System::Threading::OpenProcessToken;
  // let mut token: HANDLE = 0;
  // let mut username = [0u16; 256 + 1];

  Ok(UserInfo {
    username: deno_whoami::username(),
    homedir: home::home_dir().map(|path| path.to_string_lossy().to_string()),
    shell: None,
  })
}

#[op2]
#[serde]
pub fn op_node_os_user_info<P>(
  state: &mut OpState,
  #[smi] uid: u32,
) -> Result<UserInfo, AnyError>
where
  P: NodePermissions + 'static,
{
  {
    let permissions = state.borrow_mut::<P>();
    permissions.check_sys("username", "node:os.userInfo()")?;
  }

  get_user_info(uid)
}

#[op2(fast)]
pub fn op_geteuid<P>(state: &mut OpState) -> Result<u32, AnyError>
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
pub fn op_getegid<P>(state: &mut OpState) -> Result<u32, AnyError>
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
pub fn op_cpus<P>(state: &mut OpState) -> Result<Vec<cpus::CpuInfo>, AnyError>
where
  P: NodePermissions + 'static,
{
  {
    let permissions = state.borrow_mut::<P>();
    permissions.check_sys("cpus", "node:os.cpus()")?;
  }

  cpus::cpu_info().ok_or_else(|| type_error("Failed to get cpu info"))
}

#[op2]
#[string]
pub fn op_homedir<P>(state: &mut OpState) -> Result<Option<String>, AnyError>
where
  P: NodePermissions + 'static,
{
  {
    let permissions = state.borrow_mut::<P>();
    permissions.check_sys("homedir", "node:os.homedir()")?;
  }

  Ok(home::home_dir().map(|path| path.to_string_lossy().to_string()))
}
