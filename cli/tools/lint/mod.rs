// Copyright 2018-2025 the Deno authors. MIT license.

//! This module provides file linting utilities using
//! [`deno_lint`](https://github.com/denoland/deno_lint).

use std::collections::HashSet;
use std::fs;
use std::io::stdin;
use std::io::Read;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use deno_ast::ModuleSpecifier;
use deno_ast::ParsedSource;
use deno_config::deno_json::LintRulesConfig;
use deno_config::glob::FileCollector;
use deno_config::glob::FilePatterns;
use deno_config::workspace::WorkspaceDirectory;
use deno_core::anyhow::anyhow;
use deno_core::anyhow::bail;
use deno_core::error::AnyError;
use deno_core::futures::future::LocalBoxFuture;
use deno_core::futures::FutureExt;
use deno_core::parking_lot::Mutex;
use deno_core::serde_json;
use deno_core::unsync::future::LocalFutureExt;
use deno_core::unsync::future::SharedLocal;
use deno_graph::ModuleGraph;
use deno_lib::util::hash::FastInsecureHasher;
use deno_lint::diagnostic::LintDiagnostic;
use log::debug;
use reporters::create_reporter;
use reporters::LintReporter;
use serde::Serialize;

use crate::args::deno_json::TsConfigResolver;
use crate::args::CliOptions;
use crate::args::Flags;
use crate::args::LintFlags;
use crate::args::LintOptions;
use crate::args::WorkspaceLintOptions;
use crate::cache::CacheDBHash;
use crate::cache::Caches;
use crate::cache::IncrementalCache;
use crate::colors;
use crate::factory::CliFactory;
use crate::graph_util::ModuleGraphCreator;
use crate::sys::CliSys;
use crate::tools::fmt::run_parallelized;
use crate::util::display;
use crate::util::file_watcher;
use crate::util::file_watcher::WatcherCommunicator;
use crate::util::fs::canonicalize_path;
use crate::util::path::is_script_ext;
use crate::util::sync::AtomicFlag;

mod ast_buffer;
mod linter;
mod plugins;
mod reporters;
mod rules;

// TODO(bartlomieju): remove once we wire plugins through the CLI linter
pub use ast_buffer::serialize_ast_to_buffer;
pub use linter::CliLinter;
pub use linter::CliLinterOptions;
pub use plugins::create_runner_and_load_plugins;
pub use plugins::PluginLogger;
pub use rules::collect_no_slow_type_diagnostics;
pub use rules::ConfiguredRules;
pub use rules::LintRuleProvider;

const JSON_SCHEMA_VERSION: u8 = 1;

static STDIN_FILE_NAME: &str = "$deno$stdin.mts";

pub async fn lint(
  flags: Arc<Flags>,
  lint_flags: LintFlags,
) -> Result<(), AnyError> {
  if lint_flags.watch.is_some() {
    if lint_flags.is_stdin() {
      return Err(anyhow!("Lint watch on standard input is not supported.",));
    }

    return lint_with_watch(flags, lint_flags).await;
  }

  let factory = CliFactory::from_flags(flags);
  let cli_options = factory.cli_options()?;
  let lint_rule_provider = factory.lint_rule_provider().await?;
  let is_stdin = lint_flags.is_stdin();
  let tsconfig_resolver = factory.tsconfig_resolver()?;
  let workspace_lint_options =
    cli_options.resolve_workspace_lint_options(&lint_flags)?;
  let success = if is_stdin {
    lint_stdin(
      cli_options,
      lint_rule_provider,
      workspace_lint_options,
      lint_flags,
      tsconfig_resolver,
    )?
  } else {
    let mut linter = WorkspaceLinter::new(
      factory.caches()?.clone(),
      lint_rule_provider,
      factory.module_graph_creator().await?.clone(),
      tsconfig_resolver.clone(),
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
          paths_with_options.dir,
          paths_with_options.paths,
        )
        .await?;
    }
    linter.finish()
  };
  if !success {
    deno_runtime::exit(1);
  }

  Ok(())
}

