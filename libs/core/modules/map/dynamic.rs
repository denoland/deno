// Copyright 2018-2026 the Deno authors. MIT license.

use std::collections::HashMap;
use std::future::Future;
use std::rc::Rc;
use std::task::Context;
use std::task::Poll;

use deno_core::error::CoreError;
use deno_error::JsErrorBox;
use futures::StreamExt;
use futures::future::FutureExt;
use v8::Function;

use super::ModuleMap;
use super::is_execution_terminated_module_error;
use super::module_error_to_v8_exception;
use crate::error::CoreErrorKind;
use crate::modules::ModuleId;
use crate::modules::ModuleImportPhase;
use crate::modules::ModuleLoadId;
use crate::modules::RequestedModuleType;
use crate::modules::ResolutionKind;
use crate::modules::module_map_data::ModuleSourceKey;
use crate::modules::recursive_load::RecursiveModuleLoad;
use crate::modules::recursive_load::RegisterOutcome;
use crate::runtime::JsRealm;

pub(super) type PrepareLoadFuture =
  dyn Future<Output = (ModuleLoadId, Result<RecursiveModuleLoad, CoreError>)>;

pub(super) struct DynImportModEvaluate {
  load_id: ModuleLoadId,
  module_id: ModuleId,
  promise: v8::Global<v8::Promise>,
}

#[derive(Debug, Clone)]
pub(super) struct DynImportState {
  resolver: v8::Global<v8::PromiseResolver>,
  cped: v8::Global<v8::Value>,
  phase: ModuleImportPhase,
}

