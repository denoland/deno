// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use deno_core::anyhow::bail;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_core::parking_lot::Mutex;
use deno_core::serde::Deserialize;
use deno_core::serde_json;
use deno_core::url::Url;
use deno_runtime::deno_fetch::reqwest;
use serde::Serialize;

use crate::fs_util;
use crate::http_cache::CACHE_PERM;

use super::cache::NpmCache;
use super::resolution::NpmVersionMatcher;

// npm registry docs: https://github.com/npm/registry/blob/master/docs/REGISTRY-API.md

#[derive(Deserialize, Serialize, Clone)]
pub struct NpmPackageInfo {
  pub name: String,
  pub versions: HashMap<String, NpmPackageVersionInfo>,
}

pub struct NpmDependencyEntry {
  pub bare_specifier: String,
  pub name: String,
  pub version_req: NpmVersionReq,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct NpmPackageVersionInfo {
  pub version: String,
  pub dist: NpmPackageVersionDistInfo,
  // Bare specifier to version (ex. `"typescript": "^3.0.1") or possibly
  // package and version (ex. `"typescript-3.0.1": "npm:typescript@3.0.1"`).
  #[serde(default)]
  pub dependencies: HashMap<String, String>,
}

impl NpmPackageVersionInfo {
  pub fn dependencies_as_entries(
    &self,
  ) -> Result<Vec<NpmDependencyEntry>, AnyError> {
    fn entry_as_bare_specifier_and_reference(
      entry: (&String, &String),
    ) -> Result<NpmDependencyEntry, AnyError> {
      let bare_specifier = entry.0.clone();
      let (name, version_req) =
        if let Some(package_and_version) = entry.1.strip_prefix("npm:") {
          if let Some((name, version)) = package_and_version.rsplit_once('@') {
            (name.to_string(), version.to_string())
          } else {
            bail!("could not find @ symbol in npm url '{}'", entry.1);
          }
        } else {
          (entry.0.clone(), entry.1.clone())
        };
      let version_req = NpmVersionReq::parse(&version_req)
        .with_context(|| format!("Dependency: {}", bare_specifier))?;
      Ok(NpmDependencyEntry {
        bare_specifier,
        name,
        version_req,
      })
    }

    self
      .dependencies
      .iter()
      .map(entry_as_bare_specifier_and_reference)
      .collect::<Result<Vec<_>, AnyError>>()
  }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct NpmPackageVersionDistInfo {
  /// URL to the tarball.
  pub tarball: String,
  pub shasum: String,
  pub integrity: Option<String>,
}

#[derive(Clone)]
pub struct NpmRegistryApi {
  base_url: Url,
  cache: NpmCache,
  mem_cache: Arc<Mutex<HashMap<String, Option<NpmPackageInfo>>>>,
  reload: bool,
}

impl NpmRegistryApi {
  pub fn default_url() -> Url {
    Url::parse("https://registry.npmjs.org").unwrap()
  }

  pub fn new(cache: NpmCache, reload: bool) -> Self {
    Self::from_base(Self::default_url(), cache, reload)
  }

  pub fn from_base(base_url: Url, cache: NpmCache, reload: bool) -> Self {
    Self {
      base_url,
      cache,
      mem_cache: Default::default(),
      reload,
    }
  }

  pub fn base_url(&self) -> &Url {
    &self.base_url
  }

  pub async fn package_info(
    &self,
    name: &str,
  ) -> Result<NpmPackageInfo, AnyError> {
    let maybe_package_info = self.maybe_package_info(name).await?;
    match maybe_package_info {
      Some(package_info) => Ok(package_info),
      None => bail!("package '{}' does not exist", name),
    }
  }

  pub async fn maybe_package_info(
    &self,
    name: &str,
  ) -> Result<Option<NpmPackageInfo>, AnyError> {
    let maybe_info = self.mem_cache.lock().get(name).cloned();
    if let Some(info) = maybe_info {
      Ok(info)
    } else {
      let mut maybe_package_info = None;
      if !self.reload {
        // attempt to load from the file cache
        maybe_package_info = self.load_file_cached_package_info(name);
      }
      if maybe_package_info.is_none() {
        maybe_package_info = self
          .load_package_info_from_registry(name)
          .await
          .with_context(|| {
          format!("Error getting response at {}", self.get_package_url(name))
        })?;
      }

      // Not worth the complexity to ensure multiple in-flight requests
      // for the same package only request once because with how this is
      // used that should never happen.
      let mut mem_cache = self.mem_cache.lock();
      Ok(match mem_cache.get(name) {
        // another thread raced here, so use its result instead
        Some(info) => info.clone(),
        None => {
          mem_cache.insert(name.to_string(), maybe_package_info.clone());
          maybe_package_info
        }
      })
    }
  }

