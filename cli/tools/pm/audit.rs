// Copyright 2018-2026 the Deno authors. MIT license.

use std::io::Write;
use std::sync::Arc;

use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_core::futures;
use deno_core::futures::FutureExt;
use deno_core::futures::StreamExt;
use deno_core::serde_json;
use deno_npm::resolution::NpmResolutionSnapshot;
use eszip::v2::Url;
use http::header::HeaderName;
use http::header::HeaderValue;
use serde::Deserialize;

use crate::args::AuditFlags;
use crate::args::Flags;
use crate::colors;
use crate::factory::CliFactory;
use crate::http_util;
use crate::http_util::HttpClient;
use crate::http_util::HttpClientProvider;
use crate::util::console::escape_terminal_control_chars;

struct FixableAction {
  module_name: String,
  target_version: String,
  is_major: bool,
}

struct AuditResult {
  exit_code: i32,
  fixable_actions: Vec<FixableAction>,
}

pub async fn audit(
  flags: Arc<Flags>,
  audit_flags: AuditFlags,
) -> Result<i32, AnyError> {
  let factory = CliFactory::from_flags(flags.clone());
  let npm_resolver = factory.npm_resolver().await?;
  let npm_resolver = npm_resolver.as_managed().unwrap();
  let snapshot = npm_resolver.resolution().snapshot();

  let npm_url = &factory.npmrc()?.default_config.registry_url;
  let http_provider = HttpClientProvider::new(None, None);
  let http_client = http_provider
    .get_or_create()
    .context("Failed to create HTTP client")?;

  let use_socket = audit_flags.socket;
  let fix = audit_flags.fix;

  let result =
    npm::call_audits_api(audit_flags, npm_url, &snapshot, http_client).await?;

  if use_socket {
    socket_dev::call_firewall_api(
      &snapshot,
      http_provider.get_or_create().unwrap(),
    )
    .await?;
  }

  if fix && !result.fixable_actions.is_empty() {
    apply_fixes(&factory, flags, &result.fixable_actions).await?;
  }

  Ok(result.exit_code)
}

/// Outcome of checking a derived fix target against what the registry has
/// actually published.
enum PublishedTarget {
  /// The oldest published version satisfying the target.
  Version(String),
  /// Registry metadata was unavailable; proceed with the derived target rather
  /// than blocking the fix on a transient fetch failure.
  Unknown,
  /// Nothing published satisfies the target.
  None,
  /// The nearest published version crosses a major boundary the target did not,
  /// so applying it would be a major upgrade in disguise.
  CrossesMajor(String),
}

/// Snap a derived fix target to the oldest published version that satisfies it.
///
/// `target_version` is a *lower bound* (`>=X`), and when inferred from a
/// vulnerable range's exclusive upper bound it assumes that bound was really
/// released -- which is not guaranteed. Writing it into the manifest verbatim
/// can pin a version that does not exist, so resolve it against the registry
/// first. Prereleases are skipped unless the target is itself a prerelease.
async fn resolve_published_target(
  deps: &super::deps::DepManager,
  action: &FixableAction,
) -> PublishedTarget {
  let Ok(target) = deno_semver::Version::parse_standard(&action.target_version)
  else {
    return PublishedTarget::Unknown;
  };
  let Some(info) = deps
    .npm_fetch_resolver
    .package_info(&action.module_name)
    .await
  else {
    return PublishedTarget::Unknown;
  };
  let nearest = info
    .versions
    .keys()
    .filter(|v| **v >= target)
    .filter(|v| v.pre.is_empty() || !target.pre.is_empty())
    .min();
  match nearest {
    None => PublishedTarget::None,
    Some(v) if v.major > target.major => {
      PublishedTarget::CrossesMajor(v.to_string())
    }
    Some(v) => PublishedTarget::Version(v.to_string()),
  }
}

async fn apply_fixes(
  factory: &CliFactory,
  flags: Arc<Flags>,
  fixable_actions: &[FixableAction],
) -> Result<(), AnyError> {
  use deno_semver::VersionReq;

  use super::CacheTopLevelDepsOptions;

  let (mut deps, jsr_fetch_resolver) =
    super::create_dep_manager_and_resolvers(factory).await?;

  let mut fixed = Vec::new();
  let mut unfixable = Vec::new();

  // Build a map of dep name -> (dep_id, version_req_str) for matching.
  // If multiple deps share the same package name (e.g. aliases), treat
  // them as unfixable to avoid updating the wrong one.
  let mut dep_lookup: std::collections::HashMap<
    String,
    Option<(super::deps::DepId, String)>,
  > = std::collections::HashMap::new();
  for (id, dep) in deps.deps_with_ids() {
    let name = dep.req.name.to_string();
    let entry = dep_lookup.entry(name);
    match entry {
      std::collections::hash_map::Entry::Vacant(e) => {
        e.insert(Some((id, dep.req.version_req.to_string())));
      }
      std::collections::hash_map::Entry::Occupied(mut e) => {
        // Duplicate - mark as ambiguous
        e.insert(None);
      }
    }
  }

  for action in fixable_actions {
    if action.is_major {
      unfixable.push(format!(
        "{} (major upgrade to {})",
        action.module_name, action.target_version
      ));
      continue;
    }

    match dep_lookup.get(&action.module_name) {
      Some(Some((dep_id, version_req_str))) => {
        // The target can be a version that was never published: when the
        // registry omits `patched_versions` it is inferred from the vulnerable
        // range's exclusive upper bound, which assumes that bound is a real
        // release. Snap it to the oldest published version that actually
        // satisfies it, so `--fix` never commits a manifest that cannot
        // install. Done here, after the dep is known to be updatable, so an
        // untouchable dep is reported as such instead of triggering a fetch.
        let target_version = match resolve_published_target(&deps, action).await
        {
          PublishedTarget::Unknown => action.target_version.clone(),
          PublishedTarget::Version(v) => v,
          PublishedTarget::None => {
            unfixable.push(format!(
              "{} (no published version satisfies >={})",
              action.module_name, action.target_version
            ));
            continue;
          }
          PublishedTarget::CrossesMajor(v) => {
            unfixable.push(format!(
              "{} (nearest published fix {} is a major upgrade)",
              action.module_name, v
            ));
            continue;
          }
        };

        // Preserve the original version requirement style.
        // Only handle simple spec styles (caret, tilde, exact pin)
        // to avoid silently rewriting complex ranges.
        let trimmed = version_req_str.trim();
        let new_spec = if trimmed.starts_with('~') {
          format!("~{}", target_version)
        } else if trimmed.starts_with('^') {
          format!("^{}", target_version)
        } else if trimmed.chars().next().is_some_and(|c| c.is_ascii_digit()) {
          target_version.clone()
        } else {
          unfixable.push(format!(
            "{} (unsupported version spec: {})",
            action.module_name, version_req_str
          ));
          continue;
        };
        let new_version_req = VersionReq::parse_from_specifier(&new_spec)?;
        deps.update_dep(*dep_id, new_version_req);
        fixed.push(format!(
          "{} {} -> {}",
          action.module_name, version_req_str, new_spec
        ));
      }
      Some(None) => {
        unfixable.push(format!(
          "{} (ambiguous: multiple dependencies with this name)",
          action.module_name
        ));
      }
      None => {
        unfixable
          .push(format!("{} (transitive dependency)", action.module_name));
      }
    }
  }

  if !fixed.is_empty() {
    deps.commit_changes()?;

    super::npm_install_after_modification(
      flags,
      Some(jsr_fetch_resolver),
      CacheTopLevelDepsOptions {
        lockfile_only: false,
        additional_roots: vec![],
      },
    )
    .await?;
  }
  print_fix_summary(&mut std::io::stdout(), &fixed, &unfixable);

  Ok(())
}

