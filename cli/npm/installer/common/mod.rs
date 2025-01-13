// Copyright 2018-2025 the Deno authors. MIT license.

use async_trait::async_trait;
use deno_error::JsErrorBox;

use super::PackageCaching;

pub mod bin_entries;
pub mod lifecycle_scripts;

/// Part of the resolution that interacts with the file system.
#[async_trait(?Send)]
pub trait NpmPackageFsInstaller: std::fmt::Debug + Send + Sync {
  async fn cache_packages<'a>(
    &self,
    caching: PackageCaching<'a>,
  ) -> Result<(), JsErrorBox>;
}