  fn load_file_cached_package_info(
    &self,
    name: &str,
  ) -> Option<NpmPackageInfo> {
    let file_cache_path = self.get_package_file_cache_path(name);
    let file_text = fs::read_to_string(file_cache_path).ok()?;
    match serde_json::from_str(&file_text) {
      Ok(result) => Some(result),
      Err(err) => {
        if cfg!(debug_assertions) {
          panic!("could not deserialize: {:#}", err);
        } else {
          None
        }
      }
    }
  }

  fn save_package_info_to_file_cache(
    &self,
    name: &str,
    package_info: &NpmPackageInfo,
  ) {
    let file_cache_path = self.get_package_file_cache_path(name);
    let file_text = serde_json::to_string_pretty(&package_info).unwrap();
    let _ignore =
      fs_util::atomic_write_file(&file_cache_path, file_text, CACHE_PERM);
  }

  async fn load_package_info_from_registry(
    &self,
    name: &str,
  ) -> Result<Option<NpmPackageInfo>, AnyError> {
    let response = match reqwest::get(self.get_package_url(name)).await {
      Ok(response) => response,
      Err(err) => {
        // attempt to use the local cache
        if let Some(info) = self.load_file_cached_package_info(name) {
          return Ok(Some(info));
        } else {
          return Err(err.into());
        }
      }
    };

    if response.status() == 404 {
      Ok(None)
    } else if !response.status().is_success() {
      bail!("Bad response: {:?}", response.status());
    } else {
      let bytes = response.bytes().await?;
      let package_info = serde_json::from_slice(&bytes)?;
      self.save_package_info_to_file_cache(name, &package_info);
      Ok(Some(package_info))
    }
  }

  fn get_package_url(&self, name: &str) -> Url {
    self.base_url.join(name).unwrap()
  }

  fn get_package_file_cache_path(&self, name: &str) -> PathBuf {
    let name_folder_path = self.cache.package_name_folder(name, &self.base_url);
    name_folder_path.join("registry.json")
  }
}

/// A version requirement found in an npm package's dependencies.
pub struct NpmVersionReq {
  raw_text: String,
  comparators: Vec<semver::VersionReq>,
}

impl NpmVersionReq {
  pub fn parse(text: &str) -> Result<NpmVersionReq, AnyError> {
    // semver::VersionReq doesn't support spaces between comparators
    // and it doesn't support using || for "OR", so we pre-process
    // the version requirement in order to make this work.
    let raw_text = text.to_string();
    let part_texts = text.split("||").collect::<Vec<_>>();
    let mut comparators = Vec::with_capacity(part_texts.len());
    for part in part_texts {
      comparators.push(npm_version_req_parse_part(part)?);
    }
    Ok(NpmVersionReq {
      raw_text,
      comparators,
    })
  }
}

impl NpmVersionMatcher for NpmVersionReq {
  fn matches(&self, version: &semver::Version) -> bool {
    self.comparators.iter().any(|c| c.matches(version))
  }

  fn version_text(&self) -> String {
    self.raw_text.to_string()
  }
}

fn npm_version_req_parse_part(
  text: &str,
) -> Result<semver::VersionReq, AnyError> {
  let text = text.trim();
  let mut chars = text.chars().enumerate().peekable();
  let mut final_text = String::new();
  while chars.peek().is_some() {
    let (i, c) = chars.next().unwrap();
    let is_greater_or_less_than = c == '<' || c == '>';
    if is_greater_or_less_than || c == '=' {
      if i > 0 {
        final_text = final_text.trim().to_string();
        // add a comma to make semver::VersionReq parse this
        final_text.push(',');
      }
      final_text.push(c);
      let next_char = chars.peek().map(|(_, c)| c);
      if is_greater_or_less_than && matches!(next_char, Some('=')) {
        let c = chars.next().unwrap().1; // skip
        final_text.push(c);
      }
    } else {
      final_text.push(c);
    }
  }
  Ok(semver::VersionReq::parse(&final_text)?)
}

#[cfg(test)]
mod test {
  use super::*;

  struct NpmVersionReqTester(NpmVersionReq);

  impl NpmVersionReqTester {
    fn matches(&self, version: &str) -> bool {
      self.0.matches(&semver::Version::parse(version).unwrap())
    }
  }

  #[test]
  pub fn npm_version_req_ranges() {
    let tester = NpmVersionReqTester(
      NpmVersionReq::parse(">= 2.1.2 < 3.0.0 || 5.x").unwrap(),
    );
    assert!(!tester.matches("2.1.1"));
    assert!(tester.matches("2.1.2"));
    assert!(tester.matches("2.9.9"));
    assert!(!tester.matches("3.0.0"));
    assert!(tester.matches("5.0.0"));
    assert!(tester.matches("5.1.0"));
    assert!(!tester.matches("6.1.0"));
  }
}
