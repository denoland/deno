// Copyright 2018-2025 the Deno authors. MIT license.

/* Copyright Joyent, Inc. and other Node contributors. All rights reserved.
 *
 * Permission is hereby granted, free of charge, to any person obtaining a copy
 * of this software and associated documentation files (the "Software"), to
 * deal in the Software without restriction, including without limitation the
 * rights to use, copy, modify, merge, publish, distribute, sublicense, and/or
 * sell copies of the Software, and to permit persons to whom the Software is
 * furnished to do so, subject to the following conditions:
 *
 * The above copyright notice and this permission notice shall be included in
 * all copies or substantial portions of the Software.
 *
 * THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
 * IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
 * FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
 * AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
 * LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
 * FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS
 * IN THE SOFTWARE.
 */
// Ported partly from https://github.com/libuv/libuv/blob/b00c5d1a09c094020044e79e19f478a25b8e1431/src/win/process-stdio.c

use std::ffi::c_int;
use std::ptr::null_mut;

use buffer::StdioBuffer;
use windows_sys::Win32::Foundation::CloseHandle;
use windows_sys::Win32::Foundation::DUPLICATE_SAME_ACCESS;
use windows_sys::Win32::Foundation::DuplicateHandle;
use windows_sys::Win32::Foundation::GetLastError;
use windows_sys::Win32::Foundation::HANDLE;
use windows_sys::Win32::Foundation::HANDLE_FLAG_INHERIT;
use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
use windows_sys::Win32::Foundation::SetHandleInformation;
use windows_sys::Win32::Security::SECURITY_ATTRIBUTES;
use windows_sys::Win32::Storage::FileSystem::CreateFileW;
use windows_sys::Win32::Storage::FileSystem::FILE_GENERIC_READ;
use windows_sys::Win32::Storage::FileSystem::FILE_GENERIC_WRITE;
use windows_sys::Win32::Storage::FileSystem::FILE_READ_ATTRIBUTES;
use windows_sys::Win32::Storage::FileSystem::FILE_SHARE_READ;
use windows_sys::Win32::Storage::FileSystem::FILE_SHARE_WRITE;
use windows_sys::Win32::Storage::FileSystem::FILE_TYPE_CHAR;
use windows_sys::Win32::Storage::FileSystem::FILE_TYPE_DISK;
use windows_sys::Win32::Storage::FileSystem::FILE_TYPE_PIPE;
use windows_sys::Win32::Storage::FileSystem::FILE_TYPE_REMOTE;
use windows_sys::Win32::Storage::FileSystem::FILE_TYPE_UNKNOWN;
use windows_sys::Win32::Storage::FileSystem::GetFileType;
use windows_sys::Win32::Storage::FileSystem::OPEN_EXISTING;
use windows_sys::Win32::System::Console::GetStdHandle;
use windows_sys::Win32::System::Console::STD_ERROR_HANDLE;
use windows_sys::Win32::System::Console::STD_HANDLE;
use windows_sys::Win32::System::Console::STD_INPUT_HANDLE;
use windows_sys::Win32::System::Console::STD_OUTPUT_HANDLE;
use windows_sys::Win32::System::Threading::GetCurrentProcess;
use windows_sys::Win32::System::Threading::GetStartupInfoW;
use windows_sys::Win32::System::Threading::STARTUPINFOW;

use crate::process::SpawnOptions;

const FOPEN: u8 = 0x01;
// const FEOFLAG: u8 = 0x02;
// const FCRLF: u8 = 0x04;
const FPIPE: u8 = 0x08;
// const FNOINHERIT: u8 = 0x10;
// const FAPPEND: u8 = 0x20;
const FDEV: u8 = 0x40;
// const FTEXT: u8 = 0x80;

const fn child_stdio_size(count: usize) -> usize {
  size_of::<c_int>() + size_of::<u8>() * count + size_of::<usize>() * count
}

unsafe fn child_stdio_count(buffer: *mut u8) -> usize {
  unsafe { buffer.cast::<std::ffi::c_uint>().read_unaligned() as usize }
  // unsafe { *buffer.cast::<std::ffi::c_uint>() as usize }
}

