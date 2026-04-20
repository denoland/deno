// Copyright 2018-2026 the Deno authors. MIT license.

use std::borrow::Cow;
use std::collections::HashSet;
use std::ffi::OsString;
use std::path::Path;
use std::path::PathBuf;
use std::str::FromStr;

use color_print::cstr;
// Re-export all flag types from the parser crate. This is the single source
// of truth for flag type definitions.
pub use deno_cli_parser::flags::*;
use deno_config::glob::FilePatterns;
use deno_config::glob::PathOrPatternSet;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_graph::GraphKind;
use deno_lib::version::DENO_VERSION_INFO;
use deno_npm::NpmSystemInfo;
use deno_path_util::normalize_path;
use deno_path_util::resolve_url_or_path;
use deno_path_util::url_to_file_path;
use deno_semver::jsr::JsrDepPackageReq;
use deno_telemetry::OtelConfig;
use deno_telemetry::OtelConsoleConfig;
use deno_telemetry::OtelPropagators;
use node_shim::parse_node_options_env_var;

use crate::util::env::resolve_cwd;
use crate::util::fs::canonicalize_path;

// ============================================================
// Error type for flag parsing (replaces clap::Error)
// ============================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlagsErrorKind {
  DisplayVersion,
  MissingRequiredArgument,
  InvalidValue,
  ArgumentConflict,
  Other,
}

#[derive(Debug)]
pub struct FlagsError {
  kind: FlagsErrorKind,
  message: String,
}

impl FlagsError {
  pub fn new(kind: FlagsErrorKind, message: impl Into<String>) -> Self {
    Self {
      kind,
      message: message.into(),
    }
  }

  pub fn kind(&self) -> FlagsErrorKind {
    self.kind
  }

}

impl std::fmt::Display for FlagsError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{}", self.message)
  }
}

impl std::error::Error for FlagsError {}

// ============================================================
// Extension traits for flag types that need CLI-specific methods.
// The type definitions themselves are in `deno_cli_parser::flags`.
// ============================================================

pub trait FileFlagsExt {
  fn as_file_patterns(&self, base: &Path) -> Result<FilePatterns, AnyError>;
}

impl FileFlagsExt for FileFlags {
  fn as_file_patterns(&self, base: &Path) -> Result<FilePatterns, AnyError> {
    Ok(FilePatterns {
      include: if self.include.is_empty() {
        None
      } else {
        Some(PathOrPatternSet::from_include_relative_path_or_patterns(
          base,
          &self.include,
        )?)
      },
      exclude: PathOrPatternSet::from_exclude_relative_path_or_patterns(
        base,
        &self.ignore,
      )?,
      base: base.to_path_buf(),
    })
  }
}

pub trait CompileFlagsExt {
  fn resolve_target(&self) -> String;
}

impl CompileFlagsExt for CompileFlags {
  fn resolve_target(&self) -> String {
    self
      .target
      .clone()
      .unwrap_or_else(|| env!("TARGET").to_string())
  }
}

pub trait DenoSubcommandExt {
  fn npm_system_info(&self) -> NpmSystemInfo;
}

impl DenoSubcommandExt for DenoSubcommand {
  fn npm_system_info(&self) -> NpmSystemInfo {
    match self {
      DenoSubcommand::Compile(CompileFlags {
        target: Some(target),
        ..
      }) => {
        // the values of NpmSystemInfo align with the possible values for the
        // `arch` and `platform` fields of Node.js' `process` global:
        // https://nodejs.org/api/process.html
        match target.as_str() {
          "aarch64-apple-darwin" => NpmSystemInfo {
            os: "darwin".into(),
            cpu: "arm64".into(),
          },
          "aarch64-unknown-linux-gnu" => NpmSystemInfo {
            os: "linux".into(),
            cpu: "arm64".into(),
          },
          "x86_64-apple-darwin" => NpmSystemInfo {
            os: "darwin".into(),
            cpu: "x64".into(),
          },
          "x86_64-unknown-linux-gnu" => NpmSystemInfo {
            os: "linux".into(),
            cpu: "x64".into(),
          },
          "x86_64-pc-windows-msvc" => NpmSystemInfo {
            os: "win32".into(),
            cpu: "x64".into(),
          },
          value => {
            log::warn!(
              concat!(
                "Not implemented npm system info for target '{}'. Using current ",
                "system default. This may impact architecture specific dependencies."
              ),
              value,
            );
            NpmSystemInfo::default()
          }
        }
      }
      _ => {
        let arch = std::env::var_os("DENO_INSTALL_ARCH");
        if let Some(var) = arch.as_ref().and_then(|s| s.to_str()) {
          NpmSystemInfo::from_rust(std::env::consts::OS, var)
        } else {
          NpmSystemInfo::default()
        }
      }
    }
  }
}

pub trait TypeCheckModeExt {
  fn as_graph_kind(&self) -> GraphKind;
}

impl TypeCheckModeExt for TypeCheckMode {
  /// Gets the corresponding module `GraphKind` that should be created
  /// for the current `TypeCheckMode`.
  fn as_graph_kind(&self) -> GraphKind {
    match self.is_true() {
      true => GraphKind::All,
      false => GraphKind::CodeOnly,
    }
  }
}

