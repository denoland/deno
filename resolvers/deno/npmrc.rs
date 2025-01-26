// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;
use std::path::Path;
use std::path::PathBuf;

use boxed_error::Boxed;
use deno_config::workspace::Workspace;
use deno_npm::npm_rc::NpmRc;
use deno_npm::npm_rc::ResolvedNpmRc;
use sys_traits::EnvHomeDir;
use sys_traits::EnvVar;
use sys_traits::FsRead;
use thiserror::Error;
use url::Url;

#[allow(clippy::disallowed_types)]
pub type ResolvedNpmRcRc = crate::sync::MaybeArc<ResolvedNpmRc>;

#[derive(Debug, Boxed)]
pub struct NpmRcDiscoverError(pub Box<NpmRcDiscoverErrorKind>);

#[derive(Debug, Error)]
pub enum NpmRcDiscoverErrorKind {
  #[error(transparent)]
  Load(#[from] NpmRcLoadError),
  #[error(transparent)]
  Parse(#[from] NpmRcParseError),
  #[error(transparent)]
  Resolve(#[from] NpmRcOptionsResolveError),
  #[error(transparent)]
  UrlToFilePath(#[from] deno_path_util::UrlToFilePathError),
}

#[derive(Debug, Error)]
#[error("Error loading .npmrc at {}.", path.display())]
pub struct NpmRcLoadError {
  path: PathBuf,
  #[source]
  source: std::io::Error,
}

#[derive(Debug, Error)]
#[error("Failed to parse .npmrc at {}.", path.display())]
pub struct NpmRcParseError {
  path: PathBuf,
  #[source]
  source: std::io::Error,
}

#[derive(Debug, Error)]
#[error("Failed to resolve .npmrc options at {}.", path.display())]
pub struct NpmRcOptionsResolveError {
  path: PathBuf,
  #[source]
  source: deno_npm::npm_rc::ResolveError,
}

/// Discover `.npmrc` file - currently we only support it next to `package.json`,
/// next to `deno.json`, or in the user's home directory.
///
/// In the future we will need to support it in the global directory
/// as per https://docs.npmjs.com/cli/v10/configuring-npm/npmrc#files.
pub fn discover_npmrc_from_workspace<TSys: EnvVar + EnvHomeDir + FsRead>(
  sys: &TSys,
  workspace: &Workspace,
) -> Result<(ResolvedNpmRc, Option<PathBuf>), NpmRcDiscoverError> {
  let root_folder = workspace.root_folder_configs();
  discover_npmrc(
    sys,
    root_folder.pkg_json.as_ref().map(|p| p.path.clone()),
    match &root_folder.deno_json {
      Some(cf) if cf.specifier.scheme() == "file" => {
        Some(deno_path_util::url_to_file_path(&cf.specifier)?)
      }
      _ => None,
    },
  )
}

fn discover_npmrc<TSys: EnvVar + EnvHomeDir + FsRead>(
  sys: &TSys,
  maybe_package_json_path: Option<PathBuf>,
  maybe_deno_json_path: Option<PathBuf>,
) -> Result<(ResolvedNpmRc, Option<PathBuf>), NpmRcDiscoverError> {
  const NPMRC_NAME: &str = ".npmrc";

  fn try_to_read_npmrc(
    sys: &impl FsRead,
    dir: &Path,
  ) -> Result<Option<(Cow<'static, str>, PathBuf)>, NpmRcLoadError> {
    let path = dir.join(NPMRC_NAME);
    let maybe_source = match sys.fs_read_to_string(&path) {
      Ok(source) => Some(source),
      Err(err) if err.kind() == std::io::ErrorKind::NotFound => None,
      Err(err) => return Err(NpmRcLoadError { path, source: err }),
    };

    Ok(maybe_source.map(|source| (source, path)))
  }

  fn try_to_parse_npmrc(
    sys: &impl EnvVar,
    source: &str,
    path: &Path,
  ) -> Result<ResolvedNpmRc, NpmRcDiscoverError> {
    let npmrc = NpmRc::parse(source, &|name| sys.env_var(name).ok()).map_err(
      |source| {
        NpmRcParseError {
          path: path.to_path_buf(),
          // todo(dsherret): use source directly here once it's no longer an internal type
          source: std::io::Error::new(std::io::ErrorKind::InvalidData, source),
        }
      },
    )?;
    let resolved =
      npmrc
        .as_resolved(&npm_registry_url(sys))
        .map_err(|source| NpmRcOptionsResolveError {
          path: path.to_path_buf(),
          source,
        })?;
    log::debug!(".npmrc found at: '{}'", path.display());
    Ok(resolved)
  }

  fn merge_resolved_npm_rc(
    rc1: ResolvedNpmRc,
    rc2: ResolvedNpmRc,
  ) -> ResolvedNpmRc {
    let mut merged_registry_configs = rc1.registry_configs.clone();
    for (key, value) in rc2.registry_configs {
      merged_registry_configs.entry(key).or_insert(value);
    }

    let mut merged_default_config = rc1.default_config.clone();
    if let Some(host) = merged_default_config.registry_url.host_str() {
      if let Some(config) = merged_registry_configs.get(host) {
        merged_default_config.config = config.clone();
      } else if let Some(config) =
        merged_registry_configs.get(format!("{}/", host).as_str())
      {
        merged_default_config.config = config.clone();
      }
    }

    let mut merged_scopes = rc1.scopes.clone();
    for (key, value) in rc2.scopes {
      merged_scopes.entry(key).or_insert(value);
    }
    for data in merged_scopes.values_mut() {
      if let (Some(host), Some(port)) =
        (data.registry_url.host_str(), data.registry_url.port())
      {
        let path = data.registry_url.path();
        let url = format!("{}:{}{}", host, port, path);
        if let Some(config) = merged_registry_configs.get(&url) {
          data.config = config.clone();
        }
      }
    }

    ResolvedNpmRc {
      default_config: merged_default_config,
      scopes: merged_scopes,
      registry_configs: merged_registry_configs,
    }
  }

  let mut home_npmrc = None;
  let mut project_npmrc = None;
  let mut npmrc_path = None;

  // 1. Try `.npmrc` in the user's home directory
  if let Some(home_dir) = sys.env_home_dir() {
    match try_to_read_npmrc(sys, &home_dir) {
      Ok(Some((source, path))) => {
        let npmrc = try_to_parse_npmrc(sys, &source, &path)?;
        home_npmrc = Some(npmrc);
        npmrc_path = Some(path);
      }
      Ok(None) => {}
      Err(err) if err.source.kind() == std::io::ErrorKind::PermissionDenied => {
        log::debug!(
            "Skipping .npmrc in home directory due to permission denied error. {:#}",
            err
          );
      }
      Err(err) => {
        return Err(err.into());
      }
    }
  }

  // 2. Try `.npmrc` next to `package.json`
  if let Some(package_json_path) = maybe_package_json_path {
    if let Some(package_json_dir) = package_json_path.parent() {
      if let Some((source, path)) = try_to_read_npmrc(sys, package_json_dir)? {
        let npmrc = try_to_parse_npmrc(sys, &source, &path)?;
        project_npmrc = Some(npmrc);
        npmrc_path = Some(path);
      }
    }
  }

  // 3. Try `.npmrc` next to `deno.json(c)` when not found `package.json`
  if project_npmrc.is_none() {
    if let Some(deno_json_path) = maybe_deno_json_path {
      if let Some(deno_json_dir) = deno_json_path.parent() {
        if let Some((source, path)) = try_to_read_npmrc(sys, deno_json_dir)? {
          let npmrc = try_to_parse_npmrc(sys, &source, &path)?;
          project_npmrc = Some(npmrc);
          npmrc_path = Some(path);
        }
      }
    }
  }

  match (home_npmrc, project_npmrc) {
    (None, None) => {
      log::debug!("No .npmrc file found");
      Ok((create_default_npmrc(sys), None))
    }
    (None, Some(project_rc)) => {
      log::debug!("Only project .npmrc file found");
      Ok((project_rc, npmrc_path))
    }
    (Some(home_rc), None) => {
      log::debug!("Only home .npmrc file found");
      Ok((home_rc, npmrc_path))
    }
    (Some(home_rc), Some(project_rc)) => {
      log::debug!("Both home and project .npmrc files found");
      let merged_npmrc = merge_resolved_npm_rc(project_rc, home_rc);
      Ok((merged_npmrc, npmrc_path))
    }
  }
}

pub fn create_default_npmrc(sys: &impl EnvVar) -> ResolvedNpmRc {
  ResolvedNpmRc {
    default_config: deno_npm::npm_rc::RegistryConfigWithUrl {
      registry_url: npm_registry_url(sys).clone(),
      config: Default::default(),
    },
    scopes: Default::default(),
    registry_configs: Default::default(),
  }
}

pub fn npm_registry_url(sys: &impl EnvVar) -> Url {
  let env_var_name = "NPM_CONFIG_REGISTRY";
  if let Ok(registry_url) = sys.env_var(env_var_name) {
    // ensure there is a trailing slash for the directory
    let registry_url = format!("{}/", registry_url.trim_end_matches('/'));
    match Url::parse(&registry_url) {
      Ok(url) => {
        return url;
      }
      Err(err) => {
        log::debug!("Invalid {} environment variable: {:#}", env_var_name, err,);
      }
    }
  }

  Url::parse("https://registry.npmjs.org").unwrap()
}
