// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use pretty_assertions::assert_eq;
use std::fs;
use std::process::Stdio;
use tempfile::TempDir;
use test_util as util;

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
  println!("{}", String::from_utf8_lossy(&output.stdout).trim());
  assert_eq!(
    String::from_utf8_lossy(&output.stderr).trim(),
    "error: Using an import map found in the output directory is not supported.",
  );
  assert!(!output.status.success());
}
