// Copyright 2018-2026 the Deno authors. MIT license.

use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::HashMap;
use std::collections::HashSet;
use std::rc::Rc;

use super::RequestedModuleType;
use crate::ModuleCodeString;
use crate::ModuleSource;
use crate::fast_string::FastString;
use crate::modules::ModuleId;
use crate::modules::ModuleInfo;
use crate::modules::ModuleLoadId;
use crate::modules::ModuleName;
use crate::modules::ModuleReference;
use crate::modules::ModuleRequest;
use crate::modules::ModuleType;
use crate::runtime::SnapshotDataId;
use crate::runtime::SnapshotLoadDataStore;
use crate::runtime::SnapshotStoreDataStore;
use crate::snapshot_format::Decoder;
use crate::snapshot_format::Encoder;
use crate::snapshot_format::SnapshotResult;

/// A symbolic module entity.
#[derive(Debug, PartialEq)]
pub(crate) enum SymbolicModule {
  /// This module is an alias to another module.
  /// This is useful such that multiple names could point to
  /// the same underlying module (particularly due to redirects).
  Alias(ModuleName),
  /// This module associates with a V8 module by id.
  Mod(ModuleId),
}

/// Map of [`ModuleName`] and [`RequestedModuleType`] to a data field.
struct ModuleNameTypeMap<T> {
  submaps: Vec<HashMap<ModuleName, T>>,
  map_index: HashMap<RequestedModuleType, usize>,
  len: usize,
}

impl<T> Default for ModuleNameTypeMap<T> {
  fn default() -> Self {
    Self {
      submaps: Default::default(),
      map_index: Default::default(),
      len: 0,
    }
  }
}

impl<T> ModuleNameTypeMap<T> {
  pub fn len(&self) -> usize {
    self.len
  }

  fn map_index(&self, ty: &RequestedModuleType) -> Option<usize> {
    self.map_index.get(ty).copied()
  }

  pub fn get<Q>(&self, ty: &RequestedModuleType, name: &Q) -> Option<&T>
  where
    ModuleName: std::borrow::Borrow<Q>,
    Q: std::cmp::Eq + std::hash::Hash + std::fmt::Debug + ?Sized,
  {
    let index = self.map_index(ty)?;
    let map = self.submaps.get(index)?;
    map.get(name)
  }

  pub fn insert(
    &mut self,
    module_type: &RequestedModuleType,
    name: FastString,
    module: T,
  ) {
    let index = match self.map_index(module_type) {
      Some(index) => index,
      None => {
        let index = self.submaps.len();
        self.map_index.insert(module_type.clone(), index);
        self.submaps.push(Default::default());
        index
      }
    };

    if self
      .submaps
      .get_mut(index)
      .unwrap()
      .insert(name, module)
      .is_none()
    {
      self.len += 1;
    }
  }

  /// Rather than providing an iterator, we provide a drain method. This is mainly because Rust
  /// doesn't have generators.
  pub fn drain(
    mut self,
    mut f: impl FnMut(usize, &RequestedModuleType, ModuleName, T),
  ) {
    let mut i = 0;
    for (ty, value) in self.map_index.into_iter() {
      for (key, value) in self.submaps.get_mut(value).unwrap().drain() {
        f(i, &ty, key, value);
        i += 1;
      }
    }
  }
}

/// An array of tuples that provide module exports.
///
/// "default" name will make the export "default" - ie. one that can be imported
/// with `import foo from "./virtual.js"`;.
/// All other name provide "named exports" - ie. ones that can be imported like
/// so: `import { name1, name2 } from "./virtual.js`.
pub(crate) type SyntheticModuleExports =
  Vec<(v8::Global<v8::String>, v8::Global<v8::Value>)>;

// TODO(bartlomieju): add an assertion that checks for that assumption?
// If it's true we can simplify the type to be an `Option` instead of a `HashMap`.
/// This hash map is not expected to hold more than one element at a time.
/// It is a temporary store, so we can forward data to
/// `synthetic_module_evaluation_steps` callback.
pub(crate) type SyntheticModuleExportsStore =
  HashMap<v8::Global<v8::Module>, SyntheticModuleExports>;

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub(crate) enum ModuleSourceKind {
  Wasm,
}

