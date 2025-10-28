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
// Ported partly from https://github.com/libuv/libuv/blob/b00c5d1a09c094020044e79e19f478a25b8e1431/src/win/process.c

#![allow(nonstandard_style)]
use std::borrow::Cow;
use std::ffi::CStr;
use std::ffi::OsStr;
use std::future::Future;
use std::io;
use std::mem;
use std::ops::BitAnd;
use std::ops::BitAndAssign;
use std::ops::BitOr;
use std::ops::BitOrAssign;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::AsRawHandle;
use std::os::windows::io::FromRawHandle;
use std::os::windows::io::OwnedHandle;
use std::pin::Pin;
use std::ptr::null_mut;
use std::ptr::{self};
use std::sync::OnceLock;
use std::task::Poll;

use futures_channel::oneshot;
use windows_sys::Win32::Foundation::BOOL;
use windows_sys::Win32::Foundation::BOOLEAN;
use windows_sys::Win32::Foundation::CloseHandle;
use windows_sys::Win32::Foundation::ERROR_ACCESS_DENIED;
use windows_sys::Win32::Foundation::ERROR_INVALID_PARAMETER;
use windows_sys::Win32::Foundation::ERROR_SUCCESS;
use windows_sys::Win32::Foundation::FALSE;
use windows_sys::Win32::Foundation::GENERIC_WRITE;
use windows_sys::Win32::Foundation::GetLastError;
use windows_sys::Win32::Foundation::HANDLE;
use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
use windows_sys::Win32::Foundation::LocalFree;
use windows_sys::Win32::Foundation::STILL_ACTIVE;
use windows_sys::Win32::Foundation::TRUE;
use windows_sys::Win32::Foundation::WAIT_FAILED;
use windows_sys::Win32::Foundation::WAIT_OBJECT_0;
use windows_sys::Win32::Foundation::WAIT_TIMEOUT;
use windows_sys::Win32::Globalization::GetSystemDefaultLangID;
use windows_sys::Win32::Security::SECURITY_ATTRIBUTES;
use windows_sys::Win32::Storage::FileSystem::CREATE_NEW;
use windows_sys::Win32::Storage::FileSystem::CreateDirectoryW;
use windows_sys::Win32::Storage::FileSystem::CreateFileW;
use windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_NORMAL;
use windows_sys::Win32::Storage::FileSystem::FILE_DISPOSITION_INFO;
use windows_sys::Win32::Storage::FileSystem::FileDispositionInfo;
use windows_sys::Win32::Storage::FileSystem::GetShortPathNameW;
use windows_sys::Win32::Storage::FileSystem::SYNCHRONIZE;
use windows_sys::Win32::Storage::FileSystem::SetFileInformationByHandle;
use windows_sys::Win32::System::Com::CoTaskMemFree;
use windows_sys::Win32::System::Diagnostics::Debug::FORMAT_MESSAGE_ALLOCATE_BUFFER;
use windows_sys::Win32::System::Diagnostics::Debug::FORMAT_MESSAGE_FROM_SYSTEM;
use windows_sys::Win32::System::Diagnostics::Debug::FORMAT_MESSAGE_IGNORE_INSERTS;
use windows_sys::Win32::System::Diagnostics::Debug::FormatMessageA;
use windows_sys::Win32::System::Diagnostics::Debug::MINIDUMP_TYPE;
use windows_sys::Win32::System::Diagnostics::Debug::MiniDumpIgnoreInaccessibleMemory;
use windows_sys::Win32::System::Diagnostics::Debug::MiniDumpWithFullMemory;
use windows_sys::Win32::System::Diagnostics::Debug::MiniDumpWriteDump;
use windows_sys::Win32::System::Diagnostics::Debug::SymGetOptions;
use windows_sys::Win32::System::Diagnostics::Debug::SymSetOptions;
use windows_sys::Win32::System::Environment::NeedCurrentDirectoryForExePathW;
use windows_sys::Win32::System::ProcessStatus::GetModuleBaseNameW;
use windows_sys::Win32::System::Registry::HKEY;
use windows_sys::Win32::System::Registry::HKEY_LOCAL_MACHINE;
use windows_sys::Win32::System::Registry::KEY_QUERY_VALUE;
use windows_sys::Win32::System::Registry::RRF_RT_ANY;
use windows_sys::Win32::System::Registry::RegCloseKey;
use windows_sys::Win32::System::Registry::RegGetValueW;
use windows_sys::Win32::System::Registry::RegOpenKeyExW;
use windows_sys::Win32::System::SystemInformation::GetSystemDirectoryW;
use windows_sys::Win32::System::Threading::CREATE_NEW_PROCESS_GROUP;
use windows_sys::Win32::System::Threading::CREATE_NO_WINDOW;
use windows_sys::Win32::System::Threading::CREATE_SUSPENDED;
use windows_sys::Win32::System::Threading::CREATE_UNICODE_ENVIRONMENT;
use windows_sys::Win32::System::Threading::CreateProcessW;
use windows_sys::Win32::System::Threading::DETACHED_PROCESS;
use windows_sys::Win32::System::Threading::GetCurrentProcess;
use windows_sys::Win32::System::Threading::GetExitCodeProcess;
use windows_sys::Win32::System::Threading::GetProcessId;
use windows_sys::Win32::System::Threading::INFINITE;
use windows_sys::Win32::System::Threading::OpenProcess;
use windows_sys::Win32::System::Threading::PROCESS_INFORMATION;
use windows_sys::Win32::System::Threading::PROCESS_QUERY_INFORMATION;
use windows_sys::Win32::System::Threading::PROCESS_TERMINATE;
use windows_sys::Win32::System::Threading::RegisterWaitForSingleObject;
use windows_sys::Win32::System::Threading::ResumeThread;
use windows_sys::Win32::System::Threading::STARTF_USESHOWWINDOW;
use windows_sys::Win32::System::Threading::STARTF_USESTDHANDLES;
use windows_sys::Win32::System::Threading::STARTUPINFOW;
use windows_sys::Win32::System::Threading::TerminateProcess;
use windows_sys::Win32::System::Threading::UnregisterWaitEx;
use windows_sys::Win32::System::Threading::WT_EXECUTEINWAITTHREAD;
use windows_sys::Win32::System::Threading::WT_EXECUTEONLYONCE;
use windows_sys::Win32::System::Threading::WaitForSingleObject;
use windows_sys::Win32::UI::Shell::FOLDERID_LocalAppData;
use windows_sys::Win32::UI::Shell::SHGetKnownFolderPath;
use windows_sys::Win32::UI::WindowsAndMessaging::SW_HIDE;
use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWDEFAULT;
use windows_sys::w;

use crate::env::CommandEnv;
use crate::env::EnvKey;
use crate::process_stdio::StdioContainer;
use crate::process_stdio::free_stdio_buffer;
use crate::process_stdio::uv_stdio_create;
use crate::uv_error;
use crate::widestr::WCStr;
use crate::widestr::WCString;

