// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use deno_core::unsync::TaskQueue;
use deno_core::unsync::TaskQueuePermit;
use deno_graph::ModuleGraph;

/// Holds the `ModuleGraph`.
pub struct WorkerModuleGraphContainer {
  // Allow only one request to update the graph data at a time,
  // but allow other requests to read from it at any time even
  // while another request is updating the data.
  update_queue: Rc<TaskQueue>,
  inner: Rc<RefCell<Arc<ModuleGraph>>>,
}

impl WorkerModuleGraphContainer {
  pub fn new(module_graph: Arc<ModuleGraph>) -> Self {
    Self {
      update_queue: Default::default(),
      inner: Rc::new(RefCell::new(module_graph)),
    }
  }

  /// Acquires a permit to modify the module graph without other code
  /// having the chance to modify it. In the meantime, other code may
  /// still read from the existing module graph.
  pub async fn acquire_update_permit(&self) -> WorkerModuleGraphUpdatePermit {
    let permit = self.update_queue.acquire().await;
    WorkerModuleGraphUpdatePermit {
      permit,
      inner: self.inner.clone(),
      graph: (**self.inner.borrow()).clone(),
    }
  }

  pub fn graph(&self) -> Arc<ModuleGraph> {
    self.inner.borrow().clone()
  }
}

/// A permit for updating the module graph. When complete and
/// everything looks fine, calling `.commit()` will store the
/// new graph in the ModuleGraphContainer.
pub struct WorkerModuleGraphUpdatePermit {
  permit: TaskQueuePermit,
  inner: Rc<RefCell<Arc<ModuleGraph>>>,
  graph: ModuleGraph,
}

impl WorkerModuleGraphUpdatePermit {
  /// Gets the module graph for mutation.
  pub fn graph_mut(&mut self) -> &mut ModuleGraph {
    &mut self.graph
  }

  /// Saves the mutated module graph in the container.
  pub fn commit(self) {
    *self.inner.borrow_mut() = Arc::new(self.graph);
    drop(self.permit); // explicit drop for clarity
  }
}
