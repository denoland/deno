// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use std::ffi::OsStr;
use std::path::PathBuf;

pub const DENO: &str = env!("CARGO_PKG_VERSION");

pub fn v8() -> &'static str {
  deno_core::v8_version()
}

pub fn ts_version() -> String {
  let mut manifest_dir =
    PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap());
  let deno_root = Some(OsStr::new("deno"));
  while manifest_dir.file_name() != deno_root && manifest_dir.pop() {}
  manifest_dir.push("cli/tsc/00_typescript.js");
  std::fs::read_to_string(manifest_dir.into_os_string().to_str().unwrap())
    .unwrap()
    .lines()
    .find(|l| l.contains("ts.version = "))
    .expect(
      "Failed to find the pattern `ts.version = ` in typescript source code",
    )
    .chars()
    .skip_while(|c| !char::is_numeric(*c))
    .take_while(|c| *c != '"')
    .collect::<String>()
}