impl ModuleMap {
  // Initiate loading of a module graph imported using `import()`.
  #[allow(clippy::too_many_arguments, reason = "internal code")]
  pub(crate) fn load_dynamic_import(
    self: Rc<Self>,
    scope: &mut v8::PinScope,
    specifier: String,
    referrer: String,
    requested_module_type: RequestedModuleType,
    phase: ModuleImportPhase,
    resolver_handle: v8::Global<v8::PromiseResolver>,
    cped_handle: v8::Global<v8::Value>,
  ) -> bool {
    let resolve_response = self.resolve_with_scope(
      scope,
      &specifier,
      &referrer,
      ResolutionKind::DynamicImport,
      &HashMap::new(),
    );

    // Fast path: if the module is already loaded, resolve the import
    // immediately without async work.
    if phase == ModuleImportPhase::Evaluation
      && let ref resolve_result = resolve_response
      && let Ok(module_specifier) = resolve_result
      && let Some(id) = self
        .data
        .borrow()
        .get_id(module_specifier.as_str(), &requested_module_type)
    {
      let module = self
        .data
        .borrow()
        .get_handle(id)
        .map(|handle| v8::Local::new(scope, handle))
        .expect("Dyn import module info not found");

      if module.get_status() == v8::ModuleStatus::Evaluated {
        // Check if this module has a pending TLA (top-level await) evaluation.
        let has_pending_tla = self
          .pending_dyn_mod_evaluations
          .borrow()
          .iter()
          .any(|pending| pending.module_id == id);

        // Queue this resolver to be resolved when the TLA completes.
        if has_pending_tla {
          self
            .pending_tla_waiters
            .borrow_mut()
            .entry(id)
            .or_default()
            .push(resolver_handle);
          return false;
        }

        // No pending TLA, safe to resolve immediately
        let resolver = resolver_handle.open(scope);
        let module_namespace = module.get_module_namespace();
        resolver.resolve(scope, module_namespace).unwrap();

        return false;
      }
    }

    // Fast path for lazy-loaded ESM: load synchronously and resolve
    // immediately, avoiding the async RecursiveModuleLoad path entirely.
    if phase == ModuleImportPhase::Evaluation
      && let ref resolve_result = resolve_response
      && let Ok(module_specifier) = resolve_result
      && self.has_lazy_esm_source(module_specifier.as_str())
    {
      match self.lazy_load_esm_module(scope, module_specifier.as_str()) {
        Ok(module_ns) => {
          let resolver = resolver_handle.open(scope);
          let module_ns_local = v8::Local::new(scope, module_ns);
          resolver.resolve(scope, module_ns_local).unwrap();
          return false;
        }
        Err(e)
          if scope.is_execution_terminating()
            || matches!(e.as_kind(), CoreErrorKind::ExecutionTerminated) =>
        {
          scope.terminate_execution();
          return false;
        }
        Err(e) => {
          let exception = e.to_v8_error(scope);
          let exception_local = v8::Local::new(scope, exception);
          let resolver = resolver_handle.open(scope);
          resolver.reject(scope, exception_local).unwrap();
          return false;
        }
      }
    }

    // Fast path for `synthetic_esm`-registered modules: build the
    // synthetic module synchronously and resolve immediately, same
    // pattern as the lazy ESM fast path above.
    if phase == ModuleImportPhase::Evaluation
      && let ref resolve_result = resolve_response
      && let Ok(module_specifier) = resolve_result
      && self.has_synthetic_esm_module(module_specifier.as_str())
      && !self
        .loader
        .borrow()
        .should_load_synthetic_esm(module_specifier.as_str())
    {
      match self
        .lazy_load_synthetic_esm_module(scope, module_specifier.as_str())
      {
        Ok(module_ns) => {
          let resolver = resolver_handle.open(scope);
          let module_ns_local = v8::Local::new(scope, module_ns);
          resolver.resolve(scope, module_ns_local).unwrap();
          return false;
        }
        Err(e)
          if scope.is_execution_terminating()
            || matches!(e.as_kind(), CoreErrorKind::ExecutionTerminated) =>
        {
          scope.terminate_execution();
          return false;
        }
        Err(e) => {
          let exception = e.to_v8_error(scope);
          let exception_local = v8::Local::new(scope, exception);
          let resolver = resolver_handle.open(scope);
          resolver.reject(scope, exception_local).unwrap();
          return false;
        }
      }
    }

    let load = RecursiveModuleLoad::new_dynamic_import(
      specifier,
      referrer,
      requested_module_type,
      phase,
      self.clone(),
      resolve_response,
    );

    self.dynamic_import_map.borrow_mut().insert(
      load.id(),
      DynImportState {
        resolver: resolver_handle,
        cped: cped_handle,
        phase,
      },
    );

    let load_id = load.id();
    let fut = async move {
      let mut load = load;
      (load_id, load.prepare().await.map(|()| load))
    }
    .boxed_local();

    self.preparing_dynamic_imports.push(fut);

    true
  }

  pub(crate) fn has_pending_dynamic_imports(&self) -> bool {
    self.preparing_dynamic_imports.is_pending()
      || self.pending_dynamic_imports.is_pending()
  }

  pub(crate) fn has_pending_dyn_module_evaluation(&self) -> bool {
    self.pending_dyn_mod_evaluations.is_pending()
  }

