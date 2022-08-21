// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use crate::itest;
use deno_core::url::Url;
use test_util as util;

/// Tests in this file should use `std_file_url` to override `DENO_NODE_COMPAT_URL`
/// env variable. This speeds up tests significantly as they no longer
/// download contents of `deno_std` from `https://deno.land` in each test.

/// Return a file URL pointing to "std" submodule
/// in "test_util" directory. It must have a trailing slash.
fn std_file_url() -> String {
  let u = Url::from_directory_path(util::std_path()).unwrap();
  u.to_string()
}

itest!(globals {
  args: "run --compat --no-check --unstable --allow-read --allow-env compat/globals.ts",
  output: "compat/globals.out",
  envs: vec![("DENO_NODE_COMPAT_URL".to_string(), std_file_url())],
});

itest!(fs_promises {
  args: "run --compat --no-check --unstable -A compat/fs_promises.mjs",
  output: "compat/fs_promises.out",
  envs: vec![("DENO_NODE_COMPAT_URL".to_string(), std_file_url())],
});

// https://github.com/denoland/deno/issues/12494
itest_flaky!(node_prefix_fs_promises {
  args: "run --compat --no-check --unstable -A compat/node_fs_promises.mjs",
  output: "compat/fs_promises.out",
  envs: vec![("DENO_NODE_COMPAT_URL".to_string(), std_file_url())],
});

itest!(compat_with_import_map_and_https_imports {
  args: "run --quiet --no-check --compat --unstable -A --import-map=compat/import_map.json compat/import_map_https_imports.mjs",
  output: "compat/import_map_https_imports.out",
  envs: vec![("DENO_NODE_COMPAT_URL".to_string(), std_file_url())],
});

itest!(compat_dyn_import_rejects_with_node_compatible_error {
  args:
    "run --quiet --no-check --compat --unstable -A compat/dyn_import_reject.js",
  output: "compat/dyn_import_reject.out",
  envs: vec![("DENO_NODE_COMPAT_URL".to_string(), std_file_url())],
});

itest!(import_esm_from_cjs {
  args:
    "run --compat --unstable -A --quiet compat/import_esm_from_cjs/index.js",
  output_str: Some("function\n"),
  envs: vec![("DENO_NODE_COMPAT_URL".to_string(), std_file_url())],
});

itest!(test_runner_cjs {
  args: "test --compat --unstable -A --quiet compat/test_runner/cjs.js",
  exit_code: 1,
  output: "compat/test_runner/cjs.out",
  envs: vec![("DENO_NODE_COMPAT_URL".to_string(), std_file_url())],
});

itest!(test_runner_esm {
  args: "test --compat --unstable -A --quiet compat/test_runner/esm.mjs",
  exit_code: 1,
  output: "compat/test_runner/esm.out",
  envs: vec![("DENO_NODE_COMPAT_URL".to_string(), std_file_url())],
});

// Top level assertion test mostly just make sure that the test runner finishes correctly on compat mode
// when there is no tests
itest!(top_level_assertion_cjs {
  args: "test --compat --unstable -A --quiet compat/test_runner/top_level_assertion_cjs.js",
	exit_code: 0,
  output: "compat/test_runner/top_level_assertion_cjs.out",
  envs: vec![("DENO_NODE_COMPAT_URL".to_string(), std_file_url())],
});

itest!(top_level_assertion_esm {
  args: "test --compat --unstable -A --quiet compat/test_runner/top_level_assertion_esm.mjs",
	exit_code: 0,
  output: "compat/test_runner/top_level_assertion_esm.out",
  envs: vec![("DENO_NODE_COMPAT_URL".to_string(), std_file_url())],
});

itest!(top_level_fail_cjs {
  args: "test --compat --unstable -A --quiet compat/test_runner/top_level_fail_cjs.js",
	exit_code: 1,
  output: "compat/test_runner/top_level_fail_cjs.out",
  envs: vec![("DENO_NODE_COMPAT_URL".to_string(), std_file_url())],
});

itest!(top_level_fail_esm {
  args: "test --compat --unstable -A --quiet compat/test_runner/top_level_fail_esm.mjs",
	exit_code: 1,
  output: "compat/test_runner/top_level_fail_esm.out",
  envs: vec![("DENO_NODE_COMPAT_URL".to_string(), std_file_url())],
});

itest!(compat_worker {
  args: "run --compat --unstable -A --quiet --no-check compat/worker/worker_test.mjs",
  output: "compat/worker/worker_test.out",
  envs: vec![("DENO_NODE_COMPAT_URL".to_string(), std_file_url())],
});

itest!(cjs_esm_interop {
  args:
    "run --compat --unstable -A --quiet --no-check compat/import_cjs_from_esm/main.mjs",
  output: "compat/import_cjs_from_esm.out",
  envs: vec![("DENO_NODE_COMPAT_URL".to_string(), std_file_url())],
});

itest!(cjs_esm_interop_dynamic {
  args:
    "run --compat --unstable -A --quiet --no-check compat/import_cjs_from_esm/main_dynamic.mjs",
  output: "compat/import_cjs_from_esm.out",
  envs: vec![("DENO_NODE_COMPAT_URL".to_string(), std_file_url())],
});

#[test]
fn globals_in_repl() {
  let (out, _err) = util::run_and_collect_output_with_args(
    true,
    vec!["repl", "--compat", "--unstable", "--no-check", "--quiet"],
    Some(vec!["global.window == window"]),
    Some(vec![("DENO_NODE_COMPAT_URL".to_string(), std_file_url())]),
    false,
  );
  assert!(out.contains("true"));
}

#[test]
fn require_in_repl() {
  let (out, _err) = util::run_and_collect_output_with_args(
    true,
    vec!["repl", "--compat", "--unstable", "--quiet"],
    Some(vec![
      "const foo = require('./compat/import_esm_from_cjs/index');",
    ]),
    Some(vec![("DENO_NODE_COMPAT_URL".to_string(), std_file_url())]),
    false,
  );
  assert!(out.contains("function"));
}

#[test]
fn node_compat_url() {
  let (out, err) = util::run_and_collect_output_with_args(
    false,
    vec!["repl", "--compat", "--unstable", "--no-check", "--quiet"],
    None,
    Some(vec![(
      "DENO_NODE_COMPAT_URL".to_string(),
      "file:///non_existent/".to_string(),
    )]),
    false,
  );
  assert!(out.is_empty());
  assert!(!err.is_empty());
  assert!(err.contains("file:///non_existent/node/global.ts"));
}

#[test]
fn native_modules_as_global_vars() {
  let (out, _err) = util::run_and_collect_output_with_args(
    true,
    vec!["repl", "--compat", "--unstable", "--quiet"],
    Some(vec!["if(cluster && v8 && sys) { true } else { false }"]),
    Some(vec![("DENO_NODE_COMPAT_URL".to_string(), std_file_url())]),
    false,
  );
  assert!(out.contains("true"));
}

#[ignore] // todo(dsherret): re-enable
#[test]
fn ext_node_cjs_execution() {
  let (out, _err) = util::run_and_collect_output_with_args(
    true,
    vec![
      "run",
      "-A",
      "--unstable",
      "--quiet",
      "commonjs/init.js",
      "./example.js",
    ],
    None,
    Some(vec![("DENO_NODE_COMPAT_URL".to_string(), std_file_url())]),
    false,
  );
  assert!(out.contains("{ hello: \"world\" }"));
}
