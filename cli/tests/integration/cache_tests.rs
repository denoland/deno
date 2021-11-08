// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use crate::itest;

itest!(_036_import_map_fetch {
  args:
    "cache --quiet --reload --import-map=import_maps/import_map.json import_maps/test.ts",
  output: "036_import_map_fetch.out",
});

itest!(_037_fetch_multiple {
  args: "cache --reload fetch/test.ts fetch/other.ts",
  http_server: true,
  output: "037_fetch_multiple.out",
});

itest!(_095_cache_with_bare_import {
  args: "cache 095_cache_with_bare_import.ts",
  output: "095_cache_with_bare_import.ts.out",
  exit_code: 1,
});

itest!(cache_extensionless {
  args: "cache --reload http://localhost:4545/subdir/no_js_ext",
  output: "cache_extensionless.out",
  http_server: true,
});

itest!(cache_random_extension {
  args: "cache --reload http://localhost:4545/subdir/no_js_ext@1.0.0",
  output: "cache_random_extension.out",
  http_server: true,
});

itest!(performance_stats {
  args: "cache --reload --log-level debug 002_hello.ts",
  output: "performance_stats.out",
});

itest!(redirect_cache {
  http_server: true,
  args: "cache --reload http://localhost:4548/subdir/redirects/a.ts",
  output: "redirect_cache.out",
});

itest!(ignore_require {
  args: "cache --reload --no-check ignore_require.js",
  output_str: Some(""),
  exit_code: 0,
});
