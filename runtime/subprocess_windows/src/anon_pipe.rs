// Copyright 2018-2025 the Deno authors. MIT license.

// Copyright (c) The Rust Project Contributors

// Permission is hereby granted, free of charge, to any
// person obtaining a copy of this software and associated
// documentation files (the "Software"), to deal in the
// Software without restriction, including without
// limitation the rights to use, copy, modify, merge,
// publish, distribute, sublicense, and/or sell copies of
// the Software, and to permit persons to whom the Software
// is furnished to do so, subject to the following
// conditions:

// The above copyright notice and this permission notice
// shall be included in all copies or substantial portions
// of the Software.

// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF
// ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED
// TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A
// PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT
// SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY
// CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION
// OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR
// IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
// DEALINGS IN THE SOFTWARE.

// Pulled from https://github.com/rust-lang/rust/blob/3e674b06b5c74adea662bd0b0b06450757994b16/library/std/src/sys/pal/windows/pipe.rs
use std::cmp;
use std::ffi::OsStr;
use std::fs::OpenOptions;
use std::io;
use std::mem;
use std::os::windows::prelude::*;
use std::ptr;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering::Relaxed;

use windows_sys::Win32::Foundation::BOOL;
use windows_sys::Win32::Foundation::ERROR_ACCESS_DENIED;
use windows_sys::Win32::Foundation::ERROR_BROKEN_PIPE;
use windows_sys::Win32::Foundation::ERROR_HANDLE_EOF;
use windows_sys::Win32::Foundation::ERROR_IO_PENDING;
use windows_sys::Win32::Foundation::FALSE;
use windows_sys::Win32::Foundation::GENERIC_READ;
use windows_sys::Win32::Foundation::GENERIC_WRITE;
use windows_sys::Win32::Foundation::GetLastError;
use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
use windows_sys::Win32::Foundation::TRUE;
use windows_sys::Win32::Foundation::WAIT_OBJECT_0;
use windows_sys::Win32::Security::SECURITY_ATTRIBUTES;
use windows_sys::Win32::Storage::FileSystem::CreateFileW;
use windows_sys::Win32::Storage::FileSystem::FILE_FLAG_FIRST_PIPE_INSTANCE;
use windows_sys::Win32::Storage::FileSystem::FILE_FLAG_OVERLAPPED;
use windows_sys::Win32::Storage::FileSystem::OPEN_EXISTING;
use windows_sys::Win32::Storage::FileSystem::PIPE_ACCESS_INBOUND;
use windows_sys::Win32::Storage::FileSystem::PIPE_ACCESS_OUTBOUND;
use windows_sys::Win32::Storage::FileSystem::ReadFile;
use windows_sys::Win32::System::IO::CancelIo;
use windows_sys::Win32::System::IO::GetOverlappedResult;
use windows_sys::Win32::System::IO::OVERLAPPED;
use windows_sys::Win32::System::Pipes::CreateNamedPipeW;
use windows_sys::Win32::System::Pipes::PIPE_READMODE_BYTE;
use windows_sys::Win32::System::Pipes::PIPE_REJECT_REMOTE_CLIENTS;
use windows_sys::Win32::System::Pipes::PIPE_TYPE_BYTE;
use windows_sys::Win32::System::Pipes::PIPE_WAIT;
use windows_sys::Win32::System::Threading::CreateEventW;
use windows_sys::Win32::System::Threading::GetCurrentProcessId;
use windows_sys::Win32::System::Threading::INFINITE;
use windows_sys::Win32::System::Threading::WaitForMultipleObjects;

pub type Handle = std::os::windows::io::OwnedHandle;

////////////////////////////////////////////////////////////////////////////////
// Anonymous pipes
////////////////////////////////////////////////////////////////////////////////

pub struct AnonPipe {
  inner: Handle,
}

impl AnonPipe {
  // fn try_clone(&self) -> io::Result<AnonPipe> {
  //     let handle = handle_dup(&self.inner, 0, false, DUPLICATE_SAME_ACCESS)?;
  //     Ok(AnonPipe { inner: handle })
  // }
}

impl FromRawHandle for AnonPipe {
  unsafe fn from_raw_handle(handle: RawHandle) -> Self {
    AnonPipe {
      inner: unsafe { Handle::from_raw_handle(handle) },
    }
  }
}

fn get_last_error() -> u32 {
  unsafe { GetLastError() }
}

pub struct Pipes {
  pub ours: AnonPipe,
  pub theirs: AnonPipe,
}

fn cvt(res: BOOL) -> io::Result<()> {
  if res == 0 {
    Err(io::Error::last_os_error())
  } else {
    Ok(())
  }
}

