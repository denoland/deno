// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use deno_core::serde_json::Value;
use deno_lockfile::Lockfile;
use test_util as util;
use url::Url;
use util::env_vars_for_jsr_tests;
use util::TestContextBuilder;

itest!(no_module_graph_run {
  args: "run jsr/no_module_graph/main.ts",
  output: "jsr/no_module_graph/main.out",
  envs: env_vars_for_jsr_tests(),
  http_server: true,
});

itest!(no_module_graph_info {
  args: "info jsr/no_module_graph/main.ts",
  output: "jsr/no_module_graph/main_info.out",
  envs: env_vars_for_jsr_tests(),
  http_server: true,
});

itest!(same_package_multiple_versions {
  args: "run --quiet jsr/no_module_graph/multiple.ts",
  output: "jsr/no_module_graph/multiple.out",
  envs: env_vars_for_jsr_tests(),
  http_server: true,
});

itest!(module_graph_run {
  args: "run jsr/module_graph/main.ts",
  output: "jsr/module_graph/main.out",
  envs: env_vars_for_jsr_tests(),
  http_server: true,
});

itest!(module_graph_info {
  args: "info jsr/module_graph/main.ts",
  output: "jsr/module_graph/main_info.out",
  envs: env_vars_for_jsr_tests(),
  http_server: true,
});

itest!(deps_run {
  args: "run jsr/deps/main.ts",
  output: "jsr/deps/main.out",
  envs: env_vars_for_jsr_tests(),
  http_server: true,
});

itest!(deps_info {
  args: "info jsr/deps/main.ts",
  output: "jsr/deps/main_info.out",
  envs: env_vars_for_jsr_tests(),
  http_server: true,
});

itest!(subset_type_graph {
  args: "check --all jsr/subset_type_graph/main.ts",
  output: "jsr/subset_type_graph/main.check.out",
  envs: env_vars_for_jsr_tests(),
  http_server: true,
  exit_code: 1,
});

itest!(version_not_found {
  args: "run jsr/version_not_found/main.ts",
  output: "jsr/version_not_found/main.out",
  envs: env_vars_for_jsr_tests(),
  http_server: true,
  exit_code: 1,
});

#[test]
fn specifiers_in_lockfile() {
  let test_context = TestContextBuilder::for_jsr().use_temp_cwd().build();
  let temp_dir = test_context.temp_dir();

  temp_dir.write(
    "main.ts",
    r#"import version from "jsr:@denotest/no_module_graph@0.1";

console.log(version);"#,
  );
  temp_dir.write("deno.json", "{}"); // to automatically create a lockfile

  test_context
    .new_command()
    .args("run --quiet main.ts")
    .run()
    .assert_matches_text("0.1.1\n");

  let lockfile_path = temp_dir.path().join("deno.lock");
  let mut lockfile = Lockfile::new(lockfile_path.to_path_buf(), false).unwrap();
  *lockfile
    .content
    .packages
    .specifiers
    .get_mut("jsr:@denotest/no_module_graph@0.1")
    .unwrap() = "jsr:@denotest/no_module_graph@0.1.0".to_string();
  lockfile_path.write(lockfile.as_json_string());

  test_context
    .new_command()
    .args("run --quiet main.ts")
    .run()
    .assert_matches_text("0.1.0\n");
}

#[test]
fn reload_info_not_found_cache_but_exists_remote() {
  fn remove_version(registry_json: &mut Value, version: &str) {
    registry_json
      .as_object_mut()
      .unwrap()
      .get_mut("versions")
      .unwrap()
      .as_object_mut()
      .unwrap()
      .remove(version);
  }

  fn remove_version_for_package(
    deno_dir: &util::TempDir,
    package: &str,
    version: &str,
  ) {
    let specifier =
      Url::parse(&format!("http://127.0.0.1:4250/{}/meta.json", package))
        .unwrap();
    let registry_json_path = deno_dir
      .path()
      .join("deps")
      .join(deno_cache_dir::url_to_filename(&specifier).unwrap());
    let mut registry_json = registry_json_path.read_json_value();
    remove_version(&mut registry_json, version);
    registry_json_path.write_json(&registry_json);
  }

  // This tests that when a local machine doesn't have a version
  // specified in a dependency that exists in the npm registry
  let test_context = TestContextBuilder::for_jsr().use_temp_cwd().build();
  let deno_dir = test_context.deno_dir();
  let temp_dir = test_context.temp_dir();
  temp_dir.write(
    "main.ts",
    "import { add } from 'jsr:@denotest/add@1'; console.log(add(1, 2));",
  );

  // cache successfully to the deno_dir
  let output = test_context.new_command().args("cache main.ts").run();
  output.assert_matches_text(concat!(
    "Download http://127.0.0.1:4250/@denotest/add/meta.json\n",
    "Download http://127.0.0.1:4250/@denotest/add/1.0.0_meta.json\n",
    "Download http://127.0.0.1:4250/@denotest/add/1.0.0/mod.ts\n",
  ));

  // modify the package information in the cache to remove the latest version
  remove_version_for_package(deno_dir, "@denotest/add", "1.0.0");

  // should error when `--cache-only` is used now because the version is not in the cache
  let output = test_context
    .new_command()
    .args("run --cached-only main.ts")
    .run();
  output.assert_exit_code(1);
  output.assert_matches_text("error: Failed to resolve version constraint. Try running again without --cached-only
    at file:///[WILDCARD]main.ts:1:21
");

  // now try running without it, it should download the package now
  test_context
    .new_command()
    .args("run main.ts")
    .run()
    .assert_matches_text(concat!(
      "Download http://127.0.0.1:4250/@denotest/add/meta.json\n",
      "Download http://127.0.0.1:4250/@denotest/add/1.0.0_meta.json\n",
      "3\n",
    ))
    .assert_exit_code(0);
}
