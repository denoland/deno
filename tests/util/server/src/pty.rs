// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::borrow::Cow;
use std::collections::HashMap;
use std::collections::HashSet;
use std::io::Read;
use std::io::Write;
use std::path::Path;
use std::time::Duration;
use std::time::Instant;

use once_cell::sync::Lazy;

use crate::strip_ansi_codes;

static IS_CI: Lazy<bool> = Lazy::new(|| std::env::var("CI").is_ok());

/// Points to know about when writing pty tests:
///
/// - Consecutive writes cause issues where you might write while a prompt
///   is not showing. So when you write, always `.expect(...)` on the output.
/// - Similar to the last point, using `.expect(...)` can help make the test
///   more deterministic. If the test is flaky, try adding more `.expect(...)`s
pub struct Pty {
  pty: Box<dyn SystemPty>,
  read_bytes: Vec<u8>,
  last_index: usize,
}

impl Pty {
  pub fn new(
    program: &Path,
    args: &[&str],
    cwd: &Path,
    env_vars: Option<HashMap<String, String>>,
  ) -> Self {
    let pty = create_pty(program, args, cwd, env_vars);
    let mut pty = Self {
      pty,
      read_bytes: Vec::new(),
      last_index: 0,
    };
    if args.is_empty() || args[0] == "repl" && !args.contains(&"--quiet") {
      // wait for the repl to start up before writing to it
      pty.read_until_condition_with_timeout(
        |pty| {
          pty
            .all_output()
            .contains("exit using ctrl+d, ctrl+c, or close()")
        },
        // it sometimes takes a while to startup on the CI, so use a longer timeout
        Duration::from_secs(60),
      );
    }

    pty
  }

  pub fn is_supported() -> bool {
    let is_windows = cfg!(windows);
    if is_windows && *IS_CI {
      // the pty tests don't really start up on the windows CI for some reason
      // so ignore them for now
      eprintln!("Ignoring windows CI.");
      false
    } else {
      true
    }
  }

  #[track_caller]
  pub fn write_raw(&mut self, line: impl AsRef<str>) {
    let line = if cfg!(windows) {
      line.as_ref().replace('\n', "\r\n")
    } else {
      line.as_ref().to_string()
    };
    if let Err(err) = self.pty.write(line.as_bytes()) {
      panic!("{:#}", err)
    }
    self.pty.flush().unwrap();
  }

  /// Pause for a human-like delay to read or react to something (human responses are ~100ms).
  #[track_caller]
  pub fn human_delay(&mut self) {
    std::thread::sleep(Duration::from_millis(250));
  }

  #[track_caller]
  pub fn write_line(&mut self, line: impl AsRef<str>) {
    self.write_line_raw(&line);

    // expect what was written to show up in the output
    // due to "pty echo"
    for line in line.as_ref().lines() {
      self.expect(line);
    }
  }

  /// Writes a line without checking if it's in the output.
  #[track_caller]
  pub fn write_line_raw(&mut self, line: impl AsRef<str>) {
    self.write_raw(format!("{}\n", line.as_ref()));
  }

  #[track_caller]
  pub fn read_until(&mut self, end_text: impl AsRef<str>) -> String {
    self.read_until_with_advancing(|text| {
      text
        .find(end_text.as_ref())
        .map(|index| index + end_text.as_ref().len())
    })
  }

  #[track_caller]
  pub fn expect(&mut self, text: impl AsRef<str>) {
    self.read_until(text.as_ref());
  }

  #[track_caller]
  pub fn expect_any(&mut self, texts: &[&str]) {
    self.read_until_with_advancing(|text| {
      for find_text in texts {
        if let Some(index) = text.find(find_text) {
          return Some(index);
        }
      }
      None
    });
  }