unsafe fn child_stdio_handle(buffer: *mut u8, fd: i32) -> HANDLE {
  unsafe {
    buffer.add(
      size_of::<c_int>()
        + child_stdio_count(buffer)
        + size_of::<HANDLE>() * (fd as usize),
    )
  }
  .cast()
}

unsafe fn child_stdio_crt_flags(buffer: *mut u8, fd: i32) -> *mut u8 {
  unsafe { buffer.add(size_of::<c_int>() + fd as usize) }.cast()
}

#[allow(dead_code)]
unsafe fn uv_stdio_verify(buffer: *mut u8, size: u16) -> bool {
  if buffer.is_null() {
    return false;
  }

  if (size as usize) < child_stdio_size(0) {
    return false;
  }

  let count = unsafe { child_stdio_count(buffer) };
  if count > 256 {
    return false;
  }

  if (size as usize) < child_stdio_size(count) {
    return false;
  }

  true
}

fn uv_create_nul_handle(access: u32) -> Result<HANDLE, std::io::Error> {
  let sa = SECURITY_ATTRIBUTES {
    nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
    lpSecurityDescriptor: null_mut(),
    bInheritHandle: 1,
  };

  let handle = unsafe {
    CreateFileW(
      windows_sys::w!("NUL"),
      access,
      FILE_SHARE_READ | FILE_SHARE_WRITE,
      &sa,
      OPEN_EXISTING,
      0,
      null_mut(),
    )
  };

  if handle == INVALID_HANDLE_VALUE {
    return Err(std::io::Error::last_os_error());
  }

  Ok(handle)
}

#[allow(dead_code)]
unsafe fn uv_stdio_noinherit(buffer: *mut u8) {
  let count = unsafe { child_stdio_count(buffer) };
  for i in 0..count {
    let handle = unsafe { uv_stdio_handle(buffer, i as i32) };
    if handle != INVALID_HANDLE_VALUE {
      unsafe { SetHandleInformation(handle, HANDLE_FLAG_INHERIT, 0) };
    }
  }
}

pub(crate) unsafe fn uv_stdio_handle(buffer: *mut u8, fd: i32) -> HANDLE {
  let mut handle = INVALID_HANDLE_VALUE;
  unsafe {
    copy_handle(
      child_stdio_handle(buffer, fd)
        .cast::<HANDLE>()
        .read_unaligned(),
      &mut handle,
    )
  };
  handle
}

pub unsafe fn uv_duplicate_handle(
  handle: HANDLE,
) -> Result<HANDLE, std::io::Error> {
  if handle == INVALID_HANDLE_VALUE
    || handle.is_null()
    || handle == ((-2i32) as usize as HANDLE)
  {
    return Err(std::io::Error::new(
      std::io::ErrorKind::InvalidInput,
      "Invalid handle",
    ));
  }

  let mut dup = INVALID_HANDLE_VALUE;
  let current_process = unsafe { GetCurrentProcess() };

  if unsafe {
    DuplicateHandle(
      current_process,
      handle,
      current_process,
      &mut dup,
      0,
      1,
      DUPLICATE_SAME_ACCESS,
    )
  } == 0
  {
    return Err(std::io::Error::last_os_error());
  }

  Ok(dup)
}

pub unsafe fn free_stdio_buffer(buffer: *mut u8) {
  let _ = unsafe { StdioBuffer::from_raw(buffer) };
}

/*INLINE static HANDLE uv__get_osfhandle(int fd)
{
  /* _get_osfhandle() raises an assert in debug builds if the FD is invalid.
   * But it also correctly checks the FD and returns INVALID_HANDLE_VALUE for
   * invalid FDs in release builds (or if you let the assert continue). So this
   * wrapper function disables asserts when calling _get_osfhandle. */

  HANDLE handle;
  UV_BEGIN_DISABLE_CRT_ASSERT();
  handle = (HANDLE) _get_osfhandle(fd);
  UV_END_DISABLE_CRT_ASSERT();
  return handle;
}
 */

unsafe fn uv_get_osfhandle(fd: i32) -> HANDLE {
  unsafe { libc::get_osfhandle(fd) as usize as HANDLE }
}

fn uv_duplicate_fd(fd: i32) -> Result<HANDLE, std::io::Error> {
  let handle = unsafe { uv_get_osfhandle(fd) };
  unsafe { uv_duplicate_handle(handle) }
}

