// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use deno_core::error::custom_error;
use deno_core::error::type_error;
use deno_core::error::AnyError;
use deno_core::op;

#[op]
pub fn op_node_os_get_priority(pid: u32) -> Result<i32, AnyError> {
  get_priority(pid)
}

#[op]
pub fn op_node_os_set_priority(
  pid: u32,
  priority: i32,
) -> Result<(), AnyError> {
  set_priority(pid, priority)
}

#[op]
pub fn op_node_os_username() -> Result<String, AnyError> {
  Ok(whoami::username())
}

fn path_into_string(s: std::ffi::OsString) -> Result<String, AnyError> {
  s.into_string().map_err(|s| {
    let message = format!("File name or path {s:?} is not valid UTF-8");
    custom_error("InvalidData", message)
  })
}

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

pub const PRIORITY_LOW: i32 = 19;
pub const PRIORITY_BELOW_NORMAL: i32 = 10;
pub const PRIORITY_NORMAL: i32 = 0;
pub const PRIORITY_ABOVE_NORMAL: i32 = -7;
pub const PRIORITY_HIGH: i32 = -14;
pub const PRIORITY_HIGHEST: i32 = -20;

#[cfg(unix)]
pub fn get_priority(pid: u32) -> Result<i32, AnyError> {
  set_errno(Errno(0));
  match (unsafe { libc::getpriority(PRIO_PROCESS, pid) }, errno()) {
    (-1, Errno(0)) => Ok(PRIORITY_HIGH),
    (-1, _) => Err(std::io::Error::last_os_error().into()),
    (priority, _) => Ok(priority),
  }
}

#[cfg(unix)]
pub fn set_priority(pid: u32, priority: i32) -> Result<(), AnyError> {
  match unsafe { libc::setpriority(PRIO_PROCESS, pid, priority) } {
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
