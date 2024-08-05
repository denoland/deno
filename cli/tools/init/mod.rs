// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use crate::args::InitFlags;
use crate::colors;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_core::serde_json::json;
use log::info;
use std::io::Write;
use std::path::Path;

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

  if init_flags.serve {
    create_file(
      &dir,
      "main.ts",
      r#"export default {
  fetch(req) {
      console.log(req.url);
      return new Response("Hello world");
  }
}
"#,
    )?;
    create_file(
      &dir,
      "main_test.ts",
      r#"import { assertEquals } from "@std/assert";
import server from "./main.ts";

Deno.test(async function serverFetch() {
  const req = new Request("https://deno.land");
  const res = await server.fetch(req);
  assertEquals(await res.text(), "Hello world");
});
"#,
    )?;

    create_json_file(
      &dir,
      "deno.json",
      &json!({
        "tasks": {
          "dev": "deno serve --watch main.ts"
        },
        "imports": {
          "@std/assert": "jsr:@std/assert@1"
        }
      }),
    )?;
  } else if init_flags.lib {
    // Extract the directory name to use as the project name
    let project_name = dir
      .file_name()
      .unwrap_or_else(|| dir.as_os_str())
      .to_str()
      .unwrap();

    create_file(
      &dir,
      "mod.ts",
      r#"export function add(a: number, b: number): number {
  return a + b;
}
"#,
    )?;
    create_file(
      &dir,
      "mod_test.ts",
      r#"import { assertEquals } from "@std/assert";
import { add } from "./mod.ts";

Deno.test(function addTest() {
  assertEquals(add(2, 3), 5);
});
"#,
    )?;

    create_json_file(
      &dir,
      "deno.json",
      &json!({
        "name": project_name,
        "version": "0.1.0",
        "tasks": {
          "dev": "deno test --watch mod.ts"
        },
        "imports": {
          "@std/assert": "jsr:@std/assert@1"
        },
        "exports": "./mod.ts"
      }),
    )?;
  } else {
    create_file(
      &dir,
      "main.ts",
      r#"export function add(a: number, b: number): number {
  return a + b;
}

// Learn more at https://docs.deno.com/runtime/manual/examples/module_metadata#concepts
if (import.meta.main) {
  console.log("Add 2 + 3 =", add(2, 3));
}
"#,
    )?;
    create_file(
      &dir,
      "main_test.ts",
      r#"import { assertEquals } from "@std/assert";
import { add } from "./main.ts";

Deno.test(function addTest() {
  assertEquals(add(2, 3), 5);
});
"#,
    )?;

    create_json_file(
      &dir,
      "deno.json",
      &json!({
        "tasks": {
          "dev": "deno run --watch main.ts"
        },
        "imports": {
          "@std/assert": "jsr:@std/assert@1"
        }
      }),
    )?;
  }

  info!("✅ {}", colors::green("Project initialized"));
  info!("");
  info!("{}", colors::gray("Run these commands to get started"));
  info!("");
  if let Some(dir) = init_flags.dir {
    info!("  cd {}", dir);
    info!("");
  }
  if init_flags.serve {
    info!("  {}", colors::gray("# Run the server"));
    info!("  deno serve main.ts");
    info!("");
    info!(
      "  {}",
      colors::gray("# Run the server and watch for file changes")
    );
    info!("  deno task dev");
    info!("");
    info!("  {}", colors::gray("# Run the tests"));
    info!("  deno test");
  } else if init_flags.lib {
    info!("  {}", colors::gray("# Run the tests"));
    info!("  deno test");
    info!("");
    info!(
      "  {}",
      colors::gray("# Run the tests and watch for file changes")
    );
    info!("  deno task dev");
    info!("");
    info!("  {}", colors::gray("# Publish to JSR (dry run)"));
    info!("  deno publish --dry-run");
  } else {
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
  }
  Ok(())
}

fn create_json_file(
  dir: &Path,
  filename: &str,
  value: &deno_core::serde_json::Value,
) -> Result<(), AnyError> {
  let mut text = deno_core::serde_json::to_string_pretty(value)?;
  text.push('\n');
  create_file(dir, filename, &text)
}

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