impl ModuleSourceKind {
  pub fn from_module_type(module_type: &ModuleType) -> Option<Self> {
    match module_type {
      ModuleType::Wasm => Some(Self::Wasm),
      _ => None,
    }
  }
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
enum ModuleSourceType {
  Loaded(ModuleType),
  Requested(RequestedModuleType),
}

#[derive(Debug, Eq, PartialEq, Hash)]
pub(crate) struct ModuleSourceKey {
  pub name: ModuleName,
  typ: ModuleSourceType,
}

impl ModuleSourceKey {
  pub fn from_reference(module_reference: &ModuleReference) -> Self {
    Self {
      name: module_reference.specifier.to_string().into(),
      typ: ModuleSourceType::Requested(
        module_reference.requested_module_type.clone(),
      ),
    }
  }

  pub fn from_loaded_source(loaded_source: &mut ModuleSource) -> Self {
    let name = if let Some(module_url_found) =
      loaded_source.cheap_copy_module_url_found()
    {
      module_url_found
    } else {
      loaded_source.cheap_copy_module_url_specified()
    };
    Self {
      name,
      typ: ModuleSourceType::Loaded(loaded_source.module_type.clone()),
    }
  }
}

#[derive(Default)]
pub(crate) struct ModuleMapData {
  /// Inverted index from module to index in `info`.
  pub(crate) handles_inverted: HashMap<v8::Global<v8::Module>, usize>,
  /// The handles we have loaded so far, corresponding with the [`ModuleInfo`] in `info`.
  pub(crate) handles: Vec<v8::Global<v8::Module>>,
  pub(crate) main_module_callbacks: Vec<v8::Global<v8::Function>>,
  /// The modules we have loaded so far.
  pub(crate) info: Vec<ModuleInfo>,
  /// [`ModuleName`] to [`SymbolicModule`] for modules.
  by_name: ModuleNameTypeMap<SymbolicModule>,
  /// The next ID used for a load.
  pub(crate) next_load_id: ModuleLoadId,
  /// If a main module has been loaded, points to it by index.
  pub(crate) main_module_id: Option<ModuleId>,
  /// This store is used to temporarily store data that is used
  /// to evaluate a "synthetic module".
  pub(crate) synthetic_module_exports_store: SyntheticModuleExportsStore,
  pub(crate) lazy_esm_sources:
    Rc<RefCell<HashMap<ModuleName, ModuleCodeString>>>,
  pub(crate) residual_lazy_esm_sources:
    &'static [(&'static str, &'static str)],
  /// Specifiers of lazy-loaded ESM modules known to exist (survives
  /// snapshotting). Used to check if a module should be loaded from
  /// `lazy_esm_sources` without going through the external module loader.
  /// `Cow` so that specifiers restored from a snapshot can borrow the snapshot
  /// blob instead of being copied onto the heap for every isolate.
  pub(crate) known_lazy_esm: RefCell<HashSet<Cow<'static, str>>>,
  pub(crate) lazy_script_sources:
    Rc<RefCell<HashMap<ModuleName, ModuleCodeString>>>,
  pub(crate) residual_lazy_script_sources:
    &'static [(&'static str, &'static str)],
  /// Results of `load_ext_script` evaluations. Populated on first
  /// evaluation so later callers (`Deno.core.loadExtScript()` from JS,
  /// the `synthetic_esm` dispatch from Rust) share a single evaluated
  /// exports value — the source is removed from `lazy_script_sources`
  /// on first eval, so subsequent reads come from here. Runtime-only —
  /// not snapshotted.
  pub(crate) loaded_script_results:
    Rc<RefCell<HashMap<ModuleName, v8::Global<v8::Value>>>>,
  /// `synthetic_esm` registry: module specifier (e.g. `node:worker_threads`)
  /// to backing-script specifier (e.g. `ext:deno_node/worker_threads.ts`).
  /// Populated at extension init from each extension's
  /// `synthetic_esm_files` list. Runtime-only — not snapshotted.
  pub(crate) synthetic_esm_modules:
    Rc<RefCell<HashMap<ModuleName, ModuleName>>>,
  /// Set of scripts currently being loaded (for circular dep detection).
  pub(crate) lazy_script_loading: Rc<RefCell<HashSet<ModuleName>>>,
  /// Snapshot-time `__bootstrap` view (frozen clone of `core.ops` etc.)
  /// registered from JS via `op_set_captured_bootstrap`. `load_ext_script`
  /// temporarily installs this on `globalThis.__bootstrap` for the duration
  /// of each script evaluation if `__bootstrap` isn't already on the global
  /// — every lazy_loaded_js polyfill's IIFE preamble destructures it, and
  /// the `synthetic_esm` dispatch goes straight through Rust without the
  /// JS `core.loadExtScript` wrapper. Runtime-only — not snapshotted.
  pub(crate) captured_bootstrap: RefCell<Option<v8::Global<v8::Value>>>,
  pub(crate) sources: HashMap<ModuleSourceKey, v8::Global<v8::Object>>,
  /// Specifiers of `lazy_loaded_esm` / `lazy_loaded_js` files whose source
  /// was actually compiled by V8 during snapshot creation. Their bytes live
  /// in the snapshot blob; the binary does not need a separate copy.
  pub(crate) consumed_lazy_specifiers: RefCell<HashSet<String>>,
}

/// Snapshot-compatible representation of this data.
#[derive(Default)]
pub(crate) struct ModuleMapSnapshotData {
  next_load_id: i32,
  main_module_id: Option<i32>,
  modules: Vec<ModuleInfo>,
  module_handles: Vec<SnapshotDataId>,
  main_module_callbacks: Vec<SnapshotDataId>,
  by_name: Vec<(FastString, RequestedModuleType, SymbolicModule)>,
  /// Specifiers of lazy-loaded ESM modules that are known to exist but
  /// are not compiled/instantiated in the snapshot. They will be loaded
  /// from the binary on first access at runtime.
  lazy_esm_specifiers: Vec<Cow<'static, str>>,
  /// `load_ext_script` cache snapshot. Captures the exports object
  /// returned by each polyfill IIFE evaluated at snapshot time so the
  /// runtime can share the same value (with the `synthetic_esm` dispatch
  /// in particular) without re-evaluating — re-eval would clobber
  /// registered `internals.__*` hooks and duplicate class identities.
  loaded_script_results: Vec<(FastString, SnapshotDataId)>,
  /// Snapshot of the captured `__bootstrap` view registered via
  /// `op_set_captured_bootstrap` at the end of `01_core.js`. Restored
  /// at runtime so the Rust `load_ext_script` can reinstall it on
  /// `globalThis.__bootstrap` during each script evaluation.
  captured_bootstrap: Option<SnapshotDataId>,
}

impl SymbolicModule {
  fn encode(&self, e: &mut Encoder) {
    match self {
      Self::Alias(name) => {
        e.u8(0);
        e.str(name.as_str());
      }
      Self::Mod(id) => {
        e.u8(1);
        e.usize(*id);
      }
    }
  }

