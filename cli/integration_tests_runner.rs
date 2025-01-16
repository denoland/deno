// Copyright 2018-2025 the Deno authors. MIT license.

// Welcome to this extremely hacky setup in order to get the deno
// and denort executable to build before running the integration tests
// found in the tests folder at the root of the repo when only
// running `cargo test`.
//
// Cargo has no way to tell it to build a specific executable. The only
// time it builds it is when there is an integration test in the workflow.
// We exploit this property by defining integration tests with a custom
// harness in both the deno and denort crates. Then we cause the denort
// and cli_tests crates to not actually run unless signaled by this test
// runner here.
//
// So what occurs is:
// 1. The deno, denort, and cli_tests crates are run as integration tests.
//    - This causes the deno and denort executables to be built.
// 2. The denort and cli_tests integration test runner exit because they
//    are not signaled to run.
// 3. This script then spawns cargo to run the denort integration test
// 4. That integration test then runs the cli_tests crate.
//
// This is terrible, but it enables building denort and deno in parallel
// as soon as calling `cargo test` and it doesn't require having to remember
// to build these executables beforehand. Hopefully one day cargo improves
// so that we can remove this code.
pub fn main() {
  let mut args = vec!["cargo", "test", "-p", "denort", "--test", "integration"];

  if !cfg!(debug_assertions) {
    args.push("--release");
  }

  args.push("--");
  args.push("--actually-run");

  // If any args were passed to this process, pass them through to the child
  let orig_args = std::env::args().skip(1).collect::<Vec<_>>();
  let orig_args: Vec<&str> =
    orig_args.iter().map(|x| x.as_ref()).collect::<Vec<_>>();
  args.extend(orig_args);

  test_util::spawn::exec_replace("cargo", &args).unwrap();
}
