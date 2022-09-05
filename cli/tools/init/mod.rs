// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use crate::args::InitFlags;
use crate::deno_std;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use log::info;
use std::io::Write;
use std::path::Path;

fn create_file(
  dir: &Path,
  filename: &str,
  content: &str,
) -> Result<(), AnyError> {
  let mut file = std::fs::OpenOptions::new()
    .write(true)
    .create_new(true)
    .open(dir.join(filename))
    .with_context(|| format!("Failed to create {} file", filename))?;
  file.write_all(content.as_bytes())?;
  Ok(())
}

pub async fn init_project(init_flags: InitFlags) -> Result<(), AnyError> {
  let cwd =
    std::env::current_dir().context("Can't read current working directory.")?;
  let dir = if let Some(dir) = &init_flags.dir {
    let dir = cwd.join(dir);
    std::fs::create_dir_all(&dir)?;
    dir
  } else {
    cwd
  };

  let main_ts = include_str!("./templates/main.ts");
  create_file(&dir, "main.ts", main_ts)?;

  let main_test_ts = include_str!("./templates/main_test.ts")
    .replace("{CURRENT_STD_URL}", deno_std::CURRENT_STD_URL.as_str());
  create_file(&dir, "main_test.ts", &main_test_ts)?;

  info!("✅ Project initialized");
  info!("Run these commands to get started");
  if let Some(dir) = init_flags.dir {
    info!("  cd {}", dir);
  }
  info!("  deno run main.ts");
  info!("  deno test");
  Ok(())
}