  /// Consumes and expects to find all the text until a timeout is hit.
  #[track_caller]
  pub fn expect_all(&mut self, texts: &[&str]) {
    let mut pending_texts: HashSet<&&str> = HashSet::from_iter(texts);
    let mut max_index: Option<usize> = None;
    self.read_until_with_advancing(|text| {
      for pending_text in pending_texts.clone() {
        if let Some(index) = text.find(pending_text) {
          let index = index + pending_text.len();
          match &max_index {
            Some(current) => {
              if *current < index {
                max_index = Some(index);
              }
            }
            None => {
              max_index = Some(index);
            }
          }
          pending_texts.remove(pending_text);
        }
      }
      if pending_texts.is_empty() {
        max_index
      } else {
        None
      }
    });
  }

  /// Expects the raw text to be found, which may include ANSI codes.
  /// Note: this expects the raw bytes in any output that has already
  /// occurred or may occur within the next few seconds.
  #[track_caller]
  pub fn expect_raw_in_current_output(&mut self, text: impl AsRef<str>) {
    self.read_until_condition(|pty| {
      let data = String::from_utf8_lossy(&pty.read_bytes);
      data.contains(text.as_ref())
    });
  }

  /// Expects the raw text to be found next.
  #[track_caller]
  pub fn expect_raw_next(&mut self, text: impl AsRef<str>) {
    let expected = text.as_ref();
    let last_index = self.read_bytes.len();
    self.read_until_condition(|pty| {
      if pty.read_bytes.len() >= last_index + expected.len() {
        let data = String::from_utf8_lossy(
          &pty.read_bytes[last_index..last_index + expected.len()],
        );
        data == expected
      } else {
        false
      }
    });
  }

  pub fn all_output(&self) -> Cow<str> {
    String::from_utf8_lossy(&self.read_bytes)
  }

  #[track_caller]
  fn read_until_with_advancing(
    &mut self,
    mut condition: impl FnMut(&str) -> Option<usize>,
  ) -> String {
    let mut final_text = String::new();
    self.read_until_condition(|pty| {
      let text = pty.next_text();
      if let Some(end_index) = condition(&text) {
        pty.last_index += end_index;
        final_text = text[..end_index].to_string();
        true
      } else {
        false
      }
    });
    final_text
  }

  #[track_caller]
  fn read_until_condition(&mut self, condition: impl FnMut(&mut Self) -> bool) {
    let duration = if *IS_CI {
      Duration::from_secs(30)
    } else {
      Duration::from_secs(15)
    };
    self.read_until_condition_with_timeout(condition, duration);
  }

  #[track_caller]
  fn read_until_condition_with_timeout(
    &mut self,
    condition: impl FnMut(&mut Self) -> bool,
    timeout_duration: Duration,
  ) {
    if self.try_read_until_condition_with_timeout(condition, timeout_duration) {
      return;
    }

    panic!("Timed out.")
  }

  /// Reads until the specified condition with a timeout duration returning
  /// `true` on success or `false` on timeout.
  fn try_read_until_condition_with_timeout(
    &mut self,
    mut condition: impl FnMut(&mut Self) -> bool,
    timeout_duration: Duration,
  ) -> bool {
    let timeout_time = Instant::now().checked_add(timeout_duration).unwrap();
    while Instant::now() < timeout_time {
      self.fill_more_bytes();
      if condition(self) {
        return true;
      }
    }

    let text = self.next_text();
    eprintln!(
      "------ Start Full Text ------\n{:?}\n------- End Full Text -------",
      String::from_utf8_lossy(&self.read_bytes)
    );
    eprintln!("Next text: {:?}", text);

    false
  }

  fn next_text(&self) -> String {
    let text = String::from_utf8_lossy(&self.read_bytes).to_string();
    let text = strip_ansi_codes(&text);
    text[self.last_index..].to_string()
  }

  fn fill_more_bytes(&mut self) {
    let mut buf = [0; 256];
    match self.pty.read(&mut buf) {
      Ok(count) if count > 0 => {
        self.read_bytes.extend(&buf[..count]);
      }
      _ => {
        // be a bit easier on the CI
        std::thread::sleep(Duration::from_millis(if *IS_CI {
          100
        } else {
          20
        }));
      }
    }
  }
}

trait SystemPty: Read + Write {}

impl SystemPty for std::fs::File {}

