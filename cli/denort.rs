// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

mod flags_rt;
mod rt;
mod version;

fn main() {
  #[cfg(windows)]
  deno_runtime::colors::enable_ansi(); // For Windows 10

  let args: Vec<String> = std::env::args().collect();
  if let Err(err) = rt::try_run_standalone_binary(args) {
    eprintln!(
      "{}: {}",
      deno_runtime::colors::red_bold("error"),
      err.to_string()
    );
    std::process::exit(1);
  }

  // TODO (yos1p) Specify better error message
  eprintln!(
    "{}: Runtime Error.",
    deno_runtime::colors::red_bold("error")
  );
  std::process::exit(1);
}