/// Parse --inspect-publish-uid from a comma-separated string like "stderr,http".
pub fn parse_inspect_publish_uid(s: &str) -> Result<InspectPublishUid, String> {
  let mut result = InspectPublishUid {
    console: false,
    http: false,
  };
  for part in s.split(',') {
    let part = part.trim();
    match part {
      "stderr" => result.console = true,
      "http" => result.http = true,
      "" => {}
      _ => {
        return Err(format!(
          "--inspect-publish-uid destination can be stderr or http, got '{}'",
          part
        ));
      }
    }
  }
  Ok(result)
}

fn join_paths(allowlist: &[String], d: &str) -> String {
  allowlist
    .iter()
    .map(|path| path.to_string())
    .collect::<Vec<String>>()
    .join(d)
}

pub trait FlagsExt {
  fn to_permission_args(&self) -> Vec<String>;
  fn no_legacy_abort(&self) -> bool;
  fn otel_config(&self) -> OtelConfig;
  fn config_path_args(&self, current_dir: &Path) -> Option<Vec<PathBuf>>;
  fn resolve_watch_exclude_set(&self) -> Result<PathOrPatternSet, AnyError>;
}

impl FlagsExt for Flags {
  /// Return list of permission arguments that are equivalent
  /// to the ones used to create `self`.
  fn to_permission_args(&self) -> Vec<String> {
    let mut args = vec![];

    if self.permissions.allow_all {
      args.push("--allow-all".to_string());
      return args;
    }

    match &self.permissions.allow_read {
      Some(read_allowlist) if read_allowlist.is_empty() => {
        args.push("--allow-read".to_string());
      }
      Some(read_allowlist) => {
        let s = format!("--allow-read={}", join_paths(read_allowlist, ","));
        args.push(s);
      }
      _ => {}
    }

    match &self.permissions.deny_read {
      Some(read_denylist) if read_denylist.is_empty() => {
        args.push("--deny-read".to_string());
      }
      Some(read_denylist) => {
        let s = format!("--deny-read={}", join_paths(read_denylist, ","));
        args.push(s);
      }
      _ => {}
    }

    match &self.permissions.allow_write {
      Some(write_allowlist) if write_allowlist.is_empty() => {
        args.push("--allow-write".to_string());
      }
      Some(write_allowlist) => {
        let s = format!("--allow-write={}", join_paths(write_allowlist, ","));
        args.push(s);
      }
      _ => {}
    }

    match &self.permissions.deny_write {
      Some(write_denylist) if write_denylist.is_empty() => {
        args.push("--deny-write".to_string());
      }
      Some(write_denylist) => {
        let s = format!("--deny-write={}", join_paths(write_denylist, ","));
        args.push(s);
      }
      _ => {}
    }

    match &self.permissions.allow_net {
      Some(net_allowlist) if net_allowlist.is_empty() => {
        args.push("--allow-net".to_string());
      }
      Some(net_allowlist) => {
        let s = format!("--allow-net={}", net_allowlist.join(","));
        args.push(s);
      }
      _ => {}
    }

    match &self.permissions.deny_net {
      Some(net_denylist) if net_denylist.is_empty() => {
        args.push("--deny-net".to_string());
      }
      Some(net_denylist) => {
        let s = format!("--deny-net={}", net_denylist.join(","));
        args.push(s);
      }
      _ => {}
    }

    match &self.unsafely_ignore_certificate_errors {
      Some(ic_allowlist) if ic_allowlist.is_empty() => {
        args.push("--unsafely-ignore-certificate-errors".to_string());
      }
      Some(ic_allowlist) => {
        let s = format!(
          "--unsafely-ignore-certificate-errors={}",
          ic_allowlist.join(",")
        );
        args.push(s);
      }
      _ => {}
    }

    match &self.permissions.allow_env {
      Some(env_allowlist) if env_allowlist.is_empty() => {
        args.push("--allow-env".to_string());
      }
      Some(env_allowlist) => {
        let s = format!("--allow-env={}", env_allowlist.join(","));
        args.push(s);
      }
      _ => {}
    }

    match &self.permissions.deny_env {
      Some(env_denylist) if env_denylist.is_empty() => {
        args.push("--deny-env".to_string());
      }
      Some(env_denylist) => {
        let s = format!("--deny-env={}", env_denylist.join(","));
        args.push(s);
      }
      _ => {}
    }

    match &self.permissions.ignore_env {
      Some(ignorelist) if ignorelist.is_empty() => {
        args.push("--ignore-env".to_string());
      }
      Some(ignorelist) => {
        let s = format!("--ignore-env={}", ignorelist.join(","));
        args.push(s);
      }
      _ => {}
    }

    match &self.permissions.ignore_read {
      Some(ignorelist) if ignorelist.is_empty() => {
        args.push("--ignore-read".to_string());
      }
      Some(ignorelist) => {
        let s = format!("--ignore-read={}", ignorelist.join(","));
        args.push(s);
      }
      _ => {}
    }

    match &self.permissions.allow_run {
      Some(run_allowlist) if run_allowlist.is_empty() => {
        args.push("--allow-run".to_string());
      }
      Some(run_allowlist) => {
        let s = format!("--allow-run={}", run_allowlist.join(","));
        args.push(s);
      }
      _ => {}
    }

    match &self.permissions.deny_run {
      Some(run_denylist) if run_denylist.is_empty() => {
        args.push("--deny-run".to_string());
      }
      Some(run_denylist) => {
        let s = format!("--deny-run={}", run_denylist.join(","));
        args.push(s);
      }
      _ => {}
    }

    match &self.permissions.allow_sys {
      Some(sys_allowlist) if sys_allowlist.is_empty() => {
        args.push("--allow-sys".to_string());
      }
      Some(sys_allowlist) => {
        let s = format!("--allow-sys={}", sys_allowlist.join(","));
        args.push(s)
      }
      _ => {}
    }

    match &self.permissions.deny_sys {
      Some(sys_denylist) if sys_denylist.is_empty() => {
        args.push("--deny-sys".to_string());
      }
      Some(sys_denylist) => {
        let s = format!("--deny-sys={}", sys_denylist.join(","));
        args.push(s)
      }
      _ => {}
    }

    match &self.permissions.allow_ffi {
      Some(ffi_allowlist) if ffi_allowlist.is_empty() => {
        args.push("--allow-ffi".to_string());
      }
      Some(ffi_allowlist) => {
        let s = format!("--allow-ffi={}", join_paths(ffi_allowlist, ","));
        args.push(s);
      }
      _ => {}
    }

    match &self.permissions.deny_ffi {
      Some(ffi_denylist) if ffi_denylist.is_empty() => {
        args.push("--deny-ffi".to_string());
      }
      Some(ffi_denylist) => {
        let s = format!("--deny-ffi={}", join_paths(ffi_denylist, ","));
        args.push(s);
      }
      _ => {}
    }

    match &self.permissions.allow_import {
      Some(allowlist) if allowlist.is_empty() => {
        args.push("--allow-import".to_string());
      }
      Some(allowlist) => {
        let s = format!("--allow-import={}", allowlist.join(","));
        args.push(s);
      }
      _ => {}
    }

    match &self.permissions.deny_import {
      Some(denylist) if denylist.is_empty() => {
        args.push("--deny-import".to_string());
      }
      Some(denylist) => {
        let s = format!("--deny-import={}", denylist.join(","));
        args.push(s);
      }
      _ => {}
    }

    args
  }

