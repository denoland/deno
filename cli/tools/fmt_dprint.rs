// Copyright 2018-2026 the Deno authors. MIT license.

//! Prototype: format files using the pinned `dprint` npm package + its WASM
//! plugins, instead of the in-process dprint plugin crates
//! (`dprint-plugin-typescript`, `malva`, `markup_fmt`, ...).
//!
//! Gated behind the `DENO_FMT_DPRINT` environment variable so the existing
//! in-process formatter stays the default. Two backends are implemented for
//! comparison, selected by the variable's value:
//!   * `1` / `true` / `cli` - shell out to the pinned `dprint` CLI npm package
//!     in a `deno run` subprocess, once per batch (see [`run_dprint`]).
//!   * `wasm` - run the dprint WASM plugins *in the same process* by embedding a
//!     `MainWorker` (like `deno lint`'s plugin host), loading the WASM plugins
//!     through `@dprint/formatter` from pinned `@dprint/*` npm packages, and
//!     calling into the embedded `fmt_dprint_formatter.js` (see [`run_wasm`]).
//!     No subprocess; the plugins run on the embedded runtime's V8 WebAssembly.
//!
//! Both backends share the same machinery: they reuse Deno's own file discovery
//! (gitignore / workspace / `deno.json` `fmt` include+exclude) to decide *which*
//! files to format, and generate dprint config from the resolved `deno.json`
//! `fmt` options + CLI flags (mapping the full [`FmtOptionsConfig`] set).
//!
//! Known prototype limitations: SQL formatting goes through the dprint SQL
//! plugin, which differs from the in-process `sqlformat` crate; the WASM
//! backend loads CSS/HTML/YAML plugins from plugins.dprint.dev (not npm) so it
//! isn't fully offline yet; and stdin always uses the CLI backend.

use std::io::Read;
use std::io::Write;
use std::io::stdin;
use std::io::stdout;
use std::path::Path;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;

use deno_config::glob::FilePatterns;
use deno_core::PollEventLoopOptions;
use deno_core::anyhow::Context;
use deno_core::anyhow::anyhow;
use deno_core::anyhow::bail;
use deno_core::error::AnyError;
use deno_core::serde_json;
use deno_core::serde_json::Value;
use deno_core::serde_json::json;
use deno_core::serde_v8;
use deno_core::v8;
use deno_path_util::resolve_url_or_path;
use deno_path_util::url_from_file_path;
use deno_runtime::WorkerExecutionMode;
use deno_runtime::tokio_util;
use tokio::process::Command;

use crate::args::Flags;
use crate::args::FmtFlags;
use crate::args::FmtOptions;
use crate::args::FmtOptionsConfig;
use crate::args::PermissionFlags;
use crate::args::ProseWrap;
use crate::factory::CliFactory;

/// Pinned `dprint` CLI version (npm package). This is the single version baked
/// into the binary; bumping it here changes the default for every `deno fmt`.
const DPRINT_NPM_VERSION: &str = "0.47.2";

/// Pinned set of `dprint` WASM plugins, one per supported file category. These
/// URLs are content-addressed by version, so the behaviour is reproducible.
const DPRINT_PLUGINS: &[&str] = &[
  // TypeScript / JavaScript / JSX / TSX.
  "https://plugins.dprint.dev/typescript-0.96.0.wasm",
  // JSON / JSONC.
  "https://plugins.dprint.dev/json-0.21.3.wasm",
  // Markdown (+ embedded code fences).
  "https://plugins.dprint.dev/markdown-0.21.1.wasm",
  // TOML.
  "https://plugins.dprint.dev/toml-0.7.0.wasm",
  // CSS / SCSS / SASS / LESS.
  "https://plugins.dprint.dev/g-plane/malva-v0.16.0.wasm",
  // HTML / Vue / Svelte / Astro / Angular templates.
  "https://plugins.dprint.dev/g-plane/markup_fmt-v0.27.2.wasm",
  // YAML.
  "https://plugins.dprint.dev/g-plane/pretty_yaml-v0.6.0.wasm",
  // SQL. NOTE: this is the dprint SQL plugin, which is a different formatter
  // from the in-process `sqlformat` crate, so output may differ slightly.
  "https://plugins.dprint.dev/sql-0.3.0.wasm",
  // Jupyter notebooks (.ipynb) - delegates cell formatting to the plugins
  // above (typescript / markdown / ...).
  "https://plugins.dprint.dev/jupyter-0.2.3.wasm",
];

