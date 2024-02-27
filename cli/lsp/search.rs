// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use deno_core::error::AnyError;
use deno_semver::Version;
use std::sync::Arc;

#[async_trait::async_trait]
pub trait PackageSearchApi {
  async fn search(&self, query: &str) -> Result<Arc<Vec<String>>, AnyError>;
  async fn versions(&self, name: &str) -> Result<Arc<Vec<Version>>, AnyError>;
}

#[cfg(test)]
pub mod tests {
  use super::*;
  use deno_core::anyhow::anyhow;
  use std::collections::BTreeMap;
  use std::collections::BTreeSet;

  #[derive(Debug, Default)]
  pub struct TestPackageSearchApi {
    package_versions: BTreeMap<String, BTreeSet<Version>>,
  }

  impl TestPackageSearchApi {
    pub fn with_package_version(mut self, name: &str, version: &str) -> Self {
      self
        .package_versions
        .entry(name.to_string())
        .or_default()
        .insert(Version::parse_standard(version).unwrap());
      self
    }
  }

  #[async_trait::async_trait]
  impl PackageSearchApi for TestPackageSearchApi {
    async fn search(&self, query: &str) -> Result<Arc<Vec<String>>, AnyError> {
      let names = self
        .package_versions
        .keys()
        .filter_map(|n| n.contains(query).then(|| n.clone()))
        .collect::<Vec<_>>();
      Ok(Arc::new(names))
    }

    async fn versions(
      &self,
      name: &str,
    ) -> Result<Arc<Vec<Version>>, AnyError> {
      let Some(versions) = self.package_versions.get(name) else {
        return Err(anyhow!("Package not found."));
      };
      Ok(Arc::new(versions.iter().rev().cloned().collect()))
    }
  }
}
