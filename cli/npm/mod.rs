// Copyright 2018-2025 the Deno authors. MIT license.

pub mod installer;
mod managed;

use std::collections::HashMap;
use std::sync::Arc;

use dashmap::DashMap;
use deno_config::workspace::Workspace;
use deno_core::futures::stream::FuturesOrdered;
use deno_core::futures::TryStreamExt;
use deno_core::serde_json;
use deno_core::url::Url;
use deno_error::JsErrorBox;
use deno_lib::version::DENO_VERSION_INFO;
use deno_npm::npm_rc::ResolvedNpmRc;
use deno_npm::registry::NpmPackageInfo;
use deno_npm::registry::NpmPackageVersionInfo;
use deno_npm::registry::NpmRegistryApi;
use deno_npm::resolution::DefaultTarballUrlProvider;
use deno_resolver::npm::ByonmNpmResolverCreateOptions;
use deno_runtime::colors;
use deno_semver::package::PackageName;
use deno_semver::package::PackageNv;
use deno_semver::package::PackageReq;
use deno_semver::SmallStackString;
use deno_semver::StackString;
use deno_semver::Version;
use http::HeaderName;
use http::HeaderValue;
use indexmap::IndexMap;
use thiserror::Error;

pub use self::managed::CliManagedNpmResolverCreateOptions;
pub use self::managed::CliNpmResolverManagedSnapshotOption;
pub use self::managed::NpmResolutionInitializer;
use crate::file_fetcher::CliFileFetcher;
use crate::http_util::HttpClientProvider;
use crate::npm::managed::DefaultTarballUrl;
use crate::sys::CliSys;
use crate::util::progress_bar::ProgressBar;

pub type CliNpmTarballCache =
  deno_npm_cache::TarballCache<CliNpmCacheHttpClient, CliSys>;
pub type CliNpmCache = deno_npm_cache::NpmCache<CliSys>;
pub type CliNpmRegistryInfoProvider =
  deno_npm_cache::RegistryInfoProvider<CliNpmCacheHttpClient, CliSys>;
pub type CliNpmResolver = deno_resolver::npm::NpmResolver<CliSys>;
pub type CliManagedNpmResolver = deno_resolver::npm::ManagedNpmResolver<CliSys>;
pub type CliNpmResolverCreateOptions =
  deno_resolver::npm::NpmResolverCreateOptions<CliSys>;
pub type CliByonmNpmResolverCreateOptions =
  ByonmNpmResolverCreateOptions<CliSys>;

pub struct NpmPackageInfoApiAdapter {
  api: Arc<dyn NpmRegistryApi + Send + Sync>,
  workspace_patch_packages: Arc<WorkspaceNpmPatchPackages>,
}

impl NpmPackageInfoApiAdapter {
  pub fn new(
    api: Arc<dyn NpmRegistryApi + Send + Sync>,
    workspace_patch_packages: Arc<WorkspaceNpmPatchPackages>,
  ) -> Self {
    Self {
      api,
      workspace_patch_packages,
    }
  }
}
async fn get_infos(
  info_provider: &(dyn NpmRegistryApi + Send + Sync),
  workspace_patch_packages: &WorkspaceNpmPatchPackages,
  values: &[PackageNv],
) -> Result<
  Vec<deno_lockfile::Lockfile5NpmInfo>,
  Box<dyn std::error::Error + Send + Sync>,
> {
  let futs = values
    .iter()
    .map(|v| async move {
      let info = info_provider.package_info(v.name.as_str()).await?;
      let version_info = info.version_info(v, &workspace_patch_packages.0)?;
      Ok::<_, Box<dyn std::error::Error + Send + Sync>>(
        deno_lockfile::Lockfile5NpmInfo {
          tarball_url: version_info.dist.as_ref().and_then(|d| {
            if d.tarball == DefaultTarballUrl.default_tarball_url(v) {
              None
            } else {
              Some(d.tarball.clone())
            }
          }),
          optional_dependencies: version_info
            .optional_dependencies
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect::<std::collections::BTreeMap<_, _>>(),
          cpu: version_info.cpu.iter().map(|s| s.to_string()).collect(),
          os: version_info.os.iter().map(|s| s.to_string()).collect(),
          deprecated: version_info.deprecated.is_some(),
          has_bin: version_info.bin.is_some(),
          has_scripts: version_info.scripts.contains_key("preinstall")
            || version_info.scripts.contains_key("install")
            || version_info.scripts.contains_key("postinstall"),
          optional_peers: version_info
            .peer_dependencies_meta
            .iter()
            .filter_map(|(k, v)| {
              if v.optional {
                version_info
                  .peer_dependencies
                  .get(k)
                  .map(|v| (k.to_string(), v.to_string()))
              } else {
                None
              }
            })
            .collect::<std::collections::BTreeMap<_, _>>(),
        },
      )
    })
    .collect::<FuturesOrdered<_>>();
  let package_infos = futs.try_collect::<Vec<_>>().await?;
  Ok(package_infos)
}

