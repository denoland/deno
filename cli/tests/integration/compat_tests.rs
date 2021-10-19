// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use crate::itest;
use test_util as util;

itest!(globals {
  args: "run --compat --unstable --allow-read --allow-env compat/globals.ts",
  output: "compat/globals.out",
});

itest!(fs_promises {
  args: "run --compat --unstable -A compat/fs_promises.mjs",
  output: "compat/fs_promises.out",
});

itest!(node_prefix_fs_promises {
  args: "run --compat --unstable -A compat/node_fs_promises.mjs",
  output: "compat/fs_promises.out",
});

#[test]
fn globals_in_repl() {
  let (out, _err) = util::run_and_collect_output_with_args(
    true,
    vec!["repl", "--compat", "--unstable", "--quiet"],
    Some(vec!["global == window"]),
    None,
    false,
  );
  assert!(out.contains("true"));
}
