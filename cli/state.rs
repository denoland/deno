// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use crate::global_state::GlobalState;
use crate::import_map::ImportMap;
use crate::permissions::Permissions;
use crate::tsc::TargetLib;
use deno_core::error::AnyError;
use deno_core::ModuleLoadId;
use deno_core::ModuleLoader;
use deno_core::ModuleSpecifier;
use deno_core::OpState;
use futures::future::FutureExt;
use futures::Future;
use std::cell::RefCell;
use std::pin::Pin;
use std::rc::Rc;
use std::str;
use std::sync::Arc;

// This is named "CliState" instead of just "State" to avoid confusion with all
// other state structs (GlobalState, OpState, GothamState).
// TODO(ry) Many of the items in this struct should be moved out and into
// OpState, removing redundant RefCell wrappers if possible.
pub struct CliState {
  pub global_state: Arc<GlobalState>,
  pub main_module: ModuleSpecifier,
  /// When flags contains a `.import_map_path` option, the content of the
  /// import map file will be resolved and set.
  pub import_map: Option<ImportMap>,
  pub target_lib: TargetLib,
  pub is_main: bool,
}

pub fn exit_unstable(api_name: &str) {
  eprintln!(
    "Unstable API '{}'. The --unstable flag must be provided.",
    api_name
  );
  std::process::exit(70);
}

impl ModuleLoader for CliState {
  fn resolve(
    &self,
    specifier: &str,
    referrer: &str,
    is_main: bool,
  ) -> Result<ModuleSpecifier, AnyError> {
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
    module_specifier: &ModuleSpecifier,
    maybe_referrer: Option<ModuleSpecifier>,
    _is_dyn_import: bool,
  ) -> Pin<Box<deno_core::ModuleSourceFuture>> {
    let module_specifier = module_specifier.to_owned();
    let module_url_specified = module_specifier.to_string();
    let global_state = self.global_state.clone();

    // TODO(bartlomieju): `fetch_compiled_module` should take `load_id` param
    let fut = async move {
      let compiled_module = global_state
        .fetch_compiled_module(module_specifier, maybe_referrer)
        .await?;
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
    module_specifier: &ModuleSpecifier,
    maybe_referrer: Option<String>,
    is_dyn_import: bool,
  ) -> Pin<Box<dyn Future<Output = Result<(), AnyError>>>> {
    let module_specifier = module_specifier.clone();
    let target_lib = self.target_lib.clone();
    let maybe_import_map = self.import_map.clone();
    // Only "main" module is loaded without permission check,
    // ie. module that is associated with "is_main" state
    // and is not a dynamic import.
    let permissions = if self.is_main && !is_dyn_import {
      Permissions::allow_all()
    } else {
      let state = op_state.borrow();
      state.borrow::<Permissions>().clone()
    };
    let global_state = self.global_state.clone();
    // TODO(bartlomieju): I'm not sure if it's correct to ignore
    // bad referrer - this is the case for `Deno.core.evalContext()` where
    // `ref_str` is `<unknown>`.
    let maybe_referrer = if let Some(ref_str) = maybe_referrer {
      ModuleSpecifier::resolve_url(&ref_str).ok()
    } else {
      None
    };

    // TODO(bartlomieju): `prepare_module_load` should take `load_id` param
    async move {
      global_state
        .prepare_module_load(
          module_specifier,
          maybe_referrer,
          target_lib,
          permissions,
          is_dyn_import,
          maybe_import_map,
        )
        .await
    }
    .boxed_local()
  }
}

impl CliState {
  /// If `shared_permission` is None then permissions from globa state are used.
  pub fn new(
    global_state: &Arc<GlobalState>,
    main_module: ModuleSpecifier,
    maybe_import_map: Option<ImportMap>,
  ) -> Result<Rc<Self>, AnyError> {
    let state = CliState {
      global_state: global_state.clone(),
      main_module,
      import_map: maybe_import_map,
      target_lib: TargetLib::Main,
      is_main: true,
    };
    Ok(Rc::new(state))
  }

  /// If `shared_permission` is None then permissions from globa state are used.
  pub fn new_for_worker(
    global_state: &Arc<GlobalState>,
    main_module: ModuleSpecifier,
  ) -> Result<Rc<Self>, AnyError> {
    let state = CliState {
      global_state: global_state.clone(),
      main_module,
      import_map: None,
      target_lib: TargetLib::Worker,
      is_main: false,
    };
    Ok(Rc::new(state))
  }

  #[cfg(test)]
  pub fn mock(main_module: &str) -> Rc<Self> {
    let module_specifier = ModuleSpecifier::resolve_url_or_path(main_module)
      .expect("Invalid entry module");
    CliState::new(
      &GlobalState::mock(vec!["deno".to_string()], None),
      module_specifier,
      None,
    )
    .unwrap()
  }

  /// Quits the process if the --unstable flag was not provided.
  ///
  /// This is intentionally a non-recoverable check so that people cannot probe
  /// for unstable APIs from stable programs.
  pub fn check_unstable(&self, api_name: &str) {
    // TODO(ry) Maybe use IsolateHandle::terminate_execution here to provide a
    // stack trace in JS.
    if !self.global_state.flags.unstable {
      exit_unstable(api_name);
    }
  }
}
