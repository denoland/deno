// Copyright 2018-2026 the Deno authors. MIT license.

use std::path::PathBuf;

/// A process that currently holds an open handle to one of the files we tried
/// to remove. Used to give the user an actionable message (for example, that a
/// running Deno language server is keeping cache files locked) when `deno
/// clean` cannot delete part of the cache on Windows.
pub struct LockingProcess {
  pub pid: u32,
  /// Friendly application name as reported by the Restart Manager.
  pub name: String,
  /// Full path to the process image, when it could be resolved.
  pub exe: Option<PathBuf>,
  /// Whether the process image looks like a `deno` executable.
  pub is_deno: bool,
}

/// Returns the processes that currently hold open handles to any of `paths`.
///
/// This is only meaningful on Windows, where an open file cannot be deleted; on
/// other platforms open files can be unlinked, so this always returns an empty
/// list.
#[cfg(not(windows))]
pub fn processes_locking_files(_paths: &[PathBuf]) -> Vec<LockingProcess> {
  Vec::new()
}

/// Returns the processes that currently hold open handles to any of `paths`,
/// using the Windows Restart Manager. Returns an empty list on any failure so
/// callers can degrade gracefully.
#[cfg(windows)]
pub fn processes_locking_files(paths: &[PathBuf]) -> Vec<LockingProcess> {
  use std::os::windows::ffi::OsStrExt;

  use windows_sys::Win32::Foundation::ERROR_MORE_DATA;
  use windows_sys::Win32::Foundation::ERROR_SUCCESS;
  use windows_sys::Win32::System::RestartManager::CCH_RM_SESSION_KEY;
  use windows_sys::Win32::System::RestartManager::RM_PROCESS_INFO;
  use windows_sys::Win32::System::RestartManager::RmEndSession;
  use windows_sys::Win32::System::RestartManager::RmGetList;
  use windows_sys::Win32::System::RestartManager::RmRegisterResources;
  use windows_sys::Win32::System::RestartManager::RmStartSession;

  if paths.is_empty() {
    return Vec::new();
  }

  // SAFETY: Win32 Restart Manager calls. The session is always closed with
  // RmEndSession before returning, and all buffers passed to the API outlive
  // the calls that use them.
  unsafe {
    let mut session: u32 = 0;
    let mut session_key = [0u16; CCH_RM_SESSION_KEY as usize + 1];
    if RmStartSession(&mut session, 0, session_key.as_mut_ptr())
      != ERROR_SUCCESS
    {
      return Vec::new();
    }

    // Wide, null-terminated copies of the file names. These must stay alive for
    // the RmRegisterResources call below, which borrows pointers into them.
    let wide_paths: Vec<Vec<u16>> = paths
      .iter()
      .map(|path| {
        path
          .as_os_str()
          .encode_wide()
          .chain(std::iter::once(0))
          .collect()
      })
      .collect();
    let file_ptrs: Vec<*const u16> =
      wide_paths.iter().map(|w| w.as_ptr()).collect();

    let registered = RmRegisterResources(
      session,
      file_ptrs.len() as u32,
      file_ptrs.as_ptr(),
      0,
      std::ptr::null(),
      0,
      std::ptr::null(),
    );
    if registered != ERROR_SUCCESS {
      RmEndSession(session);
      return Vec::new();
    }

    // First call discovers how many processes hold the resources.
    let mut needed: u32 = 0;
    let mut have: u32 = 0;
    let mut reboot_reasons: u32 = 0;
    let status = RmGetList(
      session,
      &mut needed,
      &mut have,
      std::ptr::null_mut(),
      &mut reboot_reasons,
    );
    if (status != ERROR_SUCCESS && status != ERROR_MORE_DATA) || needed == 0 {
      RmEndSession(session);
      return Vec::new();
    }

    // Second call fills a buffer sized for the reported number of processes.
    let mut infos: Vec<RM_PROCESS_INFO> = Vec::with_capacity(needed as usize);
    have = needed;
    let status = RmGetList(
      session,
      &mut needed,
      &mut have,
      infos.as_mut_ptr(),
      &mut reboot_reasons,
    );

    let mut result = Vec::new();
    if status == ERROR_SUCCESS {
      infos.set_len(have as usize);
      for info in &infos {
        let pid = info.Process.dwProcessId;
        let name = wide_to_string(&info.strAppName);
        let exe = process_image_path(pid);
        let is_deno = exe.as_deref().is_some_and(is_deno_exe);
        result.push(LockingProcess {
          pid,
          name,
          exe,
          is_deno,
        });
      }
    }

    RmEndSession(session);
    result
  }
}

#[cfg(windows)]
fn wide_to_string(buf: &[u16]) -> String {
  let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
  String::from_utf16_lossy(&buf[..len])
}

#[cfg(windows)]
fn is_deno_exe(path: &std::path::Path) -> bool {
  path
    .file_stem()
    .and_then(|stem| stem.to_str())
    .is_some_and(|stem| stem.eq_ignore_ascii_case("deno"))
}

/// Resolves the full image path for a process id, or `None` if it cannot be
/// queried (for example, the process exited or access was denied).
#[cfg(windows)]
fn process_image_path(pid: u32) -> Option<PathBuf> {
  use std::ffi::OsString;
  use std::os::windows::ffi::OsStringExt;

  use windows_sys::Win32::Foundation::CloseHandle;
  use windows_sys::Win32::System::Threading::OpenProcess;
  use windows_sys::Win32::System::Threading::PROCESS_QUERY_LIMITED_INFORMATION;
  use windows_sys::Win32::System::Threading::QueryFullProcessImageNameW;

  // SAFETY: Win32 calls. The opened handle is always closed before returning.
  unsafe {
    let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
    if handle.is_null() {
      return None;
    }
    let mut buf = [0u16; 1024];
    let mut size = buf.len() as u32;
    let ok = QueryFullProcessImageNameW(handle, 0, buf.as_mut_ptr(), &mut size);
    CloseHandle(handle);
    if ok == 0 {
      return None;
    }
    Some(PathBuf::from(OsString::from_wide(&buf[..size as usize])))
  }
}

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
