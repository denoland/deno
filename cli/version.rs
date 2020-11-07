// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

pub fn deno() -> &'static str {
  option_env!("DENO_NIGHTLY").unwrap_or(env!("CARGO_PKG_VERSION"))
}
pub fn is_nightly() -> bool {
  option_env!("DENO_NIGHTLY").is_some()
}

pub const GIT_COMMIT_HASH: &str = env!("GIT_COMMIT_HASH");
pub const TYPESCRIPT: &str = crate::js::TS_VERSION;

pub fn v8() -> &'static str {
  deno_core::v8_version()
}
