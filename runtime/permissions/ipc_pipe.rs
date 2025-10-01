// Copyright 2018-2025 the Deno authors. MIT license.

use std::ffi::OsStr;
use std::io::Read;
use std::io::Write;
use std::io::{self};

pub struct IpcPipe(Inner);

#[cfg(unix)]
type Inner = std::os::unix::net::UnixStream;

#[cfg(not(unix))]
type Inner = std::fs::File;

impl IpcPipe {
  /// Connect to a local IPC endpoint.
  /// - Unix: `addr` like `/tmp/deno.sock`
  /// - Windows: `addr` like `\\.\pipe\deno-permission-broker`
  pub fn connect(addr: impl AsRef<OsStr>) -> io::Result<Self> {
    Self::connect_impl(addr.as_ref())
  }
}

#[cfg(unix)]
impl IpcPipe {
  fn connect_impl(addr: &OsStr) -> io::Result<Self> {
    use std::os::unix::net::UnixStream;
    use std::path::Path;
    let s = UnixStream::connect(Path::new(addr))?;
    s.set_nonblocking(false)?;
    Ok(Self(s))
  }
}

#[cfg(windows)]
impl IpcPipe {
  fn connect_impl(addr: &OsStr) -> io::Result<Self> {
    use std::os::windows::ffi::OsStrExt;
    use std::os::windows::io::FromRawHandle;

    use windows_sys::Win32::Foundation::ERROR_PIPE_BUSY;
    use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
    use windows_sys::Win32::Storage::FileSystem::CreateFileW;
    use windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_NORMAL;
    use windows_sys::Win32::Storage::FileSystem::FILE_GENERIC_READ;
    use windows_sys::Win32::Storage::FileSystem::FILE_GENERIC_WRITE;
    use windows_sys::Win32::Storage::FileSystem::OPEN_EXISTING;
    use windows_sys::Win32::System::Pipes::NMPWAIT_WAIT_FOREVER;
    use windows_sys::Win32::System::Pipes::PIPE_READMODE_BYTE;
    use windows_sys::Win32::System::Pipes::SetNamedPipeHandleState;
    use windows_sys::Win32::System::Pipes::WaitNamedPipeW;

    // OsStr -> UTF-16 + NUL
    let mut wide: Vec<u16> = addr.encode_wide().collect();
    wide.push(0);

    // Try to open; if the pipe is busy, wait and retry.
    let handle = loop {
      // SAFETY: WinAPI call
      let h = unsafe {
        CreateFileW(
          wide.as_ptr(),
          FILE_GENERIC_READ | FILE_GENERIC_WRITE,
          0, // no sharing
          std::ptr::null(),
          OPEN_EXISTING,
          FILE_ATTRIBUTE_NORMAL, // blocking
          std::ptr::null_mut(),
        )
      };
      if h != INVALID_HANDLE_VALUE {
        break h;
      }
      let err = io::Error::last_os_error();
      if err.raw_os_error() == Some(ERROR_PIPE_BUSY as i32) {
        // SAFETY: WinAPI call
        unsafe { WaitNamedPipeW(wide.as_ptr(), NMPWAIT_WAIT_FOREVER) };
        continue;
      } else {
        return Err(err);
      }
    };

    // Ensure byte mode to mirror Unix stream semantics.
    // SAFETY: WinAPI call
    unsafe {
      let _ = SetNamedPipeHandleState(
        handle,
        &PIPE_READMODE_BYTE,
        std::ptr::null_mut(),
        std::ptr::null_mut(),
      );
    }

    // SAFETY: Passing WinAPI handle
    let file = unsafe { std::fs::File::from_raw_handle(handle as _) };
    Ok(Self(file))
  }
}

#[cfg(all(not(unix), not(windows)))]
impl IpcPipe {
  fn connect_impl(_addr: &OsStr) -> io::Result<Self> {
    Err(io::Error::new(
      io::ErrorKind::Unsupported,
      "Platform not supported.",
    ))
  }
}

impl Read for IpcPipe {
  fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
    self.0.read(buf)
  }
}

impl Write for IpcPipe {
  fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
    self.0.write(buf)
  }

  fn flush(&mut self) -> io::Result<()> {
    self.0.flush()
  }
}
