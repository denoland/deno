// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use crate::itest;
use tempfile::TempDir;
use test_util as util;

#[test]
fn bundle_exports() {
  // First we have to generate a bundle of some module that has exports.
  let mod1 = util::testdata_path().join("subdir/mod1.ts");
  assert!(mod1.is_file());
  let t = TempDir::new().expect("tempdir fail");
  let bundle = t.path().join("mod1.bundle.js");
  let mut deno = util::deno_cmd()
    .current_dir(util::testdata_path())
    .arg("bundle")
    .arg(mod1)
    .arg(&bundle)
    .spawn()
    .expect("failed to spawn script");
  let status = deno.wait().expect("failed to wait for the child process");
  assert!(status.success());
  assert!(bundle.is_file());

  // Now we try to use that bundle from another module.
  let test = t.path().join("test.js");
  std::fs::write(
    &test,
    "
      import { printHello3 } from \"./mod1.bundle.js\";
      printHello3(); ",
  )
  .expect("error writing file");

  let output = util::deno_cmd()
    .current_dir(util::testdata_path())
    .arg("run")
    .arg(&test)
    .output()
    .expect("failed to spawn script");
  // check the output of the test.ts program.
  assert!(std::str::from_utf8(&output.stdout)
    .unwrap()
    .trim()
    .ends_with("Hello"));
  assert_eq!(output.stderr, b"");
}

#[test]
fn bundle_exports_no_check() {
  // First we have to generate a bundle of some module that has exports.
  let mod1 = util::testdata_path().join("subdir/mod1.ts");
  assert!(mod1.is_file());
  let t = TempDir::new().expect("tempdir fail");
  let bundle = t.path().join("mod1.bundle.js");
  let mut deno = util::deno_cmd()
    .current_dir(util::testdata_path())
    .arg("bundle")
    .arg("--no-check")
    .arg(mod1)
    .arg(&bundle)
    .spawn()
    .expect("failed to spawn script");
  let status = deno.wait().expect("failed to wait for the child process");
  assert!(status.success());
  assert!(bundle.is_file());

  // Now we try to use that bundle from another module.
  let test = t.path().join("test.js");
  std::fs::write(
    &test,
    "
      import { printHello3 } from \"./mod1.bundle.js\";
      printHello3(); ",
  )
  .expect("error writing file");

  let output = util::deno_cmd()
    .current_dir(util::testdata_path())
    .arg("run")
    .arg(&test)
    .output()
    .expect("failed to spawn script");
  // check the output of the test.ts program.
  assert!(std::str::from_utf8(&output.stdout)
    .unwrap()
    .trim()
    .ends_with("Hello"));
  assert_eq!(output.stderr, b"");
}

#[test]
fn bundle_circular() {
  // First we have to generate a bundle of some module that has exports.
  let circular1 = util::testdata_path().join("subdir/circular1.ts");
  assert!(circular1.is_file());
  let t = TempDir::new().expect("tempdir fail");
  let bundle = t.path().join("circular1.bundle.js");
  let mut deno = util::deno_cmd()
    .current_dir(util::testdata_path())
    .arg("bundle")
    .arg(circular1)
    .arg(&bundle)
    .spawn()
    .expect("failed to spawn script");
  let status = deno.wait().expect("failed to wait for the child process");
  assert!(status.success());
  assert!(bundle.is_file());

  let output = util::deno_cmd()
    .current_dir(util::testdata_path())
    .arg("run")
    .arg(&bundle)
    .output()
    .expect("failed to spawn script");
  // check the output of the the bundle program.
  assert!(std::str::from_utf8(&output.stdout)
    .unwrap()
    .trim()
    .ends_with("f2\nf1"));
  assert_eq!(output.stderr, b"");
}

#[test]
fn bundle_single_module() {
  // First we have to generate a bundle of some module that has exports.
  let single_module = util::testdata_path().join("subdir/single_module.ts");
  assert!(single_module.is_file());
  let t = TempDir::new().expect("tempdir fail");
  let bundle = t.path().join("single_module.bundle.js");
  let mut deno = util::deno_cmd()
    .current_dir(util::testdata_path())
    .arg("bundle")
    .arg(single_module)
    .arg(&bundle)
    .spawn()
    .expect("failed to spawn script");
  let status = deno.wait().expect("failed to wait for the child process");
  assert!(status.success());
  assert!(bundle.is_file());

  let output = util::deno_cmd()
    .current_dir(util::testdata_path())
    .arg("run")
    .arg(&bundle)
    .output()
    .expect("failed to spawn script");
  // check the output of the the bundle program.
  assert!(std::str::from_utf8(&output.stdout)
    .unwrap()
    .trim()
    .ends_with("Hello world!"));
  assert_eq!(output.stderr, b"");
}

