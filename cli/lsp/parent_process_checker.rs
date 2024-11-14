// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::time::Duration;

/// Starts a thread that will check for the existence of the
/// provided process id. Once that process no longer exists
/// it will terminate the current process.
pub fn start(parent_process_id: u32) {
  // use a separate thread in case the runtime gets hung up
  std::thread::spawn(move || loop {
    std::thread::sleep(Duration::from_secs(10));

    if !is_process_active(parent_process_id) {
      deno_runtime::exit(1);
    }
  });
}

#[cfg(unix)]
fn is_process_active(process_id: u32) -> bool {
  // TODO(bartlomieju):
  #[allow(clippy::undocumented_unsafe_blocks)]
  unsafe {
    // signal of 0 checks for the existence of the process id
    libc::kill(process_id as i32, 0) == 0
  }
}

#[cfg(windows)]
fn is_process_active(process_id: u32) -> bool {
  use winapi::shared::minwindef::DWORD;
  use winapi::shared::minwindef::FALSE;
  use winapi::shared::ntdef::NULL;
  use winapi::shared::winerror::WAIT_TIMEOUT;
  use winapi::um::handleapi::CloseHandle;
  use winapi::um::processthreadsapi::OpenProcess;
  use winapi::um::synchapi::WaitForSingleObject;
  use winapi::um::winnt::SYNCHRONIZE;

  // SAFETY: winapi calls
  unsafe {
    let process = OpenProcess(SYNCHRONIZE, FALSE, process_id as DWORD);
    let result = if process == NULL {
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
  use super::is_process_active;
  use std::process::Command;
  use test_util::deno_exe_path;

  #[test]
  fn process_active() {
    // launch a long running process
    let mut child = Command::new(deno_exe_path()).arg("lsp").spawn().unwrap();

    let pid = child.id();
    assert!(is_process_active(pid));
    child.kill().unwrap();
    child.wait().unwrap();
    assert!(!is_process_active(pid));
  }
}