async fn lint_with_watch_inner(
  flags: Arc<Flags>,
  lint_flags: LintFlags,
  watcher_communicator: Arc<WatcherCommunicator>,
  changed_paths: Option<Vec<PathBuf>>,
) -> Result<(), AnyError> {
  let factory = CliFactory::from_flags(flags);
  let cli_options = factory.cli_options()?;
  let mut paths_with_options_batches =
    resolve_paths_with_options_batches(cli_options, &lint_flags)?;
  for paths_with_options in &mut paths_with_options_batches {
    _ = watcher_communicator.watch_paths(paths_with_options.paths.clone());

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
    factory.tsconfig_resolver()?.clone(),
    cli_options.start_dir.clone(),
    &cli_options.resolve_workspace_lint_options(&lint_flags)?,
  );
  for paths_with_options in paths_with_options_batches {
    linter
      .lint_files(
        cli_options,
        paths_with_options.options,
        paths_with_options.dir,
        paths_with_options.paths,
      )
      .await?;
  }

  linter.finish();

  Ok(())
}

async fn lint_with_watch(
  flags: Arc<Flags>,
  lint_flags: LintFlags,
) -> Result<(), AnyError> {
  let watch_flags = lint_flags.watch.as_ref().unwrap();

  file_watcher::watch_func(
    flags,
    file_watcher::PrintConfig::new("Lint", !watch_flags.no_clear_screen),
    move |flags, watcher_communicator, changed_paths| {
      let lint_flags = lint_flags.clone();
      watcher_communicator.show_path_changed(changed_paths.clone());
      Ok(lint_with_watch_inner(
        flags,
        lint_flags,
        watcher_communicator,
        changed_paths,
      ))
    },
  )
  .await
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
    let files = collect_lint_files(cli_options, lint_options.files.clone());
    if !files.is_empty() {
      paths_with_options_batches.push(PathsWithOptions {
        dir,
        paths: files,
        options: lint_options,
      });
    }
  }
  if paths_with_options_batches.is_empty() && !lint_flags.permit_no_files {
    return Err(anyhow!("No target files found."));
  }
  Ok(paths_with_options_batches)
}

type WorkspaceModuleGraphFuture =
  SharedLocal<LocalBoxFuture<'static, Result<Rc<ModuleGraph>, Rc<AnyError>>>>;