/// File extensions covered by [`DPRINT_PLUGINS`]. Used to keep us from handing
/// the `dprint` CLI a file none of the pinned plugins can format (which it
/// would report as an error).
const DPRINT_SUPPORTED_EXTS: &[&str] = &[
  "ts", "tsx", "js", "jsx", "mjs", "cjs", "mts", "cts", // typescript
  "json", "jsonc", // json
  "md", "markdown", // markdown
  "toml",     // toml
  "css", "scss", "sass", "less", // malva
  "html", "vue", "svelte", "astro", // markup_fmt
  "yaml", "yml",   // pretty_yaml
  "sql",   // sql
  "ipynb", // jupyter
];

/// The embedded JS worker for the in-process WASM backend.
const WASM_FORMATTER_JS: &str = include_str!("fmt_dprint_formatter.js");

/// Which prototype formatting backend to use.
enum Backend {
  /// Shell out to the pinned `dprint` CLI (npm package) once per batch.
  Cli,
  /// Run the dprint WASM plugins in-process through `@dprint/formatter`,
  /// loaded from pinned npm packages (see [`WASM_FORMATTER_JS`]).
  Wasm,
}

/// Whether the `dprint`-npm prototype should handle `deno fmt`.
pub fn is_enabled() -> bool {
  matches!(
    std::env::var("DENO_FMT_DPRINT").as_deref(),
    Ok("1") | Ok("true") | Ok("cli") | Ok("wasm")
  )
}

/// Reads the backend selection from `DENO_FMT_DPRINT` (defaults to the CLI
/// backend for `1`/`true`).
fn backend() -> Backend {
  match std::env::var("DENO_FMT_DPRINT").as_deref() {
    Ok("wasm") => Backend::Wasm,
    _ => Backend::Cli,
  }
}

/// Entry point mirroring [`crate::tools::fmt::format`], but backed by the
/// `dprint` CLI.
pub async fn format(
  flags: Arc<Flags>,
  fmt_flags: FmtFlags,
) -> Result<(), AnyError> {
  let quiet = flags.log_level == Some(log::Level::Error);
  let factory = CliFactory::from_flags(flags.clone());
  let cli_options = factory.cli_options()?;

  if fmt_flags.is_stdin() {
    let start_dir = &cli_options.start_dir;
    let fmt_config = start_dir
      .to_fmt_config(FilePatterns::new_with_base(start_dir.dir_path()))?;
    let fmt_options = FmtOptions::resolve(
      fmt_config,
      cli_options.resolve_config_unstable_fmt_options(),
      &fmt_flags,
    );
    let ext = cli_options
      .ext_flag()
      .clone()
      .unwrap_or_else(|| "ts".to_string());
    return format_stdin(&fmt_flags, &fmt_options, &ext).await;
  }

  // Reuse Deno's existing per-member option resolution + file discovery
  // (gitignore, node_modules skipping, workspace + deno.json include/exclude).
  let batches = crate::tools::fmt::resolve_paths_with_options_batches(
    cli_options,
    &fmt_flags,
  )?;

  // Drop files no pinned plugin can handle so the formatter doesn't error.
  let batches = batches.into_iter().filter_map(|batch| {
    let paths: Vec<PathBuf> = batch
      .paths
      .into_iter()
      .filter(|p| is_dprint_supported(p))
      .collect();
    (!paths.is_empty()).then_some((batch.options.options, batch.base, paths))
  });

  match backend() {
    Backend::Cli => {
      let mut any_failed = false;
      for (options, base, paths) in batches {
        let config_path = write_temp_config(&options, &base)?;
        let result = run_dprint(&fmt_flags, &config_path, &paths).await;
        // Best effort cleanup of the generated config.
        let _ = std::fs::remove_file(&config_path);
        if !result? {
          any_failed = true;
        }
      }
      if any_failed {
        bail!("Found errors.");
      }
    }
    Backend::Wasm => {
      // The WASM backend runs every batch in a single embedded runtime, so
      // collect all the jobs first and hand them off together.
      let jobs: Vec<Value> = batches
        .map(|(options, _base, paths)| {
          build_wasm_job(&options, &fmt_flags, &paths, quiet)
        })
        .collect();
      if !jobs.is_empty() && !run_wasm(flags, jobs).await? {
        bail!("Found errors.");
      }
    }
  }
  Ok(())
}

