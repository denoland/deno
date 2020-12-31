// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

mod standalone;

fn main() {
    #[cfg(windows)]
    deno_runtime::colors::enable_ansi(); // For Windows 10
  
    let args: Vec<String> = std::env::args().collect();
    if let Err(err) = standalone::try_run_standalone_binary(args) {
      eprintln!(
        "{}: {}",
        deno_runtime::colors::red_bold("error"),
        err.to_string()
      );
      std::process::exit(1);
    }
  }
  