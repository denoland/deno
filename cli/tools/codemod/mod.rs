// Copyright 2018-2026 the Deno authors. MIT license.

//! The `deno codemod` subcommand.
//!
//! A codemod is a [`Deno.lint.Plugin`] whose rules report fixes. `deno codemod`
//! loads such a plugin and applies *every* reported fix to the targeted source
//! files. Where `deno lint --fix` is meant for continuously enforced rules,
//! `deno codemod` is meant for one-off, intentional, automated refactors (for
//! example migrating off a deprecated API).
//!
//! This is currently a thin layer on top of the lint plugin infrastructure:
//! the heavy lifting (AST serialization, the JS plugin runtime, the
//! fixer/quick-fix application loop) is all shared with `deno lint`.

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use deno_config::glob::FileCollector;
use deno_config::glob::FilePatterns;
use deno_config::glob::PathOrPatternSet;
use deno_core::anyhow::bail;
use deno_core::error::AnyError;
use deno_core::parking_lot::Mutex;
use deno_core::resolve_url_or_path;

use crate::args::CodemodFlags;
use crate::args::Flags;
use crate::colors;
use crate::factory::CliFactory;
use crate::sys::CliSys;
use crate::tools::fmt::run_parallelized;
use crate::tools::lint::CliLinter;
use crate::tools::lint::CliLinterOptions;
use crate::tools::lint::ConfiguredRules;
use crate::tools::lint::PluginLogger;
use crate::tools::lint::create_runner_and_load_plugins;
use crate::tools::lint::resolve_lint_config;
use crate::util::path::is_script_ext;

#[allow(clippy::print_stdout, reason = "plugin logger")]
#[allow(clippy::print_stderr, reason = "plugin logger")]
fn logger_printer(msg: &str, is_err: bool) {
  if is_err {
    eprint!("{}", msg);
  } else {
    print!("{}", msg);
  }
}

#[allow(clippy::print_stdout, reason = "user facing output")]
pub async fn codemod(
  flags: Arc<Flags>,
  codemod_flags: CodemodFlags,
) -> Result<(), AnyError> {
  if codemod_flags.plugin.is_empty() {
    bail!("Missing codemod plugin. Usage: deno codemod <plugin> [files...]");
  }

  let factory = CliFactory::from_flags(flags);
  let cli_options = factory.cli_options()?;
  let initial_cwd = cli_options.initial_cwd();

  // Resolve the codemod plugin specifier relative to the current directory.
  let plugin_specifier =
    resolve_url_or_path(&codemod_flags.plugin, initial_cwd)?;

  // Collect the files the codemod should run over.
  let target_files = collect_target_files(&factory, &codemod_flags)?;
  if target_files.is_empty() {
    bail!("No target files found.");
  }

  // Spin up the plugin runtime and load the codemod as a lint plugin.
  let logger = PluginLogger::new(logger_printer);
  let runner = create_runner_and_load_plugins(
    vec![plugin_specifier],
    logger,
    // `exclude` here is a list of rule names to disable, which doesn't apply to
    // codemods (we run every rule the plugin defines).
    None,
  )
  .await?;

  // A codemod runs only the plugin, never the built-in lint rules, and always
  // applies fixes. `write_fixes` is gated on `--dry-run`.
  let linter = Arc::new(CliLinter::new(CliLinterOptions {
    configured_rules: ConfiguredRules::default(),
    fix: true,
    deno_lint_config: resolve_lint_config(
      factory.compiler_options_resolver()?,
      cli_options.start_dir.dir_url(),
    )?,
    maybe_plugin_runner: Some(Arc::new(runner)),
    write_fixes: !codemod_flags.dry_run,
  }));

  let ext = cli_options.ext_flag().clone();
  let checked = target_files.len();
  let changed_files = Arc::new(Mutex::new(Vec::new()));

  // The plugin runner drives its own current-thread runtime per file, so the
  // actual transformation has to happen off the main async runtime thread.
  // `run_parallelized` dispatches each file onto a blocking thread, exactly
  // like `deno lint` does.
  let changed_files_ = changed_files.clone();
  run_parallelized(target_files, move |path| {
    let original = deno_ast::strip_bom(fs::read_to_string(&path)?);
    let (source, _diagnostics) =
      linter.lint_file(&path, original.clone(), ext.as_deref())?;
    if source.text().as_ref() != original.as_str() {
      changed_files_.lock().push(path);
    }
    Ok(())
  })
  .await?;

  let mut changed_files = Arc::try_unwrap(changed_files)
    .map(|m| m.into_inner())
    .unwrap_or_default();
  changed_files.sort();

  report_summary(&changed_files, checked, codemod_flags.dry_run);

  Ok(())
}

fn collect_target_files(
  factory: &CliFactory,
  codemod_flags: &CodemodFlags,
) -> Result<Vec<PathBuf>, AnyError> {
  let cli_options = factory.cli_options()?;
  let initial_cwd = cli_options.initial_cwd();

  // Default to the current directory when no files are passed.
  let include = if codemod_flags.files.include.is_empty() {
    vec![".".to_string()]
  } else {
    codemod_flags.files.include.clone()
  };

  let include_patterns =
    PathOrPatternSet::from_include_relative_path_or_patterns(
      initial_cwd,
      &include,
    )?;
  let exclude_patterns =
    PathOrPatternSet::from_exclude_relative_path_or_patterns(
      initial_cwd,
      &codemod_flags.files.ignore,
    )?;
  let file_patterns = FilePatterns {
    base: initial_cwd.to_path_buf(),
    include: Some(include_patterns),
    exclude: exclude_patterns,
  };

  let ext_flag_present = cli_options.ext_flag().is_some();
  let files = FileCollector::new(|e| {
    is_script_ext(e.path) || (e.path.extension().is_none() && ext_flag_present)
  })
  .ignore_git_folder()
  .ignore_node_modules()
  .use_gitignore()
  .set_vendor_folder(cli_options.vendor_dir_path().map(ToOwned::to_owned))
  .collect_file_patterns(&CliSys::default(), &file_patterns);

  Ok(files)
}

#[allow(clippy::print_stdout, reason = "user facing output")]
fn report_summary(changed: &[PathBuf], checked: usize, dry_run: bool) {
  for path in changed {
    let verb = if dry_run { "Would modify" } else { "Modified" };
    println!("{} {}", colors::yellow(verb), path.display());
  }

  let file_word = if checked == 1 { "file" } else { "files" };
  if changed.is_empty() {
    println!(
      "{}",
      colors::gray(format!("Checked {} {}, no changes.", checked, file_word))
    );
  } else {
    let action = if dry_run {
      "would be changed"
    } else {
      "changed"
    };
    println!(
      "{}",
      colors::green(format!(
        "{} of {} {} {}.",
        changed.len(),
        checked,
        file_word,
        action
      ))
    );
  }
}
