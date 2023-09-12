// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use test_util as util;
use util::env_vars_for_jsr_tests;
use util::env_vars_for_jsr_tests_no_sync_download;

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
  envs: env_vars_for_jsr_tests_no_sync_download(),
  http_server: true,
});

itest!(module_graph {
  args: "run jsr/module_graph/main.ts",
  output: "jsr/module_graph/main.out",
  envs: env_vars_for_jsr_tests(),
  http_server: true,
});

itest!(deps {
  args: "run jsr/deps/main.ts",
  output: "jsr/deps/main.out",
  envs: env_vars_for_jsr_tests(),
  http_server: true,
});
