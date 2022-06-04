// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

mod build_runtime;
mod build_tsc;

fn main() {
  let mut args = std::env::args();
  args.next().unwrap();
  let arg1 = std::path::PathBuf::from(args.next().unwrap());
  let arg2 = std::path::PathBuf::from(args.next().unwrap());
  build_runtime::create_runtime_snapshot(&arg1);
  build_tsc::create_tsc_snapshot(&arg2);
}
