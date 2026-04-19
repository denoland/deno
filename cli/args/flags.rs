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

  pub fn print(&self) -> std::io::Result<()> {
    use std::io::Write;
    let mut stderr = std::io::stderr().lock();
    write!(stderr, "{}", self.message)
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
          "error: '--eval' requires a task expression",
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

#[cfg(test)]
thread_local! {
  static TEST_NODE_OPTIONS: std::cell::RefCell<Option<String>> = const { std::cell::RefCell::new(None) };
}

/// Reads some flags from NODE_OPTIONS:
/// https://nodejs.org/api/cli.html#node_optionsoptions
/// Currently supports:
/// - `--require` / `-r`
/// - `--inspect-publish-uid`
fn apply_node_options(flags: &mut Flags) {
  let node_options = match std::env::var("NODE_OPTIONS") {
    Ok(val) if !val.is_empty() => val,
    _ => {
      #[cfg(test)]
      {
        match TEST_NODE_OPTIONS.with(|opt| opt.borrow().clone()) {
          Some(val) if !val.is_empty() => val,
          _ => return,
        }
      }
      #[cfg(not(test))]
      return;
    }
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

  deno_cli_parser::completions::try_complete(
    &deno_cli_parser::defs::DENO_ROOT,
    &args,
    &shell,
  );

  Ok(())
}

#[cfg(test)]
fn escape_and_split_commas(s: String) -> Result<Vec<String>, String> {
  let mut result = vec![];
  let mut current = String::new();
  let mut chars = s.chars();

  while let Some(c) = chars.next() {
    if c == ',' {
      if let Some(next) = chars.next() {
        if next == ',' {
          current.push(',');
        } else {
          if current.is_empty() {
            return Err("Empty values are not allowed".to_string());
          }

          result.push(current.clone());
          current.clear();
          current.push(next);
        }
      } else {
        return Err("Empty values are not allowed".to_string());
      }
    } else {
      current.push(c);
    }
  }

  if current.is_empty() {
    return Err("Empty values are not allowed".to_string());
  }

  result.push(current);

  Ok(result)
}

#[cfg(test)]
mod tests {
  use std::net::SocketAddr;
  use std::num::NonZeroU8;
  use std::num::NonZeroU32;
  use std::num::NonZeroUsize;

  use deno_semver::package::PackageReq;
  use pretty_assertions::assert_eq;

  use super::*;

  /// Creates vector of strings, Vec<String>
  macro_rules! svec {
    ($($x:expr),* $(,)?) => (vec![$($x.to_string().into()),*]);
  }

  #[test]
  fn global_flags() {
    #[rustfmt::skip]
    let r = flags_from_vec(svec!["deno", "--log-level", "debug", "--quiet", "run", "script.ts"]);

    let flags = r.unwrap();
    assert_eq!(
      flags,
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "script.ts".to_string()
        )),
        log_level: Some(Level::Error),
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
    #[rustfmt::skip]
    let r2 = flags_from_vec(svec!["deno", "run", "--log-level", "debug", "--quiet", "script.ts"]);
    let flags2 = r2.unwrap();
    assert_eq!(flags2, flags);
  }

  #[test]
  fn upgrade() {
    let r = flags_from_vec(svec!["deno", "upgrade", "--dry-run", "--force"]);
    let flags = r.unwrap();
    assert_eq!(
      flags,
      Flags {
        subcommand: DenoSubcommand::Upgrade(UpgradeFlags {
          force: true,
          dry_run: true,
          canary: false,
          release_candidate: false,
          version: None,
          output: None,
          version_or_hash_or_channel: None,
          checksum: None,
          pr: None,
          branch: None,
        }),
        ..Flags::default()
      }
    );
  }

  #[test]
  fn upgrade_with_output_flag() {
    let r = flags_from_vec(svec!["deno", "upgrade", "--output", "example.txt"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Upgrade(UpgradeFlags {
          force: false,
          dry_run: false,
          canary: false,
          release_candidate: false,
          version: None,
          output: Some(String::from("example.txt")),
          version_or_hash_or_channel: None,
          checksum: None,
          pr: None,
          branch: None,
        }),
        ..Flags::default()
      }
    );
  }

  #[test]
  fn version() {
    let r = flags_from_vec(svec!["deno", "--version"]);
    assert_eq!(r.unwrap_err().kind(), FlagsErrorKind::DisplayVersion);
    let r = flags_from_vec(svec!["deno", "-V"]);
    assert_eq!(r.unwrap_err().kind(), FlagsErrorKind::DisplayVersion);
  }

  #[test]
  fn run_reload() {
    let r = flags_from_vec(svec!["deno", "run", "-r", "script.ts"]);
    let flags = r.unwrap();
    assert_eq!(
      flags,
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "script.ts".to_string()
        )),
        reload: true,
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn run_watch() {
    let r = flags_from_vec(svec!["deno", "run", "--watch", "script.ts"]);
    let flags = r.unwrap();
    assert_eq!(
      flags,
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags {
          script: "script.ts".to_string(),
          watch: Some(WatchFlagsWithPaths {
            hmr: false,
            paths: vec![],
            no_clear_screen: false,
            exclude: vec![],
          }),
          bare: false,
          coverage_dir: None,
          print_task_list: false,
        }),
        code_cache_enabled: true,
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec![
      "deno",
      "--watch",
      "--no-clear-screen",
      "script.ts"
    ]);
    let flags = r.unwrap();
    assert_eq!(
      flags,
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags {
          script: "script.ts".to_string(),
          watch: Some(WatchFlagsWithPaths {
            hmr: false,
            paths: vec![],
            no_clear_screen: true,
            exclude: vec![],
          }),
          bare: true,
          coverage_dir: None,
          print_task_list: false,
        }),
        code_cache_enabled: true,
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec![
      "deno",
      "run",
      "--watch-hmr",
      "--no-clear-screen",
      "script.ts"
    ]);
    let flags = r.unwrap();
    assert_eq!(
      flags,
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags {
          script: "script.ts".to_string(),
          watch: Some(WatchFlagsWithPaths {
            hmr: true,
            paths: vec![],
            no_clear_screen: true,
            exclude: vec![],
          }),
          bare: false,
          coverage_dir: None,
          print_task_list: false,
        }),
        code_cache_enabled: true,
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec![
      "deno",
      "run",
      "--unstable-hmr",
      "--no-clear-screen",
      "script.ts"
    ]);
    let flags = r.unwrap();
    assert_eq!(
      flags,
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags {
          script: "script.ts".to_string(),
          watch: Some(WatchFlagsWithPaths {
            hmr: true,
            paths: vec![],
            no_clear_screen: true,
            exclude: vec![],
          }),
          bare: false,
          coverage_dir: None,
          print_task_list: false,
        }),
        code_cache_enabled: true,
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec![
      "deno",
      "run",
      "--watch-hmr=foo.txt",
      "--no-clear-screen",
      "script.ts"
    ]);
    let flags = r.unwrap();
    assert_eq!(
      flags,
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags {
          script: "script.ts".to_string(),
          watch: Some(WatchFlagsWithPaths {
            hmr: true,
            paths: vec![String::from("foo.txt")],
            no_clear_screen: true,
            exclude: vec![],
          }),
          bare: false,
          coverage_dir: None,
          print_task_list: false,
        }),
        code_cache_enabled: true,
        ..Flags::default()
      }
    );

    let r =
      flags_from_vec(svec!["deno", "run", "--hmr", "--watch", "script.ts"]);
    assert!(r.is_err());
  }

  #[test]
  fn run_watch_with_external() {
    let r = flags_from_vec(svec!["deno", "--watch=file1,file2", "script.ts"]);
    let flags = r.unwrap();
    assert_eq!(
      flags,
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags {
          script: "script.ts".to_string(),
          watch: Some(WatchFlagsWithPaths {
            hmr: false,
            paths: vec![String::from("file1"), String::from("file2")],
            no_clear_screen: false,
            exclude: vec![],
          }),
          bare: true,
          coverage_dir: None,
          print_task_list: false,
        }),
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn run_watch_with_no_clear_screen() {
    let r = flags_from_vec(svec![
      "deno",
      "run",
      "--watch",
      "--no-clear-screen",
      "script.ts"
    ]);

    let flags = r.unwrap();
    assert_eq!(
      flags,
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags {
          script: "script.ts".to_string(),
          watch: Some(WatchFlagsWithPaths {
            hmr: false,
            paths: vec![],
            no_clear_screen: true,
            exclude: vec![],
          }),
          bare: false,
          coverage_dir: None,
          print_task_list: false,
        }),
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn run_watch_with_excluded_paths() {
    let r = flags_from_vec(svec!(
      "deno",
      "--watch",
      "--watch-exclude=foo",
      "script.ts"
    ));

    let flags = r.unwrap();
    assert_eq!(
      flags,
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags {
          script: "script.ts".to_string(),
          watch: Some(WatchFlagsWithPaths {
            hmr: false,
            paths: vec![],
            no_clear_screen: false,
            exclude: vec![String::from("foo")],
          }),
          bare: true,
          coverage_dir: None,
          print_task_list: false,
        }),
        code_cache_enabled: true,
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec!(
      "deno",
      "run",
      "--watch=foo",
      "--watch-exclude=bar",
      "script.ts"
    ));
    let flags = r.unwrap();
    assert_eq!(
      flags,
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags {
          script: "script.ts".to_string(),
          watch: Some(WatchFlagsWithPaths {
            hmr: false,
            paths: vec![String::from("foo")],
            no_clear_screen: false,
            exclude: vec![String::from("bar")],
          }),
          bare: false,
          coverage_dir: None,
          print_task_list: false,
        }),
        code_cache_enabled: true,
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec![
      "deno",
      "run",
      "--watch",
      "--watch-exclude=foo,bar",
      "script.ts"
    ]);

    let flags = r.unwrap();
    assert_eq!(
      flags,
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags {
          script: "script.ts".to_string(),
          watch: Some(WatchFlagsWithPaths {
            hmr: false,
            paths: vec![],
            no_clear_screen: false,
            exclude: vec![String::from("foo"), String::from("bar")],
          }),
          bare: false,
          coverage_dir: None,
          print_task_list: false,
        }),
        code_cache_enabled: true,
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec![
      "deno",
      "--watch=foo,bar",
      "--watch-exclude=baz,qux",
      "script.ts"
    ]);

    let flags = r.unwrap();
    assert_eq!(
      flags,
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags {
          script: "script.ts".to_string(),
          watch: Some(WatchFlagsWithPaths {
            hmr: false,
            paths: vec![String::from("foo"), String::from("bar")],
            no_clear_screen: false,
            exclude: vec![String::from("baz"), String::from("qux"),],
          }),
          bare: true,
          coverage_dir: None,
          print_task_list: false,
        }),
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn run_reload_allow_write() {
    let r =
      flags_from_vec(svec!["deno", "run", "-r", "--allow-write", "script.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        reload: true,
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "script.ts".to_string()
        )),
        permissions: PermissionFlags {
          allow_write: Some(vec![]),
          ..Default::default()
        },
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn run_coverage() {
    let r = flags_from_vec(svec!["deno", "run", "--coverage=foo", "script.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags {
          script: "script.ts".to_string(),
          watch: None,
          bare: false,
          coverage_dir: Some("foo".to_string()),
          print_task_list: false,
        }),
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn run_v8_flags() {
    let r = flags_from_vec(svec!["deno", "run", "--v8-flags=--help"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default("_".to_string())),
        v8_flags: svec!["--help"],
        code_cache_enabled: true,
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec![
      "deno",
      "run",
      "--v8-flags=--expose-gc,--gc-stats=1",
      "script.ts"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "script.ts".to_string(),
        )),
        v8_flags: svec!["--expose-gc", "--gc-stats=1"],
        code_cache_enabled: true,
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec!["deno", "run", "--v8-flags=--expose-gc"]);
    assert!(r.is_ok());
  }

  #[test]
  fn serve_flags() {
    let r = flags_from_vec(svec!["deno", "serve", "main.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Serve(ServeFlags::new_default(
          "main.ts".to_string(),
          8000,
          "0.0.0.0"
        )),
        permissions: PermissionFlags {
          allow_net: None,
          ..Default::default()
        },
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
    let r = flags_from_vec(svec!["deno", "serve", "--port", "5000", "main.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Serve(ServeFlags::new_default(
          "main.ts".to_string(),
          5000,
          "0.0.0.0"
        )),
        permissions: PermissionFlags {
          allow_net: None,
          ..Default::default()
        },
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
    let r = flags_from_vec(svec![
      "deno",
      "serve",
      "--port",
      "5000",
      "--allow-net=example.com",
      "main.ts"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Serve(ServeFlags::new_default(
          "main.ts".to_string(),
          5000,
          "0.0.0.0"
        )),
        permissions: PermissionFlags {
          allow_net: Some(vec!["example.com".to_string(),]),
          ..Default::default()
        },
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
    let r = flags_from_vec(svec![
      "deno",
      "serve",
      "--port",
      "5000",
      "--allow-net",
      "main.ts"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Serve(ServeFlags::new_default(
          "main.ts".to_string(),
          5000,
          "0.0.0.0"
        )),
        permissions: PermissionFlags {
          allow_net: Some(vec![]),
          ..Default::default()
        },
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn has_permission() {
    let r = flags_from_vec(svec!["deno", "--allow-read", "x.ts"]);
    assert_eq!(r.unwrap().has_permission(), true);

    let r = flags_from_vec(svec!["deno", "run", "--deny-read", "x.ts"]);
    assert_eq!(r.unwrap().has_permission(), true);

    let r = flags_from_vec(svec!["deno", "run", "x.ts"]);
    assert_eq!(r.unwrap().has_permission(), false);
  }

  #[test]
  fn has_permission_in_argv() {
    let r = flags_from_vec(svec!["deno", "run", "x.ts", "--allow-read"]);
    assert_eq!(r.unwrap().has_permission_in_argv(), true);

    let r = flags_from_vec(svec!["deno", "x.ts", "--deny-read"]);
    assert_eq!(r.unwrap().has_permission_in_argv(), true);

    let r = flags_from_vec(svec!["deno", "run", "x.ts"]);
    assert_eq!(r.unwrap().has_permission_in_argv(), false);
  }

  #[test]
  fn script_args() {
    let r = flags_from_vec(svec![
      "deno",
      "run",
      "--allow-net",
      "gist.ts",
      "--title",
      "X"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "gist.ts".to_string()
        )),
        argv: svec!["--title", "X"],
        permissions: PermissionFlags {
          allow_net: Some(vec![]),
          ..Default::default()
        },
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn allow_all() {
    let r = flags_from_vec(svec!["deno", "run", "--allow-all", "gist.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "gist.ts".to_string()
        )),
        permissions: PermissionFlags {
          allow_all: true,
          ..Default::default()
        },
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn allow_read() {
    let r = flags_from_vec(svec!["deno", "run", "--allow-read", "gist.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "gist.ts".to_string()
        )),
        permissions: PermissionFlags {
          allow_read: Some(vec![]),
          ..Default::default()
        },
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn short_permission_flags() {
    let r = flags_from_vec(svec!["deno", "run", "-RNESWI", "gist.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "gist.ts".to_string()
        )),
        permissions: PermissionFlags {
          allow_read: Some(vec![]),
          allow_write: Some(vec![]),
          allow_env: Some(vec![]),
          allow_import: Some(vec![]),
          allow_net: Some(vec![]),
          allow_sys: Some(vec![]),
          ..Default::default()
        },
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn deny_read() {
    let r = flags_from_vec(svec!["deno", "--deny-read", "gist.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags {
          script: "gist.ts".to_string(),
          watch: None,
          bare: true,
          coverage_dir: None,
          print_task_list: false,
        }),
        permissions: PermissionFlags {
          deny_read: Some(vec![]),
          ..Default::default()
        },
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn double_hyphen() {
    // notice that flags passed after double dash will not
    // be parsed to Flags but instead forwarded to
    // script args as Deno.args
    let r = flags_from_vec(svec![
      "deno",
      "run",
      "--allow-write",
      "script.ts",
      "--",
      "-D",
      "--allow-net"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "script.ts".to_string(),
        )),
        argv: svec!["--", "-D", "--allow-net"],
        permissions: PermissionFlags {
          allow_write: Some(vec![]),
          ..Default::default()
        },
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn fmt() {
    let r = flags_from_vec(svec!["deno", "fmt", "script_1.ts", "script_2.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Fmt(FmtFlags {
          check: false,
          fail_fast: false,
          files: FileFlags {
            include: vec!["script_1.ts".to_string(), "script_2.ts".to_string()],
            ignore: vec![],
          },
          permit_no_files: false,
          use_tabs: None,
          line_width: None,
          indent_width: None,
          single_quote: None,
          prose_wrap: None,
          no_semicolons: None,
          unstable_component: false,
          unstable_sql: false,
          watch: Default::default(),
        }),
        ..Flags::default()
      }
    );

    let r =
      flags_from_vec(svec!["deno", "fmt", "--permit-no-files", "--check"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Fmt(FmtFlags {
          check: true,
          fail_fast: false,
          files: FileFlags {
            include: vec![],
            ignore: vec![],
          },
          permit_no_files: true,
          use_tabs: None,
          line_width: None,
          indent_width: None,
          single_quote: None,
          prose_wrap: None,
          no_semicolons: None,
          unstable_component: false,
          unstable_sql: false,
          watch: Default::default(),
        }),
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec!["deno", "fmt"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Fmt(FmtFlags {
          check: false,
          fail_fast: false,
          files: FileFlags {
            include: vec![],
            ignore: vec![],
          },
          permit_no_files: false,
          use_tabs: None,
          line_width: None,
          indent_width: None,
          single_quote: None,
          prose_wrap: None,
          no_semicolons: None,
          unstable_component: false,
          unstable_sql: false,
          watch: Default::default(),
        }),
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec!["deno", "fmt", "--watch"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Fmt(FmtFlags {
          check: false,
          fail_fast: false,
          files: FileFlags {
            include: vec![],
            ignore: vec![],
          },
          permit_no_files: false,
          use_tabs: None,
          line_width: None,
          indent_width: None,
          single_quote: None,
          prose_wrap: None,
          no_semicolons: None,
          unstable_component: false,
          unstable_sql: false,
          watch: Some(Default::default()),
        }),
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec![
      "deno",
      "fmt",
      "--watch",
      "--no-clear-screen",
      "--unstable-css",
      "--unstable-html",
      "--unstable-component",
      "--unstable-yaml",
      "--unstable-sql"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Fmt(FmtFlags {
          check: false,
          fail_fast: false,
          files: FileFlags {
            include: vec![],
            ignore: vec![],
          },
          permit_no_files: false,
          use_tabs: None,
          line_width: None,
          indent_width: None,
          single_quote: None,
          prose_wrap: None,
          no_semicolons: None,
          unstable_component: true,
          unstable_sql: true,
          watch: Some(WatchFlags {
            hmr: false,
            no_clear_screen: true,
            exclude: vec![],
          })
        }),
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec![
      "deno",
      "fmt",
      "--check",
      "--watch",
      "foo.ts",
      "--ignore=bar.js"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Fmt(FmtFlags {
          check: true,
          fail_fast: false,
          files: FileFlags {
            include: vec!["foo.ts".to_string()],
            ignore: vec!["bar.js".to_string()],
          },
          permit_no_files: false,
          use_tabs: None,
          line_width: None,
          indent_width: None,
          single_quote: None,
          prose_wrap: None,
          no_semicolons: None,
          unstable_component: false,
          unstable_sql: false,
          watch: Some(Default::default()),
        }),
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec!["deno", "fmt", "--config", "deno.jsonc"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Fmt(FmtFlags {
          check: false,
          fail_fast: false,
          files: FileFlags {
            include: vec![],
            ignore: vec![],
          },
          permit_no_files: false,
          use_tabs: None,
          line_width: None,
          indent_width: None,
          single_quote: None,
          prose_wrap: None,
          no_semicolons: None,
          unstable_component: false,
          unstable_sql: false,
          watch: Default::default(),
        }),
        config_flag: ConfigFlag::Path("deno.jsonc".to_string()),
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec![
      "deno",
      "fmt",
      "--config",
      "deno.jsonc",
      "--watch",
      "foo.ts"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Fmt(FmtFlags {
          check: false,
          fail_fast: false,
          files: FileFlags {
            include: vec!["foo.ts".to_string()],
            ignore: vec![],
          },
          permit_no_files: false,
          use_tabs: None,
          line_width: None,
          indent_width: None,
          single_quote: None,
          prose_wrap: None,
          no_semicolons: None,
          unstable_component: false,
          unstable_sql: false,
          watch: Some(Default::default()),
        }),
        config_flag: ConfigFlag::Path("deno.jsonc".to_string()),
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec![
      "deno",
      "fmt",
      "--use-tabs",
      "--line-width",
      "60",
      "--indent-width",
      "4",
      "--single-quote",
      "--prose-wrap",
      "never",
      "--no-semicolons",
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Fmt(FmtFlags {
          check: false,
          fail_fast: false,
          files: FileFlags {
            include: vec![],
            ignore: vec![],
          },
          permit_no_files: false,
          use_tabs: Some(true),
          line_width: Some(NonZeroU32::new(60).unwrap()),
          indent_width: Some(NonZeroU8::new(4).unwrap()),
          single_quote: Some(true),
          prose_wrap: Some("never".to_string()),
          no_semicolons: Some(true),
          unstable_component: false,
          unstable_sql: false,
          watch: Default::default(),
        }),
        ..Flags::default()
      }
    );

    // try providing =false to the booleans
    let r = flags_from_vec(svec![
      "deno",
      "fmt",
      "--use-tabs=false",
      "--single-quote=false",
      "--no-semicolons=false",
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Fmt(FmtFlags {
          check: false,
          fail_fast: false,
          files: FileFlags {
            include: vec![],
            ignore: vec![],
          },
          permit_no_files: false,
          use_tabs: Some(false),
          line_width: None,
          indent_width: None,
          single_quote: Some(false),
          prose_wrap: None,
          no_semicolons: Some(false),
          unstable_component: false,
          unstable_sql: false,
          watch: Default::default(),
        }),
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec!["deno", "fmt", "--ext", "html"]);
    assert!(r.is_err());
    let r = flags_from_vec(svec!["deno", "fmt", "--ext", "html", "./**"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Fmt(FmtFlags {
          check: false,
          fail_fast: false,
          files: FileFlags {
            include: vec!["./**".to_string()],
            ignore: vec![],
          },
          permit_no_files: false,
          use_tabs: None,
          line_width: None,
          indent_width: None,
          single_quote: None,
          prose_wrap: None,
          no_semicolons: None,
          unstable_component: false,
          unstable_sql: false,
          watch: Default::default(),
        }),
        ext: Some("html".to_string()),
        ..Flags::default()
      }
    );
  }

  #[test]
  fn lint() {
    let r = flags_from_vec(svec!["deno", "lint", "script_1.ts", "script_2.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Lint(LintFlags {
          files: FileFlags {
            include: vec!["script_1.ts".to_string(), "script_2.ts".to_string(),],
            ignore: vec![],
          },
          fix: false,
          rules: false,
          maybe_rules_tags: None,
          maybe_rules_include: None,
          maybe_rules_exclude: None,
          permit_no_files: false,
          json: false,
          compact: false,
          watch: Default::default(),
        }),
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec![
      "deno",
      "lint",
      "--permit-no-files",
      "--allow-import",
      "--watch",
      "script_1.ts",
      "script_2.ts"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Lint(LintFlags {
          files: FileFlags {
            include: vec!["script_1.ts".to_string(), "script_2.ts".to_string()],
            ignore: vec![],
          },
          fix: false,
          rules: false,
          maybe_rules_tags: None,
          maybe_rules_include: None,
          maybe_rules_exclude: None,
          permit_no_files: true,
          json: false,
          compact: false,
          watch: Some(Default::default()),
        }),
        permissions: PermissionFlags {
          allow_import: Some(vec![]),
          ..Default::default()
        },
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec![
      "deno",
      "lint",
      "--watch",
      "--no-clear-screen",
      "script_1.ts",
      "script_2.ts"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Lint(LintFlags {
          files: FileFlags {
            include: vec!["script_1.ts".to_string(), "script_2.ts".to_string()],
            ignore: vec![],
          },
          fix: false,
          rules: false,
          maybe_rules_tags: None,
          maybe_rules_include: None,
          maybe_rules_exclude: None,
          permit_no_files: false,
          json: false,
          compact: false,
          watch: Some(WatchFlags {
            hmr: false,
            no_clear_screen: true,
            exclude: vec![],
          }),
        }),
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec![
      "deno",
      "lint",
      "--fix",
      "--ignore=script_1.ts,script_2.ts"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Lint(LintFlags {
          files: FileFlags {
            include: vec![],
            ignore: vec!["script_1.ts".to_string(), "script_2.ts".to_string()],
          },
          fix: true,
          rules: false,
          maybe_rules_tags: None,
          maybe_rules_include: None,
          maybe_rules_exclude: None,
          permit_no_files: false,
          json: false,
          compact: false,
          watch: Default::default(),
        }),
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec!["deno", "lint", "--rules"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Lint(LintFlags {
          files: FileFlags {
            include: vec![],
            ignore: vec![],
          },
          fix: false,
          rules: true,
          maybe_rules_tags: None,
          maybe_rules_include: None,
          maybe_rules_exclude: None,
          permit_no_files: false,
          json: false,
          compact: false,
          watch: Default::default(),
        }),
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec![
      "deno",
      "lint",
      "--rules",
      "--rules-tags=recommended"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Lint(LintFlags {
          files: FileFlags {
            include: vec![],
            ignore: vec![],
          },
          fix: false,
          rules: true,
          maybe_rules_tags: Some(svec!["recommended"]),
          maybe_rules_include: None,
          maybe_rules_exclude: None,
          permit_no_files: false,
          json: false,
          compact: false,
          watch: Default::default(),
        }),
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec![
      "deno",
      "lint",
      "--rules-tags=",
      "--rules-include=ban-untagged-todo,no-undef",
      "--rules-exclude=no-const-assign"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Lint(LintFlags {
          files: FileFlags {
            include: vec![],
            ignore: vec![],
          },
          fix: false,
          rules: false,
          maybe_rules_tags: Some(svec![""]),
          maybe_rules_include: Some(svec!["ban-untagged-todo", "no-undef"]),
          maybe_rules_exclude: Some(svec!["no-const-assign"]),
          permit_no_files: false,
          json: false,
          compact: false,
          watch: Default::default(),
        }),
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec!["deno", "lint", "--json", "script_1.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Lint(LintFlags {
          files: FileFlags {
            include: vec!["script_1.ts".to_string()],
            ignore: vec![],
          },
          fix: false,
          rules: false,
          maybe_rules_tags: None,
          maybe_rules_include: None,
          maybe_rules_exclude: None,
          permit_no_files: false,
          json: true,
          compact: false,
          watch: Default::default(),
        }),
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec![
      "deno",
      "lint",
      "--config",
      "Deno.jsonc",
      "--json",
      "script_1.ts"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Lint(LintFlags {
          files: FileFlags {
            include: vec!["script_1.ts".to_string()],
            ignore: vec![],
          },
          fix: false,
          rules: false,
          maybe_rules_tags: None,
          maybe_rules_include: None,
          maybe_rules_exclude: None,
          permit_no_files: false,
          json: true,
          compact: false,
          watch: Default::default(),
        }),
        config_flag: ConfigFlag::Path("Deno.jsonc".to_string()),
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec![
      "deno",
      "lint",
      "--config",
      "Deno.jsonc",
      "--compact",
      "script_1.ts"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Lint(LintFlags {
          files: FileFlags {
            include: vec!["script_1.ts".to_string()],
            ignore: vec![],
          },
          fix: false,
          rules: false,
          maybe_rules_tags: None,
          maybe_rules_include: None,
          maybe_rules_exclude: None,
          permit_no_files: false,
          json: false,
          compact: true,
          watch: Default::default(),
        }),
        config_flag: ConfigFlag::Path("Deno.jsonc".to_string()),
        ..Flags::default()
      }
    );
  }

  #[test]
  fn types() {
    let r = flags_from_vec(svec!["deno", "types"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Types,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn cache() {
    let r = flags_from_vec(svec!["deno", "cache", "script.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Cache(CacheFlags {
          files: svec!["script.ts"],
        }),
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec!["deno", "cache", "--env-file", "script.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Cache(CacheFlags {
          files: svec!["script.ts"],
        }),
        env_file: Some(svec![".env"]),
        ..Flags::default()
      }
    );
  }

  #[test]
  fn check() {
    let r = flags_from_vec(svec!["deno", "check", "script.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Check(CheckFlags {
          files: svec!["script.ts"],
          doc: false,
          doc_only: false,
          check_js: false,
        }),
        type_check_mode: TypeCheckMode::Local,
        code_cache_enabled: true,
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec!["deno", "check"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Check(CheckFlags {
          files: svec!["."],
          doc: false,
          doc_only: false,
          check_js: false,
        }),
        type_check_mode: TypeCheckMode::Local,
        code_cache_enabled: true,
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec!["deno", "check", "--doc", "script.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Check(CheckFlags {
          files: svec!["script.ts"],
          doc: true,
          doc_only: false,
          check_js: false,
        }),
        type_check_mode: TypeCheckMode::Local,
        code_cache_enabled: true,
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec!["deno", "check", "--doc-only", "markdown.md"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Check(CheckFlags {
          files: svec!["markdown.md"],
          doc: false,
          doc_only: true,
          check_js: false,
        }),
        type_check_mode: TypeCheckMode::Local,
        code_cache_enabled: true,
        ..Flags::default()
      }
    );

    // `--doc` and `--doc-only` are mutually exclusive
    let r = flags_from_vec(svec![
      "deno",
      "check",
      "--doc",
      "--doc-only",
      "script.ts"
    ]);
    assert_eq!(r.unwrap_err().kind(), FlagsErrorKind::ArgumentConflict);

    for all_flag in ["--remote", "--all"] {
      let r = flags_from_vec(svec!["deno", "check", all_flag, "script.ts"]);
      assert_eq!(
        r.unwrap(),
        Flags {
          subcommand: DenoSubcommand::Check(CheckFlags {
            files: svec!["script.ts"],
            doc: false,
            doc_only: false,
            check_js: false,
          }),
          type_check_mode: TypeCheckMode::All,
          code_cache_enabled: true,
          ..Flags::default()
        }
      );

      let r = flags_from_vec(svec![
        "deno",
        "check",
        all_flag,
        "--no-remote",
        "script.ts"
      ]);
      assert_eq!(r.unwrap_err().kind(), FlagsErrorKind::ArgumentConflict);
    }

    let r = flags_from_vec(svec!["deno", "check", "--check-js", "script.js"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Check(CheckFlags {
          files: svec!["script.js"],
          doc: false,
          doc_only: false,
          check_js: true,
        }),
        type_check_mode: TypeCheckMode::Local,
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn info() {
    let r = flags_from_vec(svec!["deno", "info", "script.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Info(InfoFlags {
          json: false,
          file: Some("script.ts".to_string()),
        }),
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec!["deno", "info", "--reload", "script.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Info(InfoFlags {
          json: false,
          file: Some("script.ts".to_string()),
        }),
        reload: true,
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec!["deno", "info", "--json", "script.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Info(InfoFlags {
          json: true,
          file: Some("script.ts".to_string()),
        }),
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec!["deno", "info"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Info(InfoFlags {
          json: false,
          file: None
        }),
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec!["deno", "info", "--json"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Info(InfoFlags {
          json: true,
          file: None
        }),
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec![
      "deno",
      "info",
      "--no-npm",
      "--no-remote",
      "--config",
      "tsconfig.json"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Info(InfoFlags {
          json: false,
          file: None
        }),
        config_flag: ConfigFlag::Path("tsconfig.json".to_owned()),
        no_npm: true,
        no_remote: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn tsconfig() {
    let r =
      flags_from_vec(svec!["deno", "run", "-c", "tsconfig.json", "script.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "script.ts".to_string(),
        )),
        config_flag: ConfigFlag::Path("tsconfig.json".to_owned()),
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn eval() {
    let r = flags_from_vec(svec!["deno", "eval", "'console.log(\"hello\")'"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Eval(EvalFlags {
          print: false,
          code: "'console.log(\"hello\")'".to_string(),
        }),
        permissions: PermissionFlags {
          allow_all: true,
          ..Default::default()
        },
        ..Flags::default()
      }
    );
  }

  #[test]
  fn eval_p() {
    let r = flags_from_vec(svec!["deno", "eval", "-p", "1+2"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Eval(EvalFlags {
          print: true,
          code: "1+2".to_string(),
        }),
        permissions: PermissionFlags {
          allow_all: true,
          ..Default::default()
        },
        ..Flags::default()
      }
    );
  }

  #[test]
  fn eval_typescript() {
    let r = flags_from_vec(svec![
      "deno",
      "eval",
      "--ext=ts",
      "'console.log(\"hello\")'"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Eval(EvalFlags {
          print: false,
          code: "'console.log(\"hello\")'".to_string(),
        }),
        permissions: PermissionFlags {
          allow_all: true,
          ..Default::default()
        },
        ext: Some("ts".to_string()),
        ..Flags::default()
      }
    );
  }

  #[test]
  fn eval_with_flags() {
    #[rustfmt::skip]
    let r = flags_from_vec(svec!["deno", "eval", "--import-map", "import_map.json", "--no-remote", "--config", "tsconfig.json", "--no-check", "--reload", "--lock", "lock.json", "--cert", "example.crt", "--cached-only", "--location", "https:foo", "--v8-flags=--help", "--seed", "1", "--inspect=127.0.0.1:9229", "--env=.example.env", "42"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Eval(EvalFlags {
          print: false,
          code: "42".to_string(),
        }),
        import_map_path: Some("import_map.json".to_string()),
        no_remote: true,
        config_flag: ConfigFlag::Path("tsconfig.json".to_owned()),
        type_check_mode: TypeCheckMode::None,
        reload: true,
        lock: Some(String::from("lock.json")),
        ca_data: Some(CaData::File("example.crt".to_string())),
        cached_only: true,
        location: Some(Url::parse("https://foo/").unwrap()),
        v8_flags: svec!["--help", "--random-seed=1"],
        seed: Some(1),
        inspect: Some("127.0.0.1:9229".parse().unwrap()),
        permissions: PermissionFlags {
          allow_all: true,
          ..Default::default()
        },
        env_file: Some(vec![".example.env".to_owned()]),
        ..Flags::default()
      }
    );
  }

  #[test]
  fn eval_args() {
    let r = flags_from_vec(svec![
      "deno",
      "eval",
      "console.log(Deno.args)",
      "arg1",
      "arg2"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Eval(EvalFlags {
          print: false,
          code: "console.log(Deno.args)".to_string(),
        }),
        argv: svec!["arg1", "arg2"],
        permissions: PermissionFlags {
          allow_all: true,
          ..Default::default()
        },
        ..Flags::default()
      }
    );
  }

  #[test]
  fn repl() {
    let r = flags_from_vec(svec!["deno"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Repl(ReplFlags {
          eval_files: None,
          eval: None,
          is_default_command: true,
          json: false,
        }),
        unsafely_ignore_certificate_errors: None,
        permissions: PermissionFlags {
          allow_all: true,
          ..Default::default()
        },
        ..Flags::default()
      }
    );
  }

  #[test]
  fn repl_trace_ops() {
    // Lightly test this undocumented flag
    let r = flags_from_vec(svec!["deno", "repl", "--trace-ops"]);
    assert_eq!(r.unwrap().trace_ops, Some(vec![]));
    let r = flags_from_vec(svec!["deno", "repl", "--trace-ops=http,websocket"]);
    assert_eq!(
      r.unwrap().trace_ops,
      Some(vec!["http".to_string(), "websocket".to_string()])
    );
  }

  #[test]
  fn repl_with_flags() {
    #[rustfmt::skip]
    let r = flags_from_vec(svec!["deno", "repl", "-A", "--import-map", "import_map.json", "--no-remote", "--config", "tsconfig.json", "--reload", "--lock", "lock.json", "--cert", "example.crt", "--cached-only", "--location", "https:foo", "--v8-flags=--help", "--seed", "1", "--inspect=127.0.0.1:9229", "--unsafely-ignore-certificate-errors", "--env=.example.env"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Repl(ReplFlags {
          eval_files: None,
          eval: None,
          is_default_command: false,
          json: false,
        }),
        import_map_path: Some("import_map.json".to_string()),
        no_remote: true,
        config_flag: ConfigFlag::Path("tsconfig.json".to_owned()),
        type_check_mode: TypeCheckMode::None,
        reload: true,
        lock: Some(String::from("lock.json")),
        ca_data: Some(CaData::File("example.crt".to_string())),
        cached_only: true,
        location: Some(Url::parse("https://foo/").unwrap()),
        v8_flags: svec!["--help", "--random-seed=1"],
        seed: Some(1),
        inspect: Some("127.0.0.1:9229".parse().unwrap()),
        permissions: PermissionFlags {
          allow_all: true,
          ..Default::default()
        },
        env_file: Some(vec![".example.env".to_owned()]),
        unsafely_ignore_certificate_errors: Some(vec![]),
        ..Flags::default()
      }
    );
  }

  #[test]
  fn repl_with_eval_flag() {
    #[rustfmt::skip]
    let r = flags_from_vec(svec!["deno", "repl", "--allow-write", "--eval", "console.log('hello');"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Repl(ReplFlags {
          eval_files: None,
          eval: Some("console.log('hello');".to_string()),
          is_default_command: false,
          json: false,
        }),
        permissions: PermissionFlags {
          allow_write: Some(vec![]),
          ..Default::default()
        },
        ..Flags::default()
      }
    );
  }

  #[test]
  fn repl_with_eval_file_flag() {
    #[rustfmt::skip]
    let r = flags_from_vec(svec!["deno", "repl", "--eval-file=./a.js,./b.ts,https://docs.deno.com/hello_world.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Repl(ReplFlags {
          eval_files: Some(vec![
            "./a.js".to_string(),
            "./b.ts".to_string(),
            "https://docs.deno.com/hello_world.ts".to_string()
          ]),
          eval: None,
          is_default_command: false,
          json: false,
        }),
        ..Flags::default()
      }
    );
  }

  #[test]
  fn repl_with_eval_file_flag_no_equals() {
    // Test without equals sign (for hashbang usage)
    #[rustfmt::skip]
    let r = flags_from_vec(svec!["deno", "repl", "--eval-file", "./script.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Repl(ReplFlags {
          eval_files: Some(vec!["./script.ts".to_string()]),
          eval: None,
          is_default_command: false,
          json: false,
        }),
        ..Flags::default()
      }
    );
  }

  #[test]
  fn repl_with_eval_file_flag_multiple() {
    // Test multiple --eval-file flags
    #[rustfmt::skip]
    let r = flags_from_vec(svec!["deno", "repl", "--eval-file", "./a.ts", "--eval-file", "./b.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Repl(ReplFlags {
          eval_files: Some(vec!["./a.ts".to_string(), "./b.ts".to_string()]),
          eval: None,
          is_default_command: false,
          json: false,
        }),
        ..Flags::default()
      }
    );
  }

  #[test]
  fn allow_read_allowlist() {
    use test_util::TempDir;
    let temp_dir_guard = TempDir::new();
    let temp_dir = temp_dir_guard.path().to_string();

    let r = flags_from_vec(svec![
      "deno",
      "run",
      format!("--allow-read=.,{}", temp_dir),
      "script.ts"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        permissions: PermissionFlags {
          allow_read: Some(vec![String::from("."), temp_dir]),
          ..Default::default()
        },
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "script.ts".to_string(),
        )),
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn deny_read_denylist() {
    use test_util::TempDir;
    let temp_dir_guard = TempDir::new();
    let temp_dir = temp_dir_guard.path().to_string();

    let r = flags_from_vec(svec![
      "deno",
      "run",
      format!("--deny-read=.,{}", temp_dir),
      "script.ts"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        permissions: PermissionFlags {
          deny_read: Some(vec![String::from("."), temp_dir]),
          ..Default::default()
        },
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "script.ts".to_string(),
        )),
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn ignore_read_ignorelist() {
    let r = flags_from_vec(svec![
      "deno",
      "run",
      "--ignore-read=something.txt",
      "script.ts"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "script.ts".to_string(),
        )),
        permissions: PermissionFlags {
          ignore_read: Some(svec!["something.txt"]),
          ..Default::default()
        },
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn ignore_read_ignorelist_multiple() {
    let r = flags_from_vec(svec![
      "deno",
      "run",
      "--ignore-read=something.txt",
      "--ignore-read=something2.txt",
      "script.ts"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "script.ts".to_string(),
        )),
        permissions: PermissionFlags {
          ignore_read: Some(svec!["something.txt", "something2.txt"]),
          ..Default::default()
        },
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn allow_write_allowlist() {
    use test_util::TempDir;
    let temp_dir_guard = TempDir::new();
    let temp_dir = temp_dir_guard.path().to_string();

    let r = flags_from_vec(svec![
      "deno",
      "run",
      format!("--allow-write=.,{}", temp_dir),
      "script.ts"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        permissions: PermissionFlags {
          allow_write: Some(vec![String::from("."), temp_dir]),
          ..Default::default()
        },
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "script.ts".to_string(),
        )),
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn deny_write_denylist() {
    use test_util::TempDir;
    let temp_dir_guard = TempDir::new();
    let temp_dir = temp_dir_guard.path().to_string();

    let r = flags_from_vec(svec![
      "deno",
      "run",
      format!("--deny-write=.,{}", temp_dir),
      "script.ts"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        permissions: PermissionFlags {
          deny_write: Some(vec![String::from("."), temp_dir]),
          ..Default::default()
        },
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "script.ts".to_string(),
        )),
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn allow_net_allowlist() {
    let r = flags_from_vec(svec![
      "deno",
      "run",
      "--allow-net=127.0.0.1",
      "script.ts"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "script.ts".to_string(),
        )),
        permissions: PermissionFlags {
          allow_net: Some(svec!["127.0.0.1"]),
          ..Default::default()
        },
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn deny_net_denylist() {
    let r = flags_from_vec(svec!["deno", "--deny-net=127.0.0.1", "script.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags {
          script: "script.ts".to_string(),
          watch: None,
          bare: true,
          coverage_dir: None,
          print_task_list: false,
        }),
        permissions: PermissionFlags {
          deny_net: Some(svec!["127.0.0.1"]),
          ..Default::default()
        },
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn allow_env_allowlist() {
    let r =
      flags_from_vec(svec!["deno", "run", "--allow-env=HOME", "script.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "script.ts".to_string(),
        )),
        permissions: PermissionFlags {
          allow_env: Some(svec!["HOME"]),
          ..Default::default()
        },
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn deny_env_denylist() {
    let r =
      flags_from_vec(svec!["deno", "run", "--deny-env=HOME", "script.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "script.ts".to_string(),
        )),
        permissions: PermissionFlags {
          deny_env: Some(svec!["HOME"]),
          ..Default::default()
        },
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn ignore_env_ignorelist() {
    let r =
      flags_from_vec(svec!["deno", "run", "--ignore-env=HOME", "script.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "script.ts".to_string(),
        )),
        permissions: PermissionFlags {
          ignore_env: Some(svec!["HOME"]),
          ..Default::default()
        },
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn allow_env_allowlist_multiple() {
    let r = flags_from_vec(svec![
      "deno",
      "run",
      "--allow-env=HOME,PATH",
      "script.ts"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "script.ts".to_string(),
        )),
        permissions: PermissionFlags {
          allow_env: Some(svec!["HOME", "PATH"]),
          ..Default::default()
        },
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn deny_env_denylist_multiple() {
    let r =
      flags_from_vec(svec!["deno", "run", "--deny-env=HOME,PATH", "script.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "script.ts".to_string(),
        )),
        permissions: PermissionFlags {
          deny_env: Some(svec!["HOME", "PATH"]),
          ..Default::default()
        },
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn deny_env_ignorelist_multiple() {
    let r = flags_from_vec(svec![
      "deno",
      "run",
      "--ignore-env=HOME,PATH",
      "script.ts"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "script.ts".to_string(),
        )),
        permissions: PermissionFlags {
          ignore_env: Some(svec!["HOME", "PATH"]),
          ..Default::default()
        },
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn allow_env_allowlist_validator() {
    let r =
      flags_from_vec(svec!["deno", "run", "--allow-env=HOME", "script.ts"]);
    assert!(r.is_ok());
    let r = flags_from_vec(svec!["deno", "--allow-env=H=ME", "script.ts"]);
    assert!(r.is_err());
    let r =
      flags_from_vec(svec!["deno", "run", "--allow-env=H\0ME", "script.ts"]);
    assert!(r.is_err());
  }

  #[test]
  fn deny_env_denylist_validator() {
    let r =
      flags_from_vec(svec!["deno", "run", "--deny-env=HOME", "script.ts"]);
    assert!(r.is_ok());
    let r =
      flags_from_vec(svec!["deno", "run", "--deny-env=H=ME", "script.ts"]);
    assert!(r.is_err());
    let r = flags_from_vec(svec!["deno", "--deny-env=H\0ME", "script.ts"]);
    assert!(r.is_err());
  }

  #[test]
  fn allow_sys() {
    let r = flags_from_vec(svec!["deno", "run", "--allow-sys", "script.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "script.ts".to_string(),
        )),
        permissions: PermissionFlags {
          allow_sys: Some(vec![]),
          ..Default::default()
        },
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn deny_sys() {
    let r = flags_from_vec(svec!["deno", "run", "--deny-sys", "script.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "script.ts".to_string(),
        )),
        permissions: PermissionFlags {
          deny_sys: Some(vec![]),
          ..Default::default()
        },
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn allow_sys_allowlist() {
    let r =
      flags_from_vec(svec!["deno", "run", "--allow-sys=hostname", "script.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "script.ts".to_string(),
        )),
        permissions: PermissionFlags {
          allow_sys: Some(svec!["hostname"]),
          ..Default::default()
        },
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn deny_sys_denylist() {
    let r = flags_from_vec(svec!["deno", "--deny-sys=hostname", "script.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags {
          script: "script.ts".to_string(),
          watch: None,
          bare: true,
          coverage_dir: None,
          print_task_list: false,
        }),
        permissions: PermissionFlags {
          deny_sys: Some(svec!["hostname"]),
          ..Default::default()
        },
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn allow_sys_allowlist_multiple() {
    let r = flags_from_vec(svec![
      "deno",
      "run",
      "--allow-sys=hostname,osRelease",
      "script.ts"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "script.ts".to_string(),
        )),
        permissions: PermissionFlags {
          allow_sys: Some(svec!["hostname", "osRelease"]),
          ..Default::default()
        },
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn deny_sys_denylist_multiple() {
    let r = flags_from_vec(svec![
      "deno",
      "run",
      "--deny-sys=hostname,osRelease",
      "script.ts"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "script.ts".to_string(),
        )),
        permissions: PermissionFlags {
          deny_sys: Some(svec!["hostname", "osRelease"]),
          ..Default::default()
        },
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn allow_sys_allowlist_validator() {
    let r =
      flags_from_vec(svec!["deno", "run", "--allow-sys=hostname", "script.ts"]);
    assert!(r.is_ok());
    let r = flags_from_vec(svec![
      "deno",
      "run",
      "--allow-sys=hostname,osRelease",
      "script.ts"
    ]);
    assert!(r.is_ok());
    let r =
      flags_from_vec(svec!["deno", "run", "--allow-sys=foo", "script.ts"]);
    assert!(r.is_err());
    let r = flags_from_vec(svec![
      "deno",
      "run",
      "--allow-sys=hostname,foo",
      "script.ts"
    ]);
    assert!(r.is_err());
  }

  #[test]
  fn deny_sys_denylist_validator() {
    let r =
      flags_from_vec(svec!["deno", "run", "--deny-sys=hostname", "script.ts"]);
    assert!(r.is_ok());
    let r = flags_from_vec(svec![
      "deno",
      "run",
      "--deny-sys=hostname,osRelease",
      "script.ts"
    ]);
    assert!(r.is_ok());
    let r = flags_from_vec(svec!["deno", "run", "--deny-sys=foo", "script.ts"]);
    assert!(r.is_err());
    let r = flags_from_vec(svec![
      "deno",
      "run",
      "--deny-sys=hostname,foo",
      "script.ts"
    ]);
    assert!(r.is_err());
  }

  #[test]
  fn reload_validator() {
    let r = flags_from_vec(svec![
      "deno",
      "run",
      "--reload=http://deno.land/",
      "script.ts"
    ]);
    assert!(r.is_ok(), "should accept valid urls");

    let r = flags_from_vec(svec![
      "deno",
      "run",
      "--reload=http://deno.land/a,http://deno.land/b",
      "script.ts"
    ]);
    assert!(r.is_ok(), "should accept accept multiple valid urls");

    let r = flags_from_vec(svec![
      "deno",
      "run",
      "--reload=./relativeurl/",
      "script.ts"
    ]);
    assert!(r.is_err(), "Should reject relative urls that start with ./");

    let r = flags_from_vec(svec![
      "deno",
      "run",
      "--reload=relativeurl/",
      "script.ts"
    ]);
    assert!(r.is_err(), "Should reject relative urls");

    let r =
      flags_from_vec(svec!["deno", "run", "--reload=/absolute", "script.ts"]);
    assert!(r.is_err(), "Should reject absolute urls");

    let r = flags_from_vec(svec!["deno", "--reload=/", "script.ts"]);
    assert!(r.is_err(), "Should reject absolute root url");

    let r = flags_from_vec(svec!["deno", "run", "--reload=", "script.ts"]);
    assert!(r.is_err(), "Should reject when nothing is provided");

    let r = flags_from_vec(svec!["deno", "run", "--reload=,", "script.ts"]);
    assert!(r.is_err(), "Should reject when a single comma is provided");

    let r = flags_from_vec(svec![
      "deno",
      "run",
      "--reload=,http://deno.land/a",
      "script.ts"
    ]);
    assert!(r.is_err(), "Should reject a leading comma");

    let r = flags_from_vec(svec![
      "deno",
      "run",
      "--reload=http://deno.land/a,",
      "script.ts"
    ]);
    assert!(r.is_err(), "Should reject a trailing comma");
  }

  #[test]
  fn run_import_map() {
    let r = flags_from_vec(svec![
      "deno",
      "run",
      "--import-map=import_map.json",
      "script.ts"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "script.ts".to_string(),
        )),
        import_map_path: Some("import_map.json".to_owned()),
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn info_import_map() {
    let r = flags_from_vec(svec![
      "deno",
      "info",
      "--import-map=import_map.json",
      "script.ts"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Info(InfoFlags {
          file: Some("script.ts".to_string()),
          json: false,
        }),
        import_map_path: Some("import_map.json".to_owned()),
        ..Flags::default()
      }
    );
  }

  #[test]
  fn cache_import_map() {
    let r = flags_from_vec(svec![
      "deno",
      "cache",
      "--import-map=import_map.json",
      "script.ts"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Cache(CacheFlags {
          files: svec!["script.ts"],
        }),
        import_map_path: Some("import_map.json".to_owned()),
        ..Flags::default()
      }
    );
  }

  #[test]
  fn doc_import_map() {
    let r = flags_from_vec(svec![
      "deno",
      "doc",
      "--import-map=import_map.json",
      "script.ts"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Doc(DocFlags {
          source_files: DocSourceFileFlag::Paths(vec!["script.ts".to_owned()]),
          private: false,
          json: false,
          html: None,
          lint: false,
          filter: None,
        }),
        import_map_path: Some("import_map.json".to_owned()),
        ..Flags::default()
      }
    );
  }

  #[test]
  fn run_env_default() {
    let r = flags_from_vec(svec!["deno", "run", "--env", "script.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "script.ts".to_string(),
        )),
        env_file: Some(vec![".env".to_owned()]),
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn run_env_file_default() {
    let r = flags_from_vec(svec!["deno", "run", "--env-file", "script.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "script.ts".to_string(),
        )),
        env_file: Some(vec![".env".to_owned()]),
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn run_no_code_cache() {
    let r = flags_from_vec(svec!["deno", "--no-code-cache", "script.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags {
          script: "script.ts".to_string(),
          watch: None,
          bare: true,
          coverage_dir: None,
          print_task_list: false,
        }),
        ..Flags::default()
      }
    );
  }

  #[test]
  fn run_env_defined() {
    let r =
      flags_from_vec(svec!["deno", "run", "--env=.another_env", "script.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "script.ts".to_string(),
        )),
        env_file: Some(vec![".another_env".to_owned()]),
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn run_env_file_defined() {
    let r = flags_from_vec(svec![
      "deno",
      "run",
      "--env-file=.another_env",
      "script.ts"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "script.ts".to_string(),
        )),
        env_file: Some(vec![".another_env".to_owned()]),
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn run_multiple_env_file_defined() {
    let r = flags_from_vec(svec![
      "deno",
      "run",
      "--env-file",
      "--env-file=.two_env",
      "script.ts"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "script.ts".to_string(),
        )),
        env_file: Some(vec![".env".to_owned(), ".two_env".to_owned()]),
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn cache_multiple() {
    let r =
      flags_from_vec(svec!["deno", "cache", "script.ts", "script_two.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Cache(CacheFlags {
          files: svec!["script.ts", "script_two.ts"],
        }),
        ..Flags::default()
      }
    );
  }

  #[test]
  fn run_seed() {
    let r = flags_from_vec(svec!["deno", "run", "--seed", "250", "script.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "script.ts".to_string(),
        )),
        seed: Some(250_u64),
        v8_flags: svec!["--random-seed=250"],
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn run_seed_with_v8_flags() {
    let r = flags_from_vec(svec![
      "deno",
      "run",
      "--seed",
      "250",
      "--v8-flags=--expose-gc",
      "script.ts"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "script.ts".to_string(),
        )),
        seed: Some(250_u64),
        v8_flags: svec!["--expose-gc", "--random-seed=250"],
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn install() {
    let r = flags_from_vec(svec![
      "deno",
      "install",
      "-g",
      "jsr:@std/http/file-server",
      "npm:chalk",
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Install(InstallFlags::Global(
          InstallFlagsGlobal {
            name: None,
            module_urls: svec!["jsr:@std/http/file-server", "npm:chalk"],
            args: vec![],
            root: None,
            force: false,
            compile: false,
          }
        ),),
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec![
      "deno",
      "install",
      "-g",
      "jsr:@std/http/file-server"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Install(InstallFlags::Global(
          InstallFlagsGlobal {
            name: None,
            module_urls: svec!["jsr:@std/http/file-server"],
            args: vec![],
            root: None,
            force: false,
            compile: false,
          }
        ),),
        ..Flags::default()
      }
    );
  }

  #[test]
  fn install_with_flags() {
    #[rustfmt::skip]
    let r = flags_from_vec(svec!["deno", "install", "--global", "--import-map", "import_map.json", "--no-remote", "--config", "tsconfig.json", "--no-check", "--unsafely-ignore-certificate-errors", "--reload", "--lock", "lock.json", "--cert", "example.crt", "--cached-only", "--allow-read", "--allow-net", "--v8-flags=--help", "--seed", "1", "--inspect=127.0.0.1:9229", "--name", "file_server", "--root", "/foo", "--force", "--env=.example.env", "jsr:@std/http/file-server", "--", "foo", "bar"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Install(InstallFlags::Global(
          InstallFlagsGlobal {
            name: Some("file_server".to_string()),
            module_urls: svec!["jsr:@std/http/file-server"],
            args: svec!["foo", "bar"],
            root: Some("/foo".to_string()),
            force: true,
            compile: false,
          }
        ),),
        import_map_path: Some("import_map.json".to_string()),
        no_remote: true,
        config_flag: ConfigFlag::Path("tsconfig.json".to_owned()),
        type_check_mode: TypeCheckMode::None,
        reload: true,
        lock: Some(String::from("lock.json")),
        ca_data: Some(CaData::File("example.crt".to_string())),
        cached_only: true,
        v8_flags: svec!["--help", "--random-seed=1"],
        seed: Some(1),
        inspect: Some("127.0.0.1:9229".parse().unwrap()),
        unsafely_ignore_certificate_errors: Some(vec![]),
        permissions: PermissionFlags {
          allow_net: Some(vec![]),
          allow_read: Some(vec![]),
          ..Default::default()
        },
        env_file: Some(vec![".example.env".to_owned()]),
        ..Flags::default()
      }
    );
  }

  #[test]
  fn uninstall() {
    let r = flags_from_vec(svec!["deno", "uninstall"]);
    assert!(r.is_err(),);

    let r = flags_from_vec(svec![
      "deno",
      "uninstall",
      "--frozen",
      "--lockfile-only",
      "@std/load"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Uninstall(UninstallFlags {
          kind: UninstallKind::Local(RemoveFlags {
            packages: vec!["@std/load".to_string()],
            lockfile_only: true,
          }),
        }),
        frozen_lockfile: Some(true),
        ..Flags::default()
      }
    );

    let r =
      flags_from_vec(svec!["deno", "uninstall", "file_server", "@std/load"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Uninstall(UninstallFlags {
          kind: UninstallKind::Local(RemoveFlags {
            packages: vec!["file_server".to_string(), "@std/load".to_string()],
            lockfile_only: false,
          }),
        }),
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec!["deno", "uninstall", "-g", "file_server"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Uninstall(UninstallFlags {
          kind: UninstallKind::Global(UninstallFlagsGlobal {
            name: "file_server".to_string(),
            root: None,
          }),
        }),
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec![
      "deno",
      "uninstall",
      "-g",
      "--root",
      "/user/foo/bar",
      "file_server"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Uninstall(UninstallFlags {
          kind: UninstallKind::Global(UninstallFlagsGlobal {
            name: "file_server".to_string(),
            root: Some("/user/foo/bar".to_string()),
          }),
        }),
        ..Flags::default()
      }
    );
  }

  #[test]
  fn uninstall_with_help_flag() {
    let r = flags_from_vec(svec!["deno", "uninstall", "--help"]);
    assert!(r.is_ok());
  }

  #[test]
  fn log_level() {
    let r =
      flags_from_vec(svec!["deno", "run", "--log-level=debug", "script.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "script.ts".to_string(),
        )),
        log_level: Some(Level::Debug),
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn quiet() {
    let r = flags_from_vec(svec!["deno", "-q", "script.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags {
          script: "script.ts".to_string(),
          watch: None,
          bare: true,
          coverage_dir: None,
          print_task_list: false,
        }),
        log_level: Some(Level::Error),
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn completions() {
    let r = flags_from_vec(svec!["deno", "completions", "zsh"]).unwrap();

    match r.subcommand {
      DenoSubcommand::Completions(CompletionsFlags::Static(buf)) => {
        assert!(!buf.is_empty())
      }
      _ => unreachable!(),
    }
  }

  #[test]
  fn run_with_args() {
    let r = flags_from_vec(svec![
      "deno",
      "run",
      "script.ts",
      "--allow-read",
      "--allow-net"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "script.ts".to_string(),
        )),
        argv: svec!["--allow-read", "--allow-net"],
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
    let r = flags_from_vec(svec![
      "deno",
      "run",
      "--location",
      "https:foo",
      "--allow-read",
      "script.ts",
      "--allow-net",
      "-r",
      "--help",
      "--foo",
      "bar"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "script.ts".to_string(),
        )),
        location: Some(Url::parse("https://foo/").unwrap()),
        permissions: PermissionFlags {
          allow_read: Some(vec![]),
          ..Default::default()
        },
        argv: svec!["--allow-net", "-r", "--help", "--foo", "bar"],
        code_cache_enabled: true,
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec!["deno", "run", "script.ts", "foo", "bar"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "script.ts".to_string(),
        )),
        argv: svec!["foo", "bar"],
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
    let r = flags_from_vec(svec!["deno", "run", "script.ts", "-"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "script.ts".to_string(),
        )),
        argv: svec!["-"],
        code_cache_enabled: true,
        ..Flags::default()
      }
    );

    let r =
      flags_from_vec(svec!["deno", "run", "script.ts", "-", "foo", "bar"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "script.ts".to_string(),
        )),
        argv: svec!["-", "foo", "bar"],
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn no_check() {
    let r = flags_from_vec(svec!["deno", "--no-check", "script.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags {
          script: "script.ts".to_string(),
          watch: None,
          bare: true,
          coverage_dir: None,
          print_task_list: false,
        }),
        type_check_mode: TypeCheckMode::None,
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn no_check_remote() {
    let r =
      flags_from_vec(svec!["deno", "run", "--no-check=remote", "script.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "script.ts".to_string(),
        )),
        type_check_mode: TypeCheckMode::Local,
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn repl_with_unsafely_ignore_certificate_errors() {
    let r = flags_from_vec(svec![
      "deno",
      "repl",
      "--eval",
      "console.log('hello');",
      "--unsafely-ignore-certificate-errors"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Repl(ReplFlags {
          eval_files: None,
          eval: Some("console.log('hello');".to_string()),
          is_default_command: false,
          json: false,
        }),
        unsafely_ignore_certificate_errors: Some(vec![]),
        type_check_mode: TypeCheckMode::None,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn run_with_unsafely_ignore_certificate_errors() {
    let r = flags_from_vec(svec![
      "deno",
      "run",
      "--unsafely-ignore-certificate-errors",
      "script.ts"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "script.ts".to_string(),
        )),
        unsafely_ignore_certificate_errors: Some(vec![]),
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn run_with_unsafely_treat_insecure_origin_as_secure_with_ipv6_address() {
    let r = flags_from_vec(svec![
      "deno",
      "run",
      "--unsafely-ignore-certificate-errors=deno.land,localhost,[::],127.0.0.1,[::1],1.2.3.4",
      "script.ts"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "script.ts".to_string(),
        )),
        unsafely_ignore_certificate_errors: Some(svec![
          "deno.land",
          "localhost",
          "[::]",
          "127.0.0.1",
          "[::1]",
          "1.2.3.4"
        ]),
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn repl_with_unsafely_treat_insecure_origin_as_secure_with_ipv6_address() {
    let r = flags_from_vec(svec![
      "deno",
      "repl",
      "--unsafely-ignore-certificate-errors=deno.land,localhost,[::],127.0.0.1,[::1],1.2.3.4"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Repl(ReplFlags {
          eval_files: None,
          eval: None,
          is_default_command: false,
          json: false,
        }),
        unsafely_ignore_certificate_errors: Some(svec![
          "deno.land",
          "localhost",
          "[::]",
          "127.0.0.1",
          "[::1]",
          "1.2.3.4"
        ]),
        type_check_mode: TypeCheckMode::None,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn no_remote() {
    let r = flags_from_vec(svec!["deno", "run", "--no-remote", "script.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "script.ts".to_string(),
        )),
        no_remote: true,
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn no_npm() {
    let r = flags_from_vec(svec!["deno", "run", "--no-npm", "script.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "script.ts".to_string(),
        )),
        no_npm: true,
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn local_npm() {
    let r = flags_from_vec(svec!["deno", "--node-modules-dir", "script.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags {
          script: "script.ts".to_string(),
          watch: None,
          bare: true,
          coverage_dir: None,
          print_task_list: false,
        }),
        node_modules_dir: Some(NodeModulesDirMode::Auto),
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn vendor_flag() {
    let r = flags_from_vec(svec!["deno", "run", "--vendor", "script.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "script.ts".to_string(),
        )),
        vendor: Some(true),
        code_cache_enabled: true,
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec!["deno", "run", "--vendor=false", "script.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "script.ts".to_string(),
        )),
        vendor: Some(false),
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn cached_only() {
    let r = flags_from_vec(svec!["deno", "run", "--cached-only", "script.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "script.ts".to_string(),
        )),
        cached_only: true,
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn allow_net_allowlist_with_ports() {
    let r = flags_from_vec(svec![
      "deno",
      "run",
      "--allow-net=deno.land,:8000,:4545",
      "script.ts"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "script.ts".to_string(),
        )),
        permissions: PermissionFlags {
          allow_net: Some(svec![
            "deno.land",
            "0.0.0.0:8000",
            "127.0.0.1:8000",
            "localhost:8000",
            "0.0.0.0:4545",
            "127.0.0.1:4545",
            "localhost:4545"
          ]),
          ..Default::default()
        },
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn deny_net_denylist_with_ports() {
    let r = flags_from_vec(svec![
      "deno",
      "run",
      "--deny-net=deno.land,:8000,:4545",
      "script.ts"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "script.ts".to_string(),
        )),
        permissions: PermissionFlags {
          deny_net: Some(svec![
            "deno.land",
            "0.0.0.0:8000",
            "127.0.0.1:8000",
            "localhost:8000",
            "0.0.0.0:4545",
            "127.0.0.1:4545",
            "localhost:4545"
          ]),
          ..Default::default()
        },
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn allow_net_allowlist_with_ipv6_address() {
    let r = flags_from_vec(svec![
      "deno",
      "run",
      "--allow-net=deno.land,deno.land:80,[::],127.0.0.1,[::1],1.2.3.4:5678,:5678,[::1]:8080",
      "script.ts"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "script.ts".to_string(),
        )),
        permissions: PermissionFlags {
          allow_net: Some(svec![
            "deno.land",
            "deno.land:80",
            "[::]",
            "127.0.0.1",
            "[::1]",
            "1.2.3.4:5678",
            "0.0.0.0:5678",
            "127.0.0.1:5678",
            "localhost:5678",
            "[::1]:8080"
          ]),
          ..Default::default()
        },
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn deny_net_denylist_with_ipv6_address() {
    let r = flags_from_vec(svec![
      "deno",
      "run",
      "--deny-net=deno.land,deno.land:80,[::],127.0.0.1,[::1],1.2.3.4:5678,:5678,[::1]:8080",
      "script.ts"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "script.ts".to_string(),
        )),
        permissions: PermissionFlags {
          deny_net: Some(svec![
            "deno.land",
            "deno.land:80",
            "[::]",
            "127.0.0.1",
            "[::1]",
            "1.2.3.4:5678",
            "0.0.0.0:5678",
            "127.0.0.1:5678",
            "localhost:5678",
            "[::1]:8080"
          ]),
          ..Default::default()
        },
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn test_with_flags() {
    #[rustfmt::skip]
    let r = flags_from_vec(svec!["deno", "test", "--no-npm", "--no-remote", "--trace-leaks", "--no-run", "--filter", "- foo", "--coverage=cov", "--clean", "--location", "https:foo", "--allow-net", "--permit-no-files", "dir1/", "dir2/", "--", "arg1", "arg2"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Test(TestFlags {
          no_run: true,
          doc: false,
          fail_fast: None,
          filter: Some("- foo".to_string()),
          permit_no_files: true,
          files: FileFlags {
            include: vec!["dir1/".to_string(), "dir2/".to_string()],
            ignore: vec![],
          },
          shuffle: None,
          parallel: false,
          trace_leaks: true,
          coverage_dir: Some("cov".to_string()),
          coverage_raw_data_only: false,
          clean: true,
          watch: Default::default(),
          reporter: Default::default(),
          junit_path: None,
          hide_stacktraces: false,
        }),
        no_npm: true,
        no_remote: true,
        location: Some(Url::parse("https://foo/").unwrap()),
        type_check_mode: TypeCheckMode::Local,
        permissions: PermissionFlags {
          no_prompt: true,
          allow_net: Some(vec![]),
          ..Default::default()
        },
        argv: svec!["arg1", "arg2"],
        ..Flags::default()
      }
    );
  }

  #[test]
  fn run_with_cafile() {
    let r = flags_from_vec(svec![
      "deno",
      "run",
      "--cert",
      "example.crt",
      "script.ts"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "script.ts".to_string(),
        )),
        ca_data: Some(CaData::File("example.crt".to_owned())),
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn run_with_base64_cafile() {
    let r = flags_from_vec(svec![
      "deno",
      "run",
      "--cert",
      "base64:bWVvdw==",
      "script.ts"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "script.ts".to_string(),
        )),
        ca_data: Some(CaData::Bytes(b"meow".into())),
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn run_with_enable_testing_features() {
    let r = flags_from_vec(svec![
      "deno",
      "run",
      "--enable-testing-features-do-not-use",
      "script.ts"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "script.ts".to_string(),
        )),
        enable_testing_features: true,
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn test_with_fail_fast() {
    let r = flags_from_vec(svec!["deno", "test", "--fail-fast=3"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Test(TestFlags {
          no_run: false,
          doc: false,
          fail_fast: Some(NonZeroUsize::new(3).unwrap()),
          filter: None,
          permit_no_files: false,
          shuffle: None,
          files: FileFlags {
            include: vec![],
            ignore: vec![],
          },
          parallel: false,
          trace_leaks: false,
          coverage_dir: None,
          coverage_raw_data_only: false,
          clean: false,
          watch: Default::default(),
          reporter: Default::default(),
          junit_path: None,
          hide_stacktraces: false,
        }),
        type_check_mode: TypeCheckMode::Local,
        permissions: PermissionFlags {
          no_prompt: true,
          ..Default::default()
        },
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec!["deno", "test", "--fail-fast=0"]);
    assert!(r.is_err());
  }

  #[test]
  fn test_with_enable_testing_features() {
    let r = flags_from_vec(svec![
      "deno",
      "test",
      "--enable-testing-features-do-not-use"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Test(TestFlags {
          no_run: false,
          doc: false,
          fail_fast: None,
          filter: None,
          permit_no_files: false,
          shuffle: None,
          files: FileFlags {
            include: vec![],
            ignore: vec![],
          },
          parallel: false,
          trace_leaks: false,
          coverage_dir: None,
          coverage_raw_data_only: false,
          clean: false,
          watch: Default::default(),
          reporter: Default::default(),
          junit_path: None,
          hide_stacktraces: false,
        }),
        permissions: PermissionFlags {
          no_prompt: true,
          ..Default::default()
        },
        type_check_mode: TypeCheckMode::Local,
        enable_testing_features: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn test_reporter() {
    let r = flags_from_vec(svec!["deno", "test", "--reporter=pretty"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Test(TestFlags {
          reporter: TestReporterConfig::Pretty,
          ..Default::default()
        }),
        permissions: PermissionFlags {
          no_prompt: true,
          ..Default::default()
        },
        type_check_mode: TypeCheckMode::Local,
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec!["deno", "test", "--reporter=dot"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Test(TestFlags {
          reporter: TestReporterConfig::Dot,
          ..Default::default()
        }),
        permissions: PermissionFlags {
          no_prompt: true,
          ..Default::default()
        },
        type_check_mode: TypeCheckMode::Local,
        log_level: Some(Level::Error),
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec!["deno", "test", "--reporter=junit"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Test(TestFlags {
          reporter: TestReporterConfig::Junit,
          ..Default::default()
        }),
        permissions: PermissionFlags {
          no_prompt: true,
          ..Default::default()
        },
        type_check_mode: TypeCheckMode::Local,
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec!["deno", "test", "--reporter=tap"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Test(TestFlags {
          reporter: TestReporterConfig::Tap,
          ..Default::default()
        }),
        permissions: PermissionFlags {
          no_prompt: true,
          ..Default::default()
        },
        type_check_mode: TypeCheckMode::Local,
        log_level: Some(Level::Error),
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec![
      "deno",
      "test",
      "--reporter=dot",
      "--junit-path=report.xml"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Test(TestFlags {
          reporter: TestReporterConfig::Dot,
          junit_path: Some("report.xml".to_string()),
          ..Default::default()
        }),
        permissions: PermissionFlags {
          no_prompt: true,
          ..Default::default()
        },
        type_check_mode: TypeCheckMode::Local,
        log_level: Some(Level::Error),
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec!["deno", "test", "--junit-path"]);
    assert!(r.is_err());
  }

  #[test]
  fn test_shuffle() {
    let r = flags_from_vec(svec!["deno", "test", "--shuffle=1"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Test(TestFlags {
          no_run: false,
          doc: false,
          fail_fast: None,
          filter: None,
          permit_no_files: false,
          shuffle: Some(1),
          files: FileFlags {
            include: vec![],
            ignore: vec![],
          },
          parallel: false,
          trace_leaks: false,
          coverage_dir: None,
          coverage_raw_data_only: false,
          clean: false,
          watch: Default::default(),
          reporter: Default::default(),
          junit_path: None,
          hide_stacktraces: false,
        }),
        permissions: PermissionFlags {
          no_prompt: true,
          ..Default::default()
        },
        type_check_mode: TypeCheckMode::Local,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn test_watch() {
    let r = flags_from_vec(svec!["deno", "test", "--watch"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Test(TestFlags {
          no_run: false,
          doc: false,
          fail_fast: None,
          filter: None,
          permit_no_files: false,
          shuffle: None,
          files: FileFlags {
            include: vec![],
            ignore: vec![],
          },
          parallel: false,
          trace_leaks: false,
          coverage_dir: None,
          coverage_raw_data_only: false,
          clean: false,
          watch: Some(Default::default()),
          reporter: Default::default(),
          junit_path: None,
          hide_stacktraces: false,
        }),
        permissions: PermissionFlags {
          no_prompt: true,
          ..Default::default()
        },
        type_check_mode: TypeCheckMode::Local,
        ..Flags::default()
      }
    );
  }
  #[test]
  fn test_watch_explicit_cwd() {
    let r = flags_from_vec(svec!["deno", "test", "--watch", "./"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Test(TestFlags {
          no_run: false,
          doc: false,
          fail_fast: None,
          filter: None,
          permit_no_files: false,
          shuffle: None,
          files: FileFlags {
            include: vec!["./".to_string()],
            ignore: vec![],
          },
          parallel: false,
          trace_leaks: false,
          coverage_dir: None,
          coverage_raw_data_only: false,
          clean: false,
          watch: Some(Default::default()),
          reporter: Default::default(),
          junit_path: None,
          hide_stacktraces: false,
        }),
        permissions: PermissionFlags {
          no_prompt: true,
          ..Default::default()
        },
        type_check_mode: TypeCheckMode::Local,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn test_watch_with_no_clear_screen() {
    let r =
      flags_from_vec(svec!["deno", "test", "--watch", "--no-clear-screen"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Test(TestFlags {
          no_run: false,
          doc: false,
          fail_fast: None,
          filter: None,
          permit_no_files: false,
          shuffle: None,
          files: FileFlags {
            include: vec![],
            ignore: vec![],
          },
          parallel: false,
          trace_leaks: false,
          coverage_dir: None,
          coverage_raw_data_only: false,
          clean: false,
          watch: Some(WatchFlagsWithPaths {
            hmr: false,
            no_clear_screen: true,
            exclude: vec![],
            paths: vec![],
          }),
          reporter: Default::default(),
          junit_path: None,
          hide_stacktraces: false,
        }),
        type_check_mode: TypeCheckMode::Local,
        permissions: PermissionFlags {
          no_prompt: true,
          ..Default::default()
        },
        ..Flags::default()
      }
    );
  }

  #[test]
  fn test_watch_with_paths() {
    let r = flags_from_vec(svec!("deno", "test", "--watch=foo"));

    let flags = r.unwrap();
    assert_eq!(
      flags,
      Flags {
        subcommand: DenoSubcommand::Test(TestFlags {
          watch: Some(WatchFlagsWithPaths {
            hmr: false,
            paths: vec![String::from("foo")],
            no_clear_screen: false,
            exclude: vec![],
          }),
          ..TestFlags::default()
        }),
        type_check_mode: TypeCheckMode::Local,
        permissions: PermissionFlags {
          no_prompt: true,
          ..Default::default()
        },
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec!["deno", "test", "--watch=foo,bar"]);

    let flags = r.unwrap();
    assert_eq!(
      flags,
      Flags {
        subcommand: DenoSubcommand::Test(TestFlags {
          watch: Some(WatchFlagsWithPaths {
            hmr: false,
            paths: vec![String::from("foo"), String::from("bar")],
            no_clear_screen: false,
            exclude: vec![],
          }),
          ..TestFlags::default()
        }),
        type_check_mode: TypeCheckMode::Local,
        permissions: PermissionFlags {
          no_prompt: true,
          ..Default::default()
        },
        ..Flags::default()
      }
    );
  }

  #[test]
  fn test_watch_with_excluded_paths() {
    let r =
      flags_from_vec(svec!("deno", "test", "--watch", "--watch-exclude=foo",));

    let flags = r.unwrap();
    assert_eq!(
      flags,
      Flags {
        subcommand: DenoSubcommand::Test(TestFlags {
          watch: Some(WatchFlagsWithPaths {
            hmr: false,
            paths: vec![],
            no_clear_screen: false,
            exclude: vec![String::from("foo")],
          }),
          ..TestFlags::default()
        }),
        type_check_mode: TypeCheckMode::Local,
        permissions: PermissionFlags {
          no_prompt: true,
          ..Default::default()
        },
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec!(
      "deno",
      "test",
      "--watch=foo",
      "--watch-exclude=bar",
    ));
    let flags = r.unwrap();
    assert_eq!(
      flags,
      Flags {
        subcommand: DenoSubcommand::Test(TestFlags {
          watch: Some(WatchFlagsWithPaths {
            hmr: false,
            paths: vec![String::from("foo")],
            no_clear_screen: false,
            exclude: vec![String::from("bar")],
          }),
          ..TestFlags::default()
        }),
        type_check_mode: TypeCheckMode::Local,
        permissions: PermissionFlags {
          no_prompt: true,
          ..Default::default()
        },
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec![
      "deno",
      "test",
      "--watch",
      "--watch-exclude=foo,bar",
    ]);

    let flags = r.unwrap();
    assert_eq!(
      flags,
      Flags {
        subcommand: DenoSubcommand::Test(TestFlags {
          watch: Some(WatchFlagsWithPaths {
            hmr: false,
            paths: vec![],
            no_clear_screen: false,
            exclude: vec![String::from("foo"), String::from("bar")],
          }),
          ..TestFlags::default()
        }),
        type_check_mode: TypeCheckMode::Local,
        permissions: PermissionFlags {
          no_prompt: true,
          ..Default::default()
        },
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec![
      "deno",
      "test",
      "--watch=foo,bar",
      "--watch-exclude=baz,qux",
    ]);

    let flags = r.unwrap();
    assert_eq!(
      flags,
      Flags {
        subcommand: DenoSubcommand::Test(TestFlags {
          watch: Some(WatchFlagsWithPaths {
            hmr: false,
            paths: vec![String::from("foo"), String::from("bar")],
            no_clear_screen: false,
            exclude: vec![String::from("baz"), String::from("qux"),],
          }),
          ..TestFlags::default()
        }),
        type_check_mode: TypeCheckMode::Local,
        permissions: PermissionFlags {
          no_prompt: true,
          ..Default::default()
        },
        ..Flags::default()
      }
    );
  }

  #[test]
  fn test_coverage_default_dir() {
    let r = flags_from_vec(svec!["deno", "test", "--coverage"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Test(TestFlags {
          coverage_dir: Some("coverage".to_string()),
          ..TestFlags::default()
        }),
        type_check_mode: TypeCheckMode::Local,
        permissions: PermissionFlags {
          no_prompt: true,
          ..Default::default()
        },
        ..Flags::default()
      }
    );
  }

  #[test]
  fn test_hide_stacktraces() {
    let r = flags_from_vec(svec!["deno", "test", "--hide-stacktraces"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Test(TestFlags {
          hide_stacktraces: true,
          ..TestFlags::default()
        }),
        type_check_mode: TypeCheckMode::Local,
        permissions: PermissionFlags {
          no_prompt: true,
          ..Default::default()
        },
        ..Flags::default()
      }
    );
  }

  #[test]
  fn upgrade_with_ca_file() {
    let r = flags_from_vec(svec!["deno", "upgrade", "--cert", "example.crt"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Upgrade(UpgradeFlags {
          force: false,
          dry_run: false,
          canary: false,
          release_candidate: false,
          version: None,
          output: None,
          version_or_hash_or_channel: None,
          checksum: None,
          pr: None,
          branch: None,
        }),
        ca_data: Some(CaData::File("example.crt".to_owned())),
        ..Flags::default()
      }
    );
  }

  #[test]
  fn upgrade_release_candidate() {
    let r = flags_from_vec(svec!["deno", "upgrade", "--rc"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Upgrade(UpgradeFlags {
          force: false,
          dry_run: false,
          canary: false,
          release_candidate: true,
          version: None,
          output: None,
          version_or_hash_or_channel: None,
          checksum: None,
          pr: None,
          branch: None,
        }),
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec!["deno", "upgrade", "--rc", "--canary"]);
    assert!(r.is_err());

    let r = flags_from_vec(svec!["deno", "upgrade", "--rc", "--version"]);
    assert!(r.is_err());
  }

  #[test]
  fn upgrade_pr() {
    let r = flags_from_vec(svec!["deno", "upgrade", "pr", "12345"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Upgrade(UpgradeFlags {
          force: false,
          dry_run: false,
          canary: false,
          release_candidate: false,
          version: None,
          output: None,
          version_or_hash_or_channel: None,
          checksum: None,
          pr: Some(12345),
          branch: None,
        }),
        ..Flags::default()
      }
    );
  }

  #[test]
  fn upgrade_pr_with_hash_prefix() {
    let r = flags_from_vec(svec!["deno", "upgrade", "pr", "#6789"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Upgrade(UpgradeFlags {
          force: false,
          dry_run: false,
          canary: false,
          release_candidate: false,
          version: None,
          output: None,
          version_or_hash_or_channel: None,
          checksum: None,
          pr: Some(6789),
          branch: None,
        }),
        ..Flags::default()
      }
    );
  }

  #[test]
  fn upgrade_pr_with_flags() {
    let r =
      flags_from_vec(svec!["deno", "upgrade", "--dry-run", "pr", "33250"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Upgrade(UpgradeFlags {
          force: false,
          dry_run: true,
          canary: false,
          release_candidate: false,
          version: None,
          output: None,
          version_or_hash_or_channel: None,
          checksum: None,
          pr: Some(33250),
          branch: None,
        }),
        ..Flags::default()
      }
    );
  }

  #[test]
  fn upgrade_pr_missing_number() {
    let r = flags_from_vec(svec!["deno", "upgrade", "pr"]);
    assert!(r.is_err());
  }

  #[test]
  fn upgrade_pr_invalid_number() {
    let r = flags_from_vec(svec!["deno", "upgrade", "pr", "abc"]);
    assert!(r.is_err());
  }

  #[test]
  fn cache_with_cafile() {
    let r = flags_from_vec(svec![
      "deno",
      "cache",
      "--cert",
      "example.crt",
      "script.ts",
      "script_two.ts"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Cache(CacheFlags {
          files: svec!["script.ts", "script_two.ts"],
        }),
        ca_data: Some(CaData::File("example.crt".to_owned())),
        ..Flags::default()
      }
    );
  }

  #[test]
  fn info_with_cafile() {
    let r = flags_from_vec(svec![
      "deno",
      "info",
      "--cert",
      "example.crt",
      "https://example.com"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Info(InfoFlags {
          json: false,
          file: Some("https://example.com".to_string()),
        }),
        ca_data: Some(CaData::File("example.crt".to_owned())),
        ..Flags::default()
      }
    );
  }

  #[test]
  fn doc() {
    let r = flags_from_vec(svec!["deno", "doc", "--json", "path/to/module.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Doc(DocFlags {
          private: false,
          json: true,
          html: None,
          lint: false,
          source_files: DocSourceFileFlag::Paths(svec!["path/to/module.ts"]),
          filter: None,
        }),
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec!["deno", "doc", "--html", "path/to/module.ts"]);
    assert!(r.is_ok());

    let r = flags_from_vec(svec![
      "deno",
      "doc",
      "--html",
      "--name=My library",
      "path/to/module.ts"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Doc(DocFlags {
          private: false,
          json: false,
          lint: false,
          html: Some(DocHtmlFlag {
            name: Some("My library".to_string()),
            category_docs_path: None,
            symbol_redirect_map_path: None,
            default_symbol_map_path: None,
            strip_trailing_html: false,
            output: String::from("./docs/"),
          }),
          source_files: DocSourceFileFlag::Paths(svec!["path/to/module.ts"]),
          filter: None,
        }),
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec![
      "deno",
      "doc",
      "--html",
      "--name=My library",
      "--lint",
      "--output=./foo",
      "path/to/module.ts"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Doc(DocFlags {
          private: false,
          json: false,
          html: Some(DocHtmlFlag {
            name: Some("My library".to_string()),
            category_docs_path: None,
            symbol_redirect_map_path: None,
            default_symbol_map_path: None,
            strip_trailing_html: false,
            output: String::from("./foo"),
          }),
          lint: true,
          source_files: DocSourceFileFlag::Paths(svec!["path/to/module.ts"]),
          filter: None,
        }),
        ..Flags::default()
      }
    );

    let r =
      flags_from_vec(svec!["deno", "doc", "--html", "--name=My library",]);
    assert!(r.is_err());

    let r = flags_from_vec(svec![
      "deno",
      "doc",
      "--filter",
      "SomeClass.someField",
      "path/to/module.ts",
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Doc(DocFlags {
          private: false,
          json: false,
          html: None,
          lint: false,
          source_files: DocSourceFileFlag::Paths(vec![
            "path/to/module.ts".to_string()
          ]),
          filter: Some("SomeClass.someField".to_string()),
        }),
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec!["deno", "doc"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Doc(DocFlags {
          private: false,
          json: false,
          html: None,
          lint: false,
          source_files: Default::default(),
          filter: None,
        }),
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec![
      "deno",
      "doc",
      "--filter",
      "Deno.Listener",
      "--builtin"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Doc(DocFlags {
          private: false,
          lint: false,
          json: false,
          html: None,
          source_files: DocSourceFileFlag::Builtin,
          filter: Some("Deno.Listener".to_string()),
        }),
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec![
      "deno",
      "doc",
      "--no-npm",
      "--no-remote",
      "--private",
      "path/to/module.js"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Doc(DocFlags {
          private: true,
          lint: false,
          json: false,
          html: None,
          source_files: DocSourceFileFlag::Paths(svec!["path/to/module.js"]),
          filter: None,
        }),
        no_npm: true,
        no_remote: true,
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec![
      "deno",
      "doc",
      "path/to/module.js",
      "path/to/module2.js"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Doc(DocFlags {
          private: false,
          lint: false,
          json: false,
          html: None,
          source_files: DocSourceFileFlag::Paths(vec![
            "path/to/module.js".to_string(),
            "path/to/module2.js".to_string()
          ]),
          filter: None,
        }),
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec![
      "deno",
      "doc",
      "path/to/module.js",
      "--builtin",
      "path/to/module2.js"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Doc(DocFlags {
          private: false,
          json: false,
          html: None,
          lint: false,
          source_files: DocSourceFileFlag::Paths(vec![
            "path/to/module.js".to_string(),
            "path/to/module2.js".to_string()
          ]),
          filter: None,
        }),
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec!["deno", "doc", "--lint",]);
    assert!(r.is_err());

    let r = flags_from_vec(svec![
      "deno",
      "doc",
      "--lint",
      "path/to/module.js",
      "path/to/module2.js"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Doc(DocFlags {
          private: false,
          lint: true,
          json: false,
          html: None,
          source_files: DocSourceFileFlag::Paths(vec![
            "path/to/module.js".to_string(),
            "path/to/module2.js".to_string()
          ]),
          filter: None,
        }),
        ..Flags::default()
      }
    );
  }

  #[test]
  fn inspect_default_host() {
    let r = flags_from_vec(svec!["deno", "run", "--inspect", "foo.js"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "foo.js".to_string(),
        )),
        inspect: Some("127.0.0.1:9229".parse().unwrap()),
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn inspect_wait() {
    let r = flags_from_vec(svec!["deno", "--inspect-wait", "foo.js"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags {
          script: "foo.js".to_string(),
          watch: None,
          bare: true,
          coverage_dir: None,
          print_task_list: false,
        }),
        inspect_wait: Some("127.0.0.1:9229".parse().unwrap()),
        code_cache_enabled: true,
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec![
      "deno",
      "run",
      "--inspect-wait=127.0.0.1:3567",
      "foo.js"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "foo.js".to_string(),
        )),
        inspect_wait: Some("127.0.0.1:3567".parse().unwrap()),
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn compile() {
    let r = flags_from_vec(svec![
      "deno",
      "compile",
      "https://examples.deno.land/color-logging.ts"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Compile(CompileFlags {
          source_file: "https://examples.deno.land/color-logging.ts"
            .to_string(),
          output: None,
          args: vec![],
          target: None,
          no_terminal: false,
          icon: None,
          include: Default::default(),
          exclude: Default::default(),
          eszip: false,
          self_extracting: false,
        }),
        type_check_mode: TypeCheckMode::Local,
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn compile_with_flags() {
    #[rustfmt::skip]
    let r = flags_from_vec(svec!["deno", "compile", "--include", "include.txt", "--exclude", "exclude.txt", "--import-map", "import_map.json", "--no-code-cache", "--no-remote", "--config", "tsconfig.json", "--no-check", "--unsafely-ignore-certificate-errors", "--reload", "--lock", "lock.json", "--cert", "example.crt", "--cached-only", "--location", "https:foo", "--allow-read", "--allow-net", "--v8-flags=--help", "--seed", "1", "--no-terminal", "--icon", "favicon.ico", "--output", "colors", "--env=.example.env", "https://examples.deno.land/color-logging.ts", "foo", "bar", "-p", "8080"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Compile(CompileFlags {
          source_file: "https://examples.deno.land/color-logging.ts"
            .to_string(),
          output: Some(String::from("colors")),
          args: svec!["foo", "bar", "-p", "8080"],
          target: None,
          no_terminal: true,
          icon: Some(String::from("favicon.ico")),
          include: vec!["include.txt".to_string()],
          exclude: vec!["exclude.txt".to_string()],
          eszip: false,
          self_extracting: false,
        }),
        import_map_path: Some("import_map.json".to_string()),
        no_remote: true,
        code_cache_enabled: false,
        config_flag: ConfigFlag::Path("tsconfig.json".to_owned()),
        type_check_mode: TypeCheckMode::None,
        reload: true,
        lock: Some(String::from("lock.json")),
        ca_data: Some(CaData::File("example.crt".to_string())),
        cached_only: true,
        location: Some(Url::parse("https://foo/").unwrap()),
        permissions: PermissionFlags {
          allow_read: Some(vec![]),
          allow_net: Some(vec![]),
          ..Default::default()
        },
        unsafely_ignore_certificate_errors: Some(vec![]),
        v8_flags: svec!["--help", "--random-seed=1"],
        seed: Some(1),
        env_file: Some(vec![".example.env".to_owned()]),
        ..Flags::default()
      }
    );
  }

  #[test]
  fn coverage() {
    let r = flags_from_vec(svec!["deno", "coverage", "foo.json"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Coverage(CoverageFlags {
          files: FileFlags {
            include: vec!["foo.json".to_string()],
            ignore: vec![],
          },
          include: vec![r"^file:".to_string()],
          exclude: vec![r"test\.(js|mjs|ts|jsx|tsx)$".to_string()],
          ..CoverageFlags::default()
        }),
        ..Flags::default()
      }
    );
  }

  #[test]
  fn coverage_with_lcov_and_out_file() {
    let r = flags_from_vec(svec![
      "deno",
      "coverage",
      "--lcov",
      "--output=foo.lcov",
      "foo.json"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Coverage(CoverageFlags {
          files: FileFlags {
            include: vec!["foo.json".to_string()],
            ignore: vec![],
          },
          include: vec![r"^file:".to_string()],
          exclude: vec![r"test\.(js|mjs|ts|jsx|tsx)$".to_string()],
          r#type: CoverageType::Lcov,
          output: Some(String::from("foo.lcov")),
        }),
        ..Flags::default()
      }
    );
  }

  #[test]
  fn coverage_with_default_files() {
    let r = flags_from_vec(svec!["deno", "coverage",]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Coverage(CoverageFlags {
          files: FileFlags {
            include: vec!["coverage".to_string()],
            ignore: vec![],
          },
          include: vec![r"^file:".to_string()],
          exclude: vec![r"test\.(js|mjs|ts|jsx|tsx)$".to_string()],
          ..CoverageFlags::default()
        }),
        ..Flags::default()
      }
    );
  }

  #[test]
  fn location_with_bad_scheme() {
    #[rustfmt::skip]
    let r = flags_from_vec(svec!["deno", "run", "--location", "foo:", "mod.ts"]);
    assert!(r.is_err());
    assert!(
      r.unwrap_err()
        .to_string()
        .contains("Expected protocol \"http\" or \"https\"")
    );
  }

  #[test]
  fn test_config_path_args() {
    let flags = flags_from_vec(svec!["deno", "run", "foo.js"]).unwrap();
    let cwd = resolve_cwd(None).unwrap().into_owned();

    assert_eq!(flags.config_path_args(&cwd), Some(vec![cwd.clone()]));

    let flags = flags_from_vec(svec!["deno", "run", "sub_dir/foo.js"]).unwrap();
    let cwd = resolve_cwd(None).unwrap().into_owned();
    assert_eq!(
      flags.config_path_args(&cwd),
      Some(vec![cwd.join("sub_dir").clone()])
    );

    let flags =
      flags_from_vec(svec!["deno", "https://example.com/foo.js"]).unwrap();
    assert_eq!(flags.config_path_args(&cwd), None);

    let flags =
      flags_from_vec(svec!["deno", "lint", "dir/a/a.js", "dir/b/b.js"])
        .unwrap();
    assert_eq!(
      flags.config_path_args(&cwd),
      Some(vec![cwd.join("dir/a/a.js"), cwd.join("dir/b/b.js")])
    );

    let flags = flags_from_vec(svec!["deno", "lint"]).unwrap();
    assert_eq!(flags.config_path_args(&cwd), Some(vec![cwd.clone()]));

    let flags = flags_from_vec(svec!["deno", "cache", "sub/test.js"]).unwrap();
    assert_eq!(flags.config_path_args(&cwd), Some(vec![cwd.join("sub")]));

    let flags = flags_from_vec(svec!["deno", "cache", "."]).unwrap();
    assert_eq!(flags.config_path_args(&cwd), Some(vec![cwd.clone()]));

    let flags =
      flags_from_vec(svec!["deno", "install", "-e", "sub/test.js"]).unwrap();
    assert_eq!(flags.config_path_args(&cwd), Some(vec![cwd.join("sub")]));

    let flags = flags_from_vec(svec![
      "deno",
      "fmt",
      "dir/a/a.js",
      "dir/a/a2.js",
      "dir/b.js"
    ])
    .unwrap();
    assert_eq!(
      flags.config_path_args(&cwd),
      Some(vec![
        cwd.join("dir/a/a.js"),
        cwd.join("dir/a/a2.js"),
        cwd.join("dir/b.js")
      ])
    );
  }

  #[test]
  fn test_no_clear_watch_flag_without_watch_flag() {
    let r = flags_from_vec(svec!["deno", "run", "--no-clear-screen", "foo.js"]);
    assert!(r.is_err());
    let error_message = r.unwrap_err().to_string();
    assert!(
      &error_message
        .contains("error: the following required arguments were not provided:")
    );
    assert!(
      error_message.contains("--watch")
        || error_message.contains("--no-clear-screen"),
      "error should mention watch dependency: {error_message}"
    );
  }

  #[test]
  fn task_subcommand() {
    let r = flags_from_vec(svec!["deno", "task", "build", "hello", "world",]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Task(TaskFlags {
          cwd: None,
          task: Some("build".to_string()),
          is_run: false,
          recursive: false,
          filter: None,
          eval: false,
        }),
        argv: svec!["hello", "world"],
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec!["deno", "task", "build"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Task(TaskFlags {
          cwd: None,
          task: Some("build".to_string()),
          is_run: false,
          recursive: false,
          filter: None,
          eval: false,
        }),
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec!["deno", "task", "--cwd", "foo", "build"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Task(TaskFlags {
          cwd: Some("foo".to_string()),
          task: Some("build".to_string()),
          is_run: false,
          recursive: false,
          filter: None,
          eval: false,
        }),
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec!["deno", "task", "--filter", "*", "build"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Task(TaskFlags {
          cwd: None,
          task: Some("build".to_string()),
          is_run: false,
          recursive: false,
          filter: Some("*".to_string()),
          eval: false,
        }),
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec!["deno", "task", "--recursive", "build"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Task(TaskFlags {
          cwd: None,
          task: Some("build".to_string()),
          is_run: false,
          recursive: true,
          filter: Some("*".to_string()),
          eval: false,
        }),
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec!["deno", "task", "-r", "build"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Task(TaskFlags {
          cwd: None,
          task: Some("build".to_string()),
          is_run: false,
          recursive: true,
          filter: Some("*".to_string()),
          eval: false,
        }),
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec!["deno", "task", "--eval", "echo 1"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Task(TaskFlags {
          cwd: None,
          task: Some("echo 1".to_string()),
          is_run: false,
          recursive: false,
          filter: None,
          eval: true,
        }),
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec!["deno", "task", "--eval"]);
    assert!(r.is_err());
  }

  #[test]
  fn task_subcommand_double_hyphen() {
    let r = flags_from_vec(svec![
      "deno",
      "task",
      "-c",
      "deno.json",
      "build",
      "--",
      "hello",
      "world",
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Task(TaskFlags {
          cwd: None,
          task: Some("build".to_string()),
          is_run: false,
          recursive: false,
          filter: None,
          eval: false,
        }),
        argv: svec!["--", "hello", "world"],
        config_flag: ConfigFlag::Path("deno.json".to_owned()),
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec![
      "deno", "task", "--cwd", "foo", "build", "--", "hello", "world"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Task(TaskFlags {
          cwd: Some("foo".to_string()),
          task: Some("build".to_string()),
          is_run: false,
          recursive: false,
          filter: None,
          eval: false,
        }),
        argv: svec!["--", "hello", "world"],
        ..Flags::default()
      }
    );
  }

  #[test]
  fn task_subcommand_double_hyphen_only() {
    // edge case, but it should forward
    let r = flags_from_vec(svec!["deno", "task", "build", "--"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Task(TaskFlags {
          cwd: None,
          task: Some("build".to_string()),
          is_run: false,
          recursive: false,
          filter: None,
          eval: false,
        }),
        argv: svec!["--"],
        ..Flags::default()
      }
    );
  }

  #[test]
  fn task_following_arg() {
    let r = flags_from_vec(svec!["deno", "task", "build", "-1", "--test"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Task(TaskFlags {
          cwd: None,
          task: Some("build".to_string()),
          is_run: false,
          recursive: false,
          filter: None,
          eval: false,
        }),
        argv: svec!["-1", "--test"],
        ..Flags::default()
      }
    );
  }

  #[test]
  fn task_following_double_hyphen_arg() {
    let r = flags_from_vec(svec!["deno", "task", "build", "--test"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Task(TaskFlags {
          cwd: None,
          task: Some("build".to_string()),
          is_run: false,
          recursive: false,
          filter: None,
          eval: false,
        }),
        argv: svec!["--test"],
        ..Flags::default()
      }
    );
  }

  #[test]
  fn task_with_global_flags() {
    // can fail if the custom parser in task_parse() starts at the wrong index
    let r = flags_from_vec(svec!["deno", "--quiet", "task", "build"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Task(TaskFlags {
          cwd: None,
          task: Some("build".to_string()),
          is_run: false,
          recursive: false,
          filter: None,
          eval: false,
        }),
        log_level: Some(log::Level::Error),
        ..Flags::default()
      }
    );
  }

  #[test]
  fn task_subcommand_empty() {
    let r = flags_from_vec(svec!["deno", "task"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Task(TaskFlags {
          cwd: None,
          task: None,
          is_run: false,
          recursive: false,
          filter: None,
          eval: false,
        }),
        ..Flags::default()
      }
    );
  }

  #[test]
  fn task_subcommand_config() {
    let r = flags_from_vec(svec!["deno", "task", "--config", "deno.jsonc"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Task(TaskFlags {
          cwd: None,
          task: None,
          is_run: false,
          recursive: false,
          filter: None,
          eval: false,
        }),
        config_flag: ConfigFlag::Path("deno.jsonc".to_string()),
        ..Flags::default()
      }
    );
  }

  #[test]
  fn task_subcommand_config_short() {
    let r = flags_from_vec(svec!["deno", "task", "-c", "deno.jsonc"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Task(TaskFlags {
          cwd: None,
          task: None,
          is_run: false,
          recursive: false,
          filter: None,
          eval: false,
        }),
        config_flag: ConfigFlag::Path("deno.jsonc".to_string()),
        ..Flags::default()
      }
    );
  }

  #[test]
  fn task_subcommand_noconfig_invalid() {
    let r = flags_from_vec(svec!["deno", "task", "--no-config"]);
    assert!(r.is_err());
  }

  #[test]
  fn bench_with_flags() {
    let r = flags_from_vec(svec![
      "deno",
      "bench",
      "--json",
      "--no-npm",
      "--no-remote",
      "--no-run",
      "--filter",
      "- foo",
      "--location",
      "https:foo",
      "--allow-net",
      "dir1/",
      "dir2/",
      "--",
      "arg1",
      "arg2"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Bench(BenchFlags {
          filter: Some("- foo".to_string()),
          json: true,
          no_run: true,
          files: FileFlags {
            include: vec!["dir1/".to_string(), "dir2/".to_string()],
            ignore: vec![],
          },
          watch: Default::default(),
          permit_no_files: false,
        }),
        no_npm: true,
        no_remote: true,
        type_check_mode: TypeCheckMode::Local,
        location: Some(Url::parse("https://foo/").unwrap()),
        permissions: PermissionFlags {
          allow_net: Some(vec![]),
          no_prompt: true,
          ..Default::default()
        },
        argv: svec!["arg1", "arg2"],
        ..Flags::default()
      }
    );
  }

  #[test]
  fn bench_watch() {
    let r = flags_from_vec(svec!["deno", "bench", "--watch"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Bench(BenchFlags {
          filter: None,
          json: false,
          no_run: false,
          files: FileFlags {
            include: vec![],
            ignore: vec![],
          },
          watch: Some(Default::default()),
          permit_no_files: false
        }),
        permissions: PermissionFlags {
          no_prompt: true,
          ..Default::default()
        },
        type_check_mode: TypeCheckMode::Local,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn bench_no_files() {
    let r = flags_from_vec(svec!["deno", "bench", "--permit-no-files"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Bench(BenchFlags {
          filter: None,
          json: false,
          no_run: false,
          files: FileFlags {
            include: vec![],
            ignore: vec![],
          },
          watch: None,
          permit_no_files: true
        }),
        permissions: PermissionFlags {
          no_prompt: true,
          ..Default::default()
        },
        type_check_mode: TypeCheckMode::Local,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn run_with_check() {
    let r = flags_from_vec(svec!["deno", "run", "--check", "script.ts",]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "script.ts".to_string(),
        )),
        type_check_mode: TypeCheckMode::Local,
        code_cache_enabled: true,
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec!["deno", "run", "--check=all", "script.ts",]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "script.ts".to_string(),
        )),
        type_check_mode: TypeCheckMode::All,
        code_cache_enabled: true,
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec!["deno", "--check=foo", "script.ts",]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags {
          script: "script.ts".to_string(),
          watch: None,
          bare: true,
          coverage_dir: None,
          print_task_list: false,
        }),
        type_check_mode: TypeCheckMode::None,
        code_cache_enabled: true,
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec![
      "deno",
      "run",
      "--no-check",
      "--check",
      "script.ts",
    ]);
    assert!(r.is_err());
  }

  #[test]
  fn no_config() {
    let r = flags_from_vec(svec!["deno", "run", "--no-config", "script.ts",]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "script.ts".to_string(),
        )),
        config_flag: ConfigFlag::Disabled,
        code_cache_enabled: true,
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec![
      "deno",
      "run",
      "--config",
      "deno.json",
      "--no-config",
      "script.ts",
    ]);
    assert!(r.is_err());
  }

  #[test]
  fn init() {
    let r = flags_from_vec(svec!["deno", "init"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Init(InitFlags {
          package: None,
          package_args: vec![],
          dir: None,
          lib: false,
          serve: false,
          empty: false,
          yes: false,
        }),
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec!["deno", "init", "foo"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Init(InitFlags {
          package: None,
          package_args: vec![],
          dir: Some(String::from("foo")),
          lib: false,
          serve: false,
          empty: false,
          yes: false,
        }),
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec!["deno", "init", "--quiet"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Init(InitFlags {
          package: None,
          package_args: vec![],
          dir: None,
          lib: false,
          serve: false,
          empty: false,
          yes: false,
        }),
        log_level: Some(Level::Error),
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec!["deno", "init", "--lib"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Init(InitFlags {
          package: None,
          package_args: vec![],
          dir: None,
          lib: true,
          serve: false,
          empty: false,
          yes: false,
        }),
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec!["deno", "init", "--serve"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Init(InitFlags {
          package: None,
          package_args: vec![],
          dir: None,
          lib: false,
          serve: true,
          empty: false,
          yes: false,
        }),
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec!["deno", "init", "foo", "--lib"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Init(InitFlags {
          package: None,
          package_args: vec![],
          dir: Some(String::from("foo")),
          lib: true,
          serve: false,
          empty: false,
          yes: false,
        }),
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec!["deno", "init", "--lib", "--npm", "vite"]);
    assert!(r.is_err());

    let r = flags_from_vec(svec!["deno", "init", "--serve", "--npm", "vite"]);
    assert!(r.is_err());

    let r = flags_from_vec(svec!["deno", "init", "--npm", "vite", "--lib"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Init(InitFlags {
          package: Some("npm:vite".to_string()),
          package_args: svec!["--lib"],
          dir: None,
          lib: false,
          serve: false,
          empty: false,
          yes: false,
        }),
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec!["deno", "init", "--npm", "vite", "--serve"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Init(InitFlags {
          package: Some("npm:vite".to_string()),
          package_args: svec!["--serve"],
          dir: None,
          lib: false,
          serve: false,
          empty: false,
          yes: false,
        }),
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec!["deno", "init", "--npm", "vite", "new_dir"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Init(InitFlags {
          package: Some("npm:vite".to_string()),
          package_args: svec!["new_dir"],
          dir: None,
          lib: false,
          serve: false,
          empty: false,
          yes: false,
        }),
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec!["deno", "init", "--npm", "--yes", "npm:vite"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Init(InitFlags {
          package: Some("npm:vite".to_string()),
          package_args: vec![],
          dir: None,
          lib: false,
          serve: false,
          empty: false,
          yes: true,
        }),
        ..Flags::default()
      }
    );

    // --jsr basic
    let r = flags_from_vec(svec!["deno", "init", "--jsr", "@denotest/create"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Init(InitFlags {
          package: Some("jsr:@denotest/create".to_string()),
          package_args: vec![],
          dir: None,
          lib: false,
          serve: false,
          empty: false,
          yes: false,
        }),
        ..Flags::default()
      }
    );

    // --jsr with jsr: prefix already present
    let r = flags_from_vec(svec!["deno", "init", "--jsr", "jsr:@fresh/init"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Init(InitFlags {
          package: Some("jsr:@fresh/init".to_string()),
          package_args: vec![],
          dir: None,
          lib: false,
          serve: false,
          empty: false,
          yes: false,
        }),
        ..Flags::default()
      }
    );

    // --jsr with --yes
    let r = flags_from_vec(svec![
      "deno",
      "init",
      "--jsr",
      "--yes",
      "@denotest/create"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Init(InitFlags {
          package: Some("jsr:@denotest/create".to_string()),
          package_args: vec![],
          dir: None,
          lib: false,
          serve: false,
          empty: false,
          yes: true,
        }),
        ..Flags::default()
      }
    );

    // --jsr with extra args
    let r = flags_from_vec(svec![
      "deno",
      "init",
      "--jsr",
      "@denotest/create",
      "my-project"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Init(InitFlags {
          package: Some("jsr:@denotest/create".to_string()),
          package_args: svec!["my-project"],
          dir: None,
          lib: false,
          serve: false,
          empty: false,
          yes: false,
        }),
        ..Flags::default()
      }
    );

    // --jsr conflicts with --npm, --lib, --serve, --empty
    let r = flags_from_vec(svec!["deno", "init", "--jsr", "--npm", "@foo/bar"]);
    assert!(r.is_err());

    let r = flags_from_vec(svec!["deno", "init", "--jsr", "--lib", "@foo/bar"]);
    assert!(r.is_err());

    // --jsr without package name
    let r = flags_from_vec(svec!["deno", "init", "--jsr"]);
    assert!(r.is_err());
  }

  #[test]
  fn create() {
    let r = flags_from_vec(svec!["deno", "create"]);
    assert!(r.is_err());

    let r = flags_from_vec(svec!["deno", "create", "vite"]);
    assert!(r.is_err());

    let r = flags_from_vec(svec!["deno", "create", "npm:vite", "my-project"]);
    assert!(r.is_err());

    let r =
      flags_from_vec(svec!["deno", "create", "npm:vite", "--", "my-project"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Init(InitFlags {
          package: Some("npm:vite".to_string()),
          package_args: svec!["my-project"],
          dir: None,
          lib: false,
          serve: false,
          empty: false,
          yes: false,
        }),
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec!["deno", "create", "--npm", "vite"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Init(InitFlags {
          package: Some("npm:vite".to_string()),
          package_args: vec![],
          dir: None,
          lib: false,
          serve: false,
          empty: false,
          yes: false,
        }),
        ..Flags::default()
      }
    );

    let r =
      flags_from_vec(svec!["deno", "create", "--npm", "vite", "my-project"]);
    assert!(r.is_err());

    let r = flags_from_vec(svec!["deno", "create", "--yes", "npm:vite"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Init(InitFlags {
          package: Some("npm:vite".to_string()),
          package_args: vec![],
          dir: None,
          lib: false,
          serve: false,
          empty: false,
          yes: true,
        }),
        ..Flags::default()
      }
    );

    let r =
      flags_from_vec(svec!["deno", "create", "jsr:@std/http/file-server"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Init(InitFlags {
          package: Some("jsr:@std/http/file-server".to_string()),
          package_args: vec![],
          dir: None,
          lib: false,
          serve: false,
          empty: false,
          yes: false,
        }),
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec!["deno", "create", "jsr:@fresh/init"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Init(InitFlags {
          package: Some("jsr:@fresh/init".to_string()),
          package_args: vec![],
          dir: None,
          lib: false,
          serve: false,
          empty: false,
          yes: false,
        }),
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec!["deno", "create", "--yes", "jsr:@fresh/init"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Init(InitFlags {
          package: Some("jsr:@fresh/init".to_string()),
          package_args: vec![],
          dir: None,
          lib: false,
          serve: false,
          empty: false,
          yes: true,
        }),
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec![
      "deno",
      "create",
      "jsr:@fresh/init",
      "--",
      "my-project"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Init(InitFlags {
          package: Some("jsr:@fresh/init".to_string()),
          package_args: svec!["my-project"],
          dir: None,
          lib: false,
          serve: false,
          empty: false,
          yes: false,
        }),
        ..Flags::default()
      }
    );

    // empty jsr: prefix
    let r = flags_from_vec(svec!["deno", "create", "jsr:"]);
    assert!(r.is_err());

    // --jsr flag
    let r = flags_from_vec(svec!["deno", "create", "--jsr", "@fresh/init"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Init(InitFlags {
          package: Some("jsr:@fresh/init".to_string()),
          package_args: vec![],
          dir: None,
          lib: false,
          serve: false,
          empty: false,
          yes: false,
        }),
        ..Flags::default()
      }
    );

    // --jsr with --yes
    let r =
      flags_from_vec(svec!["deno", "create", "--jsr", "--yes", "@fresh/init"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Init(InitFlags {
          package: Some("jsr:@fresh/init".to_string()),
          package_args: vec![],
          dir: None,
          lib: false,
          serve: false,
          empty: false,
          yes: true,
        }),
        ..Flags::default()
      }
    );

    // --jsr with npm: specifier is contradictory
    let r = flags_from_vec(svec!["deno", "create", "--jsr", "npm:vite"]);
    assert!(r.is_err());

    // --jsr and --npm conflict
    let r = flags_from_vec(svec!["deno", "create", "--jsr", "--npm", "@foo"]);
    assert!(r.is_err());

    let r = flags_from_vec(svec!["deno", "create", "npm:"]);
    assert!(r.is_err());

    // --npm with jsr: is contradictory
    let r = flags_from_vec(svec!["deno", "create", "--npm", "jsr:@std/http"]);
    assert!(r.is_err());
  }

  #[test]
  fn jupyter() {
    let r = flags_from_vec(svec!["deno", "jupyter"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Jupyter(JupyterFlags {
          install: false,
          kernel: false,
          conn_file: None,
          name: None,
          display: None,
          force: false,
        }),
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec!["deno", "jupyter", "--install"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Jupyter(JupyterFlags {
          install: true,
          kernel: false,
          conn_file: None,
          name: None,
          display: None,
          force: false,
        }),
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec!["deno", "jupyter", "--install", "--force"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Jupyter(JupyterFlags {
          install: true,
          kernel: false,
          conn_file: None,
          name: None,
          display: None,
          force: true,
        }),
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec![
      "deno",
      "jupyter",
      "--install",
      "--name",
      "debugdeno",
      "--display",
      "Deno (debug)"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Jupyter(JupyterFlags {
          install: true,
          kernel: false,
          conn_file: None,
          name: Some("debugdeno".to_string()),
          display: Some("Deno (debug)".to_string()),
          force: false,
        }),
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec!["deno", "jupyter", "-n", "debugdeno",]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Jupyter(JupyterFlags {
          install: false,
          kernel: false,
          conn_file: None,
          name: Some("debugdeno".to_string()),
          display: None,
          force: false,
        }),
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec![
      "deno",
      "jupyter",
      "--kernel",
      "--conn",
      "path/to/conn/file"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Jupyter(JupyterFlags {
          install: false,
          kernel: true,
          conn_file: Some(String::from("path/to/conn/file")),
          name: None,
          display: None,
          force: false,
        }),
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec![
      "deno",
      "jupyter",
      "--install",
      "--conn",
      "path/to/conn/file"
    ]);
    r.unwrap_err();
    let r = flags_from_vec(svec!["deno", "jupyter", "--kernel",]);
    r.unwrap_err();
    let r = flags_from_vec(svec!["deno", "jupyter", "--install", "--kernel",]);
    r.unwrap_err();
    let r = flags_from_vec(svec!["deno", "jupyter", "--display", "deno"]);
    r.unwrap_err();
    let r = flags_from_vec(svec!["deno", "jupyter", "--kernel", "--display"]);
    r.unwrap_err();
    let r = flags_from_vec(svec!["deno", "jupyter", "--force"]);
    r.unwrap_err();
  }

  #[test]
  fn publish_args() {
    let r = flags_from_vec(svec![
      "deno",
      "publish",
      "--no-provenance",
      "--dry-run",
      "--allow-slow-types",
      "--allow-dirty",
      "--token=asdf",
      "--set-version=1.0.1",
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Publish(PublishFlags {
          token: Some("asdf".to_string()),
          dry_run: true,
          allow_slow_types: true,
          allow_dirty: true,
          no_provenance: true,
          set_version: Some("1.0.1".to_string()),
        }),
        type_check_mode: TypeCheckMode::Local,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn add_or_install_subcommand() {
    let r = flags_from_vec(svec!["deno", "add"]);
    r.unwrap_err();
    for cmd in ["add", "install"] {
      let mk_flags = |flags: AddFlags| -> Flags {
        match cmd {
          "add" => Flags {
            subcommand: DenoSubcommand::Add(flags),
            ..Flags::default()
          },
          "install" => Flags {
            subcommand: DenoSubcommand::Install(InstallFlags::Local(
              InstallFlagsLocal::Add(flags),
            )),
            ..Flags::default()
          },
          _ => unreachable!(),
        }
      };

      {
        let r = flags_from_vec(svec!["deno", cmd, "@david/which"]);
        assert_eq!(
          r.unwrap(),
          mk_flags(AddFlags {
            packages: svec!["@david/which"],
            dev: false, // default is false
            default_registry: None,
            lockfile_only: false,
            save_exact: false,
          })
        );
      }
      {
        let r = flags_from_vec(svec![
          "deno",
          cmd,
          "--frozen",
          "--lockfile-only",
          "@david/which",
          "@luca/hello"
        ]);
        let mut expected_flags = mk_flags(AddFlags {
          packages: svec!["@david/which", "@luca/hello"],
          dev: false,
          default_registry: None,
          lockfile_only: true,
          save_exact: false,
        });
        expected_flags.frozen_lockfile = Some(true);
        assert_eq!(r.unwrap(), expected_flags);
      }
      {
        let r = flags_from_vec(svec!["deno", cmd, "--dev", "npm:chalk"]);
        assert_eq!(
          r.unwrap(),
          mk_flags(AddFlags {
            packages: svec!["npm:chalk"],
            dev: true,
            default_registry: None,
            lockfile_only: false,
            save_exact: false,
          }),
        );
      }
      {
        let r = flags_from_vec(svec!["deno", cmd, "--npm", "chalk"]);
        assert_eq!(
          r.unwrap(),
          mk_flags(AddFlags {
            packages: svec!["chalk"],
            dev: false,
            default_registry: Some(DefaultRegistry::Npm),
            lockfile_only: false,
            save_exact: false,
          }),
        );
      }
      {
        let r = flags_from_vec(svec!["deno", cmd, "--jsr", "@std/fs"]);
        assert_eq!(
          r.unwrap(),
          mk_flags(AddFlags {
            packages: svec!["@std/fs"],
            dev: false,
            default_registry: Some(DefaultRegistry::Jsr),
            lockfile_only: false,
            save_exact: false,
          }),
        );
      }
    }
  }

  #[test]
  fn remove_subcommand() {
    let r = flags_from_vec(svec!["deno", "remove"]);
    r.unwrap_err();

    let r = flags_from_vec(svec!["deno", "remove", "@david/which"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Remove(RemoveFlags {
          packages: svec!["@david/which"],
          lockfile_only: false,
        }),
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec![
      "deno",
      "remove",
      "--frozen",
      "--lockfile-only",
      "@david/which",
      "@luca/hello"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Remove(RemoveFlags {
          packages: svec!["@david/which", "@luca/hello"],
          lockfile_only: true,
        }),
        frozen_lockfile: Some(true),
        ..Flags::default()
      }
    );
  }

  #[test]
  fn run_with_frozen_lockfile() {
    let cases = [
      (Some("--frozen"), Some(true)),
      (Some("--frozen=true"), Some(true)),
      (Some("--frozen=false"), Some(false)),
      (None, None),
    ];
    for (flag, frozen) in cases {
      let mut args = svec!["deno", "run"];
      if let Some(f) = flag {
        args.push(f.into());
      }
      args.push("script.ts".into());
      let r = flags_from_vec(args);
      assert_eq!(
        r.unwrap(),
        Flags {
          subcommand: DenoSubcommand::Run(RunFlags::new_default(
            "script.ts".to_string(),
          )),
          frozen_lockfile: frozen,
          code_cache_enabled: true,
          ..Flags::default()
        }
      );
    }
  }

  #[test]
  fn allow_scripts() {
    let cases = [
      (Some("--allow-scripts"), Ok(PackagesAllowedScripts::All)),
      (None, Ok(PackagesAllowedScripts::None)),
      (
        Some("--allow-scripts=npm:foo"),
        Ok(PackagesAllowedScripts::Some(vec![
          PackageReq::from_str("foo").unwrap(),
        ])),
      ),
      (
        Some("--allow-scripts=npm:foo,npm:bar@2"),
        Ok(PackagesAllowedScripts::Some(vec![
          PackageReq::from_str("foo").unwrap(),
          PackageReq::from_str("bar@2").unwrap(),
        ])),
      ),
      (Some("--allow-scripts=foo"), Err("Invalid package")),
      (
        Some("--allow-scripts=npm:foo@next"),
        Err("Tags are not supported in --allow-scripts: npm:foo@next"),
      ),
      (
        Some("--allow-scripts=jsr:@foo/bar"),
        Err("An 'npm:' specifier is required"),
      ),
    ];
    for (flag, value) in cases {
      let mut args = svec!["deno", "cache"];
      if let Some(flag) = flag {
        args.push(flag.into());
      }
      args.push("script.ts".into());
      let r = flags_from_vec(args);
      match value {
        Ok(value) => {
          assert_eq!(
            r.unwrap(),
            Flags {
              subcommand: DenoSubcommand::Cache(CacheFlags {
                files: svec!["script.ts"],
              }),
              allow_scripts: value,
              ..Flags::default()
            }
          );
        }
        Err(e) => {
          let err = r.unwrap_err();
          assert!(
            err.to_string().contains(e),
            "expected to contain '{e}' got '{err}'"
          );
        }
      }
    }
  }

  #[test]
  fn bare_run() {
    let r = flags_from_vec(svec!["deno", "--no-config", "script.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags {
          script: "script.ts".to_string(),
          watch: None,
          bare: true,
          coverage_dir: None,
          print_task_list: false,
        }),
        config_flag: ConfigFlag::Disabled,
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn bare_global() {
    let r = flags_from_vec(svec!["deno", "--log-level=debug"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Repl(ReplFlags {
          eval_files: None,
          eval: None,
          is_default_command: true,
          json: false,
        }),
        log_level: Some(Level::Debug),
        permissions: PermissionFlags {
          allow_all: true,
          ..Default::default()
        },
        ..Flags::default()
      }
    );
  }

  #[test]
  fn repl_user_args() {
    let r = flags_from_vec(svec!["deno", "repl", "foo"]);
    assert!(r.is_err());
    let r = flags_from_vec(svec!["deno", "repl", "--", "foo"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Repl(ReplFlags {
          eval_files: None,
          eval: None,
          is_default_command: false,
          json: false,
        }),
        argv: svec!["foo"],
        ..Flags::default()
      }
    );
  }

  #[test]
  fn bare_with_flag_no_file() {
    let r = flags_from_vec(svec!["deno", "--no-config"]);

    let err = r.unwrap_err();
    assert!(err.to_string().contains("error: [SCRIPT_ARG] may only be omitted with --v8-flags=--help, else to use the repl with arguments, please use the `deno repl` subcommand"));
    assert!(
      err
        .to_string()
        .contains("Usage: deno [OPTIONS] [COMMAND] [SCRIPT_ARG]...")
    );
  }

  #[test]
  fn equal_help_output() {
    // Verify --help and -h produce the same output for all subcommands
    for sub in deno_cli_parser::defs::DENO_ROOT.subcommands {
      if sub.name == "help"
        || sub.name == "json_reference"
        || sub.name == "deploy"
        || sub.name == "sandbox"
      {
        continue;
      }

      let long_flag = match flags_from_vec(svec!["deno", sub.name, "--help"])
        .unwrap()
        .subcommand
      {
        DenoSubcommand::Help(help) => help.help,
        _ => {
          unreachable!("{} --help should produce Help", sub.name)
        }
      };
      let short_flag = match flags_from_vec(svec!["deno", sub.name, "-h"])
        .unwrap()
        .subcommand
      {
        DenoSubcommand::Help(help) => help.help,
        _ => {
          unreachable!("{} -h should produce Help", sub.name)
        }
      };
      assert_eq!(long_flag, short_flag, "{} subcommand", sub.name);
    }
  }

  #[test]
  fn install_permissions_non_global() {
    let r =
      flags_from_vec(svec!["deno", "install", "--allow-net", "jsr:@std/fs"]);

    assert!(
      r.unwrap_err().to_string().contains(
        "Note: Permission flags can only be used in a global setting"
      )
    );
  }

  #[test]
  fn jupyter_unstable_flags() {
    let r = flags_from_vec(svec![
      "deno",
      "jupyter",
      "--unstable-ffi",
      "--unstable-bare-node-builtins",
      "--unstable-worker-options"
    ]);

    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Jupyter(JupyterFlags {
          install: false,
          kernel: false,
          conn_file: None,
          name: None,
          display: None,
          force: false,
        }),
        unstable_config: UnstableConfig {
          bare_node_builtins: true,
          sloppy_imports: false,
          features: svec!["bare-node-builtins", "ffi", "worker-options"],
          ..Default::default()
        },
        ..Flags::default()
      }
    );
  }

  #[test]
  fn serve_with_allow_all() {
    let r = flags_from_vec(svec!["deno", "serve", "--allow-all", "./main.ts"]);
    let flags = r.unwrap();
    assert_eq!(
      &flags,
      &Flags {
        subcommand: DenoSubcommand::Serve(ServeFlags::new_default(
          "./main.ts".into(),
          8000,
          "0.0.0.0"
        )),
        permissions: PermissionFlags {
          allow_all: true,
          allow_net: None,
          ..Default::default()
        },
        code_cache_enabled: true,
        ..Default::default()
      }
    );
  }

  #[test]
  fn escape_and_split_commas_test() {
    assert_eq!(escape_and_split_commas("foo".to_string()).unwrap(), ["foo"]);
    assert!(escape_and_split_commas("foo,".to_string()).is_err());
    assert_eq!(
      escape_and_split_commas("foo,,".to_string()).unwrap(),
      ["foo,"]
    );
    assert!(escape_and_split_commas("foo,,,".to_string()).is_err());
    assert_eq!(
      escape_and_split_commas("foo,,,,".to_string()).unwrap(),
      ["foo,,"]
    );
    assert_eq!(
      escape_and_split_commas("foo,bar".to_string()).unwrap(),
      ["foo", "bar"]
    );
    assert_eq!(
      escape_and_split_commas("foo,,bar".to_string()).unwrap(),
      ["foo,bar"]
    );
    assert_eq!(
      escape_and_split_commas("foo,,,bar".to_string()).unwrap(),
      ["foo,", "bar"]
    );
  }

  #[test]
  fn net_flag_with_url() {
    let r = flags_from_vec(svec![
      "deno",
      "run",
      "--allow-net=https://example.com",
      "script.ts"
    ]);
    assert_eq!(
      r.unwrap_err().to_string(),
      "error: invalid value 'https://example.com': URLs are not supported, only domains and ips"
    );
  }

  #[test]
  fn node_modules_dir_default() {
    let r =
      flags_from_vec(svec!["deno", "run", "--node-modules-dir", "./foo.ts"]);
    let flags = r.unwrap();
    assert_eq!(
      flags,
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags {
          script: "./foo.ts".into(),
          ..Default::default()
        }),
        node_modules_dir: Some(NodeModulesDirMode::Auto),
        code_cache_enabled: true,
        ..Default::default()
      }
    )
  }

  #[test]
  fn flag_before_subcommand() {
    let r = flags_from_vec(svec!["deno", "--allow-net", "repl"]);
    let err = r.unwrap_err().to_string();
    assert!(
      err.contains("--allow-net"),
      "error should mention the flag: {err}"
    );
    assert!(
      err.contains("repl") || err.contains("'repl --allow-net' exists"),
      "error should mention the subcommand: {err}"
    );
  }

  #[test]
  fn allow_all_conflicts_allow_perms() {
    let flags = [
      "--allow-read",
      "--allow-write",
      "--allow-net",
      "--allow-env",
      "--allow-run",
      "--allow-sys",
      "--allow-ffi",
      "--allow-import",
    ];
    for flag in flags {
      let r =
        flags_from_vec(svec!["deno", "run", "--allow-all", flag, "foo.ts"]);
      assert!(r.is_err());
    }
  }

  #[test]
  fn allow_import_with_url() {
    let r = flags_from_vec(svec![
      "deno",
      "run",
      "--allow-import=https://example.com",
      "script.ts"
    ]);
    assert_eq!(
      r.unwrap_err().to_string(),
      "error: invalid value 'https://example.com': URLs are not supported, only domains and ips"
    );
  }

  #[test]
  fn deny_import_with_url() {
    let r = flags_from_vec(svec![
      "deno",
      "run",
      "--deny-import=https://example.com",
      "script.ts",
    ]);
    assert_eq!(
      r.unwrap_err().to_string(),
      "error: invalid value 'https://example.com': URLs are not supported, only domains and ips"
    );
  }

  #[test]
  fn outdated_subcommand() {
    let cases = [
      (
        svec![],
        OutdatedFlags {
          filters: vec![],
          kind: OutdatedKind::PrintOutdated { compatible: false },
          recursive: false,
        },
      ),
      (
        svec!["--recursive"],
        OutdatedFlags {
          filters: vec![],
          kind: OutdatedKind::PrintOutdated { compatible: false },
          recursive: true,
        },
      ),
      (
        svec!["--recursive", "--compatible"],
        OutdatedFlags {
          filters: vec![],
          kind: OutdatedKind::PrintOutdated { compatible: true },
          recursive: true,
        },
      ),
      (
        svec!["--update"],
        OutdatedFlags {
          filters: vec![],
          kind: OutdatedKind::Update {
            latest: false,
            interactive: false,
            lockfile_only: false,
          },
          recursive: false,
        },
      ),
      (
        svec!["--update", "--latest"],
        OutdatedFlags {
          filters: vec![],
          kind: OutdatedKind::Update {
            latest: true,
            interactive: false,
            lockfile_only: false,
          },
          recursive: false,
        },
      ),
      (
        svec!["--update", "--recursive"],
        OutdatedFlags {
          filters: vec![],
          kind: OutdatedKind::Update {
            latest: false,
            interactive: false,
            lockfile_only: false,
          },
          recursive: true,
        },
      ),
      (
        svec!["--update", "--lockfile-only"],
        OutdatedFlags {
          filters: vec![],
          kind: OutdatedKind::Update {
            latest: false,
            interactive: false,
            lockfile_only: true,
          },
          recursive: false,
        },
      ),
      (
        svec!["--update", "@foo/bar"],
        OutdatedFlags {
          filters: svec!["@foo/bar"],
          kind: OutdatedKind::Update {
            latest: false,
            interactive: false,
            lockfile_only: false,
          },
          recursive: false,
        },
      ),
      (
        svec!["--latest"],
        OutdatedFlags {
          filters: svec![],
          kind: OutdatedKind::PrintOutdated { compatible: false },
          recursive: false,
        },
      ),
      (
        svec!["--update", "--latest", "--interactive"],
        OutdatedFlags {
          filters: svec![],
          kind: OutdatedKind::Update {
            latest: true,
            interactive: true,
            lockfile_only: false,
          },
          recursive: false,
        },
      ),
    ];
    for (input, expected) in cases {
      let mut args = svec!["deno", "outdated"];
      args.extend(input);
      let r = flags_from_vec(args.clone()).unwrap();
      assert_eq!(
        r.subcommand,
        DenoSubcommand::Outdated(expected),
        "incorrect result for args: {:?}",
        args
      );
    }
  }

  #[test]
  fn update_subcommand() {
    let cases = [
      (
        svec![],
        OutdatedFlags {
          filters: vec![],
          kind: OutdatedKind::Update {
            latest: false,
            interactive: false,
            lockfile_only: false,
          },
          recursive: false,
        },
      ),
      (
        svec!["--latest"],
        OutdatedFlags {
          filters: vec![],
          kind: OutdatedKind::Update {
            latest: true,
            interactive: false,
            lockfile_only: false,
          },
          recursive: false,
        },
      ),
      (
        svec!["--recursive"],
        OutdatedFlags {
          filters: vec![],
          kind: OutdatedKind::Update {
            latest: false,
            interactive: false,
            lockfile_only: false,
          },
          recursive: true,
        },
      ),
      (
        svec!["--lockfile-only"],
        OutdatedFlags {
          filters: vec![],
          kind: OutdatedKind::Update {
            latest: false,
            interactive: false,
            lockfile_only: true,
          },
          recursive: false,
        },
      ),
      (
        svec!["@foo/bar"],
        OutdatedFlags {
          filters: svec!["@foo/bar"],
          kind: OutdatedKind::Update {
            latest: false,
            interactive: false,
            lockfile_only: false,
          },
          recursive: false,
        },
      ),
      (
        svec!["--latest", "--interactive"],
        OutdatedFlags {
          filters: svec![],
          kind: OutdatedKind::Update {
            latest: true,
            interactive: true,
            lockfile_only: false,
          },
          recursive: false,
        },
      ),
    ];
    for (input, expected) in cases {
      let mut args = svec!["deno", "update"];
      args.extend(input);
      let r = flags_from_vec(args.clone()).unwrap();
      assert_eq!(
        r.subcommand,
        DenoSubcommand::Outdated(expected),
        "incorrect result for args: {:?}",
        args
      );
    }
  }

  #[test]
  fn update_subcommand_frozen_flag() {
    let r = flags_from_vec(svec!["deno", "update", "--frozen=false"]).unwrap();
    assert_eq!(r.frozen_lockfile, Some(false));

    let r = flags_from_vec(svec!["deno", "update", "--frozen"]).unwrap();
    assert_eq!(r.frozen_lockfile, Some(true));
  }

  #[test]
  fn outdated_subcommand_frozen_flag() {
    let r =
      flags_from_vec(svec!["deno", "outdated", "--frozen=false"]).unwrap();
    assert_eq!(r.frozen_lockfile, Some(false));
  }

  #[test]
  fn approve_scripts_subcommand() {
    let cases = [
      (
        svec![],
        ApproveScriptsFlags {
          packages: vec![],
          lockfile_only: false,
        },
      ),
      (
        svec!["npm:pkg@1.0.0"],
        ApproveScriptsFlags {
          packages: vec!["npm:pkg@1.0.0".to_string()],
          lockfile_only: false,
        },
      ),
      (
        svec!["npm:pkg1@1.0.0", "npm:pkg2@2.0.0"],
        ApproveScriptsFlags {
          packages: vec![
            "npm:pkg1@1.0.0".to_string(),
            "npm:pkg2@2.0.0".to_string(),
          ],
          lockfile_only: false,
        },
      ),
      (
        svec!["npm:pkg1@1.0.0,npm:pkg2@2.0.0"],
        ApproveScriptsFlags {
          packages: vec![
            "npm:pkg1@1.0.0".to_string(),
            "npm:pkg2@2.0.0".to_string(),
          ],
          lockfile_only: false,
        },
      ),
      (
        svec!["--lockfile-only"],
        ApproveScriptsFlags {
          packages: vec![],
          lockfile_only: true,
        },
      ),
      (
        svec!["--lockfile-only", "npm:pkg@1.0.0"],
        ApproveScriptsFlags {
          packages: vec!["npm:pkg@1.0.0".to_string()],
          lockfile_only: true,
        },
      ),
      (
        svec!["npm:pkg@1.0.0", "--lockfile-only"],
        ApproveScriptsFlags {
          packages: vec!["npm:pkg@1.0.0".to_string()],
          lockfile_only: true,
        },
      ),
      (
        svec!["npm:pkg1@1.0.0", "npm:pkg2@2.0.0", "--lockfile-only"],
        ApproveScriptsFlags {
          packages: vec![
            "npm:pkg1@1.0.0".to_string(),
            "npm:pkg2@2.0.0".to_string(),
          ],
          lockfile_only: true,
        },
      ),
    ];
    for (input, expected) in cases {
      let mut args = svec!["deno", "approve-scripts"];
      args.extend(input);
      let r = flags_from_vec(args.clone()).unwrap();
      assert_eq!(
        r.subcommand,
        DenoSubcommand::ApproveScripts(expected),
        "incorrect result for args: {:?}",
        args
      );
    }
  }

  #[test]
  fn clean_subcommand() {
    let cases = [
      (
        svec![],
        CleanFlags {
          except_paths: vec![],
          dry_run: false,
        },
      ),
      (
        svec!["--except", "path1"],
        CleanFlags {
          except_paths: vec!["path1".to_string()],
          dry_run: false,
        },
      ),
      (
        svec!["--except", "path1", "path2"],
        CleanFlags {
          except_paths: vec!["path1".to_string(), "path2".to_string()],
          dry_run: false,
        },
      ),
      (
        svec!["--except", "path1", "--dry-run"],
        CleanFlags {
          except_paths: vec!["path1".to_string()],
          dry_run: true,
        },
      ),
    ];
    for (input, expected) in cases {
      let cached_only = !input.is_empty();
      let mut args = svec!["deno", "clean"];
      args.extend(input);
      let r = flags_from_vec(args.clone())
        .inspect_err(|e| {
          #[allow(clippy::print_stderr, reason = "actually want to output")]
          {
            eprintln!("error: {:?} on input: {:?}", e, args);
          }
        })
        .unwrap();
      assert_eq!(
        r,
        Flags {
          subcommand: DenoSubcommand::Clean(expected),
          cached_only,
          ..Flags::default()
        },
        "incorrect result for args: {:?}",
        args
      );
    }
  }

  #[test]
  fn conditions_test() {
    let flags = flags_from_vec(svec![
      "deno",
      "run",
      "--conditions",
      "development",
      "main.ts"
    ])
    .unwrap();
    assert_eq!(
      flags,
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags {
          script: "main.ts".into(),
          ..Default::default()
        }),
        node_conditions: svec!["development"],
        code_cache_enabled: true,
        ..Default::default()
      }
    );

    let flags = flags_from_vec(svec![
      "deno",
      "run",
      "--conditions",
      "development,production",
      "main.ts"
    ])
    .unwrap();
    assert_eq!(
      flags,
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags {
          script: "main.ts".into(),
          ..Default::default()
        }),
        node_conditions: svec!["development", "production"],
        code_cache_enabled: true,
        ..Default::default()
      }
    );

    let flags = flags_from_vec(svec![
      "deno",
      "run",
      "--conditions",
      "development",
      "--conditions",
      "production",
      "main.ts"
    ])
    .unwrap();
    assert_eq!(
      flags,
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags {
          script: "main.ts".into(),
          ..Default::default()
        }),
        node_conditions: svec!["development", "production"],
        code_cache_enabled: true,
        ..Default::default()
      }
    );
  }

  #[test]
  fn preload_flag_test() {
    let flags = flags_from_vec(svec![
      "deno",
      "run",
      "--preload",
      "preload.js",
      "main.ts"
    ])
    .unwrap();
    assert_eq!(
      flags,
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags {
          script: "main.ts".into(),
          ..Default::default()
        }),
        preload: svec!["preload.js"],
        code_cache_enabled: true,
        ..Default::default()
      }
    );

    let flags =
      flags_from_vec(svec!["deno", "run", "--preload", "data:,()", "main.ts"])
        .unwrap();
    assert_eq!(
      flags,
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags {
          script: "main.ts".into(),
          ..Default::default()
        }),
        preload: svec!["data:,()"],
        code_cache_enabled: true,
        ..Default::default()
      }
    );

    let flags = flags_from_vec(svec![
      "deno",
      "compile",
      "--preload",
      "p1.js",
      "--preload",
      "./p2.js",
      "main.ts"
    ])
    .unwrap();
    assert_eq!(
      flags,
      Flags {
        subcommand: DenoSubcommand::Compile(CompileFlags {
          source_file: "main.ts".into(),
          output: None,
          args: vec![],
          target: None,
          no_terminal: false,
          icon: None,
          include: Default::default(),
          exclude: Default::default(),
          eszip: false,
          self_extracting: false,
        }),
        type_check_mode: TypeCheckMode::Local,
        preload: svec!["p1.js", "./p2.js"],
        code_cache_enabled: true,
        ..Default::default()
      }
    );

    let flags = flags_from_vec(svec![
      "deno",
      "test",
      "--preload",
      "p1.js",
      "--import",
      "./p2.js",
    ])
    .unwrap();
    assert_eq!(
      flags,
      Flags {
        subcommand: DenoSubcommand::Test(TestFlags::default()),
        preload: svec!["p1.js", "./p2.js"],
        type_check_mode: TypeCheckMode::Local,
        code_cache_enabled: false,
        permissions: PermissionFlags {
          no_prompt: true,
          ..Default::default()
        },
        ..Default::default()
      }
    );

    let flags = flags_from_vec(svec![
      "deno",
      "bench",
      "--preload",
      "p1.js",
      "--import",
      "./p2.js",
    ])
    .unwrap();
    assert_eq!(
      flags,
      Flags {
        subcommand: DenoSubcommand::Bench(BenchFlags::default()),
        preload: svec!["p1.js", "./p2.js"],
        type_check_mode: TypeCheckMode::Local,
        code_cache_enabled: false,
        permissions: PermissionFlags {
          no_prompt: true,
          ..Default::default()
        },
        ..Default::default()
      }
    );
  }

  #[test]
  fn require_flag_test() {
    let flags = flags_from_vec(svec![
      "deno",
      "run",
      "--require",
      "require.js",
      "main.ts"
    ])
    .unwrap();
    assert_eq!(
      flags,
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags {
          script: "main.ts".into(),
          ..Default::default()
        }),
        require: svec!["require.js"],
        code_cache_enabled: true,
        ..Default::default()
      }
    );

    let flags = flags_from_vec(svec![
      "deno",
      "run",
      "--require",
      "r1.js",
      "--require",
      "./r2.js",
      "main.ts"
    ])
    .unwrap();
    assert_eq!(
      flags,
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags {
          script: "main.ts".into(),
          ..Default::default()
        }),
        require: svec!["r1.js", "./r2.js"],
        code_cache_enabled: true,
        ..Default::default()
      }
    );
  }

  #[test]
  fn check_with_v8_flags() {
    let flags =
      flags_from_vec(svec!["deno", "check", "--v8-flags=--help", "script.ts",])
        .unwrap();
    assert_eq!(
      flags,
      Flags {
        subcommand: DenoSubcommand::Check(CheckFlags {
          files: svec!["script.ts"],
          doc: false,
          doc_only: false,
          check_js: false,
        }),
        type_check_mode: TypeCheckMode::Local,
        code_cache_enabled: true,
        v8_flags: svec!["--help"],
        ..Flags::default()
      }
    );
  }

  #[test]
  fn multiple_allow_all() {
    let flags = flags_from_vec(svec![
      "deno",
      "run",
      "--allow-all",
      "--inspect",
      "-A",
      "script.ts",
    ])
    .unwrap();
    assert_eq!(
      flags,
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags {
          script: "script.ts".to_string(),
          ..Default::default()
        }),
        inspect: Some("127.0.0.1:9229".parse().unwrap()),
        code_cache_enabled: true,
        permissions: PermissionFlags {
          allow_all: true,
          ..Default::default()
        },
        ..Flags::default()
      }
    );
  }

  #[test]
  fn inspect_flag_parsing() {
    use std::net::IpAddr;
    use std::net::Ipv4Addr;

    let cases = vec![
      (
        "127.0.0.1:9229",
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 9229),
      ),
      (
        "192.168.0.1",
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 0, 1)), 9229),
      ),
      (
        "10000",
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 10000),
      ),
      (
        ":10000",
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 10000),
      ),
      (
        ":0",
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 0),
      ),
      (
        "0",
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 0),
      ),
    ];

    for case in cases {
      let flags = flags_from_vec(svec![
        "deno",
        "run",
        &format!("--inspect={}", case.0),
        "script.ts",
      ])
      .unwrap();
      assert_eq!(
        flags,
        Flags {
          subcommand: DenoSubcommand::Run(RunFlags {
            script: "script.ts".to_string(),
            ..Default::default()
          }),
          inspect: Some(case.1),
          code_cache_enabled: true,
          ..Flags::default()
        }
      );
    }
  }

  #[test]
  fn inspect_publish_uid_flag_parsing() {
    // Test with both stderr and http
    let flags = flags_from_vec(svec![
      "deno",
      "run",
      "--inspect",
      "--inspect-publish-uid=stderr,http",
      "script.ts",
    ])
    .unwrap();
    assert_eq!(
      flags.inspect_publish_uid,
      Some(InspectPublishUid {
        console: true,
        http: true,
      })
    );

    // Test with only stderr
    let flags = flags_from_vec(svec![
      "deno",
      "run",
      "--inspect",
      "--inspect-publish-uid=stderr",
      "script.ts",
    ])
    .unwrap();
    assert_eq!(
      flags.inspect_publish_uid,
      Some(InspectPublishUid {
        console: true,
        http: false,
      })
    );

    // Test with only http
    let flags = flags_from_vec(svec![
      "deno",
      "run",
      "--inspect",
      "--inspect-publish-uid=http",
      "script.ts",
    ])
    .unwrap();
    assert_eq!(
      flags.inspect_publish_uid,
      Some(InspectPublishUid {
        console: false,
        http: true,
      })
    );

    // Test without the flag (should be None)
    let flags =
      flags_from_vec(svec!["deno", "run", "--inspect", "script.ts",]).unwrap();
    assert_eq!(flags.inspect_publish_uid, None);
  }

  fn set_test_node_options(value: Option<&str>) {
    TEST_NODE_OPTIONS.with(|opt| {
      *opt.borrow_mut() = value.map(|s| s.to_string());
    });
  }

  #[test]
  fn node_options_require() {
    // Test NODE_OPTIONS --require when no CLI --require is passed
    set_test_node_options(Some("--require only.js"));
    let flags = flags_from_vec(svec!["deno", "run", "script.ts",]).unwrap();
    set_test_node_options(None);
    assert_eq!(flags.require, vec!["only.js"]);
  }

  #[test]
  fn node_options_require_prepend_to_cli() {
    // Test NODE_OPTIONS --require is prepended to CLI --require values
    set_test_node_options(Some("--require foo.js --require bar.js"));
    let flags =
      flags_from_vec(svec!["deno", "run", "--require", "cli.js", "script.ts",])
        .unwrap();
    set_test_node_options(None);
    assert_eq!(flags.require, vec!["foo.js", "bar.js", "cli.js"]);
  }

  #[test]
  fn node_options_inspect_publish_uid() {
    set_test_node_options(Some("--inspect-publish-uid=http"));
    let flags = flags_from_vec(svec!["deno", "run", "script.ts",]).unwrap();
    set_test_node_options(None);
    assert_eq!(
      flags.inspect_publish_uid,
      Some(InspectPublishUid {
        console: false,
        http: true,
      })
    );
  }

  #[test]
  fn node_options_inspect_publish_uid_cli_precedence() {
    set_test_node_options(Some("--inspect-publish-uid=http"));
    let flags = flags_from_vec(svec![
      "deno",
      "run",
      "--inspect-publish-uid=stderr",
      "script.ts",
    ])
    .unwrap();
    set_test_node_options(None);
    assert_eq!(
      flags.inspect_publish_uid,
      Some(InspectPublishUid {
        console: true,
        http: false,
      })
    );
  }

  #[test]
  fn node_options_combined() {
    // Test NODE_OPTIONS with both --require and --inspect-publish-uid
    set_test_node_options(Some(
      "--require foo.js --inspect-publish-uid=stderr,http",
    ));
    let flags = flags_from_vec(svec!["deno", "run", "script.ts",]).unwrap();
    set_test_node_options(None);
    assert_eq!(flags.require, vec!["foo.js"]);
    assert_eq!(
      flags.inspect_publish_uid,
      Some(InspectPublishUid {
        console: true,
        http: true,
      })
    );
  }

  #[test]
  fn node_options_empty() {
    set_test_node_options(Some(""));
    let flags = flags_from_vec(svec!["deno", "run", "script.ts",]).unwrap();
    set_test_node_options(None);
    assert!(flags.require.is_empty());
    assert_eq!(flags.inspect_publish_uid, None);
  }

  #[test]
  fn node_options_ignores_unknown_flags() {
    set_test_node_options(Some(
      "--require known.js --unknown-flag --another-unknown",
    ));
    let flags = flags_from_vec(svec!["deno", "run", "script.ts",]).unwrap();
    set_test_node_options(None);
    assert_eq!(flags.require, vec!["known.js"]);
  }
}
