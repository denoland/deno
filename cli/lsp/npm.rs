// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use dashmap::DashMap;
use deno_core::anyhow::anyhow;
use deno_core::error::AnyError;
use deno_core::serde_json;
use deno_npm::registry::NpmPackageInfo;
use deno_runtime::permissions::PermissionsContainer;
use deno_semver::package::PackageNv;
use deno_semver::Version;
use serde::Deserialize;
use std::sync::Arc;

use crate::args::npm_registry_default_url;
use crate::file_fetcher::FileFetcher;

use super::search::PackageSearchApi;

#[derive(Debug, Clone)]
pub struct CliNpmSearchApi {
  file_fetcher: FileFetcher,
  search_cache: Arc<DashMap<String, Arc<Vec<String>>>>,
  versions_cache: Arc<DashMap<String, Arc<Vec<Version>>>>,
}

impl CliNpmSearchApi {
  pub fn new(file_fetcher: FileFetcher) -> Self {
    Self {
      file_fetcher,
      search_cache: Default::default(),
      versions_cache: Default::default(),
    }
  }
}

#[async_trait::async_trait]
impl PackageSearchApi for CliNpmSearchApi {
  async fn search(&self, query: &str) -> Result<Arc<Vec<String>>, AnyError> {
    if let Some(names) = self.search_cache.get(query) {
      return Ok(names.clone());
    }
    let mut search_url = npm_registry_default_url().clone();
    search_url
      .path_segments_mut()
      .map_err(|_| anyhow!("Custom npm registry URL cannot be a base."))?
      .pop_if_empty()
      .extend("-/v1/search".split('/'));
    search_url
      .query_pairs_mut()
      .append_pair("text", &format!("{} boost-exact:false", query));
    let file = self
      .file_fetcher
      .fetch(&search_url, PermissionsContainer::allow_all())
      .await?
      .into_text_decoded()?;
    let names = Arc::new(parse_npm_search_response(&file.source)?);
    self.search_cache.insert(query.to_string(), names.clone());
    Ok(names)
  }

  async fn versions(&self, name: &str) -> Result<Arc<Vec<Version>>, AnyError> {
    if let Some(versions) = self.versions_cache.get(name) {
      return Ok(versions.clone());
    }
    let mut info_url = npm_registry_default_url().clone();
    info_url
      .path_segments_mut()
      .map_err(|_| anyhow!("Custom npm registry URL cannot be a base."))?
      .pop_if_empty()
      .push(name);
    let file = self
      .file_fetcher
      .fetch(&info_url, PermissionsContainer::allow_all())
      .await?;
    let info = serde_json::from_slice::<NpmPackageInfo>(&file.source)?;
    let mut versions = info.versions.into_keys().collect::<Vec<_>>();
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
    _nv: &PackageNv,
  ) -> Result<Arc<Vec<String>>, AnyError> {
    Ok(Default::default())
  }
}

fn parse_npm_search_response(source: &str) -> Result<Vec<String>, AnyError> {
  #[derive(Debug, Deserialize)]
  struct Package {
    name: String,
  }
  #[derive(Debug, Deserialize)]
  struct Object {
    package: Package,
  }
  #[derive(Debug, Deserialize)]
  struct Response {
    objects: Vec<Object>,
  }
  let objects = serde_json::from_str::<Response>(source)?.objects;
  Ok(objects.into_iter().map(|o| o.package.name).collect())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_parse_npm_search_response() {
    // This is a subset of a realistic response only containing data currently
    // used by our parser. It's enough to catch regressions.
    let names = parse_npm_search_response(r#"{"objects":[{"package":{"name":"puppeteer"}},{"package":{"name":"puppeteer-core"}},{"package":{"name":"puppeteer-extra-plugin-stealth"}},{"package":{"name":"puppeteer-extra-plugin"}}]}"#).unwrap();
    assert_eq!(
      names,
      vec![
        "puppeteer".to_string(),
        "puppeteer-core".to_string(),
        "puppeteer-extra-plugin-stealth".to_string(),
        "puppeteer-extra-plugin".to_string()
      ]
    );
  }
}