#[test]
fn bundle_tla() {
  // First we have to generate a bundle of some module that has exports.
  let tla_import = util::testdata_path().join("subdir/tla.ts");
  assert!(tla_import.is_file());
  let t = tempfile::TempDir::new().expect("tempdir fail");
  let bundle = t.path().join("tla.bundle.js");
  let mut deno = util::deno_cmd()
    .current_dir(util::testdata_path())
    .arg("bundle")
    .arg(tla_import)
    .arg(&bundle)
    .spawn()
    .expect("failed to spawn script");
  let status = deno.wait().expect("failed to wait for the child process");
  assert!(status.success());
  assert!(bundle.is_file());

  // Now we try to use that bundle from another module.
  let test = t.path().join("test.js");
  std::fs::write(
    &test,
    "
      import { foo } from \"./tla.bundle.js\";
      console.log(foo); ",
  )
  .expect("error writing file");

  let output = util::deno_cmd()
    .current_dir(util::testdata_path())
    .arg("run")
    .arg(&test)
    .output()
    .expect("failed to spawn script");
  // check the output of the test.ts program.
  assert!(std::str::from_utf8(&output.stdout)
    .unwrap()
    .trim()
    .ends_with("Hello"));
  assert_eq!(output.stderr, b"");
}

#[test]
fn bundle_js() {
  // First we have to generate a bundle of some module that has exports.
  let mod6 = util::testdata_path().join("subdir/mod6.js");
  assert!(mod6.is_file());
  let t = TempDir::new().expect("tempdir fail");
  let bundle = t.path().join("mod6.bundle.js");
  let mut deno = util::deno_cmd()
    .current_dir(util::testdata_path())
    .arg("bundle")
    .arg(mod6)
    .arg(&bundle)
    .spawn()
    .expect("failed to spawn script");
  let status = deno.wait().expect("failed to wait for the child process");
  assert!(status.success());
  assert!(bundle.is_file());

  let output = util::deno_cmd()
    .current_dir(util::testdata_path())
    .arg("run")
    .arg(&bundle)
    .output()
    .expect("failed to spawn script");
  // check that nothing went to stderr
  assert_eq!(output.stderr, b"");
}

#[test]
fn bundle_dynamic_import() {
  let _g = util::http_server();
  let dynamic_import = util::testdata_path().join("bundle_dynamic_import.ts");
  assert!(dynamic_import.is_file());
  let t = TempDir::new().expect("tempdir fail");
  let bundle = t.path().join("bundle_dynamic_import.bundle.js");
  let mut deno = util::deno_cmd()
    .current_dir(util::testdata_path())
    .arg("bundle")
    .arg(dynamic_import)
    .arg(&bundle)
    .spawn()
    .expect("failed to spawn script");
  let status = deno.wait().expect("failed to wait for the child process");
  assert!(status.success());
  assert!(bundle.is_file());

  let output = util::deno_cmd()
    .current_dir(util::testdata_path())
    .arg("run")
    .arg("--allow-net")
    .arg("--quiet")
    .arg(&bundle)
    .output()
    .expect("failed to spawn script");
  // check the output of the test.ts program.
  assert!(std::str::from_utf8(&output.stdout)
    .unwrap()
    .trim()
    .ends_with("Hello"));
  assert_eq!(output.stderr, b"");
}

#[test]
fn bundle_import_map() {
  let import = util::testdata_path().join("bundle_im.ts");
  let import_map_path = util::testdata_path().join("bundle_im.json");
  assert!(import.is_file());
  let t = TempDir::new().expect("tempdir fail");
  let bundle = t.path().join("import_map.bundle.js");
  let mut deno = util::deno_cmd()
    .current_dir(util::testdata_path())
    .arg("bundle")
    .arg("--import-map")
    .arg(import_map_path)
    .arg(import)
    .arg(&bundle)
    .spawn()
    .expect("failed to spawn script");
  let status = deno.wait().expect("failed to wait for the child process");
  assert!(status.success());
  assert!(bundle.is_file());

  // Now we try to use that bundle from another module.
  let test = t.path().join("test.js");
  std::fs::write(
    &test,
    "
      import { printHello3 } from \"./import_map.bundle.js\";
      printHello3(); ",
  )
  .expect("error writing file");

  let output = util::deno_cmd()
    .current_dir(util::testdata_path())
    .arg("run")
    .arg(&test)
    .output()
    .expect("failed to spawn script");
  // check the output of the test.ts program.
  assert!(std::str::from_utf8(&output.stdout)
    .unwrap()
    .trim()
    .ends_with("Hello"));
  assert_eq!(output.stderr, b"");
}

