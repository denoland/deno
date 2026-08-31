// Copyright 2018-2026 the Deno authors. MIT license.

use std::borrow::Cow;
use std::cell::Cell;
use std::cell::Ref;
use std::cell::RefCell;
use std::collections::HashMap;
use std::collections::HashSet;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::task::Context;
use std::task::Poll;

use deno_core::FastString;
use deno_core::error::CoreError;
use deno_error::JsErrorBox;
use futures::future::FutureExt;
use futures::stream::StreamFuture;
use futures::task::AtomicWaker;

use super::CustomModuleEvaluationKind;
use super::ImportAttributesContext;
use super::IntoModuleCodeString;
use super::IntoModuleName;
use super::ModuleConcreteError;
use super::RequestedModuleType;
use super::module_map_data::ModuleMapData;
use super::module_map_data::ModuleMapSnapshotData;
use crate::FastStaticString;
use crate::JsRuntime;
use crate::ModuleCodeBytes;
use crate::ModuleResolveResponse;
use crate::ModuleSource;
use crate::ModuleSourceCode;
use crate::ModuleSpecifier;
use crate::ascii_str;
use crate::error::CoreErrorKind;
#[cfg(debug_assertions)]
use crate::error::JsError;
use crate::error::exception_to_err_result;
use crate::modules::ImportAttributesKind;
use crate::modules::ModuleCodeString;
use crate::modules::ModuleError;
use crate::modules::ModuleId;
use crate::modules::ModuleImportPhase;
use crate::modules::ModuleLoadId;
use crate::modules::ModuleLoader;
use crate::modules::ModuleName;
use crate::modules::ModuleReference;
use crate::modules::ModuleRequest;
use crate::modules::ModuleType;
use crate::modules::ResolutionKind;
use crate::modules::get_requested_module_type_from_attributes;
use crate::modules::module_map_data::ModuleSourceKey;
use crate::modules::parse_import_attributes;
use crate::modules::recursive_load::RecursiveModuleLoad;
use crate::runtime::JsRealm;
use crate::runtime::SnapshotLoadDataStore;
use crate::runtime::SnapshotStoreDataStore;
use crate::runtime::exception_state::ExceptionState;
use crate::source_map::DATA_PREFIX;
use crate::source_map::SourceMapper;

mod dynamic;
mod evaluation;
mod ext_script;
mod tracked;
mod wasm;

use dynamic::DynImportModEvaluate;
use dynamic::DynImportState;
use dynamic::PrepareLoadFuture;
pub use ext_script::wrap_lazy_ext_script;
use tracked::TrackedFutures;
use tracked::TrackedVec;

fn is_internal_scheme(scheme: &str) -> bool {
  matches!(scheme, "ext" | "node" | "checkin")
}

pub(crate) fn is_internal_module_specifier(specifier: &str) -> bool {
  let Ok(specifier) = ModuleSpecifier::parse(specifier) else {
    return false;
  };
  is_internal_scheme(specifier.scheme())
}

type CodeCacheReadyFuture = dyn Future<Output = ()>;

type CodeCacheReadyCallback =
  Box<dyn FnOnce(&[u8]) -> Pin<Box<dyn Future<Output = ()>>>>;
pub(crate) struct CodeCacheInfo {
  data: Option<Cow<'static, [u8]>>,
  ready_callback: CodeCacheReadyCallback,
}

pub const BOM_CHAR: &[u8] = &[0xef, 0xbb, 0xbf];

/// Strips the byte order mark from the provided text if it exists.
fn strip_bom(source_code: &[u8]) -> &[u8] {
  if source_code.starts_with(BOM_CHAR) {
    &source_code[BOM_CHAR.len()..]
  } else {
    source_code
  }
}

/// A collection of JS modules.
pub(crate) struct ModuleMap {
  // Handling of futures for loading module sources
  // TODO(mmastrac): we should not be swapping this loader out
  pub(crate) loader: RefCell<Rc<dyn ModuleLoader>>,

  pub(crate) source_mapper: Rc<RefCell<SourceMapper>>,
  exception_state: Rc<ExceptionState>,
  dynamic_import_map: RefCell<HashMap<ModuleLoadId, DynImportState>>,
  preparing_dynamic_imports: TrackedFutures<Pin<Box<PrepareLoadFuture>>>,
  pending_dynamic_imports: TrackedFutures<StreamFuture<RecursiveModuleLoad>>,
  pending_dyn_mod_evaluations: TrackedVec<DynImportModEvaluate>,
  pending_tla_waiters:
    RefCell<HashMap<ModuleId, Vec<v8::Global<v8::PromiseResolver>>>>,
  pending_mod_evaluation: Cell<bool>,
  /// Set to `true` while inside `module.evaluate()` in `mod_evaluate`.
  /// Used to suppress microtask checkpoints in `lazy_load_es_module_with_code`
  /// during module evaluation, preventing premature draining of TLA-related microtasks.
  evaluating_top_level: Cell<bool>,
  code_cache_ready_futs: TrackedFutures<Pin<Box<CodeCacheReadyFuture>>>,
  module_waker: AtomicWaker,
  data: RefCell<ModuleMapData>,
  will_snapshot: bool,
  loading_internal_modules: Cell<bool>,

  /// A counter used to delay our dynamic import deadlock detection by one spin
  /// of the event loop.
  pub(crate) dyn_module_evaluate_idle_counter: Cell<u32>,

  /// Tracks module IDs currently being evaluated via `op_import_sync` to
  /// detect require() cycles that V8's module status alone cannot catch.
  pub(crate) import_sync_eval_stack: RefCell<Vec<ModuleId>>,
}

struct LoadingInternalModulesGuard<'a> {
  module_map: &'a ModuleMap,
  previous: bool,
}

impl<'a> LoadingInternalModulesGuard<'a> {
  fn new(module_map: &'a ModuleMap) -> Self {
    Self {
      module_map,
      previous: module_map.loading_internal_modules.replace(true),
    }
  }
}

impl Drop for LoadingInternalModulesGuard<'_> {
  fn drop(&mut self) {
    self.module_map.loading_internal_modules.set(self.previous);
  }
}

/// Outcome of compiling a module's source.
pub(crate) enum NewModuleResult {
  Ready(ModuleId),
}

impl NewModuleResult {
  fn into_ready(self) -> ModuleId {
    match self {
      NewModuleResult::Ready(id) => id,
    }
  }
}

impl ModuleMap {
  /// There is a circular Rc reference between the module map and the futures,
  /// so when destroying the module map we need to clear the pending futures.
  pub(crate) fn destroy(&self) {
    self.dynamic_import_map.borrow_mut().clear();
    self.preparing_dynamic_imports.clear();
    self.pending_dynamic_imports.clear();
    self.pending_dyn_mod_evaluations.clear();
    self.pending_tla_waiters.borrow_mut().clear();
    self.code_cache_ready_futs.clear();
    std::mem::take(&mut *self.data.borrow_mut());
  }

  pub(crate) fn next_load_id(&self) -> i32 {
    // TODO(mmastrac): move recursive module loading into here so we can avoid making this pub
    let mut data = self.data.borrow_mut();
    let id = data.next_load_id;
    data.next_load_id += 1;
    id + 1
  }

