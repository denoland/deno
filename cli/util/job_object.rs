// Copyright 2018-2026 the Deno authors. MIT license.

//! Windows job object with [`JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`].
//!
//! Used for desktop subprocess trees (LAUFEY, framework dev servers) so
//! grandchildren are torn down when the parent closes the job handle or exits.
//! Unlike the runtime's global job in `subprocess_windows`, this job does not
//! set breakaway flags — see `runtime/subprocess_windows/src/process.rs`.

use deno_core::error::AnyError;

/// Placeholder on non-Windows targets.
#[cfg(not(windows))]
pub struct KillOnCloseJob;

#[cfg(not(windows))]
impl KillOnCloseJob {
  pub fn new() -> Result<Self, AnyError> {
    Ok(Self)
  }

  pub fn assign_child(
    &self,
    _child: &tokio::process::Child,
  ) -> Result<(), AnyError> {
    Ok(())
  }
}

#[cfg(windows)]
pub struct KillOnCloseJob {
  handle: windows_sys::Win32::Foundation::HANDLE,
}

#[cfg(windows)]
impl KillOnCloseJob {
  pub fn new() -> Result<Self, AnyError> {
    use std::mem;
    use std::ptr;

    use windows_sys::Win32::Foundation::FALSE;
    use windows_sys::Win32::Foundation::GetLastError;
    use windows_sys::Win32::Security::SECURITY_ATTRIBUTES;
    use windows_sys::Win32::System::JobObjects::*;

    // SAFETY: Win32 job-object setup; `SECURITY_ATTRIBUTES` and limit info are
    // zeroed POD structs passed to CreateJobObjectW / SetInformationJobObject.
    let handle = unsafe {
      let mut attr = mem::zeroed::<SECURITY_ATTRIBUTES>();
      attr.bInheritHandle = FALSE;

      let mut info = mem::zeroed::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>();
      info.BasicLimitInformation.LimitFlags =
        JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;

      let job = CreateJobObjectW(&attr, ptr::null());
      if job.is_null() {
        return Err(std::io::Error::last_os_error().into());
      }

      if SetInformationJobObject(
        job,
        JobObjectExtendedLimitInformation,
        &raw const info as _,
        mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
      ) == 0
      {
        let err = GetLastError();
        windows_sys::Win32::Foundation::CloseHandle(job);
        return Err(std::io::Error::from_raw_os_error(err as i32).into());
      }

      job
    };

    Ok(Self { handle })
  }

  /// Assigns `child` and its descendants (without breakaway) to this job.
  pub fn assign_child(
    &self,
    child: &tokio::process::Child,
  ) -> Result<(), AnyError> {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::Foundation::ERROR_ACCESS_DENIED;
    use windows_sys::Win32::Foundation::FALSE;
    use windows_sys::Win32::Foundation::GetLastError;
    use windows_sys::Win32::System::JobObjects::AssignProcessToJobObject;
    use windows_sys::Win32::System::Threading::OpenProcess;
    use windows_sys::Win32::System::Threading::PROCESS_SET_QUOTA;
    use windows_sys::Win32::System::Threading::PROCESS_TERMINATE;

    let pid = child.id().ok_or_else(|| {
      std::io::Error::new(
        std::io::ErrorKind::NotFound,
        "child process has no pid (already exited?)",
      )
    })?;

    // SAFETY: OpenProcess + AssignProcessToJobObject; handle always closed.
    unsafe {
      let process =
        OpenProcess(PROCESS_SET_QUOTA | PROCESS_TERMINATE, FALSE, pid);
      if process.is_null() {
        return Err(std::io::Error::last_os_error().into());
      }

      let assigned = AssignProcessToJobObject(self.handle, process);
      let err = GetLastError();
      CloseHandle(process);

      if assigned == 0 {
        if err == ERROR_ACCESS_DENIED {
          // Process may already be under job control without nested jobs.
          log::debug!(
            "AssignProcessToJobObject(pid={pid}): access denied; \
             subprocess tree may outlive parent"
          );
          return Ok(());
        }
        return Err(std::io::Error::from_raw_os_error(err as i32).into());
      }
    }

    Ok(())
  }
}

#[cfg(windows)]
impl Drop for KillOnCloseJob {
  fn drop(&mut self) {
    // SAFETY: job handle from CreateJobObjectW.
    unsafe {
      windows_sys::Win32::Foundation::CloseHandle(self.handle);
    }
  }
}

#[cfg(all(windows, test))]
mod tests {
  use super::*;

  /// Smoke test: spawn `cmd /C exit 0` in a kill-on-close job.
  #[tokio::test]
  async fn assign_cmd_exit_zero() {
    let job = KillOnCloseJob::new().unwrap();
    let mut child = tokio::process::Command::new("cmd")
      .args(["/C", "exit", "0"])
      .spawn()
      .unwrap();
    job.assign_child(&child).unwrap();
    let status = child.wait().await.unwrap();
    assert!(status.success());
  }
}
