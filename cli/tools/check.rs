// Copyright 2018-2026 the Deno authors. MIT license.

use std::path::Path;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use std::sync::LazyLock;

use deno_core::error::AnyError;
use deno_terminal::colors;
use regex::Regex;

use crate::args::CheckFlags;
use crate::args::Flags;
use crate::factory::CliFactory;
use crate::tsc::Diagnostic;
use crate::tsc::DiagnosticCategory;
use crate::tsc::Diagnostics;
use crate::tsc::Position;
use crate::util::file_watcher;

pub async fn check(
  flags: Arc<Flags>,
  check_flags: CheckFlags,
) -> Result<(), AnyError> {
  if let Some(watch_flags) = &flags.watch {
    let no_clear_screen = watch_flags.no_clear_screen;
    file_watcher::watch_func(
      flags,
      file_watcher::PrintConfig::new("Check", !no_clear_screen),
      move |flags, watcher_communicator, changed_paths| {
        let check_flags = check_flags.clone();
        watcher_communicator.show_path_changed(changed_paths);
        Ok(async move { native_check(flags, check_flags).await })
      },
    )
    .await
  } else {
    native_check(flags, check_flags).await
  }
}

/// Type-check the project with the native TypeScript compiler.
///
/// `deno check` generates a stock `tsconfig.json` and materializes the types of
/// the project's dependencies (the same work `deno sync-types` performs), then
/// runs the pinned native `tsc` against it and remaps the compiler's
/// diagnostics back onto the original module specifiers.
async fn native_check(
  flags: Arc<Flags>,
  check_flags: CheckFlags,
) -> Result<(), AnyError> {
  if check_flags.doc || check_flags.doc_only {
    // Doc snippet extraction was handled by the previous in-isolate checker;
    // the native compiler does not type-check markdown/JSDoc snippets yet.
    log::warn!(
      "{} --doc/--doc-only is not yet supported by the native type checker and will be ignored",
      colors::yellow("Warning")
    );
  }

  // Generate the tsconfig.json and materialize dependency types so the native
  // compiler can resolve the project's jsr:/http(s): imports.
  crate::tools::installer::sync_types_command(flags.clone()).await?;

  let factory = CliFactory::from_flags(flags);
  let tsc_path = ensure_native_tsc_downloaded(&factory).await?;

  let cli_options = factory.cli_options()?;
  let project_root = cli_options
    .workspace()
    .root_dir_url()
    .to_file_path()
    .map_err(|_| {
      deno_core::anyhow::anyhow!("workspace root is not a local directory")
    })?;
  let tsconfig_path = project_root.join("tsconfig.json");

  log::info!(
    "{} {}",
    colors::green("Check"),
    colors::gray(format!(
      "(native tsc {})",
      crate::tsc::native::TYPESCRIPT_VERSION
    ))
  );

  // `--pretty false` yields the stable one-line-per-diagnostic grep format
  // (`path(line,col): error TS####: message`) that we parse below. The
  // generated tsconfig already sets `noEmit`; pass it too so a stray option
  // can't trigger emit.
  let output = tokio::process::Command::new(&tsc_path)
    .arg("--project")
    .arg(&tsconfig_path)
    .arg("--noEmit")
    .arg("--pretty")
    .arg("false")
    .current_dir(&project_root)
    // The native compiler is written in Go, whose `os.Getwd` trusts the `PWD`
    // environment variable over `getcwd()`. `current_dir` calls `chdir` but
    // does not update the inherited `PWD`, so a symlinked launch directory
    // (e.g. `/tmp` -> `/private/tmp` on macOS) would leave `PWD` pointing at
    // the symlink and make tsc report every path relative to it (as
    // `../../<abs>`). Pin `PWD` to the same directory we chdir'd into.
    .env("PWD", &project_root)
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .output()
    .await
    .map_err(|e| deno_core::anyhow::anyhow!("failed to run native tsc: {e}"))?;

  let stdout = String::from_utf8_lossy(&output.stdout);
  let stderr = String::from_utf8_lossy(&output.stderr);
  let diagnostics = parse_tsc_diagnostics(&stdout, &project_root);

  if diagnostics.has_diagnostic() {
    log::error!("{}\n", diagnostics);
    return Err(deno_core::anyhow::anyhow!("Type checking failed."));
  }

  // No parseable diagnostics but tsc still failed (e.g. an internal error or a
  // malformed generated config). Surface whatever it printed so the failure
  // isn't swallowed.
  if !output.status.success() {
    let detail = stdout.trim();
    let detail = if detail.is_empty() {
      stderr.trim()
    } else {
      detail
    };
    return Err(deno_core::anyhow::anyhow!(
      "native tsc exited with {} without parseable diagnostics{}",
      output.status,
      if detail.is_empty() {
        String::new()
      } else {
        format!(":\n{detail}")
      }
    ));
  }

  Ok(())
}