  fn dynamic_import_module_evaluate(
    &self,
    scope: &mut v8::PinScope,
    id: ModuleId,
    load_id: ModuleLoadId,
    state: DynImportState,
  ) -> Result<(), CoreError> {
    let module_handle = self.get_handle(id).expect("ModuleInfo not found");

    let status = {
      let module = module_handle.open(scope);
      module.get_status()
    };

    match status {
      v8::ModuleStatus::Instantiated | v8::ModuleStatus::Evaluated => {}
      _ => return Ok(()),
    }

    // IMPORTANT: Top-level-await is enabled, which means that return value
    // of module evaluation is a promise.
    //
    // This promise is internal, and not the same one that gets returned to
    // the user. We add handlers to wake the event loop when the promise resolves
    // (or rejects). The catch handler also serves to prevent an exception if the internal promise
    // rejects. That will instead happen for the other if not handled by the user.
    //
    // For more details see:
    // https://github.com/denoland/deno/issues/4908
    // https://v8.dev/features/top-level-await#module-execution-order
    v8::tc_scope!(let tc_scope, scope);

    let cped = v8::Local::new(tc_scope, state.cped);
    tc_scope.set_continuation_preserved_embedder_data(cped);

    let module = v8::Local::new(tc_scope, &module_handle);
    // Set `evaluating_top_level` so that any nested `lazy_load_esm_module`
    // calls (triggered e.g. by CJS `require()` chains under
    // `npm:` packages) skip their post-evaluate `perform_microtask_checkpoint`.
    // Draining microtasks while V8 is inside `module.evaluate()` on an
    // async module graph leaves the evaluation promise permanently
    // Pending — V8 advances AsyncModuleExecutionFulfilled for the resumed
    // TLA dep but cannot then run `ExecuteModule` on the still-evaluating
    // parent.
    self.evaluating_top_level.set(true);
    let maybe_value = module.evaluate(tc_scope);
    self.evaluating_top_level.set(false);

    // Update status after evaluating.
    let status = module.get_status();

    if let Some(value) = maybe_value {
      debug_assert!(
        status == v8::ModuleStatus::Evaluated
          || status == v8::ModuleStatus::Errored
      );

      fn wake_module(
        scope: &mut v8::PinScope<'_, '_>,
        _args: v8::FunctionCallbackArguments<'_>,
        _rv: v8::ReturnValue,
      ) {
        let module_map = JsRealm::module_map_from(scope);
        module_map.module_waker.wake();
      }

      let promise = v8::Local::<v8::Promise>::try_from(value)
        .expect("Expected to get promise as module evaluation result");

      let wake_module_cb = Function::builder(wake_module).build(tc_scope);

      if let Some(wake_module_cb) = wake_module_cb {
        promise.then2(tc_scope, wake_module_cb, wake_module_cb);
      } else {
        // If the runtime is shutting down, we can't attach the handlers.
        // It doesn't really matter though, because they're just for waking the
        // event loop.
      }
      let dyn_import_mod_evaluate = DynImportModEvaluate {
        load_id,
        module_id: id,
        promise: v8::Global::new(tc_scope, promise),
      };

      self
        .pending_dyn_mod_evaluations
        .push(dyn_import_mod_evaluate);
    } else if tc_scope.has_terminated() || tc_scope.is_execution_terminating() {
      return Err(CoreErrorKind::EvaluateDynamicImportedModule.into_box());
    } else {
      assert_eq!(status, v8::ModuleStatus::Errored);
    }

    Ok(())
  }

  // Returns true if some dynamic import was resolved.
  fn evaluate_dyn_imports(
    &self,
    scope: &mut v8::PinScope,
  ) -> Result<bool, CoreError> {
    if !self.pending_dyn_mod_evaluations.is_pending() {
      return Ok(false);
    }

    let pending = self.pending_dyn_mod_evaluations.take();
    let mut resolved_any = false;
    let mut still_pending = vec![];
    for eval in pending {
      let promise = eval.promise.open(scope);
      match promise.state() {
        v8::PromiseState::Pending => {
          still_pending.push(eval);
        }
        v8::PromiseState::Fulfilled => {
          resolved_any = true;
          self.dynamic_import_resolve(scope, eval.load_id, eval.module_id)?;
          self.resolve_tla_waiters(scope, eval.module_id)?;
        }
        v8::PromiseState::Rejected => {
          resolved_any = true;
          let exception = v8::Global::new(scope, promise.result(scope));
          self.dynamic_import_reject(scope, eval.load_id, exception.clone())?;
          self.reject_tla_waiters(scope, eval.module_id, exception)?;
        }
      }
    }
    self.pending_dyn_mod_evaluations.set(still_pending);
    Ok(resolved_any)
  }

