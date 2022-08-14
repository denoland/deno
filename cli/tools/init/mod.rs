// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use crate::args::InitFlags;
use crate::compat;
use deno_core::error::AnyError;
use std::io::Write;

pub async fn init_project(init_flags: InitFlags) -> Result<(), AnyError> {
  let dir = if let Some(dir) = &init_flags.dir {
    let dir = std::env::current_dir()?.join(dir);
    std::fs::create_dir_all(&dir)?;
    dir
  } else {
    std::env::current_dir()?
  };

  let mod_ts = include_str!("./templates/mod.ts");
  let mut file = std::fs::OpenOptions::new()
    .write(true)
    .create_new(true)
    .open(dir.join("mod.ts"))?;
  file.write_all(mod_ts.as_bytes())?;

  let mod_test_ts = include_str!("./templates/mod_test.ts")
    .replace("{CURRENT_STD_URL}", compat::STD_URL_STR);
  let mut file = std::fs::OpenOptions::new()
    .write(true)
    .create_new(true)
    .open(dir.join("mod_test.ts"))?;
  file.write_all(mod_test_ts.as_bytes())?;

  println!("Project initalized");
  println!("Run these commands to get started");
  if let Some(dir) = init_flags.dir {
    println!("  cd {}", dir);
  }
  println!("  deno run mod.ts");
  println!("  deno test mod_test.ts");
  Ok(())
}
