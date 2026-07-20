// Copyright 2018-2026 the Deno authors. MIT license.

// The native `deno check` path that consumes this module lands on top of this
// PR, so everything here is unused until then.
#![allow(dead_code, reason = "Will be used in a follow up")]

use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use std::sync::LazyLock;

use deno_core::anyhow;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_core::serde_json::Value;
use deno_npm::registry::NpmRegistryApi;
use deno_npm_cache::TarballCache;
use deno_npmrc::ResolvedNpmRc;
use deno_resolver::workspace::WorkspaceNpmLinkPackagesRc;
use deno_semver::package::PackageNv;
use regex::Regex;

use crate::cache::DenoDir;
use crate::npm::CliNpmCache;
use crate::npm::CliNpmCacheHttpClient;
use crate::npm::CliNpmRegistryInfoProvider;
use crate::sys::CliSys;
use crate::tsc::Diagnostic;
use crate::tsc::DiagnosticCategory;
use crate::tsc::Position;

/// Pinned version of the native TypeScript compiler (`typescript@N`, the
/// Go/`tsgo` line) that `deno check` runs. Bumping this constant is the only
/// supported way to change the compiler; `deno check` never floats to the
/// latest published version so that a given Deno release always type-checks
/// with a known compiler.
pub const TYPESCRIPT_VERSION: &str = "7.0.2";

/// npm platform-package suffix for the current host, matching the
/// `@typescript/typescript-<suffix>` optional dependencies shipped by
/// `typescript`.
fn typescript_platform() -> Result<&'static str, AnyError> {
  typescript_platform_for(std::env::consts::ARCH, std::env::consts::OS)
}

/// Map a Rust `(ARCH, OS)` pair (as in [`std::env::consts`]) to the
/// `@typescript/typescript-<suffix>` package suffix. Split out so the mapping
/// can be unit-tested for every supported host.
fn typescript_platform_for(
  arch: &str,
  os: &str,
) -> Result<&'static str, AnyError> {
  Ok(match (arch, os) {
    ("x86_64", "linux") => "linux-x64",
    ("aarch64", "linux") => "linux-arm64",
    ("x86_64", "macos") => "darwin-x64",
    ("aarch64", "macos") => "darwin-arm64",
    ("x86_64", "windows") => "win32-x64",
    ("aarch64", "windows") => "win32-arm64",
    _ => anyhow::bail!(
      "Unsupported platform for the native TypeScript compiler: {arch} {os}"
    ),
  })
}

/// Everything the native `tsc` download step needs from the CLI factory,
/// bundled into one cloneable struct so it can be stored on `TypeChecker` and
/// handed to [`ensure_native_tsc`] without threading six separate arguments.
#[derive(Clone)]
pub struct NativeTscInstallDeps {
  pub deno_dir: DenoDir,
  pub npmrc: Arc<ResolvedNpmRc>,
  pub registry_info: Arc<CliNpmRegistryInfoProvider>,
  pub npm_link_packages: WorkspaceNpmLinkPackagesRc,
  pub tarball_cache: Arc<TarballCache<CliNpmCacheHttpClient, CliSys>>,
  pub npm_cache: Arc<CliNpmCache>,
}

