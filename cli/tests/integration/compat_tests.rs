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

#[test]
fn node_compat_url() {
  let (out, err) = util::run_and_collect_output_with_args(
    false,
    vec!["repl", "--compat", "--unstable", "--quiet"],
    None,
    Some(vec![(
      "DENO_NODE_COMPAT_URL".to_string(),
      "file:///non_existent/".to_string(),
    )]),
    false,
  );
  assert!(out.is_empty());
  assert!(!err.is_empty());
  assert!(err.contains("file:///non_existent/node/global.ts"));
}