  /// Resolve all waiters that are waiting for a module's TLA to complete.
  fn resolve_tla_waiters(
    &self,
    scope: &mut v8::PinScope,
    module_id: ModuleId,
  ) -> Result<(), CoreError> {
    let waiters = self.pending_tla_waiters.borrow_mut().remove(&module_id);
    if let Some(waiters) = waiters
      && let Some(module) = self
        .data
        .borrow()
        .get_handle(module_id)
        .map(|handle| v8::Local::new(scope, handle))
    {
      let module_namespace = module.get_module_namespace();

      for resolver_handle in waiters {
        let resolver = resolver_handle.open(scope);
        if resolver.resolve(scope, module_namespace).is_none() {
          if scope.is_execution_terminating() {
            return Err(CoreErrorKind::ExecutionTerminated.into_box());
          }
          return Err(
            JsErrorBox::generic("Failed to resolve a top-level-await waiter")
              .into(),
          );
        }
      }
      if !JsRealm::state_from_scope(scope).has_tick_scheduled() {
        scope.perform_microtask_checkpoint();
        if scope.is_execution_terminating() {
          return Err(CoreErrorKind::ExecutionTerminated.into_box());
        }
      }
    }
    Ok(())
  }

  /// Reject all waiters that are waiting for a module's TLA to complete.
  fn reject_tla_waiters(
    &self,
    scope: &mut v8::PinScope,
    module_id: ModuleId,
    exception: v8::Global<v8::Value>,
  ) -> Result<(), CoreError> {
    let waiters = self.pending_tla_waiters.borrow_mut().remove(&module_id);
    if let Some(waiters) = waiters {
      let exception = v8::Local::new(scope, exception);
      for resolver_handle in waiters {
        let resolver = resolver_handle.open(scope);
        if resolver.reject(scope, exception).is_none() {
          if scope.is_execution_terminating() {
            return Err(CoreErrorKind::ExecutionTerminated.into_box());
          }
          return Err(
            JsErrorBox::generic("Failed to reject a top-level-await waiter")
              .into(),
          );
        }
      }
      if !JsRealm::state_from_scope(scope).has_tick_scheduled() {
        scope.perform_microtask_checkpoint();
        if scope.is_execution_terminating() {
          return Err(CoreErrorKind::ExecutionTerminated.into_box());
        }
      }
    }
    Ok(())
  }

  pub(crate) fn dynamic_import_reject(
    &self,
    scope: &mut v8::PinScope,
    id: ModuleLoadId,
    exception: v8::Global<v8::Value>,
  ) -> Result<(), CoreError> {
    let resolver_handle = self
      .dynamic_import_map
      .borrow_mut()
      .remove(&id)
      .expect("Invalid dynamic import id")
      .resolver;
    let resolver = resolver_handle.open(scope);

    let exception = v8::Local::new(scope, exception);
    if resolver.reject(scope, exception).is_none() {
      if scope.is_execution_terminating() {
        return Err(CoreErrorKind::ExecutionTerminated.into_box());
      }
      return Err(
        JsErrorBox::generic("Failed to reject a dynamic import").into(),
      );
    }
    if !JsRealm::state_from_scope(scope).has_tick_scheduled() {
      scope.perform_microtask_checkpoint();
      if scope.is_execution_terminating() {
        return Err(CoreErrorKind::ExecutionTerminated.into_box());
      }
    }
    Ok(())
  }