  fn decode(d: &mut Decoder) -> SnapshotResult<Self> {
    Ok(match d.u8()? {
      0 => Self::Alias(d.fast_string()?),
      1 => Self::Mod(d.usize()?),
      n => return Decoder::invalid_discriminant("SymbolicModule", n as u32),
    })
  }
}

impl ModuleMapSnapshotData {
  pub(crate) fn encode(&self, e: &mut Encoder) {
    e.i32(self.next_load_id);
    e.option(self.main_module_id, |e, v| e.i32(v));
    e.seq(self.modules.iter(), |e, m| m.encode(e));
    e.seq(self.module_handles.iter(), |e, v| e.u32(*v));
    e.seq(self.main_module_callbacks.iter(), |e, v| e.u32(*v));
    e.seq(self.by_name.iter(), |e, (name, ty, module)| {
      e.str(name.as_str());
      ty.encode(e);
      module.encode(e);
    });
    e.seq(self.lazy_esm_specifiers.iter(), |e, s| e.str(s));
    e.seq(self.loaded_script_results.iter(), |e, (name, id)| {
      e.str(name.as_str());
      e.u32(*id);
    });
    e.option(self.captured_bootstrap, |e, v| e.u32(v));
  }

  pub(crate) fn decode(d: &mut Decoder) -> SnapshotResult<Self> {
    Ok(Self {
      next_load_id: d.i32()?,
      main_module_id: d.option(|d| d.i32())?,
      modules: d.seq(ModuleInfo::decode)?,
      module_handles: d.seq(|d| d.u32())?,
      main_module_callbacks: d.seq(|d| d.u32())?,
      by_name: d.seq(|d| {
        Ok((
          d.fast_string()?,
          RequestedModuleType::decode(d)?,
          SymbolicModule::decode(d)?,
        ))
      })?,
      lazy_esm_specifiers: d.seq(|d| d.cow_str())?,
      loaded_script_results: d.seq(|d| Ok((d.fast_string()?, d.u32()?)))?,
      captured_bootstrap: d.option(|d| d.u32())?,
    })
  }
}

impl ModuleMapData {
  pub fn create_module_info(
    &mut self,
    name: FastString,
    module_type: ModuleType,
    handle: v8::Global<v8::Module>,
    main: bool,
    requests: Vec<ModuleRequest>,
  ) -> ModuleId {
    let data = self;
    let id = data.handles.len();
    let (name1, name2) = name.into_cheap_copy();
    data.handles_inverted.insert(handle.clone(), id);
    data.handles.push(handle);
    if main {
      data.main_module_id = Some(id);
    }
    // TODO(bartlomieju): verify if we can store `ModuleType` here instead
    let requested_module_type = RequestedModuleType::from(module_type.clone());
    data
      .by_name
      .insert(&requested_module_type, name1, SymbolicModule::Mod(id));
    data.info.push(ModuleInfo {
      id,
      main,
      name: name2,
      requests,
      module_type,
    });

    id
  }

