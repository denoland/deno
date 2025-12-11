// Copyright 2018-2025 the Deno authors. MIT license.

use std::cell::RefCell;
use std::io::Write;

thread_local! {
    static OUTPUT_BUFFER: RefCell<Option<Vec<u8>>> = const { RefCell::new(None) };
}

/// Print to stdout, or to the thread-local buffer if one is set
pub fn print_stdout(data: &[u8]) {
  OUTPUT_BUFFER.with(|buffer| {
    if let Some(ref mut buf) = *buffer.borrow_mut() {
      buf.extend_from_slice(data);
    } else {
      let _ = std::io::stdout().write_all(data);
    }
  });
}

/// Print to stderr, or to the thread-local buffer if one is set
pub fn print_stderr(data: &[u8]) {
  OUTPUT_BUFFER.with(|buffer| {
    if let Some(ref mut buf) = *buffer.borrow_mut() {
      buf.extend_from_slice(data);
    } else {
      let _ = std::io::stderr().write_all(data);
    }
  });
}

/// Capture output from a function, returning both the output and the function's result
pub fn with_captured_output<F, R>(f: F) -> (Vec<u8>, R)
where
  F: FnOnce() -> R,
{
  if !*file_test_runner::NO_CAPTURE {
    set_buffer(true);
  }
  let result = f();
  let output = take_buffer();
  (output, result)
}

fn set_buffer(enabled: bool) {
  OUTPUT_BUFFER.with(|buffer| {
    let mut buffer = buffer.borrow_mut();
    *buffer = if enabled { Some(Vec::new()) } else { None };
  });
}

fn take_buffer() -> Vec<u8> {
  OUTPUT_BUFFER.with(|buffer| buffer.borrow_mut().take().unwrap_or_default())
}

/// Print to stdout with a newline
#[macro_export]
macro_rules! println {
    () => {
        $crate::print::print_stdout(b"\n")
    };
    ($($arg:tt)*) => {{
        let mut msg = format!($($arg)*);
        msg.push('\n');
        $crate::print::print_stdout(msg.as_bytes());
    }};
}

/// Print to stdout without a newline
#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {{
        let msg = format!($($arg)*);
        $crate::print::print_stdout(msg.as_bytes());
    }};
}

/// Print to stderr with a newline
#[macro_export]
macro_rules! eprintln {
    () => {
        $crate::print::print_stderr(b"\n")
    };
    ($($arg:tt)*) => {{
        let mut msg = format!($($arg)*);
        msg.push('\n');
        $crate::print::print_stderr(msg.as_bytes());
    }};
}

/// Print to stderr without a newline
#[macro_export]
macro_rules! eprint {
    ($($arg:tt)*) => {{
        let msg = format!($($arg)*);
        $crate::print::print_stderr(msg.as_bytes());
    }};
}
