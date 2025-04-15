// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;

use deno_runtime::deno_telemetry::OtelRuntimeConfig;

use crate::shared::ReleaseChannel;

pub fn otel_runtime_config() -> OtelRuntimeConfig {
  OtelRuntimeConfig {
    runtime_name: Cow::Borrowed("deno"),
    runtime_version: Cow::Borrowed(crate::version::DENO_VERSION_INFO.deno),
  }
}

const GIT_COMMIT_HASH: &str = env!("GIT_COMMIT_HASH");
const TYPESCRIPT: &str = "5.7.3";
pub const DENO_VERSION: &str = env!("DENO_VERSION");
// TODO(bartlomieju): ideally we could remove this const.
const IS_CANARY: bool = option_env!("DENO_CANARY").is_some();
// TODO(bartlomieju): this is temporary, to allow Homebrew to cut RC releases as well
const IS_RC: bool = option_env!("DENO_RC").is_some();

pub static DENO_VERSION_INFO: std::sync::LazyLock<DenoVersionInfo> =
  std::sync::LazyLock::new(|| {
    let release_channel = libsui::find_section("denover")
      .and_then(|buf| std::str::from_utf8(buf).ok())
      .and_then(|str_| ReleaseChannel::deserialize(str_).ok())
      .unwrap_or({
        if IS_CANARY {
          ReleaseChannel::Canary
        } else if IS_RC {
          ReleaseChannel::Rc
        } else {
          ReleaseChannel::Stable
        }
      });

    DenoVersionInfo {
      deno: if release_channel == ReleaseChannel::Canary {
        concat!(env!("DENO_VERSION"), "+", env!("GIT_COMMIT_HASH_SHORT"))
      } else {
        env!("DENO_VERSION")
      },

      release_channel,

      git_hash: GIT_COMMIT_HASH,

      // Keep in sync with `deno` field.
      user_agent: if release_channel == ReleaseChannel::Canary {
        concat!(
          "Deno/",
          env!("DENO_VERSION"),
          "+",
          env!("GIT_COMMIT_HASH_SHORT")
        )
      } else {
        concat!("Deno/", env!("DENO_VERSION"))
      },

      typescript: TYPESCRIPT,
    }
  });

pub struct DenoVersionInfo {
  /// Human-readable version of the current Deno binary.
  ///
  /// For stable release, a semver, eg. `v1.46.2`.
  /// For canary release, a semver + 7-char git hash, eg. `v1.46.3+asdfqwq`.
  pub deno: &'static str,

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
    if self.release_channel == ReleaseChannel::Canary {
      self.git_hash
    } else {
      DENO_VERSION
    }
  }
}
