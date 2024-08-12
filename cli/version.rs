// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

pub use crate::shared::ReleaseChannel;

pub const GIT_COMMIT_HASH: &str = env!("GIT_COMMIT_HASH");
pub const TYPESCRIPT: &str = env!("TS_VERSION");
pub const IS_CANARY: bool = option_env!("DENO_CANARY").is_some();

pub fn deno() -> &'static str {
  if IS_CANARY {
    concat!(
      env!("CARGO_PKG_VERSION"),
      "+",
      env!("GIT_COMMIT_HASH_SHORT")
    )
  } else {
    env!("CARGO_PKG_VERSION")
  }
}

// Keep this in sync with `deno()` above
pub fn get_user_agent() -> &'static str {
  if IS_CANARY {
    concat!(
      "Deno/",
      env!("CARGO_PKG_VERSION"),
      "+",
      env!("GIT_COMMIT_HASH_SHORT")
    )
  } else {
    concat!("Deno/", env!("CARGO_PKG_VERSION"))
  }
}

pub fn release_version_or_canary_commit_hash() -> &'static str {
  if IS_CANARY {
    GIT_COMMIT_HASH
  } else {
    env!("CARGO_PKG_VERSION")
  }
}