/// Ensure the pinned native `tsc` for the host platform is available and
/// return the path to the executable, downloading the
/// `@typescript/typescript-<platform>` npm package if it isn't cached yet.
///
/// This deliberately fetches only the single host-platform package rather than
/// resolving `typescript` itself (whose optional dependencies would pull every
/// platform binary), mirroring how [`crate::tools::bundle::esbuild`] obtains
/// esbuild. Unlike esbuild (a single standalone binary), the tsc binary lives
/// in a `lib/` directory next to the default `lib.*.d.ts` files it loads at
/// runtime, so the whole `lib/` tree is materialized alongside it.
pub async fn ensure_native_tsc(
  deps: &NativeTscInstallDeps,
) -> Result<PathBuf, AnyError> {
  let deno_dir = &deps.deno_dir;
  let npmrc = &deps.npmrc;
  let api = &deps.registry_info;
  let workspace_link_packages = &deps.npm_link_packages;
  let tarball_cache = &deps.tarball_cache;
  let npm_cache = &deps.npm_cache;
  // Allow pointing at an already-available `tsc` binary instead of downloading
  // one, mirroring `DENORT_BIN`. Used by the test harness and CI to avoid
  // re-downloading the compiler for every run, and lets a user supply their
  // own build.
  if let Some(path) = std::env::var_os("DENO_TSC_BIN") {
    let path = PathBuf::from(path);
    if path.exists() {
      return Ok(path);
    }
    log::warn!(
      "DENO_TSC_BIN is set to {} but it does not exist; downloading the pinned compiler instead",
      path.display()
    );
  }

  let target = typescript_platform()?;
  // Keep the compiler under `$DENO_DIR/tsc/<version>/<platform>` so all of a
  // given version's files live in one predictable, versioned directory.
  let install_dir = deno_dir
    .root
    .join("tsc")
    .join(TYPESCRIPT_VERSION)
    .join(target);
  let bin_name = if cfg!(windows) { "tsc.exe" } else { "tsc" };
  let tsc_path = install_dir.join("lib").join(bin_name);

  if tsc_path.exists() {
    return Ok(tsc_path);
  }

  let pkg_name = format!("@typescript/typescript-{}", target);
  let nv = PackageNv::from_str(&format!("{}@{}", pkg_name, TYPESCRIPT_VERSION))
    .unwrap();
  let mut info = api.package_info(&pkg_name).await?;
  let version_info = match info.version_info(&nv, &workspace_link_packages.0) {
    Ok(version_info) => version_info,
    Err(_) => {
      api.mark_force_reload();
      info = api.package_info(&pkg_name).await?;
      info.version_info(&nv, &workspace_link_packages.0)?
    }
  };
  let Some(dist) = &version_info.dist else {
    anyhow::bail!(
      "could not resolve the native TypeScript compiler; download {} manually and copy its lib/ next to {}",
      nv,
      tsc_path.display()
    );
  };

  let registry_url = npmrc.get_registry_url(&nv.name);
  let package_folder =
    npm_cache.package_folder_for_nv_and_url(&nv, registry_url);
  let existed = package_folder.exists();
  if !existed {
    // `ensure_package` downloads the tarball and verifies it against the
    // registry `dist` integrity/shasum before extracting (the standard npm
    // pipeline), so the materialized compiler is checksum-validated at install.
    tarball_cache
      .ensure_package(&nv, dist)
      .await
      .with_context(|| {
        format!(
          "failed to download the TypeScript compiler tarball {} from {}",
          nv, dist.tarball
        )
      })?;
  }

  // Materialize `lib/` (the native binary plus the default lib `.d.ts` files it
  // resolves relative to itself) atomically: copy into a sibling temp dir and
  // rename it into place. The rename is atomic, so a concurrent `deno check`
  // never observes a half-copied tree through the `exists()` check above.
  let version_dir = install_dir.parent().unwrap();
  std::fs::create_dir_all(version_dir).with_context(|| {
    format!("failed to create directory {}", version_dir.display())
  })?;
  let tmp_dir =
    version_dir.join(format!(".{}-{}.tmp", target, std::process::id()));
  let _ = std::fs::remove_dir_all(&tmp_dir);
  crate::tools::compile::copy_dir_all(
    &package_folder.join("lib"),
    &tmp_dir.join("lib"),
  )
  .with_context(|| {
    format!(
      "failed to copy the TypeScript compiler out of {}",
      package_folder.display()
    )
  })?;
  match std::fs::rename(&tmp_dir, &install_dir) {
    Ok(()) => {}
    // Another process won the race and installed it already; discard our copy.
    Err(_) if tsc_path.exists() => {
      let _ = std::fs::remove_dir_all(&tmp_dir);
    }
    Err(err) => {
      let _ = std::fs::remove_dir_all(&tmp_dir);
      return Err(err).with_context(|| {
        format!(
          "failed to move the TypeScript compiler into place at {}",
          install_dir.display()
        )
      });
    }
  }

  if !existed {
    let _ = std::fs::remove_dir_all(&package_folder).inspect_err(|e| {
      log::warn!(
        "failed to remove directory {}: {}",
        package_folder.display(),
        e
      )
    });
  }

  Ok(tsc_path)
}

/// Run the native `tsc` at `tsc_path` against `tsconfig_path` and capture its
/// output. `--pretty false` yields the stable
/// `path(line,col): error TS####: message` format; `--diagnostics` appends a
/// stats block.
pub async fn run_native_tsc(
  tsc_path: &Path,
  tsconfig_path: &Path,
  project_root: &Path,
) -> Result<std::process::Output, AnyError> {
  tokio::process::Command::new(tsc_path)
    .arg("--project")
    .arg(tsconfig_path)
    .arg("--noEmit")
    .arg("--pretty")
    .arg("false")
    .arg("--diagnostics")
    .current_dir(project_root)
    // The native compiler is written in Go, whose `os.Getwd` trusts the `PWD`
    // environment variable over `getcwd()`. `current_dir` calls `chdir` but
    // does not update the inherited `PWD`, so a symlinked launch directory
    // (e.g. `/tmp` -> `/private/tmp` on macOS) would leave `PWD` pointing at
    // the symlink and make tsc report every path relative to it. Pin `PWD` to
    // the same directory we chdir'd into.
    .env("PWD", project_root)
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .output()
    .await
    .map_err(|e| anyhow::anyhow!("failed to run native tsc: {e}"))
}

/// Result of running the native compiler over a set of roots: the combined
/// diagnostics (tsc's own plus deno's graph diagnostics for missing
/// entrypoints). The caller decides whether these constitute a failure and
/// whether to record the incremental check hash.
pub struct NativeCheckOutcome {
  pub diagnostics: crate::tsc::Diagnostics,
}

