// Copyright 2018-2026 the Deno authors. MIT license.

pub use impl_::*;

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum PriorityError {
  #[class(inherit)]
  #[error("{0}")]
  Io(#[from] std::io::Error),
  #[cfg(windows)]
  #[class(type)]
  #[error("Invalid priority")]
  InvalidPriority,
}

#[cfg(unix)]
mod impl_ {
  use errno::Errno;
  use errno::errno;
  use errno::set_errno;
  use libc::PRIO_PROCESS;
  use libc::id_t;

  // Ref: https://github.com/libuv/libuv/blob/55376b044b74db40772e8a6e24d67a8673998e02/src/unix/core.c#L1533-L1547
  pub fn get_priority(pid: u32) -> Result<i32, super::PriorityError> {
    set_errno(Errno(0));
    match (
      // SAFETY: libc::getpriority is unsafe
      unsafe { libc::getpriority(PRIO_PROCESS, pid as id_t) },
      errno(),
    ) {
      (-1, Errno(0)) => Ok(-1),
      (-1, _) => Err(std::io::Error::last_os_error().into()),
      (priority, _) => Ok(priority),
    }
  }

  pub fn set_priority(
    pid: u32,
    priority: i32,
  ) -> Result<(), super::PriorityError> {
    // SAFETY: libc::setpriority is unsafe
    match unsafe { libc::setpriority(PRIO_PROCESS, pid as id_t, priority) } {
      -1 => Err(std::io::Error::last_os_error().into()),
      _ => Ok(()),
    }
  }
}

#[cfg(windows)]
mod impl_ {
  use windows_sys::Win32::Foundation::CloseHandle;
  use windows_sys::Win32::Foundation::FALSE;
  use windows_sys::Win32::System::Threading::ABOVE_NORMAL_PRIORITY_CLASS;
  use windows_sys::Win32::System::Threading::BELOW_NORMAL_PRIORITY_CLASS;
  use windows_sys::Win32::System::Threading::GetCurrentProcess;
  use windows_sys::Win32::System::Threading::GetPriorityClass;
  use windows_sys::Win32::System::Threading::HIGH_PRIORITY_CLASS;
  use windows_sys::Win32::System::Threading::IDLE_PRIORITY_CLASS;
  use windows_sys::Win32::System::Threading::NORMAL_PRIORITY_CLASS;
  use windows_sys::Win32::System::Threading::OpenProcess;
  use windows_sys::Win32::System::Threading::PROCESS_QUERY_LIMITED_INFORMATION;
  use windows_sys::Win32::System::Threading::PROCESS_SET_INFORMATION;
  use windows_sys::Win32::System::Threading::REALTIME_PRIORITY_CLASS;
  use windows_sys::Win32::System::Threading::SetPriorityClass;

  // Taken from: https://github.com/libuv/libuv/blob/a877ca2435134ef86315326ef4ef0c16bdbabf17/include/uv.h#L1318-L1323
  const PRIORITY_LOW: i32 = 19;
  const PRIORITY_BELOW_NORMAL: i32 = 10;
  const PRIORITY_NORMAL: i32 = 0;
  const PRIORITY_ABOVE_NORMAL: i32 = -7;
  const PRIORITY_HIGH: i32 = -14;
  const PRIORITY_HIGHEST: i32 = -20;

  // Ported from: https://github.com/libuv/libuv/blob/a877ca2435134ef86315326ef4ef0c16bdbabf17/src/win/util.c#L1649-L1685
  pub fn get_priority(pid: u32) -> Result<i32, super::PriorityError> {
    // SAFETY: Windows API calls
    unsafe {
      let handle = if pid == 0 {
        GetCurrentProcess()
      } else {
        OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, FALSE, pid as u32)
      };
      if handle.is_null() {
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

  // Ported from: https://github.com/libuv/libuv/blob/a877ca2435134ef86315326ef4ef0c16bdbabf17/src/win/util.c#L1688-L1719
  pub fn set_priority(
    pid: u32,
    priority: i32,
  ) -> Result<(), super::PriorityError> {
    // SAFETY: Windows API calls
    unsafe {
      let handle = if pid == 0 {
        GetCurrentProcess()
      } else {
        OpenProcess(PROCESS_SET_INFORMATION, FALSE, pid as u32)
      };
      if handle.is_null() {
        Err(std::io::Error::last_os_error().into())
      } else {
        let priority_class =
          if !(PRIORITY_HIGHEST..=PRIORITY_LOW).contains(&priority) {
            return Err(super::PriorityError::InvalidPriority);
          } else if priority < PRIORITY_HIGH {
            REALTIME_PRIORITY_CLASS
          } else if priority < PRIORITY_ABOVE_NORMAL {
            HIGH_PRIORITY_CLASS
          } else if priority < PRIORITY_NORMAL {
            ABOVE_NORMAL_PRIORITY_CLASS
          } else if priority < PRIORITY_BELOW_NORMAL {
            NORMAL_PRIORITY_CLASS
          } else if priority < PRIORITY_LOW {
            BELOW_NORMAL_PRIORITY_CLASS
          } else {
            IDLE_PRIORITY_CLASS
          };

        let result = match SetPriorityClass(handle, priority_class) {
          FALSE => Err(std::io::Error::last_os_error().into()),
          _ => Ok(()),
        };
        CloseHandle(handle);
        result
      }
    }
  }
}
