// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

//! This module provides file linting utilities using
//! [`deno_lint`](https://github.com/denoland/deno_lint).

use deno_ast::ModuleSpecifier;
use deno_ast::ParsedSource;
use deno_config::deno_json::LintRulesConfig;
use deno_config::glob::FileCollector;
use deno_config::glob::FilePatterns;
use deno_config::workspace::WorkspaceDirectory;
use deno_core::anyhow::anyhow;
use deno_core::error::generic_error;
use deno_core::error::AnyError;
use deno_core::futures::future::LocalBoxFuture;
use deno_core::futures::FutureExt;
use deno_core::parking_lot::Mutex;
use deno_core::serde_json;
use deno_core::unsync::future::LocalFutureExt;
use deno_core::unsync::future::SharedLocal;
use deno_graph::ModuleGraph;
use deno_lint::diagnostic::LintDiagnostic;
use deno_lint::linter::LintConfig;
use log::debug;
use reporters::create_reporter;
use reporters::LintReporter;
use serde::Serialize;
use std::collections::HashSet;
use std::fs;
use std::io::stdin;
use std::io::Read;
use std::path::Path;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use crate::args::CliOptions;
use crate::args::Flags;
use crate::args::LintFlags;
use crate::args::LintOptions;
use crate::args::WorkspaceLintOptions;
use crate::cache::Caches;
use crate::cache::IncrementalCache;
use crate::colors;
use crate::factory::CliFactory;
use crate::graph_util::ModuleGraphCreator;
use crate::tools::fmt::run_parallelized;
use crate::util::display;
use crate::util::file_watcher;
use crate::util::fs::canonicalize_path;
use crate::util::path::is_script_ext;
use crate::util::sync::AtomicFlag;

mod linter;
mod reporters;
mod rules;

pub use linter::CliLinter;
pub use linter::CliLinterOptions;
pub use rules::collect_no_slow_type_diagnostics;
pub use rules::ConfiguredRules;
pub use rules::LintRuleProvider;

const JSON_SCHEMA_VERSION: u8 = 1;

static STDIN_FILE_NAME: &str = "$deno$stdin.ts";

pub async fn lint(
  flags: Arc<Flags>,
  lint_flags: LintFlags,
) -> Result<(), AnyError> {
  if let Some(watch_flags) = &lint_flags.watch {
    if lint_flags.is_stdin() {
      return Err(generic_error(
        "Lint watch on standard input is not supported.",
      ));
    }
    file_watcher::watch_func(
      flags,
      file_watcher::PrintConfig::new("Lint", !watch_flags.no_clear_screen),
      move |flags, watcher_communicator, changed_paths| {
        let lint_flags = lint_flags.clone();
        Ok(async move {
          let factory = CliFactory::from_flags(flags);
          let cli_options = factory.cli_options()?;
          let lint_config = cli_options.resolve_deno_lint_config()?;
          let mut paths_with_options_batches =
            resolve_paths_with_options_batches(cli_options, &lint_flags)?;
          for paths_with_options in &mut paths_with_options_batches {
            _ = watcher_communicator
              .watch_paths(paths_with_options.paths.clone());

            let files = std::mem::take(&mut paths_with_options.paths);
            paths_with_options.paths = if let Some(paths) = &changed_paths {
              // lint all files on any changed (https://github.com/denoland/deno/issues/12446)
              files
                .iter()
                .any(|path| {
                  canonicalize_path(path)
                    .map(|p| paths.contains(&p))
                    .unwrap_or(false)
                })
                .then_some(files)
                .unwrap_or_else(|| [].to_vec())
            } else {
              files
            };
          }

          let mut linter = WorkspaceLinter::new(
            factory.caches()?.clone(),
            factory.lint_rule_provider().await?,
            factory.module_graph_creator().await?.clone(),
            cli_options.start_dir.clone(),
            &cli_options.resolve_workspace_lint_options(&lint_flags)?,
          );
          for paths_with_options in paths_with_options_batches {
            linter
              .lint_files(
                cli_options,
                paths_with_options.options,
                lint_config.clone(),
                paths_with_options.dir,
                paths_with_options.paths,
              )
              .await?;
          }

          linter.finish();

          Ok(())
        })
      },
    )
    .await?;
  } else {
    let factory = CliFactory::from_flags(flags);
    let cli_options = factory.cli_options()?;
    let is_stdin = lint_flags.is_stdin();
    let deno_lint_config = cli_options.resolve_deno_lint_config()?;
    let workspace_lint_options =
      cli_options.resolve_workspace_lint_options(&lint_flags)?;
    let success = if is_stdin {
      let start_dir = &cli_options.start_dir;
      let reporter_lock = Arc::new(Mutex::new(create_reporter(
        workspace_lint_options.reporter_kind,
      )));
      let lint_config = start_dir
        .to_lint_config(FilePatterns::new_with_base(start_dir.dir_path()))?;
      let lint_options = LintOptions::resolve(lint_config, &lint_flags);
      let lint_rules = factory
        .lint_rule_provider()
        .await?
        .resolve_lint_rules_err_empty(
          lint_options.rules,
          start_dir.maybe_deno_json().map(|c| c.as_ref()),
        )?;
      let mut file_path = cli_options.initial_cwd().join(STDIN_FILE_NAME);
      if let Some(ext) = cli_options.ext_flag() {
        file_path.set_extension(ext);
      }
      let r = lint_stdin(&file_path, lint_rules, deno_lint_config);
      let success = handle_lint_result(
        &file_path.to_string_lossy(),
        r,
        reporter_lock.clone(),
      );
      reporter_lock.lock().close(1);
      success
    } else {
      let mut linter = WorkspaceLinter::new(
        factory.caches()?.clone(),
        factory.lint_rule_provider().await?,
        factory.module_graph_creator().await?.clone(),
        cli_options.start_dir.clone(),
        &workspace_lint_options,
      );
      let paths_with_options_batches =
        resolve_paths_with_options_batches(cli_options, &lint_flags)?;
      for paths_with_options in paths_with_options_batches {
        linter
          .lint_files(
            cli_options,
            paths_with_options.options,
            deno_lint_config.clone(),
            paths_with_options.dir,
            paths_with_options.paths,
          )
          .await?;
      }
      linter.finish()
    };
    if !success {
      std::process::exit(1);
    }
  }

  Ok(())
}