  fn no_legacy_abort(&self) -> bool {
    self
      .unstable_config
      .features
      .contains(&String::from("no-legacy-abort"))
  }

  fn otel_config(&self) -> OtelConfig {
    let otel_var = |name| match std::env::var(name) {
      Ok(s) if s.eq_ignore_ascii_case("true") => Some(true),
      Ok(s) if s.eq_ignore_ascii_case("false") => Some(false),
      Ok(_) => {
        log::warn!(
          "'{name}' env var value not recognized, only 'true' and 'false' are accepted"
        );
        None
      }
      Err(_) => None,
    };

    let disabled = otel_var("OTEL_SDK_DISABLED").unwrap_or(false);
    let default = !disabled && otel_var("OTEL_DENO").unwrap_or(false);

    let propagators = if default {
      if let Ok(propagators) = std::env::var("OTEL_PROPAGATORS") {
        propagators
          .split(',')
          .filter_map(|p| match p.trim() {
            "tracecontext" => Some(OtelPropagators::TraceContext),
            "baggage" => Some(OtelPropagators::Baggage),
            _ => None,
          })
          .collect()
      } else {
        HashSet::from([OtelPropagators::TraceContext, OtelPropagators::Baggage])
      }
    } else {
      HashSet::default()
    };

    OtelConfig {
      tracing_enabled: !disabled
        && otel_var("OTEL_DENO_TRACING").unwrap_or(default),
      metrics_enabled: !disabled
        && otel_var("OTEL_DENO_METRICS").unwrap_or(default),
      propagators,
      console: match std::env::var("OTEL_DENO_CONSOLE").as_deref() {
        Ok(_) if disabled => OtelConsoleConfig::Ignore,
        Ok("ignore") => OtelConsoleConfig::Ignore,
        Ok("capture") => OtelConsoleConfig::Capture,
        Ok("replace") => OtelConsoleConfig::Replace,
        res => {
          if res.is_ok() {
            log::warn!("'OTEL_DENO_CONSOLE' env var value not recognized, only 'ignore', 'capture', or 'replace' are accepted");
          }
          if default {
            OtelConsoleConfig::Capture
          } else {
            OtelConsoleConfig::Ignore
          }
        }
      },
      deterministic_prefix: std::env::var("DENO_UNSTABLE_OTEL_DETERMINISTIC")
        .as_deref()
        .map(u8::from_str)
        .map(|x| match x {
          Ok(x) => Some(x),
          Err(_) => {
            log::warn!("'DENO_UNSTABLE_OTEL_DETERMINISTIC' env var value not recognized, only integers are accepted");
            None
          }
        })
        .ok()
        .flatten(),
    }
  }