/// Build the per-check tsconfig, run the native compiler over exactly `roots`,
/// and remap its diagnostics back onto the original module specifiers.
///
/// This is the "spawn tsc + build tsconfig + remap diagnostics" half of the
/// native check pipeline, split out so it can be shared beyond `deno check`.
/// It does not decide the incremental cache or record the check hash - the
/// caller owns that (recording only happens on a clean check).
///
/// `honor_user_tsconfig_root` is `Some(<root tsconfig path>)` when Deno honors a
/// user `tsconfig.json` (the caller makes that decision, since it needs
/// `CliOptions`); otherwise `None` points tsc at the generated
/// `.deno/tsconfig.json` directly.
pub async fn run_native_check_over_roots(
  tsc_path: &Path,
  project_root: &Path,
  roots: &[deno_core::ModuleSpecifier],
  current_dir: &deno_core::ModuleSpecifier,
  honor_user_tsconfig_root: Option<&Path>,
  root_diagnostics: crate::tsc::Diagnostics,
) -> Result<NativeCheckOutcome, AnyError> {
  // When Deno honors a user `tsconfig.json`, base tsc on a throwaway overlay of
  // it (its options + our generated `extends`/`references`) written to a temp
  // file in the project root - so the user's path-based options (rootDirs,
  // baseUrl, include/files) resolve relative to the project, WITHOUT us
  // rewriting their committed file. Otherwise point tsc at the generated config
  // directly. See `sync_types_command` / `build_check_root_overlay`.
  //
  // Holds the root overlay temp file open until tsc has run (dropping deletes
  // it), keeping `deno check` side-effect-free on the user's tree.
  let _root_tsconfig_guard;
  let base_tsconfig = if let Some(root_tsconfig) = honor_user_tsconfig_root {
    let overlay = crate::tsc::tsconfig_gen::build_check_root_overlay(
      project_root,
      root_tsconfig,
    )?;
    let mut tmp = tempfile::Builder::new()
      .prefix("deno-check-root-")
      .suffix(".tsconfig.json")
      .tempfile_in(project_root)?;
    std::io::Write::write_all(
      &mut tmp,
      deno_core::serde_json::to_string_pretty(&overlay)?.as_bytes(),
    )?;
    let path = tmp.path().to_path_buf();
    _root_tsconfig_guard = tmp;
    path
  } else {
    project_root.join(".deno").join("tsconfig.json")
  };
  // Pin tsc to exactly the roots deno resolved (glob-expanded and exclude-applied
  // above), so `deno check *.ts` checks only the matched files - not the whole
  // project - and an excluded file stays excluded. Local roots map to their
  // `file://` path; remote roots map to their `.deno/remote/` mirror (added just
  // below). Only an extensionless remote root - which stock tsc can't load - is
  // left out, falling back to the base config's `include`.
  let mut files: Vec<String> = roots
    .iter()
    .filter(|s| s.scheme() == "file")
    .filter_map(|s| s.to_file_path().ok())
    .filter(|p| p.exists())
    .map(|p| p.to_string_lossy().replace('\\', "/"))
    .collect();
  // A remote (`http(s):`) root is not a `file://` path, but sync-types mirrored
  // it under `.deno/remote/`. Pin tsc to those mirror files too, so a remote
  // entrypoint is checked as itself rather than triggering the whole-project
  // `include` fallback below (which would check unrelated files, or nothing).
  files.extend(remote_root_mirror_files(project_root, roots));

  // Every requested root was a missing (non-existent) local entrypoint: there's
  // nothing for tsc to check, so report deno's graph diagnostics for them
  // directly instead of falling back to the base config's project-wide
  // `include` (which would check unrelated files).
  if files.is_empty() && root_diagnostics.has_diagnostic() {
    log_check_roots(roots, current_dir);
    return Ok(NativeCheckOutcome {
      diagnostics: root_diagnostics,
    });
  }

  // Holds the per-file config's temp file open until tsc has run (dropping it
  // deletes the file).
  let _check_tsconfig_guard;
  let tsconfig_path = if files.is_empty() {
    base_tsconfig
  } else {
    // `deno check <files>` checks only the named files (and their imports), not
    // the whole project. The generated `tsconfig.json` keeps an open `include`
    // (so bundlers can consume its resolver mappings), so write a per-file
    // config that extends it (by absolute path) and pins `files` - `files`/
    // `include` are not inherited through `extends`, so only these files are
    // type-checked while compilerOptions/paths still apply. `include: []`
    // nullifies the base's open `include` (tsc unions `files` with an inherited
    // `include`, which would otherwise re-add the whole project).
    //
    // `files`/`include` are not inherited through `extends`, so the base config's
    // own `files` - the declaration files sync-types materialized from
    // `compilerOptions.types` (npm packages, relative paths) that provide global
    // augmentations - would be dropped. Carry them over so those types still
    // apply when checking specific files.
    let generated_base = project_root.join(".deno").join("tsconfig.json");
    if let Ok(text) = std::fs::read_to_string(&generated_base)
      && let Ok(value) =
        deno_core::serde_json::from_str::<deno_core::serde_json::Value>(&text)
      && let Some(base_files) = value.get("files").and_then(|f| f.as_array())
    {
      for f in base_files {
        if let Some(s) = f.as_str() {
          let s = s.to_string();
          if !files.contains(&s) {
            files.push(s);
          }
        }
      }
    }
    // Write to the system temp dir, not the project root: this config only
    // references absolute paths (`extends` the base config by absolute path,
    // `files` are absolute, `include` is empty), and tsc runs with its cwd
    // pinned to `project_root` regardless of where the config lives, so its
    // location does not affect resolution. Keeping it out of the project tree
    // avoids leaving an artifact behind and, more importantly, avoids racing
    // with anything enumerating the project directory - e.g. a sibling
    // spec-test variant that copies the directory while this ephemeral file
    // briefly exists (and then vanishes when the guard drops).
    let content = deno_core::serde_json::json!({
      "extends": base_tsconfig.to_string_lossy().replace('\\', "/"),
      "include": [],
      "files": files,
    });
    let mut tmp = tempfile::Builder::new()
      .prefix("deno-check-")
      .suffix(".tsconfig.json")
      .tempfile()?;
    std::io::Write::write_all(
      &mut tmp,
      deno_core::serde_json::to_string_pretty(&content)?.as_bytes(),
    )?;
    let path = tmp.path().to_path_buf();
    _check_tsconfig_guard = tmp;
    path
  };

  log_check_roots(roots, current_dir);

  let output = run_native_tsc(tsc_path, &tsconfig_path, project_root).await?;

  let stdout = String::from_utf8_lossy(&output.stdout);
  let stderr = String::from_utf8_lossy(&output.stderr);
  let mut diagnostics =
    crate::tsc::Diagnostics::from(parse_tsc_diagnostics(&stdout, project_root));
  // Fold in deno's own diagnostics for missing entrypoints (some roots existed
  // and were type-checked by tsc above; any that didn't are reported here).
  diagnostics.extend(root_diagnostics);

  // Captured for an upcoming "Checked N files" summary; not surfaced yet.
  let stats = parse_tsc_stats(&stdout);
  log::debug!("native tsc {stats:?}");

  if diagnostics.has_diagnostic() {
    return Ok(NativeCheckOutcome { diagnostics });
  }

  // tsc exited non-zero but we have nothing to show. If it printed diagnostic
  // lines we deliberately dropped (e.g. `noImplicitOverride` in remote modules),
  // its exit code is expected - treat the check as clean. Only when tsc produced
  // no recognizable diagnostics at all is this a genuine failure (an internal
  // error or a malformed generated config); surface whatever it printed then.
  let tsc_reported_diagnostics = output_has_diagnostics(&stdout);
  if !output.status.success() && !tsc_reported_diagnostics {
    let detail = stdout.trim();
    let detail = if detail.is_empty() {
      stderr.trim()
    } else {
      detail
    };
    return Err(anyhow::anyhow!(
      "native tsc exited with {} without parseable diagnostics{}",
      output.status,
      if detail.is_empty() {
        String::new()
      } else {
        format!(":\n{detail}")
      }
    ));
  }

  Ok(NativeCheckOutcome { diagnostics })
}