  #[cfg(debug_assertions)]
  pub(crate) fn check_all_modules_evaluated(
    &self,
    scope: &mut v8::PinScope,
  ) -> Result<(), CoreError> {
    let mut not_evaluated = vec![];
    let data = self.data.borrow();

    for (handle, i) in data.handles_inverted.iter() {
      let module = v8::Local::new(scope, handle);
      match module.get_status() {
        v8::ModuleStatus::Errored => {
          return Err(
            CoreErrorKind::Js(JsError::from_v8_exception(
              scope,
              module.get_exception(),
            ))
            .into_box(),
          );
        }
        v8::ModuleStatus::Evaluated => {}
        _ => {
          not_evaluated.push(data.info[*i].name.as_str().to_string());
        }
      }
    }

    if !not_evaluated.is_empty() {
      return Err(CoreErrorKind::NonEvaluatedModules(not_evaluated).into_box());
    }

    Ok(())
  }

  pub(crate) fn new(
    loader: Rc<dyn ModuleLoader>,
    source_mapper: Rc<RefCell<SourceMapper>>,
    exception_state: Rc<ExceptionState>,
    will_snapshot: bool,
  ) -> Self {
    Self {
      will_snapshot,
      loader: loader.into(),
      source_mapper,
      exception_state,
      dyn_module_evaluate_idle_counter: Default::default(),
      dynamic_import_map: Default::default(),
      preparing_dynamic_imports: Default::default(),
      pending_dynamic_imports: Default::default(),
      pending_dyn_mod_evaluations: Default::default(),
      pending_tla_waiters: Default::default(),
      pending_mod_evaluation: Default::default(),
      evaluating_top_level: Default::default(),
      code_cache_ready_futs: Default::default(),
      module_waker: Default::default(),
      data: Default::default(),
      loading_internal_modules: Default::default(),
      import_sync_eval_stack: Default::default(),
    }
  }

  pub(crate) fn set_loading_internal_modules(&self, value: bool) {
    self.loading_internal_modules.set(value);
  }

  pub(crate) fn update_with_snapshotted_data(
    &self,
    scope: &mut v8::PinScope,
    data_store: &mut SnapshotLoadDataStore,
    data: ModuleMapSnapshotData,
  ) {
    self
      .data
      .borrow_mut()
      .update_with_snapshotted_data(scope, data_store, data);
  }

  /// Get module id, following all aliases in case of module specifier
  /// that had been redirected.
  pub(crate) fn get_id(
    &self,
    name: &str,
    requested_module_type: impl AsRef<RequestedModuleType>,
  ) -> Option<ModuleId> {
    self.data.borrow().get_id(name, requested_module_type)
  }

  /// Register an additional `(name, requested_module_type) -> module_id`
  /// mapping for an already-registered module. See
  /// `ModuleMapData::register_under_type`.
  pub(crate) fn register_under_type(
    &self,
    name: FastString,
    requested_module_type: &RequestedModuleType,
    module_id: ModuleId,
  ) {
    self.data.borrow_mut().register_under_type(
      name,
      requested_module_type,
      module_id,
    );
  }

  pub(crate) fn is_main_module(&self, global: &v8::Global<v8::Module>) -> bool {
    self.data.borrow().is_main_module(global)
  }

  pub(crate) fn is_main_module_id(&self, id: ModuleId) -> bool {
    self.data.borrow().main_module_id == Some(id)
  }

  pub(crate) fn get_name_by_module(
    &self,
    global: &v8::Global<v8::Module>,
  ) -> Option<String> {
    self.data.borrow().get_name_by_module(global)
  }

  pub(crate) fn get_name_by_id(&self, id: ModuleId) -> Option<String> {
    self.data.borrow().get_name_by_id(id)
  }

  pub(crate) fn get_type_by_module(
    &self,
    global: &v8::Global<v8::Module>,
  ) -> Option<ModuleType> {
    self.data.borrow().get_type_by_module(global)
  }

  pub(crate) fn get_handle(
    &self,
    id: ModuleId,
  ) -> Option<v8::Global<v8::Module>> {
    self.data.borrow().get_handle(id)
  }

  /// For each module id, whether its V8 module has already been instantiated
  /// (or evaluated). Used to decide which import edges are dead weight in the
  /// snapshot — see [`ModuleMapData::serialize_for_snapshotting`].
  pub(crate) fn instantiated_flags(
    &self,
    scope: &mut v8::PinScope,
  ) -> Vec<bool> {
    self
      .data
      .borrow()
      .handles
      .iter()
      .map(|handle| {
        let module = v8::Local::new(scope, handle);
        !matches!(
          module.get_status(),
          v8::ModuleStatus::Uninstantiated | v8::ModuleStatus::Instantiating
        )
      })
      .collect()
  }

  pub(crate) fn serialize_for_snapshotting(
    &self,
    data_store: &mut SnapshotStoreDataStore,
    instantiated: &[bool],
  ) -> ModuleMapSnapshotData {
    let data = std::mem::take(&mut *self.data.borrow_mut());
    data.serialize_for_snapshotting(data_store, instantiated)
  }

  #[cfg(test)]
  pub fn is_alias(
    &self,
    name: &str,
    requested_module_type: impl AsRef<RequestedModuleType>,
  ) -> bool {
    self.data.borrow().is_alias(name, requested_module_type)
  }

  pub(crate) fn get_data(&self) -> &RefCell<ModuleMapData> {
    &self.data
  }

  #[cfg(test)]
  pub fn assert_module_map(
    &self,
    modules: &Vec<super::ModuleInfo>,
    restored_from_snapshot: usize,
  ) {
    self
      .data
      .borrow()
      .assert_module_map(modules, restored_from_snapshot);
  }

  #[cfg(all(test, not(miri)))]
  pub(crate) fn new_module(
    &self,
    scope: &mut v8::PinScope,
    main: bool,
    dynamic: bool,
    module_source: ModuleSource,
  ) -> Result<ModuleId, ModuleError> {
    Ok(
      self
        .new_module_with_pending(scope, main, dynamic, module_source)?
        .into_ready(),
    )
  }