unsafe extern "C" {
  fn wcsncpy(dest: *mut u16, src: *const u16, count: usize) -> *mut u16;
  fn wcspbrk(str: *const u16, accept: *const u16) -> *const u16;
}

unsafe fn wcslen(mut str: *const u16) -> usize {
  let mut len = 0;
  while unsafe { *str != 0 } {
    len += 1;
    str = str.wrapping_add(1);
  }
  len
}

fn get_last_error() -> u32 {
  unsafe { GetLastError() }
}

struct GlobalJobHandle(HANDLE);
unsafe impl Send for GlobalJobHandle {}
unsafe impl Sync for GlobalJobHandle {}

static UV_GLOBAL_JOB_HANDLE: OnceLock<GlobalJobHandle> = OnceLock::new();

fn uv_fatal_error_with_no(syscall: &str, errno: Option<u32>) {
  let errno = errno.unwrap_or_else(|| unsafe { GetLastError() });
  let mut buf: *mut i8 = null_mut();
  unsafe {
    FormatMessageA(
      FORMAT_MESSAGE_ALLOCATE_BUFFER
        | FORMAT_MESSAGE_FROM_SYSTEM
        | FORMAT_MESSAGE_IGNORE_INSERTS,
      null_mut(),
      errno,
      GetSystemDefaultLangID().into(),
      (&raw mut buf).cast(),
      0,
      null_mut(),
    );
  }
  let errmsg = if buf.is_null() {
    "Unknown error"
  } else {
    unsafe { CStr::from_ptr(buf).to_str().unwrap() }
  };

  let msg = if syscall.is_empty() {
    format!("({}) {}", errno, errmsg)
  } else {
    format!("{}: ({}) {}", syscall, errno, errmsg)
  };
  if !buf.is_null() {
    unsafe { LocalFree(buf.cast()) };
  }
  panic!("{}", msg);
}

fn uv_fatal_error(syscall: &str) {
  uv_fatal_error_with_no(syscall, None)
}

fn uv_init_global_job_handle() {
  use windows_sys::Win32::System::JobObjects::*;
  UV_GLOBAL_JOB_HANDLE.get_or_init(|| {
    unsafe {
      // SAFETY: SECURITY_ATTRIBUTES is a POD type, repr(C)
      let mut attr = mem::zeroed::<SECURITY_ATTRIBUTES>();
      // SAFETY: JOBOBJECT_EXTENDED_LIMIT_INFORMATION is a POD type, repr(C)
      let mut info = mem::zeroed::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>();
      attr.bInheritHandle = FALSE;

      info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_BREAKAWAY_OK
        | JOB_OBJECT_LIMIT_SILENT_BREAKAWAY_OK
        | JOB_OBJECT_LIMIT_DIE_ON_UNHANDLED_EXCEPTION
        | JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;

      // SAFETY: called with valid parameters
      let job = CreateJobObjectW(&attr, ptr::null());
      if job.is_null() {
        uv_fatal_error("CreateJobObjectW");
      }

      if SetInformationJobObject(
        job,
        JobObjectExtendedLimitInformation,
        &raw const info as _,
        mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
      ) == 0
      {
        uv_fatal_error("SetInformationJobObject");
      }

      if AssignProcessToJobObject(job, GetCurrentProcess()) == 0 {
        let err = GetLastError();
        if err != ERROR_ACCESS_DENIED {
          uv_fatal_error_with_no("AssignProcessToJobObject", Some(err));
        }
      }

      GlobalJobHandle(job)
    }
  });
}

#[derive(Debug)]
struct Waiting {
  rx: oneshot::Receiver<()>,
  wait_object: HANDLE,
  tx: *mut Option<oneshot::Sender<()>>,
}

impl Drop for Waiting {
  fn drop(&mut self) {
    unsafe {
      let rc = UnregisterWaitEx(self.wait_object, INVALID_HANDLE_VALUE);
      if rc == 0 {
        panic!("failed to unregister: {}", io::Error::last_os_error());
      }
      drop(Box::from_raw(self.tx));
    }
  }
}

unsafe impl Sync for Waiting {}
unsafe impl Send for Waiting {}

pub struct SpawnOptions<'a> {
  // pub exit_cb: Option<fn(*const uv_process, u64, i32)>,
  pub flags: u32,
  pub file: Cow<'a, OsStr>,
  pub args: Vec<Cow<'a, OsStr>>,
  pub env: &'a CommandEnv,
  pub cwd: Option<Cow<'a, OsStr>>,
  pub stdio: Vec<super::process_stdio::StdioContainer>,
}

macro_rules! wchar {
  ($s: literal) => {{
    const INPUT: char = $s;
    const OUTPUT: u16 = {
      let len = INPUT.len_utf16();
      if len != 1 {
        panic!("wchar! macro requires a single UTF-16 character");
      }
      let mut buf = [0; 1];
      INPUT.encode_utf16(&mut buf);
      buf[0]
    };
    OUTPUT
  }};
}

fn quote_cmd_arg(src: &WCStr, target: &mut Vec<u16>) {
  let len = src.len();

  if len == 0 {
    // Need double quotation for empty argument
    target.push(wchar!('"'));
    target.push(wchar!('"'));
    return;
  }

  debug_assert!(src.has_nul());

  if unsafe { wcspbrk(src.as_ptr(), w!(" \t\"")) }.is_null() {
    // No quotation needed
    target.extend(src.wchars_no_null());
    return;
  }

  if unsafe { wcspbrk(src.as_ptr(), w!("\"\\")) }.is_null() {
    // No embedded double quotes or backlashes, so I can just wrap
    // quote marks around the whole thing.
    target.push(wchar!('"'));
    target.extend(src.wchars_no_null());
    target.push(wchar!('"'));
    return;
  }

  // Expected input/output:
  //   input : hello"world
  //   output: "hello\"world"
  //   input : hello""world
  //   output: "hello\"\"world"
  //   input : hello\world
  //   output: hello\world
  //   input : hello\\world
  //   output: hello\\world
  //   input : hello\"world
  //   output: "hello\\\"world"
  //   input : hello\\"world
  //   output: "hello\\\\\"world"
  //   input : hello world\
  //   output: "hello world\\"

  target.push(wchar!('"'));
  let start = target.len();
  let mut quote_hit = true;

  for i in (0..len).rev() {
    target.push(src[i]);

    if quote_hit && src[i] == wchar!('\\') {
      target.push(wchar!('\\'));
    } else if src[i] == wchar!('"') {
      quote_hit = true;
      target.push(wchar!('\\'));
    } else {
      quote_hit = false;
    }
  }

  target[start..].reverse();
  target.push(wchar!('"'));
}