#[test]
fn bundle_import_map_no_check() {
  let import = util::testdata_path().join("bundle_im.ts");
  let import_map_path = util::testdata_path().join("bundle_im.json");
  assert!(import.is_file());
  let t = TempDir::new().expect("tempdir fail");
  let bundle = t.path().join("import_map.bundle.js");
  let mut deno = util::deno_cmd()
    .current_dir(util::testdata_path())
    .arg("bundle")
    .arg("--no-check")
    .arg("--import-map")
    .arg(import_map_path)
    .arg(import)
    .arg(&bundle)
    .spawn()
    .expect("failed to spawn script");
  let status = deno.wait().expect("failed to wait for the child process");
  assert!(status.success());
  assert!(bundle.is_file());

  // Now we try to use that bundle from another module.
  let test = t.path().join("test.js");
  std::fs::write(
    &test,
    "
      import { printHello3 } from \"./import_map.bundle.js\";
      printHello3(); ",
  )
  .expect("error writing file");

  let output = util::deno_cmd()
    .current_dir(util::testdata_path())
    .arg("run")
    .arg(&test)
    .output()
    .expect("failed to spawn script");
  // check the output of the test.ts program.
  assert!(std::str::from_utf8(&output.stdout)
    .unwrap()
    .trim()
    .ends_with("Hello"));
  assert_eq!(output.stderr, b"");
}

#[test]
fn bundle_json_module() {
  // First we have to generate a bundle of some module that has exports.
  let mod7 = util::testdata_path().join("subdir/mod7.js");
  assert!(mod7.is_file());
  let t = TempDir::new().expect("tempdir fail");
  let bundle = t.path().join("mod7.bundle.js");
  let mut deno = util::deno_cmd()
    .current_dir(util::testdata_path())
    .arg("bundle")
    .arg(mod7)
    .arg(&bundle)
    .spawn()
    .expect("failed to spawn script");
  let status = deno.wait().expect("failed to wait for the child process");
  assert!(status.success());
  assert!(bundle.is_file());

  let output = util::deno_cmd()
    .current_dir(util::testdata_path())
    .arg("run")
    .arg(&bundle)
    .output()
    .expect("failed to spawn script");
  // check that nothing went to stderr
  assert_eq!(output.stderr, b"");
  // ensure the output looks right
  assert!(String::from_utf8(output.stdout)
    .unwrap()
    .contains("with space"));
}

#[test]
fn bundle_json_module_escape_sub() {
  // First we have to generate a bundle of some module that has exports.
  let mod8 = util::testdata_path().join("subdir/mod8.js");
  assert!(mod8.is_file());
  let t = TempDir::new().expect("tempdir fail");
  let bundle = t.path().join("mod8.bundle.js");
  let mut deno = util::deno_cmd()
    .current_dir(util::testdata_path())
    .arg("bundle")
    .arg(mod8)
    .arg(&bundle)
    .spawn()
    .expect("failed to spawn script");
  let status = deno.wait().expect("failed to wait for the child process");
  assert!(status.success());
  assert!(bundle.is_file());

  let output = util::deno_cmd()
    .current_dir(util::testdata_path())
    .arg("run")
    .arg(&bundle)
    .output()
    .expect("failed to spawn script");
  // check that nothing went to stderr
  assert_eq!(output.stderr, b"");
  // make sure the output looks right and the escapes were effective
  assert!(String::from_utf8(output.stdout)
    .unwrap()
    .contains("${globalThis}`and string literal`"));
}

itest!(lock_check_err_with_bundle {
  args: "bundle --lock=lock_check_err_with_bundle.json http://127.0.0.1:4545/subdir/mod1.ts",
  output: "lock_check_err_with_bundle.out",
  exit_code: 10,
  http_server: true,
});

itest!(bundle {
  args: "bundle subdir/mod1.ts",
  output: "bundle.test.out",
});

itest!(bundle_jsx {
  args: "bundle jsx_import_from_ts.ts",
  output: "bundle_jsx.out",
});

itest!(error_027_bundle_with_bare_import {
  args: "bundle error_027_bundle_with_bare_import.ts",
  output: "error_027_bundle_with_bare_import.ts.out",
  exit_code: 1,
});

itest!(ts_decorators_bundle {
  args: "bundle ts_decorators_bundle.ts",
  output: "ts_decorators_bundle.out",
});

itest!(bundle_export_specifier_with_alias {
  args: "bundle bundle/file_tests-fixture16.ts",
  output: "bundle/fixture16.out",
});

itest!(bundle_ignore_directives {
  args: "bundle subdir/mod1.ts",
  output: "bundle_ignore_directives.test.out",
});
