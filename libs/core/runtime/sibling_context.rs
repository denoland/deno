// Copyright 2018-2026 the Deno authors. MIT license.

//! Per-module isolation primitives.
//!
//! This module provides the low-level building blocks for the foolproof
//! per-module-permissions design tracked in
//! <https://github.com/denoland/orchid/issues/64>:
//!
//! * [`create_sibling_context`] — create a `v8::Context` within the current
//!   isolate that shares the runtime's `ContextState` / `ModuleMap` slots
//!   but has its own global object. Modeled on
//!   `ContextifyContext::attach_vanilla` in `ext/node/ops/vm.rs`.
//!
//! * [`compile_module_in`], [`instantiate_module_in`],
//!   [`evaluate_module_in`], [`module_namespace_in`] — the four steps for
//!   compiling, instantiating, evaluating, and inspecting an ES module
//!   inside a sibling context. Modeled on the `node:vm` `SourceTextModule`
//!   ops in the same file.
//!
//! * [`create_module_namespace_bridge`] — create a synthetic module in
//!   *one* realm whose exports are bound to `v8::Global` handles drawn
//!   from another realm's module namespace. This is the cross-realm
//!   membrane primitive: a module in realm-A can `import` from a denied
//!   module in realm-B by going through such a bridge.
//!
//! Together these are sufficient to compile an untrusted ES module into a
//! sibling realm with a stripped `Deno` binding, then expose its exports
//! to a trusted importer without giving the untrusted code any reference
//! to the trusted realm's globals.

use std::cell::RefCell;
use std::collections::HashMap;
use std::num::NonZeroI32;

use crate::runtime::CONTEXT_STATE_SLOT_INDEX;
use crate::runtime::MODULE_MAP_SLOT_INDEX;

/// Create a sibling `v8::Context` within the current isolate that shares
/// the main context's `ContextState` and `ModuleMap` embedder slots.
///
/// The returned context has its own global object, so callers can install
/// per-realm bindings (e.g. a policy-scoped `Deno`) without affecting the
/// main realm. The security token matches the main context.
///
/// Caller responsibilities:
/// * Wrap the returned context in `v8::Global` if it should outlive the
///   current scope.
/// * Install whatever bindings the realm needs (e.g. `Deno`) before
///   executing untrusted code in it.
///
/// # Panics
///
/// Panics if the current context has not been set up with the runtime's
/// `CONTEXT_STATE_SLOT_INDEX` and `MODULE_MAP_SLOT_INDEX` embedder slots —
/// in practice this means the function must be called from a scope whose
/// current context is the runtime's main context (or a sibling created
/// previously by this primitive).
pub fn create_sibling_context<'s, 'i>(
  scope: &mut v8::PinScope<'s, 'i>,
) -> v8::Local<'s, v8::Context> {
  let main_context = scope.get_current_context();
  let context_state_ptr = main_context
    .get_aligned_pointer_from_embedder_data(CONTEXT_STATE_SLOT_INDEX);
  let module_map_ptr =
    main_context.get_aligned_pointer_from_embedder_data(MODULE_MAP_SLOT_INDEX);
  assert!(
    !context_state_ptr.is_null(),
    "create_sibling_context: parent context has no ContextState slot"
  );
  assert!(
    !module_map_ptr.is_null(),
    "create_sibling_context: parent context has no ModuleMap slot"
  );

  let security_token = main_context.get_security_token(scope);

  let context = {
    let esc_scope = std::pin::pin!(v8::EscapableHandleScope::new(scope));
    let esc_scope = &mut esc_scope.init();
    let ctx = v8::Context::new(esc_scope, v8::ContextOptions::default());
    esc_scope.escape(ctx)
  };

  context.set_security_token(security_token);

  // SAFETY: the pointers come from a live `ContextState` and `ModuleMap`
  // owned by the parent context, which outlives this sibling (they share
  // an isolate and the parent is the main realm of the running runtime).
  // Both pointers are read-only after install — sibling and main share the
  // same heap-allocated objects. The owning `JsRealmInner::destroy` path
  // is the only writer, and that runs only when the runtime is torn down,
  // after every sibling context has already been dropped.
  unsafe {
    context.set_aligned_pointer_in_embedder_data(
      CONTEXT_STATE_SLOT_INDEX,
      context_state_ptr,
    );
    context.set_aligned_pointer_in_embedder_data(
      MODULE_MAP_SLOT_INDEX,
      module_map_ptr,
    );
  }

  context
}

