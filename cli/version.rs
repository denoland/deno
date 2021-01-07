// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

pub const GIT_COMMIT_HASH: &str = env!("GIT_COMMIT_HASH");
pub const TYPESCRIPT: &str = env!("TS_VERSION");

pub fn deno() -> String {
  let semver = env!("CARGO_PKG_VERSION");
  option_env!("DENO_CANARY").map_or(semver.to_string(), |_| {
    format!("{}+{}", semver, GIT_COMMIT_HASH)
  })
}

pub fn is_canary() -> bool {
  option_env!("DENO_CANARY").is_some()
}
