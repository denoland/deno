// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
use rand::thread_rng;
use rand::RngCore;
use std::io;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::RawHandle;
use std::sync::atomic::AtomicU32;
use std::sync::atomic::Ordering;
use std::time::Duration;
use winapi::um::errhandlingapi::GetLastError;
use winapi::um::fileapi::CreateFileW;
use winapi::um::fileapi::OPEN_EXISTING;
use winapi::um::handleapi::CloseHandle;
use winapi::um::handleapi::INVALID_HANDLE_VALUE;
use winapi::um::namedpipeapi::CreateNamedPipeW;
use winapi::um::winbase::FILE_FLAG_FIRST_PIPE_INSTANCE;
use winapi::um::winbase::FILE_FLAG_OVERLAPPED;
use winapi::um::winbase::PIPE_ACCESS_DUPLEX;
use winapi::um::winbase::PIPE_READMODE_BYTE;
use winapi::um::winbase::PIPE_TYPE_BYTE;
use winapi::um::winnt::GENERIC_READ;
use winapi::um::winnt::GENERIC_WRITE;

/// Create a pair of file descriptors for a named pipe with non-inheritable handles. We cannot use
/// the anonymous pipe from `os_pipe` because that does not support OVERLAPPED (aka async) I/O.
///
/// This is the same way that Rust and pretty much everyone else does it.
///
/// For more information, there is an interesting S.O. question that explains the history, as
/// well as offering a complex NTAPI solution if we decide to try to make these pipes truely
/// anonymous: https://stackoverflow.com/questions/60645/overlapped-i-o-on-anonymous-pipe
pub fn create_named_pipe() -> io::Result<(RawHandle, RawHandle)> {
  // Silently retry up to 10 times.
  for _ in 0..10 {
    if let Ok(res) = create_named_pipe_inner() {
      return Ok(res);
    }
  }
  create_named_pipe_inner()
}

fn create_named_pipe_inner() -> io::Result<(RawHandle, RawHandle)> {
  static NEXT_ID: AtomicU32 = AtomicU32::new(0);
  // Create an extremely-likely-unique pipe name from randomness, identity and a serial counter.
  let pipe_name_utf8 = format!(
    r#"\\.\pipe\deno_pipe_{:x}.{:x}.{:x}"#,
    thread_rng().next_u64(),
    std::process::id(),
    NEXT_ID.fetch_add(1, Ordering::SeqCst),
  );

  let mut pipe_name =
    Vec::from_iter(std::ffi::OsStr::new(&pipe_name_utf8).encode_wide());
  pipe_name.push(0);

  // SAFETY: Create the pipe server with non-inheritable handle
  let server_handle = unsafe {
    CreateNamedPipeW(
      pipe_name.as_ptr(),
      PIPE_ACCESS_DUPLEX | FILE_FLAG_OVERLAPPED | FILE_FLAG_FIRST_PIPE_INSTANCE,
      // Read and write bytes, not messages
      PIPE_TYPE_BYTE | PIPE_READMODE_BYTE,
      // The maximum number of instances that can be created for this pipe.
      1,
      // 4kB buffer sizes
      4096,
      4096,
      // "The default time-out value, in milliseconds, if the WaitNamedPipe function specifies NMPWAIT_USE_DEFAULT_WAIT.
      // Each instance of a named pipe must specify the same value. A value of zero will result in a default time-out of
      // 50 milliseconds."
      0,
      // Uninheritable handle by default.
      std::ptr::null_mut(),
    )
  };

  if server_handle == INVALID_HANDLE_VALUE {
    // This should not happen, so we would like to get some better diagnostics here.
    // SAFETY: Printing last error for diagnostics
    let error = unsafe { GetLastError() };
    eprintln!(
      "*** Unexpected server pipe failure '{pipe_name_utf8:?}': {:x}",
      error
    );
    // There is a very rare case where the pipe cannot be opened. Spin for 1ms before we
    // return.
    if error == winapi::shared::winerror::ERROR_FILE_NOT_FOUND
      || error == winapi::shared::winerror::ERROR_PATH_NOT_FOUND
    {
      std::thread::sleep(Duration::from_millis(1));
    }

    return Err(io::Error::last_os_error());
  }

  // The pipe might not be ready yet in rare cases, so we loop for a bit
  for i in 0..10 {
    // SAFETY: Create the pipe client with non-inheritable handle
    let client_handle = unsafe {
      CreateFileW(
        pipe_name.as_ptr(),
        GENERIC_READ | GENERIC_WRITE,
        0,
        // Uninheritable handle by default.
        std::ptr::null_mut(),
        OPEN_EXISTING,
        FILE_FLAG_OVERLAPPED,
        std::ptr::null_mut(),
      )
    };

    // There is a very rare case where the pipe is not ready to open. If we get `ERROR_PATH_NOT_FOUND`,
    // we spin and try again in 1-10ms.
    if client_handle == INVALID_HANDLE_VALUE {
      // SAFETY: Getting last error for diagnostics
      let error = unsafe { GetLastError() };
      if error == winapi::shared::winerror::ERROR_FILE_NOT_FOUND
        || error == winapi::shared::winerror::ERROR_PATH_NOT_FOUND
      {
        // Exponential backoff, but don't sleep longer than 10ms
        eprintln!(
          "*** Unexpected client pipe not found failure '{pipe_name_utf8:?}': {:x}",
          error
        );
        std::thread::sleep(Duration::from_millis(10.min(2_u64.pow(i) + 1)));
        continue;
      }

      // This should not happen, so we would like to get some better diagnostics here.
      eprintln!(
        "*** Unexpected client pipe failure '{pipe_name_utf8:?}': {:x}",
        error
      );
      let err = io::Error::last_os_error();
      // SAFETY: Close the handles if we failed
      unsafe {
        CloseHandle(server_handle);
      }
      return Err(err);
    }

    return Ok((server_handle, client_handle));
  }

  // We failed to open the pipe despite sleeping
  Err(std::io::ErrorKind::NotFound.into())
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::fs::File;
  use std::io::Read;
  use std::io::Write;
  use std::os::windows::io::FromRawHandle;
  use std::sync::Arc;
  use std::sync::Barrier;

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

  #[test]
  fn make_many_named_pipes_serial() {
    let mut handles = vec![];
    for _ in 0..1000 {
      let (server, client) = create_named_pipe().unwrap();
      // SAFETY: For testing
      let mut server = unsafe { File::from_raw_handle(server) };
      server.write_all(&[0; 1024]).unwrap();
      // SAFETY: For testing
      let mut client = unsafe { File::from_raw_handle(client) };
      client.write_all(&[0; 1024]).unwrap();
      handles.push((server, client))
    }
  }

  #[test]
  fn make_many_named_pipes_parallel() {
    let mut handles = vec![];
    let barrier = Arc::new(Barrier::new(50));
    for _ in 0..50 {
      let barrier = barrier.clone();
      handles.push(std::thread::spawn(move || {
        barrier.wait();
        let mut handles = vec![];
        // Create 1000 pipes in this thread
        for _ in 0..1000 {
          let (server, client) = create_named_pipe().unwrap();
          // SAFETY: For testing
          let mut server = unsafe { File::from_raw_handle(server) };
          server.write_all(&[0; 1024]).unwrap();
          // SAFETY: For testing
          let mut client = unsafe { File::from_raw_handle(client) };
          client.write_all(&[0; 1024]).unwrap();
          handles.push((server, client))
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
        drop(handles);
      }));
    }
    for handle in handles {
      handle.join().unwrap();
    }
  }
}
