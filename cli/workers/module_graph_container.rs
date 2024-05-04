// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use deno_core::parking_lot::RwLock;
use deno_graph::GraphKind;
use deno_graph::ModuleGraph;

pub trait ModuleGraphContainer: Clone + 'static {
  /// Acquires a permit to modify the module graph without other code
  /// having the chance to modify it. In the meantime, other code may
  /// still read from the existing module graph.
  async fn acquire_update_permit(&self) -> impl ModuleGraphUpdatePermit;
  /// Gets a copy of the graph.
  fn graph(&self) -> Arc<ModuleGraph>;
}

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
}

impl MainModuleGraphContainer {
  pub fn new(graph_kind: GraphKind) -> Self {
    Self {
      update_queue: Default::default(),
      inner: Arc::new(RwLock::new(Arc::new(ModuleGraph::new(graph_kind)))),
    }
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

/// Holds the `ModuleGraph` in workers.
#[derive(Clone)]
pub struct WorkerModuleGraphContainer {
  // Allow only one request to update the graph data at a time,
  // but allow other requests to read from it at any time even
  // while another request is updating the data.
  update_queue: Rc<deno_core::unsync::TaskQueue>,
  inner: Rc<RefCell<Arc<ModuleGraph>>>,
}

impl WorkerModuleGraphContainer {
  pub fn new(module_graph: Arc<ModuleGraph>) -> Self {
    Self {
      update_queue: Default::default(),
      inner: Rc::new(RefCell::new(module_graph)),
    }
  }
}

impl ModuleGraphContainer for WorkerModuleGraphContainer {
  async fn acquire_update_permit(&self) -> impl ModuleGraphUpdatePermit {
    let permit = self.update_queue.acquire().await;
    WorkerModuleGraphUpdatePermit {
      permit,
      inner: self.inner.clone(),
      graph: (**self.inner.borrow()).clone(),
    }
  }

  fn graph(&self) -> Arc<ModuleGraph> {
    self.inner.borrow().clone()
  }
}

/// A permit for updating the module graph. When complete and
/// everything looks fine, calling `.commit()` will store the
/// new graph in the ModuleGraphContainer.
pub struct WorkerModuleGraphUpdatePermit {
  permit: deno_core::unsync::TaskQueuePermit,
  inner: Rc<RefCell<Arc<ModuleGraph>>>,
  graph: ModuleGraph,
}

impl ModuleGraphUpdatePermit for WorkerModuleGraphUpdatePermit {
  fn graph_mut(&mut self) -> &mut ModuleGraph {
    &mut self.graph
  }

  fn commit(self) {
    *self.inner.borrow_mut() = Arc::new(self.graph);
    drop(self.permit); // explicit drop for clarity
  }
}
