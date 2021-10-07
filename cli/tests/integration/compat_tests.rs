// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use crate::itest;

itest!(globals {
  args: "run --compat --unstable --allow-read --allow-env compat/globals.ts",
  output: "compat/globals.out",
});

itest!(fs_promises {
  args: "run --compat --unstable -A compat/fs_promises.js",
  output: "compat/fs_promises.out",
});

itest!(node_prefix_fs_promises {
  args: "run --compat --unstable -A compat/node_fs_promises.js",
  output: "compat/fs_promises.out",
});

itest!(existing_import_map {
  args: "run --compat --unstable --import-map compat/existing_import_map.json compat/fs_promises.js",
  output: "compat/existing_import_map.out",
  exit_code: 1,
});
