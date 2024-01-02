// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use deno_lockfile::Lockfile;
use test_util as util;
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