  pub(crate) fn new_module_with_pending(
    &self,
    scope: &mut v8::PinScope,
    main: bool,
    dynamic: bool,
    module_source: ModuleSource,
  ) -> Result<NewModuleResult, ModuleError> {
    let ModuleSource {
      code,
      module_type,
      module_url_found,
      module_url_specified,
      code_cache,
    } = module_source;

    // Register the module in the module map unless it's already there. If the
    // specified URL and the "true" URL are different, register the alias.
    let module_url_found = if let Some(module_url_found) = module_url_found {
      let (module_url_found1, module_url_found2) =
        module_url_found.into_cheap_copy();
      self.data.borrow_mut().alias(
        module_url_specified,
        &module_type.clone().into(),
        module_url_found1,
      );
      module_url_found2
    } else {
      module_url_specified
    };

    // TODO(bartlomieju): I have a hunch that this is wrong - write a test
    // that tries to "confuse" the type system, by first requesting a module
    // with type `RequestedModuleType::Other("foo".into)``, and then the loader
    // actually returns `ModuleType::Other("bar".into())`. See if it leads to
    // unexpected result in how `ModuleMap` is structured and verify how
    // querying the module map works (`ModuleMap::get_by_id`, `ModuleMap::get_by_name`).
    let requested_module_type = RequestedModuleType::from(module_type.clone());
    let maybe_module_id = self.get_id(&module_url_found, requested_module_type);

    if let Some(module_id) = maybe_module_id {
      return Ok(NewModuleResult::Ready(module_id));
    }
    let module_id = match module_type {
      ModuleType::JavaScript => {
        let code = ModuleSource::get_string_source(code);

        let (code_cache_info, module_url_found) =
          if let Some(code_cache) = code_cache {
            let (module_url_found1, module_url_found2) =
              module_url_found.into_cheap_copy();
            let loader = self.loader.borrow().clone();
            (
              Some(CodeCacheInfo {
                data: code_cache.data,
                ready_callback: Box::new(move |cache| {
                  let specifier =
                    ModuleSpecifier::parse(module_url_found1.as_str()).unwrap();
                  loader.code_cache_ready(specifier, code_cache.hash, cache)
                }),
              }),
              module_url_found2,
            )
          } else {
            (None, module_url_found)
          };

        self
          .new_module_from_js_source_with_pending(
            scope,
            main,
            ModuleType::JavaScript,
            module_url_found,
            code,
            dynamic,
            code_cache_info,
          )?
          .into_ready()
      }
      ModuleType::Wasm => {
        self.new_wasm_module(scope, module_url_found, code, dynamic)?
      }
      ModuleType::Json => self.new_json_module(
        scope,
        module_url_found,
        ModuleSource::get_string_source(code),
      )?,
      ModuleType::Text => self.new_text_module(
        scope,
        module_url_found,
        ModuleSource::get_string_source(code),
      )?,
      ModuleType::Bytes => {
        let ModuleSourceCode::Bytes(code) = code else {
          return Err(ModuleError::Concrete(
            ModuleConcreteError::BytesNotBytes,
          ));
        };
        self.new_bytes_module(scope, module_url_found, code)?
      }
      ModuleType::Other(module_type) => {
        let state = JsRuntime::state_from(scope);
        let custom_module_evaluation_cb =
          state.custom_module_evaluation_cb.as_ref();

        let Some(custom_evaluation_cb) = custom_module_evaluation_cb else {
          return Err(ModuleError::Concrete(
            ModuleConcreteError::UnsupportedKind(module_type.to_string()),
          ));
        };

        // TODO(bartlomieju): creating a global just to create a local from it
        // seems superfluous. However, changing `CustomModuleEvaluationCb` to have
        // a lifetime will have a viral effect and required `JsRuntimeOptions`
        // to have a callback as well as `JsRuntime`.
        let module_evaluation_kind = custom_evaluation_cb(
          scope,
          module_type.clone(),
          &module_url_found,
          code,
        )
        .map_err(|e| ModuleError::Core(e.into()))?;

        match module_evaluation_kind {
          // Simple case, we just got a single value so we create a regular
          // synthetic module.
          CustomModuleEvaluationKind::Synthetic(value_global) => {
            let value = v8::Local::new(scope, value_global);
            let exports = vec![(ascii_str!("default"), value)];
            self.new_synthetic_module(
              scope,
              module_url_found,
              ModuleType::Other(module_type.clone()),
              exports,
            )
          }

          // Complex case - besides a synthetic module, we will create a new
          // module from JS code.
          CustomModuleEvaluationKind::ComputedAndSynthetic(
            computed_src,
            synthetic_value,
            synthetic_module_type,
          ) => {
            let (url1, url2) = module_url_found.into_cheap_copy();
            let value = v8::Local::new(scope, synthetic_value);
            let exports = vec![(ascii_str!("default"), value)];
            let _synthetic_mod_id = self.new_synthetic_module(
              scope,
              url1,
              synthetic_module_type,
              exports,
            );

            let (code_cache_info, url2) = if let Some(code_cache) = code_cache {
              let (url1, url2) = url2.into_cheap_copy();
              let loader = self.loader.borrow().clone();
              (
                Some(CodeCacheInfo {
                  data: code_cache.data,
                  ready_callback: Box::new(move |cache| {
                    let specifier =
                      ModuleSpecifier::parse(url1.as_str()).unwrap();
                    loader.code_cache_ready(specifier, code_cache.hash, cache)
                  }),
                }),
                url2,
              )
            } else {
              (None, url2)
            };

            self
              .new_module_from_js_source_with_pending(
                scope,
                main,
                ModuleType::Other(module_type.clone()),
                url2,
                computed_src,
                dynamic,
                code_cache_info,
              )?
              .into_ready()
          }
        }
      }
    };
    Ok(NewModuleResult::Ready(module_id))
  }

  /// Creates a synthetic module whose exports mirror the own string-keyed
  /// properties of `exports_obj`, plus a `default` export pointing at
  /// `exports_obj` itself. Matches the shape of Node's
  /// `BuiltinModule.getESMFacade` so a CJS-style polyfill module can be
  /// imported as ESM without a hand-written wrapper.
  ///
  /// Property values are read once at creation time — synthetic exports
  /// are static snapshots, not live references back to the object.
  pub fn new_synthetic_module_from_exports_object<'s, 'i>(
    &self,
    scope: &mut v8::PinScope<'s, 'i>,
    name: impl IntoModuleName,
    exports_obj: v8::Local<'s, v8::Object>,
  ) -> ModuleId {
    let name = name.into_module_name();
    let name_str = name.v8_string(scope).unwrap();

    // Enumerate own string-keyed properties of the exports object.
    let property_names = exports_obj
      .get_own_property_names(
        scope,
        v8::GetPropertyNamesArgsBuilder::new()
          .mode(v8::KeyCollectionMode::OwnOnly)
          .property_filter(v8::PropertyFilter::SKIP_SYMBOLS)
          .key_conversion(v8::KeyConversionMode::ConvertToString)
          .build(),
      )
      .unwrap();
    let len = property_names.length();

    let mut export_names: Vec<v8::Local<v8::String>> =
      Vec::with_capacity(len as usize + 1);
    let mut export_values: Vec<v8::Local<v8::Value>> =
      Vec::with_capacity(len as usize + 1);
    // If the IIFE returns `{ default: <ns>, ...named }`, treat the inner
    // `default` as the ESM default export. This mirrors the manual
    // `export default mod.default` pattern used by the old `*_esm.ts`
    // wrappers and Node's behavior for builtins whose `module.exports`
    // includes a `default` property. Otherwise fall back to the entire
    // exports object as the default (matches `module.exports = { ... }`
    // shape).
    let mut default_value: v8::Local<v8::Value> = exports_obj.into();
    for i in 0..len {
      let key_val = property_names.get_index(scope, i).unwrap();
      let key_str = key_val.to_string(scope).unwrap();
      let value = exports_obj.get(scope, key_val).unwrap();
      if key_str.to_rust_string_lossy(scope) == "default" {
        default_value = value;
        continue;
      }
      export_names.push(key_str);
      export_values.push(value);
    }
    let default_str = v8::String::new(scope, "default").unwrap();
    export_names.push(default_str);
    export_values.push(default_value);

    let module = v8::Module::create_synthetic_module(
      scope,
      name_str,
      &export_names,
      synthetic_module_evaluation_steps,
    );

    let handle = v8::Global::<v8::Module>::new(scope, module);
    let mut exports_global = Vec::with_capacity(export_names.len());
    for i in 0..export_names.len() {
      exports_global.push((
        v8::Global::new(scope, export_names[i]),
        v8::Global::new(scope, export_values[i]),
      ));
    }

    self
      .data
      .borrow_mut()
      .synthetic_module_exports_store
      .insert(handle.clone(), exports_global);

    let id = self.data.borrow_mut().create_module_info(
      name,
      ModuleType::JavaScript,
      handle,
      false,
      vec![],
    );

    // Synthetic modules have no imports so their instantation must never fail.
    self.instantiate_module(scope, id).unwrap();
    // Eagerly evaluate so the `synthetic_module_evaluation_steps` callback
    // fires now (which sets the exports from the staged store) instead of
    // at first read. Important during snapshot creation: V8 needs the
    // module in `Evaluated` state with its exports populated before the
    // snapshot is serialized; otherwise consumers that look up the
    // module's namespace at snapshot-finalize time hit
    // "GetModuleNamespace must be used on an instantiated module" or get
    // unbound exports. Evaluation is synchronous for synthetic modules.
    {
      let handle = self.get_handle(id).unwrap();
      let local = v8::Local::new(scope, handle);
      let _ = local.evaluate(scope);
    }

    id
  }

