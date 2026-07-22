// Copyright 2018-2026 the Deno authors. MIT license.

use std::sync::Arc;

use deno_config::deno_json::NewestDependencyDate;
use deno_config::deno_json::NodeModulesDirMode;
use deno_core::error::AnyError;
use deno_core::serde_json;
use deno_core::url::Url;
use deno_graph::packages::JsrPackageInfo;
use deno_path_util::ResolveUrlOrPathError;
use deno_runtime::WorkerExecutionMode;
use deno_runtime::deno_permissions::PermissionsContainer;

use crate::args::ConfigFlag;
use crate::args::DeployFlags;
use crate::args::Flags;
use crate::factory::CliFactory;
use crate::ops;

pub async fn deploy(
  mut flags: Flags,
  deploy_flags: DeployFlags,
) -> Result<i32, AnyError> {
  flags.node_modules_dir = Some(NodeModulesDirMode::None);
  flags.no_lock = true;
  flags.minimum_dependency_age = Some(NewestDependencyDate::Disabled);
  if deploy_flags.sandbox {
    // SAFETY: only this subcommand is running, nothing else, so it's safe to set an env var.
    unsafe {
      std::env::set_var("DENO_DEPLOY_CLI_SANDBOX", "1");
    }
  }

  let dev_specifier = std::env::var("DENO_DEPLOY_CLI_SPECIFIER").ok();

  // The jsr-fetched CLI resolves its imports from JSR package metadata and
  // reads the user's config itself, in-process. Disable config discovery so
  // the workspace at the user's cwd isn't applied to the deploy CLI's own
  // module graph (which otherwise breaks when a workspace member's name
  // collides with one of the CLI's dependencies). The dev path
  // (DENO_DEPLOY_CLI_SPECIFIER) still wants discovery relative to the CLI
  // source.
  if dev_specifier.is_none() {
    flags.config_flag = ConfigFlag::Disabled;
  }

  let mut factory = CliFactory::from_flags(Arc::new(flags));

  let specifier = if let Some(specifier) = dev_specifier {
    let specifier =
      Url::parse(&specifier).map_err(ResolveUrlOrPathError::UrlParse)?;
    if let Ok(path) = specifier.to_file_path() {
      factory.set_initial_cwd(path);
    }

    specifier
  } else {
    let registry_url = crate::args::jsr_url();
    let file = factory
      .file_fetcher()?
      .fetch_with_options(
        &registry_url.join("@deno/deploy/meta.json").unwrap(),
        deno_resolver::file_fetcher::FetchPermissionsOptionRef::AllowAll,
        deno_resolver::file_fetcher::FetchOptions {
          // jsr.io serves meta.json with `cache-control: max-age=60`, so
          // respect those headers rather than pinning the first cached copy
          // forever (see denoland/deno#35736).
          maybe_cache_setting: Some(
            &deno_cache_dir::file_fetcher::CacheSetting::RespectHeaders,
          ),
          ..Default::default()
        },
      )
      .await?;
    let info = serde_json::from_slice::<JsrPackageInfo>(&file.source)?;
    let latest_version = resolve_deploy_cli_version(&info)
      .expect("expected @deno/deploy to be published");
    Url::parse(&format!("jsr:@deno/deploy@{latest_version}"))
      .map_err(ResolveUrlOrPathError::UrlParse)?
  };

  let worker_factory =
    Arc::new(factory.create_cli_main_worker_factory().await?);

  let mut worker = worker_factory
    .create_custom_worker(
      WorkerExecutionMode::Deploy,
      specifier,
      vec![],
      vec![],
      PermissionsContainer::allow_all(
        factory.permission_desc_parser()?.clone(),
      ),
      vec![ops::deploy::deno_deploy::init()],
      Default::default(),
      None,
    )
    .await?;

  Ok(worker.run().await?)
}

fn resolve_deploy_cli_version(
  info: &JsrPackageInfo,
) -> Option<&deno_semver::Version> {
  info.latest.as_ref().or_else(|| info.versions.keys().max())
}

pub fn get_token_entry() -> Result<keyring::Entry, keyring::Error> {
  keyring::Entry::new("Deno Deploy Token", "Deno Deploy")
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn resolve_deploy_cli_version_uses_jsr_latest() {
    let info = serde_json::from_value::<JsrPackageInfo>(serde_json::json!({
      "latest": "1.0.0",
      "versions": {
        "1.0.0": {},
        "2.0.0": {}
      }
    }))
    .unwrap();

    assert_eq!(
      resolve_deploy_cli_version(&info).unwrap().to_string(),
      "1.0.0"
    );
  }
}
