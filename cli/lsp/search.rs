// Copyright 2018-2025 the Deno authors. MIT license.

use std::sync::Arc;

use deno_core::error::AnyError;
use deno_semver::package::PackageNv;
use deno_semver::Version;

#[async_trait::async_trait(?Send)]
pub trait PackageSearchApi {
  async fn search(&self, query: &str) -> Result<Arc<Vec<String>>, AnyError>;
  async fn versions(&self, name: &str) -> Result<Arc<Vec<Version>>, AnyError>;
  async fn exports(&self, nv: &PackageNv)
    -> Result<Arc<Vec<String>>, AnyError>;
}

#[cfg(test)]
pub mod tests {
  use std::collections::BTreeMap;

  use deno_core::anyhow::anyhow;

  use super::*;

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

  #[async_trait::async_trait(?Send)]
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
      let Some(exports_by_version) =
        self.package_versions.get(nv.name.as_str())
      else {
        return Err(anyhow!("Package not found."));
      };
      let Some(exports) = exports_by_version.get(&nv.version) else {
        return Err(anyhow!("Package version not found."));
      };
      Ok(Arc::new(exports.clone()))
    }
  }
}