fn module_origin<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  name: v8::Local<'s, v8::String>,
) -> v8::ScriptOrigin<'s> {
  v8::ScriptOrigin::new(
    scope,
    name.into(),
    0,
    0,
    false,
    -1,
    None,
    false,
    false,
    true, // is_module
    None,
  )
}

/// Compile an ES module's source into `context`.
///
/// On compile error, the V8 exception is returned as a `v8::Global` so
/// the caller can rethrow it on its own scope.
pub fn compile_module_in<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  context: v8::Local<'s, v8::Context>,
  name: &str,
  source: &str,
) -> Result<v8::Global<v8::Module>, v8::Global<v8::Value>> {
  let scope = &mut v8::ContextScope::new(scope, context);
  let name_str = v8::String::new(scope, name).unwrap();
  let source_str = v8::String::new(scope, source).unwrap();
  let origin = module_origin(scope, name_str);

  v8::tc_scope!(let tc_scope, scope);
  let mut compile_source =
    v8::script_compiler::Source::new(source_str, Some(&origin));
  let module =
    v8::script_compiler::compile_module(tc_scope, &mut compile_source);
  if tc_scope.has_caught() {
    let exception = tc_scope.exception().unwrap();
    return Err(v8::Global::new(tc_scope, exception));
  }
  Ok(v8::Global::new(tc_scope, module.unwrap()))
}

/// Resolution table used while instantiating a module in a sibling realm:
/// maps an `import` specifier appearing in the module to the already-
/// compiled `v8::Module` that should satisfy it.
pub type ResolutionTable = HashMap<String, v8::Global<v8::Module>>;

thread_local! {
  /// Active resolution table per (referrer module identity hash). Set up
  /// by `instantiate_module_in` for the duration of one V8
  /// `instantiate_module` call so that the V8 resolve callback (which
  /// runs synchronously inside `instantiate_module`) can look up the
  /// referrer's import targets without needing closure capture.
  static ACTIVE_RESOLUTIONS: RefCell<HashMap<NonZeroI32, ResolutionTable>> =
    RefCell::new(HashMap::new());
}

fn isolated_resolve_callback<'s>(
  context: v8::Local<'s, v8::Context>,
  specifier: v8::Local<'s, v8::String>,
  _import_attributes: v8::Local<'s, v8::FixedArray>,
  referrer: v8::Local<'s, v8::Module>,
) -> Option<v8::Local<'s, v8::Module>> {
  // SAFETY: `CallbackScope` is constructed from a live V8 callback context.
  v8::callback_scope!(unsafe scope, context);
  let referrer_hash = referrer.get_identity_hash();
  let specifier_str = specifier.to_rust_string_lossy(scope);

  let resolved = ACTIVE_RESOLUTIONS.with(|r| {
    let map = r.borrow();
    map
      .get(&referrer_hash)
      .and_then(|t| t.get(&specifier_str))
      .map(|g| v8::Local::new(scope, g))
  });

  if let Some(local) = resolved {
    return Some(local);
  }

  let message = v8::String::new(
    scope,
    &format!(
      "compile_module_in: no resolution provided for import '{specifier_str}'"
    ),
  )?;
  let exception = v8::Exception::error(scope, message);
  scope.throw_exception(exception);
  None
}

