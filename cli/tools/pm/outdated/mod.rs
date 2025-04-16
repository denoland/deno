// Copyright 2018-2025 the Deno authors. MIT license.

mod interactive;

use std::collections::HashSet;
use std::sync::Arc;

use deno_cache_dir::file_fetcher::CacheSetting;
use deno_core::anyhow::bail;
use deno_core::error::AnyError;
use deno_semver::package::PackageNv;
use deno_semver::package::PackageReq;
use deno_semver::StackString;
use deno_semver::VersionReq;
use deno_terminal::colors;

use super::deps::Dep;
use super::deps::DepId;
use super::deps::DepKind;
use super::deps::DepManager;
use super::deps::DepManagerArgs;
use super::deps::PackageLatestVersion;
use crate::args::Flags;
use crate::args::OutdatedFlags;
use crate::factory::CliFactory;
use crate::file_fetcher::CliFileFetcher;
use crate::jsr::JsrFetchResolver;
use crate::npm::NpmFetchResolver;

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
struct OutdatedPackage {
  kind: DepKind,
  latest: String,
  semver_compatible: String,
  current: String,
  name: StackString,
}

#[allow(clippy::print_stdout)]
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

fn print_suggestion(compatible: bool) {
  log::info!("");
  let (cmd, txt) = if compatible {
    ("", "compatible")
  } else {
    (" --latest", "available")
  };
  log::info!(
    "{}",
    color_print::cformat!(
      "<p(245)>Run</> <u>deno outdated --update{}</> <p(245)>to update to the latest {} versions,</>\n<p(245)>or</> <u>deno outdated --help</> <p(245)>for more information.</>",
      cmd,
      txt,
    )
  );
}

fn print_outdated(
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
    outdated.sort();
    print_outdated_table(&outdated);
    print_suggestion(compatible);
  }

  Ok(())
}

pub async fn outdated(
  flags: Arc<Flags>,
  update_flags: OutdatedFlags,
) -> Result<(), AnyError> {
  let factory = CliFactory::from_flags(flags.clone());
  let cli_options = factory.cli_options()?;
  let workspace = cli_options.workspace();
  let http_client = factory.http_client_provider();
  let deps_http_cache = factory.global_http_cache()?;
  let file_fetcher = CliFileFetcher::new(
    deps_http_cache.clone(),
    http_client.clone(),
    factory.sys(),
    Default::default(),
    None,
    true,
    CacheSetting::RespectHeaders,
    log::Level::Trace,
  );
  let file_fetcher = Arc::new(file_fetcher);
  let npm_fetch_resolver = Arc::new(NpmFetchResolver::new(
    file_fetcher.clone(),
    factory.npmrc()?.clone(),
  ));
  let jsr_fetch_resolver =
    Arc::new(JsrFetchResolver::new(file_fetcher.clone()));

  if !cli_options.start_dir.has_deno_json()
    && !cli_options.start_dir.has_pkg_json()
  {
    bail!(
      "No deno.json or package.json in \"{}\".",
      cli_options.initial_cwd().display(),
    );
  }

  let args = dep_manager_args(
    &factory,
    npm_fetch_resolver.clone(),
    jsr_fetch_resolver.clone(),
  )
  .await?;

  let filter_set = filter::FilterSet::from_filter_strings(
    update_flags.filters.iter().map(|s| s.as_str()),
  )?;

  let filter_fn = |alias: Option<&str>, req: &PackageReq, _: DepKind| {
    if filter_set.is_empty() {
      return true;
    }
    let name = alias.unwrap_or(&req.name);
    filter_set.matches(name)
  };
  let mut deps = if update_flags.recursive {
    super::deps::DepManager::from_workspace(workspace, filter_fn, args)?
  } else {
    super::deps::DepManager::from_workspace_dir(
      &cli_options.start_dir,
      filter_fn,
      args,
    )?
  };

  deps.resolve_versions().await?;

  match update_flags.kind {
    crate::args::OutdatedKind::Update {
      latest,
      interactive,
    } => {
      update(deps, latest, &filter_set, interactive, flags).await?;
    }
    crate::args::OutdatedKind::PrintOutdated { compatible } => {
      print_outdated(&mut deps, compatible)?;
    }
  }

  Ok(())
}

enum ChosenVersionReq {
  Some(VersionReq),
  None { latest_available: bool },
}

