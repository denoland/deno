// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use crate::import_map::ImportMap;
use crate::module_graph::TypeLib;
use crate::program_state::ProgramState;
use deno_core::error::AnyError;
use deno_core::futures::future::FutureExt;
use deno_core::futures::Future;
use deno_core::ModuleLoadId;
use deno_core::ModuleLoader;
use deno_core::ModuleSpecifier;
use deno_core::OpState;
use deno_runtime::permissions::Permissions;
use std::cell::RefCell;
use std::collections::HashMap;
use std::pin::Pin;
use std::rc::Rc;
use std::str;
use std::sync::Arc;

pub struct CliModuleLoader {
  /// When flags contains a `.import_map_path` option, the content of the
  /// import map file will be resolved and set.
  pub import_map: Option<ImportMap>,
  pub lib: TypeLib,
  /// The initial set of permissions used to resolve the imports in the worker.
  /// They are decoupled from the worker permissions since read access errors
  /// must be raised based on the parent thread permissions
  pub initial_permissions: Rc<RefCell<Option<Permissions>>>,
  pub program_state: Arc<ProgramState>,
}

impl CliModuleLoader {
  pub fn new(program_state: Arc<ProgramState>) -> Rc<Self> {
    let lib = if program_state.flags.unstable {
      TypeLib::UnstableDenoWindow
    } else {
      TypeLib::DenoWindow
    };

    let import_map = program_state.maybe_import_map.clone();

    Rc::new(CliModuleLoader {
      import_map,
      lib,
      initial_permissions: Rc::new(RefCell::new(None)),
      program_state,
    })
  }

  pub fn new_for_worker(
    program_state: Arc<ProgramState>,
    permissions: Permissions,
  ) -> Rc<Self> {
    let lib = if program_state.flags.unstable {
      TypeLib::UnstableDenoWorker
    } else {
      TypeLib::DenoWorker
    };

    Rc::new(CliModuleLoader {
      import_map: None,
      lib,
      initial_permissions: Rc::new(RefCell::new(Some(permissions))),
      program_state,
    })
  }
}

impl ModuleLoader for CliModuleLoader {
  fn resolve(
    &self,
    _op_state: Rc<RefCell<OpState>>,
    specifier: &str,
    referrer: &str,
    is_main: bool,
  ) -> Result<ModuleSpecifier, AnyError> {
    // FIXME(bartlomieju): hacky way to provide compatibility with repl
    let referrer = if referrer.is_empty() && self.program_state.flags.repl {
      deno_core::DUMMY_SPECIFIER
    } else {
      referrer
    };

    if !is_main {
      if let Some(import_map) = &self.import_map {
        let result = import_map.resolve(specifier, referrer)?;
        if let Some(r) = result {
          return Ok(r);
        }
      }
    }

    let module_specifier = deno_core::resolve_import(specifier, referrer)?;

    Ok(module_specifier)
  }

  fn load(
    &self,
    _op_state: Rc<RefCell<OpState>>,
    module_specifier: &ModuleSpecifier,
    maybe_referrer: Option<ModuleSpecifier>,
    _import_assertions: HashMap<String, String>,
    _is_dynamic: bool,
  ) -> Pin<Box<deno_core::ModuleSourceFuture>> {
    let module_specifier = module_specifier.clone();
    let program_state = self.program_state.clone();

    // NOTE: this block is async only because of `deno_core`
    // interface requirements; module was already loaded
    // when constructing module graph during call to `prepare_load`.
    async move { program_state.load(module_specifier, maybe_referrer) }
      .boxed_local()
  }

  fn prepare_load(
    &self,
    op_state: Rc<RefCell<OpState>>,
    _load_id: ModuleLoadId,
    specifier: &ModuleSpecifier,
    _maybe_referrer: Option<String>,
    _import_assertions: HashMap<String, String>,
    is_dynamic: bool,
  ) -> Pin<Box<dyn Future<Output = Result<(), AnyError>>>> {
    let specifier = specifier.clone();
    let program_state = self.program_state.clone();
    let maybe_import_map = self.import_map.clone();
    let state = op_state.borrow();

    // The permissions that should be applied to any dynamically imported module
    let dynamic_permissions =
      // If there are initial permissions assigned to the loader take them 
      // and use only once for top level module load.
      // Otherwise use permissions assigned to the current worker.
      if let Some(permissions) = self.initial_permissions.borrow_mut().take() {
        permissions
      } else {
        state.borrow::<Permissions>().clone()
      };

    let lib = self.lib.clone();
    drop(state);

    // TODO(bartlomieju): `prepare_module_load` should take `load_id` param
    async move {
      program_state
        .prepare_module_load(
          specifier,
          lib,
          dynamic_permissions,
          is_dynamic,
          maybe_import_map,
        )
        .await
    }
    .boxed_local()
  }
}
