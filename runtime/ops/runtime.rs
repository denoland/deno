// Copyright 2018-2025 the Deno authors. MIT license.

use deno_core::op2;
use deno_core::ModuleSpecifier;
use deno_core::OpState;

deno_core::extension!(
  deno_runtime,
  ops = [op_main_module, op_ppid, op_internal_log],
  options = { main_module: ModuleSpecifier },
  state = |state, options| {
    state.put::<ModuleSpecifier>(options.main_module);
  },
);

#[op2]
#[string]
fn op_main_module(state: &mut OpState) -> String {
  let main_url = state.borrow::<ModuleSpecifier>();
  main_url.to_string()
}

/// This is an op instead of being done at initialization time because
/// it's expensive to retrieve the ppid on Windows.
#[op2(fast)]
#[number]
pub fn op_ppid() -> i64 {
  #[cfg(windows)]
  {
    // Adopted from rustup:
    // https://github.com/rust-lang/rustup/blob/1.21.1/src/cli/self_update.rs#L1036
    // Copyright Diggory Blake, the Mozilla Corporation, and rustup contributors.
    // Licensed under either of
    // - Apache License, Version 2.0
    // - MIT license
    use std::mem;

    use winapi::shared::minwindef::DWORD;
    use winapi::um::handleapi::CloseHandle;
    use winapi::um::handleapi::INVALID_HANDLE_VALUE;
    use winapi::um::processthreadsapi::GetCurrentProcessId;
    use winapi::um::tlhelp32::CreateToolhelp32Snapshot;
    use winapi::um::tlhelp32::Process32First;
    use winapi::um::tlhelp32::Process32Next;
    use winapi::um::tlhelp32::PROCESSENTRY32;
    use winapi::um::tlhelp32::TH32CS_SNAPPROCESS;
    // SAFETY: winapi calls
    unsafe {
      // Take a snapshot of system processes, one of which is ours
      // and contains our parent's pid
      let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
      if snapshot == INVALID_HANDLE_VALUE {
        return -1;
      }

      let mut entry: PROCESSENTRY32 = mem::zeroed();
      entry.dwSize = mem::size_of::<PROCESSENTRY32>() as DWORD;

      // Iterate over system processes looking for ours
      let success = Process32First(snapshot, &mut entry);
      if success == 0 {
        CloseHandle(snapshot);
        return -1;
      }

      let this_pid = GetCurrentProcessId();
      while entry.th32ProcessID != this_pid {
        let success = Process32Next(snapshot, &mut entry);
        if success == 0 {
          CloseHandle(snapshot);
          return -1;
        }
      }
      CloseHandle(snapshot);

      // FIXME: Using the process ID exposes a race condition
      // wherein the parent process already exited and the OS
      // reassigned its ID.
      let parent_id = entry.th32ParentProcessID;
      parent_id.into()
    }
  }
  #[cfg(not(windows))]
  {
    use std::os::unix::process::parent_id;
    parent_id().into()
  }
}

#[allow(clippy::match_single_binding)] // needed for temporary lifetime
#[op2(fast)]
fn op_internal_log(
  #[string] url: &str,
  #[smi] level: u32,
  #[string] message: &str,
) {
  let level = match level {
    1 => log::Level::Error,
    2 => log::Level::Warn,
    3 => log::Level::Info,
    4 => log::Level::Debug,
    5 => log::Level::Trace,
    _ => unreachable!(),
  };
  let target = url.replace('/', "::");
  match format_args!("{message}") {
    args => {
      let record = log::Record::builder()
        .file(Some(url))
        .module_path(Some(url))
        .target(&target)
        .level(level)
        .args(args)
        .build();
      log::logger().log(&record);
    }
  }
}
