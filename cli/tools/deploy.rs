// Copyright 2018-2025 the Deno authors. MIT license.

use std::sync::Arc;

use deno_config::deno_json::NodeModulesDirMode;
use deno_core::error::AnyError;
use deno_core::url::Url;
use deno_path_util::ResolveUrlOrPathError;
use deno_runtime::WorkerExecutionMode;
use deno_runtime::deno_permissions::PermissionsContainer;

use crate::args::DeployFlags;
use crate::args::Flags;
use crate::args::jsr_api_url;
use crate::factory::CliFactory;
use crate::ops;
use crate::registry;

pub async fn deploy(
  mut flags: Flags,
  deploy_flags: DeployFlags,
) -> Result<i32, AnyError> {
  flags.node_modules_dir = Some(NodeModulesDirMode::None);
  flags.no_lock = true;
  if deploy_flags.sandbox {
    // SAFETY: only this subcommand is running, nothing else, so it's safe to set an env var.
    unsafe {
      std::env::set_var("DENO_DEPLOY_CLI_SANDBOX", "1");
    }
  }

  let mut factory = CliFactory::from_flags(Arc::new(flags));

  let maybe_specifier_override =
    if let Ok(specifier) = std::env::var("DENO_DEPLOY_CLI_SPECIFIER") {
      let specifier =
        Url::parse(&specifier).map_err(ResolveUrlOrPathError::UrlParse)?;
      if let Ok(path) = specifier.to_file_path() {
        factory.set_initial_cwd(path);
      }

      Some(specifier)
    } else {
      None
    };

  let client = factory.http_client_provider().get_or_create()?;
  let registry_api_url = jsr_api_url();

  let response =
    registry::get_package(&client, registry_api_url, "deno", "deploy").await?;
  let res = registry::parse_response::<registry::Package>(response).await?;

  let worker_factory =
    Arc::new(factory.create_cli_main_worker_factory().await?);

  let specifier = if let Some(specifier) = maybe_specifier_override {
    specifier
  } else {
    Url::parse(&format!(
      "jsr:@deno/deploy@{}",
      res
        .latest_version
        .expect("expected @deno/deploy to be published")
    ))
    .map_err(ResolveUrlOrPathError::UrlParse)?
  };

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

pub fn get_token_entry() -> Result<keyring::Entry, keyring::Error> {
  keyring::Entry::new("Deno Deploy Token", "Deno Deploy")
}
