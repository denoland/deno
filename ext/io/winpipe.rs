// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
use rand::thread_rng;
use rand::RngCore;
use std::io;
use std::os::windows::io::RawHandle;
use winapi::shared::minwindef::DWORD;
use winapi::um::fileapi::CreateFileA;
use winapi::um::fileapi::OPEN_EXISTING;
use winapi::um::handleapi::CloseHandle;
use winapi::um::handleapi::INVALID_HANDLE_VALUE;
use winapi::um::minwinbase::SECURITY_ATTRIBUTES;
use winapi::um::winbase::CreateNamedPipeA;
use winapi::um::winbase::FILE_FLAG_FIRST_PIPE_INSTANCE;
use winapi::um::winbase::FILE_FLAG_OVERLAPPED;
use winapi::um::winbase::PIPE_TYPE_BYTE;
use winapi::um::winnt::FILE_ATTRIBUTE_NORMAL;
use winapi::um::winnt::GENERIC_READ;
use winapi::um::winnt::GENERIC_WRITE;

/// Create a pair of file descriptors for a named pipe with non-inheritable handles
pub fn create_named_pipe() -> io::Result<(RawHandle, RawHandle)> {
  let pipe_name = format!(
    r#"\\.\pipe\deno_pipe_{:x}_{:x}\0"#,
    std::process::id(),
    thread_rng().next_u64()
  );

  // Create security attributes to make the pipe handles non-inheritable
  let mut security_attributes = SECURITY_ATTRIBUTES {
    nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as DWORD,
    lpSecurityDescriptor: std::ptr::null_mut(),
    bInheritHandle: 0,
  };

  // SAFETY: Create the pipe server with non-inheritable handle
  let server_handle = unsafe {
    CreateNamedPipeA(
      pipe_name.as_ptr() as *const i8,
      GENERIC_READ
        | GENERIC_WRITE
        | FILE_FLAG_OVERLAPPED
        | FILE_FLAG_FIRST_PIPE_INSTANCE,
      PIPE_TYPE_BYTE,
      1,
      4096,
      4096,
      0,
      &mut security_attributes,
    )
  };

  if server_handle == INVALID_HANDLE_VALUE {
    return Err(io::Error::last_os_error());
  }

  // SAFETY: Create the pipe client with non-inheritable handle
  let client_handle = unsafe {
    CreateFileA(
      pipe_name.as_ptr() as *const i8,
      GENERIC_READ | GENERIC_WRITE | FILE_FLAG_OVERLAPPED,
      0,
      std::ptr::null_mut(),
      OPEN_EXISTING,
      FILE_ATTRIBUTE_NORMAL,
      std::ptr::null_mut(),
    )
  };

  if client_handle == INVALID_HANDLE_VALUE {
    // SAFETY: Close the handles if we failed
    unsafe {
      CloseHandle(server_handle);
    }
    return Err(io::Error::last_os_error());
  }

  Ok((server_handle, client_handle))
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::fs::File;
  use std::io::Read;
  use std::io::Write;
  use std::os::windows::io::FromRawHandle;

  #[test]
  fn make_named_pipe() {
    let (server, client) = create_named_pipe().unwrap();
    // SAFETY: For testing
    let mut server = unsafe { File::from_raw_handle(server) };
    // SAFETY: For testing
    let mut client = unsafe { File::from_raw_handle(client) };

    // Write to the server and read from the client
    server.write_all(b"hello").unwrap();
    let mut buf: [u8; 5] = Default::default();
    client.read_exact(&mut buf).unwrap();
    assert_eq!(&buf, b"hello");
  }
}
