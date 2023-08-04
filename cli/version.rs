// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

pub const GIT_COMMIT_HASH: &str = env!("GIT_COMMIT_HASH");
pub const TYPESCRIPT: &str = env!("TS_VERSION");

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum ReleaseState {
  Unpromoted,
  Released,
}

#[derive(Copy, Clone, Debug)]
#[repr(C, align(128))]
pub struct ReleaseInfo {
  pub metadata_magic: u128,
  pub metadata_version: u32,
  pub git_hash: [u8; 40],
  pub version_string: [u8; 16],
  pub release_state: ReleaseState,
}

impl ReleaseInfo {
  pub fn git_hash(&self) -> &str {
    std::str::from_utf8(&self.git_hash)
      .unwrap_or("(invalid bytes)")
      .trim_end_matches('\0')
  }

  pub fn version_string(&self) -> &str {
    std::str::from_utf8(&self.version_string)
      .unwrap_or("(invalid bytes)")
      .trim_end_matches('\0')
  }
}

pub const DENO_METADATA_BLOCK_SIGNATURE: u128 =
  0x6b6f6c62617461646174656d6f6e6564;

#[used]
#[no_mangle]
static mut DENO_RELEASE_SYMBOL: ReleaseInfo = ReleaseInfo {
  metadata_magic: DENO_METADATA_BLOCK_SIGNATURE,
  metadata_version: 1,
  git_hash: /*git_hash_as_bytes!()*/ [0; 40],
  version_string: [0; 16],
  release_state: ReleaseState::Unpromoted,
};

pub fn deno() -> &'static str {
  if is_canary() {
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
  if is_canary() {
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

pub fn is_canary() -> bool {
  option_env!("DENO_CANARY").is_some()
}

pub fn release_version_or_canary_commit_hash() -> &'static str {
  if is_canary() {
    GIT_COMMIT_HASH
  } else {
    env!("CARGO_PKG_VERSION")
  }
}