/// Formats (or checks) stdin by piping it through `dprint fmt --stdin`.
async fn format_stdin(
  fmt_flags: &FmtFlags,
  fmt_options: &FmtOptions,
  ext: &str,
) -> Result<(), AnyError> {
  let mut source = String::new();
  stdin()
    .read_to_string(&mut source)
    .context("Failed to read from stdin")?;

  // The config content doesn't depend on a base directory; "stdin" just
  // gives the generated temp file a stable, unique-enough name.
  let config_path =
    write_temp_config(&fmt_options.options, Path::new("stdin"))?;

  // dprint always *formats* on `--stdin`; we emulate `--check` ourselves by
  // comparing the formatted output to the input.
  let deno_exe = std::env::current_exe()?;
  let mut cmd = Command::new(&deno_exe);
  cmd
    .arg("run")
    .arg("-A")
    .arg("--quiet")
    .arg("--no-config")
    .arg(format!("npm:dprint@{DPRINT_NPM_VERSION}"))
    .arg("fmt")
    .arg(format!("--config={}", config_path.display()))
    .arg("--stdin")
    .arg(ext)
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::inherit());

  let mut child = cmd.spawn().context("Failed to spawn dprint")?;
  {
    use tokio::io::AsyncWriteExt;
    let mut child_stdin = child.stdin.take().unwrap();
    child_stdin.write_all(source.as_bytes()).await?;
    child_stdin.shutdown().await?;
  }
  let output = child.wait_with_output().await?;
  let _ = std::fs::remove_file(&config_path);

  if !output.status.success() {
    bail!("dprint failed to format stdin");
  }
  let formatted = String::from_utf8(output.stdout)
    .context("dprint produced non-UTF-8 output")?;

  if fmt_flags.check {
    if formatted != source {
      // Mirror the wording used by the in-process formatter.
      #[allow(clippy::print_stdout, reason = "actually want to output")]
      {
        println!("Not formatted stdin");
      }
      bail!("Found errors.");
    }
  } else {
    stdout().write_all(formatted.as_bytes())?;
  }
  Ok(())
}

/// Runs the pinned `dprint` CLI over `paths`. Returns `Ok(true)` on success,
/// `Ok(false)` when dprint reported formatting problems (non-zero exit).
async fn run_dprint(
  fmt_flags: &FmtFlags,
  config_path: &Path,
  paths: &[PathBuf],
) -> Result<bool, AnyError> {
  let subcommand = if fmt_flags.check { "check" } else { "fmt" };
  let deno_exe = std::env::current_exe()?;

  let mut cmd = Command::new(&deno_exe);
  cmd
    .arg("run")
    .arg("-A")
    // Quiet the npm "ignored build scripts" warning emitted when caching the
    // dprint package; we only want dprint's own output.
    .arg("--quiet")
    .arg("--no-config")
    .arg(format!("npm:dprint@{DPRINT_NPM_VERSION}"))
    .arg(subcommand)
    .arg(format!("--config={}", config_path.display()))
    .stdout(Stdio::inherit())
    .stderr(Stdio::inherit());
  // All file paths and the config path are absolute, so the working directory
  // is irrelevant (we pass explicit files rather than relying on dprint's own
  // include/exclude globbing).
  for path in paths {
    cmd.arg(path);
  }

  let status = cmd
    .status()
    .await
    .context("Failed to run the pinned dprint CLI")?;
  Ok(status.success())
}

/// Builds the JSON job for one batch consumed by the WASM worker
/// (`fmt_dprint_formatter.js`).
fn build_wasm_job(
  options: &FmtOptionsConfig,
  fmt_flags: &FmtFlags,
  paths: &[PathBuf],
  quiet: bool,
) -> Value {
  json!({
    "check": fmt_flags.check,
    "quiet": quiet,
    "global": build_global_config(options),
    "plugins": build_plugin_sections(options),
    "files": paths
      .iter()
      .map(|p| p.to_string_lossy().into_owned())
      .collect::<Vec<_>>(),
  })
}