struct PathsWithOptions {
  dir: WorkspaceDirectory,
  paths: Vec<PathBuf>,
  options: LintOptions,
}

fn resolve_paths_with_options_batches(
  cli_options: &CliOptions,
  lint_flags: &LintFlags,
) -> Result<Vec<PathsWithOptions>, AnyError> {
  let members_lint_options =
    cli_options.resolve_lint_options_for_members(lint_flags)?;
  let mut paths_with_options_batches =
    Vec::with_capacity(members_lint_options.len());
  for (dir, lint_options) in members_lint_options {
    let files = collect_lint_files(cli_options, lint_options.files.clone())?;
    if !files.is_empty() {
      paths_with_options_batches.push(PathsWithOptions {
        dir,
        paths: files,
        options: lint_options,
      });
    }
  }
  if paths_with_options_batches.is_empty() {
    return Err(generic_error("No target files found."));
  }
  Ok(paths_with_options_batches)
}

type WorkspaceModuleGraphFuture =
  SharedLocal<LocalBoxFuture<'static, Result<Rc<ModuleGraph>, Rc<AnyError>>>>;

struct WorkspaceLinter {
  caches: Arc<Caches>,
  lint_rule_provider: LintRuleProvider,
  module_graph_creator: Arc<ModuleGraphCreator>,
  workspace_dir: Arc<WorkspaceDirectory>,
  reporter_lock: Arc<Mutex<Box<dyn LintReporter + Send>>>,
  workspace_module_graph: Option<WorkspaceModuleGraphFuture>,
  has_error: Arc<AtomicFlag>,
  file_count: usize,
}

impl WorkspaceLinter {
  pub fn new(
    caches: Arc<Caches>,
    lint_rule_provider: LintRuleProvider,
    module_graph_creator: Arc<ModuleGraphCreator>,
    workspace_dir: Arc<WorkspaceDirectory>,
    workspace_options: &WorkspaceLintOptions,
  ) -> Self {
    let reporter_lock =
      Arc::new(Mutex::new(create_reporter(workspace_options.reporter_kind)));
    Self {
      caches,
      lint_rule_provider,
      module_graph_creator,
      workspace_dir,
      reporter_lock,
      workspace_module_graph: None,
      has_error: Default::default(),
      file_count: 0,
    }
  }

