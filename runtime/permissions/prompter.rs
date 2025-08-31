// Copyright 2018-2025 the Deno authors. MIT license.

use once_cell::sync::Lazy;
use parking_lot::Mutex;

/// Helper function to make control characters visible so users can see the underlying filename.
#[cfg(not(target_arch = "wasm32"))]
fn escape_control_characters(s: &str) -> std::borrow::Cow<'_, str> {
  use deno_terminal::colors;

  if !s.contains(|c: char| c.is_ascii_control() || c.is_control()) {
    return std::borrow::Cow::Borrowed(s);
  }
  let mut output = String::with_capacity(s.len() * 2);
  for c in s.chars() {
    match c {
      c if c.is_ascii_control() => output.push_str(
        &colors::white_bold_on_red(c.escape_debug().to_string()).to_string(),
      ),
      c if c.is_control() => output.push_str(
        &colors::white_bold_on_red(c.escape_debug().to_string()).to_string(),
      ),
      c => output.push(c),
    }
  }
  output.into()
}

pub const PERMISSION_EMOJI: &str = "⚠️";

#[derive(Debug, Eq, PartialEq)]
pub enum PromptResponse {
  Allow,
  Deny,
  AllowAll,
}

#[cfg(not(target_arch = "wasm32"))]
type DefaultPrompter = TtyPrompter;
#[cfg(target_arch = "wasm32")]
type DefaultPrompter = DeniedPrompter;

static PERMISSION_PROMPTER: Lazy<Mutex<Box<dyn PermissionPrompter>>> =
  Lazy::new(|| Mutex::new(Box::new(DefaultPrompter::default())));

static MAYBE_BEFORE_PROMPT_CALLBACK: Lazy<Mutex<Option<PromptCallback>>> =
  Lazy::new(|| Mutex::new(None));

static MAYBE_AFTER_PROMPT_CALLBACK: Lazy<Mutex<Option<PromptCallback>>> =
  Lazy::new(|| Mutex::new(None));

pub(crate) static MAYBE_CURRENT_STACKTRACE: Lazy<
  Mutex<Option<GetFormattedStackFn>>,
> = Lazy::new(|| Mutex::new(None));

pub fn set_current_stacktrace(get_stack: GetFormattedStackFn) {
  *MAYBE_CURRENT_STACKTRACE.lock() = Some(get_stack);
}