fn print_fix_summary(
  stdout: &mut impl Write,
  fixed: &[String],
  unfixable: &[String],
) {
  if !fixed.is_empty() {
    _ = writeln!(
      stdout,
      "\nFixed {} vulnerabilit{}:",
      fixed.len(),
      if fixed.len() == 1 { "y" } else { "ies" }
    );
    for f in fixed {
      _ = writeln!(stdout, "  {}", escape_terminal_control_chars(f));
    }
  }

  if !unfixable.is_empty() {
    _ = writeln!(
      stdout,
      "\n{} vulnerabilit{} could not be fixed automatically:",
      unfixable.len(),
      if unfixable.len() == 1 { "y" } else { "ies" }
    );
    for u in unfixable {
      _ = writeln!(stdout, "  {}", escape_terminal_control_chars(u));
    }
  }
}

mod npm {
  use std::collections::HashMap;
  use std::collections::HashSet;

  use super::*;

  #[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
  enum AdvisorySeverity {
    Low,
    Moderate,
    High,
    Critical,
  }

  impl AdvisorySeverity {
    fn parse(str_: &str) -> Option<Self> {
      match str_ {
        "low" => Some(Self::Low),
        "moderate" => Some(Self::Moderate),
        "high" => Some(Self::High),
        "critical" => Some(Self::Critical),
        _ => None,
      }
    }
  }

  pub async fn call_audits_api_inner(
    client: &HttpClient,
    npm_url: Url,
    body: serde_json::Value,
  ) -> Result<BulkAuditResponse, AnyError> {
    let url = npm_url.join("-/npm/v1/security/advisories/bulk").unwrap();
    let future = client.post_json(url, &body)?.send().boxed_local();
    let response = future.await?;
    let json_str = http_util::body_to_string(response)
      .await
      .context("Failed to read response from the npm registry API")?;
    let response: BulkAuditResponse = serde_json::from_str(&json_str)
      .context("Failed to deserialize response from the npm registry API")?;
    Ok(response)
  }

  pub async fn call_audits_api(
    audit_flags: AuditFlags,
    npm_url: &Url,
    npm_resolution_snapshot: &NpmResolutionSnapshot,
    client: HttpClient,
  ) -> Result<super::AuditResult, AnyError> {
    // Build request body for the bulk advisory endpoint:
    // { "pkg-name": ["ver1", "ver2"], ... }
    let mut body_map: HashMap<String, HashSet<String>> = HashMap::new();
    for pkg in npm_resolution_snapshot.all_packages_for_every_system() {
      body_map
        .entry(pkg.id.nv.name.to_string())
        .or_default()
        .insert(pkg.id.nv.version.to_string());
    }
    let body: HashMap<String, Vec<String>> = body_map
      .into_iter()
      .map(|(k, v)| (k, v.into_iter().collect()))
      .collect();
    let body = serde_json::to_value(&body).unwrap();

    let bulk_response =
      match call_audits_api_inner(&client, npm_url.clone(), body).await {
        Ok(s) => s,
        Err(err) => {
          if audit_flags.ignore_registry_errors {
            log::error!("Failed to get data from the registry: {}", err);
            return Ok(super::AuditResult {
              exit_code: 0,
              fixable_actions: vec![],
            });
          } else {
            return Err(err);
          }
        }
      };

    // Build map of installed versions per package for vulnerability filtering
    // and fix target computation.
    let mut installed_versions: HashMap<String, Vec<deno_semver::Version>> =
      HashMap::new();
    for pkg in npm_resolution_snapshot.all_packages_for_every_system() {
      installed_versions
        .entry(pkg.id.nv.name.to_string())
        .or_default()
        .push(pkg.id.nv.version.clone());
    }

    // Convert bulk response to flat list of advisories
    let mut advisories: Vec<AuditAdvisory> = Vec::new();
    for (pkg_name, pkg_advisories) in &bulk_response {
      let installed = installed_versions
        .get(pkg_name)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
      for adv in pkg_advisories {
        // The public registry does not return `patched_versions`, so infer it
        // from the vulnerable range when missing. The inferred flag is tracked
        // so the report can flag it as a guess rather than registry-supplied.
        let api_patched =
          adv.patched_versions.clone().filter(|p| !p.is_empty());
        let (patched_versions, patched_inferred) = match api_patched {
          Some(p) => (p, false),
          None => match derive_patched_from_vulnerable(
            &adv.vulnerable_versions,
            installed,
          ) {
            Some(p) => (p, true),
            None => (String::new(), false),
          },
        };
        advisories.push(AuditAdvisory {
          title: adv.title.clone(),
          severity: adv.severity.clone(),
          url: adv.url.clone(),
          module_name: pkg_name.clone(),
          vulnerable_versions: adv.vulnerable_versions.clone(),
          patched_versions,
          patched_inferred,
          cves: adv.cves.clone(),
          ghsa_id: extract_ghsa_id(adv),
          advisory_id: adv.id,
        });
      }
    }

    // Filter out advisories where no installed version falls within
    // the vulnerable range. This handles package.json overrides that
    // force a patched version.
    advisories.retain(|adv| {
      let Ok(vulnerable_range) =
        deno_semver::VersionReq::parse_from_npm(&adv.vulnerable_versions)
      else {
        return true;
      };
      if let Some(versions) = installed_versions.get(&adv.module_name) {
        versions.iter().any(|v| vulnerable_range.matches(v))
      } else {
        false
      }
    });

    // Filter out ignored advisories, matched by GHSA id, numeric advisory id,
    // or CVE id (case-insensitive).
    if !audit_flags.ignore.is_empty() {
      let ignore: HashSet<String> = audit_flags
        .ignore
        .iter()
        .map(|s| s.trim().to_ascii_lowercase())
        .collect();
      advisories.retain(|adv| !adv.is_ignored(&ignore));
    }

    // Compute vulnerability counts from remaining advisories
    let mut vulns = AuditVulnerabilities {
      low: 0,
      moderate: 0,
      high: 0,
      critical: 0,
    };
    for adv in &advisories {
      match AdvisorySeverity::parse(&adv.severity) {
        Some(AdvisorySeverity::Low) => vulns.low += 1,
        Some(AdvisorySeverity::Moderate) => vulns.moderate += 1,
        Some(AdvisorySeverity::High) => vulns.high += 1,
        Some(AdvisorySeverity::Critical) => vulns.critical += 1,
        None => {}
      }
    }

    if vulns.total() == 0 {
      _ = writeln!(&mut std::io::stdout(), "No known vulnerabilities found",);
      return Ok(super::AuditResult {
        exit_code: 0,
        fixable_actions: vec![],
      });
    }

    advisories.sort_by_cached_key(|adv| {
      format!("{}@{}", adv.module_name, adv.vulnerable_versions)
    });

    let minimal_severity =
      AdvisorySeverity::parse(&audit_flags.severity).unwrap();

    // Derive fixable actions from advisories at or above the severity
    // threshold that have patched versions. The bulk API does not return
    // explicit "actions" like the retired full audit API did, so we
    // extract the minimum satisfying version from patched_versions ranges.
    //
    // Computed before `print_report` so the report's `Actions:` line reflects
    // what `--fix` will actually do: an advisory can have a patched range yet
    // no applicable action (e.g. the downgrade guard skips it when a newer copy
    // is already installed), and the report must not tell the user to run
    // `--fix` for something `--fix` will silently skip.
    let fixable_actions = derive_fixable_actions(
      &advisories,
      &installed_versions,
      minimal_severity,
    );
    let actions_by_module: HashMap<&str, &super::FixableAction> =
      fixable_actions
        .iter()
        .map(|a| (a.module_name.as_str(), a))
        .collect();

    print_report(
      &vulns,
      &advisories,
      minimal_severity,
      audit_flags.ignore_unfixable,
      &actions_by_module,
    );

    // Exit code 1 only if there are vulnerabilities at or above the specified level
    let exit_code = if vulns.count_at_or_above(minimal_severity) > 0 {
      1
    } else {
      0
    };
    Ok(super::AuditResult {
      exit_code,
      fixable_actions,
    })
  }

