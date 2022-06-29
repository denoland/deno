// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

mod build_runtime;
mod build_tsc;

fn main() {
  let out_dir = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
  build_runtime::create_runtime_snapshot(&out_dir.join("CLI_SNAPSHOT.bin"));
  build_tsc::create_tsc_snapshot(&out_dir.join("COMPILER_SNAPSHOT.bin"));
}