  /// Extract the paths the config file should be discovered from.
  ///
  /// Returns `None` if the config file should not be auto-discovered.
  fn config_path_args(&self, current_dir: &Path) -> Option<Vec<PathBuf>> {
    fn resolve_multiple_files(
      files_or_dirs: &[String],
      current_dir: &Path,
    ) -> Vec<PathBuf> {
      let mut seen = HashSet::with_capacity(files_or_dirs.len());
      let result = files_or_dirs
        .iter()
        .filter_map(|p| {
          let path = normalize_path(Cow::Owned(current_dir.join(p)));
          if seen.insert(path.clone()) {
            Some(path.into_owned())
          } else {
            None
          }
        })
        .collect::<Vec<_>>();
      if result.is_empty() {
        vec![current_dir.to_path_buf()]
      } else {
        result
      }
    }

    fn resolve_single_folder_path(
      arg: &str,
      current_dir: &Path,
      maybe_resolve_directory: impl FnOnce(PathBuf) -> Option<PathBuf>,
    ) -> Option<PathBuf> {
      if let Ok(module_specifier) = resolve_url_or_path(arg, current_dir) {
        if module_specifier.scheme() == "file"
          || module_specifier.scheme() == "npm"
        {
          if let Ok(p) = url_to_file_path(&module_specifier) {
            maybe_resolve_directory(p)
          } else {
            Some(current_dir.to_path_buf())
          }
        } else {
          // When the entrypoint doesn't have file: scheme (it's the remote
          // script), then we don't auto discover the config file.
          None
        }
      } else {
        Some(current_dir.to_path_buf())
      }
    }

    use DenoSubcommand::*;
    match &self.subcommand {
      Fmt(FmtFlags { files, .. }) => {
        Some(resolve_multiple_files(&files.include, current_dir))
      }
      Lint(LintFlags { files, .. }) => {
        Some(resolve_multiple_files(&files.include, current_dir))
      }
      Run(RunFlags { script, .. })
      | Compile(CompileFlags {
        source_file: script,
        ..
      }) => resolve_single_folder_path(script, current_dir, |mut p| {
        if p.pop() { Some(p) } else { None }
      })
      .map(|p| vec![p]),
      Task(TaskFlags {
        cwd: Some(path), ..
      }) => {
        // todo(dsherret): Why is this canonicalized? Document why.
        // attempt to resolve the config file from the task subcommand's
        // `--cwd` when specified
        match canonicalize_path(Path::new(path)) {
          Ok(path) => Some(vec![path]),
          Err(_) => Some(vec![current_dir.to_path_buf()]),
        }
      }
      Cache(CacheFlags { files, .. })
      | Install(InstallFlags::Local(InstallFlagsLocal::Entrypoints(
        InstallEntrypointsFlags {
          entrypoints: files, ..
        },
      ))) => Some(vec![
        files
          .iter()
          .filter_map(|file| {
            resolve_single_folder_path(file, current_dir, |mut p| {
              if p.is_dir() {
                return Some(p);
              }
              if p.pop() { Some(p) } else { None }
            })
          })
          .next()
          .unwrap_or_else(|| current_dir.to_path_buf()),
      ]),
      _ => Some(vec![current_dir.to_path_buf()]),
    }
  }

  fn resolve_watch_exclude_set(&self) -> Result<PathOrPatternSet, AnyError> {
    match self.subcommand.watch_flags() {
      Some(WatchFlagsRef::WithPaths(WatchFlagsWithPaths {
        exclude: excluded_paths,
        ..
      }))
      | Some(WatchFlagsRef::Watch(WatchFlags {
        exclude: excluded_paths,
        ..
      })) => {
        let cwd = resolve_cwd(self.initial_cwd.as_deref())?;
        PathOrPatternSet::from_exclude_relative_path_or_patterns(
          &cwd,
          excluded_paths,
        )
        .context("Failed resolving watch exclude patterns.")
      }
      _ => Ok(PathOrPatternSet::default()),
    }
  }
}

pub fn flags_from_vec(args: Vec<OsString>) -> Result<Flags, FlagsError> {
  flags_from_vec_with_initial_cwd(args, None)
}

// (Conversion layer removed — the parser crate now produces the real types.)

/// Helper to create flags errors from our validation code.
fn make_flags_error(
  kind: FlagsErrorKind,
  message: impl std::fmt::Display,
) -> FlagsError {
  FlagsError::new(kind, format!("{message}\n"))
}