/// Instantiate an already-compiled module inside `context`, using the
/// supplied resolution table to satisfy its static imports.
///
/// Each module request in the source must have an entry in `resolutions`;
/// the resolved module must live in the same context as `module` (V8
/// rejects cross-context module linkage).
pub fn instantiate_module_in(
  scope: &mut v8::PinScope<'_, '_>,
  context: v8::Local<v8::Context>,
  module: &v8::Global<v8::Module>,
  resolutions: ResolutionTable,
) -> Result<(), v8::Global<v8::Value>> {
  let scope = &mut v8::ContextScope::new(scope, context);
  let module_local = v8::Local::new(scope, module);
  let referrer_hash = module_local.get_identity_hash();

  ACTIVE_RESOLUTIONS.with(|r| {
    r.borrow_mut().insert(referrer_hash, resolutions);
  });

  v8::tc_scope!(let tc_scope, scope);
  let outcome =
    module_local.instantiate_module(tc_scope, isolated_resolve_callback);

  ACTIVE_RESOLUTIONS.with(|r| {
    r.borrow_mut().remove(&referrer_hash);
  });

  if tc_scope.has_caught() {
    let exception = tc_scope.exception().unwrap();
    return Err(v8::Global::new(tc_scope, exception));
  }
  if outcome != Some(true) {
    let msg =
      v8::String::new(tc_scope, "module instantiation returned false").unwrap();
    let exception = v8::Exception::error(tc_scope, msg);
    return Err(v8::Global::new(tc_scope, exception));
  }
  Ok(())
}

/// Evaluate an instantiated module inside `context`. Returns the V8
/// evaluation result (a promise under top-level-await mode).
pub fn evaluate_module_in(
  scope: &mut v8::PinScope<'_, '_>,
  context: v8::Local<v8::Context>,
  module: &v8::Global<v8::Module>,
) -> Result<v8::Global<v8::Value>, v8::Global<v8::Value>> {
  let scope = &mut v8::ContextScope::new(scope, context);
  let module_local = v8::Local::new(scope, module);

  v8::tc_scope!(let tc_scope, scope);
  let result = module_local.evaluate(tc_scope);
  if tc_scope.has_caught() {
    let exception = tc_scope.exception().unwrap();
    return Err(v8::Global::new(tc_scope, exception));
  }
  let value = result.expect("module.evaluate returned None without exception");
  Ok(v8::Global::new(tc_scope, value))
}

/// Get the module namespace object of an instantiated module, as a
/// `v8::Global`. The namespace object's own properties enumerate the
/// module's static exports.
pub fn module_namespace_in(
  scope: &mut v8::PinScope<'_, '_>,
  context: v8::Local<v8::Context>,
  module: &v8::Global<v8::Module>,
) -> v8::Global<v8::Object> {
  let scope = &mut v8::ContextScope::new(scope, context);
  let module_local = v8::Local::new(scope, module);
  let ns = module_local.get_module_namespace();
  let ns_obj: v8::Local<v8::Object> = ns.try_into().expect(
    "module namespace is an object once the module has been instantiated",
  );
  v8::Global::new(scope, ns_obj)
}

/// `(export name, v8::Global value)` pairs that the bridge's
/// evaluation step will hand to `set_synthetic_module_export`.
type BridgeExports = Vec<(String, v8::Global<v8::Value>)>;

thread_local! {
  /// Bridge values keyed by the synthetic-bridge module's identity hash.
  /// Each entry holds the exports stashed at bridge-creation time and
  /// drained when V8 evaluates the synthetic module.
  static BRIDGE_EXPORTS: RefCell<HashMap<NonZeroI32, BridgeExports>> =
    RefCell::new(HashMap::new());
}

