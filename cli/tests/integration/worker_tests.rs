// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use crate::itest;
use test_util as util;

#[test]
fn workers() {
  let _g = util::http_server();
  let status = util::deno_cmd()
    .current_dir(util::tests_path())
    .arg("test")
    .arg("--reload")
    .arg("--location")
    .arg("http://127.0.0.1:4545/cli/tests/")
    .arg("--allow-net")
    .arg("--allow-read")
    .arg("--unstable")
    .arg("workers/test.ts")
    .spawn()
    .unwrap()
    .wait()
    .unwrap();
  assert!(status.success());
}

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
