// Copyright 2018-2025 the Deno authors. MIT license.

use std::collections::HashSet;
use std::fmt::Write as _;
use std::sync::Arc;

use console_static_text::TextItem;
use deno_config::deno_json::AllowScriptsConfig;
use deno_config::deno_json::AllowScriptsValueConfig;
use deno_core::anyhow;
use deno_core::anyhow::Context;
use deno_core::anyhow::bail;
use deno_core::error::AnyError;
use deno_path_util::url_to_file_path;
use deno_semver::jsr::JsrDepPackageReq;
use deno_semver::jsr::JsrDepPackageReqParseError;
use deno_semver::package::PackageKind;
use deno_semver::package::PackageNv;
use deno_semver::package::PackageReq;
use deno_terminal::colors;
use jsonc_parser::json;

use super::CacheTopLevelDepsOptions;
use crate::args::ApproveScriptsFlags;
use crate::args::Flags;
use crate::factory::CliFactory;
use crate::npm::CliNpmResolver;
use crate::tools::pm::ConfigKind;
use crate::tools::pm::ConfigUpdater;
use crate::tools::pm::create_deno_json;
use crate::tools::pm::interactive_picker;

struct ScriptCandidate {
  req: PackageReq,
  specifier: String,
  scripts: Vec<String>,
}

pub async fn approve_scripts(
  flags: Arc<Flags>,
  approve_flags: ApproveScriptsFlags,
) -> Result<(), AnyError> {
  let mut factory = CliFactory::from_flags(flags.clone());
  let mut options = factory.cli_options()?;
  if options.start_dir.maybe_deno_json().is_none() {
    factory = create_deno_json(&flags, options)?;
    options = factory.cli_options()?;
  }
  let deno_json = options.workspace().root_deno_json().ok_or_else(|| {
    anyhow::anyhow!("A deno.json file could not be found or created")
  })?;

  let deno_json_path = url_to_file_path(&deno_json.specifier)?;

  let mut config_updater =
    ConfigUpdater::new(ConfigKind::DenoJson, deno_json_path.clone())?;

  let allow_scripts_config = deno_json.to_allow_scripts_config()?;

  let (mut allow_list, mut deny_list) = match allow_scripts_config.allow {
    AllowScriptsValueConfig::All => {
      log::info!(
        "Lifecycle scripts are already allowed for all npm packages in the workspace.",
      );
      return Ok(());
    }
    AllowScriptsValueConfig::Limited(list) => (list, allow_scripts_config.deny),
  };

  let mut existing_allowed: HashSet<PackageReq> =
    allow_list.iter().map(|req| req.req.clone()).collect();
  let deny_reqs: Vec<PackageReq> =
    deny_list.iter().map(|req| req.req.clone()).collect();

  let (approvals, denials) = if !approve_flags.packages.is_empty() {
    (
      parse_user_packages(&approve_flags.packages, &mut existing_allowed)?,
      Vec::new(),
    )
  } else {
    let npm_resolver = factory.npm_resolver().await?;
    let candidates = find_script_candidates(
      npm_resolver,
      &flags.subcommand.npm_system_info(),
      &allow_list,
      &deny_reqs,
    )?;
    if candidates.is_empty() {
      log::info!("No npm packages with lifecycle scripts need approval.");
      return Ok(());
    }

    let chosen = pick_candidates(&candidates, &mut existing_allowed)?;
    (chosen.approved, chosen.denied)
  };

  if approvals.is_empty() && denials.is_empty() {
    log::info!("No new packages to approve.");
    return Ok(());
  }

  for req in &approvals {
    allow_list.push(JsrDepPackageReq::npm(req.clone()));
  }
  for req in &denials {
    deny_list.push(JsrDepPackageReq::npm(req.clone()));
  }

  if !approvals.is_empty() {
    deny_list.retain(|entry| {
      !(entry.kind == PackageKind::Npm && approvals.contains(&entry.req))
    });
  }

  allow_list.sort_by_key(|a| a.to_string());
  allow_list.dedup_by(|a, b| a.req == b.req && a.kind == b.kind);
  deny_list.sort_by_key(|a| a.to_string());
  deny_list.dedup_by(|a, b| a.req == b.req && a.kind == b.kind);

  let updated_allow_scripts = AllowScriptsConfig {
    allow: AllowScriptsValueConfig::Limited(allow_list),
    deny: deny_list,
  };
  let allow_scripts_value = allow_scripts_to_value(&updated_allow_scripts);

  config_updater.set_allow_scripts_value(allow_scripts_value);
  config_updater.commit()?;

  for req in denials {
    log::info!(
      "{} {}{}",
      colors::yellow("Denied"),
      colors::gray("npm:"),
      req
    )
  }
  for req in approvals.iter() {
    log::info!(
      "{} {}{}",
      colors::green("Approved"),
      colors::gray("npm:"),
      req
    );
  }

  super::npm_install_after_modification(
    flags,
    None,
    CacheTopLevelDepsOptions {
      lockfile_only: approve_flags.lockfile_only,
    },
  )
  .await?;

  for req in approvals {
    log::info!(
      "{} {}{}",
      colors::cyan("Ran build script"),
      colors::gray("npm:"),
      req
    );
  }

  Ok(())
}

fn parse_user_packages(
  packages: &[String],
  existing_allowed: &mut HashSet<PackageReq>,
) -> Result<Vec<PackageReq>, AnyError> {
  let mut additions = Vec::new();
  for raw in packages {
    let req = parse_npm_package_req(raw)
      .with_context(|| format!("Failed to parse package: {}", raw))?;
    if existing_allowed.insert(req.clone()) {
      additions.push(req);
    }
  }
  Ok(additions)
}

