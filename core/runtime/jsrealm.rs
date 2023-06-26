// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
use super::bindings;
use crate::error::exception_to_err_result;
use crate::modules::ModuleCode;
use crate::ops::OpCtx;
use crate::runtime::JsRuntimeState;
use crate::task::MaskResultAsSend;
use crate::JsRuntime;
use crate::OpId;
use crate::OpResult;
use crate::PromiseId;
use anyhow::Error;
use std::cell::RefCell;
use std::collections::HashSet;
use std::collections::VecDeque;
use std::hash::BuildHasherDefault;
use std::hash::Hasher;
use std::option::Option;
use std::rc::Rc;
use tokio::task::JoinSet;
use v8::HandleScope;
use v8::Local;

// Hasher used for `unrefed_ops`. Since these are rolling i32, there's no
// need to actually hash them.
#[derive(Default)]
pub(crate) struct IdentityHasher(u64);

impl Hasher for IdentityHasher {
  fn write_i32(&mut self, i: i32) {
    self.0 = i as u64;
  }

  fn finish(&self) -> u64 {
    self.0
  }

  fn write(&mut self, _bytes: &[u8]) {
    unreachable!()
  }
}

#[derive(Default)]
pub(crate) struct ContextState {
  pub(crate) js_event_loop_tick_cb: Option<Rc<v8::Global<v8::Function>>>,
  pub(crate) js_build_custom_error_cb: Option<Rc<v8::Global<v8::Function>>>,
  pub(crate) js_promise_reject_cb: Option<Rc<v8::Global<v8::Function>>>,
  pub(crate) js_format_exception_cb: Option<Rc<v8::Global<v8::Function>>>,
  pub(crate) js_wasm_streaming_cb: Option<Rc<v8::Global<v8::Function>>>,
  pub(crate) pending_promise_rejections:
    VecDeque<(v8::Global<v8::Promise>, v8::Global<v8::Value>)>,
  pub(crate) unrefed_ops: HashSet<i32, BuildHasherDefault<IdentityHasher>>,
  pub(crate) pending_ops:
    JoinSet<MaskResultAsSend<(PromiseId, OpId, OpResult)>>,
  // We don't explicitly re-read this prop but need the slice to live alongside
  // the context
  pub(crate) op_ctxs: Box<[OpCtx]>,
  pub(crate) isolate: Option<*mut v8::OwnedIsolate>,
}

/// A representation of a JavaScript realm tied to a [`JsRuntime`], that allows
/// execution in the realm's context.
///
/// A [`JsRealm`] instance is a reference to an already existing realm, which
/// does not hold ownership of it, so instances can be created and dropped as
/// needed. As such, calling [`JsRealm::new`] doesn't create a new realm, and
/// cloning a [`JsRealm`] only creates a new reference. See
/// [`JsRuntime::create_realm`] to create new realms instead.
///
/// Despite [`JsRealm`] instances being references, multiple instances that
/// point to the same realm won't overlap because every operation requires
/// passing a mutable reference to the [`v8::Isolate`]. Therefore, no operation
/// on two [`JsRealm`] instances tied to the same isolate can be run at the same
/// time, regardless of whether they point to the same realm.
///
/// # Panics
///
/// Every method of [`JsRealm`] will panic if you call it with a reference to a
/// [`v8::Isolate`] other than the one that corresponds to the current context.
///
/// In other words, the [`v8::Isolate`] parameter for all the related [`JsRealm`] methods
/// must be extracted from the pre-existing [`JsRuntime`].
///
/// Example usage with the [`JsRealm::execute_script`] method:
/// ```
/// use deno_core::JsRuntime;
/// use deno_core::RuntimeOptions;
///
/// let mut runtime = JsRuntime::new(RuntimeOptions::default());
/// let new_realm = runtime
///         .create_realm()
///         .expect("Handle the error properly");
/// let source_code = "var a = 0; a + 1";
/// let result = new_realm
///         .execute_script_static(runtime.v8_isolate(), "<anon>", source_code)
///         .expect("Handle the error properly");
/// # drop(result);
/// ```
///
/// # Lifetime of the realm
///
/// As long as the corresponding isolate is alive, a [`JsRealm`] instance will
/// keep the underlying V8 context alive even if it would have otherwise been
/// garbage collected.
#[derive(Clone)]
#[repr(transparent)]
pub struct JsRealm(pub(crate) JsRealmInner);

