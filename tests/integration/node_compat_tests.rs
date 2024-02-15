// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use test_util as util;
use test_util::itest;
use util::env_vars_for_npm_tests;

itest!(node_test_module {
  args: "test node/test.js",
  output: "node/test.out",
  envs: env_vars_for_npm_tests(),
  exit_code: 1,
  http_server: true,
});