#[cfg(unix)]
fn setup_pty(fd: i32) {
  use nix::fcntl::fcntl;
  use nix::fcntl::FcntlArg;
  use nix::fcntl::OFlag;
  use nix::sys::termios;
  use nix::sys::termios::tcgetattr;
  use nix::sys::termios::tcsetattr;
  use nix::sys::termios::SetArg;

  let mut term = tcgetattr(fd).unwrap();
  // disable cooked mode
  term.local_flags.remove(termios::LocalFlags::ICANON);
  tcsetattr(fd, SetArg::TCSANOW, &term).unwrap();

  // turn on non-blocking mode so we get timeouts
  let flags = fcntl(fd, FcntlArg::F_GETFL).unwrap();
  let new_flags = OFlag::from_bits_truncate(flags) | OFlag::O_NONBLOCK;
  fcntl(fd, FcntlArg::F_SETFL(new_flags)).unwrap();
}

#[cfg(unix)]
fn create_pty(
  program: &Path,
  args: &[&str],
  cwd: &Path,
  env_vars: Option<HashMap<String, String>>,
) -> Box<dyn SystemPty> {
  use crate::pty::unix::UnixPty;
  use std::os::unix::process::CommandExt;

  // Manually open pty main/secondary sides in the test process. Since we're not actually
  // changing uid/gid here, this is the easiest way to do it.

  // SAFETY: Posix APIs
  let (fdm, fds) = unsafe {
    let fdm = libc::posix_openpt(libc::O_RDWR);
    if fdm < 0 {
      panic!("posix_openpt failed");
    }
    let res = libc::grantpt(fdm);
    if res != 0 {
      panic!("grantpt failed");
    }
    let res = libc::unlockpt(fdm);
    if res != 0 {
      panic!("unlockpt failed");
    }
    let fds = libc::open(libc::ptsname(fdm), libc::O_RDWR);
    if fdm < 0 {
      panic!("open(ptsname) failed");
    }
    (fdm, fds)
  };

  // SAFETY: Posix APIs
  unsafe {
    let cmd = std::process::Command::new(program)
      .current_dir(cwd)
      .args(args)
      .envs(env_vars.unwrap_or_default())
      .pre_exec(move || {
        // Close parent's main handle
        libc::close(fdm);
        libc::dup2(fds, 0);
        libc::dup2(fds, 1);
        libc::dup2(fds, 2);
        // Note that we could close `fds` here as well, but this is a short-lived process and
        // we're just not going to worry about "leaking" it
        Ok(())
      })
      .spawn()
      .unwrap();

    // Close child's secondary handle
    libc::close(fds);
    setup_pty(fdm);

    use std::os::fd::FromRawFd;
    let pid = nix::unistd::Pid::from_raw(cmd.id() as _);
    let file = std::fs::File::from_raw_fd(fdm);
    Box::new(UnixPty { pid, file })
  }
}

#[cfg(unix)]
mod unix {
  use std::io::Read;
  use std::io::Write;

  use super::SystemPty;

  pub struct UnixPty {
    pub pid: nix::unistd::Pid,
    pub file: std::fs::File,
  }

  impl Drop for UnixPty {
    fn drop(&mut self) {
      use nix::sys::signal::kill;
      use nix::sys::signal::Signal;
      kill(self.pid, Signal::SIGTERM).unwrap()
    }
  }

  impl SystemPty for UnixPty {}

  impl Read for UnixPty {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
      self.file.read(buf)
    }
  }

  impl Write for UnixPty {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
      self.file.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
      self.file.flush()
    }
  }
}

#[cfg(target_os = "windows")]
fn create_pty(
  program: &Path,
  args: &[&str],
  cwd: &Path,
  env_vars: Option<HashMap<String, String>>,
) -> Box<dyn SystemPty> {
  let pty = windows::WinPseudoConsole::new(program, args, cwd, env_vars);
  Box::new(pty)
}

#[cfg(target_os = "windows")]
mod windows {
  use std::collections::HashMap;
  use std::io::ErrorKind;
  use std::io::Read;
  use std::path::Path;
  use std::ptr;
  use std::time::Duration;