fn make_program_args(
  args: &[&OsStr],
  verbatim_arguments: bool,
) -> Result<WCString, std::io::Error> {
  let mut dst_len = 0;
  let mut temp_buffer_len = 0;

  // Count the required size.
  for arg in args {
    let arg_len = arg.encode_wide().count();
    dst_len += arg_len;
    if arg_len > temp_buffer_len {
      temp_buffer_len = arg_len;
    }
  }

  // Adjust for potential quotes. Also assume the worst-case scenario that
  // every character needs escaping, so we need twice as much space.
  dst_len = dst_len * 2 + args.len() * 2;

  let mut dst = Vec::with_capacity(dst_len);
  let mut temp_buffer = Vec::with_capacity(temp_buffer_len);

  for (i, arg) in args.iter().enumerate() {
    temp_buffer.clear();
    temp_buffer.extend(arg.encode_wide());

    if verbatim_arguments {
      dst.extend(temp_buffer.as_slice());
    } else {
      temp_buffer.push(0);
      quote_cmd_arg(WCStr::from_wchars(&temp_buffer), &mut dst);
    }

    if i < args.len() - 1 {
      dst.push(wchar!(' '));
    }
  }

  let wcstring = WCString::from_vec(dst);
  Ok(wcstring)
}

fn cvt(result: BOOL) -> Result<(), std::io::Error> {
  if result == 0 {
    Err(std::io::Error::last_os_error())
  } else {
    Ok(())
  }
}

#[derive(Debug)]
pub struct ChildProcess {
  pid: i32,
  handle: OwnedHandle,
  waiting: Option<Waiting>,
}

impl crate::Kill for ChildProcess {
  fn kill(&mut self) -> std::io::Result<()> {
    process_kill(self.pid, SIGTERM).map_err(|e| {
      if let Some(sys_error) = e.as_sys_error() {
        std::io::Error::from_raw_os_error(sys_error as i32)
      } else if e.as_uv_error() == uv_error::UV_ESRCH {
        std::io::Error::new(
          std::io::ErrorKind::InvalidInput,
          "Process not found",
        )
      } else {
        std::io::Error::other(format!(
          "Failed to kill process: {}",
          e.as_uv_error()
        ))
      }
    })
  }
}

impl ChildProcess {
  pub fn pid(&self) -> i32 {
    self.pid
  }
  pub fn try_wait(&mut self) -> Result<Option<i32>, std::io::Error> {
    unsafe {
      match WaitForSingleObject(self.handle.as_raw_handle(), 0) {
        WAIT_OBJECT_0 => {}
        WAIT_TIMEOUT => return Ok(None),
        // TODO: io error probably
        _ => {
          return Err(std::io::Error::last_os_error());
        }
      }

      let mut status = 0;
      cvt(GetExitCodeProcess(self.handle.as_raw_handle(), &mut status))?;
      Ok(Some(status as i32))
    }
  }

  pub fn wait(&mut self) -> Result<i32, std::io::Error> {
    unsafe {
      let res = WaitForSingleObject(self.handle.as_raw_handle(), INFINITE);
      if res != WAIT_OBJECT_0 {
        return Err(std::io::Error::last_os_error());
      }

      let mut status = 0;
      cvt(GetExitCodeProcess(self.handle.as_raw_handle(), &mut status))?;
      Ok(status as i32)
    }
  }
}

unsafe extern "system" fn callback(
  ptr: *mut std::ffi::c_void,
  _timer_fired: BOOLEAN,
) {
  let complete = unsafe { &mut *(ptr as *mut Option<oneshot::Sender<()>>) };
  let _ = complete.take().unwrap().send(());
}

impl Future for ChildProcess {
  type Output = Result<i32, std::io::Error>;

  fn poll(
    self: std::pin::Pin<&mut Self>,
    cx: &mut std::task::Context<'_>,
  ) -> std::task::Poll<Self::Output> {
    let inner = Pin::get_mut(self);
    loop {
      if let Some(ref mut w) = inner.waiting {
        match Pin::new(&mut w.rx).poll(cx) {
          Poll::Ready(Ok(())) => {}
          Poll::Ready(Err(_)) => panic!("should not be canceled"),
          Poll::Pending => return Poll::Pending,
        }
        let status = inner.try_wait()?.expect("not ready yet");
        return Poll::Ready(Ok(status));
      }

      if let Some(e) = inner.try_wait()? {
        return Poll::Ready(Ok(e));
      }
      let (tx, rx) = oneshot::channel();
      let ptr = Box::into_raw(Box::new(Some(tx)));
      let mut wait_object = null_mut();
      let rc = unsafe {
        RegisterWaitForSingleObject(
          &mut wait_object,
          inner.handle.as_raw_handle() as _,
          Some(callback),
          ptr as *mut _,
          INFINITE,
          WT_EXECUTEINWAITTHREAD | WT_EXECUTEONLYONCE,
        )
      };
      if rc == 0 {
        let err = io::Error::last_os_error();
        drop(unsafe { Box::from_raw(ptr) });
        return Poll::Ready(Err(err));
      }
      inner.waiting = Some(Waiting {
        rx,
        wait_object,
        tx: ptr,
      });
    }
  }
}

