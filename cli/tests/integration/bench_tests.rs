// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use crate::itest;

itest!(requires_unstable {
  args: "bench bench/requires_unstable.js",
  exit_code: 70,
  output: "bench/requires_unstable.out",
});

itest!(overloads {
  args: "bench --unstable bench/overloads.ts",
  exit_code: 0,
  output: "bench/overloads.out",
});

itest!(meta {
  args: "bench --unstable bench/meta.ts",
  exit_code: 0,
  output: "bench/meta.out",
});

itest!(pass {
  args: "bench --unstable bench/pass.ts",
  exit_code: 0,
  output: "bench/pass.out",
});

itest!(ignore {
  args: "bench --unstable bench/ignore.ts",
  exit_code: 0,
  output: "bench/ignore.out",
});

itest!(ignore_permissions {
  args: "bench --unstable bench/ignore_permissions.ts",
  exit_code: 0,
  output: "bench/ignore_permissions.out",
});

itest!(fail {
  args: "bench --unstable bench/fail.ts",
  exit_code: 1,
  output: "bench/fail.out",
});

itest!(collect {
  args: "bench --unstable --ignore=bench/collect/ignore bench/collect",
  exit_code: 0,
  output: "bench/collect.out",
});

itest!(load_unload {
  args: "bench --unstable bench/load_unload.ts",
  exit_code: 0,
  output: "bench/load_unload.out",
});

itest!(interval {
  args: "bench --unstable bench/interval.ts",
  exit_code: 0,
  output: "bench/interval.out",
});

itest!(quiet {
  args: "bench --unstable --quiet bench/quiet.ts",
  exit_code: 0,
  output: "bench/quiet.out",
});

itest!(only {
  args: "bench --unstable bench/only.ts",
  exit_code: 1,
  output: "bench/only.out",
});

itest!(no_check {
  args: "bench --unstable --no-check bench/no_check.ts",
  exit_code: 1,
  output: "bench/no_check.out",
});

itest!(allow_all {
  args: "bench  --unstable --allow-all bench/allow_all.ts",
  exit_code: 0,
  output: "bench/allow_all.out",
});

itest!(allow_none {
  args: "bench --unstable bench/allow_none.ts",
  exit_code: 1,
  output: "bench/allow_none.out",
});

itest!(exit_sanitizer {
  args: "bench --unstable bench/exit_sanitizer.ts",
  output: "bench/exit_sanitizer.out",
  exit_code: 1,
});

itest!(clear_timeout {
  args: "bench --unstable bench/clear_timeout.ts",
  exit_code: 0,
  output: "bench/clear_timeout.out",
});

itest!(finally_timeout {
  args: "bench --unstable bench/finally_timeout.ts",
  exit_code: 1,
  output: "bench/finally_timeout.out",
});

itest!(unresolved_promise {
  args: "bench --unstable bench/unresolved_promise.ts",
  exit_code: 1,
  output: "bench/unresolved_promise.out",
});

itest!(unhandled_rejection {
  args: "bench --unstable bench/unhandled_rejection.ts",
  exit_code: 1,
  output: "bench/unhandled_rejection.out",
});

itest!(filter {
  args: "bench --unstable --filter=foo bench/filter",
  exit_code: 0,
  output: "bench/filter.out",
});

itest!(no_prompt_by_default {
  args: "bench --unstable bench/no_prompt_by_default.ts",
  exit_code: 1,
  output: "bench/no_prompt_by_default.out",
});

itest!(no_prompt_with_denied_perms {
  args: "bench --unstable --allow-read bench/no_prompt_with_denied_perms.ts",
  exit_code: 1,
  output: "bench/no_prompt_with_denied_perms.out",
});