#[derive(Clone)]
pub(crate) struct JsRealmInner {
  context_state: Rc<RefCell<ContextState>>,
  context: Rc<v8::Global<v8::Context>>,
  runtime_state: Rc<RefCell<JsRuntimeState>>,
  is_global: bool,
}

impl JsRealmInner {
  pub(crate) fn new(
    context_state: Rc<RefCell<ContextState>>,
    context: v8::Global<v8::Context>,
    runtime_state: Rc<RefCell<JsRuntimeState>>,
    is_global: bool,
  ) -> Self {
    Self {
      context_state,
      context: context.into(),
      runtime_state,
      is_global,
    }
  }

  pub fn num_pending_ops(&self) -> usize {
    self.context_state.borrow().pending_ops.len()
  }

  pub fn num_unrefed_ops(&self) -> usize {
    self.context_state.borrow().unrefed_ops.len()
  }

  #[inline(always)]
  pub fn context(&self) -> &v8::Global<v8::Context> {
    &self.context
  }

  #[inline(always)]
  pub(crate) fn state(&self) -> Rc<RefCell<ContextState>> {
    self.context_state.clone()
  }

  /// For info on the [`v8::Isolate`] parameter, check [`JsRealm#panics`].
  #[inline(always)]
  pub fn handle_scope<'s>(
    &self,
    isolate: &'s mut v8::Isolate,
  ) -> v8::HandleScope<'s> {
    v8::HandleScope::with_context(isolate, &*self.context)
  }

  pub(crate) fn check_promise_rejections(
    &self,
    scope: &mut v8::HandleScope,
  ) -> Result<(), Error> {
    let Some((_, handle)) = self.context_state.borrow_mut().pending_promise_rejections.pop_front() else {
      return Ok(());
    };

    let exception = v8::Local::new(scope, handle);
    let state_rc = JsRuntime::state_from(scope);
    let state = state_rc.borrow();
    if let Some(inspector) = &state.inspector {
      let inspector = inspector.borrow();
      inspector.exception_thrown(scope, exception, true);
      if inspector.has_blocking_sessions() {
        return Ok(());
      }
    }
    exception_to_err_result(scope, exception, true)
  }

  pub(crate) fn is_same(&self, other: &Rc<v8::Global<v8::Context>>) -> bool {
    Rc::ptr_eq(&self.context, other)
  }

  pub fn destroy(self) {
    let state = self.state();
    let raw_ptr = self.state().borrow().isolate.unwrap();
    // SAFETY: We know the isolate outlives the realm
    let isolate = unsafe { raw_ptr.as_mut().unwrap() };
    let mut realm_state = state.borrow_mut();
    // These globals will prevent snapshots from completing, take them
    std::mem::take(&mut realm_state.js_event_loop_tick_cb);
    std::mem::take(&mut realm_state.js_build_custom_error_cb);
    std::mem::take(&mut realm_state.js_promise_reject_cb);
    std::mem::take(&mut realm_state.js_format_exception_cb);
    std::mem::take(&mut realm_state.js_wasm_streaming_cb);
    // The OpCtx slice may contain a circular reference
    std::mem::take(&mut realm_state.op_ctxs);

    self.context().open(isolate).clear_all_slots(isolate);

    // Expect that this context is dead (we only check this in debug mode)
    // TODO(mmastrac): This check fails for some tests, will need to fix this
    // debug_assert_eq!(Rc::strong_count(&self.context), 1, "Realm was still alive when we wanted to destroy it. Not dropped?");
  }
}

impl JsRealm {
  pub(crate) fn new(inner: JsRealmInner) -> Self {
    Self(inner)
  }

  #[inline(always)]
  pub(crate) fn state_from_scope(
    scope: &mut v8::HandleScope,
  ) -> Rc<RefCell<ContextState>> {
    let context = scope.get_current_context();
    context
      .get_slot::<Rc<RefCell<ContextState>>>(scope)
      .unwrap()
      .clone()
  }

  #[inline(always)]
  pub fn num_pending_ops(&self) -> usize {
    self.0.num_pending_ops()
  }

  #[inline(always)]
  pub fn num_unrefed_ops(&self) -> usize {
    self.0.num_unrefed_ops()
  }

