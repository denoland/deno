// Copyright 2018-2026 the Deno authors. MIT license.

use std::io::IsTerminal;
use std::io::Write;
use std::path::Path;

use color_print::cformat;
use color_print::cstr;
use deno_config::deno_json::NodeModulesDirMode;
use deno_core::anyhow::Context;
use deno_core::anyhow::bail;
use deno_core::error::AnyError;
use deno_core::futures::FutureExt;
use deno_core::serde_json::json;
use deno_lib::args::UnstableConfig;
use deno_npm_installer::PackagesAllowedScripts;
use deno_runtime::WorkerExecutionMode;
use deno_semver::jsr::JsrPackageReqReference;
use log::info;

use crate::args::DenoSubcommand;
use crate::args::Flags;
use crate::args::InitFlags;
use crate::args::InternalFlags;
use crate::args::PermissionFlags;
use crate::args::RunFlags;
use crate::colors;
use crate::util::temp::create_temp_node_modules_dir;

pub async fn init_project(init_flags: InitFlags) -> Result<i32, AnyError> {
  if let Some(package) = &init_flags.package {
    if package.starts_with("jsr:") {
      return init_jsr(package, init_flags.package_args, init_flags.yes)
        .boxed_local()
        .await;
    } else if package.starts_with("npm:") {
      return init_npm(package, init_flags.package_args, init_flags.yes)
        .boxed_local()
        .await;
    } else {
      bail!("Unknown package kind: {}", package);
    }
  }

  let cwd =
    std::env::current_dir().context("Can't read current working directory.")?;
  let dir = if let Some(dir) = &init_flags.dir {
    let dir = cwd.join(dir);
    std::fs::create_dir_all(&dir)?;
    dir
  } else {
    cwd
  };

  if init_flags.empty {
    create_file(
      &dir,
      "main.ts",
      r#"console.log('Hello world!');
"#,
    )?;

    create_json_file(
      &dir,
      "deno.json",
      &json!({
        "tasks": {
          "dev": "deno run --watch main.ts"
        }
      }),
    )?;
  } else if init_flags.serve {
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
          "dev": "deno test --watch"
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
  if init_flags.empty {
    info!("  {}", colors::gray("# Run the program"));
    info!("  deno run main.ts");
    info!("");
    info!(
      "  {}",
      colors::gray("# Run the program and watch for file changes")
    );
    info!("  deno task dev");
  } else if init_flags.serve {
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
  Ok(0)
}

async fn init_npm(
  name: &str,
  args: Vec<String>,
  yes: bool,
) -> Result<i32, AnyError> {
  let script_name = npm_name_to_create_package(name);

  if let Some(exit_code) = confirm_package_trust(&script_name, &args, yes)? {
    return Ok(exit_code);
  }

  let temp_node_modules_dir = create_temp_node_modules_dir()
    .context("Failed creating temp directory for node_modules folder.")?;

  let new_flags = Flags {
    permissions: PermissionFlags {
      allow_all: true,
      ..Default::default()
    },
    allow_scripts: PackagesAllowedScripts::All,
    argv: args,
    node_modules_dir: Some(NodeModulesDirMode::Auto),
    subcommand: DenoSubcommand::Run(RunFlags {
      script: script_name,
      ..Default::default()
    }),
    reload: true,
    internal: InternalFlags {
      lockfile_skip_write: true,
      root_node_modules_dir_override: Some(
        temp_node_modules_dir.node_modules_dir_path().to_path_buf(),
      ),
      ..Default::default()
    },
    unstable_config: UnstableConfig {
      bare_node_builtins: true,
      sloppy_imports: true,
      detect_cjs: true,
      ..Default::default()
    },
    ..Default::default()
  };
  let result = crate::tools::run::run_script(
    WorkerExecutionMode::Run,
    new_flags.into(),
    None,
    None,
    Default::default(),
  )
  .await;
  drop(temp_node_modules_dir); // explicit drop for clarity
  result
}

fn npm_name_to_create_package(name: &str) -> String {
  let name = name.strip_prefix("npm:").unwrap_or(name);
  let mut s = "npm:".to_string();

  let mut scoped = false;
  let mut create = false;

  for (i, ch) in name.char_indices() {
    if i == 0 {
      if ch == '@' {
        scoped = true;
      } else {
        create = true;
        s.push_str("create-");
      }
    } else if scoped {
      if ch == '/' {
        scoped = false;
        create = true;
        s.push_str("/create-");
        continue;
      } else if ch == '@' && !create {
        scoped = false;
        create = true;
        s.push_str("/create@");
        continue;
      }
    }

    s.push(ch);
  }

  if !create {
    s.push_str("/create");
  }

  s
}

/// Prompts the user to confirm they trust the package, unless `--yes` was
/// passed. Returns `Ok(None)` when confirmed, or `Ok(Some(exit_code))` when
/// the user declined (caller should return early with that code).
fn confirm_package_trust(
  script_name: &str,
  args: &[String],
  yes: bool,
) -> Result<Option<i32>, AnyError> {
  if yes {
    return Ok(None);
  }

  fn print_manual_usage(script_name: &str, args: &[String]) -> i32 {
    log::info!(
      "{}",
      cformat!(
        "You can initialize project manually by running <u>deno run {}</> and applying desired permissions.",
        std::iter::once(script_name)
          .chain(args.iter().map(|a| a.as_ref()))
          .collect::<Vec<_>>()
          .join(" ")
      )
    );
    1
  }

  if std::io::stdin().is_terminal() {
    log::info!(
      cstr!(
        "⚠️  Do you fully trust <y>{}</> package? Deno will invoke code from it with all permissions. Do you want to continue? <p(245)>[y/n]</>"
      ),
      script_name
    );
    loop {
      let _ = std::io::stdout().write(b"> ")?;
      std::io::stdout().flush()?;
      let mut answer = String::new();
      if std::io::stdin().read_line(&mut answer).is_ok() {
        let answer = answer.trim().to_ascii_lowercase();
        if answer != "y" {
          return Ok(Some(print_manual_usage(script_name, args)));
        } else {
          return Ok(None);
        }
      }
    }
  } else {
    Ok(Some(print_manual_usage(script_name, args)))
  }
}

async fn init_jsr(
  name: &str,
  args: Vec<String>,
  yes: bool,
) -> Result<i32, AnyError> {
  let script_name = jsr_name_to_create_script(name);

  if let Some(exit_code) = confirm_package_trust(&script_name, &args, yes)? {
    return Ok(exit_code);
  }

  let new_flags = Flags {
    permissions: PermissionFlags {
      allow_all: true,
      ..Default::default()
    },
    argv: args,
    subcommand: DenoSubcommand::Run(RunFlags {
      script: script_name,
      ..Default::default()
    }),
    reload: true,
    internal: InternalFlags {
      lockfile_skip_write: true,
      ..Default::default()
    },
    ..Default::default()
  };
  crate::tools::run::run_script(
    WorkerExecutionMode::Run,
    new_flags.into(),
    None,
    None,
    Default::default(),
  )
  .await
}

/// Appends `/create` to the subpath of a JSR specifier.
fn jsr_name_to_create_script(name: &str) -> String {
  let Ok(parsed) = JsrPackageReqReference::from_str(name) else {
    return name.to_string();
  };

  let mut inner = parsed.into_inner();
  let new_sub_path = match inner.sub_path.as_deref() {
    Some(sub) => format!("{sub}/create"),
    None => "create".to_string(),
  };
  inner.sub_path = Some(new_sub_path.as_str().into());
  JsrPackageReqReference::new(inner).to_string()
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

#[cfg(test)]
mod test {
  use super::*;

  #[test]
  fn npm_name_to_create_package_test() {
    // See https://docs.npmjs.com/cli/v8/commands/npm-init#description
    assert_eq!(
      npm_name_to_create_package("foo"),
      "npm:create-foo".to_string()
    );
    assert_eq!(
      npm_name_to_create_package("foo@1.0.0"),
      "npm:create-foo@1.0.0".to_string()
    );
    assert_eq!(
      npm_name_to_create_package("@foo"),
      "npm:@foo/create".to_string()
    );
    assert_eq!(
      npm_name_to_create_package("@foo@1.0.0"),
      "npm:@foo/create@1.0.0".to_string()
    );
    assert_eq!(
      npm_name_to_create_package("@foo/bar"),
      "npm:@foo/create-bar".to_string()
    );
    assert_eq!(
      npm_name_to_create_package("@foo/bar@1.0.0"),
      "npm:@foo/create-bar@1.0.0".to_string()
    );
    assert_eq!(
      npm_name_to_create_package("npm:vite"),
      "npm:create-vite".to_string()
    );
    assert_eq!(
      npm_name_to_create_package("npm:@foo/bar@1.0.0"),
      "npm:@foo/create-bar@1.0.0".to_string()
    );
  }

  #[test]
  fn jsr_name_to_create_script_test() {
    // scoped without subpath -> appends /create
    assert_eq!(
      jsr_name_to_create_script("jsr:@fresh/init"),
      "jsr:@fresh/init/create"
    );
    // scoped with version, no subpath -> appends /create (canonical form)
    assert_eq!(
      jsr_name_to_create_script("jsr:@fresh/init@2"),
      "jsr:@fresh/init@2/create"
    );
    assert_eq!(
      jsr_name_to_create_script("jsr:@fresh/init@^2.0.0"),
      "jsr:@fresh/init@^2.0.0/create"
    );
    // scoped with subpath -> appends /create
    assert_eq!(
      jsr_name_to_create_script("jsr:@std/http/file-server"),
      "jsr:@std/http/file-server/create"
    );
    assert_eq!(
      jsr_name_to_create_script("jsr:@std/http/file-server/"),
      "jsr:@std/http/file-server/create"
    );
    // scoped with subpath and version -> appends /create
    assert_eq!(
      jsr_name_to_create_script("jsr:@std/http@1/file-server"),
      "jsr:@std/http@1/file-server/create"
    );
  }
}