fn choose_new_version_req(
  dep: &Dep,
  resolved: Option<&PackageNv>,
  latest_versions: &PackageLatestVersion,
  update_to_latest: bool,
  filter_set: &filter::FilterSet,
) -> ChosenVersionReq {
  let explicit_version_req = filter_set
    .matching_filter(dep.alias.as_deref().unwrap_or(&dep.req.name))
    .version_spec()
    .cloned();

  if let Some(version_req) = explicit_version_req {
    if let Some(resolved) = resolved {
      // todo(nathanwhit): handle tag
      if version_req.tag().is_none() && version_req.matches(&resolved.version) {
        return ChosenVersionReq::None {
          latest_available: false,
        };
      }
    }
    ChosenVersionReq::Some(version_req)
  } else {
    let Some(resolved) = resolved else {
      return ChosenVersionReq::None {
        latest_available: false,
      };
    };
    let Some(preferred) = (if update_to_latest {
      latest_versions.latest.as_ref()
    } else {
      latest_versions.semver_compatible.as_ref()
    }) else {
      return ChosenVersionReq::None {
        latest_available: false,
      };
    };
    if preferred.version <= resolved.version {
      return ChosenVersionReq::None {
        latest_available: !update_to_latest
          && latest_versions
            .latest
            .as_ref()
            .is_some_and(|nv| nv.version > resolved.version),
      };
    }
    let exact = if let Some(range) = dep.req.version_req.range() {
      range.0[0].start == range.0[0].end
    } else {
      false
    };
    ChosenVersionReq::Some(
      VersionReq::parse_from_specifier(
        format!("{}{}", if exact { "" } else { "^" }, preferred.version)
          .as_str(),
      )
      .unwrap(),
    )
  }
}

async fn update(
  mut deps: DepManager,
  update_to_latest: bool,
  filter_set: &filter::FilterSet,
  interactive: bool,
  flags: Arc<Flags>,
) -> Result<(), AnyError> {
  let mut to_update = Vec::new();

  let mut can_update_to_latest = false;

  for (dep_id, resolved, latest_versions) in deps
    .deps_with_resolved_latest_versions()
    .into_iter()
    .collect::<Vec<_>>()
  {
    let dep = deps.get_dep(dep_id);
    let new_version_req = choose_new_version_req(
      dep,
      resolved.as_ref(),
      &latest_versions,
      update_to_latest,
      filter_set,
    );
    let new_version_req = match new_version_req {
      ChosenVersionReq::Some(version_req) => version_req,
      ChosenVersionReq::None { latest_available } => {
        can_update_to_latest = can_update_to_latest || latest_available;
        continue;
      }
    };

    to_update.push((
      dep_id,
      format!("{}:{}", dep.kind.scheme(), dep.req.name),
      deps.resolved_version(dep.id).cloned(),
      new_version_req.clone(),
    ));
  }

  if interactive && !to_update.is_empty() {
    let selected = interactive::select_interactive(
      to_update
        .iter()
        .map(
          |(dep_id, _, current_version, new_req): &(
            DepId,
            String,
            Option<PackageNv>,
            VersionReq,
          )| {
            let dep = deps.get_dep(*dep_id);
            interactive::PackageInfo {
              id: *dep_id,
              current_version: current_version
                .as_ref()
                .map(|nv| nv.version.clone()),
              name: dep.alias_or_name().into(),
              kind: dep.kind,
              new_version: new_req.clone(),
            }
          },
        )
        .collect(),
    )?;
    if let Some(selected) = selected {
      to_update.retain(|(id, _, _, _)| selected.contains(id));
    } else {
      log::info!("Cancelled, not updating");
      return Ok(());
    }
  }

  if !to_update.is_empty() {
    for (dep_id, _, _, new_version_req) in &to_update {
      deps.update_dep(*dep_id, new_version_req.clone());
    }

    deps.commit_changes()?;

    let factory = super::npm_install_after_modification(
      flags.clone(),
      Some(deps.jsr_fetch_resolver.clone()),
    )
    .await?;

    let mut updated_to_versions = HashSet::new();
    let args = dep_manager_args(
      &factory,
      deps.npm_fetch_resolver.clone(),
      deps.jsr_fetch_resolver.clone(),
    )
    .await?;

    let mut deps = deps.reloaded_after_modification(args);
    deps.resolve_current_versions().await?;
    for (dep_id, package_name, maybe_current_version, new_version_req) in
      to_update
    {
      if let Some(nv) = deps.resolved_version(dep_id) {
        updated_to_versions.insert((
          package_name,
          maybe_current_version,
          nv.version.clone(),
        ));
      } else {
        log::warn!(
          "Failed to resolve version for new version requirement: {} -> {}",
          package_name,
          new_version_req
        );
      }
    }

    log::info!(
      "Updated {} dependenc{}:",
      updated_to_versions.len(),
      if updated_to_versions.len() == 1 {
        "y"
      } else {
        "ies"
      }
    );
    let mut updated_to_versions =
      updated_to_versions.into_iter().collect::<Vec<_>>();
    updated_to_versions.sort_by(|(k, _, _), (k2, _, _)| k.cmp(k2));
    let max_name = updated_to_versions
      .iter()
      .map(|(name, _, _)| name.len())
      .max()
      .unwrap_or(0);
    let max_old = updated_to_versions
      .iter()
      .map(|(_, maybe_current, _)| {
        maybe_current
          .as_ref()
          .map(|v| v.version.to_string().len())
          .unwrap_or(0)
      })
      .max()
      .unwrap_or(0);
    let max_new = updated_to_versions
      .iter()
      .map(|(_, _, new_version)| new_version.to_string().len())
      .max()
      .unwrap_or(0);

    for (package_name, maybe_current_version, new_version) in
      updated_to_versions
    {
      let current_version = if let Some(current_version) = maybe_current_version
      {
        current_version.version.to_string()
      } else {
        "".to_string()
      };

      log::info!(
        " - {}{} {}{} -> {}{}",
        format!(
          "{}{}",
          colors::gray(package_name[0..4].to_string()),
          package_name[4..].to_string()
        ),
        " ".repeat(max_name - package_name.len()),
        " ".repeat(max_old - current_version.len()),
        colors::gray(&current_version),
        " ".repeat(max_new - new_version.to_string().len()),
        colors::green(&new_version),
      );
    }
  } else {
    let maybe_matching = if filter_set.is_empty() {
      ""
    } else {
      "matching "
    };
    if !update_to_latest && can_update_to_latest {
      let note = deno_terminal::colors::intense_blue("note");
      log::info!(
        "All {maybe_matching}dependencies are at newest compatible versions.\n{note}: newer, incompatible versions are available.\n      Run with `--latest` to update",
      );
    } else {
      log::info!("All {maybe_matching}dependencies are up to date.");
    }
  }

  Ok(())
}

