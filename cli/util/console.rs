// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::io::IsTerminal;
use std::io::Write;

use deno_runtime::ops::tty::ConsoleSize;

/// Gets the console size.
pub fn console_size() -> Option<ConsoleSize> {
  let stderr = &deno_runtime::deno_io::STDERR_HANDLE;
  deno_runtime::ops::tty::console_size(stderr).ok()
}

pub fn confirm(text: &str, default: bool) -> bool {
  if !std::io::stderr().is_terminal() {
    return default;
  }
  let default_str = if default { "(y)" } else { "(n)" };
  eprint!("{} {} > ", text, default_str);
  let _ = std::io::stderr().flush();
  let mut input = String::new();
  let _ = std::io::stdin().read_line(&mut input);
  let input = input.trim().to_lowercase();
  if input.is_empty() {
    default
  } else {
    input == "y" || input == "yes"
  }
}
