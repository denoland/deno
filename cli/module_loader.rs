// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use crate::emit::TypeLib;
use crate::proc_state::ProcState;

use deno_core::error::AnyError;
use deno_core::futures::future::FutureExt;
use deno_core::futures::Future;
use deno_core::ModuleLoadId;
use deno_core::ModuleLoader;
use deno_core::ModuleSpecifier;
use deno_core::OpState;
use deno_runtime::permissions::Permissions;
use std::cell::RefCell;
use std::pin::Pin;
use std::rc::Rc;
use std::str;
use std::sync::Arc;
use std::collections::HashMap;
use deno_core::ModuleSource;
use deno_core::parking_lot::Mutex;

#[derive(Default)]
pub struct GraphData {
  pub modules: HashMap<ModuleSpecifier, Result<ModuleSource, AnyError>>,
  // because the graph detects resolution issues early, but is build and dropped
  // during the `prepare_module_load` method, we need to extract out the module
  // resolution map so that those errors can be surfaced at the appropriate time
  pub resolution_map:
    HashMap<ModuleSpecifier, HashMap<String, deno_graph::Resolved>>,
  // in some cases we want to provide the range where the resolution error
  // occurred but need to surface it on load, but on load we don't know who the
  // referrer and span was, so we need to cache those
  pub resolved_map: HashMap<ModuleSpecifier, deno_graph::Range>,
  // deno_graph detects all sorts of issues at build time (prepare_module_load)
  // but if they are errors at that stage, the don't cause the correct behaviors
  // so we cache the error and then surface it when appropriate (e.g. load)
  pub maybe_graph_error: Option<deno_graph::ModuleGraphError>,
}

pub(crate) struct CliModuleLoader {
  pub lib: TypeLib,
  graph_data: Arc<Mutex<GraphData>>,
  /// The initial set of permissions used to resolve the static imports in the
  /// worker. They are decoupled from the worker (dynamic) permissions since
  /// read access errors must be raised based on the parent thread permissions.
  pub root_permissions: Permissions,
  pub ps: ProcState,
}

impl CliModuleLoader {
  pub fn new(ps: ProcState) -> Rc<Self> {
    let lib = if ps.flags.unstable {
      TypeLib::UnstableDenoWindow
    } else {
      TypeLib::DenoWindow
    };

    Rc::new(CliModuleLoader {
      lib,
      graph_data: Default::default(),
      root_permissions: Permissions::allow_all(),
      ps,
    })
  }

  pub fn new_for_worker(ps: ProcState, permissions: Permissions) -> Rc<Self> {
    let lib = if ps.flags.unstable {
      TypeLib::UnstableDenoWorker
    } else {
      TypeLib::DenoWorker
    };

    Rc::new(CliModuleLoader {
      lib,
      graph_data: Default::default(),
      root_permissions: permissions,
      ps,
    })
  }
}

impl ModuleLoader for CliModuleLoader {
  fn resolve(
    &self,
    specifier: &str,
    referrer: &str,
    _is_main: bool,
  ) -> Result<ModuleSpecifier, AnyError> {
    self.ps.resolve(specifier, referrer, self.graph_data.clone())
  }

  fn load(
    &self,
    module_specifier: &ModuleSpecifier,
    maybe_referrer: Option<ModuleSpecifier>,
    is_dynamic: bool,
  ) -> Pin<Box<deno_core::ModuleSourceFuture>> {
    let module_specifier = module_specifier.clone();
    let ps = self.ps.clone();

    // NOTE: this block is async only because of `deno_core` interface
    // requirements; module was already loaded when constructing module graph
    // during call to `prepare_load`.
    let graph_data = self.graph_data.clone();
    async move { ps.load(module_specifier, maybe_referrer, is_dynamic, graph_data) }
      .boxed_local()
  }

  fn prepare_load(
    &self,
    op_state: Rc<RefCell<OpState>>,
    _load_id: ModuleLoadId,
    specifier: &ModuleSpecifier,
    _maybe_referrer: Option<String>,
    is_dynamic: bool,
  ) -> Pin<Box<dyn Future<Output = Result<(), AnyError>>>> {
    let specifier = specifier.clone();
    let ps = self.ps.clone();
    let state = op_state.borrow();

    let dynamic_permissions = state.borrow::<Permissions>().clone();
    let root_permissions = if is_dynamic {
      dynamic_permissions.clone()
    } else {
      self.root_permissions.clone()
    };

    let lib = match self.lib {
      TypeLib::DenoWindow => crate::emit::TypeLib::DenoWindow,
      TypeLib::DenoWorker => crate::emit::TypeLib::DenoWorker,
      TypeLib::UnstableDenoWindow => crate::emit::TypeLib::UnstableDenoWindow,
      TypeLib::UnstableDenoWorker => crate::emit::TypeLib::UnstableDenoWorker,
    };
    drop(state);

    let graph_data = self.graph_data.clone();
    // TODO(bartlomieju): `prepare_module_load` should take `load_id` param
    async move {
      ps.prepare_module_load(
        vec![specifier],
        is_dynamic,
        lib,
        root_permissions,
        dynamic_permissions,
        graph_data,
      )
      .await
    }
    .boxed_local()
  }
}