#[derive(serde::Deserialize)]
struct WasmRunResult {
  failed: bool,
}

/// Runs the in-process WASM backend: spins up an embedded `MainWorker` (NOT a
/// `deno run` subprocess) that loads the dprint WASM plugins through
/// `@dprint/formatter` and formats every batch. Mirrors `deno lint`'s plugin
/// host: the runtime runs on a dedicated current-thread tokio runtime to stay
/// isolated from the fmt async context. Returns `Ok(true)` on success.
async fn run_wasm(
  flags: Arc<Flags>,
  jobs: Vec<Value>,
) -> Result<bool, AnyError> {
  let jobs_json = serde_json::to_string(&jobs)?;
  let (tx, rx) = tokio::sync::oneshot::channel();
  std::thread::spawn(move || {
    let result = tokio_util::create_and_run_current_thread(run_wasm_worker(
      flags, jobs_json,
    ));
    let _ = tx.send(result);
  });
  rx.await
    .context("in-process WASM formatter thread panicked")?
}

async fn run_wasm_worker(
  flags: Arc<Flags>,
  jobs_json: String,
) -> Result<bool, AnyError> {
  // The worker needs to read+write files and fetch the URL-hosted plugins,
  // and resolve the `@dprint/*` npm packages, so grant full permissions.
  let mut flags = (*flags).clone();
  flags.permissions = PermissionFlags {
    allow_all: true,
    no_prompt: true,
    ..Default::default()
  };
  let flags = Arc::new(flags);
  let factory = CliFactory::from_flags(flags);
  let cli_options = factory.cli_options()?;

  let script_path = write_temp_file(
    "worker",
    Path::new("wasm"),
    "mjs",
    WASM_FORMATTER_JS.as_bytes(),
  )?;
  let module_specifier = url_from_file_path(&script_path)?;
  // Placeholder main module; the worker module is loaded as a side module.
  let main_module =
    resolve_url_or_path("./$deno$fmt_dprint.mjs", cli_options.initial_cwd())?;
  let permissions = factory.root_permissions_container()?.clone();
  let worker_factory = factory.create_cli_main_worker_factory().await?;
  let worker = worker_factory
    .create_custom_worker(
      WorkerExecutionMode::Run,
      main_module,
      vec![],
      vec![],
      permissions,
      vec![],
      Default::default(),
      None,
    )
    .await?;
  let mut worker = worker.into_main_worker();

  // Load + evaluate the worker module (this also resolves its npm imports).
  let mod_id = worker
    .js_runtime
    .load_side_es_module(&module_specifier)
    .await?;
  let eval = worker.js_runtime.mod_evaluate(mod_id);
  worker
    .js_runtime
    .run_event_loop(PollEventLoopOptions::default())
    .await?;
  eval.await?;
  let _ = std::fs::remove_file(&script_path);

  // Grab the exported `run` function and the jobs argument.
  let namespace = worker.js_runtime.get_module_namespace(mod_id)?;
  let (run_fn, jobs_arg) = {
    deno_core::scope!(scope, &mut worker.js_runtime);
    let namespace = v8::Local::new(scope, namespace);
    let key = v8::String::new(scope, "run").unwrap();
    let run_val = namespace
      .get(scope, key.into())
      .ok_or_else(|| anyhow!("worker module has no `run` export"))?;
    let run_fn: v8::Local<v8::Function> = run_val
      .try_into()
      .map_err(|_| anyhow!("`run` export is not a function"))?;
    let jobs_v8: v8::Local<v8::Value> =
      v8::String::new(scope, &jobs_json).unwrap().into();
    (
      v8::Global::new(scope, run_fn),
      v8::Global::new(scope, jobs_v8),
    )
  };

  // Call `run(jobsJson)` and drive the event loop until its promise resolves.
  let call = worker.js_runtime.call_with_args(&run_fn, &[jobs_arg]);
  let result = worker
    .js_runtime
    .with_event_loop_promise(call, PollEventLoopOptions::default())
    .await?;

  let result: WasmRunResult = {
    deno_core::scope!(scope, &mut worker.js_runtime);
    let local = v8::Local::new(scope, result);
    serde_v8::from_v8(scope, local)?
  };
  Ok(!result.failed)
}

