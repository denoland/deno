use std::collections::HashMap;
use std::io::Read;
use std::path::Path;

pub trait Pty: Read {
  fn write_text(&mut self, text: &str);

  fn write_line(&mut self, text: &str) {
    self.write_text(&format!("{}\n", text));
  }

  /// Reads the output to the EOF.
  fn read_all_output(&mut self) -> String {
    let mut text = String::new();
    self.read_to_string(&mut text).unwrap();
    text
  }
}

#[cfg(unix)]
pub fn create_pty(
  program: impl AsRef<Path>,
  args: &[&str],
  cwd: impl AsRef<Path>,
  env_vars: Option<HashMap<String, String>>,
) -> Box<dyn Pty> {
  let fork = pty::fork::Fork::from_ptmx().unwrap();
  if fork.is_parent().is_ok() {
    Box::new(unix::UnixPty { fork })
  } else {
    std::process::Command::new(program.as_ref())
      .current_dir(cwd)
      .args(args)
      .envs(env_vars.unwrap_or_default())
      .spawn()
      .unwrap()
      .wait()
      .unwrap();
    unreachable!();
  }
}

#[cfg(unix)]
mod unix {
  use std::io::Read;
  use std::io::Write;

  use super::Pty;

  pub struct UnixPty {
    pub fork: pty::fork::Fork,
  }

  impl Drop for UnixPty {
    fn drop(&mut self) {
      self.fork.wait().unwrap();
    }
  }

  impl Pty for UnixPty {
    fn write_text(&mut self, text: &str) {
      let mut master = self.fork.is_parent().unwrap();
      master.write_all(text.as_bytes()).unwrap();
    }
  }

  impl Read for UnixPty {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
      let mut master = self.fork.is_parent().unwrap();
      master.read(buf)
    }
  }
}

#[cfg(target_os = "windows")]
pub fn create_pty(
  program: impl AsRef<Path>,
  args: &[&str],
  cwd: impl AsRef<Path>,
  env_vars: Option<HashMap<String, String>>,
) -> Box<dyn Pty> {
  let pty = windows::WinPseudoConsole::new(
    program,
    args,
    &cwd.as_ref().to_string_lossy().to_string(),
    env_vars,
  );
  Box::new(pty)
}

#[cfg(target_os = "windows")]
mod windows {
  use std::collections::HashMap;
  use std::io::Read;
  use std::io::Write;
  use std::path::Path;
  use std::ptr;
  use std::time::Duration;

  use winapi::shared::minwindef::FALSE;
  use winapi::shared::minwindef::LPVOID;
  use winapi::shared::minwindef::TRUE;
  use winapi::shared::winerror::S_OK;
  use winapi::um::consoleapi::ClosePseudoConsole;
  use winapi::um::consoleapi::CreatePseudoConsole;
  use winapi::um::fileapi::ReadFile;
  use winapi::um::fileapi::WriteFile;
  use winapi::um::handleapi::DuplicateHandle;
  use winapi::um::handleapi::INVALID_HANDLE_VALUE;
  use winapi::um::namedpipeapi::CreatePipe;
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

  use super::Pty;

  macro_rules! assert_win_success {
    ($expression:expr) => {
      let success = $expression;
      if success != TRUE {
        panic!("{}", std::io::Error::last_os_error().to_string())
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
      program: impl AsRef<Path>,
      args: &[&str],
      cwd: &str,
      maybe_env_vars: Option<HashMap<String, String>>,
    ) -> Self {
      // https://docs.microsoft.com/en-us/windows/console/creating-a-pseudoconsole-session
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
          program.as_ref().to_string_lossy(),
          args.join(" ")
        )
        .trim()
        .to_string();
        let mut application_str =
          to_windows_str(&program.as_ref().to_string_lossy());
        let mut command_str = to_windows_str(&command);
        let mut cwd = to_windows_str(cwd);

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
      unsafe {
        loop {
          let mut bytes_read = 0;
          let success = ReadFile(
            self.stdout_read_handle.as_raw_handle(),
            buf.as_mut_ptr() as _,
            buf.len() as u32,
            &mut bytes_read,
            ptr::null_mut(),
          );

          // ignore zero-byte writes
          let is_zero_byte_write = bytes_read == 0 && success == TRUE;
          if !is_zero_byte_write {
            return Ok(bytes_read as usize);
          }
        }
      }
    }
  }

  impl Pty for WinPseudoConsole {
    fn write_text(&mut self, text: &str) {
      // windows psuedo console requires a \r\n to do a newline
      let newline_re = regex::Regex::new("\r?\n").unwrap();
      self
        .write_all(newline_re.replace_all(text, "\r\n").as_bytes())
        .unwrap();
    }
  }

  impl std::io::Write for WinPseudoConsole {
    fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
      unsafe {
        let mut bytes_written = 0;
        assert_win_success!(WriteFile(
          self.stdin_write_handle.as_raw_handle(),
          buffer.as_ptr() as *const _,
          buffer.len() as u32,
          &mut bytes_written,
          ptr::null_mut(),
        ));
        Ok(bytes_written as usize)
      }
    }

    fn flush(&mut self) -> std::io::Result<()> {
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
      unsafe {
        let process_handle = GetCurrentProcess();
        let mut duplicate_handle = ptr::null_mut();
        assert_win_success!(DuplicateHandle(
          process_handle,
          self.inner,
          process_handle,
          &mut duplicate_handle,
          0,
          0,
          DUPLICATE_SAME_ACCESS,
        ));

        WinHandle::new(duplicate_handle)
      }
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

  unsafe impl Send for WinHandle {}
  unsafe impl Sync for WinHandle {}

  impl Drop for WinHandle {
    fn drop(&mut self) {
      unsafe {
        if !self.inner.is_null() && self.inner != INVALID_HANDLE_VALUE {
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
      unsafe { DeleteProcThreadAttributeList(self.as_mut_ptr()) };
    }
  }

  fn create_pipe() -> (WinHandle, WinHandle) {
    unsafe {
      let mut read_handle = std::ptr::null_mut();
      let mut write_handle = std::ptr::null_mut();

      assert_win_success!(CreatePipe(
        &mut read_handle,
        &mut write_handle,
        ptr::null_mut(),
        0
      ));

      (WinHandle::new(read_handle), WinHandle::new(write_handle))
    }
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
      .map(|(key, value)| format!("{}={}\0", key, value))
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
