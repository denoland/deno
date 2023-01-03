// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

mod integration;

use deno_core::url::Url;
use test_util as util;

mod bench {
  use super::*;

  itest!(overloads {
    args: "bench bench/overloads.ts",
    exit_code: 0,
    output: "bench/overloads.out",
  });

  itest!(meta {
    args: "bench bench/meta.ts",
    exit_code: 0,
    output: "bench/meta.out",
  });

  itest!(pass {
    args: "bench bench/pass.ts",
    exit_code: 0,
    output: "bench/pass.out",
  });

  itest!(ignore {
    args: "bench bench/ignore.ts",
    exit_code: 0,
    output: "bench/ignore.out",
  });

  itest!(ignore_permissions {
    args: "bench bench/ignore_permissions.ts",
    exit_code: 0,
    output: "bench/ignore_permissions.out",
  });

  itest!(fail {
    args: "bench bench/fail.ts",
    exit_code: 1,
    output: "bench/fail.out",
  });

  itest!(collect {
    args: "bench --ignore=bench/collect/ignore bench/collect",
    exit_code: 0,
    output: "bench/collect.out",
  });

  itest!(load_unload {
    args: "bench bench/load_unload.ts",
    exit_code: 0,
    output: "bench/load_unload.out",
  });

  itest!(interval {
    args: "bench bench/interval.ts",
    exit_code: 0,
    output: "bench/interval.out",
  });

  itest!(quiet {
    args: "bench --quiet bench/quiet.ts",
    exit_code: 0,
    output: "bench/quiet.out",
  });

  itest!(only {
    args: "bench bench/only.ts",
    exit_code: 1,
    output: "bench/only.out",
  });

  itest!(multifile_summary {
    args: "bench bench/group_baseline.ts bench/pass.ts bench/group_baseline.ts",
    exit_code: 0,
    output: "bench/multifile_summary.out",
  });

  itest!(no_check {
    args: "bench --no-check bench/no_check.ts",
    exit_code: 1,
    output: "bench/no_check.out",
  });

  itest!(allow_all {
    args: "bench  --allow-all bench/allow_all.ts",
    exit_code: 0,
    output: "bench/allow_all.out",
  });

  itest!(allow_none {
    args: "bench bench/allow_none.ts",
    exit_code: 1,
    output: "bench/allow_none.out",
  });

  itest!(exit_sanitizer {
    args: "bench bench/exit_sanitizer.ts",
    output: "bench/exit_sanitizer.out",
    exit_code: 1,
  });

  itest!(clear_timeout {
    args: "bench bench/clear_timeout.ts",
    exit_code: 0,
    output: "bench/clear_timeout.out",
  });

  itest!(finally_timeout {
    args: "bench bench/finally_timeout.ts",
    exit_code: 1,
    output: "bench/finally_timeout.out",
  });

  itest!(group_baseline {
    args: "bench bench/group_baseline.ts",
    exit_code: 0,
    output: "bench/group_baseline.out",
  });

  itest!(unresolved_promise {
    args: "bench bench/unresolved_promise.ts",
    exit_code: 1,
    output: "bench/unresolved_promise.out",
  });

  itest!(unhandled_rejection {
    args: "bench bench/unhandled_rejection.ts",
    exit_code: 1,
    output: "bench/unhandled_rejection.out",
  });

  itest!(filter {
    args: "bench --filter=foo bench/filter",
    exit_code: 0,
    output: "bench/filter.out",
  });

  itest!(no_prompt_by_default {
    args: "bench --quiet bench/no_prompt_by_default.ts",
    exit_code: 1,
    output: "bench/no_prompt_by_default.out",
  });

  itest!(no_prompt_with_denied_perms {
    args: "bench --quiet --allow-read bench/no_prompt_with_denied_perms.ts",
    exit_code: 1,
    output: "bench/no_prompt_with_denied_perms.out",
  });

  itest!(check_local_by_default {
    args: "bench --quiet bench/check_local_by_default.ts",
    output: "bench/check_local_by_default.out",
    http_server: true,
  });

  itest!(check_local_by_default2 {
    args: "bench --quiet bench/check_local_by_default2.ts",
    output: "bench/check_local_by_default2.out",
    http_server: true,
    exit_code: 1,
  });

  itest!(bench_with_config {
    args: "bench --config bench/collect/deno.jsonc bench/collect",
    exit_code: 0,
    output: "bench/collect.out",
  });

  itest!(bench_with_config2 {
    args: "bench --config bench/collect/deno2.jsonc bench/collect",
    exit_code: 0,
    output: "bench/collect2.out",
  });

  itest!(bench_with_malformed_config {
    args: "bench --config bench/collect/deno.malformed.jsonc",
    exit_code: 1,
    output: "bench/collect_with_malformed_config.out",
  });

  #[test]
  fn recursive_permissions_pledge() {
    let output = util::deno_cmd()
      .current_dir(util::testdata_path())
      .arg("bench")
      .arg("bench/recursive_permissions_pledge.js")
      .stderr(std::process::Stdio::piped())
      .spawn()
      .unwrap()
      .wait_with_output()
      .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8(output.stderr).unwrap().contains(
      "pledge test permissions called before restoring previous pledge"
    ));
  }

  #[test]
  fn file_protocol() {
    let file_url =
      Url::from_file_path(util::testdata_path().join("bench/file_protocol.ts"))
        .unwrap()
        .to_string();

    (util::CheckOutputIntegrationTest {
      args_vec: vec!["bench", &file_url],
      exit_code: 0,
      output: "bench/file_protocol.out",
      ..Default::default()
    })
    .run();
  }
}
