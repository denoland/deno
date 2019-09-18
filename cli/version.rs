// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
pub const DENO: &str = env!("CARGO_PKG_VERSION");
pub const TYPESCRIPT: &str = deno_cli_snapshots::TS_VERSION;

pub fn v8() -> &'static str {
  deno::v8_version()
}
