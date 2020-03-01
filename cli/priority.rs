// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use crate::op_error::OpError;

#[cfg(unix)]
use errno::{errno, set_errno, Errno};
#[cfg(unix)]
use libc::{id_t, PRIO_PROCESS};
#[cfg(windows)]
use winapi::shared::minwindef::{DWORD, FALSE};
#[cfg(windows)]
use winapi::shared::ntdef::NULL;
#[cfg(windows)]
use winapi::um::handleapi::CloseHandle;
#[cfg(windows)]
use winapi::um::processthreadsapi::{
  GetCurrentProcess, GetPriorityClass, OpenProcess, SetPriorityClass,
};
#[cfg(windows)]
use winapi::um::winbase::{
  ABOVE_NORMAL_PRIORITY_CLASS, BELOW_NORMAL_PRIORITY_CLASS,
  HIGH_PRIORITY_CLASS, IDLE_PRIORITY_CLASS, NORMAL_PRIORITY_CLASS,
  REALTIME_PRIORITY_CLASS,
};
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
pub fn get_priority(pid: u32) -> Result<i32, OpError> {
  unsafe {
    set_errno(Errno(0));
    match (
      libc::getpriority(PRIO_PROCESS as priority_t, pid as id_t),
      errno(),
    ) {
      (-1, Errno(0)) => Ok(PRIORITY_HIGH),
      (-1, _) => Err(OpError::from(std::io::Error::last_os_error())),
      (priority, _) => Ok(priority),
    }
  }
}

#[cfg(unix)]
pub fn set_priority(pid: u32, priority: i32) -> Result<(), OpError> {
  unsafe {
    match libc::setpriority(PRIO_PROCESS as priority_t, pid as id_t, priority) {
      -1 => Err(OpError::from(std::io::Error::last_os_error())),
      _ => Ok(()),
    }
  }
}

#[cfg(windows)]
pub fn get_priority(pid: u32) -> Result<i32, OpError> {
  unsafe {
    let handle = if pid == 0 {
      GetCurrentProcess()
    } else {
      OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, FALSE, pid as DWORD)
    };
    if handle == NULL {
      Err(OpError::from(std::io::Error::last_os_error()))
    } else {
      let result = match GetPriorityClass(handle) {
        0 => Err(OpError::from(std::io::Error::last_os_error())),
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
pub fn set_priority(pid: u32, priority: i32) -> Result<(), OpError> {
  unsafe {
    let handle = if pid == 0 {
      GetCurrentProcess()
    } else {
      OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, FALSE, pid as DWORD)
    };
    if handle == NULL {
      Err(OpError::from(std::io::Error::last_os_error()))
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
        FALSE => Err(OpError::from(std::io::Error::last_os_error())),
        _ => Ok(()),
      };
      CloseHandle(handle);
      result
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_get_current_process_priority() {
    get_priority(0).expect("Should get priority");
  }

  #[cfg(unix)]
  #[test]
  fn test_set_current_process_high_priority_should_fail() {
    assert!(set_priority(0, PRIORITY_HIGH).is_err());
  }

  /// this test makes multiple tests at once
  /// because we need to set them in order and rust
  /// does not guarantee test order execution
  #[test]
  fn test_set_current_process_priority_from_normal_to_low() {
    set_priority(0, PRIORITY_NORMAL).expect("Should set priority");
    let priority = get_priority(0).expect("Should get priority");
    assert_eq!(priority, PRIORITY_NORMAL);

    set_priority(0, PRIORITY_BELOW_NORMAL).expect("Should set priority");
    let priority = get_priority(0).expect("Should get priority");
    assert_eq!(priority, PRIORITY_BELOW_NORMAL);

    set_priority(0, PRIORITY_LOW).expect("Should set priority");
    let priority = get_priority(0).expect("Should get priority");
    assert_eq!(priority, PRIORITY_LOW);
  }
}