  pub async fn lint_files(
    &mut self,
    cli_options: &Arc<CliOptions>,
    lint_options: LintOptions,
    lint_config: LintConfig,
    member_dir: WorkspaceDirectory,
    paths: Vec<PathBuf>,
  ) -> Result<(), AnyError> {
    self.file_count += paths.len();

    let lint_rules = self.lint_rule_provider.resolve_lint_rules_err_empty(
      lint_options.rules,
      member_dir.maybe_deno_json().map(|c| c.as_ref()),
    )?;
    let maybe_incremental_cache =
      lint_rules.incremental_cache_state().map(|state| {
        Arc::new(IncrementalCache::new(
          self.caches.lint_incremental_cache_db(),
          &state,
          &paths,
        ))
      });

    let linter = Arc::new(CliLinter::new(CliLinterOptions {
      configured_rules: lint_rules,
      fix: lint_options.fix,
      deno_lint_config: lint_config,
    }));

    let mut futures = Vec::with_capacity(2);
    if linter.has_package_rules() {
      if self.workspace_module_graph.is_none() {
        let module_graph_creator = self.module_graph_creator.clone();
        let packages = self.workspace_dir.jsr_packages_for_publish();
        self.workspace_module_graph = Some(
          async move {
            module_graph_creator
              .create_and_validate_publish_graph(&packages, true)
              .await
              .map(Rc::new)
              .map_err(Rc::new)
          }
          .boxed_local()
          .shared_local(),
        );
      }
      let workspace_module_graph_future =
        self.workspace_module_graph.as_ref().unwrap().clone();
      let publish_config = member_dir.maybe_package_config();
      if let Some(publish_config) = publish_config {
        let has_error = self.has_error.clone();
        let reporter_lock = self.reporter_lock.clone();
        let linter = linter.clone();
        let path_urls = paths
          .iter()
          .filter_map(|p| ModuleSpecifier::from_file_path(p).ok())
          .collect::<HashSet<_>>();
        futures.push(
          async move {
            let graph = workspace_module_graph_future
              .await
              .map_err(|err| anyhow!("{:#}", err))?;
            let export_urls =
              publish_config.config_file.resolve_export_value_urls()?;
            if !export_urls.iter().any(|url| path_urls.contains(url)) {
              return Ok(()); // entrypoint is not specified, so skip
            }
            let diagnostics = linter.lint_package(&graph, &export_urls);
            if !diagnostics.is_empty() {
              has_error.raise();
              let mut reporter = reporter_lock.lock();
              for diagnostic in &diagnostics {
                reporter.visit_diagnostic(diagnostic);
              }
            }
            Ok(())
          }
          .boxed_local(),
        );
      }
    }

    futures.push({
      let has_error = self.has_error.clone();
      let reporter_lock = self.reporter_lock.clone();
      let maybe_incremental_cache = maybe_incremental_cache.clone();
      let linter = linter.clone();
      let cli_options = cli_options.clone();
      async move {
        run_parallelized(paths, {
          move |file_path| {
            let file_text =
              deno_ast::strip_bom(fs::read_to_string(&file_path)?);

            // don't bother rechecking this file if it didn't have any diagnostics before
            if let Some(incremental_cache) = &maybe_incremental_cache {
              if incremental_cache.is_file_same(&file_path, &file_text) {
                return Ok(());
              }
            }

            let r = linter.lint_file(
              &file_path,
              file_text,
              cli_options.ext_flag().as_deref(),
            );
            if let Ok((file_source, file_diagnostics)) = &r {
              if let Some(incremental_cache) = &maybe_incremental_cache {
                if file_diagnostics.is_empty() {
                  // update the incremental cache if there were no diagnostics
                  incremental_cache.update_file(
                    &file_path,
                    // ensure the returned text is used here as it may have been modified via --fix
                    file_source.text(),
                  )
                }
              }
            }

            let success = handle_lint_result(
              &file_path.to_string_lossy(),
              r,
              reporter_lock.clone(),
            );
            if !success {
              has_error.raise();
            }

            Ok(())
          }
        })
        .await
      }
      .boxed_local()
    });

    if lint_options.fix {
      // run sequentially when using `--fix` to lower the chances of weird
      // bugs where a file level fix affects a package level diagnostic though
      // it probably will happen anyway
      for future in futures {
        future.await?;
      }
    } else {
      deno_core::futures::future::try_join_all(futures).await?;
    }

    if let Some(incremental_cache) = &maybe_incremental_cache {
      incremental_cache.wait_completion().await;
    }

    Ok(())
  }

