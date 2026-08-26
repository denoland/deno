// Copyright 2018-2026 the Deno authors. MIT license.

use std::borrow::Cow;
use std::collections::HashSet;
use std::env;
use std::ffi::OsString;
use std::path::Path;
use std::path::PathBuf;
use std::str::FromStr;

use color_print::cstr;
use deno_cli_parser::CliError;
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
pub use deno_runtime::deno_inspector_server::InspectPublishUid;
use deno_telemetry::OtelConfig;
use deno_telemetry::OtelConsoleConfig;
use deno_telemetry::OtelPropagators;

use crate::util::env::resolve_cwd;
use crate::util::fs::canonicalize_path;

// ---------------------------------------------------------------------------
// CLI-only helpers. These stay in the CLI because they depend on deno_graph /
// deno_telemetry / the cli build env, which the parser crate intentionally
// does not.
// ---------------------------------------------------------------------------

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

fn join_paths(allowlist: &[String], d: &str) -> String {
  allowlist
    .iter()
    .map(|path| path.to_string())
    .collect::<Vec<String>>()
    .join(d)
}

/// Resolve a subcommand's include/ignore globs against `base`.
pub fn resolve_file_patterns(
  files: &FileFlags,
  base: &Path,
) -> Result<FilePatterns, AnyError> {
  Ok(FilePatterns {
    include: if files.include.is_empty() {
      None
    } else {
      Some(PathOrPatternSet::from_include_relative_path_or_patterns(
        base,
        &files.include,
      )?)
    },
    exclude: PathOrPatternSet::from_exclude_relative_path_or_patterns(
      base,
      &files.ignore,
    )?,
    base: base.to_path_buf(),
  })
}

/// The `--target` triple for `deno compile`, defaulting to the triple this
/// binary was built for.
pub fn resolve_compile_target(flags: &CompileFlags) -> String {
  flags
    .target
    .clone()
    .unwrap_or_else(|| env!("TARGET").to_string())
}

fn npm_system_info_from_env() -> NpmSystemInfo {
  let arch = std::env::var_os("DENO_INSTALL_ARCH");
  if let Some(var) = arch.as_ref().and_then(|s| s.to_str()) {
    NpmSystemInfo::from_rust(std::env::consts::OS, var)
  } else {
    NpmSystemInfo::default()
  }
}