// The `Option` return is dictated by V8's `SyntheticModuleEvaluationSteps`
// signature; we always return `Some(promise)`.
#[allow(
  clippy::unnecessary_wraps,
  reason = "callback signature is fixed by V8"
)]
fn bridge_evaluation_steps<'s>(
  context: v8::Local<'s, v8::Context>,
  module: v8::Local<'s, v8::Module>,
) -> Option<v8::Local<'s, v8::Value>> {
  v8::callback_scope!(unsafe scope, context);
  v8::tc_scope!(let tc_scope, scope);
  let id = module.get_identity_hash();

  let exports = BRIDGE_EXPORTS
    .with(|s| s.borrow_mut().remove(&id))
    .unwrap_or_default();
  for (name, value_global) in exports {
    let name_str = v8::String::new(tc_scope, &name).unwrap();
    let value = v8::Local::new(tc_scope, &value_global);
    let set_ok = module
      .set_synthetic_module_export(tc_scope, name_str, value)
      .unwrap_or(false);
    assert!(
      set_ok,
      "bridge module failed to set synthetic export '{name}'"
    );
    assert!(!tc_scope.has_caught());
  }

  // Synthetic modules must return a settled promise.
  let resolver = v8::PromiseResolver::new(tc_scope).unwrap();
  let undefined = v8::undefined(tc_scope);
  resolver.resolve(tc_scope, undefined.into());
  Some(resolver.get_promise(tc_scope).into())
}

/// Create a *bridge module* inside `target_context` whose exports are
/// drawn from `source_namespace` (typically the module namespace of a
/// module compiled in a different realm).
///
/// The bridge is a V8 synthetic module: its export names are taken from
/// `source_namespace.GetOwnPropertyNames(...)`, and its export values
/// are `v8::Global` handles read out of that namespace. Once the bridge
/// is evaluated, the trusted realm sees the foreign realm's exports as
/// live bindings on a regular module — no globalThis access required.
///
/// The returned `v8::Global<v8::Module>` is ready to be passed into
/// [`instantiate_module_in`]'s resolution table for any other module in
/// `target_context` that imports the foreign module.
///
/// `name` is the resource name shown in stack traces; it does *not*
/// participate in module resolution.
pub fn create_module_namespace_bridge(
  scope: &mut v8::PinScope<'_, '_>,
  target_context: v8::Local<v8::Context>,
  name: &str,
  source_context: v8::Local<v8::Context>,
  source_namespace: &v8::Global<v8::Object>,
) -> v8::Global<v8::Module> {
  // 1. Read the export names and per-name values from the source
  //    namespace. We do this in the source context to make sure
  //    property access uses the source realm's prototype chain and the
  //    values come from the right realm.
  let exports: Vec<(String, v8::Global<v8::Value>)> = {
    let scope = &mut v8::ContextScope::new(scope, source_context);
    let ns_local = v8::Local::new(scope, source_namespace);
    let prop_names = ns_local
      .get_own_property_names(scope, v8::GetPropertyNamesArgs::default())
      .expect("module namespace has own property names");
    let len = prop_names.length();
    let mut out = Vec::with_capacity(len as usize);
    for i in 0..len {
      let key_val = prop_names.get_index(scope, i).unwrap();
      let name_str = key_val.to_rust_string_lossy(scope);
      // Module-namespace exports are non-configurable own data
      // properties; `get` here always succeeds for valid bindings.
      let value_local = ns_local
        .get(scope, key_val)
        .expect("namespace export lookup");
      out.push((name_str, v8::Global::new(scope, value_local)));
    }
    out
  };

  // 2. Create a synthetic module in the target context with those
  //    export names. Stash the values keyed by the synthetic module's
  //    identity hash so `bridge_evaluation_steps` can install them
  //    during evaluation.
  let scope = &mut v8::ContextScope::new(scope, target_context);
  let name_str = v8::String::new(scope, name).unwrap();
  let export_name_strs: Vec<v8::Local<v8::String>> = exports
    .iter()
    .map(|(n, _)| v8::String::new(scope, n).unwrap())
    .collect();
  let module = v8::Module::create_synthetic_module(
    scope,
    name_str,
    &export_name_strs,
    bridge_evaluation_steps,
  );
  let global = v8::Global::new(scope, module);

  let id = module.get_identity_hash();
  BRIDGE_EXPORTS.with(|s| {
    s.borrow_mut().insert(id, exports);
  });

  global
}