/// Although this looks similar to `anon_pipe` in the Unix module it's actually
/// subtly different. Here we'll return two pipes in the `Pipes` return value,
/// but one is intended for "us" where as the other is intended for "someone
/// else".
///
/// Currently the only use case for this function is pipes for stdio on
/// processes in the standard library, so "ours" is the one that'll stay in our
/// process whereas "theirs" will be inherited to a child.
///
/// The ours/theirs pipes are *not* specifically readable or writable. Each
/// one only supports a read or a write, but which is which depends on the
/// boolean flag given. If `ours_readable` is `true`, then `ours` is readable and
/// `theirs` is writable. Conversely, if `ours_readable` is `false`, then `ours`
/// is writable and `theirs` is readable.
///
/// Also note that the `ours` pipe is always a handle opened up in overlapped
/// mode. This means that technically speaking it should only ever be used
/// with `OVERLAPPED` instances, but also works out ok if it's only ever used
/// once at a time (which we do indeed guarantee).
pub fn anon_pipe(
  ours_readable: bool,
  their_handle_inheritable: bool,
) -> io::Result<Pipes> {
  // A 64kb pipe capacity is the same as a typical Linux default.
  const PIPE_BUFFER_CAPACITY: u32 = 64 * 1024;

  // Note that we specifically do *not* use `CreatePipe` here because
  // unfortunately the anonymous pipes returned do not support overlapped
  // operations. Instead, we create a "hopefully unique" name and create a
  // named pipe which has overlapped operations enabled.
  //
  // Once we do this, we connect do it as usual via `CreateFileW`, and then
  // we return those reader/writer halves. Note that the `ours` pipe return
  // value is always the named pipe, whereas `theirs` is just the normal file.
  // This should hopefully shield us from child processes which assume their
  // stdout is a named pipe, which would indeed be odd!
  unsafe {
    let ours;
    let mut name;
    let mut tries = 0;
    loop {
      tries += 1;
      name = format!(
        r"\\.\pipe\__rust_anonymous_pipe1__.{}.{}",
        GetCurrentProcessId(),
        random_number(),
      );
      let wide_name = OsStr::new(&name)
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
      let mut flags = FILE_FLAG_FIRST_PIPE_INSTANCE | FILE_FLAG_OVERLAPPED;
      if ours_readable {
        flags |= PIPE_ACCESS_INBOUND;
      } else {
        flags |= PIPE_ACCESS_OUTBOUND;
      }

      let handle = CreateNamedPipeW(
        wide_name.as_ptr(),
        flags,
        PIPE_TYPE_BYTE
          | PIPE_READMODE_BYTE
          | PIPE_WAIT
          | PIPE_REJECT_REMOTE_CLIENTS,
        1,
        PIPE_BUFFER_CAPACITY,
        PIPE_BUFFER_CAPACITY,
        0,
        ptr::null_mut(),
      );

      // We pass the `FILE_FLAG_FIRST_PIPE_INSTANCE` flag above, and we're
      // also just doing a best effort at selecting a unique name. If
      // `ERROR_ACCESS_DENIED` is returned then it could mean that we
      // accidentally conflicted with an already existing pipe, so we try
      // again.
      //
      // Don't try again too much though as this could also perhaps be a
      // legit error.
      if handle == INVALID_HANDLE_VALUE {
        let error = get_last_error();
        if tries < 10 && error == ERROR_ACCESS_DENIED {
          continue;
        } else {
          return Err(io::Error::from_raw_os_error(error as i32));
        }
      }

      ours = Handle::from_raw_handle(handle);
      break;
    }

    // Connect to the named pipe we just created. This handle is going to be
    // returned in `theirs`, so if `ours` is readable we want this to be
    // writable, otherwise if `ours` is writable we want this to be
    // readable.
    //
    // Additionally we don't enable overlapped mode on this because most
    // client processes aren't enabled to work with that.
    #[allow(clippy::disallowed_methods)]
    let mut opts = OpenOptions::new();
    opts.write(ours_readable);
    opts.read(!ours_readable);
    opts.share_mode(0);
    let access = if ours_readable {
      GENERIC_WRITE
    } else {
      GENERIC_READ
    };
    let size = size_of::<SECURITY_ATTRIBUTES>();
    let sa = SECURITY_ATTRIBUTES {
      nLength: size as u32,
      lpSecurityDescriptor: ptr::null_mut(),
      bInheritHandle: their_handle_inheritable as i32,
    };
    let path_utf16 = OsStr::new(&name)
      .encode_wide()
      .chain(Some(0))
      .collect::<Vec<_>>();
    let handle2 = CreateFileW(
      path_utf16.as_ptr(),
      access,
      0,
      &sa,
      OPEN_EXISTING,
      0,
      ptr::null_mut(),
    );
    let theirs = Handle::from_raw_handle(handle2);

    Ok(Pipes {
      ours: AnonPipe { inner: ours },
      theirs: AnonPipe { inner: theirs },
    })
  }
}

