// Copyright 2018-2025 the Deno authors. MIT license.

/// This module is shared between build script and the binaries. Use it sparsely.
use thiserror::Error;

#[derive(Debug, Error)]
#[error("Unrecognized release channel: {0}")]
pub struct UnrecognizedReleaseChannelError(pub String);

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ReleaseChannel {
  /// Stable version, eg. 1.45.4, 2.0.0, 2.1.0
  #[allow(unused)]
  Stable,

  /// Pointing to a git hash
  #[allow(unused)]
  Canary,

  /// Long term support release
  #[allow(unused)]
  Lts,

  /// Release candidate, eg. 1.46.0-rc.0, 2.0.0-rc.1
  #[allow(unused)]
  Rc,
}

impl ReleaseChannel {
  #[allow(unused)]
  pub fn name(&self) -> &str {
    match self {
      Self::Stable => "stable",
      Self::Canary => "canary",
      Self::Rc => "release candidate",
      Self::Lts => "long term support",
    }
  }

  // NOTE(bartlomieju): do not ever change these values, tools like `patchver`
  // rely on them.
  #[allow(unused)]
  pub fn serialize(&self) -> String {
    match self {
      Self::Stable => "stable",
      Self::Canary => "canary",
      Self::Rc => "rc",
      Self::Lts => "lts",
    }
    .to_string()
  }

  // NOTE(bartlomieju): do not ever change these values, tools like `patchver`
  // rely on them.
  #[allow(unused)]
  pub fn deserialize(
    str_: &str,
  ) -> Result<Self, UnrecognizedReleaseChannelError> {
    Ok(match str_ {
      "stable" => Self::Stable,
      "canary" => Self::Canary,
      "rc" => Self::Rc,
      "lts" => Self::Lts,
      unknown => {
        return Err(UnrecognizedReleaseChannelError(unknown.to_string()));
      }
    })
  }
}