#[async_trait::async_trait(?Send)]
impl deno_lockfile::NpmPackageInfoProvider for NpmPackageInfoApiAdapter {
  async fn get_npm_package_info(
    &self,
    values: &[PackageNv],
  ) -> Result<
    Vec<deno_lockfile::Lockfile5NpmInfo>,
    Box<dyn std::error::Error + Send + Sync>,
  > {
    let package_infos =
      get_infos(&*self.api, &self.workspace_patch_packages, values).await;

    match package_infos {
      Ok(package_infos) => Ok(package_infos),
      Err(err) => {
        if self.api.mark_force_reload() {
          get_infos(&*self.api, &self.workspace_patch_packages, values).await
        } else {
          Err(err)
        }
      }
    }
  }
}

#[derive(Debug, Default)]
pub struct WorkspaceNpmPatchPackages(
  pub HashMap<PackageName, Vec<NpmPackageVersionInfo>>,
);

impl WorkspaceNpmPatchPackages {
  pub fn from_workspace(workspace: &Workspace) -> Self {
    let mut entries: HashMap<PackageName, Vec<NpmPackageVersionInfo>> =
      HashMap::new();
    if workspace.has_unstable("npm-patch") {
      for pkg_json in workspace.patch_pkg_jsons() {
        let Some(name) = pkg_json.name.as_ref() else {
          log::warn!(
          "{} Patch package ignored because package.json was missing name field.\n    at {}",
          colors::yellow("Warning"),
          pkg_json.path.display(),
        );
          continue;
        };
        match pkg_json_to_version_info(pkg_json) {
          Ok(version_info) => {
            let entry = entries.entry(PackageName::from_str(name)).or_default();
            entry.push(version_info);
          }
          Err(err) => {
            log::warn!(
              "{} {}\n    at {}",
              colors::yellow("Warning"),
              err.to_string(),
              pkg_json.path.display(),
            );
          }
        }
      }
    } else if workspace.patch_pkg_jsons().next().is_some() {
      log::warn!(
        "{} {}\n    at {}",
        colors::yellow("Warning"),
        "Patching npm packages is only supported when setting \"unstable\": [\"npm-patch\"] in the root deno.json",
        workspace
          .root_deno_json()
          .map(|d| d.specifier.to_string())
          .unwrap_or_else(|| workspace.root_dir().to_string()),
      );
    }
    Self(entries)
  }
}

#[derive(Debug, Error)]
enum PkgJsonToVersionInfoError {
  #[error(
    "Patch package ignored because package.json was missing version field."
  )]
  VersionMissing,
  #[error("Patch package ignored because package.json version field could not be parsed.")]
  VersionInvalid {
    #[source]
    source: deno_semver::npm::NpmVersionParseError,
  },
}