fn random_number() -> usize {
  static N: std::sync::atomic::AtomicUsize = AtomicUsize::new(0);
  loop {
    if N.load(Relaxed) != 0 {
      return N.fetch_add(1, Relaxed);
    }

    N.store(fastrand::usize(..), Relaxed);
  }
}

impl AnonPipe {
  // pub fn handle(&self) -> &Handle {
  //     &self.inner
  // }
  pub fn into_handle(self) -> Handle {
    self.inner
  }
}

pub fn read2(
  p1: AnonPipe,
  v1: &mut Vec<u8>,
  p2: AnonPipe,
  v2: &mut Vec<u8>,
) -> io::Result<()> {
  let p1 = p1.into_handle();
  let p2 = p2.into_handle();

  let mut p1 = AsyncPipe::new(p1, v1)?;
  let mut p2 = AsyncPipe::new(p2, v2)?;
  let objs = [p1.event.as_raw_handle(), p2.event.as_raw_handle()];

  // In a loop we wait for either pipe's scheduled read operation to complete.
  // If the operation completes with 0 bytes, that means EOF was reached, in
  // which case we just finish out the other pipe entirely.
  //
  // Note that overlapped I/O is in general super unsafe because we have to
  // be careful to ensure that all pointers in play are valid for the entire
  // duration of the I/O operation (where tons of operations can also fail).
  // The destructor for `AsyncPipe` ends up taking care of most of this.
  loop {
    let res =
      unsafe { WaitForMultipleObjects(2, objs.as_ptr(), FALSE, INFINITE) };
    if res == WAIT_OBJECT_0 {
      if !p1.result()? || !p1.schedule_read()? {
        return p2.finish();
      }
    } else if res == WAIT_OBJECT_0 + 1 {
      if !p2.result()? || !p2.schedule_read()? {
        return p1.finish();
      }
    } else {
      return Err(io::Error::last_os_error());
    }
  }
}

struct AsyncPipe<'a> {
  pipe: Handle,
  event: Handle,
  overlapped: Box<OVERLAPPED>, // needs a stable address
  dst: &'a mut Vec<u8>,
  state: State,
}

#[derive(PartialEq, Debug)]
enum State {
  NotReading,
  Reading,
  Read(usize),
}