fn is_dprint_supported(path: &Path) -> bool {
  match path.extension().and_then(|e| e.to_str()) {
    Some(ext) => {
      let ext = ext.to_ascii_lowercase();
      DPRINT_SUPPORTED_EXTS.contains(&ext.as_str())
    }
    None => false,
  }
}

/// Generates a `dprint` config from the resolved `deno fmt` options.
///
/// We start each language plugin from its `"deno": true` preset (the same
/// preset Deno's own `.dprint.json` uses) so the defaults match `deno fmt`,
/// then layer every field of [`FmtOptionsConfig`] onto the plugin that owns it.
/// This mirrors `cli/tools/fmt.rs::get_typescript_config_builder` and friends,
/// but emits dprint's JSON config-file keys instead of calling the builders.
fn build_dprint_config(options: &FmtOptionsConfig) -> Value {
  let mut config = serde_json::Map::new();

  // Global options, applied by dprint to every plugin that supports them
  // (e.g. malva/markup/yaml/toml/sql, which have no "deno" preset).
  if let Value::Object(global) = build_global_config(options) {
    config.extend(global);
  }

  if let Value::Object(plugins) = build_plugin_sections(options) {
    config.extend(plugins);
  }

  config.insert("plugins".into(), json!(DPRINT_PLUGINS));

  Value::Object(config)
}

/// The dprint "global" config (shared by all plugins): width, indentation,
/// tabs and newline kind.
fn build_global_config(options: &FmtOptionsConfig) -> Value {
  let mut global = serde_json::Map::new();
  if let Some(line_width) = options.line_width {
    global.insert("lineWidth".into(), json!(line_width));
  }
  if let Some(indent_width) = options.indent_width {
    global.insert("indentWidth".into(), json!(indent_width));
  }
  if let Some(use_tabs) = options.use_tabs {
    global.insert("useTabs".into(), json!(use_tabs));
  }
  if let Some(new_line_kind) = options.new_line_kind {
    global.insert(
      "newLineKind".into(),
      json!(new_line_kind_str(new_line_kind)),
    );
  }
  Value::Object(global)
}

/// The per-plugin config sections, keyed by plugin name. Shared by both the
/// CLI backend (embedded in the dprint config file) and the WASM backend
/// (passed to `@dprint/formatter`'s `addPlugin`). TOML/SQL/Jupiter take only
/// the global options, so they are omitted here.
fn build_plugin_sections(options: &FmtOptionsConfig) -> Value {
  let mut sections = serde_json::Map::new();
  sections.insert("typescript".into(), build_typescript_config(options));
  sections.insert("json".into(), build_json_config(options));
  sections.insert("markdown".into(), build_markdown_config(options));
  sections.insert("malva".into(), build_malva_config(options));
  sections.insert("yaml".into(), build_yaml_config(options));
  if let Some(markup) = build_markup_config(options) {
    sections.insert("markup".into(), markup);
  }
  Value::Object(sections)
}

/// Inserts the global width/tabs options into a plugin section that has a
/// `"deno": true` preset, so they override the preset (plugin-section config
/// has higher precedence than the global config in dprint).
fn insert_width_overrides(
  section: &mut serde_json::Map<String, Value>,
  options: &FmtOptionsConfig,
) {
  if let Some(use_tabs) = options.use_tabs {
    section.insert("useTabs".into(), json!(use_tabs));
  }
  if let Some(line_width) = options.line_width {
    section.insert("lineWidth".into(), json!(line_width));
  }
  if let Some(indent_width) = options.indent_width {
    section.insert("indentWidth".into(), json!(indent_width));
  }
}