  /// Derive fix actions from advisory patched_versions ranges.
  ///
  /// Only considers advisories at or above `min_severity` so that
  /// `--fix` respects the `--severity` flag. For each vulnerable
  /// package, parse the patched_versions range to find the minimum
  /// version that fixes the vulnerability. If multiple advisories
  /// affect the same package, pick the highest target version so all
  /// are resolved. Compare the target major version with the installed
  /// major version to determine if it is a major upgrade.
  fn derive_fixable_actions(
    advisories: &[AuditAdvisory],
    installed_versions: &HashMap<String, Vec<deno_semver::Version>>,
    min_severity: AdvisorySeverity,
  ) -> Vec<super::FixableAction> {
    use deno_semver::Version;

    // module_name -> (target_version, is_major)
    let mut best_target: HashMap<String, (Version, bool)> = HashMap::new();

    for adv in advisories {
      // Skip advisories below the severity threshold
      let Some(severity) = AdvisorySeverity::parse(&adv.severity) else {
        continue;
      };
      if severity < min_severity {
        continue;
      }

      if adv.patched_versions.is_empty() {
        continue;
      }

      let Some(target) = min_version_from_range(&adv.patched_versions) else {
        continue;
      };

      let installed = installed_versions.get(&adv.module_name);
      // Never propose a target that isn't strictly newer than the *newest*
      // installed copy (`max`): that would be a downgrade or no-op, not a fix.
      // Deliberately conservative for multi-version installs -- if a package is
      // present at both a vulnerable and a non-vulnerable copy, propose nothing
      // rather than risk rewriting the manifest backwards.
      if let Some(max_installed) = installed.and_then(|vs| vs.iter().max())
        && target <= *max_installed
      {
        continue;
      }

      // Classify against the *oldest* installed copy (`min`) so a fix that is a
      // major bump for any copy is treated as major (non-auto-applicable).
      let installed_major = installed
        .and_then(|vs| vs.iter().min())
        .map(|v| v.major)
        .unwrap_or(0);
      let is_major = target.major > installed_major;

      match best_target.get(&adv.module_name) {
        Some((existing, _)) if *existing >= target => {}
        _ => {
          best_target.insert(adv.module_name.clone(), (target, is_major));
        }
      }
    }

    best_target
      .into_iter()
      .map(|(module_name, (target, is_major))| super::FixableAction {
        module_name,
        target_version: target.to_string(),
        is_major,
      })
      .collect()
  }

  /// Extract the minimum patched version from a npm version range string.
  ///
  /// Handles common patched_versions formats from npm advisories:
  /// - ">=1.1.0" or ">=1.1.0 <2.0.0" (lower-bounded range)
  /// - ">1.0.0" (exclusive lower bound -- we cannot determine exact
  ///   min version, so skip)
  /// - "=1.1.0" or "1.1.0" (exact version)
  ///
  /// Returns None if the range cannot be parsed or has no usable lower
  /// bound, in which case the advisory is skipped for auto-fix.
  fn min_version_from_range(range: &str) -> Option<deno_semver::Version> {
    let trimmed = range.trim();

    // ">=X.Y.Z ..." -- most common npm patched_versions format
    if let Some(rest) = trimmed.strip_prefix(">=") {
      let ver_str = rest.split_whitespace().next()?;
      return deno_semver::Version::parse_standard(ver_str).ok();
    }

    // "=X.Y.Z" -- exact version
    if let Some(rest) = trimmed.strip_prefix('=') {
      let ver_str = rest.split_whitespace().next()?;
      return deno_semver::Version::parse_standard(ver_str).ok();
    }

    // Bare version "X.Y.Z" (no operator) -- treat as exact
    if trimmed.chars().next().is_some_and(|c| c.is_ascii_digit()) {
      let ver_str = trimmed.split_whitespace().next()?;
      return deno_semver::Version::parse_standard(ver_str).ok();
    }

    // ">X.Y.Z", "~X.Y.Z", "^X.Y.Z", "||" ranges, etc.
    // We cannot reliably determine the exact minimum version
    // for these, so skip them (advisory will not be auto-fixed).
    None
  }

  /// Whether the version `--fix` would move this package to actually satisfies
  /// this advisory's patched range.
  ///
  /// Actions are derived per module (the highest target across that module's
  /// advisories), so a module-level action does not automatically resolve every
  /// advisory on that module -- one whose `patched_versions` is unusable for
  /// fixing (e.g. an exclusive `>1.0.0` lower bound) contributes no target of
  /// its own and may not be covered by a sibling's.
  fn action_resolves(
    adv: &AuditAdvisory,
    action: &super::FixableAction,
  ) -> bool {
    let Ok(target) =
      deno_semver::Version::parse_standard(&action.target_version)
    else {
      return false;
    };
    match deno_semver::VersionReq::parse_from_npm(&adv.patched_versions) {
      Ok(req) => req.matches(&target),
      // Unparseable patched range: fall back to the derived minimum so an
      // advisory that produced this very target still counts as resolved.
      Err(_) => min_version_from_range(&adv.patched_versions)
        .is_some_and(|min| target >= min),
    }
  }

  fn print_report(
    vulns: &AuditVulnerabilities,
    advisories: &[AuditAdvisory],
    minimal_severity: AdvisorySeverity,
    ignore_unfixable: bool,
    actions_by_module: &HashMap<&str, &super::FixableAction>,
  ) {
    let stdout = &mut std::io::stdout();
    print_report_to(
      stdout,
      vulns,
      advisories,
      minimal_severity,
      ignore_unfixable,
      actions_by_module,
    );
  }