  pub(crate) fn dynamic_import_resolve(
    &self,
    scope: &mut v8::PinScope,
    id: ModuleLoadId,
    mod_id: ModuleId,
  ) -> Result<(), CoreError> {
    let resolver_handle = self
      .dynamic_import_map
      .borrow_mut()
      .remove(&id)
      .expect("Invalid dynamic import id")
      .resolver;
    let resolver = resolver_handle.open(scope);

    let module = self
      .data
      .borrow()
      .get_handle(mod_id)
      .map(|handle| v8::Local::new(scope, handle))
      .expect("Dyn import module info not found");
    // Resolution success
    assert_eq!(module.get_status(), v8::ModuleStatus::Evaluated);

    // IMPORTANT: No borrows to `ModuleMap` can be held at this point because
    // resolving the promise might initiate another `import()` which will
    // in turn call `bindings::host_import_module_dynamically_callback` which
    // will reach into `ModuleMap` from within the isolate.
    let module_namespace = module.get_module_namespace();
    if resolver.resolve(scope, module_namespace).is_none() {
      if scope.is_execution_terminating() {
        return Err(CoreErrorKind::ExecutionTerminated.into_box());
      }
      return Err(
        JsErrorBox::generic("Failed to resolve a dynamic import").into(),
      );
    }
    self.dyn_module_evaluate_idle_counter.set(0);
    if !JsRealm::state_from_scope(scope).has_tick_scheduled() {
      scope.perform_microtask_checkpoint();
      if scope.is_execution_terminating() {
        return Err(CoreErrorKind::ExecutionTerminated.into_box());
      }
    }
    Ok(())
  }

  /// Drain all ready module loading work: preparing dynamic imports,
  /// loading dynamic imports, evaluating them, and flushing code cache
  /// futures. Loops until no more progress can be made.
  ///
  /// The waker from `cx` is registered so the event loop is woken when
  /// any module future makes progress.
  pub(crate) fn poll_progress(
    &self,
    cx: &mut Context,
    scope: &mut v8::PinScope,
  ) -> Result<(), CoreError> {
    let mut has_evaluated = true;

    // TODO(mmastrac): We register this waker unconditionally because we occasionally need to re-run
    // the event loop. Eventually we will want this method to correctly wake the waker on any forward
    // progress.
    self.module_waker.register(cx.waker());

    // Run in a loop so that dynamic imports that only depend on another
    // dynamic import can be resolved in this event loop iteration.
    //
    // For example, a dynamically imported module like the following can be
    // immediately resolved after `dependency.ts` is fully evaluated, but it
    // wouldn't if not for this loop.
    //
    //    await delay(1000);
    //    await import("./dependency.ts");
    //    console.log("test")
    //
    // These dynamic import dependencies can be cross-realm:
    //
    //    await delay(1000);
    //    await new ShadowRealm().importValue("./dependency.js", "default");
    //
    while has_evaluated {
      has_evaluated = false;
      loop {
        self.drain_prepare_dyn_imports(cx, scope)?;
        self.drain_dyn_imports(cx, scope)?;
        self.drain_code_cache_ready(cx);

        if self.evaluate_dyn_imports(scope)? {
          has_evaluated = true;
        } else {
          break;
        }
      }
    }

    Ok(())
  }

  /// Drain all ready preparing-dynamic-import futures, moving successful
  /// loads into `pending_dynamic_imports` and rejecting failures.
  fn drain_prepare_dyn_imports(
    &self,
    cx: &mut Context,
    scope: &mut v8::PinScope,
  ) -> Result<(), CoreError> {
    if !self.preparing_dynamic_imports.is_pending() {
      return Ok(());
    }

    while let Poll::Ready(Some((dyn_import_id, prepare_result))) =
      self.preparing_dynamic_imports.poll_next_unpin(cx)
    {
      match prepare_result {
        Ok(load) => {
          self
            .pending_dynamic_imports
            .push(StreamExt::into_future(load));
        }
        Err(err) => {
          let exception = err.to_v8_error(scope);
          self.dynamic_import_reject(scope, dyn_import_id, exception)?;
        }
      }
    }
    Ok(())
  }

