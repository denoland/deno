// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use deno_core::serde_json;
use deno_core::serde_json::json;
use pretty_assertions::assert_eq;
use std::fs;
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
    format!("error: Directory {} was not empty. Please provide an empty directory or use --force to ignore this error and potentially overwrite its contents.", vendor_dir.display()),
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
    format!("error: Directory {} was not empty. Please provide an empty directory or use --force to ignore this error and potentially overwrite its contents.", vendor_dir.display()),
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
  fs::write(t.path().join("mod.ts"), "import {Logger} from 'http://localhost:4545/vendor/logger/mod.ts?testing'; new Logger().log('outputted');").unwrap();

  let status = util::deno_cmd()
    .current_dir(t.path())
    .arg("vendor")
    .arg("mod.ts")
    .arg("--output")
    .arg("vendor2")
    .spawn()
    .unwrap()
    .wait()
    .unwrap();
  assert!(status.success());
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
        "http://localhost:4545/vendor/logger/mod.ts?testing": "./localhost_4545/vendor/logger/mod.ts",
      },
      "scopes": {
        "./localhost_4545/": {
          "./localhost_4545/vendor/logger/logger.ts?test": "./localhost_4545/vendor/logger/logger.ts"
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
    .arg("mod.ts")
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()
    .unwrap();
  let output = deno.wait_with_output().unwrap();
  assert_eq!(String::from_utf8_lossy(&output.stderr).trim(), "",);
  assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "outputted",);
  assert!(output.status.success());
}

#[test]
fn remote_module_test() {
  let _server = http_server();
  let t = TempDir::new().unwrap();
  let vendor_dir = t.path().join("vendor");

  let status = util::deno_cmd()
    .current_dir(t.path())
    .arg("vendor")
    .arg("http://localhost:4545/vendor/logger/mod.ts")
    .spawn()
    .unwrap()
    .wait()
    .unwrap();
  assert!(status.success());
  assert!(vendor_dir.exists());
  assert!(vendor_dir
    .join("localhost_4545/vendor/logger/mod.ts")
    .exists());
  assert!(vendor_dir
    .join("localhost_4545/vendor/logger/logger.ts")
    .exists());
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
          "./localhost_4545/vendor/logger/logger.ts?test": "./localhost_4545/vendor/logger/logger.ts"
        }
      }
    }),
  );
}