  fn print_report_to(
    stdout: &mut impl Write,
    vulns: &AuditVulnerabilities,
    advisories: &[AuditAdvisory],
    minimal_severity: AdvisorySeverity,
    ignore_unfixable: bool,
    actions_by_module: &HashMap<&str, &super::FixableAction>,
  ) {
    for adv in advisories {
      let Some(severity) = AdvisorySeverity::parse(&adv.severity) else {
        continue;
      };
      if severity < minimal_severity {
        continue;
      }

      // With inference in play, `patched_versions` is empty only when we could
      // not determine a fix (inclusive/open-ended/conflicting ranges) -- which
      // is indistinguishable from a genuinely unpatched advisory, since the
      // public registry never marks one as such. `--ignore-unfixable` therefore
      // suppresses both. This is a deliberate, opt-in trade-off documented in
      // the PR: the tool never *asserts* a fix it can't substantiate
      // (derivation fails closed), and advisories are only hidden when the user
      // explicitly asks to hide unfixable ones.
      let has_fix = !adv.patched_versions.is_empty();
      if !has_fix && ignore_unfixable {
        continue;
      }

      let title = escape_terminal_control_chars(&adv.title);
      let module_name = escape_terminal_control_chars(&adv.module_name);
      let vulnerable_versions =
        escape_terminal_control_chars(&adv.vulnerable_versions);
      let patched_versions =
        escape_terminal_control_chars(&adv.patched_versions);
      let url = escape_terminal_control_chars(&adv.url);

      _ = writeln!(stdout, "╭ {}", colors::bold(title));
      _ = writeln!(
        stdout,
        "│ {}   {}",
        colors::gray("Severity:"),
        match severity {
          AdvisorySeverity::Low => colors::bold("low"),
          AdvisorySeverity::Moderate => colors::yellow("moderate"),
          AdvisorySeverity::High => colors::red("high"),
          AdvisorySeverity::Critical => colors::red("critical"),
        }
      );
      if let Some(id) = adv.display_id() {
        // Surfaced so it can be passed to `deno audit --ignore <ID>`.
        _ = writeln!(
          stdout,
          "│ {}         {}",
          colors::gray("ID:"),
          escape_terminal_control_chars(&id)
        );
      }
      _ = writeln!(stdout, "│ {}    {}", colors::gray("Package:"), module_name);
      _ = writeln!(
        stdout,
        "│ {} {}",
        colors::gray("Vulnerable:"),
        vulnerable_versions
      );
      if has_fix {
        // Inferred targets are a heuristic (see derive_patched_from_vulnerable)
        // rather than registry-supplied, so flag them as such.
        let inferred = if adv.patched_inferred {
          colors::gray(" (inferred)").to_string()
        } else {
          String::new()
        };
        _ = writeln!(
          stdout,
          "│ {}    {}{}",
          colors::gray("Patched:"),
          patched_versions,
          inferred
        );
        _ = writeln!(stdout, "│ {}       {}", colors::gray("Info:"), url);
        // Drive the `Actions:` line off the actual derived action -- both
        // whether there is one and which version it targets. Actions are
        // per-module while advisories are per-vulnerability, so a module with
        // several advisories gets a single target (the highest); printing this
        // advisory's own range would name a version `--fix` never writes.
        //
        // An advisory can also have a patched range yet no applicable action at
        // all (e.g. the downgrade guard skips it when a newer copy is already
        // installed). Telling the user to run `--fix` for something it will
        // silently skip is worse than saying nothing.
        //
        // The `(inferred)` note is shown on `Patched:` above; repeating it here
        // reads as though the action itself is inferred, so keep it to one line.
        match actions_by_module.get(adv.module_name.as_str()) {
          Some(action) if action_resolves(adv, action) => {
            // `--fix` refuses major upgrades, so don't imply it will apply one.
            // It also refuses transitive, aliased and complex-spec deps, but
            // those are only knowable once the manifest is loaded in
            // `apply_fixes`, so they stay reported there.
            let note = if action.is_major {
              colors::gray(" (major upgrade, not applied by --fix)").to_string()
            } else {
              String::new()
            };
            // Printed as a lower bound rather than an exact version: `--fix`
            // snaps the target up to the oldest *published* version satisfying
            // it, which for an inferred target need not be the target itself.
            _ = writeln!(
              stdout,
              "╰ {}    update {} to >={}{}",
              colors::gray("Actions:"),
              module_name,
              escape_terminal_control_chars(&action.target_version),
              note
            );
          }
          _ => {
            _ = writeln!(
              stdout,
              "╰ {}    no automatic fix available",
              colors::gray("Actions:"),
            );
          }
        }
      } else {
        _ = writeln!(stdout, "╰ {}       {}", colors::gray("Info:"), url);
      }
      _ = writeln!(stdout);
    }

    _ = writeln!(
      stdout,
      "Found {} vulnerabilities",
      colors::red(vulns.total()),
    );
    _ = writeln!(
      stdout,
      "Severity: {} {}, {} {}, {} {}, {} {}",
      colors::bold(vulns.low),
      colors::bold("low"),
      colors::yellow(vulns.moderate),
      colors::yellow("moderate"),
      colors::red(vulns.high),
      colors::red("high"),
      colors::red(vulns.critical),
      colors::red("critical"),
    );
  }

  /// Advisory item from the bulk API response.
  #[derive(Debug, Deserialize)]
  pub struct BulkAdvisoryItem {
    /// Numeric advisory id assigned by the registry.
    #[serde(default)]
    pub id: Option<u64>,
    /// GitHub advisory id (`GHSA-xxxx-xxxx-xxxx`). The public npm registry
    /// only exposes this via `url`, but some registries include it directly.
    #[serde(default)]
    pub github_advisory_id: Option<String>,
    pub url: String,
    pub title: String,
    pub severity: String,
    pub vulnerable_versions: String,
    #[serde(default)]
    pub patched_versions: Option<String>,
    #[serde(default)]
    pub cves: Vec<String>,
    #[serde(default)]
    #[allow(dead_code, reason = "deserialized but not yet displayed")]
    pub cwe: Vec<String>,
  }

  /// Extract the GitHub advisory id (`GHSA-xxxx-xxxx-xxxx`) for an advisory.
  ///
  /// Prefers an explicit `github_advisory_id` field, falling back to the last
  /// path segment of the advisory `url`, which for public npm registry
  /// responses looks like `https://github.com/advisories/GHSA-xxxx-xxxx-xxxx`.
  ///
  /// Both sources are checked for the `GHSA-` prefix (case-insensitively, since
  /// `--ignore` matching is case-insensitive too) so a non-GitHub `url` or an
  /// unrelated registry field never gets surfaced as a GHSA id.
  pub fn extract_ghsa_id(item: &BulkAdvisoryItem) -> Option<String> {
    fn as_ghsa_id(candidate: &str) -> Option<String> {
      let candidate = candidate.trim();
      candidate
        .get(..5)
        .filter(|prefix| prefix.eq_ignore_ascii_case("GHSA-"))
        .map(|_| candidate.to_string())
    }

    if let Some(id) = item.github_advisory_id.as_deref()
      && let Some(id) = as_ghsa_id(id)
    {
      return Some(id);
    }
    // Strip the query/fragment before trimming a trailing slash, so
    // `.../GHSA-xxxx/?utm=1` still yields the id.
    let path = item.url.split(['?', '#']).next().unwrap_or(&item.url);
    as_ghsa_id(path.trim_end_matches('/').rsplit('/').next()?)
  }

