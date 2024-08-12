// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

/// This module is shared between build script and the binaries. Use it sparsely.
use deno_core::anyhow::bail;
use deno_core::error::AnyError;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ReleaseChannel {
  /// Stable version, eg. 1.45.4, 2.0.0, 2.1.0
  Stable,

  /// Pointing to a git hash
  Canary,

  /// Long term support release
  #[allow(unused)]
  Lts,

  /// Release candidate, poiting to a git hash
  Rc,
}

impl ReleaseChannel {
  pub const fn name(&self) -> &str {
    match self {
      Self::Stable => "latest",
      Self::Canary => "canary",
      Self::Rc => "release candidate",
      Self::Lts => "LTS (long term support)",
    }
  }

  pub fn serialize(&self) -> String {
    match self {
      Self::Stable => "stable",
      Self::Canary => "canary",
      Self::Rc => "rc",
      Self::Lts => "lts",
    }
    .to_string()
  }

  pub fn deserialize(str_: &str) -> Result<Self, AnyError> {
    Ok(match str_ {
      "stable" => Self::Stable,
      "canary" => Self::Canary,
      "rc" => Self::Rc,
      "lts" => Self::Lts,
      unknown => bail!("Unrecognized release channel: {}", unknown),
    })
  }
}