/// Validate converted flags and return an error if there are conflicts or
/// invalid values that the custom parser doesn't enforce. This replaces
/// clap's built-in conflict/requires validation.
fn validate_parser_flags(
  flags: &mut Flags,
  string_args: &[String],
) -> Result<(), FlagsError> {
  let has_arg = |name: &str| {
    string_args
      .iter()
      .any(|a| a == name || a.starts_with(&format!("{name}=")))
  };

  // --no-check and --check conflict
  if has_arg("--no-check") && has_arg("--check") {
    return Err(make_flags_error(
      FlagsErrorKind::ArgumentConflict,
      "error: the argument '--no-check' cannot be used with '--check'",
    ));
  }

  // --config and --no-config conflict
  if has_arg("--no-config") && (has_arg("--config") || has_arg("-c")) {
    return Err(make_flags_error(
      FlagsErrorKind::ArgumentConflict,
      "error: the argument '--no-config' cannot be used with '--config <FILE>'",
    ));
  }

  // --hmr/--watch-hmr and --watch conflict
  if (has_arg("--hmr") || has_arg("--watch-hmr") || has_arg("--unstable-hmr"))
    && has_arg("--watch")
  {
    return Err(make_flags_error(
      FlagsErrorKind::ArgumentConflict,
      "error: the argument '--watch-hmr' cannot be used with '--watch'",
    ));
  }

  // Validate --location URL scheme
  if let Some(ref url) = flags.location
    && !["http", "https"].contains(&url.scheme())
  {
    return Err(make_flags_error(
      FlagsErrorKind::InvalidValue,
      "error: invalid value for '--location <HREF>': Expected protocol \"http\" or \"https\"",
    ));
  }

  // Subcommand-specific validations
  match &flags.subcommand {
    DenoSubcommand::Add(add_flags) => {
      if add_flags.packages.is_empty() {
        return Err(make_flags_error(
          FlagsErrorKind::MissingRequiredArgument,
          "error: one or more packages are required",
        ));
      }
    }
    DenoSubcommand::Remove(remove_flags) => {
      if remove_flags.packages.is_empty() {
        return Err(make_flags_error(
          FlagsErrorKind::MissingRequiredArgument,
          "error: one or more packages are required",
        ));
      }
    }
    DenoSubcommand::Install(InstallFlags::Local(InstallFlagsLocal::Add(
      add_flags,
    ))) => {
      if add_flags.packages.is_empty() {
        return Err(make_flags_error(
          FlagsErrorKind::MissingRequiredArgument,
          "error: one or more packages are required",
        ));
      }
      if has_arg("--no-config") {
        return Err(make_flags_error(
          FlagsErrorKind::ArgumentConflict,
          "error: deno install can't be used to add packages if `--no-config` is passed.\nhint: to cache the packages without adding to a config, pass the `--entrypoint` flag\n\nUsage: deno install [OPTIONS] [PACKAGE]...",
        ));
      }
    }
    DenoSubcommand::Uninstall(uninstall_flags) => match &uninstall_flags.kind {
      UninstallKind::Local(remove_flags) => {
        if remove_flags.packages.is_empty() {
          return Err(make_flags_error(
            FlagsErrorKind::MissingRequiredArgument,
            "error: one or more packages are required",
          ));
        }
      }
      UninstallKind::Global(g) => {
        if g.name.is_empty() {
          return Err(make_flags_error(
            FlagsErrorKind::MissingRequiredArgument,
            "error: package name is required",
          ));
        }
      }
    },
    DenoSubcommand::Check(check_flags) => {
      // --doc and --doc-only are mutually exclusive
      if check_flags.doc && check_flags.doc_only {
        return Err(make_flags_error(
          FlagsErrorKind::ArgumentConflict,
          "error: the argument '--doc' cannot be used with '--doc-only'",
        ));
      }
      // --all/--remote conflicts with --no-remote
      if flags.type_check_mode == TypeCheckMode::All && flags.no_remote {
        return Err(make_flags_error(
          FlagsErrorKind::ArgumentConflict,
          "error: the argument '--all' cannot be used with '--no-remote'",
        ));
      }
    }
    DenoSubcommand::Test(_test_flags) => {
      // --fail-fast=0 is invalid (NonZeroUsize)
      if has_arg("--fail-fast=0") {
        return Err(make_flags_error(
          FlagsErrorKind::InvalidValue,
          "error: invalid value '0' for '--fail-fast <N>': 0 is not allowed",
        ));
      }
    }
    DenoSubcommand::Upgrade(upgrade_flags) => {
      // --rc and --canary conflict
      if upgrade_flags.release_candidate && upgrade_flags.canary {
        return Err(make_flags_error(
          FlagsErrorKind::ArgumentConflict,
          "error: the argument '--rc' cannot be used with '--canary'",
        ));
      }
      // --rc and --version conflict
      if upgrade_flags.release_candidate && has_arg("--version") {
        return Err(make_flags_error(
          FlagsErrorKind::ArgumentConflict,
          "error: the argument '--rc' cannot be used with '--version'",
        ));
      }
    }
    DenoSubcommand::Fmt(fmt_flags) => {
      // --ext requires files
      if flags.ext.is_some() && fmt_flags.files.include.is_empty() {
        return Err(make_flags_error(
          FlagsErrorKind::MissingRequiredArgument,
          "error: the following required arguments were not provided:\n  <files>...\n\ntip: '--ext' requires files to be specified",
        ));
      }
      // --fail-fast requires --check
      if fmt_flags.fail_fast && !fmt_flags.check {
        return Err(make_flags_error(
          FlagsErrorKind::MissingRequiredArgument,
          "error: the following required arguments were not provided:\n  --check\n\ntip: '--fail-fast' requires '--check'",
        ));
      }
    }
    DenoSubcommand::Doc(doc_flags) => {
      // --html requires source files
      if doc_flags.html.is_some()
        && !matches!(
          &doc_flags.source_files,
          DocSourceFileFlag::Paths(paths) if !paths.is_empty()
        )
      {
        return Err(make_flags_error(
          FlagsErrorKind::MissingRequiredArgument,
          "error: the following required arguments were not provided:\n  <source_file>\n\ntip: '--html' requires source files to be specified",
        ));
      }
      // --html requires --output
      if let Some(ref html) = doc_flags.html
        && html.output.is_empty()
      {
        return Err(make_flags_error(
          FlagsErrorKind::MissingRequiredArgument,
          "error: the following required arguments were not provided:\n  --output <path>",
        ));
      }
      // --lint requires source files
      if doc_flags.lint {
        let has_files = match &doc_flags.source_files {
          DocSourceFileFlag::Paths(paths) => !paths.is_empty(),
          DocSourceFileFlag::Builtin => false,
        };
        if !has_files {
          return Err(make_flags_error(
            FlagsErrorKind::MissingRequiredArgument,
            "error: the following required arguments were not provided:\n  <source_file>\n\ntip: '--lint' requires source files to be specified",
          ));
        }
      }
    }
    DenoSubcommand::Init(init_flags) => {
      // Check if this came from the `create` subcommand
      let is_create = string_args.iter().skip(1).any(|a| a == "create");

      if is_create {
        // --jsr and --npm conflict
        if has_arg("--jsr") && has_arg("--npm") {
          return Err(make_flags_error(
            FlagsErrorKind::ArgumentConflict,
            "error: the argument '--jsr' cannot be used with '--npm'",
          ));
        }
        // --jsr with npm: specifier is contradictory
        if has_arg("--jsr")
          && let Some(ref pkg) = init_flags.package
          && pkg.starts_with("npm:")
        {
          return Err(make_flags_error(
            FlagsErrorKind::InvalidValue,
            "error: cannot use '--jsr' with an npm: specifier",
          ));
        }
        // --npm with jsr: specifier is contradictory
        if has_arg("--npm")
          && let Some(ref pkg) = init_flags.package
          && pkg.starts_with("jsr:")
        {
          return Err(make_flags_error(
            FlagsErrorKind::InvalidValue,
            "error: cannot use '--npm' with a jsr: specifier",
          ));
        }
        // `deno create` requires a package
        if init_flags.package.is_none()
          || init_flags.package.as_deref() == Some("")
        {
          return Err(make_flags_error(
            FlagsErrorKind::MissingRequiredArgument,
            "error: the following required arguments were not provided:\n  <PACKAGE>",
          ));
        }
        // Package must have npm: or jsr: prefix with a non-empty name
        if let Some(ref pkg) = init_flags.package {
          if !pkg.starts_with("npm:") && !pkg.starts_with("jsr:") {
            return Err(make_flags_error(
              FlagsErrorKind::InvalidValue,
              "Missing `jsr:` or `npm:` prefix. For example: `deno create npm:vite`.",
            ));
          }
          // Check for empty package name after prefix
          let name = pkg
            .strip_prefix("npm:")
            .or_else(|| pkg.strip_prefix("jsr:"))
            .unwrap_or(pkg);
          if name.is_empty() {
            return Err(make_flags_error(
              FlagsErrorKind::InvalidValue,
              "error: empty package name after prefix",
            ));
          }
        }
        // `deno create npm:vite my-project` (args without --) should error
        // The parser puts extra positional args into package_args, but the
        // clap definition uses .last(true) which only accepts after --.
        // Detect this: if we have package_args but the raw args don't contain "--"
        if !init_flags.package_args.is_empty() {
          let has_double_dash = string_args.iter().any(|a| a == "--");
          if !has_double_dash {
            return Err(make_flags_error(
              FlagsErrorKind::InvalidValue,
              "error: unexpected arguments after package name; use '--' to pass arguments to the create package",
            ));
          }
        }
      } else {
        // Regular `deno init` validations
        let has_jsr = has_arg("--jsr");
        let has_npm = has_arg("--npm");

        // --jsr conflicts with --npm, --lib, --serve, --empty
        if has_jsr && has_npm {
          return Err(make_flags_error(
            FlagsErrorKind::ArgumentConflict,
            "error: the argument '--jsr' cannot be used with '--npm'",
          ));
        }
        if has_jsr && init_flags.lib {
          return Err(make_flags_error(
            FlagsErrorKind::ArgumentConflict,
            "error: the argument '--jsr' cannot be used with '--lib'",
          ));
        }
        // --jsr without a package name
        if has_jsr && init_flags.package.is_none() {
          return Err(make_flags_error(
            FlagsErrorKind::MissingRequiredArgument,
            "error: '--jsr' requires a package name",
          ));
        }

        // --lib/--serve conflict with --npm
        if init_flags.package.is_some() && has_npm {
          if init_flags.lib {
            return Err(make_flags_error(
              FlagsErrorKind::ArgumentConflict,
              "error: the argument '--lib' cannot be used with '--npm'",
            ));
          }
          if init_flags.serve {
            return Err(make_flags_error(
              FlagsErrorKind::ArgumentConflict,
              "error: the argument '--serve' cannot be used with '--npm'",
            ));
          }
        }
      }
    }
    DenoSubcommand::Task(task_flags) => {
      // --eval requires a task expression
      if task_flags.eval && task_flags.task.is_none() {
        return Err(make_flags_error(
          FlagsErrorKind::MissingRequiredArgument,
          "error: [TASK] must be specified when using --eval\n\nUsage: deno task [OPTIONS] [TASK]",
        ));
      }
    }
    DenoSubcommand::Jupyter(jupyter_flags) => {
      // --install and --conn conflict
      if jupyter_flags.install && jupyter_flags.conn_file.is_some() {
        return Err(make_flags_error(
          FlagsErrorKind::ArgumentConflict,
          "error: the argument '--install' cannot be used with '--conn'",
        ));
      }
      // --install and --kernel conflict
      if jupyter_flags.install && jupyter_flags.kernel {
        return Err(make_flags_error(
          FlagsErrorKind::ArgumentConflict,
          "error: the argument '--install' cannot be used with '--kernel'",
        ));
      }
      // --kernel requires --conn
      if jupyter_flags.kernel && jupyter_flags.conn_file.is_none() {
        return Err(make_flags_error(
          FlagsErrorKind::MissingRequiredArgument,
          "error: '--kernel' requires '--conn <FILE>'",
        ));
      }
      // --display requires --install (or --name implies install context)
      if jupyter_flags.display.is_some()
        && !jupyter_flags.install
        && !jupyter_flags.kernel
      {
        return Err(make_flags_error(
          FlagsErrorKind::ArgumentConflict,
          "error: '--display' requires '--install'",
        ));
      }
      // --kernel and --display conflict
      if jupyter_flags.kernel && jupyter_flags.display.is_some() {
        return Err(make_flags_error(
          FlagsErrorKind::ArgumentConflict,
          "error: the argument '--kernel' cannot be used with '--display'",
        ));
      }
      // --force requires --install
      if jupyter_flags.force && !jupyter_flags.install {
        return Err(make_flags_error(
          FlagsErrorKind::ArgumentConflict,
          "error: '--force' requires '--install'",
        ));
      }
    }
    DenoSubcommand::Clean(clean_flags) => {
      // When --except is used, set cached_only to true
      if !clean_flags.except_paths.is_empty() {
        flags.cached_only = true;
      }
    }
    _ => {}
  }

  // Validate --allow-scripts values
  validate_allow_scripts(flags, string_args)?;

  Ok(())
}