  /// Derive a `patched_versions` range from an advisory's `vulnerable_versions`
  /// when the registry does not provide one.
  ///
  /// The public npm bulk advisory endpoint only returns `vulnerable_versions`,
  /// so the first fixed version is inferred from the vulnerable range's
  /// exclusive upper bound. A vulnerable range can be a disjunction of
  /// `||`-separated alternatives (e.g. `>=4.0.0 <4.17.21 || >=5.0.0 <5.0.3`);
  /// the fix depends on which alternative the installed version falls in, so
  /// this derives per-installed-version and fails closed on ambiguity.
  ///
  /// For example `<1.1.0` or `>=1.0.0 <1.1.0` with an installed `1.0.0` yields
  /// `>=1.1.0`. Returns `None` (no inferred fix) when the matching alternative
  /// has no exclusive upper bound (`<=1.1.0`, open-ended `>=1.0.0`) or when
  /// installed versions map to conflicting fix targets, so the tool never
  /// asserts a fix -- or worse, a downgrade -- it cannot substantiate.
  pub fn derive_patched_from_vulnerable(
    vulnerable: &str,
    installed: &[deno_semver::Version],
  ) -> Option<String> {
    use deno_semver::VersionReq;

    let alternatives: Vec<&str> = vulnerable
      .split("||")
      .map(str::trim)
      .filter(|s| !s.is_empty())
      .collect();
    if alternatives.is_empty() {
      return None;
    }

    let mut target: Option<deno_semver::Version> = None;
    for version in installed {
      // Every alternative describing this installed version's vulnerability.
      // Alternatives are normally disjoint; if overlapping ones disagree on the
      // fixed version there is no single answer, so fail closed rather than
      // picking whichever happens to come first.
      let mut matching = alternatives.iter().filter(|branch| {
        VersionReq::parse_from_npm(branch)
          .map(|req| req.matches(version))
          .unwrap_or(false)
      });
      let Some(branch) = matching.next() else {
        // Not vulnerable under any alternative (e.g. already patched); skip.
        continue;
      };
      // The fixed version is that alternative's exclusive upper bound.
      let Some(upper) = exclusive_upper_bound(branch) else {
        // Open-ended vulnerable range: no known fix. Fail closed.
        return None;
      };
      for other in matching {
        if exclusive_upper_bound(other) != Some(upper.clone()) {
          return None;
        }
      }
      match &target {
        None => target = Some(upper),
        Some(existing) if *existing == upper => {}
        // Installed versions disagree on the fix target. Fail closed.
        Some(_) => return None,
      }
    }

    target.map(|v| format!(">={}", v))
  }

  /// Extract the exclusive upper bound (`<X.Y.Z`, but not `<=X.Y.Z`) from a
  /// single conjunctive npm range. Tolerates a space after the operator
  /// (`< 1.1.0`). If a range lists several exclusive upper bounds (e.g. a
  /// malformed `<1.1.0 <2.0.0`), the smallest is returned -- the conservative
  /// choice for the first fixed version. Returns `None` if there is none.
  fn exclusive_upper_bound(range: &str) -> Option<deno_semver::Version> {
    let tokens: Vec<&str> = range.split_whitespace().collect();
    let mut min: Option<deno_semver::Version> = None;
    let mut i = 0;
    while i < tokens.len() {
      if let Some(rest) = tokens[i].strip_prefix('<') {
        // "<=" is an inclusive bound; we can't know the next fixed version.
        if rest.starts_with('=') {
          i += 1;
          continue;
        }
        let ver_str = if rest.is_empty() {
          // Operator separated from the version by whitespace: "< 1.1.0".
          i += 1;
          tokens.get(i).copied().unwrap_or("")
        } else {
          rest
        };
        if let Ok(v) = deno_semver::Version::parse_standard(ver_str) {
          match &min {
            Some(m) if *m <= v => {}
            _ => min = Some(v),
          }
        }
      }
      i += 1;
    }
    min
  }

  /// The bulk advisory endpoint response: { "package-name": [advisory, ...] }
  pub type BulkAuditResponse = HashMap<String, Vec<BulkAdvisoryItem>>;

  /// Internal advisory representation with module name from the response key.
  struct AuditAdvisory {
    title: String,
    severity: String,
    url: String,
    module_name: String,
    vulnerable_versions: String,
    patched_versions: String,
    /// Whether `patched_versions` was inferred from `vulnerable_versions`
    /// rather than supplied by the registry.
    patched_inferred: bool,
    cves: Vec<String>,
    /// GitHub advisory id (`GHSA-...`), if it could be determined.
    ghsa_id: Option<String>,
    /// Numeric advisory id from the registry, if present.
    advisory_id: Option<u64>,
  }

  impl AuditAdvisory {
    /// The identifier to surface to the user for `--ignore`. Prefers the GHSA
    /// id, then the numeric id, then the first CVE.
    fn display_id(&self) -> Option<String> {
      self
        .ghsa_id
        .clone()
        .or_else(|| self.advisory_id.map(|id| id.to_string()))
        .or_else(|| self.cves.first().cloned())
    }

    /// Whether this advisory matches one of the given (lower-cased) ignore
    /// tokens, by GHSA id, numeric id, or CVE id.
    fn is_ignored(&self, ignore: &HashSet<String>) -> bool {
      if self
        .ghsa_id
        .as_ref()
        .is_some_and(|g| ignore.contains(&g.to_ascii_lowercase()))
      {
        return true;
      }
      if self
        .advisory_id
        .is_some_and(|id| ignore.contains(&id.to_string()))
      {
        return true;
      }
      self
        .cves
        .iter()
        .any(|cve| ignore.contains(&cve.to_ascii_lowercase()))
    }
  }

  struct AuditVulnerabilities {
    low: i32,
    moderate: i32,
    high: i32,
    critical: i32,
  }

  impl AuditVulnerabilities {
    fn total(&self) -> i32 {
      self.low + self.moderate + self.high + self.critical
    }

    fn count_at_or_above(&self, min_severity: AdvisorySeverity) -> i32 {
      match min_severity {
        AdvisorySeverity::Low => self.total(),
        AdvisorySeverity::Moderate => self.moderate + self.high + self.critical,
        AdvisorySeverity::High => self.high + self.critical,
        AdvisorySeverity::Critical => self.critical,
      }
    }
  }

  #[cfg(test)]
  mod tests {
    use super::*;

    fn version(v: &str) -> deno_semver::Version {
      deno_semver::Version::parse_standard(v).unwrap()
    }

    #[test]
    fn print_report_escapes_advisory_controls() {
      let vulns = AuditVulnerabilities {
        low: 0,
        moderate: 0,
        high: 1,
        critical: 0,
      };
      let advisories = [AuditAdvisory {
        title: "title\x1b[2J".to_string(),
        severity: "high".to_string(),
        url: "https://example.com/\u{202e}info".to_string(),
        module_name: "pkg\nname".to_string(),
        vulnerable_versions: "<1\u{009b}31m".to_string(),
        patched_versions: ">=2\x07".to_string(),
        patched_inferred: false,
        cves: vec![],
        ghsa_id: None,
        advisory_id: None,
      }];
      let mut output = Vec::new();
      let actions_by_module = HashMap::new();

      print_report_to(
        &mut output,
        &vulns,
        &advisories,
        AdvisorySeverity::Low,
        false,
        &actions_by_module,
      );

      let output = String::from_utf8(output).unwrap();
      assert!(output.contains(r"title\u{1b}[2J"));
      assert!(output.contains(r"pkg\nname"));
      assert!(output.contains(r"<1\u{9b}31m"));
      assert!(output.contains(r">=2\u{7}"));
      assert!(output.contains(r"https://example.com/\u{202e}info"));
      assert!(!output.contains("title\x1b[2J"));
      assert!(!output.contains('\u{202e}'));
    }

    #[test]
    fn test_derive_fixable_actions_downgrade_guard() {
      // An advisory whose patched range is older than what is installed must
      // never produce an action -- doing so would rewrite the manifest
      // backwards. This is exactly the case a wrong inferred target (from a
      // disjoint vulnerable range) could hit.
      let advisories = vec![AuditAdvisory {
        title: "t".to_string(),
        severity: "high".to_string(),
        url: "https://example.com/vuln/1".to_string(),
        module_name: "lodash".to_string(),
        vulnerable_versions: "<4.17.21".to_string(),
        patched_versions: ">=4.17.21".to_string(),
        patched_inferred: true,
        cves: vec![],
        ghsa_id: None,
        advisory_id: None,
      }];
      let mut installed = HashMap::new();
      installed.insert("lodash".to_string(), vec![version("5.0.1")]);

      let actions =
        derive_fixable_actions(&advisories, &installed, AdvisorySeverity::Low);
      assert!(actions.is_empty());
    }