async fn ensure_native_tsc_downloaded(
  factory: &CliFactory,
) -> Result<PathBuf, AnyError> {
  let installer_factory = factory.npm_installer_factory()?;
  let deno_dir = factory.deno_dir()?;
  let npmrc = factory.npmrc()?;
  let npm_registry_info = installer_factory.registry_info_provider()?;
  let resolver_factory = factory.resolver_factory()?;
  let workspace_factory = resolver_factory.workspace_factory();

  crate::tsc::native::ensure_native_tsc(
    deno_dir,
    npmrc,
    npm_registry_info,
    workspace_factory.workspace_npm_link_packages()?,
    installer_factory.tarball_cache()?,
    factory.npm_cache()?,
  )
  .await
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

fn parse_tsc_diagnostics(output: &str, project_root: &Path) -> Diagnostics {
  let mut diagnostics: Vec<Diagnostic> = Vec::new();

  for line in output.lines() {
    if let Some(caps) = DIAGNOSTIC_RE.captures(line) {
      let line_no: u64 = caps["line"].parse().unwrap_or(1);
      let col_no: u64 = caps["col"].parse().unwrap_or(1);
      diagnostics.push(make_diagnostic(
        &caps["cat"],
        caps["code"].parse().unwrap_or(0),
        Some(remap_path(&caps["file"], project_root)),
        Some(Position {
          // tsc positions are 1-based; Deno's `Position` is 0-based and adds
          // one back when rendering.
          line: line_no.saturating_sub(1),
          character: col_no.saturating_sub(1),
        }),
        caps["msg"].to_string(),
      ));
    } else if let Some(caps) = DIAGNOSTIC_NO_POS_RE.captures(line) {
      diagnostics.push(make_diagnostic(
        &caps["cat"],
        caps["code"].parse().unwrap_or(0),
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

  Diagnostics::from(diagnostics)
}

fn make_diagnostic(
  category: &str,
  code: u64,
  file_name: Option<String>,
  start: Option<Position>,
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
    end: None,
    original_source_start: None,
    message_text: Some(message_text),
    message_chain: None,
    source: None,
    source_line: None,
    file_name,
    related_information: None,
    reports_deprecated: None,
    reports_unnecessary: None,
    other: Default::default(),
    missing_specifier: None,
  }
}

/// Best-effort remap of a path reported by the native compiler back onto the
/// original module specifier. `deno check` runs tsc against a generated
/// tsconfig whose `paths` point jsr:/http(s): dependencies into mirror
/// directories, so the compiler reports those mirror paths rather than the
/// specifiers the user wrote.
fn remap_path(raw: &str, project_root: &Path) -> String {
  let normalized = raw.replace('\\', "/");

  // http(s): dependencies mirrored under `.deno/remote/<host>/<path>`.
  if let Some(rest) = normalized.split(".deno/remote/").nth(1) {
    return format!("https://{rest}");
  }

  // jsr: dependencies installed under `node_modules/@jsr/<scope>__<name>/...`.
  if let Some(rest) = normalized.split("node_modules/@jsr/").nth(1) {
    if let Some((pkg, sub)) = rest.split_once('/') {
      if let Some((scope, name)) = pkg.split_once("__") {
        return format!("jsr:@{scope}/{name}/{sub}");
      }
    }
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