/// Validate --allow-scripts values for proper npm: prefix and no tags.
fn validate_allow_scripts(
  _flags: &Flags,
  string_args: &[String],
) -> Result<(), FlagsError> {
  // Find the raw --allow-scripts=... value from args
  for arg in string_args {
    if let Some(value) = arg.strip_prefix("--allow-scripts=") {
      // Parse each comma-separated package
      for pkg_str in value.split(',') {
        let pkg_str = pkg_str.trim();
        if pkg_str.is_empty() {
          continue;
        }
        if !pkg_str.starts_with("npm:") {
          return Err(make_flags_error(
            FlagsErrorKind::InvalidValue,
            format!(
              "Invalid package for --allow-scripts: '{}'. An 'npm:' specifier is required",
              pkg_str
            ),
          ));
        }
        // Check for tags (e.g., npm:foo@next)
        let dep = JsrDepPackageReq::from_str_loose(pkg_str)
          .map_err(|e| make_flags_error(FlagsErrorKind::InvalidValue, e))?;
        if dep.req.version_req.tag().is_some() {
          return Err(make_flags_error(
            FlagsErrorKind::InvalidValue,
            format!("Tags are not supported in --allow-scripts: {}", pkg_str),
          ));
        }
      }
    }
  }
  Ok(())
}

