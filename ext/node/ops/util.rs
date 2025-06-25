// Copyright 2018-2025 the Deno authors. MIT license.

use deno_core::op2;
use deno_core::v8;
use deno_core::OpState;
use deno_core::ResourceHandle;
use deno_core::ResourceHandleFd;
use node_resolver::InNpmPackageChecker;
use node_resolver::NpmPackageFolderResolver;

use crate::ExtNodeSys;
use crate::NodeResolverRc;

#[repr(u32)]
enum HandleType {
  #[allow(dead_code)]
  Tcp = 0,
  Tty,
  #[allow(dead_code)]
  Udp,
  File,
  Pipe,
  Unknown,
}

#[op2(fast)]
pub fn op_node_guess_handle_type(state: &mut OpState, rid: u32) -> u32 {
  let handle = match state.resource_table.get_handle(rid) {
    Ok(handle) => handle,
    _ => return HandleType::Unknown as u32,
  };

  let handle_type = match handle {
    ResourceHandle::Fd(handle) => guess_handle_type(handle),
    _ => HandleType::Unknown,
  };

  handle_type as u32
}

#[cfg(windows)]
fn guess_handle_type(handle: ResourceHandleFd) -> HandleType {
  use winapi::um::consoleapi::GetConsoleMode;
  use winapi::um::fileapi::GetFileType;
  use winapi::um::winbase::FILE_TYPE_CHAR;
  use winapi::um::winbase::FILE_TYPE_DISK;
  use winapi::um::winbase::FILE_TYPE_PIPE;

  // SAFETY: Call to win32 fileapi. `handle` is a valid fd.
  match unsafe { GetFileType(handle) } {
    FILE_TYPE_DISK => HandleType::File,
    FILE_TYPE_CHAR => {
      let mut mode = 0;
      // SAFETY: Call to win32 consoleapi. `handle` is a valid fd.
      //         `mode` is a valid pointer.
      if unsafe { GetConsoleMode(handle, &mut mode) } == 1 {
        HandleType::Tty
      } else {
        HandleType::File
      }
    }
    FILE_TYPE_PIPE => HandleType::Pipe,
    _ => HandleType::Unknown,
  }
}

#[cfg(unix)]
fn guess_handle_type(handle: ResourceHandleFd) -> HandleType {
  use std::io::IsTerminal;
  // SAFETY: The resource remains open for the duration of borrow_raw.
  if unsafe { std::os::fd::BorrowedFd::borrow_raw(handle).is_terminal() } {
    return HandleType::Tty;
  }

  // SAFETY: It is safe to zero-initialize a `libc::stat` struct.
  let mut s = unsafe { std::mem::zeroed() };
  // SAFETY: Call to libc
  if unsafe { libc::fstat(handle, &mut s) } == 1 {
    return HandleType::Unknown;
  }

  match s.st_mode & 61440 {
    libc::S_IFREG | libc::S_IFCHR => HandleType::File,
    libc::S_IFIFO => HandleType::Pipe,
    libc::S_IFSOCK => HandleType::Tcp,
    _ => HandleType::Unknown,
  }
}

#[op2(fast)]
pub fn op_node_view_has_buffer(buffer: v8::Local<v8::ArrayBufferView>) -> bool {
  buffer.has_buffer()
}

/// Checks if the current call site is from a dependency package.
#[op2(fast)]
pub fn op_node_call_is_from_dependency<
  TInNpmPackageChecker: InNpmPackageChecker + 'static,
  TNpmPackageFolderResolver: NpmPackageFolderResolver + 'static,
  TSys: ExtNodeSys + 'static,
>(
  state: &mut OpState,
  scope: &mut v8::HandleScope,
) -> bool {
  // non internal call site should appear in < 20 frames
  let Some(stack_trace) = v8::StackTrace::current_stack_trace(scope, 20) else {
    return false;
  };
  for i in 0..stack_trace.get_frame_count() {
    let Some(frame) = stack_trace.get_frame(scope, i) else {
      continue;
    };
    if !frame.is_user_javascript() {
      continue;
    }
    let Some(script) = frame.get_script_name(scope) else {
      continue;
    };
    let name = script.to_rust_string_lossy(scope);
    if name.starts_with("node:") || name.starts_with("ext:") {
      continue;
    } else if name.starts_with("https:")
      || name.contains("/node_modules/")
      || name.contains(r"\node_modules\")
    {
      return true;
    } else {
      let Ok(specifier) = url::Url::parse(&name) else {
        continue;
      };
      return state.borrow::<NodeResolverRc<
        TInNpmPackageChecker,
        TNpmPackageFolderResolver,
        TSys,
      >>().in_npm_package(&specifier);
    }
  }
  false
}
