// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use crate::args::DenoSubcommand;
use crate::args::Flags;
use crate::args::InitFlags;
use crate::args::PackagesAllowedScripts;
use crate::args::PermissionFlags;
use crate::args::RunFlags;
use crate::colors;
use color_print::cformat;
use color_print::cstr;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_core::serde_json::json;
use deno_runtime::WorkerExecutionMode;
use log::info;
use std::io::IsTerminal;
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
      r#"import { serveDir } from "@std/http";

const userPagePattern = new URLPattern({ pathname: "/users/:id" });
const staticPathPattern = new URLPattern({ pathname: "/static/*" });

export default {
  fetch(req) {
    const url = new URL(req.url);

    if (url.pathname === "/") {
      return new Response("Home page");
    }

    const userPageMatch = userPagePattern.exec(url);
    if (userPageMatch) {
      return new Response(userPageMatch.pathname.groups.id);
    }

    if (staticPathPattern.test(url)) {
      return serveDir(req);
    }

    return new Response("Not found", { status: 404 });
  },
} satisfies Deno.ServeDefaultExport;
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
  assertEquals(await res.text(), "Home page");
});

Deno.test(async function serverFetchNotFound() {
  const req = new Request("https://deno.land/404");
  const res = await server.fetch(req);
  assertEquals(res.status, 404);
});

Deno.test(async function serverFetchUsers() {
  const req = new Request("https://deno.land/users/123");
  const res = await server.fetch(req);
  assertEquals(await res.text(), "123");
});

Deno.test(async function serverFetchStatic() {
  const req = new Request("https://deno.land/static/hello.js");
  const res = await server.fetch(req);
  assertEquals(await res.text(), 'console.log("Hello, world!");\n');
  assertEquals(res.headers.get("content-type"), "text/javascript; charset=UTF-8");
});
"#,
    )?;

    let static_dir = dir.join("static");
    std::fs::create_dir_all(&static_dir)?;
    create_file(
      &static_dir,
      "hello.js",
      r#"console.log("Hello, world!");
"#,
    )?;

    create_json_file(
      &dir,
      "deno.json",
      &json!({
        "tasks": {
          "dev": "deno serve --watch -R main.ts",
        },
        "imports": {
          "@std/assert": "jsr:@std/assert@1",
          "@std/http": "jsr:@std/http@1",
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
        "exports": "./mod.ts",
        "tasks": {
          "dev": "deno test --watch mod.ts"
        },
        "license": "MIT",
        "imports": {
          "@std/assert": "jsr:@std/assert@1"
        },
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
    info!("  deno serve -R main.ts");
    info!("");
    info!(
      "  {}",
      colors::gray("# Run the server and watch for file changes")
    );
    info!("  deno task dev");
    info!("");
    info!("  {}", colors::gray("# Run the tests"));
    info!("  deno test -R");
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

pub async fn init_npm(name: &str, args: Vec<String>) -> Result<i32, AnyError> {
  let name = name.strip_prefix("npm:").unwrap();
  let script_name = format!("npm:create-{}", name);

  fn print_manual_usage(script_name: &str) -> i32 {
    log::info!("{}", cformat!("You can initialize project manually by running <u>deno run {}</> and applying desired permissions.", script_name));
    1
  }

  if std::io::stdin().is_terminal() {
    log::info!(
      cstr!("Deno requires <u>all permissions</> to create a new <g>{}</> project. Do you want to continue? <p(245)>[y/n]</>"),
      name
    );
    loop {
      let _ = std::io::stdout().write(b"> ");
      let _ = std::io::stdout().flush();
      let mut answer = String::new();
      if let Ok(_) = std::io::stdin().read_line(&mut answer) {
        let answer = answer.trim().to_ascii_lowercase();
        if answer != "y" {
          return Ok(print_manual_usage(&script_name));
        } else {
          break;
        }
      }
    }
  } else {
    return Ok(print_manual_usage(&script_name));
  }

  let new_flags = Flags {
    permissions: PermissionFlags {
      allow_all: true,
      ..Default::default()
    },
    allow_scripts: PackagesAllowedScripts::All,
    argv: args,
    subcommand: DenoSubcommand::Run(RunFlags {
      script: script_name,
      ..Default::default()
    }),
    ..Default::default()
  };
  crate::tools::run::run_script(
    WorkerExecutionMode::Run,
    new_flags.into(),
    None,
  )
  .await
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
