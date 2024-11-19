use std::collections::HashSet;
use std::sync::Arc;

use deno_core::error::AnyError;
use deno_semver::package::PackageReq;
use deno_semver::VersionReq;
use deno_terminal::colors;

use crate::args::CacheSetting;
use crate::args::Flags;
use crate::args::UpdateFlags;
use crate::factory::CliFactory;
use crate::file_fetcher::FileFetcher;
use crate::jsr::JsrFetchResolver;
use crate::npm::NpmFetchResolver;
use crate::tools::registry::pm::deps::DepKind;

use super::deps::DepManager;
use super::deps::DepManagerArgs;

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
struct OutdatedPackage {
  kind: DepKind,
  latest: String,
  semver_compatible: String,
  current: String,
  name: String,
}

fn print_outdated_table(packages: &[OutdatedPackage]) {
  const HEADINGS: &[&str] = &["Package", "Current", "Update", "Latest"];

  let mut longest_package = 0;
  let mut longest_current = 0;
  let mut longest_update = 0;
  let mut longest_latest = 0;

  for package in packages {
    let name_len = package.kind.scheme().len() + 1 + package.name.len();
    longest_package = longest_package.max(name_len);
    longest_current = longest_current.max(package.current.len());
    longest_update = longest_update.max(package.semver_compatible.len());
    longest_latest = longest_latest.max(package.latest.len());
  }

  let package_column_width = longest_package.max(HEADINGS[0].len()) + 2;
  let current_column_width = longest_current.max(HEADINGS[1].len()) + 2;
  let update_column_width = longest_update.max(HEADINGS[2].len()) + 2;
  let latest_column_width = longest_latest.max(HEADINGS[3].len()) + 2;

  let package_fill = "─".repeat(package_column_width);
  let current_fill = "─".repeat(current_column_width);
  let update_fill = "─".repeat(update_column_width);
  let latest_fill = "─".repeat(latest_column_width);

  println!("┌{package_fill}┬{current_fill}┬{update_fill}┬{latest_fill}┐");
  println!(
    "│ {}{} │ {}{} │ {}{} │ {}{} │",
    colors::intense_blue(HEADINGS[0]),
    " ".repeat(package_column_width - 2 - HEADINGS[0].len()),
    colors::intense_blue(HEADINGS[1]),
    " ".repeat(current_column_width - 2 - HEADINGS[1].len()),
    colors::intense_blue(HEADINGS[2]),
    " ".repeat(update_column_width - 2 - HEADINGS[2].len()),
    colors::intense_blue(HEADINGS[3]),
    " ".repeat(latest_column_width - 2 - HEADINGS[3].len())
  );
  for package in packages {
    println!("├{package_fill}┼{current_fill}┼{update_fill}┼{latest_fill}┤",);

    print!(
      "│ {:<package_column_width$} ",
      format!("{}:{}", package.kind.scheme(), package.name),
      package_column_width = package_column_width - 2
    );
    print!(
      "│ {:<current_column_width$} ",
      package.current,
      current_column_width = current_column_width - 2
    );
    print!(
      "│ {:<update_column_width$} ",
      package.semver_compatible,
      update_column_width = update_column_width - 2
    );
    println!(
      "│ {:<latest_column_width$} │",
      package.latest,
      latest_column_width = latest_column_width - 2
    );
  }

  println!("└{package_fill}┴{current_fill}┴{update_fill}┴{latest_fill}┘",);
}

async fn outdated(
  deps: &mut DepManager,
  compatible: bool,
) -> Result<(), AnyError> {
  let mut outdated = Vec::new();
  let mut seen = std::collections::BTreeSet::new();
  for (dep_id, resolved, latest_versions) in
    deps.deps_with_resolved_latest_versions()
  {
    let dep = deps.get_dep(dep_id);

    let Some(resolved) = resolved else { continue };

    let latest = {
      let preferred = if compatible {
        &latest_versions.semver_compatible
      } else {
        &latest_versions.latest
      };
      if let Some(v) = preferred {
        v
      } else {
        continue;
      }
    };

    if latest > &resolved
      && seen.insert((dep.kind, dep.req.name.clone(), resolved.version.clone()))
    {
      outdated.push(OutdatedPackage {
        kind: dep.kind,
        name: dep.req.name.clone(),
        current: resolved.version.to_string(),
        latest: latest_versions
          .latest
          .map(|l| l.version.to_string())
          .unwrap_or_default(),
        semver_compatible: latest_versions
          .semver_compatible
          .map(|l| l.version.to_string())
          .unwrap_or_default(),
      })
    }
  }

  if !outdated.is_empty() {
    print_outdated_table(&outdated);
  }

  Ok(())
}

pub async fn update(
  flags: Arc<Flags>,
  update_flags: UpdateFlags,
) -> Result<(), AnyError> {
  let factory = CliFactory::from_flags(flags.clone());
  let cli_options = factory.cli_options()?;
  let workspace = cli_options.workspace();
  let http_client = factory.http_client_provider();
  let deps_http_cache = factory.global_http_cache()?;
  let mut file_fetcher = FileFetcher::new(
    deps_http_cache.clone(),
    CacheSetting::RespectHeaders,
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

  let args = DepManagerArgs {
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
  };
  let mut deps = if update_flags.recursive {
    super::deps::DepManager::from_workspace(
      workspace,
      |_: &PackageReq, _: DepKind| true,
      args,
    )?
  } else {
    super::deps::DepManager::from_workspace_dir(
      &cli_options.start_dir,
      |_: &PackageReq, _: DepKind| true,
      args,
    )?
  };

  // deps.resolve_versions().await?;
  // deps.fetch_latest_versions().await?;
  deps.resolve_versions().await?;

  match update_flags.kind {
    crate::args::UpdateKind::Update { latest } => {
      do_update(&mut deps, latest, jsr_resolver, flags).await?;
    }
    crate::args::UpdateKind::PrintOutdated { compatible } => {
      outdated(&mut deps, compatible).await?;
    }
  }

  Ok(())
}

async fn do_update(
  deps: &mut DepManager,
  update_to_latest: bool,
  jsr_resolver: Arc<JsrFetchResolver>,
  flags: Arc<Flags>,
) -> Result<(), AnyError> {
  let mut updated = HashSet::new();
  for (dep_id, resolved, latest_versions) in deps
    .deps_with_resolved_latest_versions()
    .into_iter()
    .collect::<Vec<_>>()
  {
    let latest = {
      let preferred = if update_to_latest {
        latest_versions.latest
      } else {
        latest_versions.semver_compatible
      };
      if let Some(v) = preferred {
        v
      } else {
        continue;
      }
    };
    let Some(resolved) = resolved else { continue };
    if latest.version <= resolved.version {
      continue;
    }
    let new_req =
      VersionReq::parse_from_specifier(format!("^{}", latest.version).as_str())
        .unwrap();
    let dep = deps.get_dep(dep_id);

    updated.insert((dep.prefixed_req(), latest.version));
    deps.update_dep(dep_id, new_req);
  }

  deps.commit_changes().await?;

  if !updated.is_empty() {
    super::npm_install_after_modification(flags, Some(jsr_resolver)).await?;
    log::info!("Updated {} dependencies:", updated.len());
    for (req, new_version) in updated {
      log::info!("{} -> {}", req, new_version);
    }
  }

  Ok(())
}
