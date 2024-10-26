// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use deno_core::error::AnyError;
use deno_semver::package::PackageNv;
use deno_semver::Version;
use std::sync::Arc;

#[async_trait::async_trait]
pub trait PackageSearchApi {
  async fn search(&self, query: &str) -> Result<Arc<Vec<String>>, AnyError>;
  async fn versions(&self, name: &str) -> Result<Arc<Vec<Version>>, AnyError>;
  async fn exports(&self, nv: &PackageNv)
    -> Result<Arc<Vec<String>>, AnyError>;
}

#[cfg(test)]
pub mod tests {
  use super::*;
  use deno_core::anyhow::anyhow;
  use std::collections::BTreeMap;

  #[derive(Debug, Default)]
  pub struct TestPackageSearchApi {
    /// [(name -> [(version -> [export])])]
    package_versions: BTreeMap<String, BTreeMap<Version, Vec<String>>>,
  }

  impl TestPackageSearchApi {
    pub fn with_package_version(
      mut self,
      name: &str,
      version: &str,
      exports: &[&str],
    ) -> Self {
      let exports_by_version =
        self.package_versions.entry(name.to_string()).or_default();
      exports_by_version.insert(
        Version::parse_standard(version).unwrap(),
        exports.iter().map(|s| s.to_string()).collect(),
      );
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
      let Some(exports_by_version) = self.package_versions.get(name) else {
        return Err(anyhow!("Package not found."));
      };
      Ok(Arc::new(exports_by_version.keys().rev().cloned().collect()))
    }

    async fn exports(
      &self,
      nv: &PackageNv,
    ) -> Result<Arc<Vec<String>>, AnyError> {
      let Some(exports_by_version) = self.package_versions.get(&nv.name) else {
        return Err(anyhow!("Package not found."));
      };
      let Some(exports) = exports_by_version.get(&nv.version) else {
        return Err(anyhow!("Package version not found."));
      };
      Ok(Arc::new(exports.clone()))
    }
  }
}