async fn dep_manager_args(
  factory: &CliFactory,
  npm_fetch_resolver: Arc<NpmFetchResolver>,
  jsr_fetch_resolver: Arc<JsrFetchResolver>,
) -> Result<DepManagerArgs, AnyError> {
  Ok(DepManagerArgs {
    module_load_preparer: factory.module_load_preparer().await?.clone(),
    jsr_fetch_resolver,
    npm_fetch_resolver,
    npm_resolver: factory.npm_resolver().await?.clone(),
    npm_installer: factory.npm_installer().await?.clone(),
    permissions_container: factory.root_permissions_container()?.clone(),
    main_module_graph_container: factory
      .main_module_graph_container()
      .await?
      .clone(),
    lockfile: factory.maybe_lockfile().await?.cloned(),
  })
}

mod filter {
  use deno_core::anyhow::anyhow;
  use deno_core::anyhow::Context;
  use deno_core::error::AnyError;
  use deno_semver::VersionReq;

  enum FilterKind {
    Exclude,
    Include,
  }
  pub struct Filter {
    kind: FilterKind,
    regex: regex::Regex,
    version_spec: Option<VersionReq>,
  }

  fn pattern_to_regex(pattern: &str) -> Result<regex::Regex, AnyError> {
    let escaped = regex::escape(pattern);
    let unescaped_star = escaped.replace(r"\*", ".*");
    Ok(regex::Regex::new(&format!("^{}$", unescaped_star))?)
  }

  impl Filter {
    pub fn version_spec(&self) -> Option<&VersionReq> {
      self.version_spec.as_ref()
    }
    pub fn from_str(input: &str) -> Result<Self, AnyError> {
      let (kind, first_idx) = if input.starts_with('!') {
        (FilterKind::Exclude, 1)
      } else {
        (FilterKind::Include, 0)
      };
      let s = &input[first_idx..];
      let (pattern, version_spec) =
        if let Some(scope_name) = s.strip_prefix('@') {
          if let Some(idx) = scope_name.find('@') {
            let (pattern, version_spec) = s.split_at(idx + 1);
            (
              pattern,
              Some(
                VersionReq::parse_from_specifier(
                  version_spec.trim_start_matches('@'),
                )
                .with_context(|| format!("Invalid filter \"{input}\""))?,
              ),
            )
          } else {
            (s, None)
          }
        } else {
          let mut parts = s.split('@');
          let Some(pattern) = parts.next() else {
            return Err(anyhow!("Invalid filter \"{input}\""));
          };
          (
            pattern,
            parts
              .next()
              .map(VersionReq::parse_from_specifier)
              .transpose()
              .with_context(|| format!("Invalid filter \"{input}\""))?,
          )
        };

      Ok(Filter {
        kind,
        regex: pattern_to_regex(pattern)
          .with_context(|| format!("Invalid filter \"{input}\""))?,
        version_spec,
      })
    }

    pub fn matches(&self, name: &str) -> bool {
      self.regex.is_match(name)
    }
  }

