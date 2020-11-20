// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

pub const DENO: &str = env!("CARGO_PKG_VERSION");
pub const GIT_COMMIT_HASH: &str = env!("GIT_COMMIT_HASH");
pub const TYPESCRIPT: &str = crate::js::TS_VERSION;

pub fn v8() -> &'static str {
  deno_core::v8_version()
}
