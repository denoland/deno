// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use crate::colors;
use deno_core::error::AnyError;
use deno_core::parking_lot::Mutex;
use once_cell::sync::Lazy;
use std::fmt::Write;
use std::io::BufRead;
use std::io::IsTerminal;
use std::io::StderrLock;
use std::io::StdinLock;
use std::io::Write as IoWrite;

/// Helper function to strip ansi codes and ASCII control characters.
fn strip_ansi_codes_and_ascii_control(s: &str) -> std::borrow::Cow<str> {
  console_static_text::ansi::strip_ansi_codes(s)
    .chars()
    .filter(|c| !c.is_ascii_control())
    .collect()
}

pub const PERMISSION_EMOJI: &str = "⚠️";

#[derive(Debug, Eq, PartialEq)]
pub enum PromptResponse {
  Allow,
  Deny,
  AllowAll,
}

static PERMISSION_PROMPTER: Lazy<Mutex<Box<dyn PermissionPrompter>>> =
  Lazy::new(|| Mutex::new(Box::new(TtyPrompter)));

static MAYBE_BEFORE_PROMPT_CALLBACK: Lazy<Mutex<Option<PromptCallback>>> =
  Lazy::new(|| Mutex::new(None));

static MAYBE_AFTER_PROMPT_CALLBACK: Lazy<Mutex<Option<PromptCallback>>> =
  Lazy::new(|| Mutex::new(None));

pub fn permission_prompt(
  message: &str,
  flag: &str,
  api_name: Option<&str>,
  is_unary: bool,
) -> PromptResponse {
  if let Some(before_callback) = MAYBE_BEFORE_PROMPT_CALLBACK.lock().as_mut() {
    before_callback();
  }
  let r = PERMISSION_PROMPTER
    .lock()
    .prompt(message, flag, api_name, is_unary);
  if let Some(after_callback) = MAYBE_AFTER_PROMPT_CALLBACK.lock().as_mut() {
    after_callback();
  }
  r
}

pub fn set_prompt_callbacks(
  before_callback: PromptCallback,
  after_callback: PromptCallback,
) {
  *MAYBE_BEFORE_PROMPT_CALLBACK.lock() = Some(before_callback);
  *MAYBE_AFTER_PROMPT_CALLBACK.lock() = Some(after_callback);
}

pub type PromptCallback = Box<dyn FnMut() + Send + Sync>;

pub trait PermissionPrompter: Send + Sync {
  fn prompt(
    &mut self,
    message: &str,
    name: &str,
    api_name: Option<&str>,
    is_unary: bool,
  ) -> PromptResponse;
}

pub struct TtyPrompter;

