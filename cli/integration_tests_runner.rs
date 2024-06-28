// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
pub fn main() {
  let mut args = vec!["cargo", "test", "-p", "cli_tests", "--features", "run"];

  if !cfg!(debug_assertions) {
    args.push("--release");
  }

  args.push("--");

  // If any args were passed to this process, pass them through to the child
  let orig_args = std::env::args().skip(1).collect::<Vec<_>>();
  let orig_args: Vec<&str> =
    orig_args.iter().map(|x| x.as_ref()).collect::<Vec<_>>();
  args.extend(orig_args);

  test_util::spawn::exec_replace("cargo", &args).unwrap();
}
