// Copyright 2018-2026 the Deno authors. MIT license.

use std::time::Duration;

/// Starts a thread that will check for the existence of the
/// provided process id. Once that process no longer exists
/// it will terminate the current process.
pub fn start(parent_process_id: u32) {
  // use a separate thread in case the runtime gets hung up
  std::thread::spawn(move || {
    loop {
      std::thread::sleep(Duration::from_secs(10));

      if !is_process_active(parent_process_id) {
        deno_runtime::exit(1);
      }
    }
  });
}

#[cfg(unix)]
fn is_process_active(process_id: u32) -> bool {
  // TODO(bartlomieju):
  // SAFETY: kill with signal 0 only checks process existence, no side effects
  unsafe {
    // signal of 0 checks for the existence of the process id
    libc::kill(process_id as i32, 0) == 0
  }
}

#[cfg(windows)]
fn is_process_active(process_id: u32) -> bool {
  use windows_sys::Win32::Foundation::CloseHandle;
  use windows_sys::Win32::Foundation::FALSE;
  use windows_sys::Win32::Foundation::WAIT_TIMEOUT;
  use windows_sys::Win32::Storage::FileSystem::SYNCHRONIZE;
  use windows_sys::Win32::System::Threading::OpenProcess;
  use windows_sys::Win32::System::Threading::WaitForSingleObject;

  // SAFETY: Win32 calls
  unsafe {
    let process = OpenProcess(SYNCHRONIZE, FALSE, process_id);
    let result = if process.is_null() {
      false
    } else {
      WaitForSingleObject(process, 0) == WAIT_TIMEOUT
    };
    CloseHandle(process);
    result
  }
}

#[cfg(test)]
mod test {
  use std::process::Command;
  use std::process::Stdio;

  use super::is_process_active;

  #[test]
  fn process_active() {
    // launch a long running process that blocks on stdin
    let mut child = Command::new(if cfg!(windows) { "cmd.exe" } else { "cat" })
      .stdin(Stdio::piped())
      .stdout(Stdio::null())
      .stderr(Stdio::null())
      .spawn()
      .unwrap();

    let pid = child.id();
    assert!(is_process_active(pid));
    child.kill().unwrap();
    child.wait().unwrap();
    assert!(!is_process_active(pid));
  }
}