impl PermissionPrompter for TtyPrompter {
  fn prompt(
    &mut self,
    message: &str,
    name: &str,
    api_name: Option<&str>,
    is_unary: bool,
  ) -> PromptResponse {
    if !std::io::stdin().is_terminal() || !std::io::stderr().is_terminal() {
      return PromptResponse::Deny;
    };

    #[cfg(unix)]
    fn clear_stdin(
      _stdin_lock: &mut StdinLock,
      _stderr_lock: &mut StderrLock,
    ) -> Result<(), AnyError> {
      // TODO(bartlomieju):
      #[allow(clippy::undocumented_unsafe_blocks)]
      let r = unsafe { libc::tcflush(0, libc::TCIFLUSH) };
      assert_eq!(r, 0);
      Ok(())
    }

    #[cfg(not(unix))]
    fn clear_stdin(
      stdin_lock: &mut StdinLock,
      stderr_lock: &mut StderrLock,
    ) -> Result<(), AnyError> {
      use deno_core::anyhow::bail;
      use winapi::shared::minwindef::TRUE;
      use winapi::shared::minwindef::UINT;
      use winapi::shared::minwindef::WORD;
      use winapi::shared::ntdef::WCHAR;
      use winapi::um::processenv::GetStdHandle;
      use winapi::um::winbase::STD_INPUT_HANDLE;
      use winapi::um::wincon::FlushConsoleInputBuffer;
      use winapi::um::wincon::PeekConsoleInputW;
      use winapi::um::wincon::WriteConsoleInputW;
      use winapi::um::wincontypes::INPUT_RECORD;
      use winapi::um::wincontypes::KEY_EVENT;
      use winapi::um::winnt::HANDLE;
      use winapi::um::winuser::MapVirtualKeyW;
      use winapi::um::winuser::MAPVK_VK_TO_VSC;
      use winapi::um::winuser::VK_RETURN;

      // SAFETY: winapi calls
      unsafe {
        let stdin = GetStdHandle(STD_INPUT_HANDLE);
        // emulate an enter key press to clear any line buffered console characters
        emulate_enter_key_press(stdin)?;
        // read the buffered line or enter key press
        read_stdin_line(stdin_lock)?;
        // check if our emulated key press was executed
        if is_input_buffer_empty(stdin)? {
          // if so, move the cursor up to prevent a blank line
          move_cursor_up(stderr_lock)?;
        } else {
          // the emulated key press is still pending, so a buffered line was read
          // and we can flush the emulated key press
          flush_input_buffer(stdin)?;
        }
      }

      return Ok(());

      unsafe fn flush_input_buffer(stdin: HANDLE) -> Result<(), AnyError> {
        let success = FlushConsoleInputBuffer(stdin);
        if success != TRUE {
          bail!(
            "Could not flush the console input buffer: {}",
            std::io::Error::last_os_error()
          )
        }
        Ok(())
      }

      unsafe fn emulate_enter_key_press(stdin: HANDLE) -> Result<(), AnyError> {
        // https://github.com/libuv/libuv/blob/a39009a5a9252a566ca0704d02df8dabc4ce328f/src/win/tty.c#L1121-L1131
        let mut input_record: INPUT_RECORD = std::mem::zeroed();
        input_record.EventType = KEY_EVENT;
        input_record.Event.KeyEvent_mut().bKeyDown = TRUE;
        input_record.Event.KeyEvent_mut().wRepeatCount = 1;
        input_record.Event.KeyEvent_mut().wVirtualKeyCode = VK_RETURN as WORD;
        input_record.Event.KeyEvent_mut().wVirtualScanCode =
          MapVirtualKeyW(VK_RETURN as UINT, MAPVK_VK_TO_VSC) as WORD;
        *input_record.Event.KeyEvent_mut().uChar.UnicodeChar_mut() =
          '\r' as WCHAR;

        let mut record_written = 0;
        let success =
          WriteConsoleInputW(stdin, &input_record, 1, &mut record_written);
        if success != TRUE {
          bail!(
            "Could not emulate enter key press: {}",
            std::io::Error::last_os_error()
          );
        }
        Ok(())
      }

      unsafe fn is_input_buffer_empty(stdin: HANDLE) -> Result<bool, AnyError> {
        let mut buffer = Vec::with_capacity(1);
        let mut events_read = 0;
        let success =
          PeekConsoleInputW(stdin, buffer.as_mut_ptr(), 1, &mut events_read);
        if success != TRUE {
          bail!(
            "Could not peek the console input buffer: {}",
            std::io::Error::last_os_error()
          )
        }
        Ok(events_read == 0)
      }

      fn move_cursor_up(stderr_lock: &mut StderrLock) -> Result<(), AnyError> {
        write!(stderr_lock, "\x1B[1A")?;
        Ok(())
      }

      fn read_stdin_line(stdin_lock: &mut StdinLock) -> Result<(), AnyError> {
        let mut input = String::new();
        stdin_lock.read_line(&mut input)?;
        Ok(())
      }
    }

    // Clear n-lines in terminal and move cursor to the beginning of the line.
    fn clear_n_lines(stderr_lock: &mut StderrLock, n: usize) {
      write!(stderr_lock, "\x1B[{n}A\x1B[0J").unwrap();
    }

    // Lock stdio streams, so no other output is written while the prompt is
    // displayed.
    let stdout_lock = std::io::stdout().lock();
    let mut stderr_lock = std::io::stderr().lock();
    let mut stdin_lock = std::io::stdin().lock();

    // For security reasons we must consume everything in stdin so that previously
    // buffered data cannot affect the prompt.
    if let Err(err) = clear_stdin(&mut stdin_lock, &mut stderr_lock) {
      eprintln!("Error clearing stdin for permission prompt. {err:#}");
      return PromptResponse::Deny; // don't grant permission if this fails
    }

    let message = strip_ansi_codes_and_ascii_control(message);
    let name = strip_ansi_codes_and_ascii_control(name);
    let api_name = api_name.map(strip_ansi_codes_and_ascii_control);

    // print to stderr so that if stdout is piped this is still displayed.
    let opts: String = if is_unary {
      format!("[y/n/A] (y = yes, allow; n = no, deny; A = allow all {name} permissions)")
    } else {
      "[y/n] (y = yes, allow; n = no, deny)".to_string()
    };

    // output everything in one shot to make the tests more reliable
    {
      let mut output = String::new();
      write!(&mut output, "┌ {PERMISSION_EMOJI}  ").unwrap();
      write!(&mut output, "{}", colors::bold("Deno requests ")).unwrap();
      write!(&mut output, "{}", colors::bold(message.clone())).unwrap();
      writeln!(&mut output, "{}", colors::bold(".")).unwrap();
      if let Some(api_name) = api_name.clone() {
        writeln!(&mut output, "├ Requested by `{api_name}` API.").unwrap();
      }
      let msg = format!("Run again with --allow-{name} to bypass this prompt.");
      writeln!(&mut output, "├ {}", colors::italic(&msg)).unwrap();
      write!(&mut output, "└ {}", colors::bold("Allow?")).unwrap();
      write!(&mut output, " {opts} > ").unwrap();

      stderr_lock.write_all(output.as_bytes()).unwrap();
    }

    let value = loop {
      let mut input = String::new();
      let result = stdin_lock.read_line(&mut input);
      if result.is_err() {
        break PromptResponse::Deny;
      };
      let ch = match input.chars().next() {
        None => break PromptResponse::Deny,
        Some(v) => v,
      };
      match ch {
        'y' | 'Y' => {
          clear_n_lines(
            &mut stderr_lock,
            if api_name.is_some() { 4 } else { 3 },
          );
          let msg = format!("Granted {message}.");
          writeln!(stderr_lock, "✅ {}", colors::bold(&msg)).unwrap();
          break PromptResponse::Allow;
        }
        'n' | 'N' => {
          clear_n_lines(
            &mut stderr_lock,
            if api_name.is_some() { 4 } else { 3 },
          );
          let msg = format!("Denied {message}.");
          writeln!(stderr_lock, "❌ {}", colors::bold(&msg)).unwrap();
          break PromptResponse::Deny;
        }
        'A' if is_unary => {
          clear_n_lines(
            &mut stderr_lock,
            if api_name.is_some() { 4 } else { 3 },
          );
          let msg = format!("Granted all {name} access.");
          writeln!(stderr_lock, "✅ {}", colors::bold(&msg)).unwrap();
          break PromptResponse::AllowAll;
        }
        _ => {
          // If we don't get a recognized option try again.
          clear_n_lines(&mut stderr_lock, 1);
          write!(
            stderr_lock,
            "└ {} {opts} > ",
            colors::bold("Unrecognized option. Allow?")
          )
          .unwrap();
        }
      };
    };

    drop(stdout_lock);
    drop(stderr_lock);
    drop(stdin_lock);

    value
  }
}

