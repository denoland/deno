// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use deno_core::serde_json;
use deno_core::serde_json::json;
use pretty_assertions::assert_eq;
use std::fs;
use std::path::PathBuf;
use std::process::Stdio;
use tempfile::TempDir;
use test_util as util;
use util::http_server;

#[test]
fn output_dir_exists() {
  let t = TempDir::new().unwrap();
  let vendor_dir = t.path().join("vendor");
  fs::write(t.path().join("mod.ts"), "").unwrap();
  fs::create_dir_all(&vendor_dir).unwrap();
  fs::write(vendor_dir.join("mod.ts"), "").unwrap();

  let deno = util::deno_cmd()
    .current_dir(t.path())
    .env("NO_COLOR", "1")
    .arg("vendor")
    .arg("mod.ts")
    .stderr(Stdio::piped())
    .spawn()
    .unwrap();
  let output = deno.wait_with_output().unwrap();
  assert_eq!(
    String::from_utf8_lossy(&output.stderr).trim(),
    concat!(
      "error: Output directory was not empty. Please specify an empty ",
      "directory or use --force to ignore this error and potentially ",
      "overwrite its contents.",
    ),
  );
  assert!(!output.status.success());

  // ensure it errors when using the `--output` arg too
  let deno = util::deno_cmd()
    .current_dir(t.path())
    .env("NO_COLOR", "1")
    .arg("vendor")
    .arg("--output")
    .arg("vendor")
    .arg("mod.ts")
    .stderr(Stdio::piped())
    .spawn()
    .unwrap();
  let output = deno.wait_with_output().unwrap();
  assert_eq!(
    String::from_utf8_lossy(&output.stderr).trim(),
    concat!(
      "error: Output directory was not empty. Please specify an empty ",
      "directory or use --force to ignore this error and potentially ",
      "overwrite its contents.",
    ),
  );
  assert!(!output.status.success());

  // now use `--force`
  let status = util::deno_cmd()
    .current_dir(t.path())
    .env("NO_COLOR", "1")
    .arg("vendor")
    .arg("mod.ts")
    .arg("--force")
    .spawn()
    .unwrap()
    .wait()
    .unwrap();
  assert!(status.success());
}

#[test]
fn import_map_output_dir() {
  let t = TempDir::new().unwrap();
  let vendor_dir = t.path().join("vendor");
  fs::write(t.path().join("mod.ts"), "").unwrap();
  fs::create_dir_all(&vendor_dir).unwrap();
  let import_map_path = vendor_dir.join("import_map.json");
  fs::write(
    &import_map_path,
    "{ \"imports\": { \"https://localhost/\": \"./localhost/\" }}",
  )
  .unwrap();

  let deno = util::deno_cmd()
    .current_dir(t.path())
    .env("NO_COLOR", "1")
    .arg("vendor")
    .arg("--force")
    .arg("--import-map")
    .arg(import_map_path)
    .arg("mod.ts")
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()
    .unwrap();
  let output = deno.wait_with_output().unwrap();
  assert_eq!(
    String::from_utf8_lossy(&output.stderr).trim(),
    "error: Using an import map found in the output directory is not supported.",
  );
  assert!(!output.status.success());
}

#[test]
fn standard_test() {
  let _server = http_server();
  let t = TempDir::new().unwrap();
  let vendor_dir = t.path().join("vendor2");
  fs::write(
    t.path().join("my_app.ts"),
    "import {Logger} from 'http://localhost:4545/vendor/query_reexport.ts?testing'; new Logger().log('outputted');",
  ).unwrap();

  let deno = util::deno_cmd()
    .current_dir(t.path())
    .arg("vendor")
    .arg("my_app.ts")
    .arg("--output")
    .arg("vendor2")
    .env("NO_COLOR", "1")
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()
    .unwrap();
  let output = deno.wait_with_output().unwrap();
  assert_eq!(
    String::from_utf8_lossy(&output.stderr).trim(),
    format!(
      concat!(
        "Download http://localhost:4545/vendor/query_reexport.ts?testing\n",
        "Download http://localhost:4545/vendor/logger.ts?test\n",
        "{}",
      ),
      success_text("2 modules", "vendor2", "my_app.ts"),
    )
  );
  assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "");
  assert!(output.status.success());

  assert!(vendor_dir.exists());
  assert!(!t.path().join("vendor").exists());
  let import_map: serde_json::Value = serde_json::from_str(
    &fs::read_to_string(vendor_dir.join("import_map.json")).unwrap(),
  )
  .unwrap();
  assert_eq!(
    import_map,
    json!({
      "imports": {
        "http://localhost:4545/": "./localhost_4545/",
        "http://localhost:4545/vendor/query_reexport.ts?testing": "./localhost_4545/vendor/query_reexport.ts",
      },
      "scopes": {
        "./localhost_4545/": {
          "./localhost_4545/vendor/logger.ts?test": "./localhost_4545/vendor/logger.ts"
        }
      }
    }),
  );

  // try running the output with `--no-remote`
  let deno = util::deno_cmd()
    .current_dir(t.path())
    .env("NO_COLOR", "1")
    .arg("run")
    .arg("--no-remote")
    .arg("--no-check")
    .arg("--import-map")
    .arg("vendor2/import_map.json")
    .arg("my_app.ts")
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()
    .unwrap();
  let output = deno.wait_with_output().unwrap();
  assert_eq!(String::from_utf8_lossy(&output.stderr).trim(), "");
  assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "outputted");
  assert!(output.status.success());
}