/// Handle the dx/denox/dnx shim by inserting "x" as the subcommand.
fn handle_dx_shim(args: Vec<OsString>) -> Vec<OsString> {
  if !args.is_empty()
    && (args[0].as_encoded_bytes().ends_with(b"dx")
      || args[0].as_encoded_bytes().ends_with(b"denox")
      || args[0].as_encoded_bytes().ends_with(b"dnx"))
  {
    let mut new_args = Vec::with_capacity(args.len() + 1);
    new_args.push(args[0].clone());
    new_args.push(OsString::from("x"));
    if args.len() >= 2 {
      new_args.extend(args.into_iter().skip(1));
    }
    new_args
  } else {
    args
  }
}

/// Main entry point for parsing deno's command line flags.
pub fn flags_from_vec_with_initial_cwd(
  args: Vec<OsString>,
  initial_cwd: Option<PathBuf>,
) -> Result<Flags, FlagsError> {
  // Handle dx/denox/dnx shim
  let args = handle_dx_shim(args);

  // Convert OsString args to String for our custom parser
  let string_args: Vec<String> = args
    .iter()
    .filter_map(|s| s.to_str())
    .map(|s| s.to_string())
    .collect();

  // Use the custom parser — it now produces the real Flags type directly.
  match deno_cli_parser::convert::flags_from_vec(string_args.clone()) {
    Ok(mut flags) => {
      if flags.initial_cwd.is_none() {
        flags.initial_cwd = initial_cwd.clone();
      }
      apply_node_options(&mut flags);

      // Run post-conversion validation
      validate_parser_flags(&mut flags, &string_args)?;

      Ok(flags)
    }
    Err(e) => {
      match e.kind {
        deno_cli_parser::CliErrorKind::DisplayVersion => Err(FlagsError::new(
          FlagsErrorKind::DisplayVersion,
          format!("deno {}\n", DENO_VERSION_INFO.deno),
        )),
        _ => {
          // Convert parser error to FlagsError
          Err(FlagsError::new(FlagsErrorKind::Other, e.to_string()))
        }
      }
    }
  }
}

pub fn did_you_mean<T, I>(v: &str, possible_values: I) -> Vec<String>
where
  T: AsRef<str>,
  I: IntoIterator<Item = T>,
{
  let mut candidates: Vec<(f64, String)> = possible_values
    .into_iter()
    // GH #4660: using `jaro` because `jaro_winkler` implementation in `strsim-rs` is wrong
    // causing strings with common prefix >=10 to be considered perfectly similar
    .map(|pv| (strsim::jaro(v, pv.as_ref()), pv.as_ref().to_owned()))
    // Confidence of 0.7 so that bar -> baz is suggested
    .filter(|(confidence, _)| *confidence > 0.8)
    .collect();
  candidates
    .sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
  candidates.into_iter().map(|(_, pv)| pv).collect()
}

