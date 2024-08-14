// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use once_cell::sync::Lazy;
use serde::Deserialize;
use serde::Serialize;

pub use crate::shared::ReleaseChannel;

const GIT_COMMIT_HASH: &str = env!("GIT_COMMIT_HASH");
const TYPESCRIPT: &str = env!("TS_VERSION");
const CARGO_PKG_VERSION: &str = env!("CARGO_PKG_VERSION");
const IS_CANARY: bool = option_env!("DENO_CANARY").is_some();

pub static DENO_VERSION_INFO: Lazy<DenoVersionInfo> = Lazy::new(|| {
  let release_channel = libsui::find_section("denoreleasechannel")
    .and_then(|buf| std::str::from_utf8(buf).ok())
    .and_then(|str_| ReleaseChannel::deserialize(str_).ok())
    .unwrap_or({
      if IS_CANARY {
        ReleaseChannel::Canary
      } else {
        ReleaseChannel::Stable
      }
    });

  DenoVersionInfo {
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

    release_channel,

    git_hash: GIT_COMMIT_HASH,

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
  }
});

#[derive(Deserialize, Serialize)]
pub struct DenoVersionInfo {
  /// Human-readable version of the current Deno binary.
  ///
  /// For stable release, a semver, eg. `v1.46.2`.
  /// For canary release, a semver + 7-char git hash, eg. `v1.46.3+asdfqwq`.
  pub deno: &'static str,

  // TODO(bartlomieju): remove, use `release_channel` instead
  pub is_canary: bool,

  pub release_channel: ReleaseChannel,

  /// A full git hash.
  pub git_hash: &'static str,

  /// A user-agent header that will be used in HTTP client.
  pub user_agent: &'static str,

  pub typescript: &'static str,
}

impl DenoVersionInfo {
  /// For stable release, a semver like, eg. `v1.46.2`.
  /// For canary release a full git hash, eg. `9bdab6fb6b93eb43b1930f40987fa4997287f9c8`.
  pub fn version_or_git_hash(&self) -> &'static str {
    if IS_CANARY {
      self.git_hash
    } else {
      CARGO_PKG_VERSION
    }
  }
}
