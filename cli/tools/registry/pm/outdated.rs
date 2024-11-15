use std::sync::Arc;

use deno_core::error::AnyError;

use crate::args::CacheSetting;
use crate::args::Flags;
use crate::args::OutdatedFlags;
use crate::factory::CliFactory;
use crate::file_fetcher::FileFetcher;
use crate::jsr::JsrFetchResolver;
use crate::npm::NpmFetchResolver;

use super::deps::DepManagerArgs;

pub async fn outdated(
  flags: Arc<Flags>,
  outdated_flags: OutdatedFlags,
) -> Result<(), AnyError> {
  let factory = CliFactory::from_flags(flags.clone());
  let cli_options = factory.cli_options()?;
  let workspace = cli_options.workspace();
  let resolver = factory.workspace_resolver().await?;
  let http_client = factory.http_client_provider();
  let deps_http_cache = factory.global_http_cache()?;
  let mut file_fetcher = FileFetcher::new(
    deps_http_cache.clone(),
    CacheSetting::ReloadAll,
    true,
    http_client.clone(),
    Default::default(),
    None,
  );
  file_fetcher.set_download_log_level(log::Level::Trace);
  let file_fetcher = Arc::new(file_fetcher);
  let npm_resolver = Arc::new(NpmFetchResolver::new(
    file_fetcher.clone(),
    cli_options.npmrc().clone(),
  ));
  let jsr_resolver = Arc::new(JsrFetchResolver::new(file_fetcher.clone()));

  let mut deps = super::deps::DepManager::from_workspace(
    workspace,
    DepManagerArgs {
      module_load_preparer: factory.module_load_preparer().await?.clone(),
      jsr_fetch_resolver: jsr_resolver,
      npm_fetch_resolver: npm_resolver,
      npm_resolver: factory.npm_resolver().await?.clone(),
      permissions_container: factory.root_permissions_container()?.clone(),
      main_module_graph_container: factory
        .main_module_graph_container()
        .await?
        .clone(),
      lockfile: cli_options.maybe_lockfile().cloned(),
    },
  )?;

  deps.resolve_versions().await?;
  deps
    .fetch_latest_versions(outdated_flags.compatible)
    .await?;

  for ((dep, resolved_version), latest_version) in deps
    .deps()
    .iter()
    .zip(deps.resolved_versions().iter())
    .zip(deps.latest_versions().iter())
  {
    if let Some(resolved_version) = resolved_version {
      if let Some(latest_version) = latest_version {
        if latest_version > resolved_version {
          eprintln!(
            "outdated dependency {} : {} -> {}",
            dep.req, resolved_version, latest_version
          );
        }
      }
    }
  }

  Ok(())
}
