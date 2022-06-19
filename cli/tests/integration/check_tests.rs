// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use crate::itest;

itest!(_095_check_with_bare_import {
  args: "check 095_cache_with_bare_import.ts",
  output: "095_cache_with_bare_import.ts.out",
  exit_code: 1,
});

itest!(check_extensionless {
  args: "check --reload http://localhost:4545/subdir/no_js_ext",
  output: "cache_extensionless.out",
  http_server: true,
});

itest!(check_random_extension {
  args: "check --reload http://localhost:4545/subdir/no_js_ext@1.0.0",
  output: "cache_random_extension.out",
  http_server: true,
});

itest!(check_all {
  args: "check --quiet --remote check_all.ts",
  output: "check_all.out",
  http_server: true,
  exit_code: 1,
});

itest!(check_all_local {
  args: "check --quiet check_all.ts",
  output_str: Some(""),
  http_server: true,
});

itest!(module_detection_force {
  args: "check --quiet module_detection_force.ts",
  output_str: Some(""),
});