fn pkg_json_to_version_info(
  pkg_json: &deno_package_json::PackageJson,
) -> Result<NpmPackageVersionInfo, PkgJsonToVersionInfoError> {
  fn parse_deps(
    deps: Option<&IndexMap<String, String>>,
  ) -> HashMap<StackString, StackString> {
    deps
      .map(|d| {
        d.into_iter()
          .map(|(k, v)| (StackString::from_str(k), StackString::from_str(v)))
          .collect()
      })
      .unwrap_or_default()
  }

  fn parse_array(v: &[String]) -> Vec<SmallStackString> {
    v.iter().map(|s| SmallStackString::from_str(s)).collect()
  }

  let Some(version) = &pkg_json.version else {
    return Err(PkgJsonToVersionInfoError::VersionMissing);
  };

  let version = Version::parse_from_npm(version)
    .map_err(|source| PkgJsonToVersionInfoError::VersionInvalid { source })?;
  Ok(NpmPackageVersionInfo {
    version,
    dist: None,
    bin: pkg_json
      .bin
      .as_ref()
      .and_then(|v| serde_json::from_value(v.clone()).ok()),
    dependencies: parse_deps(pkg_json.dependencies.as_ref()),
    optional_dependencies: parse_deps(pkg_json.optional_dependencies.as_ref()),
    peer_dependencies: parse_deps(pkg_json.peer_dependencies.as_ref()),
    peer_dependencies_meta: pkg_json
      .peer_dependencies_meta
      .clone()
      .and_then(|m| serde_json::from_value(m).ok())
      .unwrap_or_default(),
    os: pkg_json.os.as_deref().map(parse_array).unwrap_or_default(),
    cpu: pkg_json.cpu.as_deref().map(parse_array).unwrap_or_default(),
    scripts: pkg_json
      .scripts
      .as_ref()
      .map(|scripts| {
        scripts
          .iter()
          .map(|(k, v)| (SmallStackString::from_str(k), v.clone()))
          .collect()
      })
      .unwrap_or_default(),
    // not worth increasing memory for showing a deprecated
    // message for patched packages
    deprecated: None,
  })
}

#[derive(Debug)]
pub struct CliNpmCacheHttpClient {
  http_client_provider: Arc<HttpClientProvider>,
  progress_bar: ProgressBar,
}

impl CliNpmCacheHttpClient {
  pub fn new(
    http_client_provider: Arc<HttpClientProvider>,
    progress_bar: ProgressBar,
  ) -> Self {
    Self {
      http_client_provider,
      progress_bar,
    }
  }
}

#[async_trait::async_trait(?Send)]
impl deno_npm_cache::NpmCacheHttpClient for CliNpmCacheHttpClient {
  async fn download_with_retries_on_any_tokio_runtime(
    &self,
    url: Url,
    maybe_auth_header: Option<(HeaderName, HeaderValue)>,
  ) -> Result<Option<Vec<u8>>, deno_npm_cache::DownloadError> {
    let guard = self.progress_bar.update(url.as_str());
    let client = self.http_client_provider.get_or_create().map_err(|err| {
      deno_npm_cache::DownloadError {
        status_code: None,
        error: err,
      }
    })?;
    client
      .download_with_progress_and_retries(url, maybe_auth_header, &guard)
      .await
      .map_err(|err| {
        use crate::http_util::DownloadErrorKind::*;
        let status_code = match err.as_kind() {
          Fetch { .. }
          | UrlParse { .. }
          | HttpParse { .. }
          | Json { .. }
          | ToStr { .. }
          | RedirectHeaderParse { .. }
          | TooManyRedirects
          | NotFound
          | Other(_) => None,
          BadResponse(bad_response_error) => {
            Some(bad_response_error.status_code)
          }
        };
        deno_npm_cache::DownloadError {
          status_code,
          error: JsErrorBox::from_err(err),
        }
      })
  }
}

#[derive(Debug)]
pub struct NpmFetchResolver {
  nv_by_req: DashMap<PackageReq, Option<PackageNv>>,
  info_by_name: DashMap<String, Option<Arc<NpmPackageInfo>>>,
  file_fetcher: Arc<CliFileFetcher>,
  npmrc: Arc<ResolvedNpmRc>,
}

impl NpmFetchResolver {
  pub fn new(
    file_fetcher: Arc<CliFileFetcher>,
    npmrc: Arc<ResolvedNpmRc>,
  ) -> Self {
    Self {
      nv_by_req: Default::default(),
      info_by_name: Default::default(),
      file_fetcher,
      npmrc,
    }
  }

  pub async fn req_to_nv(&self, req: &PackageReq) -> Option<PackageNv> {
    if let Some(nv) = self.nv_by_req.get(req) {
      return nv.value().clone();
    }
    let maybe_get_nv = || async {
      let name = req.name.clone();
      let package_info = self.package_info(&name).await?;
      if let Some(dist_tag) = req.version_req.tag() {
        let version = package_info.dist_tags.get(dist_tag)?.clone();
        return Some(PackageNv { name, version });
      }
      // Find the first matching version of the package.
      let mut versions = package_info.versions.keys().collect::<Vec<_>>();
      versions.sort();
      let version = versions
        .into_iter()
        .rev()
        .find(|v| req.version_req.tag().is_none() && req.version_req.matches(v))
        .cloned()?;
      Some(PackageNv { name, version })
    };
    let nv = maybe_get_nv().await;
    self.nv_by_req.insert(req.clone(), nv.clone());
    nv
  }