/// The npm platform/arch a subcommand should resolve packages for. `compile`
/// and `desktop` follow their `--target`; `install` follows `--os`/`--arch`;
/// everything else uses the current system.
pub fn npm_system_info(subcommand: &DenoSubcommand) -> NpmSystemInfo {
  match subcommand {
    DenoSubcommand::Compile(CompileFlags {
      target: Some(target),
      ..
    })
    | DenoSubcommand::Desktop(DesktopFlags {
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
    DenoSubcommand::Install(InstallFlags::Local(
      _,
      NpmInstallTargetFlags { os, arch },
    )) => {
      let default = npm_system_info_from_env();
      NpmSystemInfo {
        os: os.as_deref().unwrap_or(&default.os).into(),
        cpu: arch.as_deref().unwrap_or(&default.cpu).into(),
      }
    }
    _ => npm_system_info_from_env(),
  }
}

/// The module `GraphKind` that should be created for a `TypeCheckMode`.
pub fn graph_kind(mode: TypeCheckMode) -> GraphKind {
  match mode.is_true() {
    true => GraphKind::All,
    false => GraphKind::CodeOnly,
  }
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
          || module_specifier.scheme() == "jsr"
        {
          if let Ok(p) = url_to_file_path(&module_specifier) {
            maybe_resolve_directory(p)
          } else {
            Some(current_dir.to_path_buf())
          }
        } else {
          // When the entrypoint is a remote script (e.g. https:), then we
          // don't auto discover the config file. Package entrypoints (npm:,
          // jsr:) resolve within the project context, so they do.
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
      Desktop(DesktopFlags {
        source_file: script,
        ..
      }) => resolve_single_folder_path(script, current_dir, |p| {
        // The desktop entrypoint is commonly a directory (e.g. `deno desktop
        // .` for framework detection), in which case the config lives in that
        // directory itself — don't pop up to its parent. Only pop when the
        // entrypoint is a file.
        if p.is_dir() {
          Some(p)
        } else {
          p.parent().map(|parent| parent.to_path_buf())
        }
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
      | Install(InstallFlags::Local(
        InstallFlagsLocal::Entrypoints(InstallEntrypointsFlags {
          entrypoints: files,
          ..
        }),
        _,
      )) => Some(vec![
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
    match &self.watch {
      Some(WatchFlagsWithPaths {
        exclude: excluded_paths,
        ..
      }) => {
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

pub fn flags_from_vec(args: Vec<OsString>) -> Result<Flags, CliError> {
  flags_from_vec_with_initial_cwd(args, None)
}

/// Strip a single trailing carriage return from an argument.
///
/// When a script with CRLF line endings starts with a shebang like
/// `#!/usr/bin/env -S deno run --allow-net`, the kernel passes the trailing
/// `\r` as part of the final argument (e.g. `--allow-net\r`), which then
/// fails clap parsing with a confusing "isn't valid in this context" error.
/// Trimming a trailing CR here makes such scripts run as the author intended.
fn strip_trailing_cr(arg: OsString) -> OsString {
  let bytes = arg.as_encoded_bytes();
  if bytes.ends_with(b"\r") {
    let stripped = bytes[..bytes.len() - 1].to_vec();
    // SAFETY: `\r` is the single ASCII byte 0x0D; removing an ASCII byte from
    // the end of a valid OS-string byte sequence (UTF-8 on Unix, WTF-8 on
    // Windows) cannot split a multi-byte code point and keeps the encoding
    // valid.
    unsafe { OsString::from_encoded_bytes_unchecked(stripped) }
  } else {
    arg
  }
}

/// Fast path for `deno run <file> [script args...]` with no deno-level flags
/// before the file. Building the parse tree / walking the parser costs a couple
/// microseconds; for the overwhelmingly common bare-run case we skip it and
/// build `RunFlags` directly. Any deno flag before the file (anything starting
/// with `-`, or `-` itself) falls through to the full parser. `args` must be
/// already `\r`-stripped and dx-shimmed. Returns `None` when the fast path does
/// not apply. Must stay output-identical to the full parse (see the
/// `fast_path_matches_full_parse` test).
fn bare_run_fast_path(args: &[OsString]) -> Option<Flags> {
  if args.len() >= 3
    && args[1] == "run"
    && args[2] != "-"
    && args[2].as_encoded_bytes().first() != Some(&b'-')
    && let Some(script) = args[2].to_str()
  {
    let argv = args[3..]
      .iter()
      .map(|a| a.to_string_lossy().into_owned())
      .collect::<Vec<_>>();
    let mut flags = Flags {
      subcommand: DenoSubcommand::Run(RunFlags {
        script: script.to_string(),
        bare: false,
        coverage_dir: None,
        print_task_list: false,
      }),
      argv,
      // Matches the full `run_parse`: no `--no-code-cache` flag before the
      // file resolves code caching to enabled.
      code_cache_enabled: true,
      ..Default::default()
    };
    deno_cli_parser::convert::apply_node_options(&mut flags);
    Some(flags)
  } else {
    None
  }
}

/// Main entry point for parsing deno's command line flags. Routes through the
/// hand-written `deno_cli_parser`; the `Flags` type is shared between this
/// crate and the parser crate (`pub use deno_cli_parser::flags::*`), so no
/// conversion is needed.
pub fn flags_from_vec_with_initial_cwd(
  args: Vec<OsString>,
  initial_cwd: Option<PathBuf>,
) -> Result<Flags, CliError> {
  // Strip a trailing `\r` from each arg so a CRLF-shebang script isn't poisoned
  // by a stray carriage return. The hand-written parser also does this
  // internally; doing it here first keeps the fast path below consistent with
  // the full parse. (`deno_cli_parser::convert::flags_from_vec` re-strips, which
  // is a no-op on already-stripped args.)
  let args: Vec<OsString> = args.into_iter().map(strip_trailing_cr).collect();

  // dx/denox/dnx shim: rewrite the binary name into an explicit `x` subcommand
  // before parsing (the hand-written parser doesn't special-case argv[0]).
  let args = if !args.is_empty()
    && (args[0].as_encoded_bytes().ends_with(b"dx")
      || args[0].as_encoded_bytes().ends_with(b"denox")
      || args[0].as_encoded_bytes().ends_with(b"dnx"))
  {
    let mut new_args = Vec::with_capacity(args.len() + 1);
    new_args.push(args[0].clone());
    new_args.push(OsString::from("x"));
    new_args.extend(args.into_iter().skip(1));
    new_args
  } else {
    args
  };

  // Fast path for the overwhelmingly common `deno run <file> [args...]` with no
  // deno-level flags before the file: build `RunFlags` directly and skip the
  // parser entirely. `bare_run_fast_path` produces output identical to the full
  // parse (asserted by `fast_path_matches_full_parse`).
  if let Some(mut flags) = bare_run_fast_path(&args) {
    flags.initial_cwd = initial_cwd;
    return Ok(flags);
  }

  // The hand-written parser works on `String`s, whereas clap consumed
  // `OsString` natively. Args are almost always UTF-8; the lossy conversion is
  // a deliberate behavior change for the rare non-UTF-8 arg (e.g. a script path
  // with invalid bytes), which becomes mangled (replacement chars) here instead
  // of being carried through verbatim. Accepted as a Deno 3 simplification.
  let string_args: Vec<String> = args
    .iter()
    .map(|a| a.to_string_lossy().into_owned())
    .collect();

  match deno_cli_parser::convert::flags_from_vec(string_args) {
    Ok(mut flags) => {
      // Set (and, for compile/desktop, canonicalize) the initial cwd — the
      // parser crate has no filesystem access, so this stays on the CLI side.
      flags.initial_cwd = match &flags.subcommand {
        DenoSubcommand::Compile(_) | DenoSubcommand::Desktop(_) => {
          initial_cwd.map(|cwd| canonicalize_path(&cwd).ok().unwrap_or(cwd))
        }
        _ => initial_cwd,
      };
      Ok(flags)
    }
    Err(e) => Err(e),
  }
}

/// Render the text printed by `deno --version` (`long`) and `deno -V` (short).
/// Both forms are prefixed with the binary name and terminated by a newline,
/// matching what clap's `render_long_version` / `render_version` used to emit.
pub fn render_version(long: bool) -> String {
  if long {
    debug_assert_eq!(DENO_VERSION_INFO.typescript, deno_snapshots::TS_VERSION);
    format!(
      "deno {} ({}, {}, {})\nv8 {}\ntypescript {}\n",
      DENO_VERSION_INFO.deno,
      DENO_VERSION_INFO.release_channel.name(),
      env!("PROFILE"),
      env!("TARGET"),
      deno_core::v8::VERSION_STRING,
      DENO_VERSION_INFO.typescript,
    )
  } else {
    format!("deno {}\n", DENO_VERSION_INFO.deno)
  }
}

// originally copied from clap, https://github.com/clap-rs/clap/blob/4e1a565b8adb4f2ad74a9631565574767fdc37ae/clap_builder/src/parser/features/suggestions.rs#L11-L26
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

/// Handle a dynamic-completion callback: the shell invoked us with
/// `COMPLETE=<shell>` set and the words being completed passed after a `--`
/// separator. Candidates are computed from the hand-written command tree via
/// `deno_cli_parser::completions` (no clap), with `deno task` names supplied by
/// [`dynamic_task_completer`].
pub fn handle_shell_completion(_cwd: &Path) -> Result<(), AnyError> {
  let shell = std::env::var("COMPLETE").unwrap_or_default();
  let words = shell_completion_words();
  deno_cli_parser::completions::try_complete(
    &deno_cli_parser::defs::DENO_ROOT,
    &words,
    &shell,
    &dynamic_task_completer,
  );
  Ok(())
}

/// Emit the dynamic shell-completion registration script for `shell`
/// (bash/fish/zsh). The parser records only the target shell
/// (`CompletionsFlags::Dynamic { shell }`) and defers this CLI-only generation
/// here. The emitted script calls back into this binary with `COMPLETE` set,
/// where [`handle_shell_completion`] computes the candidates.
pub fn handle_dynamic_shell_completion(shell: &str) -> Result<(), AnyError> {
  let completer = std::env::args_os()
    .next()
    .map(|s| s.to_string_lossy().into_owned())
    .unwrap_or_else(|| "deno".to_string());
  let completer = shlex::try_quote(&completer)
    .map(|c| c.into_owned())
    .unwrap_or(completer);
  let script =
    deno_cli_parser::completions::generate_dynamic(shell, "deno", &completer);
  deno_print::drop_write_stdout(&script);
  Ok(())
}

/// The words being completed, extracted from the callback invocation. The
/// registration scripts invoke `<deno> -- <typed words...>`, so everything
/// after the first `--` is the command line (starting with the command name).
fn shell_completion_words() -> Vec<String> {
  let mut words = Vec::new();
  let mut seen_sep = false;
  for a in std::env::args_os() {
    if seen_sep {
      words.push(a.to_string_lossy().into_owned());
    } else if a == "--" {
      seen_sep = true;
    }
  }
  words
}

/// Positional-argument completer injected into the parser's completion engine.
/// Completes `deno task <TAB>` with the available task names read from the
/// resolved config (the only dynamic completion Deno offers). Kept CLI-side
/// because the parser crate has no config/workspace access.
fn dynamic_task_completer(
  active_cmd: &deno_cli_parser::CommandDef,
  _current: &str,
) -> Vec<(String, Option<String>)> {
  if active_cmd.name != "task" {
    return Vec::new();
  }
  let Some(flags) = task_flags_for_completion() else {
    return Vec::new();
  };
  match crate::tools::task::get_available_tasks_for_completion(
    std::sync::Arc::new(flags),
  ) {
    Ok(tasks) => tasks
      .into_iter()
      .map(|t| (t.name, t.task.description))
      .collect(),
    Err(e) => {
      log::debug!("Error during available tasks completion: {e}");
      Vec::new()
    }
  }
}

/// Best-effort parse of the completion command line into `Flags`, used to pick
/// up `deno task`'s `--config`/`--recursive`/`--filter` for task resolution.
fn task_flags_for_completion() -> Option<Flags> {
  let words = shell_completion_words();
  // The last word is the partial being completed; if the full line doesn't
  // parse, retry without it (mirrors clap's `ignore_errors`).
  let flags = deno_cli_parser::convert::flags_from_vec(words.clone())
    .or_else(|_| {
      let mut w = words;
      w.pop();
      deno_cli_parser::convert::flags_from_vec(w)
    })
    .ok()?;
  matches!(flags.subcommand, DenoSubcommand::Task(_)).then_some(flags)
}

#[cfg(test)]
mod tests {
  use deno_cli_parser::defs::DENO_ROOT;
  use pretty_assertions::assert_eq;

  use super::*;

  /// Creates vector of strings, Vec<String>
  macro_rules! svec {
    ($($x:expr),* $(,)?) => (vec![$($x.to_string().into()),*]);
  }

  fn flags_from_vec(args: Vec<OsString>) -> Result<Flags, CliError> {
    flags_from_vec_with_initial_cwd(args, None)
  }

  /// The bare-run fast path in `flags_from_vec_with_initial_cwd` bypasses the
  /// parser and builds `RunFlags` by hand, so it must produce output identical
  /// to running the args through the full hand-written parser. Assert that for
  /// every case where the fast path fires, and assert it correctly declines
  /// (returns `None`) when any deno-level flag precedes the script.
  #[test]
  fn fast_path_matches_full_parse() {
    // Cases where the fast path SHOULD fire; its output must equal the full
    // parse (neither sets `initial_cwd`, so both default to `None`).
    let fires: &[&[&str]] = &[
      &["deno", "run", "script.ts"],
      &["deno", "run", "./path/to/mod.ts"],
      &["deno", "run", "script.ts", "a", "b"],
      &["deno", "run", "script.ts", "--", "--foo", "-x"],
      &["deno", "run", "https://example.com/mod.ts", "arg"],
    ];
    for case in fires {
      let os_args: Vec<OsString> = case.iter().map(OsString::from).collect();
      let fast = bare_run_fast_path(&os_args).unwrap_or_else(|| {
        panic!("fast path should fire for {case:?}");
      });
      let string_args: Vec<String> =
        case.iter().map(|s| s.to_string()).collect();
      let full = deno_cli_parser::convert::flags_from_vec(string_args)
        .unwrap_or_else(|e| panic!("full parse failed for {case:?}: {e:?}"));
      assert_eq!(
        fast, full,
        "fast path diverged from full parse for {case:?}"
      );
    }

    // Cases where the fast path must decline and fall through to the parser.
    let declines: &[&[&str]] = &[
      &["deno", "run"],                    // no script
      &["deno", "run", "-"],               // stdin
      &["deno", "run", "-A", "script.ts"], // flag before script
      &["deno", "run", "--watch", "s.ts"], // flag before script
      &["deno", "test", "script.ts"],      // not `run`
    ];
    for case in declines {
      let os_args: Vec<OsString> = case.iter().map(OsString::from).collect();
      assert!(
        bare_run_fast_path(&os_args).is_none(),
        "fast path should decline for {case:?}"
      );
    }
  }

  #[test]
  fn install_os_arch_flags() {
    let r = flags_from_vec(svec![
      "deno", "install", "--os", "linux", "--arch", "arm64"
    ]);
    let flags = r.unwrap();
    assert_eq!(
      flags.subcommand,
      DenoSubcommand::Install(InstallFlags::Local(
        InstallFlagsLocal::TopLevel(InstallTopLevelFlags {
          lockfile_only: false,
          production: false,
          skip_types: false,
        }),
        NpmInstallTargetFlags {
          os: Some("linux".to_string()),
          arch: Some("arm64".to_string()),
        },
      ))
    );
    assert_eq!(
      npm_system_info(&flags.subcommand),
      NpmSystemInfo {
        os: "linux".into(),
        cpu: "arm64".into(),
      }
    );
  }

  #[test]
  fn install_os_only_flag() {
    let r = flags_from_vec(svec!["deno", "install", "--os", "win32"]);
    let flags = r.unwrap();
    assert_eq!(
      flags.subcommand,
      DenoSubcommand::Install(InstallFlags::Local(
        InstallFlagsLocal::TopLevel(InstallTopLevelFlags {
          lockfile_only: false,
          production: false,
          skip_types: false,
        }),
        NpmInstallTargetFlags {
          os: Some("win32".to_string()),
          arch: None,
        },
      ))
    );
    let sys_info = npm_system_info(&flags.subcommand);
    assert_eq!(sys_info.os.as_str(), "win32");
  }

  #[test]
  fn minimum_dependency_age_pm_subcommands() {
    // The package management subcommands that resolve versions must accept the
    // flag so users can act on the "blocked by minimum dependency age" hint.
    let cases = [
      svec!["deno", "add", "--min-dep-age=0", "jsr:@std/path"],
      svec!["deno", "add", "--minimum-dependency-age=0", "jsr:@std/path"],
      svec!["deno", "remove", "--min-dep-age=0", "@std/path"],
    ];
    for args in cases {
      let flags = flags_from_vec(args.clone())
        .unwrap_or_else(|e| panic!("failed to parse {args:?}: {e}"));
      assert_eq!(
        flags.minimum_dependency_age,
        Some(NewestDependencyDate::Disabled),
        "{args:?}"
      );
    }
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

    let flags = flags_from_vec(svec!["deno", "run", "jsr:@scope/foo"]).unwrap();
    assert_eq!(flags.config_path_args(&cwd), Some(vec![cwd.clone()]));

    let flags = flags_from_vec(svec!["deno", "run", "npm:@scope/foo"]).unwrap();
    assert_eq!(flags.config_path_args(&cwd), Some(vec![cwd.clone()]));

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

    // `deno desktop .` (and bare `deno desktop`) runs framework detection on
    // the current directory, so config discovery must start in that directory
    // rather than its parent. See issue #35653.
    let flags = flags_from_vec(svec!["deno", "desktop", "."]).unwrap();
    assert_eq!(flags.config_path_args(&cwd), Some(vec![cwd.clone()]));

    let flags = flags_from_vec(svec!["deno", "desktop"]).unwrap();
    assert_eq!(flags.config_path_args(&cwd), Some(vec![cwd.clone()]));

    // An explicit file entrypoint resolves to its containing directory.
    let flags =
      flags_from_vec(svec!["deno", "desktop", "sub/main.ts"]).unwrap();
    assert_eq!(flags.config_path_args(&cwd), Some(vec![cwd.join("sub")]));
  }
  /// `deno <sub> --help`, `deno <sub> -h` and `deno help <sub>` must all render
  /// the same text for every subcommand.
  #[test]
  fn equal_help_output() {
    for command in DENO_ROOT.subcommands {
      // `help` has no help of its own, and passthrough commands (deploy,
      // sandbox) forward `--help` to the external tool they proxy rather than
      // rendering their own.
      if command.name == "help" || command.passthrough {
        continue;
      }

      let render =
        |args: Vec<OsString>| match flags_from_vec(args).unwrap().subcommand {
          DenoSubcommand::Help(help) => help.help.to_string(),
          _ => unreachable!("{} did not render help", command.name),
        };

      let long_flag = render(svec!["deno", command.name, "--help"]);
      let short_flag = render(svec!["deno", command.name, "-h"]);
      let subcommand = render(svec!["deno", "help", command.name]);

      assert_eq!(long_flag, short_flag, "{} subcommand", command.name);
      assert_eq!(long_flag, subcommand, "{} subcommand", command.name);
    }
  }

  /// When `deno <subcommand>` is spawned via `node:child_process`, the args are
  /// passed through unchanged only if node_shim recognizes the subcommand.
  /// Otherwise it is misinterpreted as a script for `deno run` (see #35591).
  /// This test fails if a subcommand is added to the CLI without also adding it
  /// to `DENO_SUBCOMMANDS` in `libs/node_shim/lib.rs`.
  ///
  /// Aliases are checked too. Under clap this only covered *visible* aliases
  /// (`install`'s `i`), which let the hidden-but-working `rm` and
  /// `approve-builds` slip through; `CommandDef` does not record alias
  /// visibility, and every alias is equally spawnable, so all are required.
  #[test]
  fn subcommands_recognized_by_node_shim() {
    for command in DENO_ROOT.subcommands {
      let name = command.name;
      assert!(
        node_shim::is_deno_subcommand(name),
        "subcommand `{name}` is missing from node_shim's DENO_SUBCOMMANDS list; \
         add it to `libs/node_shim/lib.rs` so it is passed through when spawned \
         via node:child_process",
      );
      for alias in command.aliases {
        assert!(
          node_shim::is_deno_subcommand(alias),
          "alias `{alias}` of subcommand `{name}` is missing from node_shim's \
           DENO_SUBCOMMANDS list; add it to `libs/node_shim/lib.rs`",
        );
      }
    }
  }

  /// A `value_name` containing a `:` breaks shell completions for zsh, which
  /// uses `:` to separate a completion candidate from its description.
  #[test]
  fn no_colon_in_value_name() {
    fn check(cmd: &'static deno_cli_parser::CommandDef) {
      for arg in cmd.all_args() {
        if let Some(value_name) = arg.value_name {
          assert!(
            !value_name.contains(':'),
            "`{}` arg `{}` has a value_name containing ':' ({value_name}), \
             which breaks zsh completions",
            cmd.name,
            arg.name,
          );
        }
      }
      for sub in cmd.subcommands {
        check(sub);
      }
    }
    check(&DENO_ROOT);
  }
}
