// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::collections::HashMap;
use std::sync::Arc;

use deno_core::anyhow::anyhow;
use deno_core::error::AnyError;
use deno_core::parking_lot::Mutex;
use deno_core::serde_json;
use deno_core::url::Url;
use deno_npm::registry::NpmPackageInfo;
use deno_runtime::permissions::PermissionsContainer;
use serde::Deserialize;

use crate::args::npm_registry_default_url;
use crate::file_fetcher::FileFetcher;

#[async_trait::async_trait]
pub trait NpmSearchApi {
  async fn search(&self, query: &str) -> Result<Arc<Vec<String>>, AnyError>;
  async fn package_info(
    &self,
    name: &str,
  ) -> Result<Arc<NpmPackageInfo>, AnyError>;
}

#[derive(Debug, Clone)]
pub struct CliNpmSearchApi {
  base_url: Url,
  file_fetcher: FileFetcher,
  info_cache: Arc<Mutex<HashMap<String, Arc<NpmPackageInfo>>>>,
  search_cache: Arc<Mutex<HashMap<String, Arc<Vec<String>>>>>,
}

impl CliNpmSearchApi {
  pub fn new(file_fetcher: FileFetcher, custom_base_url: Option<Url>) -> Self {
    Self {
      base_url: custom_base_url
        .unwrap_or_else(|| npm_registry_default_url().clone()),
      file_fetcher,
      info_cache: Default::default(),
      search_cache: Default::default(),
    }
  }
}

#[async_trait::async_trait]
impl NpmSearchApi for CliNpmSearchApi {
  async fn search(&self, query: &str) -> Result<Arc<Vec<String>>, AnyError> {
    if let Some(names) = self.search_cache.lock().get(query) {
      return Ok(names.clone());
    }
    let mut search_url = self.base_url.clone();
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
      .await?;
    let names = Arc::new(parse_npm_search_response(&file.source)?);
    self
      .search_cache
      .lock()
      .insert(query.to_string(), names.clone());
    Ok(names)
  }

  async fn package_info(
    &self,
    name: &str,
  ) -> Result<Arc<NpmPackageInfo>, AnyError> {
    if let Some(info) = self.info_cache.lock().get(name) {
      return Ok(info.clone());
    }
    let mut info_url = self.base_url.clone();
    info_url
      .path_segments_mut()
      .map_err(|_| anyhow!("Custom npm registry URL cannot be a base."))?
      .pop_if_empty()
      .push(name);
    let file = self
      .file_fetcher
      .fetch(&info_url, PermissionsContainer::allow_all())
      .await?;
    let info = Arc::new(serde_json::from_str::<NpmPackageInfo>(&file.source)?);
    self
      .info_cache
      .lock()
      .insert(name.to_string(), info.clone());
    Ok(info)
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
