// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

pub const GIT_COMMIT_HASH: &str = env!("GIT_COMMIT_HASH");
pub const TYPESCRIPT: &str = env!("TS_VERSION");

pub fn deno() -> &'static str {
  if is_canary() {
    concat!("2.0.0-rc.0", "+", env!("GIT_COMMIT_HASH_SHORT"))
  } else {
    "2.0.0-rc.0"
  }
}

// Keep this in sync with `deno()` above
pub fn get_user_agent() -> &'static str {
  if is_canary() {
    concat!("Deno/", "2.0.0-rc.0", "+", env!("GIT_COMMIT_HASH_SHORT"))
  } else {
    concat!("Deno/", "2.0.0-rc.0")
  }
}

// TODO(bartlomieju): how do we decide on that?
pub fn is_release_candidate() -> bool {
  false
}

pub fn is_canary() -> bool {
  option_env!("DENO_CANARY").is_some()
}

pub fn release_version_or_canary_commit_hash() -> &'static str {
  if is_canary() {
    GIT_COMMIT_HASH
  } else {
    "2.0.0-rc.0"
  }
}
