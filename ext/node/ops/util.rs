// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use deno_core::error::AnyError;
use deno_core::op2;
use deno_core::OpState;
use deno_core::ResourceHandle;
use deno_core::ResourceHandleFd;

#[repr(u32)]
enum HandleType {
  #[allow(dead_code)]
  TCP = 0,
  TTY,
  #[allow(dead_code)]
  UDP,
  FILE,
  PIPE,
  UNKNOWN,
}

#[op2(fast)]
pub fn op_node_guess_handle_type(
  state: &mut OpState,
  rid: u32,
) -> Result<u32, AnyError> {
  let handle = state.resource_table.get_handle(rid)?;
  let handle_type = match handle {
    ResourceHandle::Fd(handle) => guess_handle_type(handle),
    _ => HandleType::UNKNOWN,
  };

  Ok(handle_type as u32)
}

#[cfg(windows)]
fn guess_handle_type(handle: ResourceHandleFd) -> HandleType {
  use winapi::um::{
    consoleapi::GetConsoleMode,
    fileapi::GetFileType,
    winbase::{FILE_TYPE_CHAR, FILE_TYPE_DISK, FILE_TYPE_PIPE},
  };

  let mut mode = 0;
  match unsafe { GetFileType(handle) } {
    FILE_TYPE_DISK => HandleType::FILE,
    FILE_TYPE_CHAR => {
      if unsafe { GetConsoleMode(handle, &mut mode) } == 1 {
        HandleType::TTY
      } else {
        HandleType::FILE
      }
    }
    FILE_TYPE_PIPE => HandleType::PIPE,
    _ => HandleType::UNKNOWN,
  }
}

#[cfg(unix)]
fn guess_handle_type(handle: ResourceHandleFd) -> HandleType {
  // SAFETY: The resource remains open for the for the duration of borrow_raw
  if unsafe { std::os::fd::BorrowedFd::borrow_raw(fd).is_terminal() } {
    return HandleType::TTY;
  }

  let mut s = unsafe { std::mem::zeroed() };
  if libc::fstat(handle, &mut s) == 1 {
    return HandleType::UNKNOWN;
  }

  match s.st_mode & 61440 {
    libc::S_IFREG | libc::S_IFCHR => HandleType::FILE,
    libc::S_IFIFO => HandleType::PIPE,
    libc::S_IFSOCK => HandleType::TCP,
    _ => HandleType::UNKNOWN,
  }
}
