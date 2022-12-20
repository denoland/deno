// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use crate::args::InitFlags;
use crate::colors;
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
  let main_bench_ts = include_str!("./templates/main_bench.ts");
  create_file(&dir, "main_bench.ts", main_bench_ts)?;

  create_file(&dir, "deno.jsonc", include_str!("./templates/deno.jsonc"))?;

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
  info!("");
  info!("  {}", colors::gray("# Run the benchmarks"));
  info!("  deno bench");
  Ok(())
}