  /// Get module id, following all aliases in case of module specifier
  /// that had been redirected.
  pub fn get_id(
    &self,
    name: &str,
    requested_module_type: impl AsRef<RequestedModuleType>,
  ) -> Option<ModuleId> {
    let map = &self.by_name;
    let first_symbolic_module =
      map.get(requested_module_type.as_ref(), name)?;
    let mut mod_name = match first_symbolic_module {
      SymbolicModule::Mod(mod_id) => return Some(*mod_id),
      SymbolicModule::Alias(target) => target,
    };
    loop {
      let symbolic_module =
        map.get(requested_module_type.as_ref(), mod_name)?;
      match symbolic_module {
        SymbolicModule::Alias(target) => {
          debug_assert!(mod_name != target);
          mod_name = target;
        }
        SymbolicModule::Mod(mod_id) => return Some(*mod_id),
      }
    }
  }

  pub(crate) fn alias(
    &mut self,
    name: FastString,
    requested_module_type: &RequestedModuleType,
    target: FastString,
  ) {
    debug_assert_ne!(name, target);
    self.by_name.insert(
      requested_module_type,
      name,
      SymbolicModule::Alias(target),
    );
  }

  /// Register an additional `(name, requested_module_type) -> module_id`
  /// mapping for a module that was already registered under a different
  /// requested module type. Used to make modules visible to imports whose
  /// `with { type: ... }` attribute doesn't match the loaded module's
  /// actual type — for example when a `module.registerHooks()` load hook
  /// returns `format: "module"` for an import with a custom type
  /// attribute like `with { type: "x-css" }`.
  pub(crate) fn register_under_type(
    &mut self,
    name: FastString,
    requested_module_type: &RequestedModuleType,
    module_id: ModuleId,
  ) {
    self.by_name.insert(
      requested_module_type,
      name,
      SymbolicModule::Mod(module_id),
    );
  }

