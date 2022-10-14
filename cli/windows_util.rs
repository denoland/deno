// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

/// Ensures that stdin, stdout, and stderr are open and have valid HANDLEs
/// associated with them. There are many places where a `std::fs::File` is
/// constructed from a stdio handle; if the handle is null this causes a panic.
pub fn ensure_stdio_open() {
  #[cfg(windows)]
  // SAFETY: winapi calls
  unsafe {
    use std::mem::size_of;

    use windows_sys::Win32::Foundation::{
      GetHandleInformation, GetLastError, BOOL, ERROR_INVALID_HANDLE, HANDLE,
      INVALID_HANDLE_VALUE,
    };
    use windows_sys::Win32::Security::SECURITY_ATTRIBUTES;
    use windows_sys::Win32::Storage::FileSystem::{
      CreateFileA, FILE_ATTRIBUTE_NORMAL, FILE_GENERIC_READ,
      FILE_GENERIC_WRITE, FILE_READ_ATTRIBUTES, FILE_SHARE_READ,
      FILE_SHARE_WRITE, OPEN_EXISTING,
    };
    use windows_sys::Win32::System::Console::{
      GetStdHandle, SetStdHandle, STD_ERROR_HANDLE, STD_INPUT_HANDLE,
      STD_OUTPUT_HANDLE,
    };

    const NULL: HANDLE = 0;
    const TRUE: BOOL = 1;
    const FALSE: BOOL = 0;

    for std_handle in [STD_INPUT_HANDLE, STD_OUTPUT_HANDLE, STD_ERROR_HANDLE] {
      // Check whether stdio handle is open.
      let is_valid = match GetStdHandle(std_handle) {
        NULL | INVALID_HANDLE_VALUE => false,
        handle => {
          // The stdio handle is open; check whether its handle is valid.
          let mut flags: u32 = 0;
          match GetHandleInformation(handle, &mut flags) {
            TRUE => true,
            FALSE if GetLastError() == ERROR_INVALID_HANDLE => false,
            FALSE => {
              panic!("GetHandleInformation failed (error {})", GetLastError());
            }
            _ => unreachable!(),
          }
        }
      };

      if !is_valid {
        // Open NUL device.
        let desired_access = match std_handle {
          STD_INPUT_HANDLE => FILE_GENERIC_READ,
          _ => FILE_GENERIC_WRITE | FILE_READ_ATTRIBUTES,
        };
        let security_attributes = SECURITY_ATTRIBUTES {
          nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
          lpSecurityDescriptor: std::ptr::null_mut(),
          bInheritHandle: true.into(),
        };
        let file_handle = CreateFileA(
          b"\\\\?\\NUL\0" as *const _ as *mut _,
          desired_access,
          FILE_SHARE_READ | FILE_SHARE_WRITE,
          &security_attributes as *const _ as *mut _,
          OPEN_EXISTING,
          FILE_ATTRIBUTE_NORMAL,
          NULL,
        );
        match file_handle {
          NULL => unreachable!(),
          INVALID_HANDLE_VALUE => {
            panic!("Could not open NUL device (error {})", GetLastError());
          }
          _ => {}
        }

        // Assign the opened NUL handle to the missing stdio handle.
        let success = SetStdHandle(std_handle, file_handle);
        match success {
          TRUE => {}
          FALSE => panic!("SetStdHandle failed (error {})", GetLastError()),
          _ => unreachable!(),
        }
      }
    }
  }
}
