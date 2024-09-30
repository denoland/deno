// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use deno_core::anyhow::bail;
use deno_core::error::AnyError;
use deno_runtime::colors;
use deno_semver::VersionRange;
use deno_semver::VersionRangeSet;
use deno_semver::VersionReq;
use std::sync::Arc;

use crate::args::CacheSetting;
use crate::args::Flags;
use crate::args::OutdatedFlags;
use crate::factory::CliFactory;
use crate::file_fetcher::FileFetcher;
use crate::jsr::JsrFetchResolver;
use crate::npm::NpmFetchResolver;

use super::cache_deps::find_top_level_deps;
use super::find_package_and_select_version_for_req;
use super::AddPackageReq;
use super::AddPackageReqValue;
use super::PackageAndVersion;

#[derive(Debug)]
struct OutdatedPackage {
  registry: String,
  name: String,
  current: String,
  wanted: String,
  latest: String,
}

pub async fn outdated(
  flags: Arc<Flags>,
  outdated_flags: OutdatedFlags,
) -> Result<(), AnyError> {
  let factory = CliFactory::from_flags(flags);
  let options = factory.cli_options()?;

  // TODO(bartlomieju): check if lockfile is out of date?
  if options.maybe_lockfile().is_none() {
    bail!(
      "No lockfile provided. Install dependencies first using `deno install`."
    );
  }
  let npm_resolver = factory.npm_resolver().await?;
  let http_client = factory.http_client_provider();
  let deps_http_cache = factory.global_http_cache()?;
  let mut deps_file_fetcher = FileFetcher::new(
    deps_http_cache.clone(),
    CacheSetting::ReloadAll,
    true,
    http_client.clone(),
    Default::default(),
    None,
  );
  deps_file_fetcher.set_download_log_level(log::Level::Trace);
  let deps_file_fetcher = Arc::new(deps_file_fetcher);
  let jsr_resolver = Arc::new(JsrFetchResolver::new(deps_file_fetcher.clone()));
  let npm_resolver2 = Arc::new(NpmFetchResolver::new(deps_file_fetcher));

  let top_level_deps =
    find_top_level_deps(&factory, Some(jsr_resolver.clone())).await?;
  eprintln!("top_level_deps {:#?}", top_level_deps);

  let mut outdated_packages = vec![];

  for top_level_dep in top_level_deps {
    let url_str = top_level_dep.as_str();

    let add_package_req = AddPackageReq::parse(top_level_dep.as_str())
      .unwrap()
      .unwrap();
    let mut add_package_req2 = AddPackageReq::parse(top_level_dep.as_str())
      .unwrap()
      .unwrap();
    let add_package_req3 = AddPackageReq::parse(top_level_dep.as_str())
      .unwrap()
      .unwrap();
    match &mut add_package_req2.value {
      AddPackageReqValue::Jsr(ref mut req) => {
        req.version_req = VersionReq::from_raw_text_and_inner(
          "*".to_string(),
          deno_semver::RangeSetOrTag::RangeSet(VersionRangeSet(vec![
            VersionRange::all(),
          ])),
        );
      }
      AddPackageReqValue::Npm(ref mut req) => {
        req.version_req = VersionReq::from_raw_text_and_inner(
          "*".to_string(),
          deno_semver::RangeSetOrTag::RangeSet(VersionRangeSet(vec![
            VersionRange::all(),
          ])),
        );
      }
    };

    let PackageAndVersion::Selected(wanted_package_and_version) =
      find_package_and_select_version_for_req(
        jsr_resolver.clone(),
        npm_resolver2.clone(),
        add_package_req,
      )
      .await?
    else {
      continue;
    };

    let PackageAndVersion::Selected(latest_package_and_version) =
      find_package_and_select_version_for_req(
        jsr_resolver.clone(),
        npm_resolver2.clone(),
        add_package_req2,
      )
      .await?
    else {
      continue;
    };

    eprintln!(
      "wanted_package_and_version {:#?}",
      wanted_package_and_version
    );
    eprintln!(
      "latest_package_and_version {:#?}",
      latest_package_and_version
    );
    // if wanted_package_and_version == latest_package_and_version {
    //   continue;
    // }

    outdated_packages.push(OutdatedPackage {
      registry: top_level_dep.scheme().to_string(),
      name: add_package_req3.alias.to_string(),
      current: match add_package_req3.value {
        AddPackageReqValue::Jsr(req) => req.version_req.to_string(),
        AddPackageReqValue::Npm(req) => req.version_req.to_string(),
      },
      wanted: wanted_package_and_version.selected_version,
      latest: latest_package_and_version.selected_version,
    })
  }

  if let Some(managed_npm_resolver) = npm_resolver.as_managed() {
    let npm_deps_provider = managed_npm_resolver.npm_deps_provider();
    let pkgs = npm_deps_provider.remote_pkgs();

    // eprintln!("pkgs {:#?}", pkgs);

    for pkg in pkgs {
      let Ok(current_version) =
        managed_npm_resolver.resolve_pkg_id_from_pkg_req(&pkg.req)
      else {
        continue;
      };

      let PackageAndVersion::Selected(wanted_package_and_version) =
        find_package_and_select_version_for_req(
          jsr_resolver.clone(),
          npm_resolver2.clone(),
          AddPackageReq::parse(&format!("npm:{}", pkg.req))
            .unwrap()
            .unwrap(),
        )
        .await?
      else {
        continue;
      };

      let PackageAndVersion::Selected(latest_package_and_version) =
        find_package_and_select_version_for_req(
          jsr_resolver.clone(),
          npm_resolver2.clone(),
          AddPackageReq::parse(&format!("npm:{}", pkg.req.name))
            .unwrap()
            .unwrap(),
        )
        .await?
      else {
        continue;
      };

      // TODO(bartlomieju): this condition seems fishy, is it actually needed?
      if wanted_package_and_version == latest_package_and_version {
        continue;
      }

      outdated_packages.push(OutdatedPackage {
        registry: "npm".to_string(),
        name: pkg.req.name.to_string(),
        current: current_version.nv.version.to_string(),
        wanted: wanted_package_and_version.selected_version,
        latest: latest_package_and_version.selected_version,
      })
    }
  }

  if !outdated_flags.filters.is_empty() {
    let filters = OutdatedFilters::from_options(outdated_flags.filters)?;
    outdated_packages = outdated_packages
      .into_iter()
      .filter(|pkg| {
        for exclude in &filters.exclude {
          if pkg.name.starts_with(exclude) {
            return false;
          }
        }

        if !filters.include.is_empty() {
          for include in &filters.include {
            if pkg.name.starts_with(include) {
              return true;
            }
          }

          return false;
        }

        true
      })
      .collect();
  }

  if outdated_packages.is_empty() {
    println!("No outdated packages found");
  } else {
    display_table(&outdated_packages);
  }
  Ok(())
}