unsafe fn copy_handle(mut handle: HANDLE, dest: *mut HANDLE) {
  let handle = &raw mut handle;
  unsafe {
    std::ptr::copy_nonoverlapping(
      handle.cast::<u8>(),
      dest.cast::<u8>(),
      size_of::<HANDLE>(),
    )
  }
}

#[derive(Debug, Clone, Copy)]
pub enum StdioContainer {
  Ignore,
  InheritFd(i32),
  RawHandle(HANDLE),
}

#[inline(never)]
pub(crate) fn uv_stdio_create(
  options: &SpawnOptions,
) -> Result<StdioBuffer, std::io::Error> {
  let mut count = options.stdio.len();
  if count > 255 {
    return Err(std::io::Error::new(
      std::io::ErrorKind::InvalidInput,
      "Invalid stdio count",
    ));
  } else if count < 3 {
    count = 3;
  }

  let mut buffer = StdioBuffer::new(count);

  for i in 0..count {
    let fdopt = if i < options.stdio.len() {
      options.stdio[i]
    } else {
      StdioContainer::Ignore
    };

    match fdopt {
      StdioContainer::RawHandle(handle) => {
        let dup = unsafe { uv_duplicate_handle(handle)? };
        unsafe { buffer.set_handle(i as i32, dup) };
        let flags = unsafe { handle_file_type_flags(dup)? };
        unsafe { buffer.set_flags(i as i32, flags) };
        unsafe { CloseHandle(handle) };
      }
      StdioContainer::Ignore => unsafe {
        if i <= 2 {
          let access = if i == 0 {
            FILE_GENERIC_READ
          } else {
            FILE_GENERIC_WRITE | FILE_READ_ATTRIBUTES
          };
          let nul = uv_create_nul_handle(access)?;
          buffer.set_handle(i as i32, nul);
          buffer.set_flags(i as i32, FOPEN | FDEV);
        }
      },
      StdioContainer::InheritFd(fd) => {
        let handle = uv_duplicate_fd(fd);
        let handle = match handle {
          Ok(handle) => handle,
          Err(_) if fd <= 2 => {
            unsafe { buffer.set_flags(fd, 0) };
            unsafe { buffer.set_handle(fd, INVALID_HANDLE_VALUE) };
            continue;
          }
          Err(e) => return Err(e),
        };

        let flags = unsafe { handle_file_type_flags(handle)? };
        unsafe { buffer.set_handle(fd, handle) };
        unsafe { buffer.set_flags(fd, flags) };
      }
    }
  }

  Ok(buffer)
}

unsafe fn handle_file_type_flags(handle: HANDLE) -> Result<u8, std::io::Error> {
  Ok(match unsafe { GetFileType(handle) } {
    FILE_TYPE_DISK => FOPEN,
    FILE_TYPE_PIPE => FOPEN | FPIPE,
    FILE_TYPE_CHAR | FILE_TYPE_REMOTE => FOPEN | FDEV,
    FILE_TYPE_UNKNOWN => {
      if unsafe { GetLastError() } != 0 {
        unsafe { CloseHandle(handle) };
        return Err(std::io::Error::other("Unknown file type"));
      }
      FOPEN | FDEV
    }
    other => panic!("Unknown file type: {}", other),
  })
}

pub fn disable_stdio_inheritance() {
  let no_inherit = |h: STD_HANDLE| unsafe {
    let handle = GetStdHandle(h);
    if !handle.is_null() && handle != INVALID_HANDLE_VALUE {
      SetHandleInformation(handle, HANDLE_FLAG_INHERIT, 0);
    }
  };
  no_inherit(STD_INPUT_HANDLE);
  no_inherit(STD_OUTPUT_HANDLE);
  no_inherit(STD_ERROR_HANDLE);

  let mut si = unsafe { std::mem::zeroed::<STARTUPINFOW>() };
  unsafe { GetStartupInfoW(&mut si) };
  if let Some(mut stdio_buffer) = unsafe {
    StdioBuffer::from_raw_borrowed(si.lpReserved2, si.cbReserved2 as usize)
  } {
    stdio_buffer.no_inherit();
  }
}

mod buffer {
  use std::ffi::c_uint;
  use std::mem::ManuallyDrop;