/// Print a `Check <specifier>` line for each provided root, rendered relative
/// to `current_dir`, matching Deno 2.x's forked tsc output.
fn log_check_roots(
  roots: &[deno_core::ModuleSpecifier],
  current_dir: &deno_core::ModuleSpecifier,
) {
  for root in roots {
    log::info!(
      "{} {}",
      deno_terminal::colors::green("Check"),
      crate::util::path::relative_specifier_path_for_display(current_dir, root),
    );
  }
}

/// Whether a mirror path carries an extension stock tsc can language-detect.
/// A module Deno serves by content-type but mirrors without a code extension
/// (`no_js_ext`) can't be loaded by stock tsc, which keys language off the
/// extension alone.
fn has_checkable_extension(path: &str) -> bool {
  let lower = path.to_ascii_lowercase();
  [
    ".d.ts", ".d.mts", ".d.cts", ".ts", ".tsx", ".mts", ".cts", ".js", ".jsx",
    ".mjs", ".cjs",
  ]
  .iter()
  .any(|ext| lower.ends_with(ext))
}

/// Resolve remote (`http(s):`) roots to their mirrored local files under
/// `.deno/remote/` (via the generated tsconfig's `paths`) so tsc can be pinned
/// to exactly the requested remote entrypoint via `files`, instead of dropping
/// them and falling back to the base config's whole-project `include`.
/// Extensionless remote modules have no mirror stock tsc can load and are
/// skipped (see `has_checkable_extension`).
fn remote_root_mirror_files(
  project_root: &Path,
  roots: &[deno_core::ModuleSpecifier],
) -> Vec<String> {
  let deno_dir = project_root.join(".deno");
  let Ok(text) = std::fs::read_to_string(deno_dir.join("tsconfig.json")) else {
    return Vec::new();
  };
  let Ok(value) =
    deno_core::serde_json::from_str::<deno_core::serde_json::Value>(&text)
  else {
    return Vec::new();
  };
  let Some(paths) = value
    .get("compilerOptions")
    .and_then(|c| c.get("paths"))
    .and_then(|p| p.as_object())
  else {
    return Vec::new();
  };
  let mut files = Vec::new();
  for root in roots.iter().filter(|s| s.scheme() != "file") {
    let Some(rel) = paths
      .get(root.as_str())
      .and_then(|t| t.as_array())
      .and_then(|a| a.first())
      .and_then(|v| v.as_str())
      .and_then(|t| t.strip_prefix("./"))
    else {
      continue;
    };
    if !has_checkable_extension(rel) {
      continue;
    }
    // `paths` targets are relative to `.deno/` (where the generated tsconfig
    // lives); rebase onto an absolute path for the `files` entry.
    let local = deno_dir.join(rel);
    if local.exists() {
      files.push(local.to_string_lossy().replace('\\', "/"));
    }
  }
  files
}