pub fn spawn(options: &SpawnOptions) -> Result<ChildProcess, std::io::Error> {
  let mut startup = unsafe { mem::zeroed::<STARTUPINFOW>() };
  let mut info = unsafe { mem::zeroed::<PROCESS_INFORMATION>() };

  if options.file.is_empty() || options.args.is_empty() {
    return Err(std::io::Error::new(
      std::io::ErrorKind::InvalidInput,
      "Invalid arguments",
    ));
  }

  // Convert file path to UTF-16
  let application = WCString::new(&options.file);

  // Create environment block if provided
  let env_saw_path = options.env.have_changed_path();
  let maybe_env = options.env.capture_if_changed();

  let child_paths = if env_saw_path {
    if let Some(env) = maybe_env.as_ref() {
      env.get(&EnvKey::new("PATH")).map(|s| s.as_os_str())
    } else {
      None
    }
  } else {
    None
  };

  // Handle current working directory
  let cwd = if let Some(cwd_option) = &options.cwd {
    // Explicit cwd
    WCString::new(cwd_option)
  } else {
    // Inherit cwd
    #[allow(clippy::disallowed_methods)]
    let cwd = std::env::current_dir().unwrap();
    WCString::new(cwd)
  };

  // If cwd is too long, shorten it
  let cwd =
    if cwd.len_no_nul() >= windows_sys::Win32::Foundation::MAX_PATH as usize {
      unsafe {
        let cwd_ptr = cwd.as_ptr();
        let mut short_buf = vec![0u16; cwd.len_no_nul()];
        let cwd_len = GetShortPathNameW(
          cwd_ptr,
          short_buf.as_mut_ptr(),
          cwd.len_no_nul() as u32,
        );
        if cwd_len == 0 {
          return Err(std::io::Error::last_os_error());
        }
        WCString::from_vec(short_buf)
      }
    } else {
      cwd
    };

  // Get PATH environment variable
  let path = child_paths
    .map(|p| p.encode_wide().chain(Some(0)).collect::<Vec<_>>())
    .or_else(|| {
      // PATH not found in provided environment, get system PATH
      std::env::var_os("PATH")
        .map(|p| p.encode_wide().chain(Some(0)).collect::<Vec<_>>())
    });

  // Create and set up stdio
  let child_stdio_buffer = uv_stdio_create(options)?;

  // Search for the executable
  let Some(application_path) = search_path(
    application.as_slice_no_nul(),
    cwd.as_slice_no_nul(),
    path.as_deref(),
    options.flags,
  ) else {
    return Err(std::io::Error::new(
      std::io::ErrorKind::NotFound,
      "File not found",
    ));
  };

  // Create command line arguments
  let args: Vec<&OsStr> = options.args.iter().map(|s| s.as_ref()).collect();
  let verbatim_arguments =
    (options.flags & uv_process_flags::WindowsVerbatimArguments) != 0;

  let has_bat_extension = |program: &[u16]| {
    // lifted from https://github.com/rust-lang/rust/blob/bc1d7273dfbc6f8a11c0086fa35f6748a13e8d3c/library/std/src/sys/process/windows.rs#L284
    // Copyright The Rust Project Contributors - MIT
    matches!(
      // Case insensitive "ends_with" of UTF-16 encoded ".bat" or ".cmd"
      program.len().checked_sub(4).and_then(|i| program.get(i..)),
      Some(
        [46, 98 | 66, 97 | 65, 116 | 84] | [46, 99 | 67, 109 | 77, 100 | 68]
      )
    )
  };
  let is_batch_file = has_bat_extension(application_path.as_slice_no_nul());
  let (application_path, arguments) = if is_batch_file {
    (
      command_prompt()?,
      WCString::from_vec(make_bat_command_line(
        application_path.as_slice_no_nul(),
        &args,
        !verbatim_arguments,
      )?),
    )
  } else {
    (
      application_path,
      make_program_args(&args, verbatim_arguments)?,
    )
  };

  // Set up process creation
  startup.cb = std::mem::size_of::<STARTUPINFOW>() as u32;
  startup.lpReserved = ptr::null_mut();
  startup.lpDesktop = ptr::null_mut();
  startup.lpTitle = ptr::null_mut();
  startup.dwFlags = STARTF_USESTDHANDLES | STARTF_USESHOWWINDOW;

  startup.cbReserved2 = child_stdio_buffer.size() as u16;
  startup.hStdInput = unsafe { child_stdio_buffer.get_handle(0) };
  startup.hStdOutput = unsafe { child_stdio_buffer.get_handle(1) };
  startup.hStdError = unsafe { child_stdio_buffer.get_handle(2) };

  startup.lpReserved2 = child_stdio_buffer.into_raw();

  // Set up process flags
  let mut process_flags = CREATE_UNICODE_ENVIRONMENT;

  // Handle console window visibility
  if (options.flags & uv_process_flags::WindowsHideConsole) != 0
    || (options.flags & uv_process_flags::WindowsHide) != 0
  {
    // Avoid creating console window if stdio is not inherited
    let mut can_hide = true;
    for i in 0..options.stdio.len() {
      if matches!(options.stdio[i], StdioContainer::InheritFd(_)) {
        can_hide = false;
        break;
      }
    }
    if can_hide {
      process_flags |= CREATE_NO_WINDOW;
    }
  }

  // Set window show state
  if (options.flags & uv_process_flags::WindowsHideGui) != 0
    || (options.flags & uv_process_flags::WindowsHide) != 0
  {
    startup.wShowWindow = SW_HIDE as u16;
  } else {
    startup.wShowWindow = SW_SHOWDEFAULT as u16;
  }

  // Handle detached processes
  if (options.flags & uv_process_flags::Detached) != 0 {
    process_flags |= DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP;
    process_flags |= CREATE_SUSPENDED;
  }

  // Create the process
  let app_path_ptr = application_path.as_ptr();
  let args_ptr = arguments.as_ptr();
  let (env_ptr, _data) = crate::env::make_envp(maybe_env)?;

  let cwd_ptr = cwd.as_ptr();

  let create_result = unsafe {
    CreateProcessW(
      app_path_ptr,         // Application path
      args_ptr as *mut u16, // Command line
      ptr::null(),          // Process attributes
      ptr::null(),          // Thread attributes
      TRUE,                 // Inherit handles
      process_flags,        // Creation flags
      env_ptr as *mut _,    // Environment
      cwd_ptr,              // Current directory
      &startup,             // Startup info
      &mut info,            // Process information
    )
  };

  if create_result == 0 {
    // CreateProcessW failed
    return Err(std::io::Error::last_os_error());
  }

  // If the process isn't spawned as detached, assign to the global job object
  if (options.flags & uv_process_flags::Detached) == 0 {
    uv_init_global_job_handle();
    let job_handle = UV_GLOBAL_JOB_HANDLE.get().unwrap().0;

    unsafe {
      if windows_sys::Win32::System::JobObjects::AssignProcessToJobObject(
        job_handle,
        info.hProcess,
      ) == 0
      {
        // AssignProcessToJobObject might fail if this process is under job control
        // and the job doesn't have the JOB_OBJECT_LIMIT_SILENT_BREAKAWAY_OK flag set,
        // on a Windows version that doesn't support nested jobs.
        let err = GetLastError();
        if err != ERROR_ACCESS_DENIED {
          uv_fatal_error_with_no("AssignProcessToJobObject", Some(err));
        }
      }
    }
  }

  // Resume thread if it was suspended
  if (process_flags & CREATE_SUSPENDED) != 0 {
    unsafe {
      if ResumeThread(info.hThread) == u32::MAX {
        TerminateProcess(info.hProcess, 1);
        return Err(std::io::Error::last_os_error());
      }
    }
  }

  let child = ChildProcess {
    pid: info.dwProcessId as i32,
    handle: unsafe { OwnedHandle::from_raw_handle(info.hProcess) },
    waiting: None,
  };

  // Close the thread handle as we don't need it
  unsafe { windows_sys::Win32::Foundation::CloseHandle(info.hThread) };

  if !startup.lpReserved2.is_null() {
    unsafe { free_stdio_buffer(startup.lpReserved2) };
  }

  Ok(child)
}

macro_rules! impl_bitops {
    ($t: ty : $other: ty) => {
        impl_bitops!(@help; $t, $other; out = $other);
        impl_bitops!(@help; $other, $t; out = $other);
        impl_bitops!(@help; $t, $t; out = $other);

        impl BitOrAssign<$t> for $other {
            fn bitor_assign(&mut self, rhs: $t) {
                *self |= rhs as $other;
            }
        }
        impl BitAndAssign<$t> for $other {
            fn bitand_assign(&mut self, rhs: $t) {
                *self &= rhs as $other;
            }
        }
    };
    (@help; $lhs: ty , $rhs: ty; out = $out: ty) => {
        impl BitOr<$rhs> for $lhs {
            type Output = $out;
            fn bitor(self, rhs: $rhs) -> Self::Output {
                self as $out | rhs as $out
            }
        }
        impl BitAnd<$rhs> for $lhs {
            type Output = $out;

            fn bitand(self, rhs: $rhs) -> Self::Output {
                self as $out & rhs as $out
            }
        }
    };
}

impl_bitops!(
    uv_process_flags : u32
);