  pub async fn package_info(&self, name: &str) -> Option<Arc<NpmPackageInfo>> {
    if let Some(info) = self.info_by_name.get(name) {
      return info.value().clone();
    }
    // todo(#27198): use RegistryInfoProvider instead
    let fetch_package_info = || async {
      let info_url = deno_npm_cache::get_package_url(&self.npmrc, name);
      let registry_config = self.npmrc.get_registry_config(name);
      // TODO(bartlomieju): this should error out, not use `.ok()`.
      let maybe_auth_header =
        deno_npm_cache::maybe_auth_header_for_npm_registry(registry_config)
          .ok()?;
      let file = self
        .file_fetcher
        .fetch_bypass_permissions_with_maybe_auth(&info_url, maybe_auth_header)
        .await
        .ok()?;
      serde_json::from_slice::<NpmPackageInfo>(&file.source).ok()
    };
    let info = fetch_package_info().await.map(Arc::new);
    self.info_by_name.insert(name.to_string(), info.clone());
    info
  }
}

pub static NPM_CONFIG_USER_AGENT_ENV_VAR: &str = "npm_config_user_agent";

pub fn get_npm_config_user_agent() -> String {
  format!(
    "deno/{} npm/? deno/{} {} {}",
    DENO_VERSION_INFO.deno,
    DENO_VERSION_INFO.deno,
    std::env::consts::OS,
    std::env::consts::ARCH
  )
}

#[cfg(test)]
mod test {
  use std::path::PathBuf;

  use deno_npm::registry::NpmPeerDependencyMeta;

  use super::*;

  #[test]
  fn test_pkg_json_to_version_info() {
    fn convert(
      text: &str,
    ) -> Result<NpmPackageVersionInfo, PkgJsonToVersionInfoError> {
      let pkg_json = deno_package_json::PackageJson::load_from_string(
        PathBuf::from("package.json"),
        text,
      )
      .unwrap();
      pkg_json_to_version_info(&pkg_json)
    }

    assert_eq!(
      convert(
        r#"{
  "name": "pkg",
  "version": "1.0.0",
  "bin": "./bin.js",
  "dependencies": {
    "my-dep": "1"
  },
  "optionalDependencies": {
    "optional-dep": "~1"
  },
  "peerDependencies": {
    "my-peer-dep": "^2"
  },
  "peerDependenciesMeta": {
    "my-peer-dep": {
      "optional": true
    }
  },
  "os": ["win32"],
  "cpu": ["x86_64"],
  "scripts": {
    "script": "testing",
    "postInstall": "testing2"
  },
  "deprecated": "ignored for now"
}"#
      )
      .unwrap(),
      NpmPackageVersionInfo {
        version: Version::parse_from_npm("1.0.0").unwrap(),
        dist: None,
        bin: Some(deno_npm::registry::NpmPackageVersionBinEntry::String(
          "./bin.js".to_string()
        )),
        dependencies: HashMap::from([(
          StackString::from_static("my-dep"),
          StackString::from_static("1")
        )]),
        optional_dependencies: HashMap::from([(
          StackString::from_static("optional-dep"),
          StackString::from_static("~1")
        )]),
        peer_dependencies: HashMap::from([(
          StackString::from_static("my-peer-dep"),
          StackString::from_static("^2")
        )]),
        peer_dependencies_meta: HashMap::from([(
          StackString::from_static("my-peer-dep"),
          NpmPeerDependencyMeta { optional: true }
        )]),
        os: vec![SmallStackString::from_static("win32")],
        cpu: vec![SmallStackString::from_static("x86_64")],
        scripts: HashMap::from([
          (
            SmallStackString::from_static("script"),
            "testing".to_string(),
          ),
          (
            SmallStackString::from_static("postInstall"),
            "testing2".to_string(),
          )
        ]),
        // we don't bother ever setting this because we don't store it in deno_package_json
        deprecated: None,
      }
    );

    match convert("{}").unwrap_err() {
      PkgJsonToVersionInfoError::VersionMissing => {
        // ok
      }
      _ => unreachable!(),
    }
    match convert(r#"{ "version": "1.0.~" }"#).unwrap_err() {
      PkgJsonToVersionInfoError::VersionInvalid { source: err } => {
        assert_eq!(err.to_string(), "Invalid npm version");
      }
      _ => unreachable!(),
    }
  }
}