  /// Creates a "synthetic module", that contains only a single, "default" export.
  ///
  /// The module gets instantiated and its ID is returned.
  pub fn new_synthetic_module<'s, 'i>(
    &self,
    scope: &mut v8::PinScope<'s, 'i>,
    name: impl IntoModuleName,
    module_type: ModuleType,
    exports: Vec<(FastStaticString, v8::Local<'s, v8::Value>)>,
  ) -> ModuleId {
    let name = name.into_module_name();
    let name_str = name.v8_string(scope).unwrap();

    let export_names = exports
      .iter()
      .map(|(name, _)| name.v8_string(scope).unwrap())
      .collect::<Vec<_>>();
    let module = v8::Module::create_synthetic_module(
      scope,
      name_str,
      &export_names,
      synthetic_module_evaluation_steps,
    );

    let handle = v8::Global::<v8::Module>::new(scope, module);
    let mut exports_global = Vec::with_capacity(exports.len());

    for i in 0..exports.len() {
      let export_name = export_names[i];
      let (_, export_value) = exports[i];
      exports_global.push((
        v8::Global::new(scope, export_name),
        v8::Global::new(scope, export_value),
      ));
    }

    self
      .data
      .borrow_mut()
      .synthetic_module_exports_store
      .insert(handle.clone(), exports_global);

    let id = self.data.borrow_mut().create_module_info(
      name,
      module_type,
      handle,
      false,
      vec![],
    );

    // Synthetic modules have no imports so their instantation must never fail.
    self.instantiate_module(scope, id).unwrap();

    id
  }

  // TODO(bartlomieju): remove this method or rename it to `new_js_module`.
  /// Create and compile an ES module.
  pub(crate) fn new_es_module(
    &self,
    scope: &mut v8::PinScope,
    main: bool,
    name: ModuleName,
    source: ModuleCodeString,
    is_dynamic_import: bool,
    code_cache_info: Option<CodeCacheInfo>,
  ) -> Result<ModuleId, ModuleError> {
    self.new_module_from_js_source(
      scope,
      main,
      ModuleType::JavaScript,
      name,
      source,
      is_dynamic_import,
      code_cache_info,
    )
  }

  /// Provided given JavaScript source code, compile and create a module of given
  /// type.
  ///
  /// Passed type doesn't have to be [`ModuleType::JavaScript`]! This method
  /// can be used to create "shim" modules, that execute some JS and act as a
  /// proxy to the actual underlying module (eg. you might create a "shim" for
  /// Wasm module).
  ///
  /// Imports in the executed code are parsed (along their import attributes)
  /// and attached to associated [`ModuleInfo`].
  ///
  /// Returns an ID of newly created module.
  ///
  /// Sync call sites can use this directly.
  #[allow(clippy::too_many_arguments, reason = "TODO: cleanup")]
  pub(crate) fn new_module_from_js_source(
    &self,
    scope: &mut v8::PinScope,
    main: bool,
    module_type: ModuleType,
    name: ModuleName,
    source: ModuleCodeString,
    is_dynamic_import: bool,
    code_cache_info: Option<CodeCacheInfo>,
  ) -> Result<ModuleId, ModuleError> {
    Ok(
      self
        .new_module_from_js_source_with_pending(
          scope,
          main,
          module_type,
          name,
          source,
          is_dynamic_import,
          code_cache_info,
        )?
        .into_ready(),
    )
  }

  /// Same as [`new_module_from_js_source`] but returns [`NewModuleResult`].
  #[allow(clippy::too_many_arguments, reason = "TODO: cleanup")]
  pub(crate) fn new_module_from_js_source_with_pending(
    &self,
    scope: &mut v8::PinScope,
    main: bool,
    module_type: ModuleType,
    name: ModuleName,
    source: ModuleCodeString,
    is_dynamic_import: bool,
    mut code_cache_info: Option<CodeCacheInfo>,
  ) -> Result<NewModuleResult, ModuleError> {
    if main {
      let data = self.data.borrow();
      if let Some(main_module) = data.main_module_id {
        let main_name = self.data.borrow().get_name_by_id(main_module).unwrap();
        return Err(ModuleError::Concrete(
          ModuleConcreteError::MainModuleAlreadyExists {
            main_module: main_name.to_string(),
            new_module: name.to_string(),
          },
        ));
      }
    }

    let _loading_internal_modules_guard =
      is_internal_module_specifier(name.as_str())
        .then(|| LoadingInternalModulesGuard::new(self));

    let name_str = name.v8_string(scope).unwrap();
    let source_str = source.v8_string(scope).unwrap();
    let host_defined_options = self
      .loader
      .borrow()
      .get_host_defined_options(scope, name.as_str());
    let origin = script_origin(scope, name_str, true, host_defined_options);

    v8::tc_scope!(let tc_scope, scope);

    let (maybe_module, try_store_code_cache) = code_cache_info
      .as_ref()
      .and_then(|code_cache_info| {
        code_cache_info.data.as_ref().map(|cache| {
          let mut source = v8::script_compiler::Source::new_with_cached_data(
            source_str,
            Some(&origin),
            v8::CachedData::new(cache),
          );
          let maybe_module = v8::script_compiler::compile_module2(
            tc_scope,
            &mut source,
            v8::script_compiler::CompileOptions::ConsumeCodeCache,
            v8::script_compiler::NoCacheReason::NoReason,
          );
          // Check if the provided code cache is rejected by V8.
          let rejected = match source.get_cached_data() {
            Some(cached_data) => cached_data.rejected(),
            _ => true,
          };
          (maybe_module, rejected)
        })
      })
      .unwrap_or_else(|| {
        let mut source =
          v8::script_compiler::Source::new(source_str, Some(&origin));
        (
          v8::script_compiler::compile_module(tc_scope, &mut source),
          true,
        )
      });

    if tc_scope.has_caught() {
      assert!(maybe_module.is_none());
      let exception = tc_scope.exception().unwrap();
      let exception = v8::Global::new(tc_scope, exception);
      // TODO(bartlomieju): add a more concrete variant - like `ModuleError::CompileError`?
      return Err(ModuleError::Exception(exception));
    }

    let module = maybe_module.unwrap();

    // V8 does not support creating code caches while also snapshotting,
    // and it's not needed anyway, as the snapshot already contains it.
    if try_store_code_cache
      && !self.will_snapshot
      && let Some(code_cache_info) = code_cache_info.take()
    {
      let unbound_module_script = module.get_unbound_module_script(tc_scope);
      let code_cache =
        unbound_module_script.create_code_cache().ok_or_else(|| {
          ModuleError::Concrete(
            ModuleConcreteError::UnboundModuleScriptCodeCache,
          )
        })?;
      let fut =
        async move { (code_cache_info.ready_callback)(&code_cache).await }
          .boxed_local();
      self.code_cache_ready_futs.push(fut);
    }

    // Extract native source map URL from V8
    let unbound_module_script = module.get_unbound_module_script(tc_scope);
    let source_mapping_url_value =
      unbound_module_script.get_source_mapping_url(tc_scope);
    if !source_mapping_url_value.is_undefined()
      && !source_mapping_url_value.is_null()
    {
      let mut source_mapping_url_buf: [std::mem::MaybeUninit<u8>; 1024] =
        [std::mem::MaybeUninit::uninit(); 1024];
      let source_mapping_url: v8::Local<v8::String> =
        source_mapping_url_value.try_cast().unwrap();
      let source_mapping_url = source_mapping_url
        .to_rust_cow_lossy(tc_scope, &mut source_mapping_url_buf);

      let module_name = name
        .try_clone()
        .unwrap_or_else(|| ModuleName::from(name.as_str().to_string()));

      // Inline (`data:`) maps are stored undecoded — `SourceMapper` parses
      // them on the first stack trace that needs them, if ever. Decoding here
      // used to keep a parsed source map alive for every compiled module.
      let source_map_url = if source_mapping_url.starts_with(DATA_PREFIX) {
        source_mapping_url.into_owned()
      } else if let Ok(module_url) = ModuleSpecifier::parse(name.as_str()) {
        // Resolve external source map URL relative to the module URL
        module_url
          .join(&source_mapping_url)
          .unwrap_or(module_url)
          .to_string()
      } else {
        source_mapping_url.into_owned()
      };

      self
        .source_mapper
        .borrow_mut()
        .add_source_map_url(module_name, source_map_url);
    }

    // TODO(bartlomieju): maybe move to a helper function?
    let module_requests = module.get_module_requests();
    let requests_len = module_requests.length();
    let mut requests = Vec::with_capacity(requests_len);
    for i in 0..module_requests.length() {
      let module_request = v8::Local::<v8::ModuleRequest>::try_from(
        module_requests.get(tc_scope, i).unwrap(),
      )
      .unwrap();
      let mut import_specifier_buf: [std::mem::MaybeUninit<u8>; 1024] =
        [std::mem::MaybeUninit::uninit(); 1024];
      let import_specifier = module_request
        .get_specifier()
        .to_rust_cow_lossy(tc_scope, &mut import_specifier_buf);

      let import_attributes = module_request.get_import_attributes();

      let attributes = parse_import_attributes(
        tc_scope,
        import_attributes,
        ImportAttributesKind::StaticImport,
      );

      // FIXME(bartomieju): there are no stack frames if exception
      // is thrown here
      {
        let state = JsRuntime::state_from(tc_scope);
        if let Some(validate_import_attributes_cb) =
          &state.validate_import_attributes_cb
        {
          let location = module
            .source_offset_to_location(module_request.get_source_offset());
          let context = ImportAttributesContext {
            referrer: name.as_str().to_string(),
            specifier: import_specifier.to_string(),
            // V8 reports 0-based line/column; report them 1-based.
            line_number: Some(location.get_line_number() as u32 + 1),
            column_number: Some(location.get_column_number() as u32 + 1),
          };
          (validate_import_attributes_cb)(tc_scope, &attributes, &context);
        }
      }

      if tc_scope.has_caught() {
        let exception = tc_scope.exception().unwrap();
        let exception = v8::Global::new(tc_scope, exception);
        return Err(ModuleError::Exception(exception));
      }

      let resolve_kind = if is_dynamic_import {
        ResolutionKind::DynamicImport
      } else {
        ResolutionKind::Import
      };
      let module_specifier = match self.resolve_with_scope(
        tc_scope,
        &import_specifier,
        name.as_ref(),
        resolve_kind,
        &attributes,
      ) {
        Ok(s) => s,
        Err(e) => {
          // Fall back to lazy ESM sources for bare internal specifiers (e.g.
          // `node:_http_common` from `node:_http_outgoing`) that the
          // user-facing loader doesn't know about. If the specifier matches
          // a registered lazy ESM entry, use it verbatim.
          if self.has_lazy_esm_source(&import_specifier)
            && let Ok(parsed) = ModuleSpecifier::parse(&import_specifier)
          {
            parsed
          } else {
            return Err(ModuleError::Core(e.into()));
          }
        }
      };
      let requested_module_type =
        get_requested_module_type_from_attributes(&attributes);
      let referrer_source_offset = if let ModuleType::Wasm = module_type {
        // Wasm sources will have been rendered to synthetic JS modules, so any
        // `ModuleRequest::referrer:source_offset`s we get from v8 are not
        // applicable to user code. Disregard it.
        None
      } else {
        Some(module_request.get_source_offset())
      };
      if crate::modules::import_graph::is_enabled() {
        crate::modules::import_graph::record_esm_import(
          name.as_ref(),
          module_specifier.as_str(),
        );
      }
      let request = ModuleRequest {
        reference: ModuleReference {
          specifier: module_specifier,
          requested_module_type,
        },
        specifier_key: Some(import_specifier.into_owned()),
        referrer_source_offset,
        phase: match module_request.get_phase() {
          v8::ModuleImportPhase::kEvaluation => ModuleImportPhase::Evaluation,
          v8::ModuleImportPhase::kSource => ModuleImportPhase::Source,
          v8::ModuleImportPhase::kDefer => ModuleImportPhase::Defer,
        },
      };
      requests.push(request);
    }

    let handle = v8::Global::<v8::Module>::new(tc_scope, module);
    let id = self.data.borrow_mut().create_module_info(
      name,
      module_type,
      handle,
      main,
      requests,
    );
    Ok(NewModuleResult::Ready(id))
  }

  pub(crate) fn new_json_module(
    &self,
    scope: &mut v8::PinScope,
    name: impl IntoModuleName,
    code: impl IntoModuleCodeString,
  ) -> Result<ModuleId, ModuleError> {
    let name = name.into_module_name();
    let code = code.into_module_code();
    let source_str = v8::String::new_from_utf8(
      scope,
      strip_bom(code.as_bytes()),
      v8::NewStringType::Normal,
    )
    .unwrap();
    v8::tc_scope!(let tc_scope, scope);

    let parsed_json = match v8::json::parse(tc_scope, source_str) {
      Some(parsed_json) => parsed_json,
      None => {
        assert!(tc_scope.has_caught());
        let exception = tc_scope.exception().unwrap();
        let exception = v8::Global::new(tc_scope, exception);
        return Err(ModuleError::Exception(exception));
      }
    };
    let exports = vec![(ascii_str!("default"), parsed_json)];
    Ok(self.new_synthetic_module(tc_scope, name, ModuleType::Json, exports))
  }

  #[allow(
    clippy::unnecessary_wraps,
    reason = "consistent return type with other module constructors"
  )]
  pub(crate) fn new_text_module(
    &self,
    scope: &mut v8::PinScope,
    name: impl IntoModuleName,
    code: impl IntoModuleCodeString,
  ) -> Result<ModuleId, ModuleError> {
    let name = name.into_module_name();
    let code = code.into_module_code();
    // TODO(bartlomieju): would be much better if the string was ensured to not contain
    // BOM, then we could use a more efficient string type with `FastString::v8_string`.
    let source_str = v8::String::new_from_utf8(
      scope,
      strip_bom(code.as_bytes()),
      v8::NewStringType::Normal,
    )
    .unwrap();
    let source_str_local = v8::Local::new(scope, source_str);
    let source_value_local = v8::Local::<v8::Value>::from(source_str_local);
    let exports = vec![(ascii_str!("default"), source_value_local)];
    Ok(self.new_synthetic_module(scope, name, ModuleType::Text, exports))
  }

  #[allow(
    clippy::unnecessary_wraps,
    reason = "consistent return type with other module constructors"
  )]
  pub(crate) fn new_bytes_module(
    &self,
    scope: &mut v8::PinScope,
    name: impl IntoModuleName,
    code: ModuleCodeBytes,
  ) -> Result<ModuleId, ModuleError> {
    let name = name.into_module_name();
    let (buf_len, backing_store) = match code {
      ModuleCodeBytes::Static(bytes) => (
        bytes.len(),
        v8::ArrayBuffer::new_backing_store_from_vec(bytes.to_vec()),
      ),
      ModuleCodeBytes::Boxed(bytes) => (
        bytes.len(),
        v8::ArrayBuffer::new_backing_store_from_boxed_slice(bytes),
      ),
      ModuleCodeBytes::Arc(bytes) => (
        bytes.len(),
        v8::ArrayBuffer::new_backing_store_from_vec(bytes.to_vec()),
      ),
    };
    let backing_store_shared = backing_store.make_shared();
    let ab = v8::ArrayBuffer::with_backing_store(scope, &backing_store_shared);
    let uint8_array = v8::Uint8Array::new(scope, ab, 0, buf_len).unwrap();
    let value: v8::Local<v8::Value> = uint8_array.into();
    let exports = vec![(ascii_str!("default"), value)];
    Ok(self.new_synthetic_module(scope, name, ModuleType::Bytes, exports))
  }

  pub(crate) fn instantiate_module<'s, 'i>(
    &self,
    scope: &mut v8::PinScope<'s, 'i>,
    id: ModuleId,
  ) -> Result<(), v8::Global<v8::Value>> {
    v8::tc_scope!(let tc_scope, scope);

    let module = self
      .get_handle(id)
      .map(|handle| v8::Local::new(tc_scope, handle))
      .expect("ModuleInfo not found");

    if module.get_status() == v8::ModuleStatus::Errored {
      return Err(v8::Global::new(tc_scope, module.get_exception()));
    }

    // FIXME: instantiate_module is called more than it should be,
    // especially for dynamic imports. As a hack, bail out if the
    // module status is already being instantiated.
    if module.get_status() != v8::ModuleStatus::Uninstantiated {
      return Ok(());
    }

    // `is_internal` is computed once at registration (see
    // `ModuleMapData::create_module_info`); this used to clone the module name
    // and `Url::parse` it on every instantiation.
    let is_internal = self
      .data
      .borrow()
      .info
      .get(id)
      .is_some_and(|info| info.is_internal);
    let _loading_internal_modules_guard =
      is_internal.then(|| LoadingInternalModulesGuard::new(self));

    tc_scope.set_slot(self as *const _);
    let instantiate_result = module.instantiate_module2(
      tc_scope,
      Self::module_resolve_callback,
      Self::module_source_callback,
    );
    tc_scope.remove_slot::<*const Self>();
    if instantiate_result.is_none() {
      let exception = tc_scope.exception().unwrap();
      return Err(v8::Global::new(tc_scope, exception));
    }

    self.drop_instantiated_requests(tc_scope, id);

    Ok(())
  }

  /// Frees the import edges of every module in a just-instantiated graph.
  ///
  /// `ModuleInfo::requests` has exactly three consumers, and none of them can
  /// run for a module that is already linked:
  ///
  /// - `module_resolve_callback` / `module_source_callback`, which V8 only
  ///   invokes while *instantiating* a module (V8 skips modules that are
  ///   already linked when instantiating a later graph that imports them, and
  ///   a failed instantiation only resets modules that were still linking);
  /// - `RecursiveModuleLoad::register_and_recurse_inner`, which walks the
  ///   edges of an already-registered module purely to make sure the subgraph
  ///   is registered — and a linked module necessarily has its whole subgraph
  ///   registered already.
  ///
  /// This is the same argument `ModuleMapData::serialize_for_snapshotting`
  /// makes for dropping the edges of snapshot-instantiated modules; here we
  /// apply it at runtime, where the edges (a `Url` plus a specifier string per
  /// import) were previously retained for the life of the process.
  ///
  /// "Already linked" is checked against V8 rather than assumed: this runs
  /// re-entrantly. `instantiate_module` bails out when a module is already
  /// being instantiated (see the FIXME above), and `lazy_load_esm_module`
  /// instantiates a module from *inside* a sibling's resolve callback, so an
  /// inner graph can overlap an outer graph that V8 is still linking. Taking
  /// the edges of a module still in `Instantiating` would pull them out from
  /// under the in-flight `module_resolve_callback`/`module_source_callback`.
  /// Skipping a node also stops the walk there, which only costs retention.
  fn drop_instantiated_requests<'s, 'i>(
    &self,
    scope: &mut v8::PinScope<'s, 'i>,
    root: ModuleId,
  ) {
    let mut data = self.data.borrow_mut();
    if data.info.get(root).is_none_or(|i| i.requests.is_empty()) {
      return;
    }
    let mut stack = vec![root];
    let mut seen = HashSet::with_capacity(16);
    while let Some(id) = stack.pop() {
      if !seen.insert(id) {
        continue;
      }
      // Anything below `Instantiated` may still have its edges read by V8.
      // `Errored` is reachable only from a failed *evaluation* — a failed
      // instantiation resets the graph to `Uninstantiated` — so those modules
      // are linked and will never be resolved again.
      let status = match data.handles.get(id) {
        Some(handle) => v8::Local::new(scope, handle).get_status(),
        None => continue,
      };
      if matches!(
        status,
        v8::ModuleStatus::Uninstantiated | v8::ModuleStatus::Instantiating
      ) {
        continue;
      }
      let Some(info) = data.info.get_mut(id) else {
        continue;
      };
      let requests = std::mem::take(&mut info.requests);
      if requests.is_empty() {
        continue;
      }
      #[cfg(test)]
      data.dropped_requests.insert(id, requests.clone());
      for request in &requests {
        if let Some(child) = data.get_id(
          request.reference.specifier.as_str(),
          &request.reference.requested_module_type,
        ) {
          stack.push(child);
        }
      }
    }
  }

  /// Called by V8 during `JsRuntime::instantiate_module`. This is only used internally, so we use the Isolate's annex
  /// to propagate a &Self.
  fn module_resolve_callback<'s>(
    context: v8::Local<'s, v8::Context>,
    specifier: v8::Local<'s, v8::String>,
    import_attributes: v8::Local<'s, v8::FixedArray>,
    referrer: v8::Local<'s, v8::Module>,
  ) -> Option<v8::Local<'s, v8::Module>> {
    // SAFETY: `CallbackScope` can be safely constructed from `Local<Context>`
    v8::callback_scope!(unsafe scope, context);

    let module_map =
      // SAFETY: We retrieve the pointer from the slot, having just set it a few stack frames up
      unsafe { scope.get_slot::<*const Self>().unwrap().as_ref().unwrap() };

    let referrer_global = v8::Global::new(scope, referrer);

    let referrer_name = module_map
      .data
      .borrow()
      .get_name_by_module(&referrer_global)
      .expect("ModuleInfo not found");

    let mut specifier_buf: [std::mem::MaybeUninit<u8>; 1024] =
      [std::mem::MaybeUninit::uninit(); 1024];
    let specifier_str = specifier.to_rust_cow_lossy(scope, &mut specifier_buf);

    let attributes = parse_import_attributes(
      scope,
      import_attributes,
      ImportAttributesKind::StaticImport,
    );
    let requested_module_type =
      get_requested_module_type_from_attributes(&attributes);
    let pre_resolved_specifier = {
      let module_map_data = module_map.data.borrow();
      let referrer_info = module_map_data
        .get_info_by_module(&referrer_global)
        .expect("ModuleInfo not found");
      referrer_info
        .requests
        .iter()
        .find(|r| {
          r.specifier_key
            .as_ref()
            .is_some_and(|s| s == &specifier_str)
            && r.reference.requested_module_type == requested_module_type
        })
        .map(|r| r.reference.specifier.clone())
    };
    let maybe_module = module_map.resolve_callback(
      scope,
      &specifier_str,
      &referrer_name,
      attributes,
      pre_resolved_specifier,
    );
    if let Some(module) = maybe_module {
      return Some(module);
    }

    crate::error::throw_js_error_class(
      scope,
      &JsErrorBox::type_error(format!(
        r#"Cannot resolve module "{specifier_str}" from "{referrer_name}""#
      )),
    );
    None
  }

  fn module_source_callback<'s>(
    context: v8::Local<'s, v8::Context>,
    specifier: v8::Local<'s, v8::String>,
    import_attributes: v8::Local<'s, v8::FixedArray>,
    referrer: v8::Local<'s, v8::Module>,
  ) -> Option<v8::Local<'s, v8::Object>> {
    // SAFETY: `CallbackScope` can be safely constructed from `Local<Context>`
    v8::callback_scope!(unsafe scope, context);

    let module_map =
      // SAFETY: We retrieve the pointer from the slot, having just set it a few stack frames up
      unsafe { scope.get_slot::<*const Self>().unwrap().as_ref().unwrap() };

    let mut specifier_buf: [std::mem::MaybeUninit<u8>; 1024] =
      [std::mem::MaybeUninit::uninit(); 1024];
    let specifier_str = specifier.to_rust_cow_lossy(scope, &mut specifier_buf);
    let referrer_global = v8::Global::new(scope, referrer);
    let attributes = parse_import_attributes(
      scope,
      import_attributes,
      ImportAttributesKind::StaticImport,
    );
    let requested_module_type =
      get_requested_module_type_from_attributes(&attributes);
    // A missing edge should be impossible — V8 only asks about modules it is
    // linking, and `drop_instantiated_requests` leaves those alone. Report it
    // as a JS error rather than aborting the process if that ever stops
    // holding.
    let module_reference = {
      let module_map_data = module_map.data.borrow();
      let referrer_info = module_map_data
        .get_info_by_module(&referrer_global)
        .expect("ModuleInfo not found");
      referrer_info
        .requests
        .iter()
        .find(|r| {
          r.specifier_key
            .as_ref()
            .is_some_and(|s| s == &specifier_str)
            && r.reference.requested_module_type == requested_module_type
        })
        .map(|module_request| module_request.reference.clone())
    };
    let source = module_reference.and_then(|module_reference| {
      let key = ModuleSourceKey::from_reference(&module_reference);
      module_map.data.borrow().sources.get(&key).cloned()
    });
    if let Some(source) = source {
      return Some(v8::Local::new(scope, source));
    }

    let message = v8::String::new(
      scope,
      &format!(r#"Module source can not be imported for "{specifier_str}""#),
    )
    .unwrap();
    let exception = v8::Exception::reference_error(scope, message);
    scope.throw_exception(exception);
    None
  }

  /// Resolve provided module. This function calls out to `loader.resolve`,
  /// but applies some additional checks that disallow resolving/importing
  /// certain modules (eg. `ext:` or `node:` modules).
  ///
  pub fn resolve(
    &self,
    specifier: &str,
    referrer: &str,
    kind: ResolutionKind,
  ) -> ModuleResolveResponse {
    if let Some(resolved_specifier) =
      self.maybe_resolve_internal_import(specifier, referrer)
    {
      return Ok(resolved_specifier);
    }
    let resolved_specifier =
      self.loader.borrow().resolve(specifier, referrer, kind)?;
    self.validate_ext_module_import(&resolved_specifier, referrer)?;
    Ok(resolved_specifier)
  }

  pub fn resolve_with_scope(
    &self,
    scope: &mut v8::PinScope,
    specifier: &str,
    referrer: &str,
    kind: ResolutionKind,
    import_attributes: &HashMap<String, String>,
  ) -> ModuleResolveResponse {
    if let Some(resolved_specifier) =
      self.maybe_resolve_internal_import(specifier, referrer)
    {
      return Ok(resolved_specifier);
    }
    let resolved_specifier = self.loader.borrow().resolve_with_scope(
      scope,
      specifier,
      referrer,
      kind,
      import_attributes,
    )?;
    self.validate_ext_module_import(&resolved_specifier, referrer)?;
    Ok(resolved_specifier)
  }

  /// Resolves an internal specifier imported by an internal module without
  /// consulting the loader. Returns `None` when this isn't such an import, in
  /// which case the loader decides.
  ///
  /// Internal modules that aren't baked into the snapshot get instantiated at
  /// runtime, at which point the installed loader is the embedder's
  /// user-facing one. In Deno that loader applies the user's import map, so an
  /// entry like `{ "ext:core/mod.js": "./mod.js" }` used to rewrite the imports
  /// of internal modules such as `ext:cli/40_test_common.js` or
  /// `node:_http_agent`, breaking instantiation. Internal specifiers are owned
  /// by the runtime and always resolve to themselves.
  ///
  /// This trusts the referrer only as far as `validate_ext_module_import`
  /// does — a `node:`-looking referrer that user code made up (e.g. via
  /// `node:vm`'s `filename` option) isn't enough on its own.
  fn maybe_resolve_internal_import(
    &self,
    specifier: &str,
    referrer: &str,
  ) -> Option<ModuleSpecifier> {
    if !self.is_internal_referrer(referrer) {
      return None;
    }
    let specifier = ModuleSpecifier::parse(specifier).ok()?;
    is_internal_scheme(specifier.scheme()).then_some(specifier)
  }

  fn validate_ext_module_import(
    &self,
    resolved_specifier: &ModuleSpecifier,
    referrer: &str,
  ) -> Result<(), JsErrorBox> {
    if resolved_specifier.scheme() != "ext" {
      return Ok(());
    }

    if (self.will_snapshot || self.loading_internal_modules.get())
      && referrer == "."
    {
      return Ok(());
    }

    if self.is_internal_referrer(referrer) {
      return Ok(());
    }

    let referrer = if referrer.is_empty() {
      "(no referrer)"
    } else {
      referrer
    };
    let msg = format!(
      "Importing ext: modules is only allowed from ext: and node: modules. Tried to import {} from {}",
      resolved_specifier, referrer
    );
    Err(JsErrorBox::type_error(msg))
  }

  fn is_internal_referrer(&self, referrer: &str) -> bool {
    if !is_internal_module_specifier(referrer) {
      return false;
    }
    self.will_snapshot || self.loading_internal_modules.get()
  }

  /// Called by `module_resolve_callback` during module instantiation.
  fn resolve_callback<'s, 'i>(
    &self,
    scope: &mut v8::PinScope<'s, 'i>,
    specifier: &str,
    referrer: &str,
    import_attributes: HashMap<String, String>,
    pre_resolved_specifier: Option<ModuleSpecifier>,
  ) -> Option<v8::Local<'s, v8::Module>> {
    // Synthetic ESM dispatch first, by raw specifier. The active loader
    // may not know about the spec (e.g. `LazyEsmModuleLoader` only
    // resolves `lazy_loaded_esm` entries), so checking before
    // `resolve_sync` ensures the synthetic dispatch wins over a loader
    // "cannot resolve" error. `node:foo` specifiers are their own
    // canonical form, so no further resolution is needed.
    if self.has_synthetic_esm_module(specifier) {
      if let Some(id) = self.get_id(specifier, &RequestedModuleType::None)
        && let Some(handle) = self.get_handle(id)
      {
        return Some(v8::Local::new(scope, handle));
      }
      if let Some(module) = self.try_resolve_synthetic_esm(scope, specifier) {
        return Some(module);
      }
    }

    let module_type =
      get_requested_module_type_from_attributes(&import_attributes);
    let resolved_specifier = match pre_resolved_specifier {
      Some(specifier) => specifier,
      None => match self.resolve_with_scope(
        scope,
        specifier,
        referrer,
        ResolutionKind::Import,
        &import_attributes,
      ) {
        Ok(s) => s,
        Err(e) => {
          // Fall back to lazy ESM sources for bare internal specifiers like
          // `node:_http_common` that the runtime's user-facing loader
          // doesn't know how to resolve (only public `node:` modules go
          // through the normal path). The lazy_esm registry has them by
          // exact specifier, so if the specifier is a registered lazy ESM
          // entry, use it verbatim instead of erroring.
          if self.has_lazy_esm_source(specifier)
            && let Ok(parsed) = ModuleSpecifier::parse(specifier)
          {
            parsed
          } else {
            crate::error::throw_js_error_class(scope, &e);
            return None;
          }
        }
      },
    };

    if let Some(id) = self.get_id(resolved_specifier.as_str(), module_type)
      && let Some(handle) = self.get_handle(id)
    {
      return Some(v8::Local::new(scope, handle));
    }

    // Synthetic ESM dispatch (post-resolve): in case the loader returned
    // a redirected/normalized form, also check here. Most callers hit
    // the pre-resolve branch above.
    if let Some(module) =
      self.try_resolve_synthetic_esm(scope, resolved_specifier.as_str())
    {
      return Some(module);
    }

    // Fallback: check lazy-loaded ESM sources (modules embedded in the
    // binary but not included in the snapshot).
    let maybe_source = self.take_lazy_esm_source(resolved_specifier.as_str());
    if let Some(source_code) = maybe_source {
      match self.new_es_module(
        scope,
        false,
        resolved_specifier.into(),
        source_code,
        false,
        None,
      ) {
        Ok(mod_id) => {
          if let Some(handle) = self.get_handle(mod_id) {
            return Some(v8::Local::new(scope, handle));
          }
        }
        Err(e) => {
          let err = e.into_error(scope, false, true);
          crate::error::throw_js_error_class(scope, &err);
          return None;
        }
      }
    }

    None
  }

  /// Borrows the import edges of a module. The returned guard keeps the
  /// module map's `RefCell` borrowed for reads — callers must not try to
  /// mutate the map while holding it.
  pub(crate) fn get_requested_modules(
    &self,
    id: ModuleId,
  ) -> Option<Ref<'_, [ModuleRequest]>> {
    Ref::filter_map(self.data.borrow(), |d| {
      d.info.get(id).map(|i| i.requests.as_slice())
    })
    .ok()
  }

  /// Owned copy of a module's import edges, for assertions in tests. Falls
  /// back to the edges recorded before instantiation dropped them.
  #[cfg(test)]
  pub(crate) fn get_requested_modules_cloned(
    &self,
    id: ModuleId,
  ) -> Option<Vec<ModuleRequest>> {
    let data = self.data.borrow();
    let requests = data.info.get(id).map(|i| i.requests.clone())?;
    if requests.is_empty()
      && let Some(dropped) = data.dropped_requests.get(&id)
    {
      return Some(dropped.clone());
    }
    Some(requests)
  }

  /// Returns the module's registered (canonical, post-redirect) specifier,
  /// parsed, but only when it differs from `specifier`. Lets callers that
  /// already hold the requested specifier skip both the string copy and the
  /// `Url::parse` in the common non-redirect case.
  pub(crate) fn canonical_specifier_if_different(
    &self,
    id: ModuleId,
    specifier: &ModuleSpecifier,
  ) -> Option<ModuleSpecifier> {
    let data = self.data.borrow();
    let name = data.info.get(id).map(|i| i.name.as_str())?;
    if name == specifier.as_str() {
      return None;
    }
    ModuleSpecifier::parse(name).ok()
  }

  /// Drain all ready code-cache futures.
  fn drain_code_cache_ready(&self, cx: &mut Context) {
    if !self.code_cache_ready_futs.is_pending() {
      return;
    }

    while let Poll::Ready(Some(_)) =
      self.code_cache_ready_futs.poll_next_unpin(cx)
    {}
  }

  pub(crate) fn get_module<'s, 'i>(
    &self,
    scope: &v8::PinScope<'s, 'i>,
    module_id: ModuleId,
  ) -> Option<v8::Local<'s, v8::Module>> {
    self
      .data
      .borrow()
      .get_handle(module_id)
      .map(|g| v8::Local::new(scope, g))
  }

  /// Returns the namespace object of a module.
  ///
  /// This is only available after module evaluation has completed.
  /// This function panics if module has not been instantiated.
  pub fn get_module_namespace(
    &self,
    scope: &mut v8::PinScope,
    module_id: ModuleId,
  ) -> Result<v8::Global<v8::Object>, CoreError> {
    let module_handle = self
      .data
      .borrow()
      .get_handle(module_id)
      .expect("ModuleInfo not found");

    let module = module_handle.open(scope);

    if module.get_status() == v8::ModuleStatus::Errored {
      let exception = module.get_exception();
      return exception_to_err_result(scope, exception, false, false)
        .map_err(|e| CoreErrorKind::Js(e).into_box());
    }

    assert!(matches!(
      module.get_status(),
      v8::ModuleStatus::Instantiated | v8::ModuleStatus::Evaluated
    ));

    let module_namespace: v8::Local<v8::Object> =
      v8::Local::try_from(module.get_module_namespace())?;

    Ok(v8::Global::new(scope, module_namespace))
  }
}