#[repr(u32)]
pub enum uv_process_flags {
  /// Set the child process' user id.
  SetUid = 1 << 0,
  /// Set the child process' group id.
  SetGid = 1 << 1,
  /// Do not wrap any arguments in quotes, or perform any other escaping, when
  /// converting the argument list into a command line string. This option is
  /// only meaningful on Windows systems. On Unix it is silently ignored.
  WindowsVerbatimArguments = 1 << 2,
  /// Spawn the child process in a detached state - this will make it a process
  /// group leader, and will effectively enable the child to keep running after
  /// the parent exits. Note that the child process will still keep the
  /// parent's event loop alive unless the parent process calls uv_unref() on
  /// the child's process handle.
  Detached = 1 << 3,
  /// Hide the subprocess window that would normally be created. This option is
  /// only meaningful on Windows systems. On Unix it is silently ignored.
  WindowsHide = 1 << 4,
  /// Hide the subprocess console window that would normally be created. This
  /// option is only meaningful on Windows systems. On Unix it is silently
  /// ignored.
  WindowsHideConsole = 1 << 5,
  /// Hide the subprocess GUI window that would normally be created. This
  /// option is only meaningful on Windows systems. On Unix it is silently
  /// ignored.
  WindowsHideGui = 1 << 6,
  /// On Windows, if the path to the program to execute, specified in
  /// uv_process_options_t's file field, has a directory component,
  /// search for the exact file name before trying variants with
  /// extensions like '.exe' or '.cmd'.
  WindowsFilePathExactName = 1 << 7,
}

fn search_path_join_test(
  dir: &[u16],
  name: &[u16],
  ext: &[u16],
  cwd: &[u16],
) -> Option<WCString> {
  use windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_DIRECTORY;
  use windows_sys::Win32::Storage::FileSystem::GetFileAttributesW;
  use windows_sys::Win32::Storage::FileSystem::INVALID_FILE_ATTRIBUTES;

  let dir_len = dir.len();
  let name_len = name.len();
  let ext_len = ext.len();
  let mut cwd_len = cwd.len();

  // Adjust cwd_len based on the path type
  if dir_len > 2
    && ((dir[0] == wchar!('\\') || dir[0] == wchar!('/'))
      && (dir[1] == wchar!('\\') || dir[1] == wchar!('/')))
  {
    // UNC path, ignore cwd
    cwd_len = 0;
  } else if dir_len >= 1 && (dir[0] == wchar!('/') || dir[0] == wchar!('\\')) {
    // Full path without drive letter, use cwd's drive letter only
    cwd_len = 2;
  } else if dir_len >= 2
    && dir[1] == wchar!(':')
    && (dir_len < 3 || (dir[2] != wchar!('/') && dir[2] != wchar!('\\')))
  {
    // Relative path with drive letter
    if cwd_len < 2 || dir[..2] != cwd[..2] {
      cwd_len = 0;
    } else {
      // Skip the drive letter part in dir
      let new_dir = &dir[2..];
      return search_path_join_test(new_dir, name, ext, cwd);
    }
  } else if dir_len > 2 && dir[1] == wchar!(':') {
    // Absolute path with drive letter, don't use cwd
    cwd_len = 0;
  }

  // Allocate buffer for output
  let mut result = Vec::with_capacity(128);

  // Copy cwd
  if cwd_len > 0 {
    result.extend_from_slice(&cwd[..cwd_len]);

    // Add path separator if needed
    if let Some(last) = result.last()
      && !(*last == wchar!('\\')
        || *last == wchar!('/')
        || *last == wchar!(':'))
    {
      result.push(wchar!('\\'));
    }
  }

  // Copy dir
  if dir_len > 0 {
    result.extend_from_slice(&dir[..dir_len]);

    // Add separator if needed
    if let Some(last) = result.last()
      && !(*last == wchar!('\\')
        || *last == wchar!('/')
        || *last == wchar!(':'))
    {
      result.push(wchar!('\\'));
    }
  }

  // Copy filename
  result.extend_from_slice(&name[..name_len]);

  if ext_len > 0 {
    // Add dot if needed
    if name_len > 0 && result.last() != Some(&wchar!('.')) {
      result.push(wchar!('.'));
    }

    // Copy extension
    result.extend_from_slice(&ext[..ext_len]);
  }

  // Create WCString and check if file exists
  let path = WCString::from_vec(result);
  let attrs = unsafe { GetFileAttributesW(path.as_ptr()) };

  if attrs != INVALID_FILE_ATTRIBUTES && (attrs & FILE_ATTRIBUTE_DIRECTORY) == 0
  {
    Some(path)
  } else {
    None
  }
}

fn search_path_walk_ext(
  dir: &[u16],
  name: &[u16],
  cwd: &[u16],
  name_has_ext: bool,
) -> Option<WCString> {
  // If the name itself has a nonempty extension, try this extension first
  if name_has_ext
    && let Some(result) = search_path_join_test(dir, name, &[], cwd)
  {
    return Some(result);
  }

  // Try .com extension
  if let Some(result) = search_path_join_test(
    dir,
    name,
    &[wchar!('c'), wchar!('o'), wchar!('m')],
    cwd,
  ) {
    return Some(result);
  }

  // Try .exe extension
  if let Some(result) = search_path_join_test(
    dir,
    name,
    &[wchar!('e'), wchar!('x'), wchar!('e')],
    cwd,
  ) {
    return Some(result);
  }

  None
}

