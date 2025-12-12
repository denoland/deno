// Copyright 2018-2025 the Deno authors. MIT license.

use std::cell::RefCell;
use std::io::Write;
use std::sync::Arc;
use std::thread::JoinHandle;

use parking_lot::Mutex;

thread_local! {
  static OUTPUT_BUFFER: RefCell<Arc<Mutex<Option<Vec<u8>>>>> = RefCell::new(Arc::new(Mutex::new(None)));
}

/// Spawns a thread maintaining the output buffer for capturing printing.
pub fn spawn_thread<F, T>(f: F) -> JoinHandle<T>
where
  F: FnOnce() -> T,
  F: Send + 'static,
  T: Send + 'static,
{
  let captured_buffer = OUTPUT_BUFFER.with(|buffer| buffer.borrow().clone());
  #[allow(clippy::disallowed_methods)]
  std::thread::spawn(|| {
    OUTPUT_BUFFER.with(|buffer| {
      *buffer.borrow_mut() = captured_buffer;
    });
    f()
  })
}

/// Print to stdout, or to the thread-local buffer if one is set
pub fn print_stdout(data: &[u8]) {
  OUTPUT_BUFFER.with(|buffer_cell| {
    {
      let buffer = buffer_cell.borrow();
      let mut buffer = buffer.lock();
      if let Some(ref mut buf) = *buffer {
        buf.extend_from_slice(data);
        return;
      }
    }
    let _ = std::io::stdout().write_all(data);
  });
}

/// Print to stderr, or to the thread-local buffer if one is set
pub fn print_stderr(data: &[u8]) {
  OUTPUT_BUFFER.with(|buffer_cell| {
    {
      let buffer = buffer_cell.borrow();
      let mut buffer = buffer.lock();
      if let Some(ref mut buf) = *buffer {
        buf.extend_from_slice(data);
        return;
      }
    }
    let _ = std::io::stderr().write_all(data);
  });
}

/// Capture output from a function, returning both the output and the function's result
pub fn with_captured_output<F, R>(f: F) -> (Vec<u8>, R)
where
  F: FnOnce() -> R,
{
  /// RAII guard that ensures the output buffer is cleaned up even on panic
  struct CaptureGuard {
    enabled: bool,
  }

  impl CaptureGuard {
    fn new(enabled: bool) -> Self {
      if enabled {
        set_buffer(true);
      }
      Self { enabled }
    }
  }

  impl Drop for CaptureGuard {
    fn drop(&mut self) {
      if self.enabled {
        // Ensure buffer is disabled even on panic
        set_buffer(false);
      }
    }
  }

  let should_capture = !*file_test_runner::NO_CAPTURE;
  let _guard = CaptureGuard::new(should_capture);
  let result = f();
  let output = take_buffer();
  (output, result)
}

fn set_buffer(enabled: bool) {
  OUTPUT_BUFFER.with(|buffer| {
    let buffer = buffer.borrow();
    *buffer.lock() = if enabled { Some(Vec::new()) } else { None };
  });
}

fn take_buffer() -> Vec<u8> {
  OUTPUT_BUFFER.with(|buffer| buffer.borrow().lock().take().unwrap_or_default())
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
