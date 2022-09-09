// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use deno_core::url::Url;
use std::process::Stdio;
use test_util as util;
use util::assert_contains;
use util::http_server;

// NOTE: See how to make test npm packages at ../testdata/npm/README.md

itest!(esm_module {
  args: "run --allow-read --allow-env --unstable npm/esm/main.js",
  output: "npm/esm/main.out",
  envs: env_vars(),
  http_server: true,
});

itest!(esm_module_eval {
  args_vec: vec![
    "eval",
    "--unstable",
    "import chalk from 'npm:chalk@5'; console.log(chalk.green('chalk esm loads'));",
  ],
  output: "npm/esm/main.out",
  envs: env_vars(),
  http_server: true,
});

itest!(esm_module_deno_test {
  args: "test --allow-read --allow-env --unstable npm/esm/test.js",
  output: "npm/esm/test.out",
  envs: env_vars(),
  http_server: true,
});

itest!(cjs_with_deps {
  args: "run --allow-read --allow-env --unstable npm/cjs_with_deps/main.js",
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

itest!(cjs_local_global_decls {
  args: "run --allow-read --unstable npm/cjs_local_global_decls/main.ts",
  output: "npm/cjs_local_global_decls/main.out",
  envs: env_vars(),
  http_server: true,
});

itest!(cjs_reexport_collision {
  args: "run --unstable -A --quiet npm/cjs_reexport_collision/main.ts",
  output: "npm/cjs_reexport_collision/main.out",
  envs: env_vars(),
  http_server: true,
});

itest!(translate_cjs_to_esm {
  args: "run --unstable -A --quiet npm/translate_cjs_to_esm/main.js",
  output: "npm/translate_cjs_to_esm/main.out",
  envs: env_vars(),
  http_server: true,
});

itest!(compare_globals {
  args: "run --allow-read --unstable npm/compare_globals/main.js",
  output: "npm/compare_globals/main.out",
  envs: env_vars(),
  http_server: true,
});

itest!(conditional_exports {
  args: "run --allow-read --unstable npm/conditional_exports/main.js",
  output: "npm/conditional_exports/main.out",
  envs: env_vars(),
  http_server: true,
});

itest!(dual_cjs_esm {
  args: "run --unstable -A --quiet npm/dual_cjs_esm/main.ts",
  output: "npm/dual_cjs_esm/main.out",
  envs: env_vars(),
  http_server: true,
});

// FIXME(bartlomieju): npm: specifiers are not handled in dynamic imports
// at the moment
// itest!(dynamic_import {
//   args: "run --allow-read --allow-env --unstable npm/dynamic_import/main.ts",
//   output: "npm/dynamic_import/main.out",
//   envs: env_vars(),
//   http_server: true,
// });

itest!(env_var_re_export_dev {
  args: "run --allow-read --allow-env --unstable --quiet npm/env_var_re_export/main.js",
  output_str: Some("dev\n"),
  envs: env_vars(),
  http_server: true,
});

itest!(env_var_re_export_prod {
  args: "run --allow-read --allow-env --unstable --quiet npm/env_var_re_export/main.js",
  output_str: Some("prod\n"),
  envs: {
    let mut vars = env_vars();
    vars.push(("NODE_ENV".to_string(), "production".to_string()));
    vars
  },
  http_server: true,
});

itest!(cached_only {
  args: "run --cached-only --unstable npm/cached_only/main.ts",
  output: "npm/cached_only/main.out",
  envs: env_vars(),
  exit_code: 1,
});

itest!(no_unstable {
  args: "run npm/no_unstable/main.ts",
  output: "npm/no_unstable/main.out",
  envs: env_vars(),
  exit_code: 1,
});

itest!(import_map {
  args: "run --allow-read --allow-env --unstable --import-map npm/import_map/import_map.json npm/import_map/main.js",
  output: "npm/import_map/main.out",
  envs: env_vars(),
  http_server: true,
});

itest!(sub_paths {
  args: "run --unstable -A --quiet npm/sub_paths/main.jsx",
  output: "npm/sub_paths/main.out",
  envs: env_vars(),
  http_server: true,
});

itest!(tarball_with_global_header {
  args: "run --unstable -A --quiet npm/tarball_with_global_header/main.js",
  output: "npm/tarball_with_global_header/main.out",
  envs: env_vars(),
  http_server: true,
});

itest!(nonexistent_file {
  args: "run --unstable -A --quiet npm/nonexistent_file/main.js",
  output: "npm/nonexistent_file/main.out",
  envs: env_vars(),
  http_server: true,
  exit_code: 1,
});

#[test]
fn parallel_downloading() {
  let (out, _err) = util::run_and_collect_output_with_args(
    true,
    vec![
      "run",
      "--allow-read",
      "--unstable",
      "--allow-env",
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
fn cached_only_after_first_run() {
  let _server = http_server();

  let deno_dir = util::new_deno_dir();

  let deno = util::deno_cmd_with_deno_dir(&deno_dir)
    .current_dir(util::testdata_path())
    .arg("run")
    .arg("--unstable")
    .arg("--allow-read")
    .arg("--allow-env")
    .arg("npm/cached_only_after_first_run/main1.ts")
    .env("NO_COLOR", "1")
    .envs(env_vars())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()
    .unwrap();
  let output = deno.wait_with_output().unwrap();
  let stderr = String::from_utf8_lossy(&output.stderr);
  let stdout = String::from_utf8_lossy(&output.stdout);
  assert_contains!(stderr, "Download");
  assert_contains!(stdout, "createChalk: chalk");
  assert!(output.status.success());

  let deno = util::deno_cmd_with_deno_dir(&deno_dir)
    .current_dir(util::testdata_path())
    .arg("run")
    .arg("--unstable")
    .arg("--allow-read")
    .arg("--allow-env")
    .arg("--cached-only")
    .arg("npm/cached_only_after_first_run/main2.ts")
    .env("NO_COLOR", "1")
    .envs(env_vars())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()
    .unwrap();
  let output = deno.wait_with_output().unwrap();
  let stderr = String::from_utf8_lossy(&output.stderr);
  let stdout = String::from_utf8_lossy(&output.stdout);
  assert_contains!(
    stderr,
    "An npm specifier not found in cache: \"ansi-styles\", --cached-only is specified."
  );
  assert!(stdout.is_empty());
  assert!(!output.status.success());

  let deno = util::deno_cmd_with_deno_dir(&deno_dir)
    .current_dir(util::testdata_path())
    .arg("run")
    .arg("--unstable")
    .arg("--allow-read")
    .arg("--allow-env")
    .arg("--cached-only")
    .arg("npm/cached_only_after_first_run/main1.ts")
    .env("NO_COLOR", "1")
    .envs(env_vars())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()
    .unwrap();

  let output = deno.wait_with_output().unwrap();
  let stderr = String::from_utf8_lossy(&output.stderr);
  let stdout = String::from_utf8_lossy(&output.stdout);
  assert!(output.status.success());
  assert!(stderr.is_empty());
  assert_contains!(stdout, "createChalk: chalk");
}

#[test]
fn no_npm_after_first_run() {
  let _server = http_server();

  let deno_dir = util::new_deno_dir();

  let deno = util::deno_cmd_with_deno_dir(&deno_dir)
    .current_dir(util::testdata_path())
    .arg("run")
    .arg("--unstable")
    .arg("--allow-read")
    .arg("--allow-env")
    .arg("--no-npm")
    .arg("npm/no_npm_after_first_run/main1.ts")
    .env("NO_COLOR", "1")
    .envs(env_vars())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()
    .unwrap();
  let output = deno.wait_with_output().unwrap();
  let stderr = String::from_utf8_lossy(&output.stderr);
  let stdout = String::from_utf8_lossy(&output.stdout);
  assert_contains!(
    stderr,
    "Following npm specifiers were requested: \"chalk@5\"; but --no-npm is specified."
  );
  assert!(stdout.is_empty());
  assert!(!output.status.success());

  let deno = util::deno_cmd_with_deno_dir(&deno_dir)
    .current_dir(util::testdata_path())
    .arg("run")
    .arg("--unstable")
    .arg("--allow-read")
    .arg("--allow-env")
    .arg("npm/no_npm_after_first_run/main1.ts")
    .env("NO_COLOR", "1")
    .envs(env_vars())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()
    .unwrap();
  let output = deno.wait_with_output().unwrap();
  let stderr = String::from_utf8_lossy(&output.stderr);
  let stdout = String::from_utf8_lossy(&output.stdout);
  assert_contains!(stderr, "Download");
  assert_contains!(stdout, "createChalk: chalk");
  assert!(output.status.success());

  let deno = util::deno_cmd_with_deno_dir(&deno_dir)
    .current_dir(util::testdata_path())
    .arg("run")
    .arg("--unstable")
    .arg("--allow-read")
    .arg("--allow-env")
    .arg("--no-npm")
    .arg("npm/no_npm_after_first_run/main1.ts")
    .env("NO_COLOR", "1")
    .envs(env_vars())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()
    .unwrap();
  let output = deno.wait_with_output().unwrap();
  let stderr = String::from_utf8_lossy(&output.stderr);
  let stdout = String::from_utf8_lossy(&output.stdout);
  assert_contains!(
    stderr,
    "Following npm specifiers were requested: \"chalk@5\"; but --no-npm is specified."
  );
  assert!(stdout.is_empty());
  assert!(!output.status.success());
}

#[test]
fn deno_run_cjs_module() {
  let _server = http_server();

  let deno_dir = util::new_deno_dir();

  let deno = util::deno_cmd_with_deno_dir(&deno_dir)
    .current_dir(deno_dir.path())
    .arg("run")
    .arg("--unstable")
    .arg("--allow-read")
    .arg("--allow-env")
    .arg("--allow-write")
    .arg("npm:mkdirp@1.0.4")
    .arg("test_dir")
    .env("NO_COLOR", "1")
    .envs(env_vars())
    .spawn()
    .unwrap();
  let output = deno.wait_with_output().unwrap();
  assert!(output.status.success());

  assert!(deno_dir.path().join("test_dir").exists());
}

itest!(deno_run_cowsay {
  args: "run --unstable -A --quiet npm:cowsay@1.5.0 Hello",
  output: "npm/deno_run_cowsay.out",
  envs: env_vars_no_sync_download(),
  http_server: true,
});

itest!(deno_run_cowsay_explicit {
  args: "run --unstable -A --quiet npm:cowsay@1.5.0/cowsay Hello",
  output: "npm/deno_run_cowsay.out",
  envs: env_vars_no_sync_download(),
  http_server: true,
});

itest!(deno_run_cowthink {
  args: "run --unstable -A --quiet npm:cowsay@1.5.0/cowthink Hello",
  output: "npm/deno_run_cowthink.out",
  envs: env_vars_no_sync_download(),
  http_server: true,
});

itest!(deno_run_esm_module {
  args: "run --unstable -A --quiet npm:@denotest/esm-bin this is a test",
  output: "npm/deno_run_esm.out",
  envs: env_vars(),
  http_server: true,
});

itest!(deno_run_non_existent {
  args: "run --unstable npm:mkdirp@0.5.125",
  output: "npm/deno_run_non_existent.out",
  envs: env_vars(),
  http_server: true,
  exit_code: 1,
});

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
      if registry_json_path.exists() {
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
