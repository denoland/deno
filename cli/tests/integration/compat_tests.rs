// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use crate::itest;

itest!(fs_promises {
  args: "run --compat --unstable -A compat/fs_promises.js",
  output: "compat/fs_promises.out",
});

itest!(existing_import_map {
  args: "run --compat --import-map compat/existing_import_map.json compat/fs_promises.js",
  output: "compat/existing_import_map.out",
  exit_code: 1,
});
