// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use crate::module_graph::TypeLib;
use crate::proc_state::ProcState;
use deno_core::error::AnyError;
use deno_core::futures::future::FutureExt;
use deno_core::futures::Future;
use deno_core::ModuleLoadId;
use deno_core::ModuleLoader;
use deno_core::ModuleSpecifier;
use deno_core::OpState;
use deno_runtime::permissions::Permissions;
use import_map::ImportMap;
use std::cell::RefCell;
use std::pin::Pin;
use std::rc::Rc;
use std::str;

pub struct CliModuleLoader {
  /// When flags contains a `.import_map_path` option, the content of the
  /// import map file will be resolved and set.
  pub import_map: Option<ImportMap>,
  pub lib: TypeLib,
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

    let import_map = ps.maybe_import_map.clone();

    Rc::new(CliModuleLoader {
      import_map,
      lib,
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
      import_map: None,
      lib,
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
    is_main: bool,
  ) -> Result<ModuleSpecifier, AnyError> {
    // FIXME(bartlomieju): hacky way to provide compatibility with repl
    let referrer = if referrer.is_empty() && self.ps.flags.repl {
      deno_core::DUMMY_SPECIFIER
    } else {
      referrer
    };

    // TODO(ry) I think we can remove this conditional. At the time of writing
    // we don't have any tests that fail if it was removed.
    // https://github.com/WICG/import-maps/issues/157
    if !is_main {
      if let Some(import_map) = &self.import_map {
        return import_map
          .resolve(specifier, referrer)
          .map_err(AnyError::from);
      }
    }

    let module_specifier = deno_core::resolve_import(specifier, referrer)?;

    Ok(module_specifier)
  }

  fn load(
    &self,
    module_specifier: &ModuleSpecifier,
    maybe_referrer: Option<ModuleSpecifier>,
    _is_dynamic: bool,
  ) -> Pin<Box<deno_core::ModuleSourceFuture>> {
    let module_specifier = module_specifier.clone();
    let ps = self.ps.clone();

    // NOTE: this block is async only because of `deno_core`
    // interface requirements; module was already loaded
    // when constructing module graph during call to `prepare_load`.
    async move { ps.load(module_specifier, maybe_referrer) }.boxed_local()
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
    let maybe_import_map = self.import_map.clone();
    let state = op_state.borrow();

    let root_permissions = self.root_permissions.clone();
    let dynamic_permissions = state.borrow::<Permissions>().clone();

    let lib = self.lib.clone();
    drop(state);

    // TODO(bartlomieju): `prepare_module_load` should take `load_id` param
    async move {
      ps.prepare_module_load(
        specifier,
        lib,
        root_permissions,
        dynamic_permissions,
        is_dynamic,
        maybe_import_map,
      )
      .await
    }
    .boxed_local()
  }
}