  #[cfg(test)]
  pub(crate) fn is_alias(
    &self,
    name: &str,
    requested_module_type: impl AsRef<RequestedModuleType>,
  ) -> bool {
    let map = &self.by_name;
    let entry = map.get(requested_module_type.as_ref(), name);
    matches!(entry, Some(SymbolicModule::Alias(_)))
  }

  pub(crate) fn get_handle(
    &self,
    id: ModuleId,
  ) -> Option<v8::Global<v8::Module>> {
    self.handles.get(id).cloned()
  }

  pub(crate) fn get_name_by_module(
    &self,
    global: &v8::Global<v8::Module>,
  ) -> Option<String> {
    match self.handles_inverted.get(global) {
      Some(id) => self.get_name_by_id(*id),
      _ => None,
    }
  }

  pub(crate) fn get_type_by_module(
    &self,
    global: &v8::Global<v8::Module>,
  ) -> Option<ModuleType> {
    match self.handles_inverted.get(global) {
      Some(id) => {
        let info = self.info.get(*id).unwrap();
        Some(info.module_type.clone())
      }
      _ => None,
    }
  }

  pub(crate) fn get_info_by_module(
    &self,
    global: &v8::Global<v8::Module>,
  ) -> Option<&ModuleInfo> {
    match self.handles_inverted.get(global) {
      Some(id) => Some(self.info.get(*id).unwrap()),
      _ => None,
    }
  }

  pub(crate) fn is_main_module(&self, global: &v8::Global<v8::Module>) -> bool {
    self
      .main_module_id
      .map(|id| self.handles_inverted.get(global) == Some(&id))
      .unwrap_or_default()
  }

  pub(crate) fn get_name_by_id(&self, id: ModuleId) -> Option<String> {
    // TODO(mmastrac): Don't clone
    self.info.get(id).map(|info| info.name.as_str().to_owned())
  }

  /// Serialize for snapshotting.
  ///
  /// `instantiated` is a per-module-id flag saying whether the V8 module was
  /// already instantiated at snapshot time (see
  /// [`crate::modules::ModuleMap::instantiated_flags`]). The import edges of an
  /// instantiated module are dead weight in the snapshot: the only consumers of
  /// `ModuleInfo::requests` are `module_resolve_callback` /
  /// `module_source_callback`, which V8 only invokes while *instantiating* a
  /// module, and `RecursiveModuleLoad`, which only walks the requests of an
  /// already-registered module to make sure its subgraph is registered — and a
  /// module instantiated in the snapshot necessarily brought its whole subgraph
  /// with it. So we drop them, which also means the `Url`s inside them (the one
  /// thing in the sidecar that cannot be a borrow) are never written or parsed.
  pub fn serialize_for_snapshotting(
    self,
    data_store: &mut SnapshotStoreDataStore,
    instantiated: &[bool],
  ) -> ModuleMapSnapshotData {
    debug_assert_eq!(self.by_name.len(), self.handles.len());
    debug_assert_eq!(self.info.len(), self.handles.len());

    let mut info = self.info;
    for module in info.iter_mut() {
      if instantiated.get(module.id).copied().unwrap_or(false) {
        module.requests = Vec::new();
      }
    }

    let mut ser = ModuleMapSnapshotData {
      next_load_id: self.next_load_id,
      main_module_id: self.main_module_id.map(|x| x as _),
      modules: info,
      ..Default::default()
    };

    ser.main_module_callbacks = self
      .main_module_callbacks
      .into_iter()
      .map(|x| data_store.register(x))
      .collect();
    ser.module_handles = self
      .handles
      .into_iter()
      .map(|v| data_store.register(v))
      .collect();

    self.by_name.drain(|_, module_type, name, module| {
      ser.by_name.push((name, module_type.clone(), module));
    });

    ser.lazy_esm_specifiers =
      self.known_lazy_esm.into_inner().into_iter().collect();

    // Move out of the Rc<RefCell<...>> so we can consume the values.
    let cached_results: HashMap<ModuleName, v8::Global<v8::Value>> =
      std::mem::take(&mut *self.loaded_script_results.borrow_mut());
    ser.loaded_script_results = cached_results
      .into_iter()
      .map(|(name, value)| (name, data_store.register(value)))
      .collect();

    ser.captured_bootstrap = self
      .captured_bootstrap
      .into_inner()
      .map(|value| data_store.register(value));

    ser
  }