  /// For info on the [`v8::Isolate`] parameter, check [`JsRealm#panics`].
  #[inline(always)]
  pub fn handle_scope<'s>(
    &self,
    isolate: &'s mut v8::Isolate,
  ) -> v8::HandleScope<'s> {
    self.0.handle_scope(isolate)
  }

  #[inline(always)]
  pub fn context(&self) -> &v8::Global<v8::Context> {
    self.0.context()
  }

  /// For info on the [`v8::Isolate`] parameter, check [`JsRealm#panics`].
  pub fn global_object<'s>(
    &self,
    isolate: &'s mut v8::Isolate,
  ) -> v8::Local<'s, v8::Object> {
    let scope = &mut self.0.handle_scope(isolate);
    self.0.context.open(scope).global(scope)
  }

  fn string_from_code<'a>(
    scope: &mut HandleScope<'a>,
    code: &ModuleCode,
  ) -> Option<Local<'a, v8::String>> {
    if let Some(code) = code.try_static_ascii() {
      v8::String::new_external_onebyte_static(scope, code)
    } else {
      v8::String::new_from_utf8(
        scope,
        code.as_bytes(),
        v8::NewStringType::Normal,
      )
    }
  }

  /// Executes traditional JavaScript code (traditional = not ES modules) in the
  /// realm's context.
  ///
  /// For info on the [`v8::Isolate`] parameter, check [`JsRealm#panics`].
  ///
  /// The `name` parameter can be a filepath or any other string. E.g.:
  ///
  ///   - "/some/file/path.js"
  ///   - "<anon>"
  ///   - "[native code]"
  ///
  /// The same `name` value can be used for multiple executions.
  ///
  /// `Error` can usually be downcast to `JsError`.
  pub fn execute_script_static(
    &self,
    isolate: &mut v8::Isolate,
    name: &'static str,
    source_code: &'static str,
  ) -> Result<v8::Global<v8::Value>, Error> {
    self.execute_script(isolate, name, ModuleCode::from_static(source_code))
  }

  /// Executes traditional JavaScript code (traditional = not ES modules) in the
  /// realm's context.
  ///
  /// For info on the [`v8::Isolate`] parameter, check [`JsRealm#panics`].
  ///
  /// The `name` parameter can be a filepath or any other string. E.g.:
  ///
  ///   - "/some/file/path.js"
  ///   - "<anon>"
  ///   - "[native code]"
  ///
  /// The same `name` value can be used for multiple executions.
  ///
  /// `Error` can usually be downcast to `JsError`.
  pub fn execute_script(
    &self,
    isolate: &mut v8::Isolate,
    name: &'static str,
    source_code: ModuleCode,
  ) -> Result<v8::Global<v8::Value>, Error> {
    let scope = &mut self.0.handle_scope(isolate);

    let source = Self::string_from_code(scope, &source_code).unwrap();
    debug_assert!(name.is_ascii());
    let name =
      v8::String::new_external_onebyte_static(scope, name.as_bytes()).unwrap();
    let origin = bindings::script_origin(scope, name);

    let tc_scope = &mut v8::TryCatch::new(scope);

    let script = match v8::Script::compile(tc_scope, source, Some(&origin)) {
      Some(script) => script,
      None => {
        let exception = tc_scope.exception().unwrap();
        return exception_to_err_result(tc_scope, exception, false);
      }
    };

    match script.run(tc_scope) {
      Some(value) => {
        let value_handle = v8::Global::new(tc_scope, value);
        Ok(value_handle)
      }
      None => {
        assert!(tc_scope.has_caught());
        let exception = tc_scope.exception().unwrap();
        exception_to_err_result(tc_scope, exception, false)
      }
    }
  }

  // TODO(andreubotella): `mod_evaluate`, `load_main_module`, `load_side_module`
}

impl Drop for JsRealm {
  fn drop(&mut self) {
    // Don't do anything special with the global realm
    if self.0.is_global {
      return;
    }

    // There's us and there's the runtime
    if Rc::strong_count(&self.0.context) == 2 {
      self
        .0
        .runtime_state
        .borrow_mut()
        .remove_realm(&self.0.context);
      assert_eq!(Rc::strong_count(&self.0.context), 1);
      self.0.clone().destroy();
      assert_eq!(Rc::strong_count(&self.0.context_state), 1);
    }
  }
}
