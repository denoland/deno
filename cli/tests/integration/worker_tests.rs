// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use crate::itest;

itest!(workers {
  args: "test --reload --location http://127.0.0.1:4545/ -A --unstable workers/test.ts",
  output: "workers/test.ts.out",
  http_server: true,
});

itest!(worker_error {
  args: "run -A workers/worker_error.ts",
  output: "workers/worker_error.ts.out",
  exit_code: 1,
});

itest!(worker_nested_error {
  args: "run -A workers/worker_nested_error.ts",
  output: "workers/worker_nested_error.ts.out",
  exit_code: 1,
});

itest!(worker_async_error {
  args: "run -A --quiet --reload workers/worker_async_error.ts",
  output: "workers/worker_async_error.ts.out",
  http_server: true,
  exit_code: 1,
});

itest!(worker_message_handler_error {
  args: "run -A --quiet --reload workers/worker_message_handler_error.ts",
  output: "workers/worker_message_handler_error.ts.out",
  http_server: true,
  exit_code: 1,
});

itest!(nonexistent_worker {
  args: "run --allow-read workers/nonexistent_worker.ts",
  output: "workers/nonexistent_worker.out",
  exit_code: 1,
});

itest!(_084_worker_custom_inspect {
  args: "run --allow-read 084_worker_custom_inspect.ts",
  output: "084_worker_custom_inspect.ts.out",
});

itest!(error_worker_permissions_local {
  args: "run --reload error_worker_permissions_local.ts",
  output: "error_worker_permissions_local.ts.out",
  exit_code: 1,
});

itest!(error_worker_permissions_remote {
  args: "run --reload error_worker_permissions_remote.ts",
  http_server: true,
  output: "error_worker_permissions_remote.ts.out",
  exit_code: 1,
});

itest!(worker_permissions_remote_remote {
  args: "run --quiet --reload --allow-net=localhost:4545 workers/permissions_remote_remote.ts",
  output: "workers/permissions_remote_remote.ts.out",
  http_server: true,
  exit_code: 1,
});

itest!(worker_permissions_dynamic_remote {
  args: "run --quiet --reload --allow-net --unstable workers/permissions_dynamic_remote.ts",
  output: "workers/permissions_dynamic_remote.ts.out",
  http_server: true,
  exit_code: 1,
});

itest!(worker_permissions_data_remote {
  args: "run --quiet --reload --allow-net=localhost:4545 workers/permissions_data_remote.ts",
  output: "workers/permissions_data_remote.ts.out",
  http_server: true,
  exit_code: 1,
});

itest!(worker_permissions_blob_remote {
  args: "run --quiet --reload --allow-net=localhost:4545 workers/permissions_blob_remote.ts",
  output: "workers/permissions_blob_remote.ts.out",
  http_server: true,
  exit_code: 1,
});

itest!(worker_permissions_data_local {
  args: "run --quiet --reload --allow-net=localhost:4545 workers/permissions_data_local.ts",
  output: "workers/permissions_data_local.ts.out",
  http_server: true,
  exit_code: 1,
});

itest!(worker_permissions_blob_local {
  args: "run --quiet --reload --allow-net=localhost:4545 workers/permissions_blob_local.ts",
  output: "workers/permissions_blob_local.ts.out",
  http_server: true,
  exit_code: 1,
});

itest!(worker_terminate_tla_crash {
  args: "run --quiet --reload workers/terminate_tla_crash.js",
  output: "workers/terminate_tla_crash.js.out",
});