  use winapi::shared::minwindef::FALSE;
  use winapi::shared::minwindef::LPVOID;
  use winapi::shared::minwindef::TRUE;
  use winapi::shared::winerror::S_OK;
  use winapi::um::consoleapi::ClosePseudoConsole;
  use winapi::um::consoleapi::CreatePseudoConsole;
  use winapi::um::fileapi::FlushFileBuffers;
  use winapi::um::fileapi::ReadFile;
  use winapi::um::fileapi::WriteFile;
  use winapi::um::handleapi::DuplicateHandle;
  use winapi::um::handleapi::INVALID_HANDLE_VALUE;
  use winapi::um::namedpipeapi::CreatePipe;
  use winapi::um::namedpipeapi::PeekNamedPipe;
  use winapi::um::processthreadsapi::CreateProcessW;
  use winapi::um::processthreadsapi::DeleteProcThreadAttributeList;
  use winapi::um::processthreadsapi::GetCurrentProcess;
  use winapi::um::processthreadsapi::InitializeProcThreadAttributeList;
  use winapi::um::processthreadsapi::UpdateProcThreadAttribute;
  use winapi::um::processthreadsapi::LPPROC_THREAD_ATTRIBUTE_LIST;
  use winapi::um::processthreadsapi::PROCESS_INFORMATION;
  use winapi::um::synchapi::WaitForSingleObject;
  use winapi::um::winbase::CREATE_UNICODE_ENVIRONMENT;
  use winapi::um::winbase::EXTENDED_STARTUPINFO_PRESENT;
  use winapi::um::winbase::INFINITE;
  use winapi::um::winbase::STARTUPINFOEXW;
  use winapi::um::wincontypes::COORD;
  use winapi::um::wincontypes::HPCON;
  use winapi::um::winnt::DUPLICATE_SAME_ACCESS;
  use winapi::um::winnt::HANDLE;

  use super::SystemPty;

  macro_rules! assert_win_success {
    ($expression:expr) => {
      let success = $expression;
      if success != TRUE {
        panic!("{}", std::io::Error::last_os_error().to_string())
      }
    };
  }

  macro_rules! handle_err {
    ($expression:expr) => {
      let success = $expression;
      if success != TRUE {
        return Err(std::io::Error::last_os_error());
      }
    };
  }

  pub struct WinPseudoConsole {
    stdin_write_handle: WinHandle,
    stdout_read_handle: WinHandle,
    // keep these alive for the duration of the pseudo console
    _process_handle: WinHandle,
    _thread_handle: WinHandle,
    _attribute_list: ProcThreadAttributeList,
  }

