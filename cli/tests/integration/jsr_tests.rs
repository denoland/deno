// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use test_util as util;
use util::env_vars_for_jsr_tests;

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
