use deno_runtime::ops::tty::ConsoleSize;

/// Gets the console size.
pub fn console_size() -> Option<ConsoleSize> {
  let stderr = &deno_runtime::ops::io::STDERR_HANDLE;
  deno_runtime::ops::tty::console_size(stderr).ok()
}

pub fn show_cursor() {
  eprint!("\x1B[?25h");
}

pub fn hide_cursor() {
  eprint!("\x1B[?25l");
}