  impl WinPseudoConsole {
    pub fn new(
      program: &Path,
      args: &[&str],
      cwd: &Path,
      maybe_env_vars: Option<HashMap<String, String>>,
    ) -> Self {
      // https://docs.microsoft.com/en-us/windows/console/creating-a-pseudoconsole-session
      // SAFETY:
      // Generous use of winapi to create a PTY (thus large unsafe block).
      unsafe {
        let mut size: COORD = std::mem::zeroed();
        size.X = 800;
        size.Y = 500;
        let mut console_handle = std::ptr::null_mut();
        let (stdin_read_handle, stdin_write_handle) = create_pipe();
        let (stdout_read_handle, stdout_write_handle) = create_pipe();

        let result = CreatePseudoConsole(
          size,
          stdin_read_handle.as_raw_handle(),
          stdout_write_handle.as_raw_handle(),
          0,
          &mut console_handle,
        );
        assert_eq!(result, S_OK);

        let mut environment_vars = maybe_env_vars.map(get_env_vars);
        let mut attribute_list = ProcThreadAttributeList::new(console_handle);
        let mut startup_info: STARTUPINFOEXW = std::mem::zeroed();
        startup_info.StartupInfo.cb =
          std::mem::size_of::<STARTUPINFOEXW>() as u32;
        startup_info.lpAttributeList = attribute_list.as_mut_ptr();

        let mut proc_info: PROCESS_INFORMATION = std::mem::zeroed();
        let command = format!(
          "\"{}\" {}",
          program.to_string_lossy(),
          args
            .iter()
            .map(|a| format!("\"{}\"", a))
            .collect::<Vec<_>>()
            .join(" ")
        )
        .trim()
        .to_string();
        let mut application_str = to_windows_str(&program.to_string_lossy());
        let mut command_str = to_windows_str(&command);
        let cwd = cwd.to_string_lossy().replace('/', "\\");
        let mut cwd = to_windows_str(&cwd);

        assert_win_success!(CreateProcessW(
          application_str.as_mut_ptr(),
          command_str.as_mut_ptr(),
          ptr::null_mut(),
          ptr::null_mut(),
          FALSE,
          EXTENDED_STARTUPINFO_PRESENT | CREATE_UNICODE_ENVIRONMENT,
          environment_vars
            .as_mut()
            .map(|v| v.as_mut_ptr() as LPVOID)
            .unwrap_or(ptr::null_mut()),
          cwd.as_mut_ptr(),
          &mut startup_info.StartupInfo,
          &mut proc_info,
        ));

        // close the handles that the pseudoconsole now has
        drop(stdin_read_handle);
        drop(stdout_write_handle);

        // start a thread that will close the pseudoconsole on process exit
        let thread_handle = WinHandle::new(proc_info.hThread);
        std::thread::spawn({
          let thread_handle = thread_handle.duplicate();
          let console_handle = WinHandle::new(console_handle);
          move || {
            WaitForSingleObject(thread_handle.as_raw_handle(), INFINITE);
            // wait for the reading thread to catch up
            std::thread::sleep(Duration::from_millis(200));
            // close the console handle which will close the
            // stdout pipe for the reader
            ClosePseudoConsole(console_handle.into_raw_handle());
          }
        });

        Self {
          stdin_write_handle,
          stdout_read_handle,
          _process_handle: WinHandle::new(proc_info.hProcess),
          _thread_handle: thread_handle,
          _attribute_list: attribute_list,
        }
      }
    }
  }

  impl Read for WinPseudoConsole {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
      // don't do a blocking read in order to support timing out
      let mut bytes_available = 0;
      // SAFETY: winapi call
      handle_err!(unsafe {
        PeekNamedPipe(
          self.stdout_read_handle.as_raw_handle(),
          ptr::null_mut(),
          0,
          ptr::null_mut(),
          &mut bytes_available,
          ptr::null_mut(),
        )
      });
      if bytes_available == 0 {
        return Err(std::io::Error::new(ErrorKind::WouldBlock, "Would block."));
      }

      let mut bytes_read = 0;
      // SAFETY: winapi call
      handle_err!(unsafe {
        ReadFile(
          self.stdout_read_handle.as_raw_handle(),
          buf.as_mut_ptr() as _,
          buf.len() as u32,
          &mut bytes_read,
          ptr::null_mut(),
        )
      });

