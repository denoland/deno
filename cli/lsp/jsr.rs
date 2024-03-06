// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use crate::args::jsr_api_url;
use crate::file_fetcher::FileFetcher;
use crate::jsr::JsrFetchResolver;
use dashmap::DashMap;
use deno_core::anyhow::anyhow;
use deno_core::error::AnyError;
use deno_core::serde_json;
use deno_runtime::permissions::PermissionsContainer;
use deno_semver::package::PackageNv;
use deno_semver::Version;
use serde::Deserialize;
use std::sync::Arc;

use super::search::PackageSearchApi;

#[derive(Debug)]
pub struct CliJsrSearchApi {
  file_fetcher: FileFetcher,
  resolver: JsrFetchResolver,
  search_cache: DashMap<String, Arc<Vec<String>>>,
  versions_cache: DashMap<String, Arc<Vec<Version>>>,
  exports_cache: DashMap<PackageNv, Arc<Vec<String>>>,
}

impl CliJsrSearchApi {
  pub fn new(file_fetcher: FileFetcher) -> Self {
    let resolver = JsrFetchResolver::new(file_fetcher.clone());
    Self {
      file_fetcher,
      resolver,
      search_cache: Default::default(),
      versions_cache: Default::default(),
      exports_cache: Default::default(),
    }
  }

  pub fn get_resolver(&self) -> &JsrFetchResolver {
    &self.resolver
  }
}

#[async_trait::async_trait]
impl PackageSearchApi for CliJsrSearchApi {
  async fn search(&self, query: &str) -> Result<Arc<Vec<String>>, AnyError> {
    if let Some(names) = self.search_cache.get(query) {
      return Ok(names.clone());
    }
    let mut search_url = jsr_api_url().join("packages")?;
    search_url.query_pairs_mut().append_pair("query", query);
    let file = self
      .file_fetcher
      .fetch(&search_url, PermissionsContainer::allow_all())
      .await?
      .into_text_decoded()?;
    let names = Arc::new(parse_jsr_search_response(&file.source)?);
    self.search_cache.insert(query.to_string(), names.clone());
    Ok(names)
  }

  async fn versions(&self, name: &str) -> Result<Arc<Vec<Version>>, AnyError> {
    if let Some(versions) = self.versions_cache.get(name) {
      return Ok(versions.clone());
    }
    let info = self
      .resolver
      .package_info(name)
      .await
      .ok_or_else(|| anyhow!("JSR package info not found: {}", name))?;
    let mut versions = info.versions.keys().cloned().collect::<Vec<_>>();
    versions.sort();
    versions.reverse();
    let versions = Arc::new(versions);
    self
      .versions_cache
      .insert(name.to_string(), versions.clone());
    Ok(versions)
  }

  async fn exports(
    &self,
    nv: &PackageNv,
  ) -> Result<Arc<Vec<String>>, AnyError> {
    if let Some(exports) = self.exports_cache.get(nv) {
      return Ok(exports.clone());
    }
    let info = self
      .resolver
      .package_version_info(nv)
      .await
      .ok_or_else(|| anyhow!("JSR package version info not found: {}", nv))?;
    let mut exports = info
      .exports()
      .map(|(n, _)| n.to_string())
      .collect::<Vec<_>>();
    exports.sort();
    let exports = Arc::new(exports);
    self.exports_cache.insert(nv.clone(), exports.clone());
    Ok(exports)
  }
}

fn parse_jsr_search_response(source: &str) -> Result<Vec<String>, AnyError> {
  #[derive(Debug, Deserialize)]
  #[serde(rename_all = "camelCase")]
  struct Item {
    scope: String,
    name: String,
    version_count: usize,
  }
  #[derive(Debug, Deserialize)]
  #[serde(rename_all = "camelCase")]
  struct Response {
    items: Vec<Item>,
  }
  let items = serde_json::from_str::<Response>(source)?.items;
  Ok(
    items
      .into_iter()
      .filter(|i| i.version_count > 0)
      .map(|i| format!("@{}/{}", i.scope, i.name))
      .collect(),
  )
}
