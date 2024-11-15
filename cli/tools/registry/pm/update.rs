use std::sync::Arc;

use deno_core::error::AnyError;
use deno_semver::Version;
use deno_semver::VersionReq;

use crate::args::CacheSetting;
use crate::args::Flags;
use crate::args::UpdateFlags;
use crate::factory::CliFactory;
use crate::file_fetcher::FileFetcher;
use crate::jsr::JsrFetchResolver;
use crate::npm::NpmFetchResolver;

use super::deps::DepManagerArgs;

// fn update_lower_bound(req: VersionReq, version: Version) -> Option<VersionReq> {
//   match req.inner() {
//     deno_semver::RangeSetOrTag::RangeSet(version_range_set) => {
//       version_range_set.
//     },
//     deno_semver::RangeSetOrTag::Tag(_) => todo!(),
//   }
// }

pub async fn update(
  flags: Arc<Flags>,
  update_flags: UpdateFlags,
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
      jsr_fetch_resolver: jsr_resolver.clone(),
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

  deps.fetch_latest_versions(true).await?;

  for (dep, latest) in deps
    .deps_with_latest_versions()
    .into_iter()
    .collect::<Vec<_>>()
  {
    let Some(latest) = latest else { continue };
    let new_req =
      VersionReq::parse_from_specifier(format!("^{}", latest.version).as_str())
        .unwrap();
    deps.update_dep(dep, new_req);
  }

  deps.commit_changes().await?;

  super::npm_install_after_modification(flags, Some(jsr_resolver)).await?;

    Ok(())
}