fn display_table(packages: &[OutdatedPackage]) {
  const HEADERS: [&str; 4] = ["Package", "Current", "Update", "Latest"];

  let mut longest_cells: Vec<_> = HEADERS.iter().map(|h| h.len()).collect();

  for package in packages {
    longest_cells[0] = std::cmp::max(
      longest_cells[0],
      package.registry.len() + package.name.len() + 1,
    );
    longest_cells[1] = std::cmp::max(longest_cells[1], package.current.len());
    longest_cells[2] = std::cmp::max(longest_cells[2], package.wanted.len());
    longest_cells[3] = std::cmp::max(longest_cells[3], package.latest.len());
  }

  let width = longest_cells
    .clone()
    .into_iter()
    .reduce(|acc, e| acc + e + 5)
    .unwrap_or(0);
  println!("┌{}┐", "─".repeat(width));
  println!(
    "│ {}{} │ {}{} │ {}{} │ {}{} │",
    colors::intense_blue(HEADERS[0]),
    " ".repeat(longest_cells[0] + 1 - HEADERS[0].len()),
    " ".repeat(longest_cells[1] + 1 - HEADERS[1].len()),
    colors::intense_blue(HEADERS[1]),
    " ".repeat(longest_cells[2] + 1 - HEADERS[2].len()),
    colors::intense_blue(HEADERS[2]),
    " ".repeat(longest_cells[3] + 1 - HEADERS[3].len()),
    colors::intense_blue(HEADERS[3]),
  );
  println!(
    "│{}┼{}┼{}┼{}│",
    "─".repeat(longest_cells[0] + 3),
    "─".repeat(longest_cells[1] + 3),
    "─".repeat(longest_cells[2] + 3),
    "─".repeat(longest_cells[3] + 3),
  );
  for (idx, pkg) in packages.iter().enumerate() {
    println!(
      "│ {}{}{}{} │ {}{} │ {}{} │ {}{} │",
      crate::colors::gray(&pkg.registry),
      crate::colors::gray(":"),
      pkg.name,
      " "
        .repeat(longest_cells[0] + 1 - pkg.name.len() - pkg.registry.len() - 1),
      " ".repeat(longest_cells[1] + 1 - pkg.current.len()),
      pkg.current,
      " ".repeat(longest_cells[2] + 1 - pkg.wanted.len()),
      pkg.wanted,
      " ".repeat(longest_cells[3] + 1 - pkg.latest.len()),
      pkg.latest,
    );
    if idx < packages.len() - 1 {
      println!(
        "│{}┼{}┼{}┼{}│",
        "─".repeat(longest_cells[0] + 3),
        "─".repeat(longest_cells[1] + 3),
        "─".repeat(longest_cells[2] + 3),
        "─".repeat(longest_cells[3] + 3),
      );
    } else {
      println!("└{}┘", "─".repeat(width));
    }
  }
}

struct OutdatedFilters {
  include: Vec<String>,
  exclude: Vec<String>,
}

impl OutdatedFilters {
  fn from_options(filters: Vec<String>) -> Result<Self, AnyError> {
    let mut f = Self {
      include: vec![],
      exclude: vec![],
    };

    for filter in filters {
      if let Some(filter) = filter.strip_prefix('!') {
        f.exclude.push(filter.to_string());
      } else {
        f.include.push(filter);
      }
    }

    Ok(f)
  }
}