impl<'a> AsyncPipe<'a> {
  fn new(pipe: Handle, dst: &'a mut Vec<u8>) -> io::Result<AsyncPipe<'a>> {
    // Create an event which we'll use to coordinate our overlapped
    // operations, this event will be used in WaitForMultipleObjects
    // and passed as part of the OVERLAPPED handle.
    //
    // Note that we do a somewhat clever thing here by flagging the
    // event as being manually reset and setting it initially to the
    // signaled state. This means that we'll naturally fall through the
    // WaitForMultipleObjects call above for pipes created initially,
    // and the only time an even will go back to "unset" will be once an
    // I/O operation is successfully scheduled (what we want).
    let event = new_event(true, true)?;
    let mut overlapped: Box<OVERLAPPED> = unsafe { Box::new(mem::zeroed()) };
    overlapped.hEvent = event.as_raw_handle();
    Ok(AsyncPipe {
      pipe,
      overlapped,
      event,
      dst,
      state: State::NotReading,
    })
  }

  /// Executes an overlapped read operation.
  ///
  /// Must not currently be reading, and returns whether the pipe is currently
  /// at EOF or not. If the pipe is not at EOF then `result()` must be called
  /// to complete the read later on (may block), but if the pipe is at EOF
  /// then `result()` should not be called as it will just block forever.
  fn schedule_read(&mut self) -> io::Result<bool> {
    assert_eq!(self.state, State::NotReading);
    let amt = unsafe {
      if self.dst.capacity() == self.dst.len() {
        let additional = if self.dst.capacity() == 0 { 16 } else { 1 };
        self.dst.reserve(additional);
      }

      read_overlapped(
        &self.pipe,
        self.dst.spare_capacity_mut(),
        &mut *self.overlapped,
      )?
    };

    // If this read finished immediately then our overlapped event will
    // remain signaled (it was signaled coming in here) and we'll progress
    // down to the method below.
    //
    // Otherwise the I/O operation is scheduled and the system set our event
    // to not signaled, so we flag ourselves into the reading state and move
    // on.
    self.state = match amt {
      Some(0) => return Ok(false),
      Some(amt) => State::Read(amt),
      None => State::Reading,
    };
    Ok(true)
  }

  /// Wait for the result of the overlapped operation previously executed.
  ///
  /// Takes a parameter `wait` which indicates if this pipe is currently being
  /// read whether the function should block waiting for the read to complete.
  ///
  /// Returns values:
  ///
  /// * `true` - finished any pending read and the pipe is not at EOF (keep
  ///   going)
  /// * `false` - finished any pending read and pipe is at EOF (stop issuing
  ///   reads)
  fn result(&mut self) -> io::Result<bool> {
    let amt = match self.state {
      State::NotReading => return Ok(true),
      State::Reading => {
        overlapped_result(&self.pipe, &mut *self.overlapped, true)?
      }
      State::Read(amt) => amt,
    };
    self.state = State::NotReading;
    unsafe {
      let len = self.dst.len();
      self.dst.set_len(len + amt);
    }
    Ok(amt != 0)
  }

  /// Finishes out reading this pipe entirely.
  ///
  /// Waits for any pending and schedule read, and then calls `read_to_end`
  /// if necessary to read all the remaining information.
  fn finish(&mut self) -> io::Result<()> {
    while self.result()? && self.schedule_read()? {
      // ...
    }
    Ok(())
  }
}

impl Drop for AsyncPipe<'_> {
  fn drop(&mut self) {
    match self.state {
      State::Reading => {}
      _ => return,
    }

    // If we have a pending read operation, then we have to make sure that
    // it's *done* before we actually drop this type. The kernel requires
    // that the `OVERLAPPED` and buffer pointers are valid for the entire
    // I/O operation.
    //
    // To do that, we call `CancelIo` to cancel any pending operation, and
    // if that succeeds we wait for the overlapped result.
    //
    // If anything here fails, there's not really much we can do, so we leak
    // the buffer/OVERLAPPED pointers to ensure we're at least memory safe.
    if cancel_io(&self.pipe).is_err() || self.result().is_err() {
      let buf = mem::take(self.dst);
      let overlapped = Box::new(unsafe { mem::zeroed() });
      let overlapped = mem::replace(&mut self.overlapped, overlapped);
      mem::forget((buf, overlapped));
    }
  }
}

pub fn cancel_io(handle: &Handle) -> io::Result<()> {
  unsafe { cvt(CancelIo(handle.as_raw_handle())) }
}

pub fn overlapped_result(
  handle: &Handle,
  overlapped: *mut OVERLAPPED,
  wait: bool,
) -> io::Result<usize> {
  unsafe {
    let mut bytes = 0;
    let wait = if wait { TRUE } else { FALSE };
    let res = cvt(GetOverlappedResult(
      handle.as_raw_handle(),
      overlapped,
      &mut bytes,
      wait,
    ));
    match res {
      Ok(_) => Ok(bytes as usize),
      Err(e) => {
        if e.raw_os_error() == Some(ERROR_HANDLE_EOF as i32)
          || e.raw_os_error() == Some(ERROR_BROKEN_PIPE as i32)
        {
          Ok(0)
        } else {
          Err(e)
        }
      }
    }
  }
}

pub unsafe fn read_overlapped(
  handle: &Handle,
  buf: &mut [mem::MaybeUninit<u8>],
  overlapped: *mut OVERLAPPED,
) -> io::Result<Option<usize>> {
  // SAFETY: We have exclusive access to the buffer and it's up to the caller to
  // ensure the OVERLAPPED pointer is valid for the lifetime of this function.
  let (res, amt) = unsafe {
    let len = cmp::min(buf.len(), u32::MAX as usize) as u32;
    let mut amt = 0;
    let res = cvt(ReadFile(
      handle.as_raw_handle(),
      buf.as_mut_ptr().cast::<u8>(),
      len,
      &mut amt,
      overlapped,
    ));
    (res, amt)
  };
  match res {
    Ok(_) => Ok(Some(amt as usize)),
    Err(e) => {
      if e.raw_os_error() == Some(ERROR_IO_PENDING as i32) {
        Ok(None)
      } else if e.raw_os_error() == Some(ERROR_BROKEN_PIPE as i32) {
        Ok(Some(0))
      } else {
        Err(e)
      }
    }
  }
}

pub fn new_event(manual: bool, init: bool) -> io::Result<Handle> {
  unsafe {
    let event =
      CreateEventW(ptr::null_mut(), manual as BOOL, init as BOOL, ptr::null());
    if event.is_null() {
      Err(io::Error::last_os_error())
    } else {
      Ok(Handle::from_raw_handle(event))
    }
  }
}
