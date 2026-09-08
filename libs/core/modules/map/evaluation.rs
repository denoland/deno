// Copyright 2018-2026 the Deno authors. MIT license.

use std::rc::Rc;

use deno_core::error::CoreError;
use futures::FutureExt;
use futures::future::Either;
use tokio::sync::oneshot;
use v8::Function;
use v8::PromiseState;

use super::ModuleMap;
use crate::error::CoreErrorKind;
use crate::error::JsError;
use crate::error::exception_to_err_result;
use crate::modules::ModuleId;
use crate::runtime::JsRealm;

struct ModEvaluate {
  module_map: Rc<ModuleMap>,
  sender: Option<oneshot::Sender<Result<(), Box<JsError>>>>,
  module: Option<v8::Global<v8::Module>>,
  notify: Vec<v8::Global<v8::Function>>,
}

impl ModEvaluate {
  fn notify(&mut self, scope: &mut v8::PinScope) {
    if !self.notify.is_empty() {
      let module = v8::Local::new(scope, self.module.take().unwrap());
      let ns = module.get_module_namespace();
      let recv = v8::undefined(scope).into();
      let args = &[ns];
      for notify in std::mem::take(&mut self.notify).into_iter() {
        let notify = v8::Local::new(scope, notify);
        notify.call(scope, recv, args);
      }
    }
    _ = self.sender.take().unwrap().send(Ok(()));
  }
}

