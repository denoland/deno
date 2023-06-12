// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use deno_core::error::AnyError;
use deno_core::op;
use deno_core::OpState;
use errno::errno;
use errno::set_errno;
use errno::Errno;

#[op]
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

  get_priority(pid)
}

#[op]
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

  set_priority(pid, priority)
}

#[op]
pub fn op_node_os_username<P>(state: &mut OpState) -> Result<String, AnyError>
where
  P: NodePermissions + 'static,
{
  {
    let permissions = state.borrow_mut::<P>();
    permissions.check_sys("userInfo", "node:os.userInfo()")?;
  }

  Ok(whoami::username())
}

use crate::NodePermissions;
#[cfg(unix)]
use libc::id_t;
#[cfg(unix)]
use libc::PRIO_PROCESS;
#[cfg(windows)]
use winapi::shared::minwindef::DWORD;
#[cfg(windows)]
use winapi::shared::minwindef::FALSE;
#[cfg(windows)]
use winapi::shared::ntdef::NULL;
#[cfg(windows)]
use winapi::um::handleapi::CloseHandle;
#[cfg(windows)]
use winapi::um::processthreadsapi::GetCurrentProcess;
#[cfg(windows)]
use winapi::um::processthreadsapi::GetPriorityClass;
#[cfg(windows)]
use winapi::um::processthreadsapi::OpenProcess;
#[cfg(windows)]
use winapi::um::processthreadsapi::SetPriorityClass;
#[cfg(windows)]
use winapi::um::winbase::ABOVE_NORMAL_PRIORITY_CLASS;
#[cfg(windows)]
use winapi::um::winbase::BELOW_NORMAL_PRIORITY_CLASS;
#[cfg(windows)]
use winapi::um::winbase::HIGH_PRIORITY_CLASS;
#[cfg(windows)]
use winapi::um::winbase::IDLE_PRIORITY_CLASS;
#[cfg(windows)]
use winapi::um::winbase::NORMAL_PRIORITY_CLASS;
#[cfg(windows)]
use winapi::um::winbase::REALTIME_PRIORITY_CLASS;
#[cfg(windows)]
use winapi::um::winnt::PROCESS_QUERY_LIMITED_INFORMATION;

#[cfg(target_os = "macos")]
#[allow(non_camel_case_types)]
type priority_t = i32;
#[cfg(target_os = "linux")]
#[allow(non_camel_case_types)]
type priority_t = u32;

#[cfg(windows)]
pub const PRIORITY_LOW: i32 = 19;
#[cfg(windows)]
pub const PRIORITY_BELOW_NORMAL: i32 = 10;
#[cfg(windows)]
pub const PRIORITY_NORMAL: i32 = 0;
#[cfg(windows)]
pub const PRIORITY_ABOVE_NORMAL: i32 = -7;
pub const PRIORITY_HIGH: i32 = -14;
#[cfg(windows)]
pub const PRIORITY_HIGHEST: i32 = -20;

#[cfg(unix)]
pub fn get_priority(pid: u32) -> Result<i32, AnyError> {
  set_errno(Errno(0));
  match (
    // SAFETY: libc::getpriority is unsafe
    unsafe { libc::getpriority(PRIO_PROCESS as priority_t, pid as id_t) },
    errno(),
  ) {
    (-1, Errno(0)) => Ok(PRIORITY_HIGH),
    (-1, _) => Err(std::io::Error::last_os_error().into()),
    (priority, _) => Ok(priority),
  }
}

#[cfg(unix)]
pub fn set_priority(pid: u32, priority: i32) -> Result<(), AnyError> {
  match unsafe {
    // SAFETY: libc::setpriority is unsafe
    libc::setpriority(PRIO_PROCESS as priority_t, pid as id_t, priority)
  } {
    -1 => Err(std::io::Error::last_os_error().into()),
    _ => Ok(()),
  }
}

#[cfg(windows)]
pub fn get_priority(pid: u32) -> Result<i32, AnyError> {
  unsafe {
    let handle = if pid == 0 {
      GetCurrentProcess()
    } else {
      OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, FALSE, pid as DWORD)
    };
    if handle == NULL {
      Err(std::io::Error::last_os_error().into())
    } else {
      let result = match GetPriorityClass(handle) {
        0 => Err(std::io::Error::last_os_error().into()),
        REALTIME_PRIORITY_CLASS => Ok(PRIORITY_HIGHEST),
        HIGH_PRIORITY_CLASS => Ok(PRIORITY_HIGH),
        ABOVE_NORMAL_PRIORITY_CLASS => Ok(PRIORITY_ABOVE_NORMAL),
        NORMAL_PRIORITY_CLASS => Ok(PRIORITY_NORMAL),
        BELOW_NORMAL_PRIORITY_CLASS => Ok(PRIORITY_BELOW_NORMAL),
        IDLE_PRIORITY_CLASS => Ok(PRIORITY_LOW),
        _ => Ok(PRIORITY_LOW),
      };
      CloseHandle(handle);
      result
    }
  }
}

#[cfg(windows)]
pub fn set_priority(pid: u32, priority: i32) -> Result<(), AnyError> {
  unsafe {
    let handle = if pid == 0 {
      GetCurrentProcess()
    } else {
      OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, FALSE, pid as DWORD)
    };
    if handle == NULL {
      Err(std::io::Error::last_os_error().into())
    } else {
      let prio_class = match priority {
        p if p <= PRIORITY_HIGHEST => REALTIME_PRIORITY_CLASS,
        p if PRIORITY_HIGHEST < p && p <= PRIORITY_HIGH => HIGH_PRIORITY_CLASS,
        p if PRIORITY_HIGH < p && p <= PRIORITY_ABOVE_NORMAL => {
          ABOVE_NORMAL_PRIORITY_CLASS
        }
        p if PRIORITY_ABOVE_NORMAL < p && p <= PRIORITY_NORMAL => {
          NORMAL_PRIORITY_CLASS
        }
        p if PRIORITY_NORMAL < p && p <= PRIORITY_BELOW_NORMAL => {
          BELOW_NORMAL_PRIORITY_CLASS
        }
        _ => IDLE_PRIORITY_CLASS,
      };
      let result = match SetPriorityClass(handle, prio_class) {
        FALSE => Err(std::io::Error::last_os_error().into()),
        _ => Ok(()),
      };
      CloseHandle(handle);
      result
    }
  }
}
