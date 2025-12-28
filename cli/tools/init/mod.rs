// Copyright 2018-2025 the Deno authors. MIT license.

use std::ffi::OsString;
use std::io::IsTerminal;
use std::io::Write;
use std::path::Path;

use chrono::NaiveDate;
use color_print::cformat;
use color_print::cstr;
use deno_config::deno_json::NodeModulesDirMode;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_core::futures::FutureExt;
use deno_core::serde_json::json;
use deno_lib::args::UnstableConfig;
use deno_npm_installer::PackagesAllowedScripts;
use deno_runtime::WorkerExecutionMode;
use log::info;

use crate::args::DenoSubcommand;
use crate::args::Flags;
use crate::args::InitFlags;
use crate::args::InternalFlags;
use crate::args::PermissionFlags;
use crate::args::RunFlags;
use crate::colors;
use crate::util::fs::FsCleaner;
use crate::util::progress_bar::ProgressBar;

pub async fn init_project(init_flags: InitFlags) -> Result<i32, AnyError> {
  if let Some(package) = &init_flags.package {
    return init_npm(InitNpmOptions {
      name: package,
      args: init_flags.package_args,
      yes: init_flags.yes,
    })
    .boxed_local()
    .await;
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

fn npm_name_to_create_package(name: &str) -> String {
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

struct InitNpmOptions<'a> {
  name: &'a str,
  args: Vec<String>,
  yes: bool,
}

async fn init_npm(options: InitNpmOptions<'_>) -> Result<i32, AnyError> {
  let script_name = npm_name_to_create_package(options.name);

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

  if !options.yes {
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
            return Ok(print_manual_usage(&script_name, &options.args));
          } else {
            break;
          }
        }
      }
    } else {
      return Ok(print_manual_usage(&script_name, &options.args));
    }
  }

  let temp_node_modules_parent_tempdir = create_temp_node_modules_parent_dir()
    .context("Failed creating temp directory for node_modules folder.")?;
  let temp_node_modules_parent_dir = temp_node_modules_parent_tempdir
    .path()
    .canonicalize()
    .ok()
    .map(deno_path_util::strip_unc_prefix)
    .unwrap_or_else(|| temp_node_modules_parent_tempdir.path().to_path_buf());
  let temp_node_modules_dir = temp_node_modules_parent_dir.join("node_modules");
  log::debug!(
    "Creating node_modules directory at: {}",
    temp_node_modules_dir.display()
  );

  let new_flags = Flags {
    permissions: PermissionFlags {
      allow_all: true,
      ..Default::default()
    },
    allow_scripts: PackagesAllowedScripts::All,
    argv: options.args,
    node_modules_dir: Some(NodeModulesDirMode::Auto),
    subcommand: DenoSubcommand::Run(RunFlags {
      script: script_name,
      ..Default::default()
    }),
    reload: true,
    internal: InternalFlags {
      lockfile_skip_write: true,
      root_node_modules_dir_override: Some(temp_node_modules_dir),
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
  drop(temp_node_modules_parent_tempdir); // explicit drop for clarity
  result
}

/// Creates a node_modules directory in a folder with the following format:
///
///   <tmp-dir>/deno_init_nm/<date>/<random-value>
///
/// Old folders are automatically deleted by this function.
fn create_temp_node_modules_parent_dir() -> Result<tempfile::TempDir, AnyError>
{
  let root_temp_folder = std::env::temp_dir().join("deno_init_nm");
  let today = chrono::Utc::now().date_naive();
  // remove any old/stale temp dirs
  if let Err(err) =
    attempt_temp_dir_garbage_collection(&root_temp_folder, today)
  {
    log::debug!("Failed init temp folder garbage collection: {:#?}", err);
  }
  let day_folder = root_temp_folder.join(folder_name_for_date(today));
  std::fs::create_dir_all(&day_folder)
    .with_context(|| format!("Failed creating '{}'", day_folder.display()))?;
  let temp_node_modules_parent_dir = tempfile::TempDir::new_in(&day_folder)?;
  // write a package.json to make this be considered a "node" project to deno
  let package_json_path =
    temp_node_modules_parent_dir.path().join("package.json");
  std::fs::write(&package_json_path, "{}").with_context(|| {
    format!("Failed creating '{}'", package_json_path.display())
  })?;
  Ok(temp_node_modules_parent_dir)
}

fn attempt_temp_dir_garbage_collection(
  root_temp_folder: &Path,
  utc_now: NaiveDate,
) -> Result<(), AnyError> {
  let previous_day_str = folder_name_for_date(
    utc_now
      .checked_sub_days(chrono::Days::new(1))
      .unwrap_or(utc_now),
  );
  let current_day_str = folder_name_for_date(utc_now);
  let next_day_str = folder_name_for_date(
    utc_now
      .checked_add_days(chrono::Days::new(1))
      .unwrap_or(utc_now),
  );
  let progress_bar =
    ProgressBar::new(crate::util::progress_bar::ProgressBarStyle::TextOnly);
  let update_guard = progress_bar.deferred_update_with_prompt(
    crate::util::progress_bar::ProgressMessagePrompt::Cleaning,
    "old temp node_modules folders...",
  );

  // remove any folders that aren't the current date +- 1 day
  let mut cleaner = FsCleaner::new(Some(update_guard));
  for entry in std::fs::read_dir(root_temp_folder)? {
    let Ok(entry) = entry else {
      continue;
    };
    if entry.file_name() != previous_day_str
      && entry.file_name() != current_day_str
      && entry.file_name() != next_day_str
      && let Err(err) = cleaner.rm_rf(&entry.path())
    {
      log::debug!(
        "Failed cleaning '{}': {:#?}",
        entry.file_name().display(),
        err
      );
    }
  }

  Ok(())
}

fn folder_name_for_date(date: chrono::NaiveDate) -> OsString {
  OsString::from(date.format("%Y-%m-%d").to_string())
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

  use test_util::TempDir;

  use super::attempt_temp_dir_garbage_collection;
  use crate::tools::init::npm_name_to_create_package;

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
  }

  #[test]
  fn test_attempt_temp_dir_garbage_collection() {
    let temp_dir = TempDir::new();
    let reference_date = chrono::NaiveDate::from_ymd_opt(2020, 5, 13).unwrap();
    temp_dir.path().join("0000-00-00").create_dir_all();
    temp_dir
      .path()
      .join("2020-05-01/sub_dir/sub")
      .create_dir_all();
    temp_dir
      .path()
      .join("2020-05-01/sub_dir/sub/test.txt")
      .write("");
    temp_dir.path().join("2020-05-02/sub_dir").create_dir_all();
    temp_dir.path().join("2020-05-11").create_dir_all();
    temp_dir.path().join("2020-05-12").create_dir_all();
    temp_dir.path().join("2020-05-13").create_dir_all();
    temp_dir.path().join("2020-05-14").create_dir_all();
    temp_dir.path().join("2020-05-15").create_dir_all();
    attempt_temp_dir_garbage_collection(
      temp_dir.path().as_path(),
      reference_date,
    )
    .unwrap();
    let mut entries = std::fs::read_dir(temp_dir.path())
      .unwrap()
      .map(|e| e.unwrap().file_name().into_string().unwrap())
      .collect::<Vec<_>>();
    entries.sort();
    // should only have the current day +- 1
    assert_eq!(
      entries,
      vec![
        "2020-05-12".to_string(),
        "2020-05-13".to_string(),
        "2020-05-14".to_string()
      ]
    );
  }
}
