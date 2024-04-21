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

itest!(node_test_module_no_sanitizers {
  args: "test -A --no-check node/test_no_sanitizers/test.js",
  output: "node/test_no_sanitizers/test.out",
  envs: env_vars_for_npm_tests(),
  exit_code: 0,
  // TODO(mmastrac): fix exit sanitizer part of test
  // exit_code: 123,
  http_server: true,
});

itest!(
  node_process_beforeexit_exit_events_emitted_without_listeners {
    args: "run node/process_beforeexit_exit_events.ts",
    output: "node/process_beforeexit_exit_events.out",
    exit_code: 0,
  }
);

itest!(web_node_events_dispatched_in_correct_order {
  args: "run node/events_order.ts",
  output: "node/events_order.out",
  exit_code: 0,
});