impl ModuleMap {
  pub(crate) fn has_pending_module_evaluation(&self) -> bool {
    self.pending_mod_evaluation.get()
  }
  /// See [`JsRuntime::mod_evaluate`].
  pub fn mod_evaluate<'s, 'i>(
    self: &Rc<Self>,
    scope: &mut v8::PinScope<'s, 'i>,
    id: ModuleId,
  ) -> impl Future<Output = Result<(), CoreError>> + use<> {
    v8::tc_scope!(tc_scope, scope);

    let module = self
      .get_handle(id)
      .map(|handle| v8::Local::new(tc_scope, handle))
      .expect("ModuleInfo not found");
    let mut status = module.get_status();

    // If the module is already evaluated, return early as there's nothing to do
    if status == v8::ModuleStatus::Evaluated {
      return Either::Left(futures::future::ready(Ok(())));
    }

    assert_eq!(
      status,
      v8::ModuleStatus::Instantiated,
      "Module not instantiated: {} ({})",
      self.get_name_by_id(id).unwrap(),
      id,
    );

    let (sender, receiver) = oneshot::channel::<Result<_, Box<JsError>>>();
    let receiver = receiver.map(|res| {
      res
        .map(|r| r.map_err(|r| CoreErrorKind::Js(r).into_box()))
        .unwrap_or_else(|_| Err(CoreErrorKind::ExecutionTerminated.into_box()))
    });

    self.evaluating_top_level.set(true);
    let Some(value) = module.evaluate(tc_scope) else {
      self.evaluating_top_level.set(false);
      if tc_scope.has_terminated() || tc_scope.is_execution_terminating() {
        let undefined = v8::undefined(tc_scope).into();
        _ = sender
          .send(exception_to_err_result(tc_scope, undefined, true, false));
      } else {
        debug_assert_eq!(module.get_status(), v8::ModuleStatus::Errored);
      }
      return Either::Right(receiver);
    };
    self.evaluating_top_level.set(false);

    self.pending_mod_evaluation.set(true);

    // Update status after evaluating.
    status = module.get_status();

    if self.exception_state.has_dispatched_exception() {
      // This will be overridden in `exception_to_err_result()`.
      let exception = v8::undefined(tc_scope).into();
      sender
        .send(exception_to_err_result(tc_scope, exception, true, false))
        .expect("Failed to send module evaluation error.");
    } else {
      debug_assert!(
        status == v8::ModuleStatus::Evaluated
          || status == v8::ModuleStatus::Errored
      );
      let promise = v8::Local::<v8::Promise>::try_from(value)
        .expect("Expected to get promise as module evaluation result");

      // If this is a main module, claim the main module notification functions
      let (notify, module) = if self.is_main_module_id(id) {
        let module = Some(v8::Global::new(tc_scope, module));
        (
          std::mem::take(&mut self.data.borrow_mut().main_module_callbacks),
          module,
        )
      } else {
        (vec![], None)
      };

      // Create a ModEvaluate instance and stash it in an external
      let evaluation = v8::External::new(
        tc_scope,
        Box::into_raw(Box::new(ModEvaluate {
          module_map: self.clone(),
          sender: Some(sender),
          notify,
          module,
        })) as _,
      );

      fn get_sender(arg: v8::Local<v8::Value>) -> ModEvaluate {
        let sender = v8::Local::<v8::External>::try_from(arg).unwrap();
        *unsafe { Box::from_raw(sender.value() as _) }
      }

      let on_fulfilled = Function::builder(
        |scope: &mut v8::PinScope<'_, '_>,
         args: v8::FunctionCallbackArguments<'_>,
         _rv: v8::ReturnValue| {
          let mut sender = get_sender(args.data());
          sender.module_map.pending_mod_evaluation.set(false);
          sender.module_map.module_waker.wake();
          sender.notify(scope);
        },
      )
      .data(evaluation.into())
      .build(tc_scope);

      let on_rejected = Function::builder(
        |scope: &mut v8::PinScope<'_, '_>,
         args: v8::FunctionCallbackArguments<'_>,
         _rv: v8::ReturnValue| {
          let mut sender = get_sender(args.data());
          sender.module_map.pending_mod_evaluation.set(false);
          sender.module_map.module_waker.wake();
          _ = sender.sender.take().unwrap().send(Ok(()));
          scope.throw_exception(args.get(0));
        },
      )
      .data(evaluation.into())
      .build(tc_scope);

      // V8 GC roots all promises, so we don't need to worry about it after this
      // then2 will return None if the runtime is shutting down
      if on_fulfilled.is_none()
        || on_rejected.is_none()
        || promise
          .then2(tc_scope, on_fulfilled.unwrap(), on_rejected.unwrap())
          .is_none()
      {
        // There are two reasons we could be here:
        // 1. The runtime is shutting down, and JS ops are disabled with termination exceptions.
        // 2. User code has tampered with the runtime globals in some way that prevents us from
        //    attaching `on_fulfilled`/`on_rejected` to `promise`.
        // In these cases we still need to report something back, so synthesize the result from the
        // promise.

        // Unset pending mod evaluation as the handlers will never run. See debug_assert below.
        self.pending_mod_evaluation.set(false);

        let mut sender = get_sender(evaluation.into());
        match promise.state() {
          PromiseState::Fulfilled => {
            if let Some(exception) = tc_scope.exception() {
              _ = sender.sender.take().unwrap().send(exception_to_err_result(
                tc_scope, exception, true, false,
              ));
            } else {
              // Module loaded OK
              sender.notify(tc_scope);
            }
          }
          PromiseState::Rejected => {
            // Module was rejected
            let err = promise.result(tc_scope);
            let err = JsError::from_v8_exception(tc_scope, err);
            _ = sender.sender.take().unwrap().send(Err(err));
          }
          PromiseState::Pending => {
            // User code shouldn't be able to both cause the runtime to fail and leave the promise as
            // pending because the only way to adopt a pending promise is to use `await` and
            // `await` won't work if you've broken the runtime in such a way that `promise::then`
            // didn't work.
            debug_assert!(tc_scope.is_execution_terminating());
            // Module pending, just drop the sender at this point -- we can't do anything with a shut-down runtime.
            drop(sender);
          }
        }
      }

      // Under Explicit microtask policy, run the module-evaluation
      // checkpoint here. This matches Node's ESM ordering: Promise and
      // queueMicrotask jobs queued during top-level module evaluation run
      // before process.nextTick callbacks queued in the same evaluation.
      //
      // For async module graphs (with TLA), this checkpoint is also critical
      // for draining V8-internal TLA resume microtasks. If skipped, the
      // evaluation promise may never resolve because V8's internal async
      // module evaluation state machine relies on these microtasks being
      // processed.
      tc_scope.perform_microtask_checkpoint();
    }

    Either::Right(receiver)
  }

