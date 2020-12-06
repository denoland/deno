// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use crate::import_map::ImportMap;
use crate::module_graph::TypeLib;
use crate::permissions::Permissions;
use crate::program_state::ProgramState;
use deno_core::error::AnyError;
use deno_core::futures::future::FutureExt;
use deno_core::futures::Future;
use deno_core::ModuleLoadId;
use deno_core::ModuleLoader;
use deno_core::ModuleSpecifier;
use deno_core::OpState;
use std::cell::RefCell;
use std::pin::Pin;
use std::rc::Rc;
use std::str;
use std::sync::Arc;

pub struct CliModuleLoader {
  /// When flags contains a `.import_map_path` option, the content of the
  /// import map file will be resolved and set.
  pub import_map: Option<ImportMap>,
  pub lib: TypeLib,
}

impl CliModuleLoader {
  pub fn new(maybe_import_map: Option<ImportMap>, unstable: bool) -> Rc<Self> {
    let lib = if unstable {
      TypeLib::UnstableDenoWindow
    } else {
      TypeLib::DenoWindow
    };
    Rc::new(CliModuleLoader {
      import_map: maybe_import_map,
      lib,
    })
  }

  pub fn new_for_worker(unstable: bool) -> Rc<Self> {
    let lib = if unstable {
      TypeLib::UnstableDenoWorker
    } else {
      TypeLib::DenoWorker
    };

    Rc::new(CliModuleLoader {
      import_map: None,
      lib,
    })
  }
}

impl ModuleLoader for CliModuleLoader {
  fn resolve(
    &self,
    op_state: Rc<RefCell<OpState>>,
    specifier: &str,
    referrer: &str,
    is_main: bool,
  ) -> Result<ModuleSpecifier, AnyError> {
    let program_state = {
      let state = op_state.borrow();
      state.borrow::<Arc<ProgramState>>().clone()
    };

    // FIXME(bartlomieju): hacky way to provide compatibility with repl
    let referrer = if referrer.is_empty() && program_state.flags.repl {
      "<unknown>"
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

    let module_specifier =
      ModuleSpecifier::resolve_import(specifier, referrer)?;

    Ok(module_specifier)
  }

  fn load(
    &self,
    op_state: Rc<RefCell<OpState>>,
    module_specifier: &ModuleSpecifier,
    maybe_referrer: Option<ModuleSpecifier>,
    _is_dynamic: bool,
  ) -> Pin<Box<deno_core::ModuleSourceFuture>> {
    let module_specifier = module_specifier.to_owned();
    let module_url_specified = module_specifier.to_string();
    let program_state = {
      let state = op_state.borrow();
      state.borrow::<Arc<ProgramState>>().clone()
    };

    // TODO(@kitsonk) this shouldn't be async
    let fut = async move {
      let compiled_module = program_state
        .fetch_compiled_module(module_specifier, maybe_referrer)?;
      Ok(deno_core::ModuleSource {
        // Real module name, might be different from initial specifier
        // due to redirections.
        code: compiled_module.code,
        module_url_specified,
        module_url_found: compiled_module.name,
      })
    };

    fut.boxed_local()
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
    let maybe_import_map = self.import_map.clone();
    let state = op_state.borrow();

    // The permissions that should be applied to any dynamically imported module
    let dynamic_permissions = state.borrow::<Permissions>().clone();
    let program_state = state.borrow::<Arc<ProgramState>>().clone();
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
