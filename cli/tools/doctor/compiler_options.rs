// Copyright 2018-2026 the Deno authors. MIT license.

use deno_core::error::AnyError;
use jsonc_parser::cst::CstInputValue;

use super::ConfigFix;
use super::DoctorCheck;
use super::DoctorContext;
use super::Finding;
use super::FindingSeverity;

/// Compiler options that Deno currently defaults to a different value than
/// stock TypeScript when a project is configured via deno.json (see
/// `get_base_compiler_options_for_emit` in `deno_resolver`), and that users
/// are allowed to set in `compilerOptions`. Only options whose defaults
/// actually change type-checking semantics are pinned; the full internal
/// default set is deliberately not dumped into the user's config.
const PINNED_OPTIONS: &[PinnedOption] = &[
  PinnedOption {
    key: "strict",
    deno_default: true,
    tsc_default: false,
  },
  PinnedOption {
    key: "noImplicitOverride",
    deno_default: true,
    tsc_default: false,
  },
];

struct PinnedOption {
  key: &'static str,
  deno_default: bool,
  tsc_default: bool,
}

/// Reports implicit TypeScript compiler options that differ between Deno's
/// current defaults and stock TypeScript's defaults, which Deno 3's
/// TypeScript setup is based on. `--fix` writes today's implicit values
/// into `compilerOptions` so type checking behaves the same on both
/// versions.
pub struct CompilerOptionsCheck;

impl DoctorCheck for CompilerOptionsCheck {
  fn name(&self) -> &'static str {
    "compiler-options"
  }

  fn run(&self, ctx: &DoctorContext) -> Result<Vec<Finding>, AnyError> {
    // Pin at the workspace root: workspace members inherit root
    // compilerOptions and member-level values always take precedence, so a
    // root pin preserves current behavior for every member.
    let Some(root_deno_json) = ctx.workspace.root_deno_json() else {
      return Ok(Vec::new());
    };
    let existing_keys = root_deno_json
      .json
      .compiler_options
      .as_ref()
      .and_then(|v| v.as_object());

    let file = ctx.config_file_path(&root_deno_json.specifier)?;
    let file_display = ctx.display_path(&file);
    let mut findings = Vec::new();
    for option in PINNED_OPTIONS {
      let is_set =
        existing_keys.is_some_and(|keys| keys.contains_key(option.key));
      if is_set {
        continue;
      }
      findings.push(Finding {
        id: "compiler-options-not-pinned",
        severity: FindingSeverity::Warning,
        message: format!(
          "\"compilerOptions.{key}\" is not set in {file_display}. Deno currently type-checks deno.json projects with an implicit \"{key}\": {deno_default}, while stock TypeScript — which Deno 3's type checking is based on — defaults to {tsc_default}.",
          key = option.key,
          deno_default = option.deno_default,
          tsc_default = option.tsc_default,
        ),
        file: file.clone(),
        fix_description: Some(format!(
          "set \"compilerOptions.{}\": {} (pins Deno's current implicit default)",
          option.key, option.deno_default,
        )),
        fix: Some(ConfigFix::SetNestedKey {
          parent: "compilerOptions",
          key: option.key,
          value: CstInputValue::Bool(option.deno_default),
        }),
      });
    }
    Ok(findings)
  }
}
