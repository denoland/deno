// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use crate::itest;

itest!(compat_fs_promises {
  args: "run --compat --unstable -A compat/fs_promises.js",
  output: "compat/fs_promises.out",
});