fn search_path(
  file: &[u16],
  cwd: &[u16],
  path: Option<&[u16]>,
  _flags: u32,
) -> Option<WCString> {
  // If the caller supplies an empty filename,
  // we're not gonna return c:\windows\.exe -- GFY!
  if file.is_empty() || (file.len() == 1 && file[0] == wchar!('.')) {
    return None;
  }

  let file_len = file.len();

  // Find the start of the filename so we can split the directory from the name
  let mut file_name_start = file_len;
  while file_name_start > 0 {
    let prev = file[file_name_start - 1];
    if prev == wchar!('\\') || prev == wchar!('/') || prev == wchar!(':') {
      break;
    }
    file_name_start -= 1;
  }

  let file_has_dir = file_name_start > 0;

  // Check if the filename includes an extension
  let name_slice = &file[file_name_start..];
  let dot_pos = name_slice.iter().position(|&c| c == wchar!('.'));
  let name_has_ext = dot_pos.is_some_and(|pos| pos + 1 < name_slice.len());

  if file_has_dir {
    // The file has a path inside, don't use path
    return search_path_walk_ext(
      &file[..file_name_start],
      &file[file_name_start..],
      cwd,
      name_has_ext,
    );
  } else {
    // Check if we need to search in the current directory first
    let empty = [0u16; 1];
    let need_cwd =
      unsafe { NeedCurrentDirectoryForExePathW(empty.as_ptr()) != 0 };

    if need_cwd {
      // The file is really only a name; look in cwd first, then scan path
      if let Some(result) = search_path_walk_ext(&[], file, cwd, name_has_ext) {
        return Some(result);
      }
    }

    // If path is None, we've checked cwd and there's nothing else to do
    let path = path?;

    // Handle path segments
    let mut dir_end = 0;
    loop {
      // If we've reached the end of the path, stop searching
      if dir_end >= path.len() || path[dir_end] == 0 {
        break;
      }

      // Skip the separator that dir_end now points to
      if dir_end > 0 || path[0] == wchar!(';') {
        dir_end += 1;
      }

      // Next slice starts just after where the previous one ended
      let dir_start = dir_end;

      // Handle quoted paths
      let is_quoted =
        path[dir_start] == wchar!('"') || path[dir_start] == wchar!('\'');
      let quote_char = if is_quoted { path[dir_start] } else { 0 };

      // Find the end of this directory component
      if is_quoted {
        // Find closing quote
        dir_end = dir_start + 1;
        while dir_end < path.len() && path[dir_end] != quote_char {
          dir_end += 1;
        }
        if dir_end == path.len() {
          // No closing quote, treat rest as the path
          dir_end = path.len();
        }
      }

      // Find next separator (;) or end
      while dir_end < path.len()
        && path[dir_end] != wchar!(';')
        && path[dir_end] != 0
      {
        dir_end += 1;
      }

      // If the slice is zero-length, don't bother
      if dir_end == dir_start {
        continue;
      }

      // Determine actual directory path, handling quotes
      let mut dir_path = &path[dir_start..dir_end];

      // Adjust if the path is quoted.
      if is_quoted && !dir_path.is_empty() {
        dir_path = &dir_path[1..]; // Skip opening quote
        if !dir_path.is_empty() && (dir_path[dir_path.len() - 1] == quote_char)
        {
          dir_path = &dir_path[..dir_path.len() - 1]; // Skip closing quote
        }
      }

      if let Some(result) =
        search_path_walk_ext(dir_path, file, cwd, name_has_ext)
      {
        return Some(result);
      }
    }
  }

  None
}

// Define signal values matching the ones in libuv
const SIGKILL: i32 = 9;
const SIGINT: i32 = 2;
const SIGTERM: i32 = 15;
const SIGQUIT: i32 = 3;

// Define total number of signals
const NSIG: i32 = 32;

// Define the dump options constant missing in the Windows crate
const AVX_XSTATE_CONTEXT: MINIDUMP_TYPE = 0x00200000;

/// Kill a process identified by process handle with a specific signal
///
/// Returns 0 on success, or a negative error code.
fn uv__kill(
  process_handle: HANDLE,
  signum: i32,
) -> Result<(), ProcessKillError> {
  // Validate signal number
  if !(0..NSIG).contains(&signum) {
    return Err(ProcessKillError::from_uv(uv_error::UV_EINVAL));
  }

  // Create a dump file for SIGQUIT
  if signum == SIGQUIT {
    unsafe {
      // Local variables
      let mut registry_key = 0;
      let pid = GetProcessId(process_handle);
      let mut basename_buf = [0u16; 260]; // MAX_PATH

      // Get target process name
      GetModuleBaseNameW(
        process_handle,
        ptr::null_mut(), // No module handle, want process name
        basename_buf.as_mut_ptr(),
        basename_buf.len() as u32,
      );

      // Get LocalDumps directory path
      let registry_result = RegOpenKeyExW(
        HKEY_LOCAL_MACHINE,
        w!("SOFTWARE\\Microsoft\\Windows\\Windows Error Reporting\\LocalDumps"),
        0,
        KEY_QUERY_VALUE,
        &mut registry_key as *mut _ as *mut HKEY,
      );

      if registry_result == ERROR_SUCCESS {
        let mut dump_folder = [0u16; 260]; // MAX_PATH
        let mut dump_name = [0u16; 260]; // MAX_PATH
        let mut dump_folder_len = dump_folder.len() as u32 * 2; // Size in bytes
        let mut key_type = 0;

        // Try to get DumpFolder from registry
        let ret = RegGetValueW(
          registry_key as HKEY,
          ptr::null(),
          w!("DumpFolder"),
          RRF_RT_ANY,
          &mut key_type,
          dump_folder.as_mut_ptr() as *mut _,
          &mut dump_folder_len,
        );

        if ret != ERROR_SUCCESS {
          // Default value for dump_folder is %LOCALAPPDATA%\CrashDumps
          let mut localappdata: *mut u16 = ptr::null_mut();
          SHGetKnownFolderPath(
            &FOLDERID_LocalAppData,
            0,
            ptr::null_mut(),
            &mut localappdata,
          );

          let localappdata_len = wcslen(localappdata);
          wcsncpy(dump_folder.as_mut_ptr(), localappdata, localappdata_len);

          let crashdumps = w!("\\CrashDumps");
          let crashdumps_len = wcslen(crashdumps);
          wcsncpy(
            dump_folder.as_mut_ptr().add(localappdata_len),
            crashdumps,
            crashdumps_len,
          );

          // Null-terminate
          dump_folder[localappdata_len + crashdumps_len] = 0;

          // Free the memory allocated by SHGetKnownFolderPath
          CoTaskMemFree(localappdata as _);
        }

        // Close registry key
        RegCloseKey(registry_key as HKEY);

        // Create dump folder if it doesn't already exist
        CreateDirectoryW(dump_folder.as_ptr(), ptr::null());

        // Construct dump filename from process name and PID
        // Find the null terminator in basename
        let mut basename_len = 0;
        while basename_len < basename_buf.len()
          && basename_buf[basename_len] != 0
        {
          basename_len += 1;
        }

        // Copy dump_folder to dump_name
        let mut dump_folder_len = 0;
        while dump_folder_len < dump_folder.len()
          && dump_folder[dump_folder_len] != 0
        {
          dump_name[dump_folder_len] = dump_folder[dump_folder_len];
          dump_folder_len += 1;
        }

        // Add path separator if needed
        if dump_folder_len > 0 && dump_name[dump_folder_len - 1] != wchar!('\\')
        {
          dump_name[dump_folder_len] = wchar!('\\');
          dump_folder_len += 1;
        }

        // Concatenate basename
        dump_name[dump_folder_len..(basename_len + dump_folder_len)]
          .copy_from_slice(&basename_buf[..basename_len]);
        dump_folder_len += basename_len;

        // Add dot and PID
        dump_name[dump_folder_len] = wchar!('.');
        dump_folder_len += 1;

        // Convert PID to characters
        let mut pid_remaining = pid;
        let mut pid_digits = [0u16; 10]; // Enough for 32-bit number
        let mut pid_len = 0;

        // Handle zero case explicitly
        if pid_remaining == 0 {
          pid_digits[0] = wchar!('0');
          pid_len = 1;
        } else {
          // Extract digits in reverse order
          while pid_remaining > 0 {
            pid_digits[pid_len] = wchar!('0') + (pid_remaining % 10) as u16;
            pid_remaining /= 10;
            pid_len += 1;
          }

          // Reverse the digits
          for i in 0..pid_len / 2 {
            pid_digits.swap(i, pid_len - 1 - i);
          }
        }

        // Add PID digits to dump_name
        dump_name[dump_folder_len..(pid_len + dump_folder_len)]
          .copy_from_slice(&pid_digits[..pid_len]);
        dump_folder_len += pid_len;

        // Add .dmp extension
        let dmp_ext = w!(".dmp");
        let dmp_ext_len = wcslen(dmp_ext);
        wcsncpy(
          dump_name.as_mut_ptr().add(dump_folder_len),
          dmp_ext,
          dmp_ext_len,
        );
        // Set null terminator
        dump_name[dump_folder_len + dmp_ext_len] = 0;

        // Create dump file
        let h_dump_file = CreateFileW(
          dump_name.as_ptr(),
          GENERIC_WRITE,
          0,
          ptr::null(),
          CREATE_NEW,
          FILE_ATTRIBUTE_NORMAL,
          ptr::null_mut(),
        );

        if h_dump_file != INVALID_HANDLE_VALUE {
          // Check against INVALID_HANDLE_VALUE
          // If something goes wrong while writing it out, delete the file
          let delete_on_close = FILE_DISPOSITION_INFO { DeleteFile: 1 }; // 1 = TRUE for DeleteFile
          SetFileInformationByHandle(
            h_dump_file,
            FileDispositionInfo,
            &delete_on_close as *const _ as *const _,
            std::mem::size_of::<FILE_DISPOSITION_INFO>() as u32,
          );

          // Tell wine to dump ELF modules as well
          let sym_options = SymGetOptions();
          SymSetOptions(sym_options | 0x40000000);

          // We default to a fairly complete dump
          let dump_options: MINIDUMP_TYPE = MiniDumpWithFullMemory
            | MiniDumpIgnoreInaccessibleMemory
            | AVX_XSTATE_CONTEXT;

          let success = MiniDumpWriteDump(
            process_handle,
            pid,
            h_dump_file,
            dump_options,
            ptr::null(),
            ptr::null(),
            ptr::null(),
          );

          if success != 0 {
            // Don't delete the file on close if we successfully wrote it out
            let dont_delete_on_close = FILE_DISPOSITION_INFO { DeleteFile: 0 }; // 0 = FALSE for DeleteFile
            SetFileInformationByHandle(
              h_dump_file,
              FileDispositionInfo,
              &dont_delete_on_close as *const _ as *const _,
              std::mem::size_of::<FILE_DISPOSITION_INFO>() as u32,
            );
          }

          // Restore symbol options
          SymSetOptions(sym_options);

          // Close dump file
          CloseHandle(h_dump_file);
        }
      }
    }
  }

  // Handle different signal cases
  match signum {
    SIGQUIT | SIGTERM | SIGKILL | SIGINT => {
      // Unconditionally terminate the process
      unsafe {
        if TerminateProcess(process_handle, 1) != 0 {
          return Ok(());
        }

        // If the process already exited before TerminateProcess was called,
        // TerminateProcess will fail with ERROR_ACCESS_DENIED
        let err = GetLastError();
        if err == ERROR_ACCESS_DENIED {
          // First check using GetExitCodeProcess() with status different from
          // STILL_ACTIVE (259)
          let mut status = 0;
          if GetExitCodeProcess(process_handle, &mut status) != 0
            && status != STILL_ACTIVE as u32
          {
            return Err(ProcessKillError::esrch());
          }

          // But the process could have exited with code == STILL_ACTIVE, use
          // WaitForSingleObject with timeout zero
          if WaitForSingleObject(process_handle, 0) == WAIT_OBJECT_0 {
            return Err(ProcessKillError::esrch());
          }
        }

        Err(ProcessKillError::from_sys_error(err))
      }
    }

    // Health check: is the process still alive?
    0 => unsafe {
      let mut status = 0;
      if GetExitCodeProcess(process_handle, &mut status) == 0 {
        return Err(ProcessKillError::from_last_error());
      }

      if status != STILL_ACTIVE as u32 {
        return Err(ProcessKillError::esrch());
      }

      match WaitForSingleObject(process_handle, 0) {
        WAIT_OBJECT_0 => Err(ProcessKillError::esrch()),
        WAIT_FAILED => Err(ProcessKillError::from_last_error()),
        WAIT_TIMEOUT => Ok(()),
        _ => Err(ProcessKillError::from_uv(uv_error::UV_UNKNOWN)),
      }
    },

    // Unsupported signal
    _ => {
      Err(ProcessKillError::from_uv(uv_error::UV_EINVAL)) // TODO: is this correct?
    }
  }
}