  pub fn update_with_snapshotted_data(
    &mut self,
    scope: &mut v8::PinScope,
    data_store: &mut SnapshotLoadDataStore,
    data: ModuleMapSnapshotData,
  ) {
    self.next_load_id = data.next_load_id;
    self.main_module_id = data.main_module_id.map(|x| x as _);
    self.info = data.modules;
    self.handles.reserve(data.module_handles.len());
    self.handles_inverted.reserve(data.module_handles.len());
    self.main_module_callbacks = data
      .main_module_callbacks
      .into_iter()
      .map(|x| data_store.get(scope, x))
      .collect();

    for module_handle in data.module_handles {
      let id = self.handles.len();
      let module = data_store.get::<v8::Module>(scope, module_handle);
      self.handles_inverted.insert(module.clone(), id as _);
      self.handles.push(module);
    }

    for (name, module_type, module) in data.by_name {
      self.by_name.insert(&module_type, name, module)
    }

    *self.known_lazy_esm.borrow_mut() =
      data.lazy_esm_specifiers.into_iter().collect();

    let mut cached_results = self.loaded_script_results.borrow_mut();
    for (name, id) in data.loaded_script_results {
      let value = data_store.get::<v8::Value>(scope, id);
      cached_results.insert(name, value);
    }
    drop(cached_results);

    if let Some(id) = data.captured_bootstrap {
      let value = data_store.get::<v8::Value>(scope, id);
      *self.captured_bootstrap.borrow_mut() = Some(value);
    }
  }

  // TODO(mmastrac): this is better than giving the entire crate access to the internals.
  #[cfg(test)]
  ///
  /// `restored_from_snapshot` is the number of module ids that were rehydrated
  /// from a snapshot. Those modules were already instantiated when the snapshot
  /// was taken, so their import edges are intentionally not persisted (see
  /// [`Self::serialize_for_snapshotting`]) and are expected to be empty here.
  pub fn assert_module_map(
    &self,
    modules: &Vec<ModuleInfo>,
    restored_from_snapshot: usize,
  ) {
    use crate::runtime::NO_OF_BUILTIN_MODULES;
    let data = self;
    assert_eq!(data.handles.len(), modules.len() + NO_OF_BUILTIN_MODULES);
    assert_eq!(data.info.len(), modules.len() + NO_OF_BUILTIN_MODULES);
    assert_eq!(data.next_load_id as usize, modules.len());
    assert_eq!(data.by_name.len(), modules.len() + NO_OF_BUILTIN_MODULES);

    for info in modules {
      assert!(data.handles.get(info.id).is_some());
      let actual = data.info.get(info.id).unwrap();
      if info.id < restored_from_snapshot {
        assert_eq!(actual.id, info.id);
        assert_eq!(actual.main, info.main);
        assert_eq!(actual.name, info.name);
        assert_eq!(actual.module_type, info.module_type);
        assert!(
          actual.requests.is_empty(),
          "import edges of snapshot-instantiated module {} should be dropped",
          info.name
        );
      } else {
        assert_eq!(actual, info);
      }
      let requested_module_type =
        RequestedModuleType::from(info.module_type.clone());
      assert_eq!(
        data.by_name.get(&requested_module_type, &info.name),
        Some(&SymbolicModule::Mod(info.id))
      );
    }
  }
}

#[cfg(test)]
mod tests {
  use url::Url;

  use super::*;
  use crate::ascii_str;

  #[test]
  fn module_name_map_test() {
    let mut data: ModuleNameTypeMap<usize> = ModuleNameTypeMap::default();
    data.insert(
      &RequestedModuleType::Json,
      ascii_str!("http://example.com/").into(),
      1,
    );
    assert_eq!(
      Some(&1),
      data.get(
        &RequestedModuleType::Json,
        Url::parse("http://example.com/").unwrap().as_str()
      )
    );
  }
}
