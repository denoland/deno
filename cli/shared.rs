// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

/// This module is shared between build script and the binaries. Use it sparsely.
use serde::Deserialize;
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
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
}
