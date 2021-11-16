// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use crate::emit;
use crate::emit::TypeLib;
use crate::proc_state::ProcState;

use crate::errors::get_error_class_name;
use deno_core::error::custom_error;
use deno_core::error::AnyError;
use deno_core::futures::future::FutureExt;
use deno_core::futures::Future;
use deno_core::parking_lot::Mutex;
use deno_core::ModuleLoadId;
use deno_core::ModuleLoader;
use deno_core::ModuleSource;
use deno_core::ModuleSpecifier;
use deno_core::OpState;
use deno_graph::Dependency;
use deno_graph::ModuleGraphError;
use deno_graph::Range;
use deno_runtime::permissions::Permissions;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::collections::HashSet;
use std::collections::VecDeque;
use std::pin::Pin;
use std::rc::Rc;
use std::str;
use std::sync::Arc;

#[derive(Default)]
pub struct GraphData {
  pub modules: HashMap<ModuleSpecifier, Result<ModuleSource, ModuleGraphError>>,
  pub dependency_map: HashMap<ModuleSpecifier, BTreeMap<String, Dependency>>,
  /// A set of type libs that each module has passed a type check with this
  /// session. This would consist of window, worker or both.
  pub(crate) checked_libs_map: HashMap<ModuleSpecifier, HashSet<emit::TypeLib>>,
  /// Map of first known referrer locations for each module. Used to enhance
  /// error messages.
  pub referrer_map: HashMap<ModuleSpecifier, Range>,
}

impl GraphData {
  /// Check if `roots` are ready to be loaded by V8. Returns `Some(Ok(()))` if
  /// prepared. Returns `Some(Err(_))` if there is a known module graph error
  /// statically reachable from `roots`. Returns `None` if sufficient graph data
  /// is yet to supplied.
  pub(crate) fn check_if_prepared(
    &self,
    roots: &[ModuleSpecifier],
  ) -> Option<Result<(), AnyError>> {
    let mut seen = HashSet::<&ModuleSpecifier>::new();
    let mut visiting = VecDeque::<&ModuleSpecifier>::new();
    for root in roots {
      visiting.push_back(root);
    }
    while let Some(specifier) = visiting.pop_front() {
      match self.modules.get(specifier) {
        Some(Ok(_)) => {
          let deps = self.dependency_map.get(specifier).unwrap();
          for (_, dep) in deps.iter().rev() {
            for resolved in [&dep.maybe_code, &dep.maybe_type] {
              if !dep.is_dynamic {
                match resolved {
                  Some(Ok((dep_specifier, _))) => {
                    if !dep.is_dynamic && !seen.contains(dep_specifier) {
                      seen.insert(dep_specifier);
                      visiting.push_front(dep_specifier);
                    }
                  }
                  Some(Err(error)) => {
                    let range = error.range();
                    if !range.specifier.as_str().contains("$deno") {
                      return Some(Err(custom_error(
                        get_error_class_name(&error.clone().into()),
                        format!("{}\n    at {}", error.to_string(), range),
                      )));
                    }
                    return Some(Err(error.clone().into()));
                  }
                  None => {}
                }
              }
            }
          }
        }
        Some(Err(error)) => {
          if !roots.contains(specifier) {
            if let Some(range) = self.referrer_map.get(specifier) {
              if !range.specifier.as_str().contains("$deno") {
                let message = error.to_string();
                return Some(Err(custom_error(
                  get_error_class_name(&error.clone().into()),
                  format!("{}\n    at {}", message, range),
                )));
              }
            }
          }
          return Some(Err(error.clone().into()));
        }
        None => return None,
      }
    }
    Some(Ok(()))
  }
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
    self
      .ps
      .resolve(specifier, referrer, self.graph_data.clone())
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
        false,
      )
      .await
    }
    .boxed_local()
  }
}