/// A single `path(line,col): error TS####: message` line from `tsc --pretty
/// false`. Continuation lines (indented elaborations) are folded into the
/// preceding diagnostic's message.
static DIAGNOSTIC_RE: LazyLock<Regex> = LazyLock::new(|| {
  Regex::new(
    r"^(?P<file>.+?)\((?P<line>\d+),(?P<col>\d+)\): (?P<cat>error|warning|message) TS(?P<code>\d+): (?P<msg>.*)$",
  )
  .unwrap()
});

/// The position-less form (`error TS####: message`), e.g. `TS18003` (no
/// inputs) or a `TS5###` config error.
static DIAGNOSTIC_NO_POS_RE: LazyLock<Regex> = LazyLock::new(|| {
  Regex::new(r"^(?P<cat>error|warning|message) TS(?P<code>\d+): (?P<msg>.*)$")
    .unwrap()
});

/// Whether `tsc` output contains any recognizable diagnostic line. Used to tell
/// a genuine failure (tsc printed nothing parseable) from an expected non-zero
/// exit after we filtered every diagnostic it did print.
pub fn output_has_diagnostics(output: &str) -> bool {
  output
    .lines()
    .any(|l| DIAGNOSTIC_RE.is_match(l) || DIAGNOSTIC_NO_POS_RE.is_match(l))
}

/// Parse `tsc --pretty false` output into Deno diagnostics, remapping the
/// compiler's mirror paths back onto the specifiers the user wrote.
pub fn parse_tsc_diagnostics(
  output: &str,
  project_root: &Path,
) -> Vec<Diagnostic> {
  let mut diagnostics: Vec<Diagnostic> = Vec::new();
  // Cache of source files read to reconstruct the underlined snippet. The
  // compiler reports positions but not the source line itself, so we read it
  // back from the same file it referred to (which is always on disk: the
  // project sources, or the materialized jsr:/http(s): mirrors).
  let mut source_cache: HashMap<PathBuf, Option<Vec<String>>> = HashMap::new();
  // Reverse map (mirror path -> original URL) so remote diagnostics render the
  // specifier the user wrote, with the correct scheme and port.
  let remote_map = build_remote_url_map(project_root);

  for line in output.lines() {
    if let Some(caps) = DIAGNOSTIC_RE.captures(line) {
      let line_no: u64 = caps["line"].parse().unwrap_or(1);
      let col_no: u64 = caps["col"].parse().unwrap_or(1);
      // tsc positions are 1-based; Deno's `Position` is 0-based and adds one
      // back when rendering.
      let start = Position {
        line: line_no.saturating_sub(1),
        character: col_no.saturating_sub(1),
      };
      let source_line = read_source_line(
        &mut source_cache,
        &project_root.join(&caps["file"]),
        start.line as usize,
      );
      // The grep format has no end position, so underline the token that
      // starts at the reported column (an identifier, a quoted specifier, or a
      // single caret) as a best-effort span.
      let end = source_line.as_deref().map(|sl| Position {
        line: start.line,
        character: start.character
          + underline_len(sl, start.character as usize),
      });
      diagnostics.push(make_diagnostic(
        &caps["cat"],
        caps["code"].parse().unwrap_or(0),
        Some(remap_path(&caps["file"], project_root, &remote_map)),
        Some(start),
        end,
        source_line,
        caps["msg"].to_string(),
      ));
    } else if let Some(caps) = DIAGNOSTIC_NO_POS_RE.captures(line) {
      diagnostics.push(make_diagnostic(
        &caps["cat"],
        caps["code"].parse().unwrap_or(0),
        None,
        None,
        None,
        None,
        caps["msg"].to_string(),
      ));
    } else if line.starts_with([' ', '\t']) && !line.trim().is_empty() {
      // Indented elaboration for the previous diagnostic.
      if let Some(last) = diagnostics.last_mut() {
        let message = last.message_text.get_or_insert_with(String::new);
        message.push('\n');
        message.push_str(line.trim_end());
      }
    }
    // Any other line (blank lines, a trailing "Found N errors" summary) is
    // ignored.
  }

  // `--all` type-checks remote modules for genuine type errors, but Deno doesn't
  // apply the `noImplicitOverride` style rule to code that isn't the user's, so
  // drop override-modifier diagnostics (the TS411x family) reported in a
  // non-local (remote/jsr/npm) module. Local `file://` modules keep them.
  diagnostics.retain(|d| {
    // TS4114..=4116 are the `noImplicitOverride` family (a Deno default we don't
    // apply to code that isn't the user's). TS4113 ("member cannot have an
    // 'override' modifier because it is not declared in the base class") is an
    // unconditional error and must never be dropped.
    !matches!(d.code, 4114..=4116)
      || d
        .file_name
        .as_deref()
        .is_none_or(|f| f.starts_with("file:"))
  });

  diagnostics
}