fn find_script_candidates(
  npm_resolver: &CliNpmResolver,
  system_info: &deno_npm::NpmSystemInfo,
  allow_list: &[JsrDepPackageReq],
  deny_list: &[PackageReq],
) -> Result<Vec<ScriptCandidate>, AnyError> {
  let managed_resolver = npm_resolver.as_managed().with_context(|| {
    "Lifecycle script approval requires an npm resolution. Run `deno install` first to create one."
  })?;
  let snapshot = managed_resolver.resolution().snapshot();
  let mut candidates = Vec::new();
  let mut seen = HashSet::<PackageNv>::new();
  for package in snapshot.all_system_packages(system_info) {
    if !package.has_scripts {
      continue;
    }
    if !seen.insert(package.id.nv.clone()) {
      continue;
    }
    if allow_list
      .iter()
      .any(|req| package_req_matches_nv(&req.req, &package.id.nv))
    {
      continue;
    }
    if deny_list
      .iter()
      .any(|req| package_req_matches_nv(req, &package.id.nv))
    {
      continue;
    }
    let specifier =
      format!("npm:{}@{}", package.id.nv.name, package.id.nv.version);
    let req = PackageReq::from_str(&format!(
      "{}@{}",
      package.id.nv.name, package.id.nv.version
    ))?;
    let mut scripts = package
      .extra
      .as_ref()
      .map(|extra| {
        let mut names = extra
          .scripts
          .keys()
          .map(|k| k.to_string())
          .collect::<Vec<_>>();
        names.sort();
        names
      })
      .unwrap_or_default();
    scripts.dedup();
    candidates.push(ScriptCandidate {
      req,
      specifier,
      scripts,
    });
  }

  candidates.sort_by(|a, b| a.specifier.cmp(&b.specifier));
  Ok(candidates)
}

#[derive(Default, Debug)]
struct ChosenCandidates {
  approved: Vec<PackageReq>,
  denied: Vec<PackageReq>,
}

fn pick_candidates(
  candidates: &[ScriptCandidate],
  existing_allowed: &mut HashSet<PackageReq>,
) -> Result<ChosenCandidates, AnyError> {
  if candidates.is_empty() {
    return Ok(ChosenCandidates {
      denied: candidates.iter().map(|c| c.req.clone()).collect(),
      ..Default::default()
    });
  }

  let selected = interactive_picker::select_items(
    "Select which packages to approve lifecycle scripts for (<space> to select, ↑/↓/j/k to navigate, a to select all, i to invert selection, enter to accept, <Ctrl-c> to cancel)",
    candidates,
    HashSet::new(),
    |_idx, is_selected, is_checked, candidate| {
      render_candidate(candidate, is_selected, is_checked)
    },
  )?;

  let Some(selected) = selected else {
    return Ok(ChosenCandidates::default());
  };

  let mut approvals = Vec::with_capacity(selected.len());
  let mut denials = Vec::with_capacity(candidates.len() - selected.len());
  for (idx, candidate) in candidates.iter().enumerate() {
    if selected.contains(&idx) {
      if existing_allowed.insert(candidate.req.clone()) {
        approvals.push(candidate.req.clone());
      }
    } else {
      denials.push(candidate.req.clone());
    }
  }

  Ok(ChosenCandidates {
    approved: approvals,
    denied: denials,
  })
}

fn allow_scripts_to_value(
  config: &AllowScriptsConfig,
) -> jsonc_parser::cst::CstInputValue {
  let deny: Vec<String> = config.deny.iter().map(|r| r.to_string()).collect();
  match &config.allow {
    AllowScriptsValueConfig::All => {
      if deny.is_empty() {
        json!(true)
      } else {
        json!({ "allow": true, "deny": deny })
      }
    }
    AllowScriptsValueConfig::Limited(reqs) => {
      let allow: Vec<String> = reqs.iter().map(|req| req.to_string()).collect();
      if deny.is_empty() {
        json!(allow)
      } else {
        json!({ "allow": allow, "deny": deny })
      }
    }
  }
}

fn parse_npm_package_req(text: &str) -> Result<PackageReq, AnyError> {
  let req = match JsrDepPackageReq::from_str_loose(text) {
    Ok(JsrDepPackageReq {
      kind: PackageKind::Jsr,
      ..
    }) => {
      bail!("Only npm packages are supported: {}", text);
    }
    Ok(
      req @ JsrDepPackageReq {
        kind: PackageKind::Npm,
        ..
      },
    ) => req,
    Err(JsrDepPackageReqParseError::NotExpectedScheme(_))
      if !text.contains(':') =>
    {
      return parse_npm_package_req(&format!("npm:{text}"));
    }
    Err(e) => return Err(e.into()),
  };
  if req.req.version_req.tag().is_some() {
    bail!("Tags are not supported in the allowScripts field: {}", text);
  }
  Ok(req.req)
}

fn package_req_matches_nv(req: &PackageReq, nv: &PackageNv) -> bool {
  req.name == nv.name && req.version_req.matches(&nv.version)
}

fn render_candidate(
  candidate: &ScriptCandidate,
  is_selected: bool,
  is_checked: bool,
) -> Result<TextItem<'static>, AnyError> {
  let mut line = String::new();
  write!(
    &mut line,
    "{} {} {}",
    if is_selected {
      colors::intense_blue("❯").to_string()
    } else {
      " ".to_string()
    },
    if is_checked { "●" } else { "○" },
    candidate.specifier
  )?;
  if !candidate.scripts.is_empty() {
    write!(
      &mut line,
      " {}",
      colors::gray(format!("scripts: {}", candidate.scripts.join(", ")))
    )?;
  }
  Ok(TextItem::with_hanging_indent_owned(line, 2))
}