pub fn permission_prompt(
  message: &str,
  flag: &str,
  api_name: Option<&str>,
  is_unary: bool,
) -> PromptResponse {
  if let Some(before_callback) = MAYBE_BEFORE_PROMPT_CALLBACK.lock().as_mut() {
    before_callback();
  }
  let stack = MAYBE_CURRENT_STACKTRACE.lock().take();
  let r = PERMISSION_PROMPTER
    .lock()
    .prompt(message, flag, api_name, is_unary, stack);
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

pub fn set_prompter(prompter: Box<dyn PermissionPrompter>) {
  *PERMISSION_PROMPTER.lock() = prompter;
}

pub type PromptCallback = Box<dyn FnMut() + Send + Sync>;

pub type GetFormattedStackFn = Box<dyn Fn() -> Vec<String> + Send + Sync>;

pub trait PermissionPrompter: Send + Sync {
  fn prompt(
    &mut self,
    message: &str,
    name: &str,
    api_name: Option<&str>,
    is_unary: bool,
    get_stack: Option<GetFormattedStackFn>,
  ) -> PromptResponse;
}

#[derive(Default)]
pub struct DeniedPrompter;

impl PermissionPrompter for DeniedPrompter {
  fn prompt(
    &mut self,
    _message: &str,
    _name: &str,
    _api_name: Option<&str>,
    _is_unary: bool,
    _get_stack: Option<GetFormattedStackFn>,
  ) -> PromptResponse {
    PromptResponse::Deny
  }
}

#[cfg(unix)]
fn clear_stdin(
  _stdin_lock: &mut std::io::StdinLock,
  _stderr_lock: &mut std::io::StderrLock,
) -> Result<(), std::io::Error> {
  use std::mem::MaybeUninit;

  const STDIN_FD: i32 = 0;

  // SAFETY: use libc to flush stdin
  unsafe {
    // Create fd_set for select
    let mut raw_fd_set = MaybeUninit::<libc::fd_set>::uninit();
    libc::FD_ZERO(raw_fd_set.as_mut_ptr());
    libc::FD_SET(STDIN_FD, raw_fd_set.as_mut_ptr());

    loop {
      let r = libc::tcflush(STDIN_FD, libc::TCIFLUSH);
      if r != 0 {
        return Err(std::io::Error::other("clear_stdin failed (tcflush)"));
      }

      // Initialize timeout for select to be 100ms
      let mut timeout = libc::timeval {
        tv_sec: 0,
        tv_usec: 100_000,
      };

      // Call select with the stdin file descriptor set
      let r = libc::select(
        STDIN_FD + 1, // nfds should be set to the highest-numbered file descriptor in any of the three sets, plus 1.
        raw_fd_set.as_mut_ptr(),
        std::ptr::null_mut(),
        std::ptr::null_mut(),
        &mut timeout,
      );

      // Check if select returned an error
      if r < 0 {
        return Err(std::io::Error::other("clear_stdin failed (select)"));
      }

      // Check if select returned due to timeout (stdin is quiescent)
      if r == 0 {
        break; // Break out of the loop as stdin is quiescent
      }

      // If select returned due to data available on stdin, clear it by looping around to flush
    }
  }

  Ok(())
}

#[cfg(all(not(unix), not(target_arch = "wasm32")))]
fn clear_stdin(
  stdin_lock: &mut std::io::StdinLock,
  stderr_lock: &mut std::io::StderrLock,
) -> Result<(), std::io::Error> {
  use std::io::BufRead;
  use std::io::StdinLock;
  use std::io::Write as IoWrite;

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
  use winapi::um::winuser::MAPVK_VK_TO_VSC;
  use winapi::um::winuser::MapVirtualKeyW;
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

  unsafe fn flush_input_buffer(stdin: HANDLE) -> Result<(), std::io::Error> {
    // SAFETY: winapi calls
    let success = unsafe { FlushConsoleInputBuffer(stdin) };
    if success != TRUE {
      return Err(std::io::Error::other(format!(
        "Could not flush the console input buffer: {}",
        std::io::Error::last_os_error()
      )));
    }
    Ok(())
  }

  unsafe fn emulate_enter_key_press(
    stdin: HANDLE,
  ) -> Result<(), std::io::Error> {
    // SAFETY: winapi calls
    unsafe {
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
        return Err(std::io::Error::other(format!(
          "Could not emulate enter key press: {}",
          std::io::Error::last_os_error()
        )));
      }
    }
    Ok(())
  }

  unsafe fn is_input_buffer_empty(
    stdin: HANDLE,
  ) -> Result<bool, std::io::Error> {
    let mut buffer = Vec::with_capacity(1);
    let mut events_read = 0;
    // SAFETY: winapi calls
    let success = unsafe {
      PeekConsoleInputW(stdin, buffer.as_mut_ptr(), 1, &mut events_read)
    };
    if success != TRUE {
      return Err(std::io::Error::other(format!(
        "Could not peek the console input buffer: {}",
        std::io::Error::last_os_error()
      )));
    }
    Ok(events_read == 0)
  }

  fn move_cursor_up(
    stderr_lock: &mut std::io::StderrLock,
  ) -> Result<(), std::io::Error> {
    write!(stderr_lock, "\x1B[1A")
  }

  fn read_stdin_line(stdin_lock: &mut StdinLock) -> Result<(), std::io::Error> {
    let mut input = String::new();
    stdin_lock.read_line(&mut input)?;
    Ok(())
  }
}

// Clear n-lines in terminal and move cursor to the beginning of the line.
#[cfg(not(target_arch = "wasm32"))]
fn clear_n_lines(stderr_lock: &mut std::io::StderrLock, n: usize) {
  use std::io::Write;
  write!(stderr_lock, "\x1B[{n}A\x1B[0J").unwrap();
}

#[cfg(unix)]
fn get_stdin_metadata() -> std::io::Result<std::fs::Metadata> {
  use std::os::fd::FromRawFd;
  use std::os::fd::IntoRawFd;

  // SAFETY: we don't know if fd 0 is valid but metadata() will return an error in this case (bad file descriptor)
  // and we can panic.
  unsafe {
    let stdin = std::fs::File::from_raw_fd(0);
    let metadata = stdin.metadata().unwrap();
    let _ = stdin.into_raw_fd();
    Ok(metadata)
  }
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Default)]
pub struct TtyPrompter;