/// Compiler statistics reported by `tsc --diagnostics` (how many files/lines
/// were checked, timing, memory). Captured for an upcoming user-facing
/// "Checked N files" summary; currently only logged at debug level.
#[derive(Debug, Default)]
pub struct TscStats {
  pub files: Option<u64>,
  pub lines: Option<u64>,
  pub identifiers: Option<u64>,
  pub symbols: Option<u64>,
  pub types: Option<u64>,
  pub memory_used: Option<String>,
  pub check_time: Option<String>,
  pub total_time: Option<String>,
}

/// Parse the `Key:  value` stats block emitted by `tsc --diagnostics`. Lines
/// that aren't recognized stat keys (including the diagnostics themselves) are
/// ignored.
pub fn parse_tsc_stats(output: &str) -> TscStats {
  let mut stats = TscStats::default();
  for line in output.lines() {
    let Some((key, value)) = line.split_once(':') else {
      continue;
    };
    let value = value.trim();
    if value.is_empty() {
      continue;
    }
    match key.trim() {
      "Files" => stats.files = value.parse().ok(),
      "Lines" => stats.lines = value.parse().ok(),
      "Identifiers" => stats.identifiers = value.parse().ok(),
      "Symbols" => stats.symbols = value.parse().ok(),
      "Types" => stats.types = value.parse().ok(),
      "Memory used" => stats.memory_used = Some(value.to_string()),
      "Check time" => stats.check_time = Some(value.to_string()),
      "Total time" => stats.total_time = Some(value.to_string()),
      _ => {}
    }
  }
  stats
}

/// Read line `line_idx` (0-based) of `path`, caching the file's lines. Returns
/// `None` if the file can't be read or the line is out of range.
fn read_source_line(
  cache: &mut HashMap<PathBuf, Option<Vec<String>>>,
  path: &Path,
  line_idx: usize,
) -> Option<String> {
  let lines = cache.entry(path.to_path_buf()).or_insert_with(|| {
    std::fs::read_to_string(path)
      .ok()
      .map(|s| s.lines().map(str::to_string).collect())
  });
  lines.as_ref()?.get(line_idx).cloned()
}

/// Best-effort length (in characters) of the token to underline starting at
/// `start_char` on `line`: a quoted string (including its quotes), an
/// identifier run, or a single character otherwise.
fn underline_len(line: &str, start_char: usize) -> u64 {
  let chars: Vec<char> = line.chars().collect();
  let Some(&first) = chars.get(start_char) else {
    return 1;
  };
  if matches!(first, '\'' | '"' | '`') {
    if let Some(offset) =
      chars[start_char + 1..].iter().position(|&c| c == first)
    {
      return offset as u64 + 2; // opening quote + contents + closing quote
    }
    return 1;
  }
  if is_ident_char(first) {
    let len = chars[start_char..]
      .iter()
      .take_while(|&&c| is_ident_char(c))
      .count();
    return len as u64;
  }
  1
}

fn is_ident_char(c: char) -> bool {
  c.is_alphanumeric() || c == '_' || c == '$'
}

fn make_diagnostic(
  category: &str,
  code: u64,
  file_name: Option<String>,
  start: Option<Position>,
  end: Option<Position>,
  source_line: Option<String>,
  message_text: String,
) -> Diagnostic {
  Diagnostic {
    category: match category {
      "warning" => DiagnosticCategory::Warning,
      "message" => DiagnosticCategory::Message,
      _ => DiagnosticCategory::Error,
    },
    code,
    start,
    end,
    original_source_start: None,
    message_text: Some(message_text),
    message_chain: None,
    source: None,
    source_line,
    file_name,
    related_information: None,
    reports_deprecated: None,
    reports_unnecessary: None,
    other: Default::default(),
    missing_specifier: None,
  }
}

/// Build a reverse map from a mirror-relative path (the part after
/// `.deno/remote/`, i.e. `<host>__<port>/<path>`) to the original `http(s):`
/// URL, read from the generated tsconfig's `paths`. The mirror layout doesn't
/// encode the URL scheme, so this recovers the exact specifier (scheme + port)
/// for diagnostic display. Returns an empty map if the config can't be read.
fn build_remote_url_map(project_root: &Path) -> HashMap<String, String> {
  let config = project_root.join(".deno").join("tsconfig.json");
  let Ok(text) = std::fs::read_to_string(&config) else {
    return HashMap::new();
  };
  let Ok(value) = deno_core::serde_json::from_str::<Value>(&text) else {
    return HashMap::new();
  };
  build_remote_url_map_from_config(&value)
}