    #[test]
    fn test_exclusive_upper_bound() {
      assert_eq!(exclusive_upper_bound("<1.1.0"), Some(version("1.1.0")));
      assert_eq!(
        exclusive_upper_bound(">=1.0.0 <1.1.0"),
        Some(version("1.1.0"))
      );
      // Space after the operator.
      assert_eq!(exclusive_upper_bound("< 1.1.0"), Some(version("1.1.0")));
      // Inclusive and open-ended bounds have no exclusive upper bound.
      assert_eq!(exclusive_upper_bound("<=1.1.0"), None);
      assert_eq!(exclusive_upper_bound(">=1.0.0"), None);
      // Multiple exclusive upper bounds -> take the smallest (conservative).
      assert_eq!(
        exclusive_upper_bound("<2.0.0 <1.1.0"),
        Some(version("1.1.0"))
      );
    }

    fn advisory(patched: &str) -> AuditAdvisory {
      AuditAdvisory {
        title: "t".to_string(),
        severity: "high".to_string(),
        url: "https://example.com/vuln/1".to_string(),
        module_name: "lodash".to_string(),
        vulnerable_versions: "<4.17.21".to_string(),
        patched_versions: patched.to_string(),
        patched_inferred: false,
        cves: vec![],
        ghsa_id: None,
        advisory_id: None,
      }
    }

    fn action(target: &str) -> super::super::FixableAction {
      super::super::FixableAction {
        module_name: "lodash".to_string(),
        target_version: target.to_string(),
        is_major: false,
      }
    }

    #[test]
    fn test_action_resolves() {
      // The module-wide target satisfies this advisory's patched range.
      assert!(action_resolves(&advisory(">=4.17.21"), &action("4.17.21")));
      assert!(action_resolves(&advisory(">=4.17.21"), &action("4.17.25")));
      // A sibling advisory's lower target does not resolve this one -- the
      // report must not claim `--fix` handles it.
      assert!(!action_resolves(&advisory(">=4.17.25"), &action("4.17.21")));
      // Exclusive lower bound: unusable for deriving a target, but a target
      // above it still resolves the advisory.
      assert!(action_resolves(&advisory(">4.17.20"), &action("4.17.21")));
      assert!(!action_resolves(&advisory(">4.17.20"), &action("4.17.20")));
    }

    #[test]
    fn test_derive_fixable_actions_picks_highest_target() {
      // Two advisories on one module: `--fix` moves to the highest target, and
      // both advisories must be judged against that single target.
      let advisories = vec![advisory(">=4.17.21"), advisory(">=4.17.25")];
      let mut installed = HashMap::new();
      installed.insert("lodash".to_string(), vec![version("4.17.20")]);

      let actions =
        derive_fixable_actions(&advisories, &installed, AdvisorySeverity::Low);
      assert_eq!(actions.len(), 1);
      assert_eq!(actions[0].target_version, "4.17.25");
      assert!(action_resolves(&advisories[0], &actions[0]));
      assert!(action_resolves(&advisories[1], &actions[0]));
    }
  }
}

mod socket_dev {
  use super::*;

  pub async fn call_firewall_api(
    npm_resolution_snapshot: &NpmResolutionSnapshot,
    client: HttpClient,
  ) -> Result<(), AnyError> {
    let purls = npm_resolution_snapshot
      .all_packages_for_every_system()
      .map(|package| {
        format!("pkg:npm/{}@{}", package.id.nv.name, package.id.nv.version)
      })
      .collect::<Vec<_>>();

    let api_key = std::env::var("SOCKET_API_KEY").ok();

    let mut purl_responses = if let Some(api_key) = api_key {
      call_authenticated_api(&client, &purls, &api_key).await?
    } else {
      call_unauthenticated_api(&client, &purls).await?
    };

    purl_responses.sort_by_cached_key(|r| r.name.to_string());

    print_firewall_report(&purl_responses);

    Ok(())
  }

  async fn call_authenticated_api(
    client: &HttpClient,
    purls: &[String],
    api_key: &str,
  ) -> Result<Vec<FirewallResponse>, AnyError> {
    let socket_dev_url =
      std::env::var("SOCKET_DEV_URL").ok().unwrap_or_else(|| {
        "https://api.socket.dev/v0/purl?actions=error,warn".to_string()
      });
    let url = Url::parse(&socket_dev_url).unwrap();

    let body = serde_json::json!({
      "components": purls.iter().map(|purl| {
        serde_json::json!({ "purl": purl })
      }).collect::<Vec<_>>()
    });

    let auth_value = HeaderValue::from_str(&format!("Bearer {}", api_key))
      .context("Failed to create Authorization header")?;

    let request = client
      .post_json(url, &body)?
      .header(HeaderName::from_static("authorization"), auth_value);

    let response = request.send().boxed_local().await?;
    let text = http_util::body_to_string(response).await?;

    // Response is nJSON
    let responses = text
      .lines()
      .filter(|line| !line.trim().is_empty())
      .map(|line| {
        serde_json::from_str::<FirewallResponse>(line)
          .context("Failed to parse Socket.dev response")
      })
      .collect::<Result<Vec<_>, _>>()?;

    Ok(responses)
  }

  async fn call_unauthenticated_api(
    client: &HttpClient,
    purls: &[String],
  ) -> Result<Vec<FirewallResponse>, AnyError> {
    let socket_dev_url = std::env::var("SOCKET_DEV_URL")
      .ok()
      .unwrap_or_else(|| "https://firewall-api.socket.dev/".to_string());

    let futures = purls
      .iter()
      .map(|purl| {
        let url = Url::parse(&format!(
          "{}purl/{}",
          socket_dev_url,
          percent_encoding::utf8_percent_encode(
            purl,
            percent_encoding::NON_ALPHANUMERIC
          )
        ))
        .unwrap();
        client.download_text(url).boxed_local()
      })
      .collect::<Vec<_>>();

    let purl_results = futures::stream::iter(futures)
      .buffer_unordered(20)
      .collect::<Vec<_>>()
      .await;

    let responses = purl_results
      .into_iter()
      .filter_map(|result| match result {
        Ok(a) => Some(a),
        Err(err) => {
          log::error!("Failed to get PURL result {:?}", err);
          None
        }
      })
      .filter_map(|json_response| {
        match serde_json::from_str::<FirewallResponse>(&json_response) {
          Ok(response) => Some(response),
          Err(err) => {
            log::error!("Failed deserializing socket.dev response {:?}", err);
            None
          }
        }
      })
      .collect::<Vec<_>>();

    Ok(responses)
  }

  fn print_firewall_report(responses: &[FirewallResponse]) {
    let stdout = &mut std::io::stdout();
    print_firewall_report_to(stdout, responses);
  }

