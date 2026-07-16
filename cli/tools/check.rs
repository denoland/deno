// Copyright 2018-2026 the Deno authors. MIT license.

use std::collections::HashMap;
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
use crate::args::SyncTypesFlags;
use crate::args::TypeCheckModeExt;
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

  let factory = CliFactory::from_flags(flags.clone());
  let cli_options = factory.cli_options()?;
  let project_root = cli_options
    .workspace()
    .root_dir_url()
    .to_file_path()
    .map_err(|_| {
      deno_core::anyhow::anyhow!("workspace root is not a local directory")
    })?;

  // Build the module graph over the requested roots (or the whole project).
  // Deno owns resolution: this drives deno's own graph diagnostics (missing
  // modules + hints) and the incremental type-check cache, so we can skip the
  // external compiler entirely when nothing it sees has changed.
  // Resolve the requested roots exactly as deno's own checker does on `main`:
  // globs are expanded, workspace `exclude` is applied, and
  // `include_ignored_specified: false` means an explicitly-passed excluded file
  // is skipped rather than force-checked. An empty result is not an error - it
  // just means there's nothing to check (e.g. every match was excluded), which
  // deno reports as a warning and a clean exit.
  let graph_container = factory.main_module_graph_container().await?;
  let roots = graph_container.collect_specifiers(
    &check_flags.files,
    crate::graph_container::CollectSpecifiersOptions {
      include_ignored_specified: false,
    },
  )?;
  if roots.is_empty() {
    log::warn!("{} No matching files found.", colors::yellow("Warning"));
    return Ok(());
  }
  let graph_kind = cli_options.type_check_mode().as_graph_kind();
  let imports = factory
    .module_graph_builder()
    .await?
    .maybe_resolve_ts_config_imports(graph_kind);
  let graph = factory
    .module_graph_creator()
    .await?
    .create_graph_with_options(crate::graph_util::CreateGraphOptions {
      is_dynamic: false,
      graph_kind,
      roots: roots.clone(),
      imports,
      loader: None,
      npm_caching: cli_options.default_npm_caching_strategy(),
    })
    .await?;

  // Surface deno's own graph-resolution errors before handing off to tsc:
  // unsupported import attributes (e.g. `.css` without --unstable-raw-imports),
  // `compilerOptions.paths` that resolve to nothing, integrity/npm-resolution
  // failures, and invalid specifiers. Missing-module errors are deliberately
  // *not* surfaced here (`will_type_check` defers them to tsc, which reports
  // them as TS2307), so a missing import is never double-reported. Note: the
  // richer `deno add` hint the in-process checker attached to import-level
  // missing modules is not re-added on top of tsc's TS2307 yet; restoring it
  // additively is a follow-up. `allow_unknown_media_types: true` matches
  // `deno check`, letting tsc handle unknown types instead of erroring.
  factory
    .module_graph_builder()
    .await?
    .graph_roots_valid(&graph, &roots, true, false)?;

  // Enforce the lockfile now that the graph has resolved the project's deps:
  // under `--frozen` this errors if the lockfile is out of date, otherwise it
  // writes the updated lockfile. The in-process checker does this via the
  // module loader; native check builds the graph itself, so do it here - before
  // materializing types and spawning tsc, so `--frozen` fails fast.
  if let Some(lockfile) = factory.maybe_lockfile().await? {
    lockfile.write_if_changed()?;
  }

  // Walk the graph exactly as the in-process checker does to obtain the combined
  // check hash (tsc version folded in). The walk also produces deno's own graph
  // diagnostics (missing modules + hints), but merging them additively isn't
  // safe yet: the in-process checker filters those against tsc's reported
  // ambient-module list to suppress false "missing module" errors for `declare
  // module` shims (including our own `*.css`). Native tsc doesn't hand us that
  // list, so deno would flag ambient specifiers tsc resolves fine. Use the walk
  // only for caching for now; additive diagnostics come once ambient modules
  // are handled.
  let type_checker = factory.type_checker().await?;
  let (missing_diagnostics, maybe_check_hash) = type_checker
    .walk_graph_for_native_check(
      &graph,
      cli_options.ts_type_lib_window(),
      cli_options.type_check_mode(),
    )?;

  // A root (entrypoint) that deno's graph couldn't resolve - a local file that
  // doesn't exist, or a remote URL that failed to fetch - can't be type-checked.
  // Handing a local one to tsc yields a leaky "File not found. Part of 'files'
  // list in tsconfig.json"; a remote one isn't a tsc `files` entry at all, so it
  // silently falls back to checking the whole project. Deno's graph walk already
  // produced the proper "Cannot find module" diagnostic for the root (the walk
  // only emits one when the root failed to resolve), so surface that and keep
  // the root out of tsc. Missing *imports* stay deferred to tsc (which reports
  // TS2307 for them), so this only owns the entrypoints.
  let root_urls: Vec<String> = roots.iter().map(|s| s.to_string()).collect();
  let root_diagnostics = missing_diagnostics.filter(|d| {
    d.missing_specifier
      .as_ref()
      .is_some_and(|s| root_urls.contains(s))
  });
  let type_check_cache = type_checker.type_check_cache();

  // Cache hit: the hash is only recorded after a clean check, so a match means
  // the project type-checked cleanly and nothing the compiler sees has changed.
  // Skip both the (expensive) type materialization and the tsc spawn. A missing
  // root means there's a diagnostic to report, so never take the cache path.
  if !cli_options.reload_flag()
    && !root_diagnostics.has_diagnostic()
    && let Some(check_hash) = maybe_check_hash
    && type_check_cache.has_check_hash(check_hash)
  {
    log::debug!("Already type checked (native tsc)");
    return Ok(());
  }

  // Cache miss: generate the tsconfig.json and materialize dependency types so
  // the native compiler can resolve the project's jsr:/npm:/http(s): imports.
  // Suppress sync-types' own progress/summary output (an internal step here) so
  // it doesn't precede the type-check diagnostics.
  let prev_level = log::max_level();
  log::set_max_level(log::LevelFilter::Error);
  let sync_result = crate::tools::installer::sync_types_command(
    flags.clone(),
    SyncTypesFlags {
      roots: check_flags.files.clone(),
    },
    crate::tools::installer::RootTsConfigMode::CheckMode,
  )
  .await;
  log::set_max_level(prev_level);
  sync_result?;

  let tsc_path = ensure_native_tsc_downloaded(&factory).await?;

  // When Deno honors a user `tsconfig.json`, base tsc on a throwaway overlay of
  // it (its options + our generated `extends`/`references`) written to a temp
  // file in the project root - so the user's path-based options (rootDirs,
  // baseUrl, include/files) resolve relative to the project, WITHOUT us
  // rewriting their committed file. Otherwise point tsc at the generated config
  // directly. See `sync_types_command` / `build_check_root_overlay`.
  let config_disabled =
    matches!(flags.config_flag, crate::args::ConfigFlag::Disabled);
  let honor_user_tsconfig =
    crate::tsc::tsconfig_gen::should_honor_user_tsconfig(
      &project_root,
      config_disabled,
    );
  let root_tsconfig = project_root.join("tsconfig.json");
  // Holds the root overlay temp file open until tsc has run (dropping deletes
  // it), keeping `deno check` side-effect-free on the user's tree.
  let _root_tsconfig_guard;
  let base_tsconfig = if honor_user_tsconfig && root_tsconfig.exists() {
    let overlay = crate::tsc::tsconfig_gen::build_check_root_overlay(
      &project_root,
      &root_tsconfig,
    )?;
    let mut tmp = tempfile::Builder::new()
      .prefix("deno-check-root-")
      .suffix(".tsconfig.json")
      .tempfile_in(&project_root)?;
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
  files.extend(remote_root_mirror_files(&project_root, &roots));

  // Every requested root was a missing (non-existent) local entrypoint: there's
  // nothing for tsc to check, so report deno's graph diagnostics for them
  // directly instead of falling back to the base config's project-wide
  // `include` (which would check unrelated files).
  if files.is_empty() && root_diagnostics.has_diagnostic() {
    log::info!(
      "{} {}",
      colors::green("Check"),
      colors::gray(format!("(tsc {})", crate::tsc::native::TYPESCRIPT_VERSION))
    );
    log::error!("{}\n", root_diagnostics);
    return Err(deno_core::anyhow::anyhow!("Type checking failed."));
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
    // Write to a unique temp file rather than a fixed path: sibling `deno check`
    // runs in the same directory (e.g. spec-test variants) would otherwise race
    // on it, and a fixed path leaves an artifact behind.
    let content = deno_core::serde_json::json!({
      "extends": base_tsconfig.to_string_lossy().replace('\\', "/"),
      "include": [],
      "files": files,
    });
    let mut tmp = tempfile::Builder::new()
      .prefix("deno-check-")
      .suffix(".tsconfig.json")
      .tempfile_in(&project_root)?;
    std::io::Write::write_all(
      &mut tmp,
      deno_core::serde_json::to_string_pretty(&content)?.as_bytes(),
    )?;
    let path = tmp.path().to_path_buf();
    _check_tsconfig_guard = tmp;
    path
  };

  log::info!(
    "{} {}",
    colors::green("Check"),
    colors::gray(format!("(tsc {})", crate::tsc::native::TYPESCRIPT_VERSION))
  );

  // `--pretty false` yields the stable one-line-per-diagnostic grep format
  // (`path(line,col): error TS####: message`) that we parse below. The
  // generated tsconfig already sets `noEmit`; pass it too so a stray option
  // can't trigger emit. `--diagnostics` appends a stats block (files/lines/
  // timing/memory) that we capture but do not surface yet.
  let output = tokio::process::Command::new(&tsc_path)
    .arg("--project")
    .arg(&tsconfig_path)
    .arg("--noEmit")
    .arg("--pretty")
    .arg("false")
    .arg("--diagnostics")
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
  let mut diagnostics =
    Diagnostics::from(parse_tsc_diagnostics(&stdout, &project_root));
  // Fold in deno's own diagnostics for missing entrypoints (some roots existed
  // and were type-checked by tsc above; any that didn't are reported here).
  diagnostics.extend(root_diagnostics);

  // Captured for an upcoming "Checked N files" summary; not surfaced yet.
  let stats = parse_tsc_stats(&stdout);
  log::debug!("native tsc {stats:?}");

  if diagnostics.has_diagnostic() {
    log::error!("{}\n", diagnostics);
    return Err(deno_core::anyhow::anyhow!("Type checking failed."));
  }

  // tsc exited non-zero but we have nothing to show. If it printed diagnostic
  // lines we deliberately dropped (e.g. `noImplicitOverride` in remote modules),
  // its exit code is expected - treat the check as clean. Only when tsc produced
  // no recognizable diagnostics at all is this a genuine failure (an internal
  // error or a malformed generated config); surface whatever it printed then.
  let tsc_reported_diagnostics = stdout
    .lines()
    .any(|l| DIAGNOSTIC_RE.is_match(l) || DIAGNOSTIC_NO_POS_RE.is_match(l));
  if !output.status.success() && !tsc_reported_diagnostics {
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

  // Type-checked clean: record the hash so an unchanged re-check skips tsc.
  if let Some(check_hash) = maybe_check_hash {
    type_check_cache.add_check_hash(check_hash);
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

fn parse_tsc_diagnostics(output: &str, project_root: &Path) -> Vec<Diagnostic> {
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
    !matches!(d.code, 4113..=4116)
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
#[allow(
  dead_code,
  reason = "fields consumed by a follow-up that adds a \"Checked N files\" summary"
)]
struct TscStats {
  files: Option<u64>,
  lines: Option<u64>,
  identifiers: Option<u64>,
  symbols: Option<u64>,
  types: Option<u64>,
  memory_used: Option<String>,
  check_time: Option<String>,
  total_time: Option<String>,
}

/// Parse the `Key:  value` stats block emitted by `tsc --diagnostics`. Lines
/// that aren't recognized stat keys (including the diagnostics themselves) are
/// ignored.
fn parse_tsc_stats(output: &str) -> TscStats {
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
  let mut map = HashMap::new();
  let config = project_root.join(".deno").join("tsconfig.json");
  let Ok(text) = std::fs::read_to_string(&config) else {
    return map;
  };
  let Ok(value) =
    deno_core::serde_json::from_str::<deno_core::serde_json::Value>(&text)
  else {
    return map;
  };
  let Some(paths) = value
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