pub struct ProcessKillError {
  uv_error: i32,
  sys_error: Option<u32>,
}

impl ProcessKillError {
  pub fn as_uv_error(&self) -> i32 {
    self.uv_error
  }

  fn esrch() -> Self {
    Self::from_uv(uv_error::UV_ESRCH)
  }

  fn from_uv(code: i32) -> Self {
    Self {
      uv_error: code,
      sys_error: None,
    }
  }

  pub fn as_sys_error(&self) -> Option<u32> {
    self.sys_error
  }

  fn from_sys_error(err: u32) -> Self {
    Self {
      uv_error: uv_error::uv_translate_sys_error(err),
      sys_error: Some(err),
    }
  }

  fn from_last_error() -> Self {
    let sys_error = get_last_error();
    Self {
      uv_error: uv_error::uv_translate_sys_error(sys_error),
      sys_error: Some(sys_error),
    }
  }
}

/// Kill a process using its pid
pub fn process_kill(pid: i32, signum: i32) -> Result<(), ProcessKillError> {
  unsafe {
    // Get process handle based on pid
    let process_handle = if pid == 0 {
      GetCurrentProcess()
    } else {
      OpenProcess(
        PROCESS_TERMINATE | PROCESS_QUERY_INFORMATION | SYNCHRONIZE,
        FALSE,
        pid as u32,
      )
    };

    if process_handle.is_null() {
      let err = get_last_error();
      if err == ERROR_INVALID_PARAMETER {
        return Err(ProcessKillError::from_uv(uv_error::UV_ESRCH));
      }
      return Err(ProcessKillError::from_sys_error(err));
    }

    let result = uv__kill(process_handle, signum);

    // Close the handle if we opened it
    if pid != 0 {
      CloseHandle(process_handle);
    }

    result
  }
}