#[test]
fn remote_module_test() {
  let _server = http_server();
  let t = TempDir::new().unwrap();
  let vendor_dir = t.path().join("vendor");

  let deno = util::deno_cmd()
    .current_dir(t.path())
    .env("NO_COLOR", "1")
    .arg("vendor")
    .arg("http://localhost:4545/vendor/query_reexport.ts")
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()
    .unwrap();
  let output = deno.wait_with_output().unwrap();
  assert_eq!(
    String::from_utf8_lossy(&output.stderr).trim(),
    format!(
      concat!(
        "Download http://localhost:4545/vendor/query_reexport.ts\n",
        "Download http://localhost:4545/vendor/logger.ts?test\n",
        "{}",
      ),
      success_text("2 modules", "vendor/", "main.ts"),
    )
  );
  assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "");
  assert!(output.status.success());
  assert!(vendor_dir.exists());
  assert!(vendor_dir
    .join("localhost_4545/vendor/query_reexport.ts")
    .exists());
  assert!(vendor_dir.join("localhost_4545/vendor/logger.ts").exists());
  let import_map: serde_json::Value = serde_json::from_str(
    &fs::read_to_string(vendor_dir.join("import_map.json")).unwrap(),
  )
  .unwrap();
  assert_eq!(
    import_map,
    json!({
      "imports": {
        "http://localhost:4545/": "./localhost_4545/",
      },
      "scopes": {
        "./localhost_4545/": {
          "./localhost_4545/vendor/logger.ts?test": "./localhost_4545/vendor/logger.ts"
        }
      }
    }),
  );
}

#[test]
fn existing_import_map() {
  let _server = http_server();
  let t = TempDir::new().unwrap();
  let vendor_dir = t.path().join("vendor");
  fs::write(
    t.path().join("mod.ts"),
    "import {Logger} from 'http://localhost:4545/vendor/logger.ts';",
  )
  .unwrap();
  fs::write(
    t.path().join("imports.json"),
    r#"{ "imports": { "http://localhost:4545/vendor/": "./logger/" } }"#,
  )
  .unwrap();
  fs::create_dir(t.path().join("logger")).unwrap();
  fs::write(t.path().join("logger/logger.ts"), "export class Logger {}")
    .unwrap();

  let status = util::deno_cmd()
    .current_dir(t.path())
    .arg("vendor")
    .arg("mod.ts")
    .arg("--import-map")
    .arg("imports.json")
    .spawn()
    .unwrap()
    .wait()
    .unwrap();
  assert!(status.success());
  // it should not have found any remote dependencies because
  // the provided import map mapped it to a local directory
  assert!(!vendor_dir.join("import_map.json").exists());
}

#[test]
fn dynamic_import() {
  let _server = http_server();
  let t = TempDir::new().unwrap();
  let vendor_dir = t.path().join("vendor");
  fs::write(
    t.path().join("mod.ts"),
    "import {Logger} from 'http://localhost:4545/vendor/dynamic.ts'; new Logger().log('outputted');",
  ).unwrap();

  let status = util::deno_cmd()
    .current_dir(t.path())
    .arg("vendor")
    .arg("mod.ts")
    .spawn()
    .unwrap()
    .wait()
    .unwrap();
  assert!(status.success());
  let import_map: serde_json::Value = serde_json::from_str(
    &fs::read_to_string(vendor_dir.join("import_map.json")).unwrap(),
  )
  .unwrap();
  assert_eq!(
    import_map,
    json!({
      "imports": {
        "http://localhost:4545/": "./localhost_4545/",
      }
    }),
  );

  // try running the output with `--no-remote`
  let deno = util::deno_cmd()
    .current_dir(t.path())
    .env("NO_COLOR", "1")
    .arg("run")
    .arg("--allow-read=.")
    .arg("--no-remote")
    .arg("--no-check")
    .arg("--import-map")
    .arg("vendor/import_map.json")
    .arg("mod.ts")
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()
    .unwrap();
  let output = deno.wait_with_output().unwrap();
  assert_eq!(String::from_utf8_lossy(&output.stderr).trim(), "");
  assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "outputted");
  assert!(output.status.success());
}

#[test]
fn dynamic_non_analyzable_import() {
  let _server = http_server();
  let t = TempDir::new().unwrap();
  fs::write(
    t.path().join("mod.ts"),
    "import {Logger} from 'http://localhost:4545/vendor/dynamic_non_analyzable.ts'; new Logger().log('outputted');",
  ).unwrap();

  let deno = util::deno_cmd()
    .current_dir(t.path())
    .env("NO_COLOR", "1")
    .arg("vendor")
    .arg("--reload")
    .arg("mod.ts")
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()
    .unwrap();
  let output = deno.wait_with_output().unwrap();
  // todo(https://github.com/denoland/deno_graph/issues/138): it should warn about
  // how it couldn't analyze the dynamic import
  assert_eq!(
    String::from_utf8_lossy(&output.stderr).trim(),
    format!(
      "Download http://localhost:4545/vendor/dynamic_non_analyzable.ts\n{}",
      success_text("1 module", "vendor/", "mod.ts"),
    )
  );
  assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "");
  assert!(output.status.success());
}

fn success_text(module_count: &str, dir: &str, entry_point: &str) -> String {
  format!(
    concat!(
      "Vendored {} into {} directory.\n\n",
      "To use vendored modules, specify the `--import-map` flag when invoking deno subcommands:\n",
      "  deno run -A --import-map {} {}"
    ),
    module_count,
    dir,
    PathBuf::from(dir).join("import_map.json").display(),
    entry_point,
  )
}