fn build_typescript_config(options: &FmtOptionsConfig) -> Value {
  use deno_config::deno_json::*;

  let mut ts = serde_json::Map::new();
  ts.insert("deno".into(), json!(true));
  insert_width_overrides(&mut ts, options);

  // Only override the quote style when single quotes are requested, matching
  // the in-process builder (which leaves the Deno default otherwise).
  if options.single_quote == Some(true) {
    ts.insert("quoteStyle".into(), json!("preferSingle"));
  }
  if let Some(semi_colons) = options.semi_colons {
    ts.insert(
      "semiColons".into(),
      json!(if semi_colons { "prefer" } else { "asi" }),
    );
  }
  if let Some(quote_props) = options.quote_props {
    ts.insert(
      "quoteProps".into(),
      json!(match quote_props {
        QuoteProps::AsNeeded => "asNeeded",
        QuoteProps::Consistent => "consistent",
        QuoteProps::Preserve => "preserve",
      }),
    );
  }
  if let Some(use_braces) = options.use_braces {
    ts.insert(
      "useBraces".into(),
      json!(match use_braces {
        UseBraces::Maintain => "maintain",
        UseBraces::WhenNotSingleLine => "whenNotSingleLine",
        UseBraces::Always => "always",
        UseBraces::PreferNone => "preferNone",
      }),
    );
  }
  if let Some(brace_position) = options.brace_position {
    ts.insert(
      "bracePosition".into(),
      json!(match brace_position {
        BracePosition::Maintain => "maintain",
        BracePosition::SameLine => "sameLine",
        BracePosition::NextLine => "nextLine",
        BracePosition::SameLineUnlessHanging => "sameLineUnlessHanging",
      }),
    );
  }
  if let Some(single_body_position) = options.single_body_position {
    ts.insert(
      "singleBodyPosition".into(),
      json!(same_or_next_line_str(single_body_position)),
    );
  }
  if let Some(pos) = options.next_control_flow_position {
    ts.insert(
      "nextControlFlowPosition".into(),
      json!(match pos {
        NextControlFlowPosition::Maintain => "maintain",
        NextControlFlowPosition::SameLine => "sameLine",
        NextControlFlowPosition::NextLine => "nextLine",
      }),
    );
  }
  if let Some(trailing_commas) = options.trailing_commas {
    ts.insert(
      "trailingCommas".into(),
      json!(match trailing_commas {
        TrailingCommas::Always => "always",
        TrailingCommas::Never => "never",
        TrailingCommas::OnlyMultiLine => "onlyMultiLine",
      }),
    );
  }
  if let Some(operator_position) = options.operator_position {
    let value = match operator_position {
      OperatorPosition::Maintain => "maintain",
      OperatorPosition::SameLine => "sameLine",
      OperatorPosition::NextLine => "nextLine",
    };
    // Deno's preset sets these per-AST-node, so we have to override the same
    // AST-specific keys rather than the top-level `operatorPosition` shorthand.
    ts.insert("binaryExpression.operatorPosition".into(), json!(value));
    ts.insert(
      "conditionalExpression.operatorPosition".into(),
      json!(value),
    );
    ts.insert("conditionalType.operatorPosition".into(), json!(value));
  }
  if let Some(jsx_bracket_position) = options.jsx_bracket_position {
    ts.insert(
      "jsx.bracketPosition".into(),
      json!(match jsx_bracket_position {
        BracketPosition::Maintain => "maintain",
        BracketPosition::SameLine => "sameLine",
        BracketPosition::NextLine => "nextLine",
      }),
    );
  }
  if let Some(value) = options.jsx_force_new_lines_surrounding_content {
    ts.insert("jsx.forceNewLinesSurroundingContent".into(), json!(value));
  }
  if let Some(jsx_multi_line_parens) = options.jsx_multi_line_parens {
    ts.insert(
      "jsx.multiLineParens".into(),
      json!(match jsx_multi_line_parens {
        MultiLineParens::Never => "never",
        MultiLineParens::Prefer => "prefer",
        MultiLineParens::Always => "always",
      }),
    );
  }
  if let Some(separator_kind) = options.type_literal_separator_kind {
    ts.insert(
      "typeLiteral.separatorKind".into(),
      json!(match separator_kind {
        SeparatorKind::SemiColon => "semiColon",
        SeparatorKind::Comma => "comma",
      }),
    );
  }
  if let Some(space_around) = options.space_around {
    ts.insert("spaceAround".into(), json!(space_around));
  }
  if let Some(value) = options.space_surrounding_properties {
    ts.insert("spaceSurroundingProperties".into(), json!(value));
    ts.insert(
      "importDeclaration.spaceSurroundingNamedImports".into(),
      json!(value),
    );
    ts.insert(
      "exportDeclaration.spaceSurroundingNamedExports".into(),
      json!(value),
    );
  }

  Value::Object(ts)
}

fn build_json_config(options: &FmtOptionsConfig) -> Value {
  let mut json = serde_json::Map::new();
  json.insert("deno".into(), json!(true));
  insert_width_overrides(&mut json, options);
  Value::Object(json)
}