  /// Helper function that allows to evaluate a module and ensure it's fully
  /// evaluated without the need to poll the event loop.
  ///
  /// This is useful for evaluating internal modules that can't use Top-Level Await.
  pub(crate) fn mod_evaluate_sync(
    self: &Rc<Self>,
    scope: &mut v8::PinScope,
    id: ModuleId,
  ) -> Result<(), CoreError> {
    v8::tc_scope!(let tc_scope, scope);

    let module = self
      .get_handle(id)
      .map(|handle| v8::Local::new(tc_scope, handle))
      .expect("ModuleInfo not found");
    let status = module.get_status();

    // If the module is already evaluated, return early as there's nothing to do
    if status == v8::ModuleStatus::Evaluated {
      return Ok(());
    }

    assert_eq!(
      status,
      v8::ModuleStatus::Instantiated,
      "Module not instantiated: {} ({})",
      self.get_name_by_id(id).unwrap(),
      id,
    );

    if module.is_graph_async() {
      return Err(CoreErrorKind::TLA.into_box());
    }

    let Some(value) = module.evaluate(tc_scope) else {
      if tc_scope.has_terminated() || tc_scope.is_execution_terminating() {
        return Err(CoreErrorKind::ExecutionTerminated.into_box());
      }
      let exception = tc_scope.exception().unwrap();
      return Err(
        CoreErrorKind::Js(JsError::from_v8_exception(tc_scope, exception))
          .into_box(),
      );
    };
    if tc_scope.has_terminated() || tc_scope.is_execution_terminating() {
      return Err(CoreErrorKind::ExecutionTerminated.into_box());
    }

    // Under Explicit microtask policy, V8 won't drain microtasks after
    // module.evaluate(). We must do it ourselves so that the module
    // evaluation promise resolves for synchronous modules.
    //
    // However, skip the checkpoint when we are inside a top-level
    // `module.evaluate()` call (i.e. `evaluating_top_level` is set), e.g.
    // when a CJS `require()` of an ES module fires while V8 is evaluating
    // an async module graph. Draining microtasks at that point can resume
    // a suspended TLA dependency while its parent module is still in the
    // Evaluating state on the stack; V8 then cannot propagate the
    // completion to the parent and the graph's evaluation promise stays
    // Pending forever. The module evaluated here has a synchronous graph
    // (checked above), so its promise settles without a checkpoint.
    if !self.evaluating_top_level.get() {
      tc_scope.perform_microtask_checkpoint();
      if tc_scope.has_terminated() || tc_scope.is_execution_terminating() {
        return Err(CoreErrorKind::ExecutionTerminated.into_box());
      }
    }

    if let Some(exception) = tc_scope.exception() {
      return Err(
        CoreErrorKind::Js(JsError::from_v8_exception(tc_scope, exception))
          .into_box(),
      );
    }

    let status = module.get_status();
    debug_assert!(
      status == v8::ModuleStatus::Evaluated
        || status == v8::ModuleStatus::Errored
    );
    let promise = v8::Local::<v8::Promise>::try_from(value)
      .expect("Expected to get promise as module evaluation result");

    promise.mark_as_handled();

    match promise.state() {
      PromiseState::Fulfilled => Ok(()),
      PromiseState::Rejected => {
        let err = promise.result(tc_scope);

        let exception_state = JsRealm::exception_state_from_scope(tc_scope);
        // TODO: remove after crrev.com/c/7595271
        exception_state.track_promise_rejection(
          tc_scope,
          promise,
          v8::PromiseRejectEvent::PromiseHandlerAddedAfterReject,
          None,
        );

        Err(
          CoreErrorKind::Js(JsError::from_v8_exception(tc_scope, err))
            .into_box(),
        )
      }
      PromiseState::Pending => {
        unreachable!()
      }
    }
  }

  fn get_stalled_top_level_await_message_for_module(
    &self,
    scope: &mut v8::PinScope,
    module_id: ModuleId,
  ) -> Vec<v8::Global<v8::Message>> {
    let data = self.data.borrow();
    let module_handle = data.handles.get(module_id).unwrap();

    let module = v8::Local::new(scope, module_handle);
    // v8::Module::GetStalledTopLevelAwaitMessage() must not be called on
    // a synthetic module.
    if module.is_synthetic_module() {
      return vec![];
    }

    let stalled = module.get_stalled_top_level_await_message(scope);
    let mut messages = vec![];
    for (_, message) in stalled {
      messages.push(v8::Global::new(scope, message));
    }
    messages
  }

  pub(crate) fn find_stalled_top_level_await(
    &self,
    scope: &mut v8::PinScope,
  ) -> Vec<v8::Global<v8::Message>> {
    // First check if that's root module
    let root_module_id = self
      .data
      .borrow()
      .info
      .iter()
      .filter(|m| m.main)
      .map(|m| m.id)
      .next();

    if let Some(root_module_id) = root_module_id {
      let messages = self
        .get_stalled_top_level_await_message_for_module(scope, root_module_id);
      if !messages.is_empty() {
        return messages;
      }
    }

    // It wasn't a top module, so iterate over all modules and try to find
    // any with stalled top level await
    for module_id in 0..self.data.borrow().handles.len() {
      let messages =
        self.get_stalled_top_level_await_message_for_module(scope, module_id);
      if !messages.is_empty() {
        return messages;
      }
    }

    vec![]
  }
}