  pub struct FilterSet {
    filters: Vec<Filter>,
    has_exclude: bool,
    has_include: bool,
  }
  impl FilterSet {
    pub fn from_filter_strings<'a>(
      filter_strings: impl IntoIterator<Item = &'a str>,
    ) -> Result<Self, AnyError> {
      let filters = filter_strings
        .into_iter()
        .map(Filter::from_str)
        .collect::<Result<Vec<_>, _>>()?;
      let has_exclude = filters
        .iter()
        .any(|f| matches!(f.kind, FilterKind::Exclude));
      let has_include = filters
        .iter()
        .any(|f| matches!(f.kind, FilterKind::Include));
      Ok(FilterSet {
        filters,
        has_exclude,
        has_include,
      })
    }

    pub fn is_empty(&self) -> bool {
      self.filters.is_empty()
    }

    pub fn matches(&self, name: &str) -> bool {
      self.matching_filter(name).is_included()
    }

    pub fn matching_filter(&self, name: &str) -> MatchResult<'_> {
      if self.filters.is_empty() {
        return MatchResult::Included;
      }
      let mut matched = None;
      for filter in &self.filters {
        match filter.kind {
          FilterKind::Include => {
            if matched.is_none() && filter.matches(name) {
              matched = Some(filter);
            }
          }
          FilterKind::Exclude => {
            if filter.matches(name) {
              return MatchResult::Excluded;
            }
          }
        }
      }
      if let Some(filter) = matched {
        MatchResult::Matches(filter)
      } else if self.has_exclude && !self.has_include {
        MatchResult::Included
      } else {
        MatchResult::Excluded
      }
    }
  }

  pub enum MatchResult<'a> {
    Matches(&'a Filter),
    Included,
    Excluded,
  }

  impl MatchResult<'_> {
    pub fn version_spec(&self) -> Option<&VersionReq> {
      match self {
        MatchResult::Matches(filter) => filter.version_spec(),
        _ => None,
      }
    }
    pub fn is_included(&self) -> bool {
      matches!(self, MatchResult::Included | MatchResult::Matches(_))
    }
  }

  #[cfg(test)]
  mod test {
    fn matches_filters<'a, 'b>(
      filters: impl IntoIterator<Item = &'a str>,
      name: &str,
    ) -> bool {
      let filters = super::FilterSet::from_filter_strings(filters).unwrap();
      filters.matches(name)
    }

    fn version_spec(s: &str) -> deno_semver::VersionReq {
      deno_semver::VersionReq::parse_from_specifier(s).unwrap()
    }

    #[test]
    fn basic_glob() {
      assert!(matches_filters(["foo*"], "foo"));
      assert!(matches_filters(["foo*"], "foobar"));
      assert!(!matches_filters(["foo*"], "barfoo"));

      assert!(matches_filters(["*foo"], "foo"));
      assert!(matches_filters(["*foo"], "barfoo"));
      assert!(!matches_filters(["*foo"], "foobar"));

      assert!(matches_filters(["@scope/foo*"], "@scope/foobar"));
    }

    #[test]
    fn basic_glob_with_version() {
      assert!(matches_filters(["foo*@1"], "foo",));
      assert!(matches_filters(["foo*@1"], "foobar",));
      assert!(matches_filters(["foo*@1"], "foo-bar",));
      assert!(!matches_filters(["foo*@1"], "barfoo",));
      assert!(matches_filters(["@scope/*@1"], "@scope/foo"));
    }

    #[test]
    fn glob_exclude() {
      assert!(!matches_filters(["!foo*"], "foo"));
      assert!(!matches_filters(["!foo*"], "foobar"));
      assert!(matches_filters(["!foo*"], "barfoo"));

      assert!(!matches_filters(["!*foo"], "foo"));
      assert!(!matches_filters(["!*foo"], "barfoo"));
      assert!(matches_filters(["!*foo"], "foobar"));

      assert!(!matches_filters(["!@scope/foo*"], "@scope/foobar"));
    }

    #[test]
    fn multiple_globs() {
      assert!(matches_filters(["foo*", "bar*"], "foo"));
      assert!(matches_filters(["foo*", "bar*"], "bar"));
      assert!(!matches_filters(["foo*", "bar*"], "baz"));

      assert!(matches_filters(["foo*", "!bar*"], "foo"));
      assert!(!matches_filters(["foo*", "!bar*"], "bar"));
      assert!(matches_filters(["foo*", "!bar*"], "foobar"));
      assert!(!matches_filters(["foo*", "!*bar"], "foobar"));
      assert!(!matches_filters(["foo*", "!*bar"], "baz"));

      let filters =
        super::FilterSet::from_filter_strings(["foo*@1", "bar*@2"]).unwrap();

      assert_eq!(
        filters.matching_filter("foo").version_spec().cloned(),
        Some(version_spec("1"))
      );

      assert_eq!(
        filters.matching_filter("bar").version_spec().cloned(),
        Some(version_spec("2"))
      );
    }
  }
}
