// Copyright 2018-2026 the Deno authors. MIT license.

//! Sibling-context primitive for per-module isolation.
//!
//! Creates additional `v8::Context`s within the same isolate as the main
//! runtime, modeled on the `node:vm` sandbox pattern in `ext/node/ops/vm.rs`.
//! Each sibling context shares the runtime's `ContextState` and `ModuleMap`
//! embedder slots, so ops dispatch correctly, but has its own global object —
//! including its own `Deno` binding. The security token matches the main
//! context, enabling same-isolate cross-context object access.
//!
//! This is the low-level building block for the per-module-permissions design
//! tracked in <https://github.com/denoland/orchid/issues/64>: each ES module
//! that needs policy isolation gets compiled and instantiated into its own
//! sibling context with a policy-scoped `Deno` binding, so the module has no
//! closure path to the unwrapped op functions.
//!
//! Higher-level construction (per-realm `Deno` installation, cross-realm
//! module bridging via synthetic modules) lives outside this primitive.

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