// Clippy thinks the return value doesn't need to be an Option, it's unaware
// of the mapping that MapFnFrom<F> does for ResolveModuleCallback.
#[allow(
  clippy::unnecessary_wraps,
  reason = "required by MapFnFrom<F> for ResolveModuleCallback"
)]
pub(crate) fn synthetic_module_evaluation_steps<'s>(
  context: v8::Local<'s, v8::Context>,
  module: v8::Local<'s, v8::Module>,
) -> Option<v8::Local<'s, v8::Value>> {
  // SAFETY: `CallbackScope` can be safely constructed from `Local<Context>`
  v8::callback_scope!(unsafe scope, context);
  v8::tc_scope!(tc_scope, scope);

  let module_map = JsRealm::module_map_from(tc_scope);

  let handle = v8::Global::<v8::Module>::new(tc_scope, module);
  let exports = module_map
    .data
    .borrow_mut()
    .synthetic_module_exports_store
    .remove(&handle)
    .unwrap();

  for (export_name, export_value) in exports {
    let name = v8::Local::new(tc_scope, export_name);
    let value = v8::Local::new(tc_scope, export_value);

    // This should never fail
    assert!(
      module
        .set_synthetic_module_export(tc_scope, name, value)
        .unwrap()
    );
    assert!(!tc_scope.has_caught());
  }

  // Since Top-Level Await is active we need to return a promise.
  // This promise is resolved immediately.
  let resolver = v8::PromiseResolver::new(tc_scope).unwrap();
  let undefined = v8::undefined(tc_scope);
  resolver.resolve(tc_scope, undefined.into());
  Some(resolver.get_promise(tc_scope).into())
}

pub fn script_origin<'s, 'i>(
  s: &mut v8::PinScope<'s, 'i>,
  resource_name: v8::Local<'s, v8::String>,
  is_module: bool,
  host_defined_options: Option<v8::Local<'s, v8::Data>>,
) -> v8::ScriptOrigin<'s> {
  v8::ScriptOrigin::new(
    s,
    resource_name.into(),
    0,
    0,
    false,
    0,
    None,
    false,
    false,
    is_module,
    host_defined_options,
  )
}
