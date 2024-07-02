// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::sync::Arc;

use deno_ast::ModuleSpecifier;
use deno_core::error::AnyError;
use deno_core::parking_lot::RwLock;
use deno_core::resolve_url_or_path;
use deno_graph::ModuleGraph;
use deno_runtime::colors;
use deno_runtime::deno_permissions::PermissionsContainer;

use crate::args::CliOptions;
use crate::module_loader::ModuleLoadPreparer;

pub trait ModuleGraphContainer: Clone + 'static {
  /// Acquires a permit to modify the module graph without other code
  /// having the chance to modify it. In the meantime, other code may
  /// still read from the existing module graph.
  async fn acquire_update_permit(&self) -> impl ModuleGraphUpdatePermit;
  /// Gets a copy of the graph.
  fn graph(&self) -> Arc<ModuleGraph>;
}

/// A permit for updating the module graph. When complete and
/// everything looks fine, calling `.commit()` will store the
/// new graph in the ModuleGraphContainer.
pub trait ModuleGraphUpdatePermit {
  /// Gets the module graph for mutation.
  fn graph_mut(&mut self) -> &mut ModuleGraph;
  /// Saves the mutated module graph in the container.
  fn commit(self);
}

/// Holds the `ModuleGraph` for the main worker.
#[derive(Clone)]
pub struct MainModuleGraphContainer {
  // Allow only one request to update the graph data at a time,
  // but allow other requests to read from it at any time even
  // while another request is updating the data.
  update_queue: Arc<crate::util::sync::TaskQueue>,
  inner: Arc<RwLock<Arc<ModuleGraph>>>,
  cli_options: Arc<CliOptions>,
  module_load_preparer: Arc<ModuleLoadPreparer>,
}

impl MainModuleGraphContainer {
  pub fn new(
    cli_options: Arc<CliOptions>,
    module_load_preparer: Arc<ModuleLoadPreparer>,
  ) -> Self {
    Self {
      update_queue: Default::default(),
      inner: Arc::new(RwLock::new(Arc::new(ModuleGraph::new(
        cli_options.graph_kind(),
      )))),
      cli_options,
      module_load_preparer,
    }
  }

  pub async fn check_specifiers(
    &self,
    specifiers: &[ModuleSpecifier],
  ) -> Result<(), AnyError> {
    let mut graph_permit = self.acquire_update_permit().await;
    let graph = graph_permit.graph_mut();
    self
      .module_load_preparer
      .prepare_module_load(
        graph,
        specifiers,
        false,
        self.cli_options.ts_type_lib_window(),
        PermissionsContainer::allow_all(),
      )
      .await?;
    graph_permit.commit();
    Ok(())
  }

  /// Helper around prepare_module_load that loads and type checks
  /// the provided files.
  pub async fn load_and_type_check_files(
    &self,
    files: &[String],
  ) -> Result<(), AnyError> {
    let specifiers = self.collect_specifiers(files)?;

    if specifiers.is_empty() {
      log::warn!("{} No matching files found.", colors::yellow("Warning"));
    }

    self.check_specifiers(&specifiers).await
  }

  pub fn collect_specifiers(
    &self,
    files: &[String],
  ) -> Result<Vec<ModuleSpecifier>, AnyError> {
    let excludes = self.cli_options.workspace.resolve_config_excludes()?;
    Ok(
      files
        .iter()
        .filter_map(|file| {
          let file_url =
            resolve_url_or_path(file, self.cli_options.initial_cwd()).ok()?;
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

impl ModuleGraphContainer for MainModuleGraphContainer {
  async fn acquire_update_permit(&self) -> impl ModuleGraphUpdatePermit {
    let permit = self.update_queue.acquire().await;
    MainModuleGraphUpdatePermit {
      permit,
      inner: self.inner.clone(),
      graph: (**self.inner.read()).clone(),
    }
  }

  fn graph(&self) -> Arc<ModuleGraph> {
    self.inner.read().clone()
  }
}

/// A permit for updating the module graph. When complete and
/// everything looks fine, calling `.commit()` will store the
/// new graph in the ModuleGraphContainer.
pub struct MainModuleGraphUpdatePermit<'a> {
  permit: crate::util::sync::TaskQueuePermit<'a>,
  inner: Arc<RwLock<Arc<ModuleGraph>>>,
  graph: ModuleGraph,
}

impl<'a> ModuleGraphUpdatePermit for MainModuleGraphUpdatePermit<'a> {
  fn graph_mut(&mut self) -> &mut ModuleGraph {
    &mut self.graph
  }

  fn commit(self) {
    *self.inner.write() = Arc::new(self.graph);
    drop(self.permit); // explicit drop for clarity
  }
}