  fn print_firewall_report_to(
    stdout: &mut impl Write,
    responses: &[FirewallResponse],
  ) {
    let responses_with_alerts = responses
      .iter()
      .filter(|r| !r.alerts.is_empty())
      .collect::<Vec<_>>();

    if responses_with_alerts.is_empty() {
      return;
    }

    _ = writeln!(stdout);
    _ = writeln!(stdout, "{}", colors::bold("Socket.dev firewall report"));
    _ = writeln!(stdout);

    // Count total alerts by severity
    let mut total_critical = 0;
    let mut total_high = 0;
    let mut total_medium = 0;
    let mut total_low = 0;
    let mut packages_with_issues = 0;

    for response in responses_with_alerts {
      packages_with_issues += 1;

      _ = writeln!(
        stdout,
        "╭ pkg:npm/{}@{}",
        escape_terminal_control_chars(&response.name),
        escape_terminal_control_chars(&response.version)
      );

      if let Some(score) = &response.score {
        _ = writeln!(
          stdout,
          "│ {:<20} {:>3}",
          colors::gray("Supply Chain Risk:"),
          format_score(score.supply_chain)
        );
        _ = writeln!(
          stdout,
          "│ {:<20} {:>3}",
          colors::gray("Maintenance:"),
          format_score(score.maintenance)
        );
        _ = writeln!(
          stdout,
          "│ {:<20} {:>3}",
          colors::gray("Quality:"),
          format_score(score.quality)
        );
        _ = writeln!(
          stdout,
          "│ {:<20} {:>3}",
          colors::gray("Vulnerabilities:"),
          format_score(score.vulnerability)
        );
        _ = writeln!(
          stdout,
          "│ {:<20} {:>3}",
          colors::gray("License:"),
          format_score(score.license)
        );
      }

      // critical and high are counted as one for display.
      let mut critical_count = 0;
      let mut medium_count = 0;
      let mut low_count = 0;

      for alert in &response.alerts {
        match alert.severity.as_str() {
          "critical" => {
            total_critical += 1;
            critical_count += 1;
          }
          "high" => {
            total_high += 1;
            critical_count += 1;
          }
          "medium" => {
            total_medium += 1;
            medium_count += 1;
          }
          "low" => {
            total_low += 1;
            low_count += 1;
          }
          _ => {}
        }
      }

      if !response.alerts.is_empty() {
        let alerts_str = response
          .alerts
          .iter()
          .map(|alert| {
            let severity_bracket = match alert.severity.as_str() {
              "critical" => colors::red("critical").to_string(),
              "high" => colors::red("high").to_string(),
              "medium" => colors::yellow("medium").to_string(),
              "low" => "low".to_string(),
              _ => escape_terminal_control_chars(&alert.severity).into_owned(),
            };
            format!(
              "[{}] {}",
              severity_bracket,
              escape_terminal_control_chars(&alert.r#type)
            )
          })
          .collect::<Vec<_>>()
          .join(", ");

        let label = format!(
          "Alerts ({}/{}/{}):",
          critical_count, medium_count, low_count
        );
        _ = writeln!(stdout, "╰ {:<20} {}", colors::gray(&label), alerts_str);
      } else {
        _ = writeln!(stdout, "╰");
      }
      _ = writeln!(stdout);
    }

    let total_alerts = total_critical + total_high + total_medium + total_low;

    if total_alerts == 0 && packages_with_issues == 0 {
      _ = writeln!(stdout, "No security alerts found from Socket.dev");
      return;
    }

    if total_alerts > 0 {
      _ = writeln!(
        stdout,
        "Found {} alerts across {} packages",
        colors::red(total_alerts),
        colors::bold(packages_with_issues)
      );
      _ = writeln!(
        stdout,
        "Severity: {} {}, {} {}, {} {}, {} {}",
        colors::bold(total_low),
        colors::bold("low"),
        colors::yellow(total_medium),
        colors::yellow("medium"),
        colors::red(total_high),
        colors::red("high"),
        colors::red(total_critical),
        colors::red("critical"),
      );
    }
  }

  fn format_score(score: f64) -> String {
    let percentage = (score * 100.0) as i32;
    let colored = if percentage >= 80 {
      colors::green(percentage)
    } else if percentage >= 60 {
      colors::yellow(percentage)
    } else {
      colors::red(percentage)
    };
    format!("{}", colored)
  }

  #[derive(Debug, Deserialize)]
  #[serde(rename_all = "camelCase")]
  pub struct FirewallScore {
    pub license: f64,
    pub maintenance: f64,
    #[allow(dead_code, reason = "we don't use it yet")]
    pub overall: f64,
    pub quality: f64,
    pub supply_chain: f64,
    pub vulnerability: f64,
  }

  #[derive(Debug, Deserialize)]
  #[serde(rename_all = "camelCase")]
  pub struct FirewallAlert {
    pub r#type: String,
    #[allow(dead_code, reason = "we don't use it yet")]
    pub action: String,
    pub severity: String,
    #[allow(dead_code, reason = "we don't use it yet")]
    pub category: String,
  }

  #[derive(Debug, Deserialize)]
  #[serde(rename_all = "camelCase")]
  pub struct FirewallResponse {
    #[allow(dead_code, reason = "we don't use it yet")]
    pub id: String,
    pub name: String,
    pub version: String,
    pub score: Option<FirewallScore>,
    #[serde(default)]
    pub alerts: Vec<FirewallAlert>,
  }

  #[cfg(test)]
  mod tests {
    use super::*;

    #[test]
    fn print_firewall_report_escapes_external_text() {
      let responses = [FirewallResponse {
        id: "id".to_string(),
        name: "pkg\nname".to_string(),
        version: "1\u{202e}.0".to_string(),
        score: None,
        alerts: vec![
          FirewallAlert {
            r#type: "supply\x1b[2J\u{200b}chain".to_string(),
            action: "warn".to_string(),
            severity: "high".to_string(),
            category: "test".to_string(),
          },
          FirewallAlert {
            r#type: "other".to_string(),
            action: "warn".to_string(),
            severity: "custom\nseverity".to_string(),
            category: "test".to_string(),
          },
        ],
      }];
      let mut output = Vec::new();

      print_firewall_report_to(&mut output, &responses);

      let output = String::from_utf8(output).unwrap();
      assert!(output.contains(r"pkg\nname"));
      assert!(output.contains(r"1\u{202e}.0"));
      assert!(output.contains(r"supply\u{1b}[2J\u{200b}chain"));
      assert!(output.contains(r"custom\nseverity"));
      assert!(!output.contains("pkg\nname"));
      assert!(!output.contains('\u{202e}'));
      assert!(!output.contains("supply\x1b[2J"));
    }
  }
}

#[cfg(test)]
mod tests {
  use deno_core::serde_json;

  use super::npm::BulkAdvisoryItem;
  use super::npm::BulkAuditResponse;
  use super::npm::derive_patched_from_vulnerable;
  use super::npm::extract_ghsa_id;

  fn advisory(json: serde_json::Value) -> BulkAdvisoryItem {
    serde_json::from_value(json).unwrap()
  }

  #[test]
  fn test_extract_ghsa_id_from_url() {
    let item = advisory(serde_json::json!({
      "url": "https://github.com/advisories/GHSA-mh99-v99m-4gvg",
      "title": "t",
      "severity": "high",
      "vulnerable_versions": "<1.0.0"
    }));
    assert_eq!(
      extract_ghsa_id(&item).as_deref(),
      Some("GHSA-mh99-v99m-4gvg")
    );
  }

  #[test]
  fn test_extract_ghsa_id_prefers_explicit_field() {
    let item = advisory(serde_json::json!({
      "github_advisory_id": "GHSA-aaaa-bbbb-cccc",
      "url": "https://example.com/vuln/1",
      "title": "t",
      "severity": "high",
      "vulnerable_versions": "<1.0.0"
    }));
    assert_eq!(
      extract_ghsa_id(&item).as_deref(),
      Some("GHSA-aaaa-bbbb-cccc")
    );
  }

