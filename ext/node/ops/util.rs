// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use deno_core::error::AnyError;
use deno_core::op2;
use deno_core::OpState;
use deno_core::ResourceHandle;
use deno_core::ResourceHandleFd;

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
pub fn op_node_guess_handle_type(
  state: &mut OpState,
  rid: u32,
) -> Result<u32, AnyError> {
  let handle = state.resource_table.get_handle(rid)?;

  if handle.is_terminal() {
    return Ok(HandleType::Tty as u32);
  }

  let handle_type = match handle {
    ResourceHandle::Fd(handle) => guess_handle_type(handle),
    _ => HandleType::Unknown,
  };

  Ok(handle_type as u32)
}

#[cfg(windows)]
fn guess_handle_type(handle: ResourceHandleFd) -> HandleType {
  use winapi::um::fileapi::GetFileType;
  use winapi::um::winbase::FILE_TYPE_CHAR;
  use winapi::um::winbase::FILE_TYPE_DISK;
  use winapi::um::winbase::FILE_TYPE_PIPE;

  // SAFETY: Call to win32 fileapi. `handle` is a valid fd.
  match unsafe { GetFileType(handle) } {
    FILE_TYPE_DISK | FILE_TYPE_CHAR => HandleType::File,
    FILE_TYPE_PIPE => HandleType::Pipe,
    _ => HandleType::Unknown,
  }
}

#[cfg(unix)]
fn guess_handle_type(handle: ResourceHandleFd) -> HandleType {
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