#[cfg(not(target_arch = "wasm32"))]
impl PermissionPrompter for TtyPrompter {
  fn prompt(
    &mut self,
    message: &str,
    name: &str,
    api_name: Option<&str>,
    is_unary: bool,
    get_stack: Option<GetFormattedStackFn>,
  ) -> PromptResponse {
    use std::fmt::Write;
    use std::io::BufRead;
    use std::io::IsTerminal;
    use std::io::Write as IoWrite;

    use deno_terminal::colors;

    // 10kB of permission prompting should be enough for anyone
    const MAX_PERMISSION_PROMPT_LENGTH: usize = 10 * 1024;

    if !std::io::stdin().is_terminal() || !std::io::stderr().is_terminal() {
      return PromptResponse::Deny;
    };

    #[allow(clippy::print_stderr)]
    if message.len() > MAX_PERMISSION_PROMPT_LENGTH {
      eprintln!(
        "❌ Permission prompt length ({} bytes) was larger than the configured maximum length ({} bytes): denying request.",
        message.len(),
        MAX_PERMISSION_PROMPT_LENGTH
      );
      eprintln!(
        "❌ WARNING: This may indicate that code is trying to bypass or hide permission check requests."
      );
      eprintln!(
        "❌ Run again with --allow-{name} to bypass this check if this is really what you want to do."
      );
      return PromptResponse::Deny;
    }

    #[cfg(unix)]
    let metadata_before = get_stdin_metadata().unwrap();

    // Lock stdio streams, so no other output is written while the prompt is
    // displayed.
    let stdout_lock = std::io::stdout().lock();
    let mut stderr_lock = std::io::stderr().lock();
    let mut stdin_lock = std::io::stdin().lock();

    // For security reasons we must consume everything in stdin so that previously
    // buffered data cannot affect the prompt.
    #[allow(clippy::print_stderr)]
    if let Err(err) = clear_stdin(&mut stdin_lock, &mut stderr_lock) {
      eprintln!("Error clearing stdin for permission prompt. {err:#}");
      return PromptResponse::Deny; // don't grant permission if this fails
    }

    let message = escape_control_characters(message);
    let name = escape_control_characters(name);
    let api_name = api_name.map(escape_control_characters);

    // print to stderr so that if stdout is piped this is still displayed.
    let opts: String = if is_unary {
      format!(
        "[y/n/A] (y = yes, allow; n = no, deny; A = allow all {name} permissions)"
      )
    } else {
      "[y/n] (y = yes, allow; n = no, deny)".to_string()
    };

    // output everything in one shot to make the tests more reliable
    let stack_lines_count = {
      let mut output = String::new();
      write!(&mut output, "┏ {PERMISSION_EMOJI}  ").unwrap();
      write!(&mut output, "{}", colors::bold("Deno requests ")).unwrap();
      write!(&mut output, "{}", colors::bold(message.clone())).unwrap();
      writeln!(&mut output, "{}", colors::bold(".")).unwrap();
      if let Some(api_name) = api_name.clone() {
        writeln!(
          &mut output,
          "┠─ Requested by `{}` API.",
          colors::bold(api_name)
        )
        .unwrap();
      }
      let stack_lines_count = if let Some(get_stack) = get_stack {
        let stack = get_stack();
        let len = stack.len();
        for (idx, frame) in stack.into_iter().enumerate() {
          writeln!(
            &mut output,
            "┃  {} {}",
            colors::gray(if idx != len - 1 { "├─" } else { "└─" }),
            colors::gray(frame),
          )
          .unwrap();
        }
        len
      } else {
        writeln!(
          &mut output,
          "┠─ To see a stack trace for this prompt, set the DENO_TRACE_PERMISSIONS environmental variable.",
        ).unwrap();
        1
      };
      let msg = format!(
        "Learn more at: {}",
        colors::cyan_with_underline(&format!(
          "https://docs.deno.com/go/--allow-{}",
          name
        ))
      );
      writeln!(&mut output, "┠─ {}", colors::italic(&msg)).unwrap();
      let msg = if crate::is_standalone() {
        format!(
          "Specify the required permissions during compile time using `deno compile --allow-{name}`."
        )
      } else {
        format!("Run again with --allow-{name} to bypass this prompt.")
      };
      writeln!(&mut output, "┠─ {}", colors::italic(&msg)).unwrap();
      write!(&mut output, "┗ {}", colors::bold("Allow?")).unwrap();
      write!(&mut output, " {opts} > ").unwrap();

      stderr_lock.write_all(output.as_bytes()).unwrap();

      stack_lines_count
    };

    let value = loop {
      // Clear stdin each time we loop around in case the user accidentally pasted
      // multiple lines or otherwise did something silly to generate a torrent of
      // input. This doesn't work on Windows because `clear_stdin` has other side-effects.
      #[allow(clippy::print_stderr)]
      #[cfg(unix)]
      if let Err(err) = clear_stdin(&mut stdin_lock, &mut stderr_lock) {
        eprintln!("Error clearing stdin for permission prompt. {err:#}");
        return PromptResponse::Deny; // don't grant permission if this fails
      }

      let mut input = String::new();
      let result = stdin_lock.read_line(&mut input);
      let input = input.trim_end_matches(['\r', '\n']);
      if result.is_err() || input.len() != 1 {
        break PromptResponse::Deny;
      };

      let clear_n = if api_name.is_some() { 5 } else { 4 } + stack_lines_count;

      match input.as_bytes()[0] as char {
        'y' | 'Y' => {
          clear_n_lines(&mut stderr_lock, clear_n);
          let msg = format!("Granted {message}.");
          writeln!(stderr_lock, "✅ {}", colors::bold(&msg)).unwrap();
          break PromptResponse::Allow;
        }
        'n' | 'N' | '\x1b' => {
          clear_n_lines(&mut stderr_lock, clear_n);
          let msg = format!("Denied {message}.");
          writeln!(stderr_lock, "❌ {}", colors::bold(&msg)).unwrap();
          break PromptResponse::Deny;
        }
        'A' if is_unary => {
          clear_n_lines(&mut stderr_lock, clear_n);
          let msg = format!("Granted all {name} access.");
          writeln!(stderr_lock, "✅ {}", colors::bold(&msg)).unwrap();
          break PromptResponse::AllowAll;
        }
        _ => {
          // If we don't get a recognized option try again.
          clear_n_lines(&mut stderr_lock, 1);
          write!(
            stderr_lock,
            "┗ {} {opts} > ",
            colors::bold("Unrecognized option. Allow?")
          )
          .unwrap();
        }
      };
    };

    drop(stdout_lock);
    drop(stderr_lock);
    drop(stdin_lock);

    // Ensure that stdin has not changed from the beginning to the end of the prompt. We consider
    // it sufficient to check a subset of stat calls. We do not consider the likelihood of a stdin
    // swap attack on Windows to be high enough to add this check for that platform. These checks will
    // terminate the runtime as they indicate something nefarious is going on.
    #[cfg(unix)]
    {
      use std::os::unix::fs::MetadataExt;
      let metadata_after = get_stdin_metadata().unwrap();

      assert_eq!(metadata_before.dev(), metadata_after.dev());
      assert_eq!(metadata_before.ino(), metadata_after.ino());
      assert_eq!(metadata_before.rdev(), metadata_after.rdev());
      assert_eq!(metadata_before.uid(), metadata_after.uid());
      assert_eq!(metadata_before.gid(), metadata_after.gid());
      assert_eq!(metadata_before.mode(), metadata_after.mode());
    }

    // Ensure that stdin and stderr are still terminals before we yield the response.
    assert!(std::io::stdin().is_terminal() && std::io::stderr().is_terminal());

    value
  }
}

#[cfg(test)]
pub mod tests {
  use std::sync::atomic::AtomicBool;
  use std::sync::atomic::Ordering;

  use super::*;

  pub struct TestPrompter;

  impl PermissionPrompter for TestPrompter {
    fn prompt(
      &mut self,
      _message: &str,
      _name: &str,
      _api_name: Option<&str>,
      _is_unary: bool,
      _get_stack: Option<GetFormattedStackFn>,
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
}