  #[test]
  fn test_extract_ghsa_id_ignores_non_ghsa_explicit_field() {
    // A registry that populates the field with something else must not have it
    // surfaced as a GHSA id; fall back to the URL.
    let item = advisory(serde_json::json!({
      "github_advisory_id": "not-an-advisory-id",
      "url": "https://github.com/advisories/GHSA-mh99-v99m-4gvg",
      "title": "t",
      "severity": "high",
      "vulnerable_versions": "<1.0.0"
    }));
    assert_eq!(
      extract_ghsa_id(&item).as_deref(),
      Some("GHSA-mh99-v99m-4gvg")
    );
  }

  #[test]
  fn test_extract_ghsa_id_is_case_insensitive() {
    // `--ignore` matching is case-insensitive, so recognition must be too.
    let item = advisory(serde_json::json!({
      "url": "https://github.com/advisories/ghsa-mh99-v99m-4gvg",
      "title": "t",
      "severity": "high",
      "vulnerable_versions": "<1.0.0"
    }));
    assert_eq!(
      extract_ghsa_id(&item).as_deref(),
      Some("ghsa-mh99-v99m-4gvg")
    );
  }

  #[test]
  fn test_extract_ghsa_id_none_for_non_github_url() {
    let item = advisory(serde_json::json!({
      "url": "https://example.com/vuln/101010",
      "title": "t",
      "severity": "high",
      "vulnerable_versions": "<1.0.0"
    }));
    assert_eq!(extract_ghsa_id(&item), None);
  }

  #[test]
  fn test_extract_ghsa_id_tolerates_trailing_slash_and_query() {
    for url in [
      "https://github.com/advisories/GHSA-mh99-v99m-4gvg/",
      "https://github.com/advisories/GHSA-mh99-v99m-4gvg?utm=1",
      "https://github.com/advisories/GHSA-mh99-v99m-4gvg#summary",
      // Trailing slash *and* a query -- the query must be stripped first.
      "https://github.com/advisories/GHSA-mh99-v99m-4gvg/?utm=1",
    ] {
      let item = advisory(serde_json::json!({
        "url": url,
        "title": "t",
        "severity": "high",
        "vulnerable_versions": "<1.0.0"
      }));
      assert_eq!(
        extract_ghsa_id(&item).as_deref(),
        Some("GHSA-mh99-v99m-4gvg"),
        "failed for {url}"
      );
    }
  }

  fn versions(vs: &[&str]) -> Vec<deno_semver::Version> {
    vs.iter()
      .map(|v| deno_semver::Version::parse_standard(v).unwrap())
      .collect()
  }

  #[test]
  fn test_derive_patched_from_vulnerable() {
    let installed = versions(&["1.0.0"]);
    assert_eq!(
      derive_patched_from_vulnerable("<1.1.0", &installed).as_deref(),
      Some(">=1.1.0")
    );
    assert_eq!(
      derive_patched_from_vulnerable(">=4.0.0 <4.17.21", &versions(&["4.5.0"]))
        .as_deref(),
      Some(">=4.17.21")
    );
    // Operator separated from the version by whitespace.
    assert_eq!(
      derive_patched_from_vulnerable("< 1.1.0", &installed).as_deref(),
      Some(">=1.1.0")
    );
    // Inclusive upper bound and open-ended ranges cannot be resolved.
    assert_eq!(derive_patched_from_vulnerable("<=1.1.0", &installed), None);
    assert_eq!(derive_patched_from_vulnerable(">=1.0.0", &installed), None);
    assert_eq!(derive_patched_from_vulnerable("*", &installed), None);
    // No installed version is vulnerable -> nothing to fix.
    assert_eq!(
      derive_patched_from_vulnerable("<1.1.0", &versions(&["2.0.0"])),
      None
    );
  }

  #[test]
  fn test_derive_patched_from_vulnerable_disjoint_ranges() {
    let range = ">=4.0.0 <4.17.21 || >=5.0.0 <5.0.3";
    // The fix depends on which alternative the installed version falls in.
    assert_eq!(
      derive_patched_from_vulnerable(range, &versions(&["4.17.0"])).as_deref(),
      Some(">=4.17.21")
    );
    assert_eq!(
      derive_patched_from_vulnerable(range, &versions(&["5.0.1"])).as_deref(),
      Some(">=5.0.3")
    );
    // Installed versions in different branches disagree -> fail closed.
    assert_eq!(
      derive_patched_from_vulnerable(range, &versions(&["4.17.0", "5.0.1"])),
      None
    );
    // A branch with no exclusive upper bound has no known fix -> fail closed.
    assert_eq!(
      derive_patched_from_vulnerable(
        ">=1.0.0 <1.1.0 || >=2.0.0",
        &versions(&["2.5.0"])
      ),
      None
    );
    // Overlapping alternatives that disagree on the fixed version have no
    // single answer -> fail closed rather than taking whichever comes first.
    assert_eq!(
      derive_patched_from_vulnerable("<1.5.0 || <2.0.0", &versions(&["1.0.0"])),
      None
    );
    // Overlapping alternatives that agree are still resolvable.
    assert_eq!(
      derive_patched_from_vulnerable(
        "<1.5.0 || >=1.0.0 <1.5.0",
        &versions(&["1.0.0"])
      )
      .as_deref(),
      Some(">=1.5.0")
    );
  }

  use super::print_fix_summary;

  #[test]
  fn print_fix_summary_escapes_external_text() {
    let fixed = vec!["pkg\nname ^1 -> ^2".to_string()];
    let unfixable = vec!["pkg\u{200b}name (transitive dependency)".to_string()];
    let mut output = Vec::new();

    print_fix_summary(&mut output, &fixed, &unfixable);

    let output = String::from_utf8(output).unwrap();
    assert!(output.contains(r"pkg\nname ^1 -> ^2"));
    assert!(output.contains(r"pkg\u{200b}name (transitive dependency)"));
    assert!(!output.contains("pkg\nname"));
    assert!(!output.contains('\u{200b}'));
  }

  #[test]
  fn test_bulk_audit_response_deserialize_empty() {
    let json = r#"{}"#;
    let response: BulkAuditResponse = serde_json::from_str(json).unwrap();
    assert!(response.is_empty());
  }

  #[test]
  fn test_bulk_audit_response_deserialize_with_advisory() {
    let json = r#"{
      "@denotest/with-vuln1": [{
        "url": "https://example.com/vuln/101010",
        "title": "test vulnerability",
        "severity": "high",
        "vulnerable_versions": "<1.1.0"
      }]
    }"#;
    let response: BulkAuditResponse = serde_json::from_str(json).unwrap();
    assert_eq!(response.len(), 1);
    let advisories = &response["@denotest/with-vuln1"];
    assert_eq!(advisories.len(), 1);
    assert_eq!(advisories[0].severity, "high");
    assert!(advisories[0].patched_versions.is_none());
    assert!(advisories[0].cves.is_empty());
  }

  #[test]
  fn test_bulk_audit_response_deserialize_with_optional_fields() {
    let json = r#"{
      "test-pkg": [{
        "url": "https://example.com",
        "title": "test",
        "severity": "critical",
        "vulnerable_versions": "<2.0.0",
        "patched_versions": ">=2.0.0",
        "cves": ["CVE-2025-0001"],
        "cwe": ["CWE-1333"]
      }]
    }"#;
    let response: BulkAuditResponse = serde_json::from_str(json).unwrap();
    let advisories = &response["test-pkg"];
    assert_eq!(advisories[0].patched_versions.as_deref(), Some(">=2.0.0"));
    assert_eq!(advisories[0].cves, vec!["CVE-2025-0001"]);
    assert_eq!(advisories[0].cwe, vec!["CWE-1333"]);
  }
}