  /// Drain all ready pending-dynamic-import streams, registering loaded
  /// modules and instantiating/evaluating completed imports.
  fn drain_dyn_imports(
    &self,
    cx: &mut Context,
    scope: &mut v8::PinScope,
  ) -> Result<(), CoreError> {
    if !self.pending_dynamic_imports.is_pending() {
      return Ok(());
    }

    while let Poll::Ready(Some((maybe_result, mut load))) =
      self.pending_dynamic_imports.poll_next_unpin(cx)
    {
      let dyn_import_id = load.id();

      match maybe_result {
        Some(Ok((request, info))) => {
          // A module (not necessarily the one dynamically imported) has been
          // fetched. Create and register it, and if successful, poll for the
          // next recursive-load event related to this dynamic import.
          match load.register_and_recurse(scope, &request, info) {
            Ok(RegisterOutcome::Done) => {
              // Keep importing until it's fully drained
              self
                .pending_dynamic_imports
                .push(StreamExt::into_future(load));
            }
            Err(err) => {
              if scope.is_execution_terminating() {
                self.dynamic_import_map.borrow_mut().remove(&dyn_import_id);
                return Err(CoreErrorKind::ExecutionTerminated.into_box());
              }
              if is_execution_terminated_module_error(&err) {
                self.dynamic_import_map.borrow_mut().remove(&dyn_import_id);
                return Err(CoreErrorKind::ExecutionTerminated.into_box());
              }
              let exception = module_error_to_v8_exception(scope, err);
              self.dynamic_import_reject(scope, dyn_import_id, exception)?;
            }
          }
        }
        Some(Err(err)) => {
          // A non-javascript error occurred; this could be due to an invalid
          // module specifier, or a problem with the source map, or a failure
          // to fetch the module source code.
          let exception = err.to_v8_error(scope);
          self.dynamic_import_reject(scope, dyn_import_id, exception)?;
        }
        None => {
          // Stream finished — the full module graph has been loaded.
          let state = self
            .dynamic_import_map
            .borrow()
            .get(&dyn_import_id)
            .unwrap()
            .clone();
          match state.phase {
            ModuleImportPhase::Evaluation => {
              let module_id =
                load.root_module_id().expect("Root module should be loaded");
              let result = self.instantiate_module(scope, module_id);
              if let Err(error) = result {
                if scope.is_execution_terminating()
                  || is_execution_terminated_module_error(&error)
                {
                  self.dynamic_import_map.borrow_mut().remove(&dyn_import_id);
                  return Err(CoreErrorKind::ExecutionTerminated.into_box());
                }
                let exception = module_error_to_v8_exception(scope, error);
                self.dynamic_import_reject(scope, dyn_import_id, exception)?;
              }
              self.dynamic_import_module_evaluate(
                scope,
                module_id,
                dyn_import_id,
                state,
              )?;
            }
            ModuleImportPhase::Defer => {
              // For defer phase imports, the module is instantiated but NOT
              // eagerly evaluated. We call evaluate_for_import_defer which
              // gathers and evaluates async transitive dependencies, then
              // resolve with a deferred namespace that triggers evaluation
              // on first property access.
              let module_id =
                load.root_module_id().expect("Root module should be loaded");
              let result = self.instantiate_module(scope, module_id);
              if let Err(error) = result {
                if scope.is_execution_terminating()
                  || is_execution_terminated_module_error(&error)
                {
                  self.dynamic_import_map.borrow_mut().remove(&dyn_import_id);
                  return Err(CoreErrorKind::ExecutionTerminated.into_box());
                }
                let exception = module_error_to_v8_exception(scope, error);
                self.dynamic_import_reject(scope, dyn_import_id, exception)?;
                continue;
              }
              let module_handle =
                self.get_handle(module_id).expect("ModuleInfo not found");

              v8::tc_scope!(let tc_scope, scope);

              let cped = v8::Local::new(tc_scope, state.cped.clone());
              tc_scope.set_continuation_preserved_embedder_data(cped);

              let module = v8::Local::new(tc_scope, &module_handle);

              // Gather async transitive dependencies. Returns a promise
              // that resolves when all async deps are ready.
              let maybe_promise = module.evaluate_for_import_defer(tc_scope);

              let Some(promise_val) = maybe_promise else {
                if tc_scope.has_terminated()
                  || tc_scope.is_execution_terminating()
                {
                  self.dynamic_import_map.borrow_mut().remove(&dyn_import_id);
                  return Err(CoreErrorKind::ExecutionTerminated.into_box());
                }
                let Some(exception) = tc_scope.exception() else {
                  self.dynamic_import_map.borrow_mut().remove(&dyn_import_id);
                  return Err(
                    JsErrorBox::generic(
                      "Failed to evaluate deferred module dependencies",
                    )
                    .into(),
                  );
                };
                let exception = v8::Global::new(tc_scope, exception);
                self.dynamic_import_reject(
                  tc_scope,
                  dyn_import_id,
                  exception,
                )?;
                continue;
              };
              if tc_scope.has_terminated()
                || tc_scope.is_execution_terminating()
              {
                self.dynamic_import_map.borrow_mut().remove(&dyn_import_id);
                return Err(CoreErrorKind::ExecutionTerminated.into_box());
              }

              // Get the deferred namespace — this triggers evaluation on
              // first property access.
              let module_namespace = module
                .get_module_namespace_with_phase(v8::ModuleImportPhase::kDefer);

              let promise = v8::Local::<v8::Promise>::try_from(promise_val)
                .expect("evaluate_for_import_defer should return a promise");

              match promise.state() {
                v8::PromiseState::Fulfilled => {
                  // All async deps are ready, resolve immediately.
                  let resolver_handle = self
                    .dynamic_import_map
                    .borrow_mut()
                    .remove(&dyn_import_id)
                    .expect("Invalid dynamic import id")
                    .resolver;
                  let resolver = resolver_handle.open(tc_scope);
                  resolver.resolve(tc_scope, module_namespace).unwrap();
                  tc_scope.perform_microtask_checkpoint();
                  if tc_scope.has_terminated()
                    || tc_scope.is_execution_terminating()
                  {
                    return Err(CoreErrorKind::ExecutionTerminated.into_box());
                  }
                }
                v8::PromiseState::Rejected => {
                  let err = promise.result(tc_scope);
                  let err = v8::Global::new(tc_scope, err);
                  self.dynamic_import_reject(tc_scope, dyn_import_id, err)?;
                }
                v8::PromiseState::Pending => {
                  // Async deps still loading. Store for later resolution.
                  // The module_waker will wake us when the promise settles.
                  fn wake_module(
                    scope: &mut v8::PinScope<'_, '_>,
                    _args: v8::FunctionCallbackArguments<'_>,
                    _rv: v8::ReturnValue,
                  ) {
                    let module_map = JsRealm::module_map_from(scope);
                    module_map.module_waker.wake();
                  }

                  let wake_module_cb =
                    v8::Function::builder(wake_module).build(tc_scope);
                  if let Some(wake_module_cb) = wake_module_cb {
                    promise.then2(tc_scope, wake_module_cb, wake_module_cb);
                  }

                  let dyn_import_mod_evaluate = DynImportModEvaluate {
                    load_id: dyn_import_id,
                    module_id,
                    promise: v8::Global::new(tc_scope, promise),
                  };
                  self
                    .pending_dyn_mod_evaluations
                    .push(dyn_import_mod_evaluate);
                }
              }
            }
            ModuleImportPhase::Source => {
              let module_reference = load.root_module_reference().expect(
                "Root module reference had to have been resolved to get here.",
              );
              let key = ModuleSourceKey::from_reference(module_reference);
              let source = {
                let data = self.data.borrow();
                let source = data.sources.get(&key).expect("Source had to have been inserted successfully, or recursion would error.");
                v8::Local::new(scope, source).into()
              };
              let resolver = state.resolver.open(scope);
              if resolver.resolve(scope, source).is_none() {
                if scope.is_execution_terminating() {
                  return Err(CoreErrorKind::ExecutionTerminated.into_box());
                }
                return Err(
                  JsErrorBox::generic(
                    "Failed to resolve a source-phase dynamic import",
                  )
                  .into(),
                );
              }
            }
          }
        }
      }
    }

    Ok(())
  }
}