// lifted from https://github.com/rust-lang/rust/blob/bc1d7273dfbc6f8a11c0086fa35f6748a13e8d3c/library/std/src/sys/args/windows.rs#L293
// Copyright The Rust Project Contributors - MIT
fn make_bat_command_line(
  script: &[u16],
  args: &[&OsStr],
  force_quotes: bool,
) -> io::Result<Vec<u16>> {
  // Set the start of the command line to `cmd.exe /c "`
  // It is necessary to surround the command in an extra pair of quotes,
  // hence the trailing quote here. It will be closed after all arguments
  // have been added.
  // Using /e:ON enables "command extensions" which is essential for the `%` hack to work.
  let mut cmd: Vec<u16> = "/e:ON /v:OFF /d /c \"".encode_utf16().collect();

  // Push the script name surrounded by its quote pair.
  cmd.push(b'"' as u16);
  // Windows file names cannot contain a `"` character or end with `\\`.
  // If the script name does then return an error.
  if script.contains(&(b'"' as u16)) || script.last() == Some(&(b'\\' as u16)) {
    return Err(std::io::Error::new(
      io::ErrorKind::InvalidInput,
      "Windows file names may not contain `\"` or end with `\\`",
    ));
  }
  cmd.extend_from_slice(script.strip_suffix(&[0]).unwrap_or(script));
  cmd.push(b'"' as u16);

  // Append the arguments.
  // FIXME: This needs tests to ensure that the arguments are properly
  // reconstructed by the batch script by default.
  for arg in args.iter().skip(1) {
    cmd.push(' ' as u16);
    let arg_bytes = arg.as_encoded_bytes();
    // Disallow \r and \n as they may truncate the arguments.
    const DISALLOWED: &[u8] = b"\r\n";
    if arg_bytes.iter().any(|c| DISALLOWED.contains(c)) {
      return Err(std::io::Error::new(
        io::ErrorKind::InvalidInput,
        r#"batch file arguments are invalid"#,
      ));
    }
    append_bat_arg(&mut cmd, arg, force_quotes)?;
  }

  // Close the quote we left opened earlier.
  cmd.push(b'"' as u16);

  Ok(cmd)
}

// lifted from https://github.com/rust-lang/rust/blob/bc1d7273dfbc6f8a11c0086fa35f6748a13e8d3c/library/std/src/sys/args/windows.rs#L220C1-L291C2
// Copyright The Rust Project Contributors - MIT
fn append_bat_arg(
  cmd: &mut Vec<u16>,
  arg: &OsStr,
  mut quote: bool,
) -> io::Result<()> {
  ensure_no_nuls(arg)?;
  // If an argument has 0 characters then we need to quote it to ensure
  // that it actually gets passed through on the command line or otherwise
  // it will be dropped entirely when parsed on the other end.
  //
  // We also need to quote the argument if it ends with `\` to guard against
  // bat usage such as `"%~2"` (i.e. force quote arguments) otherwise a
  // trailing slash will escape the closing quote.
  if arg.is_empty() || arg.as_encoded_bytes().last() == Some(&b'\\') {
    quote = true;
  }
  for cp in arg.encode_wide() {
    if let Some(cp) = char::decode_utf16([cp]).next().and_then(|r| r.ok()) {
      // Rather than trying to find every ascii symbol that must be quoted,
      // we assume that all ascii symbols must be quoted unless they're known to be good.
      // We also quote Unicode control blocks for good measure.
      // Note an unquoted `\` is fine so long as the argument isn't otherwise quoted.
      static UNQUOTED: &str = r"#$*+-./:?@\_";
      let ascii_needs_quotes =
        cp.is_ascii() && !(cp.is_ascii_alphanumeric() || UNQUOTED.contains(cp));
      if ascii_needs_quotes || cp.is_control() {
        quote = true;
      }
    }
  }

  if quote {
    cmd.push('"' as u16);
  }
  // Loop through the string, escaping `\` only if followed by `"`.
  // And escaping `"` by doubling them.
  let mut backslashes: usize = 0;
  for x in arg.encode_wide() {
    if x == '\\' as u16 {
      backslashes += 1;
    } else {
      if x == '"' as u16 {
        // Add n backslashes to total 2n before internal `"`.
        cmd.extend((0..backslashes).map(|_| '\\' as u16));
        // Appending an additional double-quote acts as an escape.
        cmd.push(b'"' as u16)
      } else if x == '%' as u16 || x == '\r' as u16 {
        // yt-dlp hack: replaces `%` with `%%cd:~,%` to stop %VAR% being expanded as an environment variable.
        //
        // # Explanation
        //
        // cmd supports extracting a substring from a variable using the following syntax:
        //     %variable:~start_index,end_index%
        //
        // In the above command `cd` is used as the variable and the start_index and end_index are left blank.
        // `cd` is a built-in variable that dynamically expands to the current directory so it's always available.
        // Explicitly omitting both the start and end index creates a zero-length substring.
        //
        // Therefore it all resolves to nothing. However, by doing this no-op we distract cmd.exe
        // from potentially expanding %variables% in the argument.
        cmd.extend_from_slice(&[
          '%' as u16, '%' as u16, 'c' as u16, 'd' as u16, ':' as u16,
          '~' as u16, ',' as u16,
        ]);
      }
      backslashes = 0;
    }
    cmd.push(x);
  }
  if quote {
    // Add n backslashes to total 2n before ending `"`.
    cmd.extend((0..backslashes).map(|_| '\\' as u16));
    cmd.push('"' as u16);
  }
  Ok(())
}

// lifted from https://github.com/rust-lang/rust/blob/bc1d7273dfbc6f8a11c0086fa35f6748a13e8d3c/library/std/src/sys/pal/windows/mod.rs#L289
// Copyright The Rust Project Contributors - MIT
fn ensure_no_nuls<T: AsRef<OsStr>>(s: T) -> crate::io::Result<T> {
  if s.as_ref().encode_wide().any(|b| b == 0) {
    Err(std::io::Error::new(
      io::ErrorKind::InvalidInput,
      "nul byte found in provided data",
    ))
  } else {
    Ok(s)
  }
}

fn command_prompt() -> io::Result<WCString> {
  let mut buffer =
    vec![0u16; windows_sys::Win32::Foundation::MAX_PATH as usize];
  let len =
    unsafe { GetSystemDirectoryW(buffer.as_mut_ptr(), buffer.len() as u32) };
  if len == 0 {
    return Err(io::Error::last_os_error());
  }
  buffer.truncate(len as usize);
  buffer.extend("\\cmd.exe".encode_utf16().chain([0]));
  Ok(WCString::from_vec(buffer))
}

#[cfg(test)]
mod tests {
  use pretty_assertions::assert_eq;

  use super::*;
  #[test]
  fn test_quote_cmd_arg() {
    let cases = [
      ("hello\"world", r#""hello\"world""#),
      ("hello\"\"world", r#""hello\"\"world""#),
      ("hello\\world", "hello\\world"),
      ("hello\\\\world", "hello\\\\world"),
      ("hello\\\"world", r#""hello\\\"world""#),
      ("hello\\\\\"world", r#""hello\\\\\"world""#),
      ("hello world\\", r#""hello world\\""#),
    ];

    for (input, expected) in cases {
      let s = input.encode_utf16().chain(Some(0)).collect::<Vec<_>>();
      let s = WCString::from_vec(s);
      let mut out = Vec::new();
      quote_cmd_arg(s.as_wcstr(), &mut out);
      let out_s = String::from_utf16_lossy(&out);
      assert_eq!(out_s, expected);
    }
  }

  #[test]
  fn test_make_program_args() {
    let args = ["hello", "world", "\"hello world\""]
      .into_iter()
      .map(|s| s.as_ref())
      .collect::<Vec<_>>();
    let verbatim_arguments = false;
    let result = make_program_args(&args, verbatim_arguments).unwrap();
    assert_eq!(result, WCString::new("hello world \"\\\"hello world\\\"\""));
  }
}
