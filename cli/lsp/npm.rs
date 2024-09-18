// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use dashmap::DashMap;
use deno_core::anyhow::anyhow;
use deno_core::error::AnyError;
use deno_core::serde_json;
use deno_semver::package::PackageNv;
use deno_semver::Version;
use serde::Deserialize;
use std::sync::Arc;

use crate::args::npm_registry_url;
use crate::file_fetcher::FileFetcher;
use crate::npm::NpmFetchResolver;

use super::search::PackageSearchApi;

#[derive(Debug)]
pub struct CliNpmSearchApi {
  file_fetcher: Arc<FileFetcher>,
  resolver: NpmFetchResolver,
  search_cache: DashMap<String, Arc<Vec<String>>>,
  versions_cache: DashMap<String, Arc<Vec<Version>>>,
}

impl CliNpmSearchApi {
  pub fn new(file_fetcher: Arc<FileFetcher>) -> Self {
    let resolver = NpmFetchResolver::new(file_fetcher.clone());
    Self {
      file_fetcher,
      resolver,
      search_cache: Default::default(),
      versions_cache: Default::default(),
    }
  }

  pub fn clear_cache(&self) {
    self.file_fetcher.clear_memory_files();
    self.search_cache.clear();
    self.versions_cache.clear();
  }
}

#[async_trait::async_trait]
impl PackageSearchApi for CliNpmSearchApi {
  async fn search(&self, query: &str) -> Result<Arc<Vec<String>>, AnyError> {
    if let Some(names) = self.search_cache.get(query) {
      return Ok(names.clone());
    }
    let mut search_url = npm_registry_url().join("-/v1/search")?;
    search_url
      .query_pairs_mut()
      .append_pair("text", &format!("{} boost-exact:false", query));
    let file_fetcher = self.file_fetcher.clone();
    let file = deno_core::unsync::spawn(async move {
      file_fetcher
        .fetch_bypass_permissions(&search_url)
        .await?
        .into_text_decoded()
    })
    .await??;
    let names = Arc::new(parse_npm_search_response(&file.source)?);
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
      .ok_or_else(|| anyhow!("npm package info not found: {}", name))?;
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