/// The pure mapping used by [`build_remote_url_map`]: given a parsed generated
/// tsconfig, extract the `http(s):` `paths` entries as `<mirror-rel> -> <url>`.
/// Split out so it can be unit-tested without touching the filesystem.
fn build_remote_url_map_from_config(config: &Value) -> HashMap<String, String> {
  let mut map = HashMap::new();
  let Some(paths) = config
    .get("compilerOptions")
    .and_then(|c| c.get("paths"))
    .and_then(|p| p.as_object())
  else {
    return map;
  };
  for (url, targets) in paths {
    if !(url.starts_with("http://") || url.starts_with("https://")) {
      continue;
    }
    if let Some(rel) = targets
      .as_array()
      .and_then(|a| a.first())
      .and_then(|v| v.as_str())
      .and_then(|t| t.strip_prefix("./remote/"))
    {
      map.insert(rel.to_string(), url.clone());
    }
  }
  map
}

/// Best-effort remap of a path reported by the native compiler back onto the
/// original module specifier. `deno check` runs tsc against a generated
/// tsconfig whose `paths` point jsr:/http(s): dependencies into mirror
/// directories, so the compiler reports those mirror paths rather than the
/// specifiers the user wrote.
fn remap_path(
  raw: &str,
  project_root: &Path,
  remote_map: &HashMap<String, String>,
) -> String {
  let normalized = raw.replace('\\', "/");

  // http(s): dependencies mirrored under `.deno/remote/<host>/<path>`. The
  // mirror layout drops the URL scheme (and encodes `:port` as `__port`), so
  // recover the exact original URL from the generated tsconfig's `paths`; fall
  // back to a best-effort `https://` reconstruction when it isn't found.
  if let Some(rest) = normalized.split(".deno/remote/").nth(1) {
    if let Some(url) = remote_map.get(rest) {
      return url.clone();
    }
    return format!("https://{rest}");
  }

  // jsr: dependencies installed under `node_modules/@jsr/<scope>__<name>/...`.
  if let Some(rest) = normalized.split("node_modules/@jsr/").nth(1)
    && let Some((pkg, sub)) = rest.split_once('/')
    && let Some((scope, name)) = pkg.split_once("__")
  {
    return format!("jsr:@{scope}/{name}/{sub}");
  }

  // Other npm dependencies under `node_modules/<pkg>/...`.
  if let Some(rest) = normalized.split("node_modules/").nth(1) {
    return format!("npm:{rest}");
  }

  // A file in the project itself. tsc reports it relative to the project root
  // (the process cwd); render it as an absolute `file://` URL like the rest of
  // Deno's diagnostics.
  let path = Path::new(raw);
  let absolute = if path.is_absolute() {
    path.to_path_buf()
  } else {
    project_root.join(path)
  };
  match deno_path_util::url_from_file_path(&absolute) {
    Ok(url) => url.to_string(),
    Err(_) => raw.to_string(),
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn typescript_platform_mapping() {
    assert_eq!(
      typescript_platform_for("x86_64", "linux").unwrap(),
      "linux-x64"
    );
    assert_eq!(
      typescript_platform_for("aarch64", "linux").unwrap(),
      "linux-arm64"
    );
    assert_eq!(
      typescript_platform_for("x86_64", "macos").unwrap(),
      "darwin-x64"
    );
    assert_eq!(
      typescript_platform_for("aarch64", "macos").unwrap(),
      "darwin-arm64"
    );
    assert_eq!(
      typescript_platform_for("x86_64", "windows").unwrap(),
      "win32-x64"
    );
    assert_eq!(
      typescript_platform_for("aarch64", "windows").unwrap(),
      "win32-arm64"
    );
  }

  #[test]
  fn typescript_platform_unsupported() {
    let err = typescript_platform_for("riscv64", "linux").unwrap_err();
    assert!(err.to_string().contains("Unsupported platform"));
    // The host's own platform must be supported (this is what CI runs on).
    assert!(typescript_platform().is_ok());
  }

  #[test]
  fn parse_diagnostics_positioned() {
    // `project_root` doesn't exist on disk, so `source_line`/`end` stay `None`;
    // the parse of code/category/position/message/path is what's under test.
    // Use an absolute temp dir so the local-file -> `file://` URL mapping works
    // on Windows too (a bare "/project" isn't absolute there).
    let root = std::env::temp_dir();
    let diags = parse_tsc_diagnostics(
      "mod.ts(2,7): error TS2322: Type 'string' is not assignable to type 'number'.\n",
      &root,
    );
    assert_eq!(diags.len(), 1);
    let d = &diags[0];
    assert_eq!(d.category, DiagnosticCategory::Error);
    assert_eq!(d.code, 2322);
    // 1-based (2,7) becomes 0-based (1,6).
    let start = d.start.as_ref().unwrap();
    assert_eq!((start.line, start.character), (1, 6));
    let file_name = d.file_name.as_deref().unwrap();
    assert!(file_name.starts_with("file://"), "{file_name}");
    assert!(file_name.ends_with("/mod.ts"), "{file_name}");
  }

  #[test]
  fn parse_diagnostics_warning_and_no_position() {
    let diags = parse_tsc_diagnostics(
      "a.ts(1,1): warning TS6133: 'x' is declared but never used.\nerror TS18003: No inputs were found in config file.\n",
      Path::new("/p"),
    );
    assert_eq!(diags.len(), 2);
    assert_eq!(diags[0].category, DiagnosticCategory::Warning);
    assert_eq!(diags[0].code, 6133);
    assert_eq!(diags[1].code, 18003);
    // The position-less form has no file or start.
    assert!(diags[1].file_name.is_none());
    assert!(diags[1].start.is_none());
  }

  #[test]
  fn parse_diagnostics_folds_elaboration() {
    let diags = parse_tsc_diagnostics(
      "a.ts(1,1): error TS2322: Type 'A' is not assignable to type 'B'.\n  Types of property 'x' are incompatible.\n",
      Path::new("/p"),
    );
    assert_eq!(diags.len(), 1);
    assert_eq!(
      diags[0].message_text.as_deref(),
      Some(
        "Type 'A' is not assignable to type 'B'.\n  Types of property 'x' are incompatible."
      )
    );
  }

  #[test]
  fn parse_diagnostics_ignores_stats_and_summary() {
    let diags = parse_tsc_diagnostics(
      "a.ts(1,1): error TS1: boom\nFiles:             10\nFound 1 error.\n",
      Path::new("/p"),
    );
    assert_eq!(diags.len(), 1);
  }

  #[test]
  fn remap_path_variants() {
    // Absolute temp dir so the local-file case yields a valid `file://` URL on
    // Windows (the mirror cases below are pure string transforms).
    let root = std::env::temp_dir();
    let root = root.as_path();
    let empty = HashMap::new();
    assert_eq!(
      remap_path(
        ".deno/remote/html.spec.whatwg.org/entities.json",
        root,
        &empty
      ),
      "https://html.spec.whatwg.org/entities.json"
    );
    // With a reverse map, the exact URL (scheme + port) is recovered.
    let mut remote_map = HashMap::new();
    remote_map.insert(
      "localhost__4545/subdir/type_error.ts".to_string(),
      "http://localhost:4545/subdir/type_error.ts".to_string(),
    );
    assert_eq!(
      remap_path(
        ".deno/remote/localhost__4545/subdir/type_error.ts",
        root,
        &remote_map
      ),
      "http://localhost:4545/subdir/type_error.ts"
    );
    assert_eq!(
      remap_path("node_modules/@jsr/std__assert/mod.ts", root, &empty),
      "jsr:@std/assert/mod.ts"
    );
    assert_eq!(
      remap_path("node_modules/chalk/index.d.ts", root, &empty),
      "npm:chalk/index.d.ts"
    );
    // A backslash-separated jsr mirror path (Windows form) still remaps.
    assert_eq!(
      remap_path("node_modules\\@jsr\\std__fmt/colors.ts", root, &empty),
      "jsr:@std/fmt/colors.ts"
    );
    // A project-local file becomes an absolute file URL.
    let mapped = remap_path("src/mod.ts", root, &empty);
    assert!(mapped.starts_with("file://"), "{mapped}");
    assert!(mapped.ends_with("/src/mod.ts"), "{mapped}");
  }

  #[test]
  fn build_remote_url_map_reads_http_paths() {
    // Only `http(s):` keys whose target lives under `./remote/` are extracted,
    // keyed by the mirror-relative path; other `paths` entries are ignored.
    let config = deno_core::serde_json::json!({
      "compilerOptions": {
        "paths": {
          "https://deno.land/std/fmt/colors.ts": [
            "./remote/deno.land/std/fmt/colors.ts"
          ],
          "http://localhost:4545/mod.ts": ["./remote/localhost__4545/mod.ts"],
          "npm:chalk": ["./npm/chalk"],
        }
      }
    });
    let map = build_remote_url_map_from_config(&config);
    assert_eq!(map.len(), 2);
    assert_eq!(
      map.get("deno.land/std/fmt/colors.ts").map(String::as_str),
      Some("https://deno.land/std/fmt/colors.ts")
    );
    assert_eq!(
      map.get("localhost__4545/mod.ts").map(String::as_str),
      Some("http://localhost:4545/mod.ts")
    );
  }

  #[test]
  fn build_remote_url_map_no_paths() {
    let config = deno_core::serde_json::json!({ "compilerOptions": {} });
    assert!(build_remote_url_map_from_config(&config).is_empty());
  }

  #[test]
  fn underline_len_tokens() {
    // Identifier run.
    assert_eq!(underline_len("const foo = 1;", 6), 3);
    // Quoted specifier, including both quotes.
    assert_eq!(underline_len("import x from \"graphviz\";", 14), 10);
    // Single non-identifier character.
    assert_eq!(underline_len("= 1", 0), 1);
    // Out of range.
    assert_eq!(underline_len("abc", 10), 1);
  }

  #[test]
  fn parse_stats_block() {
    let stats = parse_tsc_stats(
      "a.ts(1,1): error TS1: boom\nFiles:             1396\nLines:           314608\nCheck time:      0.546s\nTotal time:      0.729s\nMemory used:    555106K\n",
    );
    assert_eq!(stats.files, Some(1396));
    assert_eq!(stats.lines, Some(314608));
    assert_eq!(stats.check_time.as_deref(), Some("0.546s"));
    assert_eq!(stats.total_time.as_deref(), Some("0.729s"));
    assert_eq!(stats.memory_used.as_deref(), Some("555106K"));
  }
}
