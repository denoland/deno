// Copyright 2018-2026 the Deno authors. MIT license.

use once_cell::sync::Lazy;
use parking_lot::Mutex;

/// Helper function to make control characters visible so users can see the underlying filename.
#[cfg(not(target_arch = "wasm32"))]
fn escape_control_characters(s: &str) -> std::borrow::Cow<'_, str> {
  use deno_terminal::colors;

  if !s.contains(is_prompt_control_character) {
    return std::borrow::Cow::Borrowed(s);
  }
  let mut output = String::with_capacity(s.len() * 2);
  for c in s.chars() {
    match c {
      c if is_prompt_control_character(c) => output.push_str(
        &colors::white_bold_on_red(c.escape_debug().to_string()).to_string(),
      ),
      c => output.push(c),
    }
  }
  output.into()
}

#[cfg(not(target_arch = "wasm32"))]
fn is_prompt_control_character(c: char) -> bool {
  c.is_ascii_control()
    || c.is_control()
    // Unicode formatting controls that can spoof permission prompt text. These
    // are General_Category=Format, not `char::is_control()`, so they pass
    // through unescaped unless we handle them explicitly. A few have legitimate
    // uses (e.g. joiners in some scripts and emoji), but in a security-sensitive
    // prompt we prefer to render them visibly so the label can't be forged.
    || matches!(
      c,
      // Bidirectional formatting controls. Terminals may interpret these and
      // visually reorder the text (the Trojan-Source / bidi-spoofing vector).
      '\u{061c}' // Arabic Letter Mark
        | '\u{200e}' // Left-to-Right Mark
        | '\u{200f}' // Right-to-Left Mark
        | '\u{202a}' // Left-to-Right Embedding
        | '\u{202b}' // Right-to-Left Embedding
        | '\u{202c}' // Pop Directional Formatting
        | '\u{202d}' // Left-to-Right Override
        | '\u{202e}' // Right-to-Left Override
        | '\u{2066}' // Left-to-Right Isolate
        | '\u{2067}' // Right-to-Left Isolate
        | '\u{2068}' // First Strong Isolate
        | '\u{2069}' // Pop Directional Isolate
      // Invisible / zero-width formatting controls. These render as nothing but
      // can conceal or fake label content without reordering it.
        | '\u{200b}' // Zero Width Space
        | '\u{200c}' // Zero Width Non-Joiner
        | '\u{200d}' // Zero Width Joiner
        | '\u{2060}' // Word Joiner
        | '\u{2061}' // Function Application
        | '\u{2062}' // Invisible Times
        | '\u{2063}' // Invisible Separator
        | '\u{2064}' // Invisible Plus
        | '\u{feff}' // Zero Width No-Break Space (byte order mark)
    )
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

static TERMINAL_INPUT_LOCK: std::sync::OnceLock<std::sync::Mutex<()>> =
  std::sync::OnceLock::new();

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

/// Guards direct reads from the process terminal input.
///
/// Permission prompts also read from stdin directly. Holding this lock around
/// other terminal reads prevents permission prompts from racing with
/// user-space interactive prompts and consuming or flushing their input.
pub fn lock_terminal_input() -> std::sync::MutexGuard<'static, ()> {
  TERMINAL_INPUT_LOCK
    .get_or_init(Default::default)
    .lock()
    .unwrap_or_else(|err| err.into_inner())
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

  use windows_sys::Win32::Foundation::HANDLE;
  use windows_sys::Win32::Foundation::TRUE;
  use windows_sys::Win32::System::Console::FlushConsoleInputBuffer;
  use windows_sys::Win32::System::Console::GetStdHandle;
  use windows_sys::Win32::System::Console::INPUT_RECORD;
  use windows_sys::Win32::System::Console::KEY_EVENT;
  use windows_sys::Win32::System::Console::PeekConsoleInputW;
  use windows_sys::Win32::System::Console::STD_INPUT_HANDLE;
  use windows_sys::Win32::System::Console::WriteConsoleInputW;
  use windows_sys::Win32::UI::Input::KeyboardAndMouse::MAPVK_VK_TO_VSC;
  use windows_sys::Win32::UI::Input::KeyboardAndMouse::MapVirtualKeyW;
  use windows_sys::Win32::UI::Input::KeyboardAndMouse::VK_RETURN;

  // SAFETY: Win32 calls
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
    // SAFETY: Win32 calls
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
    // SAFETY: Win32 calls
    unsafe {
      // https://github.com/libuv/libuv/blob/a39009a5a9252a566ca0704d02df8dabc4ce328f/src/win/tty.c#L1121-L1131
      let mut input_record: INPUT_RECORD = std::mem::zeroed();
      input_record.EventType = KEY_EVENT as u16;
      input_record.Event.KeyEvent.bKeyDown = TRUE;
      input_record.Event.KeyEvent.wRepeatCount = 1;
      input_record.Event.KeyEvent.wVirtualKeyCode = VK_RETURN;
      input_record.Event.KeyEvent.wVirtualScanCode =
        MapVirtualKeyW(VK_RETURN as u32, MAPVK_VK_TO_VSC) as u16;
      input_record.Event.KeyEvent.uChar.UnicodeChar = '\r' as u16;

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
    // SAFETY: Win32 calls
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

/// Returns true if stdin's terminal line discipline has been put into raw
/// mode by something else in this process — for example a Node.js library
/// calling `process.stdin.setRawMode(true)`.
///
/// When that has happened our line-oriented `read_line()` prompt loop would
/// hang forever (Enter delivers `\r` rather than `\n`, and ECHO is off so the
/// user can't see they're typing), so we bail out instead.
///
/// We require *both* canonical input and echo to be disabled, matching what
/// `setRaw`/`setRawMode` actually does (see `runtime/ops/tty.rs`). Clearing
/// canonical mode alone does not trigger the hang as long as newlines are
/// still delivered, and treating that as raw would misfire on setups that
/// disable only canonical mode (such as the test PTY harness).
#[cfg(unix)]
fn stdin_is_raw_mode() -> bool {
  // SAFETY: tcgetattr on a possibly-invalid fd 0 returns -1; on any failure we
  // conservatively report not-raw.
  unsafe {
    let mut termios = std::mem::MaybeUninit::<libc::termios>::uninit();
    if libc::tcgetattr(libc::STDIN_FILENO, termios.as_mut_ptr()) != 0 {
      return false;
    }
    let termios = termios.assume_init();
    termios.c_lflag & (libc::ICANON | libc::ECHO) == 0
  }
}

#[cfg(all(not(unix), not(target_arch = "wasm32")))]
fn stdin_is_raw_mode() -> bool {
  use windows_sys::Win32::System::Console::ENABLE_ECHO_INPUT;
  use windows_sys::Win32::System::Console::ENABLE_LINE_INPUT;
  use windows_sys::Win32::System::Console::GetConsoleMode;
  use windows_sys::Win32::System::Console::GetStdHandle;
  use windows_sys::Win32::System::Console::STD_INPUT_HANDLE;

  // SAFETY: winapi calls. GetConsoleMode returns 0 (FALSE) for non-console
  // handles (e.g. when stdin is a pipe), in which case we conservatively
  // return false.
  unsafe {
    let handle = GetStdHandle(STD_INPUT_HANDLE);
    let mut mode = 0u32;
    if GetConsoleMode(handle, &mut mode) == 0 {
      return false;
    }
    mode & (ENABLE_LINE_INPUT | ENABLE_ECHO_INPUT) == 0
  }
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

    // If stdin has been put into raw mode (e.g. a Node.js library has called
    // `process.stdin.setRawMode(true)`) our line-oriented prompt loop would
    // hang forever waiting for a `\n` that the terminal will never deliver,
    // and the user wouldn't see what they're typing either. Bail out with a
    // clear message so the program doesn't appear to freeze.
    #[allow(clippy::print_stderr, reason = "actually want to print")]
    if stdin_is_raw_mode() {
      // Escape the message/name since they can contain user-controlled strings
      // (env var names, file paths) that could otherwise spoof the terminal.
      eprintln!(
        "❌ Cannot prompt for {}: stdin is in raw mode (a library has likely called setRawMode).",
        escape_control_characters(message)
      );
      eprintln!(
        "❌ Run again with --allow-{} to grant the permission up front, or with -A to allow all permissions.",
        escape_control_characters(name)
      );
      return PromptResponse::Deny;
    }

    #[allow(clippy::print_stderr, reason = "actually want to print")]
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

    let terminal_input_guard = lock_terminal_input();

    // Lock stdio streams, so no other output is written while the prompt is
    // displayed.
    let stdout_lock = std::io::stdout().lock();
    let mut stderr_lock = std::io::stderr().lock();
    let mut stdin_lock = std::io::stdin().lock();

    // For security reasons we must consume everything in stdin so that previously
    // buffered data cannot affect the prompt.
    #[allow(clippy::print_stderr, reason = "actually want to output here")]
    if let Err(err) = clear_stdin(&mut stdin_lock, &mut stderr_lock) {
      eprintln!("Error clearing stdin for permission prompt. {err:#}");
      return PromptResponse::Deny; // don't grant permission if this fails
    }

    let message = escape_control_characters(message);
    let name = escape_control_characters(name);
    let api_name = api_name.map(escape_control_characters);

    fn visible_width(text: &str) -> usize {
      let mut width = 0;
      let mut chars = text.chars();
      while let Some(ch) = chars.next() {
        if ch == '\x1b' {
          for ch in chars.by_ref() {
            if ch.is_ascii_alphabetic() {
              break;
            }
          }
        } else {
          width += 1;
        }
      }
      width
    }

    fn border_line(content: String) -> String {
      colors::yellow(content).to_string()
    }

    fn border_cell_line(width: usize, content: &str) -> String {
      let padding = width.saturating_sub(visible_width(content));
      format!(
        "{} {content}{} {}\n",
        colors::yellow("│"),
        " ".repeat(padding),
        colors::yellow("│"),
      )
    }

    fn box_rule(width: usize) -> String {
      format!("├{}┤\n", "─".repeat(width + 2))
    }

    fn terminal_inner_width() -> Option<usize> {
      #[cfg(unix)]
      {
        // SAFETY: ioctl only writes to the winsize struct.
        unsafe {
          let mut size: libc::winsize = std::mem::zeroed();
          if libc::ioctl(2, libc::TIOCGWINSZ, &mut size as *mut _) != 0 {
            return None;
          }
          return (size.ws_col as usize).checked_sub(4);
        }
      }

      #[cfg(windows)]
      {
        use winapi::um::processenv::GetStdHandle;
        use winapi::um::winbase::STD_ERROR_HANDLE;
        use winapi::um::wincon::GetConsoleScreenBufferInfo;
        use winapi::um::wincon::CONSOLE_SCREEN_BUFFER_INFO;

        // SAFETY: winapi calls read console metadata into bufinfo.
        unsafe {
          let handle = GetStdHandle(STD_ERROR_HANDLE);
          let mut bufinfo: CONSOLE_SCREEN_BUFFER_INFO = std::mem::zeroed();
          if GetConsoleScreenBufferInfo(handle, &mut bufinfo) == 0 {
            return None;
          }
          let cols = i32::from(bufinfo.srWindow.Right)
            - i32::from(bufinfo.srWindow.Left)
            + 1;
          return usize::try_from(cols).ok()?.checked_sub(4);
        }
      }

      #[cfg(not(any(unix, windows)))]
      {
        None
      }
    }

    fn prompt_line(prefix: &str) -> String {
      format!("  {prefix} › ")
    }

    // output everything in one shot to make the tests more reliable
    let prompt_lines_count = {
      let docs_url = format!("https://docs.deno.com/go/--allow-{}", name);
      let retry = if crate::is_standalone() {
        format!(
          "deno compile --allow-{name}  # specify during compile time"
        )
      } else {
        format!("deno run --allow-{name} ...")
      };

      let mut lines = vec![
        colors::gray("Deno is asking for:").to_string(),
        String::new(),
        format!("  {}", colors::bold(message.clone())),
        String::new(),
      ];

      if let Some(api_name) = api_name.clone() {
        lines.push(format!(
          "{}   {}",
          colors::gray("Source"),
          colors::bold(api_name)
        ));
      }

      if let Some(get_stack) = get_stack {
        let stack = get_stack();
        if !stack.is_empty() {
          lines.push(colors::gray("Stack").to_string());
          let len = stack.len();
          for (idx, frame) in stack.into_iter().enumerate() {
            lines.push(format!(
              "  {} {}",
              colors::gray(if idx != len - 1 { "├─" } else { "└─" }),
              colors::gray(frame),
            ));
          }
        }
      } else {
        lines.push(
          format!(
            "{}    set DENO_TRACE_PERMISSIONS=1 for stack frames",
            colors::gray("Trace")
          ),
        );
      }

      lines.push(format!(
        "{}     {}",
        colors::gray("Docs"),
        colors::cyan_with_underline(&docs_url)
      ));
      lines.push(format!(
        "{}   {}",
        colors::gray("Bypass"),
        colors::italic(&retry)
      ));

      let actions = if is_unary {
        format!(
          "{} allow once   {} deny   {} allow all {name}",
          colors::green_bold("[y]"),
          colors::red_bold("[n]"),
          colors::yellow_bold("[A]"),
        )
      } else {
        format!(
          "{} allow once   {} deny",
          colors::green_bold("[y]"),
          colors::red_bold("[n]"),
        )
      };

      let content_width = lines
        .iter()
        .chain(std::iter::once(&actions))
        .map(|line| visible_width(line))
        .max()
        .unwrap_or(0)
        .max(54);
      let width = terminal_inner_width()
        .map(|terminal_width| terminal_width.max(content_width))
        .unwrap_or(content_width);

      let mut output = String::new();
      let title = " Permission Request ";
      writeln!(
        &mut output,
        "{}{}{}",
        colors::yellow("┌─"),
        colors::bold(title),
        border_line(format!(
          "{}┐",
          "─".repeat(width.saturating_sub(visible_width(title)) + 1)
        ))
      )
      .unwrap();
      for line in &lines {
        write!(&mut output, "{}", border_cell_line(width, line)).unwrap();
      }
      write!(&mut output, "{}", box_rule(width)).unwrap();
      write!(&mut output, "{}", border_cell_line(width, &actions)).unwrap();
      write!(
        &mut output,
        "{}",
        border_line(prompt_line("select"))
      )
      .unwrap();

      stderr_lock.write_all(output.as_bytes()).unwrap();

      lines.len() + 4
    };

    let unrecognized_prompt = || {
      if is_unary {
        format!(
          "Unrecognized option. select [y/n/A: y allow, n deny, A allow all {name}]"
        )
      } else {
        "Unrecognized option. select [y/n: y allow, n deny]".to_string()
      }
    };

    let value = loop {
      // Clear stdin each time we loop around in case the user accidentally pasted
      // multiple lines or otherwise did something silly to generate a torrent of
      // input. This doesn't work on Windows because `clear_stdin` has other side-effects.
      #[allow(
        clippy::print_stderr,
        reason = "force outputting when permission prompt fails to output"
      )]
      #[cfg(unix)]
      if let Err(err) = clear_stdin(&mut stdin_lock, &mut stderr_lock) {
        eprintln!("Error clearing stdin for permission prompt. {err:#}");
        return PromptResponse::Deny; // don't grant permission if this fails
      }

      let mut input = String::new();
      let result = stdin_lock.read_line(&mut input);
      if result.is_err() {
        break PromptResponse::Deny;
      };
      let input = input.trim_end_matches(['\r', '\n']);

      if matches!(
        input.as_bytes(),
        [b'\x1b', b'[', ..] | [b'\x1b', b'O', ..]
      ) {
        clear_n_lines(&mut stderr_lock, 1);
        write!(stderr_lock, "{}", prompt_line("select")).unwrap();
        continue;
      }

      if input.len() != 1 {
        break PromptResponse::Deny;
      };

      match input.as_bytes()[0] as char {
        'y' | 'Y' => {
          clear_n_lines(&mut stderr_lock, prompt_lines_count);
          let msg = format!("Granted {message}.");
          writeln!(stderr_lock, "✅ {}", colors::bold(&msg)).unwrap();
          break PromptResponse::Allow;
        }
        'n' | 'N' | '\x1b' => {
          clear_n_lines(&mut stderr_lock, prompt_lines_count);
          let msg = format!("Denied {message}.");
          writeln!(stderr_lock, "❌ {}", colors::bold(&msg)).unwrap();
          break PromptResponse::Deny;
        }
        'A' if is_unary => {
          clear_n_lines(&mut stderr_lock, prompt_lines_count);
          let msg = format!("Granted all {name} access.");
          writeln!(stderr_lock, "✅ {}", colors::bold(&msg)).unwrap();
          break PromptResponse::AllowAll;
        }
        _ => {
          // If we don't get a recognized option try again.
          clear_n_lines(&mut stderr_lock, 1);
          write!(
            stderr_lock,
            "{}",
            prompt_line(&colors::bold(unrecognized_prompt()).to_string())
          )
          .unwrap();
        }
      };
    };

    drop(stdout_lock);
    drop(stderr_lock);
    drop(stdin_lock);
    drop(terminal_input_guard);

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

  #[cfg(not(target_arch = "wasm32"))]
  #[test]
  fn escape_control_characters_escapes_bidi_formatting_marks() {
    let escaped =
      escape_control_characters("run access to \u{202e}txt.cilbup\u{202c}");

    assert!(!escaped.contains('\u{202e}'));
    assert!(!escaped.contains('\u{202c}'));
    assert!(escaped.contains(r"\u{202e}"));
    assert!(escaped.contains(r"\u{202c}"));
  }

  #[cfg(not(target_arch = "wasm32"))]
  #[test]
  fn escape_control_characters_escapes_invisible_formatting_marks() {
    // Zero-width / invisible formatting characters render as nothing but can
    // conceal or fake the displayed label.
    for c in [
      '\u{200b}', // Zero Width Space
      '\u{200c}', // Zero Width Non-Joiner
      '\u{200d}', // Zero Width Joiner
      '\u{2060}', // Word Joiner
      '\u{feff}', // Zero Width No-Break Space (byte order mark)
    ] {
      let input = format!("access to secret{c}.txt");
      let escaped = escape_control_characters(&input);
      assert!(!escaped.contains(c), "{c:?} should not survive unescaped");
      assert!(
        escaped.contains(&c.escape_debug().to_string()),
        "{c:?} should be rendered visibly"
      );
    }
  }

  #[cfg(not(target_arch = "wasm32"))]
  #[test]
  fn escape_control_characters_leaves_safe_unicode_visible() {
    assert_eq!(escape_control_characters("文件.txt"), "文件.txt");
  }
}