#[cfg(test)]
pub mod tests {
  use super::*;
  use std::sync::atomic::AtomicBool;
  use std::sync::atomic::Ordering;

  pub struct TestPrompter;

  impl PermissionPrompter for TestPrompter {
    fn prompt(
      &mut self,
      _message: &str,
      _name: &str,
      _api_name: Option<&str>,
      _is_unary: bool,
    ) -> PromptResponse {
      if STUB_PROMPT_VALUE.load(Ordering::SeqCst) {
        PromptResponse::Allow
      } else {
        PromptResponse::Deny
      }
    }
  }

  static STUB_PROMPT_VALUE: AtomicBool = AtomicBool::new(true);

  pub static PERMISSION_PROMPT_STUB_VALUE_SETTER: Lazy<
    Mutex<PermissionPromptStubValueSetter>,
  > = Lazy::new(|| Mutex::new(PermissionPromptStubValueSetter));

  pub struct PermissionPromptStubValueSetter;

  impl PermissionPromptStubValueSetter {
    pub fn set(&self, value: bool) {
      STUB_PROMPT_VALUE.store(value, Ordering::SeqCst);
    }
  }

  pub fn set_prompter(prompter: Box<dyn PermissionPrompter>) {
    *PERMISSION_PROMPTER.lock() = prompter;
  }
}