      Ok(bytes_read as usize)
    }
  }

  impl SystemPty for WinPseudoConsole {}

  impl std::io::Write for WinPseudoConsole {
    fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
      let mut bytes_written = 0;
      // SAFETY:
      // winapi call
      handle_err!(unsafe {
        WriteFile(
          self.stdin_write_handle.as_raw_handle(),
          buffer.as_ptr() as *const _,
          buffer.len() as u32,
          &mut bytes_written,
          ptr::null_mut(),
        )
      });
      Ok(bytes_written as usize)
    }

    fn flush(&mut self) -> std::io::Result<()> {
      // SAFETY: winapi call
      handle_err!(unsafe {
        FlushFileBuffers(self.stdin_write_handle.as_raw_handle())
      });
      Ok(())
    }
  }

  struct WinHandle {
    inner: HANDLE,
  }

  impl WinHandle {
    pub fn new(handle: HANDLE) -> Self {
      WinHandle { inner: handle }
    }

    pub fn duplicate(&self) -> WinHandle {
      // SAFETY: winapi call
      let process_handle = unsafe { GetCurrentProcess() };
      let mut duplicate_handle = ptr::null_mut();
      // SAFETY: winapi call
      assert_win_success!(unsafe {
        DuplicateHandle(
          process_handle,
          self.inner,
          process_handle,
          &mut duplicate_handle,
          0,
          0,
          DUPLICATE_SAME_ACCESS,
        )
      });

      WinHandle::new(duplicate_handle)
    }

    pub fn as_raw_handle(&self) -> HANDLE {
      self.inner
    }

    pub fn into_raw_handle(self) -> HANDLE {
      let handle = self.inner;
      // skip the drop implementation in order to not close the handle
      std::mem::forget(self);
      handle
    }
  }

  // SAFETY: These handles are ok to send across threads.
  unsafe impl Send for WinHandle {}
  // SAFETY: These handles are ok to send across threads.
  unsafe impl Sync for WinHandle {}

  impl Drop for WinHandle {
    fn drop(&mut self) {
      if !self.inner.is_null() && self.inner != INVALID_HANDLE_VALUE {
        // SAFETY: winapi call
        unsafe {
          winapi::um::handleapi::CloseHandle(self.inner);
        }
      }
    }
  }

  struct ProcThreadAttributeList {
    buffer: Vec<u8>,
  }

  impl ProcThreadAttributeList {
    pub fn new(console_handle: HPCON) -> Self {
      // SAFETY:
      // Generous use of unsafe winapi calls to create a ProcThreadAttributeList.
      unsafe {
        // discover size required for the list
        let mut size = 0;
        let attribute_count = 1;
        assert_eq!(
          InitializeProcThreadAttributeList(
            ptr::null_mut(),
            attribute_count,
            0,
            &mut size
          ),
          FALSE
        );

        let mut buffer = vec![0u8; size];
        let attribute_list_ptr = buffer.as_mut_ptr() as _;

        assert_win_success!(InitializeProcThreadAttributeList(
          attribute_list_ptr,
          attribute_count,
          0,
          &mut size,
        ));

        const PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE: usize = 0x00020016;
        assert_win_success!(UpdateProcThreadAttribute(
          attribute_list_ptr,
          0,
          PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE,
          console_handle,
          std::mem::size_of::<HPCON>(),
          ptr::null_mut(),
          ptr::null_mut(),
        ));

        ProcThreadAttributeList { buffer }
      }
    }

    pub fn as_mut_ptr(&mut self) -> LPPROC_THREAD_ATTRIBUTE_LIST {
      self.buffer.as_mut_slice().as_mut_ptr() as *mut _
    }
  }

  impl Drop for ProcThreadAttributeList {
    fn drop(&mut self) {
      // SAFETY: winapi call
      unsafe { DeleteProcThreadAttributeList(self.as_mut_ptr()) };
    }
  }

  fn create_pipe() -> (WinHandle, WinHandle) {
    let mut read_handle = std::ptr::null_mut();
    let mut write_handle = std::ptr::null_mut();

    // SAFETY: Creating an anonymous pipe with winapi.
    assert_win_success!(unsafe {
      CreatePipe(&mut read_handle, &mut write_handle, ptr::null_mut(), 0)
    });

    (WinHandle::new(read_handle), WinHandle::new(write_handle))
  }

  fn to_windows_str(str: &str) -> Vec<u16> {
    use std::os::windows::prelude::OsStrExt;
    std::ffi::OsStr::new(str)
      .encode_wide()
      .chain(Some(0))
      .collect()
  }

  fn get_env_vars(env_vars: HashMap<String, String>) -> Vec<u16> {
    // See lpEnvironment: https://docs.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-createprocessw
    let mut parts = env_vars
      .into_iter()
      // each environment variable is in the form `name=value\0`
      .map(|(key, value)| format!("{key}={value}\0"))
      .collect::<Vec<_>>();

    // all strings in an environment block must be case insensitively
    // sorted alphabetically by name
    // https://docs.microsoft.com/en-us/windows/win32/procthread/changing-environment-variables
    parts.sort_by_key(|part| part.to_lowercase());

    // the entire block is terminated by NULL (\0)
    format!("{}\0", parts.join(""))
      .encode_utf16()
      .collect::<Vec<_>>()
  }
}
