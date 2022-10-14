// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use crate::args::InitFlags;
use crate::deno_std;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_core::{anyhow::Context, error::AnyError};
use dialoguer::{theme::ColorfulTheme, MultiSelect};
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
  let dir = if let Some(dir) = &init_flags.dir {
    let dir = std::env::current_dir()?.join(dir);
    std::fs::create_dir_all(&dir)?;
    dir
  } else {
    std::env::current_dir()?
  };

  let multiselected = &[
    "Use TypeScript",
    "Add a config file",
    "Add an import map",
    "Setup VSCode extension",
  ];
  let defaults = &[true, false, false, false];
  let selections = MultiSelect::with_theme(&ColorfulTheme::default())
    .with_prompt("Initialize your project (use space to select)")
    .items(&multiselected[..])
    .defaults(&defaults[..])
    .interact()?;

  // TS or JS
  if selections.contains(&0) {
    let mod_ts = include_str!("./templates/mod.ts");
    create_file(&dir, "mod.ts", mod_ts)?;

    let mod_test_ts = include_str!("./templates/mod_test.ts").replace(
      "{CURRENT_STD_URL}",
      if selections.contains(&2) {
        "std/"
      } else {
        deno_std::CURRENT_STD_URL
      },
    );
    create_file(&dir, "mod_test.ts", &mod_test_ts)?;
  } else {
    let mod_js = include_str!("./templates/mod.js");
    create_file(&dir, "mod.js", mod_js)?;

    let mod_test_js = include_str!("./templates/mod_test.js").replace(
      "{CURRENT_STD_URL}",
      if selections.contains(&2) {
        "std/"
      } else {
        deno_std::CURRENT_STD_URL
      },
    );
    create_file(&dir, "mod_test.js", &mod_test_js)?;
  }

  // Config file
  if selections.contains(&1) {
    let deno_json = include_str!("./templates/deno.json")
      .replace(
        "{MAYBE_IMPORT_MAP}",
        if selections.contains(&2) {
          "\"importMap\": \"./import_map.json\",\n"
        } else {
          ""
        },
      )
      .replace(
        "{MOD_FILE}",
        if selections.contains(&0) {
          "./mod.ts"
        } else {
          "./mod.js"
        },
      );
    create_file(&dir, "deno.json", &deno_json)?;
  }

  // Import map
  if selections.contains(&2) {
    let import_map_json = include_str!("./templates/import_map.json")
      .replace("{CURRENT_STD_URL}", deno_std::CURRENT_STD_URL);
    create_file(&dir, "import_map.json", &import_map_json)?;
  }

  // VSCode settings
  if selections.contains(&3) {
    let vscode_settings = include_str!("./templates/vscode.json");
    std::fs::create_dir_all(&dir.join(".vscode"))
      .context("Failed to create .vscode directory")?;
    create_file(&dir, ".vscode/settings.json", vscode_settings)?;
  }

  println!("Project initalized");
  println!("Run these commands to get started");
  if let Some(dir) = init_flags.dir {
    println!("  cd {}", dir);
  }
  if selections.contains(&0) {
    println!("  deno run mod.ts");
    println!("  deno test mod_test.ts");
  } else {
    println!("  deno run mod.js");
    println!("  deno test mod_test.js");
  }
  Ok(())
}
