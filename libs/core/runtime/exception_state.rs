// Copyright 2018-2025 the Deno authors. MIT license.

use crate::error::JsError;
use crate::error::exception_to_err_result;
use std::cell::Cell;
use std::cell::RefCell;
use std::collections::VecDeque;

#[derive(Default)]
pub(crate) struct ExceptionState {
  // TODO(nayeemrmn): This is polled in `exception_to_err_result()` which is
  // flimsy. Try to poll it similarly to `pending_promise_rejections`.
  dispatched_exception: Cell<Option<v8::Global<v8::Value>>>,
  dispatched_exception_is_promise: Cell<bool>,
  #[allow(clippy::type_complexity)]
  pub(crate) pending_promise_rejections: RefCell<
    VecDeque<(
      v8::Global<v8::Promise>,
      v8::Global<v8::Value>,
      v8::Global<v8::Value>,
    )>,
  >,
  pub(crate) pending_handled_promise_rejections:
    RefCell<VecDeque<(v8::Global<v8::Promise>, v8::Global<v8::Value>)>>,
  pub(crate) js_build_custom_error_cb:
    RefCell<Option<v8::Global<v8::Function>>>,
  pub(crate) js_handled_promise_rejection_cb:
    RefCell<Option<v8::Global<v8::Function>>>,
  pub(crate) js_format_exception_cb: RefCell<Option<v8::Global<v8::Function>>>,
}

impl ExceptionState {
  /// Clear all the associated v8 objects to prepare for this isolate to be torn down, either for
  /// a snapshot or for process termination purposes.
  ///
  /// The [`ExceptionState`] is not considered valid after this operation and should not be used.
  /// It generally will not live long after this, however.
  pub(crate) fn prepare_to_destroy(&self) {
    // TODO(mmastrac): we can probably move this to Drop eventually
    self.js_build_custom_error_cb.borrow_mut().take();
    self.js_handled_promise_rejection_cb.borrow_mut().take();
    self.js_format_exception_cb.borrow_mut().take();
    self.pending_promise_rejections.borrow_mut().clear();
    self.dispatched_exception.set(None);
  }

  pub(crate) fn clear_error(&self) {
    self.dispatched_exception_is_promise.set(false);
    self.dispatched_exception.set(None);
  }

  pub(crate) fn has_dispatched_exception(&self) -> bool {
    // SAFETY: we limit access to this cell to this method only
    unsafe {
      self
        .dispatched_exception
        .as_ptr()
        .as_ref()
        .unwrap_unchecked()
        .is_some()
    }
  }

  pub(crate) fn set_dispatched_exception(
    &self,
    exception: v8::Global<v8::Value>,
    promise: bool,
  ) {
    self.dispatched_exception.set(Some(exception));
    self.dispatched_exception_is_promise.set(promise);
  }

  /// If there is an exception condition (ie: an unhandled promise rejection or exception, or
  /// the runtime is shut down), returns it from here. If not, returns `Ok`.
  pub(crate) fn check_exception_condition(
    &self,
    scope: &mut v8::PinScope,
  ) -> Result<(), Box<JsError>> {
    if self.has_dispatched_exception() {
      let undefined = v8::undefined(scope);
      exception_to_err_result(
        scope,
        undefined.into(),
        self.dispatched_exception_is_promise.get(),
        true,
      )
    } else {
      Ok(())
    }
  }

  pub(crate) fn is_dispatched_exception_promise(&self) -> bool {
    self.dispatched_exception_is_promise.get()
  }

  pub(crate) fn get_dispatched_exception_as_local<'s, 'i>(
    &self,
    scope: &mut v8::PinScope<'s, 'i>,
  ) -> Option<v8::Local<'s, v8::Value>> {
    // SAFETY: we limit access to this cell to this method only
    unsafe {
      self
        .dispatched_exception
        .as_ptr()
        .as_ref()
        .unwrap_unchecked()
    }
    .as_ref()
    .map(|global| v8::Local::new(scope, global))
  }

  /// Tracks this promise rejection until we have a chance to give it to the unhandled promise rejection handler.
  /// This performs the role of `HostPromiseRejectionTracker` from https://262.ecma-international.org/14.0/#sec-host-promise-rejection-tracker.
  ///
  /// Notes from ECMAScript's `HostPromiseRejectionTracker` operation:
  ///
  /// - HostPromiseRejectionTracker is called with the operation argument set to "reject" when a promise is rejected
  ///   without any handlers, or "handle" when a handler is added to a previously rejected promise for the first time.
  /// - Host environments can use this operation to track promise rejections without causing abrupt completion.
  /// - Implementations may notify developers of unhandled rejections and invalidate notifications if new handlers are attached.
  /// - If operation is "handle", an implementation should not hold a reference to promise in a way that would
  ///   interfere with garbage collection.
  /// - An implementation may hold a reference to promise if operation is "reject", since it is expected that rejections
  ///   will be rare and not on hot code paths.
  pub fn track_promise_rejection<'s, 'i>(
    &self,
    scope: &mut v8::PinScope<'s, 'i>,
    promise: v8::Local<v8::Promise>,
    event: v8::PromiseRejectEvent,
    rejection_value: Option<v8::Local<v8::Value>>,
  ) {
    use v8::PromiseRejectEvent::*;
    let promise_global = v8::Global::new(scope, promise);
    match event {
      PromiseRejectWithNoHandler => {
        let error = rejection_value.unwrap();
        let error_global = v8::Global::new(scope, error);
        let async_context = scope.get_continuation_preserved_embedder_data();
        let async_context_global = v8::Global::new(scope, async_context);
        self.pending_promise_rejections.borrow_mut().push_back((
          promise_global,
          error_global,
          async_context_global,
        ));
      }
      PromiseHandlerAddedAfterReject => {
        // The code has until the event loop yields to attach a handler and avoid an unhandled rejection
        // event. If we haven't delivered an unhandled exception event yet, we search for the old promise
        // in this list and remove it. If it doesn't exist, that means it was already "handled as unhandled"
        // and we need to fire a rejectionhandled event.
        let mut rejections = self.pending_promise_rejections.borrow_mut();
        let previous_len = rejections.len();
        rejections.retain(|(key, _, _)| key != &promise_global);
        if rejections.len() == previous_len {
          // Don't hold the lock while we go back into v8
          drop(rejections);
          // The unhandled rejection was already delivered, so this means we need to deliver a
          // "rejectionhandled" event if anyone cares.
          if self.js_handled_promise_rejection_cb.borrow().is_some() {
            let error = promise.result(scope);
            let error_global = v8::Global::new(scope, error);
            self
              .pending_handled_promise_rejections
              .borrow_mut()
              .push_back((promise_global, error_global));
          }
        }
      }
      PromiseRejectAfterResolved => {}
      PromiseResolveAfterResolved => {
        // Should not warn. See #1272
      }
    }
  }
}
