// Copyright 2018-2025 the Deno authors. MIT license.

use deno_runtime::ops::tty::ConsoleSize;

/// Gets the console size.
pub fn console_size() -> Option<ConsoleSize> {
  let stderr = &deno_runtime::deno_io::STDERR_HANDLE;
  deno_runtime::ops::tty::console_size(stderr).ok()
}
