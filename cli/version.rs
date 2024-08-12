// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

pub use crate::shared::ReleaseChannel;

const GIT_COMMIT_HASH: &str = env!("GIT_COMMIT_HASH");
const TYPESCRIPT: &str = env!("TS_VERSION");
const IS_CANARY: bool = option_env!("DENO_CANARY").is_some();
const RELEASE_CHANNEL: ReleaseChannel = if IS_CANARY {
  ReleaseChannel::Canary
} else {
  ReleaseChannel::Stable
};

pub const DENO_VERSION_INFO: DenoVersionInfo = DenoVersionInfo {
  deno: if IS_CANARY {
    concat!(
      env!("CARGO_PKG_VERSION"),
      "+",
      env!("GIT_COMMIT_HASH_SHORT")
    )
  } else {
    env!("CARGO_PKG_VERSION")
  },

  // TODO(bartlomieju): remove, use `release_channel` instead
  is_canary: IS_CANARY,

  release_channel: RELEASE_CHANNEL,

  version: env!("CARGO_PKG_VERSION"),

  git_hash: env!("GIT_COMMIT_HASH"),

  git_hash_short: env!("GIT_COMMIT_HASH_SHORT"),

  version_or_git_hash: if IS_CANARY {
    GIT_COMMIT_HASH
  } else {
    env!("CARGO_PKG_VERSION")
  },

  // Keep in sync with `deno` field.
  user_agent: if IS_CANARY {
    concat!(
      "Deno/",
      env!("CARGO_PKG_VERSION"),
      "+",
      env!("GIT_COMMIT_HASH_SHORT")
    )
  } else {
    concat!("Deno/", env!("CARGO_PKG_VERSION"))
  },

  typescript: TYPESCRIPT,
};

pub struct DenoVersionInfo {
  /// Human-readable version of the current Deno binary.
  ///
  /// For stable release, a semver, eg. `v1.46.2`.
  /// For canary release, a semver + 7-char git hash, eg. `v1.46.3+asdfqwq`.
  pub deno: &'static str,

  // TODO(bartlomieju): remove, use `release_channel` instead
  pub is_canary: bool,

  #[allow(unused)]
  pub release_channel: ReleaseChannel,

  #[allow(unused)]
  /// Version from `Cargo.toml`.
  pub version: &'static str,

  /// A full git hash.
  pub git_hash: &'static str,

  #[allow(unused)]
  /// A 7-char git hash.
  pub git_hash_short: &'static str,

  /// For stable release, a semver like, eg. `v1.46.2`.
  /// For canary release a full git hash, eg. `9bdab6fb6b93eb43b1930f40987fa4997287f9c8`.
  pub version_or_git_hash: &'static str,

  /// A user-agent header that will be used in HTTP client.
  pub user_agent: &'static str,

  pub typescript: &'static str,
}