fn build_markdown_config(options: &FmtOptionsConfig) -> Value {
  let mut markdown = serde_json::Map::new();
  markdown.insert("deno".into(), json!(true));
  if let Some(line_width) = options.line_width {
    markdown.insert("lineWidth".into(), json!(line_width));
  }
  if let Some(prose_wrap) = options.prose_wrap {
    markdown.insert(
      "textWrap".into(),
      json!(match prose_wrap {
        ProseWrap::Always => "always",
        ProseWrap::Never => "never",
        // dprint calls the "preserve" behaviour "maintain".
        ProseWrap::Preserve => "maintain",
      }),
    );
  }
  Value::Object(markdown)
}

/// CSS / SCSS / SASS / LESS (malva plugin). Consumes the single-quote option;
/// width/indent/tabs come from the global config.
fn build_malva_config(options: &FmtOptionsConfig) -> Value {
  json!({ "quotes": quotes_str(options.single_quote) })
}

/// YAML (pretty_yaml plugin). Consumes the single-quote option.
fn build_yaml_config(options: &FmtOptionsConfig) -> Value {
  json!({ "quotes": quotes_str(options.single_quote) })
}

/// HTML / Vue / Svelte / Astro (markup_fmt plugin). Only emitted when one of
/// its options is set; the plugin defaults otherwise match `deno fmt`.
fn build_markup_config(options: &FmtOptionsConfig) -> Option<Value> {
  use deno_config::deno_json::VueComponentCase;

  let mut markup = serde_json::Map::new();
  if let Some(vue_component_case) = options.vue_component_case {
    markup.insert(
      "vueComponentCase".into(),
      json!(match vue_component_case {
        VueComponentCase::Ignore => "ignore",
        VueComponentCase::PascalCase => "pascalCase",
        VueComponentCase::KebabCase => "kebabCase",
      }),
    );
  }
  if let Some(value) = options.angular_next_control_flow_same_line {
    markup.insert("angularNextControlFlowSameLine".into(), json!(value));
  }
  if markup.is_empty() {
    None
  } else {
    Some(Value::Object(markup))
  }
}

fn quotes_str(single_quote: Option<bool>) -> &'static str {
  if single_quote == Some(true) {
    "preferSingle"
  } else {
    "preferDouble"
  }
}

fn new_line_kind_str(
  new_line_kind: deno_config::deno_json::NewLineKind,
) -> &'static str {
  use deno_config::deno_json::NewLineKind;
  match new_line_kind {
    NewLineKind::Auto => "auto",
    NewLineKind::LineFeed => "lf",
    NewLineKind::CarriageReturnLineFeed => "crlf",
    NewLineKind::System => "system",
  }
}

fn same_or_next_line_str(
  position: deno_config::deno_json::SingleBodyPosition,
) -> &'static str {
  use deno_config::deno_json::SingleBodyPosition;
  match position {
    SingleBodyPosition::Maintain => "maintain",
    SingleBodyPosition::SameLine => "sameLine",
    SingleBodyPosition::NextLine => "nextLine",
  }
}

/// Writes the generated dprint config to a temp file and returns its path.
fn write_temp_config(
  options: &FmtOptionsConfig,
  base: &Path,
) -> Result<PathBuf, AnyError> {
  let config = build_dprint_config(options);
  let contents = serde_json::to_vec_pretty(&config)?;
  write_temp_file("config", base, "json", &contents)
}

/// Writes `contents` to a uniquely-named temp file and returns its path. The
/// name is derived from the process id and `base` so concurrent batches and
/// concurrent `deno fmt` processes don't clash.
fn write_temp_file(
  kind: &str,
  base: &Path,
  ext: &str,
  contents: &[u8],
) -> Result<PathBuf, AnyError> {
  let mut hasher = std::collections::hash_map::DefaultHasher::new();
  std::hash::Hash::hash(&base, &mut hasher);
  let hash = std::hash::Hasher::finish(&hasher);
  let path = std::env::temp_dir().join(format!(
    "deno-fmt-dprint-{}-{}-{:x}.{}",
    kind,
    std::process::id(),
    hash,
    ext
  ));
  std::fs::write(&path, contents)
    .with_context(|| format!("Failed to write {}", path.display()))?;
  Ok(path)
}
