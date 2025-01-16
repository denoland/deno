// Copyright 2018-2025 the Deno authors. MIT license.

pub fn main() {
  let mut orig_args = std::env::args().skip(1).collect::<Vec<_>>();
  if orig_args.is_empty() {
    return;
  }
  if orig_args.remove(0) != "--actually-run" {
    return;
  }

  let mut args = vec!["cargo", "test", "-p", "cli_tests", "--features", "run"];

  if !cfg!(debug_assertions) {
    args.push("--release");
  }

  args.push("--");

  // If any args were passed to this process, pass them through to the child
  let orig_args: Vec<&str> =
    orig_args.iter().map(|x| x.as_ref()).collect::<Vec<_>>();
  args.extend(orig_args);

  test_util::spawn::exec_replace("cargo", &args).unwrap();
}