  pub fn finish(self) -> bool {
    debug!("Found {} files", self.file_count);
    self.reporter_lock.lock().close(self.file_count);
    !self.has_error.is_raised() // success
  }
}

fn collect_lint_files(
  cli_options: &CliOptions,
  files: FilePatterns,
) -> Result<Vec<PathBuf>, AnyError> {
  FileCollector::new(|e| {
    is_script_ext(e.path)
      || (e.path.extension().is_none() && cli_options.ext_flag().is_some())
  })
  .ignore_git_folder()
  .ignore_node_modules()
  .set_vendor_folder(cli_options.vendor_dir_path().map(ToOwned::to_owned))
  .collect_file_patterns(&deno_config::fs::RealDenoConfigFs, files)
}

#[allow(clippy::print_stdout)]
pub fn print_rules_list(json: bool, maybe_rules_tags: Option<Vec<String>>) {
  let rule_provider = LintRuleProvider::new(None, None);
  let lint_rules = rule_provider
    .resolve_lint_rules(
      LintRulesConfig {
        tags: maybe_rules_tags.clone(),
        include: None,
        exclude: None,
      },
      None,
    )
    .rules;

  if json {
    let json_output = serde_json::json!({
      "version": JSON_SCHEMA_VERSION,
      "rules": lint_rules
        .iter()
        .map(|rule| {
          serde_json::json!({
            "code": rule.code(),
            "tags": rule.tags(),
            "docs": rule.docs(),
          })
        })
        .collect::<Vec<serde_json::Value>>(),
    });
    display::write_json_to_stdout(&json_output).unwrap();
  } else {
    // The rules should still be printed even if `--quiet` option is enabled,
    // so use `println!` here instead of `info!`.
    println!("Available rules:");
    for rule in lint_rules.iter() {
      print!(" - {}", colors::cyan(rule.code()));
      if rule.tags().is_empty() {
        println!();
      } else {
        println!(" [{}]", colors::gray(rule.tags().join(", ")))
      }
      println!(
        "{}",
        colors::gray(format!("   help: {}", rule.help_docs_url()))
      );
      println!();
    }
  }
}

/// Lint stdin and write result to stdout.
/// Treats input as TypeScript.
/// Compatible with `--json` flag.
fn lint_stdin(
  file_path: &Path,
  configured_rules: ConfiguredRules,
  deno_lint_config: LintConfig,
) -> Result<(ParsedSource, Vec<LintDiagnostic>), AnyError> {
  let mut source_code = String::new();
  if stdin().read_to_string(&mut source_code).is_err() {
    return Err(generic_error("Failed to read from stdin"));
  }

  let linter = CliLinter::new(CliLinterOptions {
    fix: false,
    configured_rules,
    deno_lint_config,
  });

  linter
    .lint_file(file_path, deno_ast::strip_bom(source_code), None)
    .map_err(AnyError::from)
}

fn handle_lint_result(
  file_path: &str,
  result: Result<(ParsedSource, Vec<LintDiagnostic>), AnyError>,
  reporter_lock: Arc<Mutex<Box<dyn LintReporter + Send>>>,
) -> bool {
  let mut reporter = reporter_lock.lock();

  match result {
    Ok((source, mut file_diagnostics)) => {
      if !source.diagnostics().is_empty() {
        for parse_diagnostic in source.diagnostics() {
          log::warn!("{}: {}", colors::yellow("warn"), parse_diagnostic);
        }
      }
      file_diagnostics.sort_by(|a, b| match a.specifier.cmp(&b.specifier) {
        std::cmp::Ordering::Equal => {
          let a_start = a.range.as_ref().map(|r| r.range.start);
          let b_start = b.range.as_ref().map(|r| r.range.start);
          match a_start.cmp(&b_start) {
            std::cmp::Ordering::Equal => a.details.code.cmp(&b.details.code),
            other => other,
          }
        }
        file_order => file_order,
      });
      for d in &file_diagnostics {
        reporter.visit_diagnostic(d);
      }
      file_diagnostics.is_empty()
    }
    Err(err) => {
      reporter.visit_error(file_path, &err);
      false
    }
  }
}

#[derive(Serialize)]
struct LintError {
  file_path: String,
  message: String,
}
