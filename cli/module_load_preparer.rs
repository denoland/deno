// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use crate::args::CliOptions;
use crate::args::TsTypeLib;
use crate::graph_util::graph_lock_or_exit;
use crate::graph_util::CreateGraphOptions;
use crate::graph_util::ModuleGraphBuilder;
use crate::tools::check;
use crate::tools::check::TypeChecker;
use crate::util::progress_bar::ProgressBar;

use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_core::parking_lot::Mutex;
use deno_core::resolve_url_or_path;
use deno_core::ModuleSpecifier;
use deno_graph::ModuleGraph;
use deno_lockfile::Lockfile;
use deno_runtime::permissions::PermissionsContainer;
use deno_terminal::colors;
use std::sync::Arc;

pub struct ModuleLoadPreparer {
  options: Arc<CliOptions>,
  lockfile: Option<Arc<Mutex<Lockfile>>>,
  module_graph_builder: Arc<ModuleGraphBuilder>,
  progress_bar: ProgressBar,
  type_checker: Arc<TypeChecker>,
}

impl ModuleLoadPreparer {
  #[allow(clippy::too_many_arguments)]
  pub fn new(
    options: Arc<CliOptions>,
    lockfile: Option<Arc<Mutex<Lockfile>>>,
    module_graph_builder: Arc<ModuleGraphBuilder>,
    progress_bar: ProgressBar,
    type_checker: Arc<TypeChecker>,
  ) -> Self {
    Self {
      options,
      lockfile,
      module_graph_builder,
      progress_bar,
      type_checker,
    }
  }

  /// This method must be called for a module or a static importer of that
  /// module before attempting to `load()` it from a `JsRuntime`. It will
  /// populate the graph data in memory with the necessary source code, write
  /// emits where necessary or report any module graph / type checking errors.
  #[allow(clippy::too_many_arguments)]
  pub async fn prepare_module_load(
    &self,
    graph: &mut ModuleGraph,
    roots: &[ModuleSpecifier],
    is_dynamic: bool,
    lib: TsTypeLib,
    permissions: PermissionsContainer,
  ) -> Result<(), AnyError> {
    log::debug!("Preparing module load.");
    let _pb_clear_guard = self.progress_bar.clear_guard();

    let mut cache = self.module_graph_builder.create_fetch_cacher(permissions);
    log::debug!("Building module graph.");
    let has_type_checked = !graph.roots.is_empty();

    self
      .module_graph_builder
      .build_graph_with_npm_resolution(
        graph,
        CreateGraphOptions {
          is_dynamic,
          graph_kind: graph.graph_kind(),
          roots: roots.to_vec(),
          loader: Some(&mut cache),
        },
      )
      .await?;

    self.module_graph_builder.graph_roots_valid(graph, &roots)?;

    // If there is a lockfile...
    if let Some(lockfile) = &self.lockfile {
      let mut lockfile = lockfile.lock();
      // validate the integrity of all the modules
      graph_lock_or_exit(graph, &mut lockfile);
      // update it with anything new
      lockfile.write().context("Failed writing lockfile.")?;
    }

    drop(_pb_clear_guard);

    // type check if necessary
    if self.options.type_check_mode().is_true() && !has_type_checked {
      self
        .type_checker
        .check(
          // todo(perf): since this is only done the first time the graph is
          // created, we could avoid the clone of the graph here by providing
          // the actual graph on the first run and then getting the Arc<ModuleGraph>
          // back from the return value.
          graph.clone(),
          check::CheckOptions {
            build_fast_check_graph: true,
            lib,
            log_ignored_options: false,
            reload: self.options.reload_flag(),
            type_check_mode: self.options.type_check_mode(),
          },
        )
        .await?;
    }

    log::debug!("Prepared module load.");

    Ok(())
  }
}

pub struct MainModuleGraphPreparer {
  options: Arc<CliOptions>,
  module_load_preparer: Arc<ModuleLoadPreparer>,
  graph: ModuleGraph,
}

impl MainModuleGraphPreparer {
  pub fn new(
    options: Arc<CliOptions>,
    module_load_preparer: Arc<ModuleLoadPreparer>,
  ) -> Self {
    Self {
      graph: ModuleGraph::new(options.graph_kind()),
      options,
      module_load_preparer,
    }
  }

  pub fn into_graph(self) -> ModuleGraph {
    self.graph
  }

  pub fn graph(&self) -> &ModuleGraph {
    &self.graph
  }

  pub async fn check_specifiers(
    &mut self,
    specifiers: &[ModuleSpecifier],
  ) -> Result<(), AnyError> {
    let lib = self.options.ts_type_lib_window();
    self
      .module_load_preparer
      .prepare_module_load(
        &mut self.graph,
        specifiers,
        false,
        lib,
        PermissionsContainer::allow_all(),
      )
      .await?;
    Ok(())
  }

  /// Helper around prepare_module_load that loads and type checks
  /// the provided files.
  pub async fn load_and_type_check_files(
    &mut self,
    files: &[String],
  ) -> Result<(), AnyError> {
    let specifiers = self.collect_specifiers(files)?;

    if specifiers.is_empty() {
      log::warn!("{} No matching files found.", colors::yellow("Warning"));
    }

    self.check_specifiers(&specifiers).await
  }

  fn collect_specifiers(
    &self,
    files: &[String],
  ) -> Result<Vec<ModuleSpecifier>, AnyError> {
    let excludes = self.options.resolve_config_excludes()?;
    Ok(
      files
        .iter()
        .filter_map(|file| {
          let file_url =
            resolve_url_or_path(file, self.options.initial_cwd()).ok()?;
          if file_url.scheme() != "file" {
            return Some(file_url);
          }
          // ignore local files that match any of files listed in `exclude` option
          let file_path = file_url.to_file_path().ok()?;
          if excludes.matches_path(&file_path) {
            None
          } else {
            Some(file_url)
          }
        })
        .collect::<Vec<_>>(),
    )
  }
}