  use super::*;
  pub struct StdioBuffer {
    ptr: *mut u8,
    borrowed: bool,
  }

  impl Drop for StdioBuffer {
    fn drop(&mut self) {
      if self.borrowed {
        return;
      }

      let count = self.get_count();
      for i in 0..count {
        let handle = unsafe { self.get_handle(i as i32) };
        if handle != INVALID_HANDLE_VALUE {
          unsafe { CloseHandle(handle) };
        }
      }

      unsafe {
        std::ptr::drop_in_place(self.ptr);
        std::alloc::dealloc(
          self.ptr as *mut _,
          std::alloc::Layout::array::<u8>(self.get_count()).unwrap(),
        );
      }
    }
  }

  unsafe fn verify_buffer(ptr: *mut u8, size: usize) -> bool {
    if ptr.is_null() {
      return false;
    }

    if size < child_stdio_size(0) {
      return false;
    }

    let count = unsafe { child_stdio_count(ptr) };
    if count > 256 {
      return false;
    }

    if size < child_stdio_size(count) {
      return false;
    }

    true
  }

  impl StdioBuffer {
    /// # Safety
    /// The buffer pointer must be valid and point to memory allocated by
    /// `std::alloc::alloc`.
    pub unsafe fn from_raw(ptr: *mut u8) -> Self {
      Self {
        ptr,
        borrowed: false,
      }
    }

    pub unsafe fn from_raw_borrowed(ptr: *mut u8, size: usize) -> Option<Self> {
      if unsafe { !verify_buffer(ptr, size) } {
        return None;
      }

      Some(Self {
        ptr,
        borrowed: true,
      })
    }

    pub fn into_raw(self) -> *mut u8 {
      ManuallyDrop::new(self).ptr
    }

    fn create_raw(count: usize) -> Self {
      let layout =
        std::alloc::Layout::array::<u8>(child_stdio_size(count)).unwrap();
      let ptr = unsafe { std::alloc::alloc(layout) };

      StdioBuffer {
        ptr,
        borrowed: false,
      }
    }
    pub fn new(count: usize) -> Self {
      let buffer = Self::create_raw(count);

      // SAFETY: Since the buffer is uninitialized, use raw pointers
      // and do not read the data.
      unsafe {
        std::ptr::write(buffer.ptr.cast::<c_uint>(), count as c_uint);
      }

      for i in 0..count {
        // SAFETY: We initialized a big enough buffer for `count`
        // handles, so `i` is within bounds.
        unsafe {
          copy_handle(
            INVALID_HANDLE_VALUE,
            child_stdio_handle(buffer.ptr, i as i32).cast(),
          );
          std::ptr::write(child_stdio_crt_flags(buffer.ptr, i as i32), 0);
        }
      }

      buffer
    }

    pub fn get_count(&self) -> usize {
      unsafe { child_stdio_count(self.ptr) }
    }

    /// # Safety
    ///
    /// This function does not check that the fd is within the bounds
    /// of the buffer.
    pub unsafe fn get_handle(&self, fd: i32) -> HANDLE {
      unsafe { uv_stdio_handle(self.ptr, fd) }
    }

    /// # Safety
    ///
    /// This function does not check that the fd is within the bounds
    /// of the buffer.
    pub unsafe fn set_flags(&mut self, fd: i32, flags: u8) {
      debug_assert!(fd < unsafe { child_stdio_count(self.ptr) } as i32,);
      unsafe {
        *child_stdio_crt_flags(self.ptr, fd) = flags;
      }
    }

    /// # Safety
    ///
    /// This function does not check that the fd is within the bounds
    /// of the buffer.
    pub unsafe fn set_handle(&mut self, fd: i32, handle: HANDLE) {
      unsafe {
        copy_handle(handle, child_stdio_handle(self.ptr, fd).cast());
      }
    }

    pub fn size(&self) -> usize {
      child_stdio_size(self.get_count())
    }

    pub fn no_inherit(&mut self) {
      let count = self.get_count();
      for i in 0..count {
        let handle = unsafe { self.get_handle(i as i32) };
        if handle != INVALID_HANDLE_VALUE {
          unsafe { SetHandleInformation(handle, HANDLE_FLAG_INHERIT, 0) };
        }
      }
    }
  }
}
