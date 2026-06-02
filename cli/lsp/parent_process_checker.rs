// Copyright 2018-2026 the Deno authors. MIT license.

use std::time::Duration;

/// Starts a thread that will check for the existence of the
/// provided process id. Once that process no longer exists
/// it will terminate the current process.
///
/// If the process is not visible from the current PID namespace at
/// startup (for example, when `deno lsp` runs inside a container and
/// the editor lives on the host), the monitor is skipped — exiting
/// based on a `kill(pid, 0)` that can never succeed would terminate
/// the language server while the editor is still alive. The stdio
/// stream closing on parent exit remains the fallback signal.
///
/// Returns `true` if the background monitor was started.
pub fn start(parent_process_id: u32) -> bool {
  if !is_process_active(parent_process_id) {
    log::debug!(
      "Skipping parent process check: PID {parent_process_id} is not visible from this PID namespace (likely running in a container)."
    );
    return false;
  }
  // use a separate thread in case the runtime gets hung up
  std::thread::spawn(move || {
    loop {
      std::thread::sleep(Duration::from_secs(10));

      if !is_process_active(parent_process_id) {
        deno_runtime::exit(1);
      }
    }
  });
  true
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
  use std::process::Command;
  use std::process::Stdio;

  use super::is_process_active;
  use super::start;

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

  // Regression test for https://github.com/denoland/deno/issues/22012:
  // When the editor's PID is not visible from the language server's PID
  // namespace (e.g. `deno lsp` running in a Docker container and the
  // editor running on the host), `start` must not begin the monitor —
  // otherwise the very first poll would exit the LSP with status 1.
  #[test]
  fn start_skips_when_pid_not_visible() {
    // pick a PID that is unlikely to be in use and was never observed by
    // this process — start by spawning a child, capturing its PID, then
    // waiting for it to fully exit so the PID is no longer active.
    let mut child = Command::new(if cfg!(windows) { "cmd.exe" } else { "cat" })
      .stdin(Stdio::piped())
      .stdout(Stdio::null())
      .stderr(Stdio::null())
      .spawn()
      .unwrap();
    let pid = child.id();
    child.kill().unwrap();
    child.wait().unwrap();
    assert!(!is_process_active(pid));

    // start() should return false rather than spawning a thread that
    // would immediately exit() the test process.
    assert!(!start(pid));
  }
}