/// Reads some flags from NODE_OPTIONS:
/// https://nodejs.org/api/cli.html#node_optionsoptions
/// Currently supports:
/// - `--require` / `-r`
/// - `--inspect-publish-uid`
fn apply_node_options(flags: &mut Flags) {
  let node_options = match std::env::var("NODE_OPTIONS") {
    Ok(val) if !val.is_empty() => val,
    _ => return,
  };

  let args = parse_node_options_env_var(&node_options).unwrap_or_default();

  let mut require_values: Vec<String> = Vec::new();
  let mut inspect_publish_uid_value: Option<String> = None;

  let mut iter = args.iter().peekable();
  while let Some(arg) = iter.next() {
    if arg == "--require" || arg == "-r" {
      if let Some(val) = iter.next() {
        require_values.push(val.clone());
      }
    } else if let Some(val) = arg.strip_prefix("--inspect-publish-uid=") {
      inspect_publish_uid_value = Some(val.to_string());
    }
  }

  if !require_values.is_empty() {
    require_values.append(&mut flags.require);
    flags.require = require_values;
  }

  if flags.inspect_publish_uid.is_none()
    && let Some(val) = inspect_publish_uid_value
    && let Ok(uid) = parse_inspect_publish_uid(&val)
  {
    flags.inspect_publish_uid = Some(uid);
  }
}

pub static UPGRADE_USAGE: &str = cstr!(
  "<g>Latest</>
  <bold>deno upgrade</>

<g>Specific version</>
  <bold>deno upgrade</> <p(245)>1.45.0</>
  <bold>deno upgrade</> <p(245)>1.46.0-rc.1</>
  <bold>deno upgrade</> <p(245)>9bc2dd29ad6ba334fd57a20114e367d3c04763d4</>

<g>Channel</>
  <bold>deno upgrade</> <p(245)>stable</>
  <bold>deno upgrade</> <p(245)>alpha</>
  <bold>deno upgrade</> <p(245)>beta</>
  <bold>deno upgrade</> <p(245)>rc</>
  <bold>deno upgrade</> <p(245)>canary</>

<g>From a pull request</> <p(245)>(requires gh CLI)</>
  <bold>deno upgrade</> <p(245)>pr 12345</>"
);

pub fn handle_shell_completion(_cwd: &Path) -> Result<(), AnyError> {
  let shell = std::env::var("COMPLETE").unwrap_or_default();
  let args: Vec<String> = std::env::args().collect();

  // Check if we're completing task names
  if is_completing_task(&args) {
    complete_task_names(&args, &shell)?;
    return Ok(());
  }

  deno_cli_parser::completions::try_complete(
    &deno_cli_parser::defs::DENO_ROOT,
    &args,
    &shell,
  );

  Ok(())
}

/// Check if the args indicate we're completing a task subcommand.
fn is_completing_task(args: &[String]) -> bool {
  args.iter().skip(1).any(|a| a == "task")
}

/// Generate task name completions by reading deno.json/package.json.
fn complete_task_names(args: &[String], shell: &str) -> Result<(), AnyError> {
  use std::io::Write;
  use std::sync::Arc;

  // Parse flags to pick up --config if specified
  let string_args: Vec<String> =
    args.iter().filter(|a| !a.is_empty()).cloned().collect();
  let flags =
    deno_cli_parser::convert::flags_from_vec(string_args).unwrap_or_default();
  let factory = crate::factory::CliFactory::from_flags(Arc::new(flags));
  let Ok(options) = factory.cli_options() else {
    return Ok(());
  };

  let member_dir = &options.start_dir;
  let Ok(tasks_config) = member_dir.to_tasks_config() else {
    return Ok(());
  };

  let mut tasks =
    crate::tools::task::get_available_tasks(member_dir, &tasks_config)
      .unwrap_or_default();
  tasks.sort_by(|a, b| a.name.cmp(&b.name));

  let stdout = std::io::stdout();
  let mut out = std::io::BufWriter::new(stdout.lock());

  for task in &tasks {
    let desc = task.task.description.as_deref().unwrap_or("");
    match shell {
      "fish" => {
        if desc.is_empty() {
          let _ = writeln!(out, "{}", task.name);
        } else {
          let _ = writeln!(out, "{}\t{}", task.name, desc);
        }
      }
      "zsh" => {
        if desc.is_empty() {
          let _ = writeln!(out, "{}", task.name);
        } else {
          let _ = writeln!(out, "{}:{}", task.name, desc);
        }
      }
      _ => {
        let _ = writeln!(out, "{}", task.name);
      }
    }
  }

  // Also add task subcommand flags
  let task_def = deno_cli_parser::defs::DENO_ROOT.find_subcommand("task");
  if let Some(task_def) = task_def {
    for arg in task_def.all_args() {
      if arg.hidden || arg.positional {
        continue;
      }
      if let Some(long) = arg.long {
        let flag = format!("--{long}");
        let help = arg.help;
        match shell {
          "fish" => {
            if help.is_empty() {
              let _ = writeln!(out, "{flag}");
            } else {
              let _ = writeln!(out, "{flag}\t{help}");
            }
          }
          "zsh" => {
            if help.is_empty() {
              let _ = writeln!(out, "{flag}");
            } else {
              let _ = writeln!(out, "{flag}:{help}");
            }
          }
          _ => {
            let _ = writeln!(out, "{flag}");
          }
        }
      }
    }
  }

  Ok(())
}
