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
pub async fn run_standalone(
  source_code: String,
  metadata: standalone::Metadata,
) -> Result<(), AnyError> {
  let main_module = deno_core::resolve_url(standalone::SPECIFIER)?;
  let (mut worker, options) = standalone::create_standalone_worker(
    main_module.clone(),
    source_code,
    metadata,
  )?;
  worker.bootstrap(&options);
  worker.execute_module(&main_module).await?;
  worker.execute("window.dispatchEvent(new Event('load'))")?;
  worker.run_event_loop().await?;
  worker.execute("window.dispatchEvent(new Event('unload'))")?;
  std::process::exit(0);
}

fn run(args: Vec<String>) -> Result<(), AnyError> {
  let (metadata, bundle) = standalone::extract_standalone(args)?
    .ok_or_else(|| anyhow!("This executable is used internally by 'deno compile', it is not meant to be invoked directly."))?;
  tokio_util::run_basic(run_standalone(bundle, metadata))
}
