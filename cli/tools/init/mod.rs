// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use crate::args::InitFlags;
use crate::colors;
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
  let path = dir.join(filename);
  if path.exists() {
    info!(
      "ℹ️ {}",
      colors::gray(format!("Skipped creating {filename} as it already exists"))
    );
    Ok(())
  } else {
    let mut file = std::fs::OpenOptions::new()
      .write(true)
      .create_new(true)
      .open(path)
      .with_context(|| format!("Failed to create {filename} file"))?;
    file.write_all(content.as_bytes())?;
    Ok(())
  }
}

pub fn init_project(init_flags: InitFlags) -> Result<(), AnyError> {
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

  create_file(
    &dir,
    "main_test.ts",
    include_str!("./templates/main_test.ts"),
  )?;
  create_file(&dir, "deno.json", include_str!("./templates/deno.json"))?;

  info!("✅ {}", colors::green("Project initialized"));
  info!("");
  info!("{}", colors::gray("Run these commands to get started"));
  info!("");
  if let Some(dir) = init_flags.dir {
    info!("  cd {}", dir);
    info!("");
  }
  info!("  {}", colors::gray("# Run the program"));
  info!("  deno run main.ts");
  info!("");
  info!(
    "  {}",
    colors::gray("# Run the program and watch for file changes")
  );
  info!("  deno task dev");
  info!("");
  info!("  {}", colors::gray("# Run the tests"));
  info!("  deno test");
  Ok(())
}
