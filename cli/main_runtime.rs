// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

#![deny(warnings)]

mod colors;
mod standalone;
mod tokio_util;
mod version;

use deno_core::error::anyhow;
use deno_core::error::AnyError;
use std::env;

pub fn main() {
  #[cfg(windows)]
  colors::enable_ansi(); // For Windows 10

  let args: Vec<String> = env::args().collect();
  if let Err(err) = run(args) {
    eprintln!("{}: {}", colors::red_bold("error"), err.to_string());
    std::process::exit(1);
  }
}

fn run(args: Vec<String>) -> Result<(), AnyError> {
  let (metadata, bundle) = standalone::extract_standalone(args)?
    .ok_or_else(|| anyhow!("This executable is used internally by 'deno compile', it is not meant to be invoked directly."))?;
  tokio_util::run_basic(standalone::run(bundle, metadata))
}
