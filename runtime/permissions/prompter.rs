// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use crate::colors;
use deno_core::error::AnyError;
use deno_core::parking_lot::Mutex;
use once_cell::sync::Lazy;

pub const PERMISSION_EMOJI: &str = "⚠️";

#[derive(Debug, Eq, PartialEq)]
pub enum PromptResponse {
  Allow,
  Deny,
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
) -> PromptResponse {
  if let Some(before_callback) = MAYBE_BEFORE_PROMPT_CALLBACK.lock().as_mut() {
    before_callback();
  }
  let r = PERMISSION_PROMPTER.lock().prompt(message, flag, api_name);
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
  ) -> PromptResponse;
}

pub struct TtyPrompter;

impl PermissionPrompter for TtyPrompter {
  fn prompt(
    &mut self,
    message: &str,
    name: &str,
    api_name: Option<&str>,
  ) -> PromptResponse {
    if !atty::is(atty::Stream::Stdin) || !atty::is(atty::Stream::Stderr) {
      return PromptResponse::Deny;
    };

    #[cfg(unix)]
    fn clear_stdin() -> Result<(), AnyError> {
      // TODO(bartlomieju):
      #[allow(clippy::undocumented_unsafe_blocks)]
      let r = unsafe { libc::tcflush(0, libc::TCIFLUSH) };
      assert_eq!(r, 0);
      Ok(())
    }

    #[cfg(not(unix))]
    fn clear_stdin() -> Result<(), AnyError> {
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
        read_stdin_line()?;
        // check if our emulated key press was executed
        if is_input_buffer_empty(stdin)? {
          // if so, move the cursor up to prevent a blank line
          move_cursor_up()?;
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

      fn move_cursor_up() -> Result<(), AnyError> {
        use std::io::Write;
        write!(std::io::stderr(), "\x1B[1A")?;
        Ok(())
      }

      fn read_stdin_line() -> Result<(), AnyError> {
        let mut input = String::new();
        let stdin = std::io::stdin();
        stdin.read_line(&mut input)?;
        Ok(())
      }
    }

    // Clear n-lines in terminal and move cursor to the beginning of the line.
    fn clear_n_lines(n: usize) {
      eprint!("\x1B[{}A\x1B[0J", n);
    }

    // For security reasons we must consume everything in stdin so that previously
    // buffered data cannot effect the prompt.
    if let Err(err) = clear_stdin() {
      eprintln!("Error clearing stdin for permission prompt. {:#}", err);
      return PromptResponse::Deny; // don't grant permission if this fails
    }

    // print to stderr so that if stdout is piped this is still displayed.
    const OPTS: &str = "[y/n] (y = yes, allow; n = no, deny)";
    eprint!("{}  ┌ ", PERMISSION_EMOJI);
    eprint!("{}", colors::bold("Deno requests "));
    eprint!("{}", colors::bold(message));
    eprintln!("{}", colors::bold("."));
    if let Some(api_name) = api_name {
      eprintln!("   ├ Requested by `{}` API", api_name);
    }
    let msg = format!("Run again with --allow-{} to bypass this prompt.", name);
    eprintln!("   ├ {}", colors::italic(&msg));
    eprint!("   └ {}", colors::bold("Allow?"));
    eprint!(" {} > ", OPTS);
    let value = loop {
      let mut input = String::new();
      let stdin = std::io::stdin();
      let result = stdin.read_line(&mut input);
      if result.is_err() {
        break PromptResponse::Deny;
      };
      let ch = match input.chars().next() {
        None => break PromptResponse::Deny,
        Some(v) => v,
      };
      match ch.to_ascii_lowercase() {
        'y' => {
          clear_n_lines(if api_name.is_some() { 4 } else { 3 });
          let msg = format!("Granted {}.", message);
          eprintln!("✅ {}", colors::bold(&msg));
          break PromptResponse::Allow;
        }
        'n' => {
          clear_n_lines(if api_name.is_some() { 4 } else { 3 });
          let msg = format!("Denied {}.", message);
          eprintln!("❌ {}", colors::bold(&msg));
          break PromptResponse::Deny;
        }
        _ => {
          // If we don't get a recognized option try again.
          clear_n_lines(1);
          eprint!("   └ {}", colors::bold("Unrecognized option. Allow?"));
          eprint!(" {} > ", OPTS);
        }
      };
    };

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
