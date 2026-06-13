// Copyright 2018-2026 the Deno authors. MIT license.

pub fn main() {
  // We have a lib.rs and main.rs in order to be able
  // to run tests without building a binary on the CI.
  //
  // Prefer to keep this file simple and mostly empty.
  deno::main()
}
