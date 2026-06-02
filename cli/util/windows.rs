// Copyright 2018-2026 the Deno authors. MIT license.

/// Ensures that stdin, stdout, and stderr are open and have valid HANDLEs
/// associated with them. There are many places where a `std::fs::File` is
/// constructed from a stdio handle; if the handle is null this causes a panic.
pub fn ensure_stdio_open() {
  #[cfg(windows)]
  // SAFETY: Win32 calls
  unsafe {
    use std::mem::size_of;

    use windows_sys::Win32::Foundation::ERROR_INVALID_HANDLE;
    use windows_sys::Win32::Foundation::FALSE;
    use windows_sys::Win32::Foundation::GetHandleInformation;
    use windows_sys::Win32::Foundation::GetLastError;
    use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
    use windows_sys::Win32::Foundation::TRUE;
    use windows_sys::Win32::Security::SECURITY_ATTRIBUTES;
    use windows_sys::Win32::Storage::FileSystem::CreateFileA;
    use windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_NORMAL;
    use windows_sys::Win32::Storage::FileSystem::FILE_GENERIC_READ;
    use windows_sys::Win32::Storage::FileSystem::FILE_GENERIC_WRITE;
    use windows_sys::Win32::Storage::FileSystem::FILE_READ_ATTRIBUTES;
    use windows_sys::Win32::Storage::FileSystem::FILE_SHARE_READ;
    use windows_sys::Win32::Storage::FileSystem::FILE_SHARE_WRITE;
    use windows_sys::Win32::Storage::FileSystem::OPEN_EXISTING;
    use windows_sys::Win32::System::Console::GetStdHandle;
    use windows_sys::Win32::System::Console::STD_ERROR_HANDLE;
    use windows_sys::Win32::System::Console::STD_INPUT_HANDLE;
    use windows_sys::Win32::System::Console::STD_OUTPUT_HANDLE;
    use windows_sys::Win32::System::Console::SetStdHandle;

    for std_handle in [STD_INPUT_HANDLE, STD_OUTPUT_HANDLE, STD_ERROR_HANDLE] {
      // Check whether stdio handle is open.
      let handle = GetStdHandle(std_handle);
      let is_valid = if handle.is_null() || handle == INVALID_HANDLE_VALUE {
        false
      } else {
        // The stdio handle is open; check whether its handle is valid.
        let mut flags: u32 = 0;
        match GetHandleInformation(handle, &mut flags) {
          FALSE if GetLastError() == ERROR_INVALID_HANDLE => false,
          FALSE => {
            panic!("GetHandleInformation failed (error {})", GetLastError());
          }
          _ => true,
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
          bInheritHandle: TRUE,
        };
        let file_handle = CreateFileA(
          c"\\\\?\\NUL".as_ptr() as *const u8,
          desired_access,
          FILE_SHARE_READ | FILE_SHARE_WRITE,
          &security_attributes,
          OPEN_EXISTING,
          FILE_ATTRIBUTE_NORMAL,
          std::ptr::null_mut(),
        );
        if file_handle == INVALID_HANDLE_VALUE {
          panic!("Could not open NUL device (error {})", GetLastError());
        }

        // Assign the opened NUL handle to the missing stdio handle.
        if SetStdHandle(std_handle, file_handle) == FALSE {
          panic!("SetStdHandle failed (error {})", GetLastError());
        }
      }
    }
  }
}