struct WorkspaceLinter {
  caches: Arc<Caches>,
  lint_rule_provider: LintRuleProvider,
  module_graph_creator: Arc<ModuleGraphCreator>,
  tsconfig_resolver: Arc<TsConfigResolver>,
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
    tsconfig_resolver: Arc<TsConfigResolver>,
    workspace_dir: Arc<WorkspaceDirectory>,
    workspace_options: &WorkspaceLintOptions,
  ) -> Self {
    let reporter_lock =
      Arc::new(Mutex::new(create_reporter(workspace_options.reporter_kind)));
    Self {
      caches,
      lint_rule_provider,
      module_graph_creator,
      tsconfig_resolver,
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
    member_dir: WorkspaceDirectory,
    paths: Vec<PathBuf>,
  ) -> Result<(), AnyError> {
    self.file_count += paths.len();

    let exclude = lint_options.rules.exclude.clone();

    let plugin_specifiers = lint_options.plugins.clone();
    let lint_rules = self.lint_rule_provider.resolve_lint_rules(
      lint_options.rules,
      member_dir.maybe_deno_json().map(|c| c.as_ref()),
    );

    let mut maybe_incremental_cache = None;

    // TODO(bartlomieju): for now we don't support incremental caching if plugins are being used.
    // https://github.com/denoland/deno/issues/28025
    if lint_rules.supports_incremental_cache() && plugin_specifiers.is_empty() {
      let mut hasher = FastInsecureHasher::new_deno_versioned();
      hasher.write_hashable(lint_rules.incremental_cache_state());
      if !plugin_specifiers.is_empty() {
        hasher.write_hashable(&plugin_specifiers);
      }
      let state_hash = hasher.finish();

      maybe_incremental_cache = Some(Arc::new(IncrementalCache::new(
        self.caches.lint_incremental_cache_db(),
        CacheDBHash::new(state_hash),
        &paths,
      )));
    }

    #[allow(clippy::print_stdout)]
    #[allow(clippy::print_stderr)]
    fn logger_printer(msg: &str, is_err: bool) {
      if is_err {
        eprint!("{}", msg);
      } else {
        print!("{}", msg);
      }
    }

    let mut plugin_runner = None;
    if !plugin_specifiers.is_empty() {
      let logger = plugins::PluginLogger::new(logger_printer);
      let runner = plugins::create_runner_and_load_plugins(
        plugin_specifiers,
        logger,
        exclude,
      )
      .await?;
      plugin_runner = Some(Arc::new(runner));
    } else if lint_rules.rules.is_empty() {
      bail!("No rules have been configured")
    }

    let linter = Arc::new(CliLinter::new(CliLinterOptions {
      configured_rules: lint_rules,
      fix: lint_options.fix,
      deno_lint_config: self
        .tsconfig_resolver
        .deno_lint_config(member_dir.dir_url())?,
      maybe_plugin_runner: plugin_runner,
    }));

    let has_error = self.has_error.clone();
    let reporter_lock = self.reporter_lock.clone();

    let mut futures = Vec::with_capacity(2);
    if linter.has_package_rules() {
      if let Some(fut) = self.run_package_rules(&linter, &member_dir, &paths) {
        futures.push(fut);
      }
    }

    let maybe_incremental_cache_ = maybe_incremental_cache.clone();
    let linter = linter.clone();
    let cli_options = cli_options.clone();
    let fut = async move {
      let operation = move |file_path: PathBuf| {
        let file_text = deno_ast::strip_bom(fs::read_to_string(&file_path)?);

        // don't bother rechecking this file if it didn't have any diagnostics before
        if let Some(incremental_cache) = &maybe_incremental_cache_ {
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
          if let Some(incremental_cache) = &maybe_incremental_cache_ {
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
      };
      run_parallelized(paths, operation).await
    }
    .boxed_local();
    futures.push(fut);

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

  fn run_package_rules(
    &mut self,
    linter: &Arc<CliLinter>,
    member_dir: &WorkspaceDirectory,
    paths: &[PathBuf],
  ) -> Option<LocalBoxFuture<Result<(), AnyError>>> {
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
    let maybe_publish_config = member_dir.maybe_package_config();
    let publish_config = maybe_publish_config?;

    let has_error = self.has_error.clone();
    let reporter_lock = self.reporter_lock.clone();
    let linter = linter.clone();
    let path_urls = paths
      .iter()
      .filter_map(|p| ModuleSpecifier::from_file_path(p).ok())
      .collect::<HashSet<_>>();
    let fut = async move {
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
    .boxed_local();
    Some(fut)
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
) -> Vec<PathBuf> {
  FileCollector::new(|e| {
    is_script_ext(e.path)
      || (e.path.extension().is_none() && cli_options.ext_flag().is_some())
  })
  .ignore_git_folder()
  .ignore_node_modules()
  .use_gitignore()
  .set_vendor_folder(cli_options.vendor_dir_path().map(ToOwned::to_owned))
  .collect_file_patterns(&CliSys::default(), files)
}

#[allow(clippy::print_stdout)]
pub fn print_rules_list(json: bool, maybe_rules_tags: Option<Vec<String>>) {
  let rule_provider = LintRuleProvider::new(None);
  let mut all_rules = rule_provider.all_rules();
  let configured_rules = rule_provider.resolve_lint_rules(
    LintRulesConfig {
      tags: maybe_rules_tags.clone(),
      include: None,
      exclude: None,
    },
    None,
  );
  all_rules.sort_by_cached_key(|rule| rule.code().to_string());

  if json {
    let json_output = serde_json::json!({
      "version": JSON_SCHEMA_VERSION,
      "rules": all_rules
        .iter()
        .map(|rule| {
          // TODO(bartlomieju): print if rule enabled
          serde_json::json!({
            "code": rule.code(),
            "tags": rule.tags().iter().map(|t| t.display()).collect::<Vec<_>>(),
            "docs": rule.help_docs_url(),
          })
        })
        .collect::<Vec<serde_json::Value>>(),
    });
    display::write_json_to_stdout(&json_output).unwrap();
  } else {
    // The rules should still be printed even if `--quiet` option is enabled,
    // so use `println!` here instead of `info!`.
    println!("Available rules:");
    for rule in all_rules.iter() {
      // TODO(bartlomieju): this is O(n) search, fix before landing
      let enabled = if configured_rules.rules.contains(rule) {
        "✓"
      } else {
        ""
      };
      println!("- {} {}", rule.code(), colors::green(enabled),);
      println!(
        "{}",
        colors::gray(format!("  help: {}", rule.help_docs_url()))
      );
      if rule.tags().is_empty() {
        println!("  {}", colors::gray("tags:"));
      } else {
        println!(
          "  {}",
          colors::gray(format!(
            "tags: {}",
            rule
              .tags()
              .iter()
              .map(|t| t.display())
              .collect::<Vec<_>>()
              .join(", ")
          ))
        );
      }
      println!();
    }
  }
}

/// Lint stdin and write result to stdout.
/// Treats input as TypeScript.
/// Compatible with `--json` flag.
fn lint_stdin(
  cli_options: &Arc<CliOptions>,
  lint_rule_provider: LintRuleProvider,
  workspace_lint_options: WorkspaceLintOptions,
  lint_flags: LintFlags,
  tsconfig_resolver: &TsConfigResolver,
) -> Result<bool, AnyError> {
  let start_dir = &cli_options.start_dir;
  let reporter_lock = Arc::new(Mutex::new(create_reporter(
    workspace_lint_options.reporter_kind,
  )));
  let lint_config = start_dir
    .to_lint_config(FilePatterns::new_with_base(start_dir.dir_path()))?;
  let deno_lint_config =
    tsconfig_resolver.deno_lint_config(start_dir.dir_url())?;
  let lint_options = LintOptions::resolve(lint_config, &lint_flags)?;
  let configured_rules = lint_rule_provider.resolve_lint_rules_err_empty(
    lint_options.rules,
    start_dir.maybe_deno_json().map(|c| c.as_ref()),
  )?;
  let mut file_path = cli_options.initial_cwd().join(STDIN_FILE_NAME);
  if let Some(ext) = cli_options.ext_flag() {
    file_path.set_extension(ext);
  }
  let mut source_code = String::new();
  if stdin().read_to_string(&mut source_code).is_err() {
    return Err(anyhow!("Failed to read from stdin"));
  }

  let linter = CliLinter::new(CliLinterOptions {
    fix: false,
    configured_rules,
    deno_lint_config,
    maybe_plugin_runner: None,
  });

  let r = linter.lint_file(&file_path, deno_ast::strip_bom(source_code), None);

  let success =
    handle_lint_result(&file_path.to_string_lossy(), r, reporter_lock.clone());
  reporter_lock.lock().close(1);
  Ok(success)
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

#[cfg(test)]
mod tests {
  use pretty_assertions::assert_eq;
  use serde::Deserialize;
  use test_util as util;

  use super::*;

  #[derive(Serialize, Deserialize)]
  struct RulesPattern {
    r#type: String,
    pattern: String,
  }

  #[derive(Serialize, Deserialize)]
  struct RulesEnum {
    r#enum: Vec<String>,
  }

  #[derive(Serialize, Deserialize)]
  struct RulesSchema {
    #[serde(rename = "$schema")]
    schema: String,

    #[serde(rename = "oneOf")]
    one_of: (RulesPattern, RulesEnum),
  }

  fn get_all_rules() -> Vec<String> {
    let rule_provider = LintRuleProvider::new(None);
    let configured_rules =
      rule_provider.resolve_lint_rules(Default::default(), None);
    let mut all_rules = configured_rules
      .all_rule_codes
      .into_iter()
      .map(|s| s.to_string())
      .collect::<Vec<String>>();
    all_rules.sort();

    all_rules
  }

  // TODO(bartlomieju): do the same for tags, once https://github.com/denoland/deno/pull/27162 lands
  #[test]
  fn all_lint_rules_are_listed_in_schema_file() {
    let all_rules = get_all_rules();

    let rules_schema_path =
      util::root_path().join("cli/schemas/lint-rules.v1.json");
    let rules_schema_file =
      std::fs::read_to_string(&rules_schema_path).unwrap();

    let schema: RulesSchema = serde_json::from_str(&rules_schema_file).unwrap();

    const UPDATE_ENV_VAR_NAME: &str = "UPDATE_EXPECTED";

    let rules_list = schema.one_of.1.r#enum;

    if std::env::var(UPDATE_ENV_VAR_NAME).ok().is_none() {
      assert_eq!(
        rules_list, all_rules,
        "Lint rules schema file not up to date. Run again with {}=1 to update the expected output",
        UPDATE_ENV_VAR_NAME
      );
      return;
    }

    let new_schema = RulesSchema {
      schema: schema.schema,
      one_of: (schema.one_of.0, RulesEnum { r#enum: all_rules }),
    };

    std::fs::write(
      &rules_schema_path,
      format!("{}\n", serde_json::to_string_pretty(&new_schema).unwrap(),),
    )
    .unwrap();
  }
}
