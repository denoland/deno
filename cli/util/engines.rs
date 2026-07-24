// Copyright 2018-2026 the Deno authors. MIT license.

//! Shared handling for the `engines` field of `deno.json` and `package.json`.
//!
//! Both files use the same shape (`{ "deno": ">=2.0.0", "node": ">=18" }`) and
//! a mismatch is always a warning, never an error, matching `npm`/`yarn`.

use std::path::Path;

use deno_package_json::EngineMismatch;
use deno_package_json::EngineMismatchKind;
use deno_semver::Version;
use deno_semver::VersionReq;
use indexmap::IndexMap;

/// Check an `engines` map against the current runtime versions, returning one
/// entry per declared engine that is unsatisfied or has an unparseable
/// requirement.
///
/// Only the `node` and `deno` keys are checked; other entries (`npm`, `pnpm`,
/// ...) are ignored since Deno cannot reason about external tool versions.
/// This mirrors `deno_package_json::PackageJson::check_engines` so `deno.json`
/// and `package.json` behave identically.
pub fn check_engines(
  engines: &IndexMap<String, String>,
  deno_version: &str,
  node_version: &str,
) -> Vec<EngineMismatch> {
  let mut mismatches = Vec::new();
  for (engine, version_req) in engines {
    let actual = match engine.as_str() {
      "node" => node_version,
      "deno" => deno_version,
      _ => continue,
    };
    let parsed_req = match VersionReq::parse_from_npm(version_req) {
      // `parse_from_npm` accepts bare dist-tags (e.g. "next"). Those don't
      // make sense for an engine constraint, so flag them as invalid instead
      // of letting `matches` panic later on.
      Ok(req) if req.tag().is_none() => req,
      _ => {
        mismatches.push(EngineMismatch {
          engine: engine.clone(),
          required: version_req.clone(),
          actual: actual.to_string(),
          kind: EngineMismatchKind::InvalidRequirement,
        });
        continue;
      }
    };
    let parsed_actual =
      match Version::parse_from_npm(actual.strip_prefix('v').unwrap_or(actual))
      {
        Ok(v) => v,
        Err(_) => continue,
      };
    if !parsed_req.matches(&parsed_actual) {
      mismatches.push(EngineMismatch {
        engine: engine.clone(),
        required: version_req.clone(),
        actual: actual.to_string(),
        kind: EngineMismatchKind::Unsatisfied,
      });
    }
  }
  mismatches
}

/// Emit a warning for each engine mismatch found in the config at `path`.
pub fn warn_engine_mismatches(path: &Path, mismatches: &[EngineMismatch]) {
  let display_path = path.display();
  for m in mismatches {
    let warning_label = deno_terminal::colors::yellow("Warning");
    let required = deno_terminal::colors::cyan(&m.required);
    let actual = deno_terminal::colors::cyan(&m.actual);
    match m.kind {
      EngineMismatchKind::Unsatisfied => {
        log::warn!(
          "{warning_label} {display_path}: engines.{engine} \"{required}\" is not satisfied by current {engine} version {actual}",
          engine = m.engine,
        );
      }
      EngineMismatchKind::InvalidRequirement => {
        log::warn!(
          "{warning_label} {display_path}: engines.{engine} value \"{required}\" is not a valid version range",
          engine = m.engine,
        );
      }
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn engines(pairs: &[(&str, &str)]) -> IndexMap<String, String> {
    pairs
      .iter()
      .map(|(k, v)| (k.to_string(), v.to_string()))
      .collect()
  }

  #[test]
  fn satisfied_requirements_produce_no_mismatch() {
    let engines = engines(&[("deno", ">=1.0.0"), ("node", ">=18.0.0")]);
    let mismatches = check_engines(&engines, "2.0.0", "20.0.0");
    assert!(mismatches.is_empty());
  }

  #[test]
  fn unsatisfied_requirement_is_reported() {
    let engines = engines(&[("deno", ">=9000.0.0")]);
    let mismatches = check_engines(&engines, "2.0.0", "20.0.0");
    assert_eq!(mismatches.len(), 1);
    assert_eq!(mismatches[0].engine, "deno");
    assert_eq!(mismatches[0].actual, "2.0.0");
    assert_eq!(mismatches[0].kind, EngineMismatchKind::Unsatisfied);
  }

  #[test]
  fn invalid_requirement_is_reported() {
    let engines = engines(&[("deno", "totally-not-a-range")]);
    let mismatches = check_engines(&engines, "2.0.0", "20.0.0");
    assert_eq!(mismatches.len(), 1);
    assert_eq!(mismatches[0].kind, EngineMismatchKind::InvalidRequirement);
  }

  #[test]
  fn non_runtime_engines_are_ignored() {
    let engines = engines(&[("npm", ">=99"), ("pnpm", ">=99")]);
    let mismatches = check_engines(&engines, "2.0.0", "20.0.0");
    assert!(mismatches.is_empty());
  }

  #[test]
  fn node_version_is_checked() {
    let engines = engines(&[("node", ">=9000.0.0")]);
    let mismatches = check_engines(&engines, "2.0.0", "20.0.0");
    assert_eq!(mismatches.len(), 1);
    assert_eq!(mismatches[0].engine, "node");
    assert_eq!(mismatches[0].actual, "20.0.0");
  }
}
