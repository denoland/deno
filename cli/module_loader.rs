// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use crate::emit::TypeLib;
use crate::proc_state::ProcState;

use deno_core::error::AnyError;
use deno_core::futures::future::FutureExt;
use deno_core::futures::Future;
use deno_core::ModuleLoader;
use deno_core::ModuleSpecifier;
use deno_core::OpState;
use deno_runtime::permissions::Permissions;
use std::cell::RefCell;
use std::pin::Pin;
use std::rc::Rc;
use std::str;

pub(crate) struct CliModuleLoader {
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

    Rc::new(CliModuleLoader {
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
    _is_main: bool,
  ) -> Result<ModuleSpecifier, AnyError> {
    self.ps.resolve(specifier, referrer)
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
    async move { ps.load(module_specifier, maybe_referrer, is_dynamic) }
      .boxed_local()
  }

  fn prepare_load(
    &self,
    op_state: Rc<RefCell<OpState>>,
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

    async move {
      ps.prepare_module_load(
        vec![(specifier, deno_graph::ModuleKind::Esm)],
        is_dynamic,
        lib,
        root_permissions,
        dynamic_permissions,
        false,
      )
      .await
    }
    .boxed_local()
  }
}
