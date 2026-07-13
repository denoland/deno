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
  let has_explicit_roots = !check_flags.files.is_empty();
  let root_patterns = if has_explicit_roots {
    check_flags.files.clone()
  } else {
    vec![".".to_string()]
  };
  let graph_container = factory.main_module_graph_container().await?;
  let roots = graph_container.collect_specifiers(
    &root_patterns,
    crate::graph_container::CollectSpecifiersOptions {
      include_ignored_specified: has_explicit_roots,
    },
  )?;
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
      roots,
      imports,
      loader: None,
      npm_caching: cli_options.default_npm_caching_strategy(),
    })
    .await?;

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
  let (_missing_diagnostics, maybe_check_hash) = type_checker
    .walk_graph_for_native_check(
      &graph,
      cli_options.ts_type_lib_window(),
      cli_options.type_check_mode(),
    )?;
  let type_check_cache = type_checker.type_check_cache();

  // Cache hit: the hash is only recorded after a clean check, so a match means
  // the project type-checked cleanly and nothing the compiler sees has changed.
  // Skip both the (expensive) type materialization and the tsc spawn.
  if !cli_options.reload_flag()
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

  // Point tsc at the user's root `tsconfig.json` (which now extends
  // `.deno/tsconfig.json`) only when Deno is honoring it; otherwise use the
  // generated config directly so a tsconfig Deno ignores can't leak its options
  // in (and so we never had to rewrite it). See `sync_types_command`.
  let config_disabled =
    matches!(flags.config_flag, crate::args::ConfigFlag::Disabled);
  let honor_user_tsconfig =
    crate::tsc::tsconfig_gen::should_honor_user_tsconfig(
      &project_root,
      config_disabled,
    );
  let root_tsconfig = project_root.join("tsconfig.json");
  let tsconfig_path = if honor_user_tsconfig && root_tsconfig.exists() {
    root_tsconfig
  } else {
    project_root.join(".deno").join("tsconfig.json")
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
  let diagnostics =
    Diagnostics::from(parse_tsc_diagnostics(&stdout, &project_root));

  // Captured for an upcoming "Checked N files" summary; not surfaced yet.
  let stats = parse_tsc_stats(&stdout);
  log::debug!("native tsc {stats:?}");

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
        Some(remap_path(&caps["file"], project_root)),
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
    assert_eq!(
      remap_path(".deno/remote/html.spec.whatwg.org/entities.json", root),
      "https://html.spec.whatwg.org/entities.json"
    );
    assert_eq!(
      remap_path("node_modules/@jsr/std__assert/mod.ts", root),
      "jsr:@std/assert/mod.ts"
    );
    assert_eq!(
      remap_path("node_modules/chalk/index.d.ts", root),
      "npm:chalk/index.d.ts"
    );
    // A backslash-separated jsr mirror path (Windows form) still remaps.
    assert_eq!(
      remap_path("node_modules\\@jsr\\std__fmt/colors.ts", root),
      "jsr:@std/fmt/colors.ts"
    );
    // A project-local file becomes an absolute file URL.
    let mapped = remap_path("src/mod.ts", root);
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
