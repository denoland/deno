// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use deno_core::url::Url;
use test_util as util;

// NOTE: It's possible to automatically update the npm registry data in the test server
// by setting the DENO_TEST_UTIL_UPDATE_NPM=1 environment variable.

itest!(esm_module {
  args: "run --allow-read npm/esm/main.js",
  output: "npm/esm/main.out",
  envs: env_vars(),
  http_server: true,
});

itest!(esm_module_eval {
  args_vec: vec![
    "eval",
    "import chalk from 'npm:chalk@5'; console.log(chalk.green('chalk esm loads'));",
  ],
  output: "npm/esm/main.out",
  envs: env_vars(),
  http_server: true,
});

itest!(esm_module_deno_test {
  args: "test --allow-read npm/esm/test.js",
  output: "npm/esm/test.out",
  envs: env_vars(),
  http_server: true,
});

itest!(cjs_with_deps {
  args: "run --allow-read --unstable npm/cjs_with_deps/main.js",
  output: "npm/cjs_with_deps/main.out",
  envs: env_vars(),
  http_server: true,
});

itest!(cjs_sub_path {
  args: "run --allow-read --unstable npm/cjs_sub_path/main.js",
  output: "npm/cjs_sub_path/main.out",
  envs: env_vars(),
  http_server: true,
});

itest!(dynamic_import {
  args: "run --allow-read --unstable npm/dynamic_import/main.ts",
  output: "npm/dynamic_import/main.out",
  envs: env_vars(),
  http_server: true,
});

itest!(import_map {
  args: "run --allow-read --unstable --import-map npm/import_map/import_map.json npm/import_map/main.js",
  output: "npm/import_map/main.out",
  envs: env_vars(),
  http_server: true,
});

#[test]
fn parallel_downloading() {
  let (out, _err) = util::run_and_collect_output_with_args(
    true,
    vec![
      "run",
      "--allow-read",
      "--unstable",
      "npm/cjs_with_deps/main.js",
    ],
    None,
    // don't use the sync env var
    Some(env_vars_no_sync_download()),
    true,
  );
  assert!(out.contains("chalk cjs loads"));
}

#[test]
fn ensure_registry_files_local() {
  // ensures the registry files all point at local tarballs
  let registry_dir_path = util::testdata_path().join("npm").join("registry");
  for entry in std::fs::read_dir(&registry_dir_path).unwrap() {
    let entry = entry.unwrap();
    if entry.metadata().unwrap().is_dir() {
      let registry_json_path = registry_dir_path
        .join(entry.file_name())
        .join("registry.json");
      let file_text = std::fs::read_to_string(&registry_json_path).unwrap();
      if file_text.contains("https://registry.npmjs.org/") {
        panic!(
          "file {} contained a reference to the npm registry",
          registry_json_path.display(),
        );
      }
    }
  }
}

fn std_file_url() -> String {
  let u = Url::from_directory_path(util::std_path()).unwrap();
  u.to_string()
}

fn env_vars_no_sync_download() -> Vec<(String, String)> {
  vec![
    ("DENO_NODE_COMPAT_URL".to_string(), std_file_url()),
    (
      "DENO_NPM_REGISTRY".to_string(),
      "http://localhost:4545/npm/registry/".to_string(),
    ),
  ]
}

fn env_vars() -> Vec<(String, String)> {
  let mut env_vars = env_vars_no_sync_download();
  env_vars.push((
    // make downloads determinstic
    "DENO_UNSTABLE_NPM_SYNC_DOWNLOAD".to_string(),
    "1".to_string(),
  ));
  env_vars
}
