// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

// This module provides higher level implementation of CoreIsolate that
// supports asynchronous loading and executution of ES Modules.
// The isolate.rs should never depend on this module.

use rusty_v8 as v8;

use crate::bindings;
use crate::errors::ErrBox;
use crate::errors::ErrWithV8Handle;
use crate::futures::FutureExt;
use futures::ready;
use futures::stream::FuturesUnordered;
use futures::stream::StreamExt;
use futures::stream::StreamFuture;
use futures::task::AtomicWaker;
use futures::Future;
use std::cell::RefCell;
use std::collections::HashMap;
use std::convert::TryFrom;
use std::ops::{Deref, DerefMut};
use std::option::Option;
use std::pin::Pin;
use std::rc::Rc;
use std::task::Context;
use std::task::Poll;

use crate::core_isolate::exception_to_err_result;
use crate::errors::attach_handle_to_error;
use crate::module_specifier::ModuleSpecifier;
use crate::modules::LoadState;
use crate::modules::ModuleId;
use crate::modules::ModuleLoadId;
use crate::modules::ModuleLoader;
use crate::modules::ModuleSource;
use crate::modules::Modules;
use crate::modules::PrepareLoadFuture;
use crate::modules::RecursiveModuleLoad;
use crate::CoreIsolate;
use crate::StartupData;

/// More specialized version of `CoreIsolate` that provides loading
/// and execution of ES Modules.
///
/// Creating `EsIsolate` requires to pass `loader` argument
/// that implements `ModuleLoader` trait - that way actual resolution and
/// loading of modules can be customized by the implementor.
pub struct EsIsolate(CoreIsolate);

pub struct EsIsolateState {
  loader: Rc<dyn ModuleLoader>,
  pub modules: Modules,
  pub(crate) dyn_import_map:
    HashMap<ModuleLoadId, v8::Global<v8::PromiseResolver>>,

  preparing_dyn_imports: FuturesUnordered<Pin<Box<PrepareLoadFuture>>>,
  pending_dyn_imports: FuturesUnordered<StreamFuture<RecursiveModuleLoad>>,
  waker: AtomicWaker,
}

impl Deref for EsIsolate {
  type Target = CoreIsolate;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl DerefMut for EsIsolate {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.0
  }
}

impl EsIsolate {
  pub fn new(
    loader: Rc<dyn ModuleLoader>,
    startup_data: StartupData,
    will_snapshot: bool,
  ) -> Self {
    let mut core_isolate = CoreIsolate::new(startup_data, will_snapshot);
    {
      core_isolate.set_host_initialize_import_meta_object_callback(
        bindings::host_initialize_import_meta_object_callback,
      );
      core_isolate.set_host_import_module_dynamically_callback(
        bindings::host_import_module_dynamically_callback,
      );
    }

    core_isolate.set_slot(Rc::new(RefCell::new(EsIsolateState {
      modules: Modules::new(),
      loader,
      dyn_import_map: HashMap::new(),
      preparing_dyn_imports: FuturesUnordered::new(),
      pending_dyn_imports: FuturesUnordered::new(),
      waker: AtomicWaker::new(),
    })));

    EsIsolate(core_isolate)
  }

  /// Low-level module creation.
  ///
  /// Called during module loading or dynamic import loading.
  fn mod_new(
    &mut self,
    main: bool,
    name: &str,
    source: &str,
  ) -> Result<ModuleId, ErrBox> {
    let state_rc = Self::state(self);

    let core_state_rc = CoreIsolate::state(self);
    let core_state = core_state_rc.borrow();
    let mut hs = v8::HandleScope::new(&mut self.0);
    let scope = hs.enter();
    let context = core_state.global_context.get(scope).unwrap();
    let mut cs = v8::ContextScope::new(scope, context);
    let scope = cs.enter();

    let name_str = v8::String::new(scope, name).unwrap();
    let source_str = v8::String::new(scope, source).unwrap();

    let origin = bindings::module_origin(scope, name_str);
    let source = v8::script_compiler::Source::new(source_str, &origin);

    let mut try_catch = v8::TryCatch::new(scope);
    let tc = try_catch.enter();

    let maybe_module = v8::script_compiler::compile_module(scope, source);

    if tc.has_caught() {
      assert!(maybe_module.is_none());
      let e = tc.exception(scope).unwrap();
      return exception_to_err_result(scope, e);
    }

    let module = maybe_module.unwrap();
    let id = module.get_identity_hash();

    let mut import_specifiers: Vec<ModuleSpecifier> = vec![];
    for i in 0..module.get_module_requests_length() {
      let import_specifier =
        module.get_module_request(i).to_rust_string_lossy(scope);
      let state = state_rc.borrow();
      let module_specifier =
        state.loader.resolve(&import_specifier, name, false)?;
      import_specifiers.push(module_specifier);
    }

    let mut handle = v8::Global::<v8::Module>::new();
    handle.set(scope, module);

    {
      let mut state = state_rc.borrow_mut();
      state
        .modules
        .register(id, name, main, handle, import_specifiers);
    }
    Ok(id)
  }

  /// Instantiates a ES module
  ///
  /// ErrBox can be downcast to a type that exposes additional information about
  /// the V8 exception. By default this type is JSError, however it may be a
  /// different type if CoreIsolate::set_js_error_create_fn() has been used.
  fn mod_instantiate(&mut self, id: ModuleId) -> Result<(), ErrBox> {
    let state_rc = Self::state(self);
    let state = state_rc.borrow();

    let core_state_rc = CoreIsolate::state(self);
    let core_state = core_state_rc.borrow();
    let mut hs = v8::HandleScope::new(&mut self.0);
    let scope = hs.enter();
    let context = core_state.global_context.get(scope).unwrap();
    let mut cs = v8::ContextScope::new(scope, context);
    let scope = cs.enter();

    let mut try_catch = v8::TryCatch::new(scope);
    let tc = try_catch.enter();

    let module_info = match state.modules.get_info(id) {
      Some(info) => info,
      None if id == 0 => return Ok(()),
      _ => panic!("module id {} not found in module table", id),
    };
    let mut module = module_info.handle.get(scope).unwrap();
    drop(state);

    if module.get_status() == v8::ModuleStatus::Errored {
      exception_to_err_result(scope, module.get_exception())?
    }

    let result =
      module.instantiate_module(context, bindings::module_resolve_callback);
    match result {
      Some(_) => Ok(()),
      None => {
        let exception = tc.exception(scope).unwrap();
        exception_to_err_result(scope, exception)
      }
    }
  }

  /// Evaluates an already instantiated ES module.
  ///
  /// ErrBox can be downcast to a type that exposes additional information about
  /// the V8 exception. By default this type is JSError, however it may be a
  /// different type if CoreIsolate::set_js_error_create_fn() has been used.
  pub fn mod_evaluate(&mut self, id: ModuleId) -> Result<(), ErrBox> {
    self.shared_init();
    let state_rc = Self::state(self);
    let state = state_rc.borrow();

    let core_state_rc = CoreIsolate::state(self);

    let mut hs = v8::HandleScope::new(&mut self.0);
    let scope = hs.enter();
    let context = {
      let core_state = core_state_rc.borrow();
      core_state.global_context.get(scope).unwrap()
    };
    let mut cs = v8::ContextScope::new(scope, context);
    let scope = cs.enter();

    let info = state.modules.get_info(id).expect("ModuleInfo not found");
    let module = info.handle.get(scope).expect("Empty module handle");
    let mut status = module.get_status();
    drop(state);
    if status == v8::ModuleStatus::Instantiated {
      // IMPORTANT: Top-level-await is enabled, which means that return value
      // of module evaluation is a promise.
      //
      // Because that promise is created internally by V8, when error occurs during
      // module evaluation the promise is rejected, and since the promise has no rejection
      // handler it will result in call to `bindings::promise_reject_callback` adding
      // the promise to pending promise rejection table - meaning Isolate will return
      // error on next poll().
      //
      // This situation is not desirable as we want to manually return error at the
      // end of this function to handle it further. It means we need to manually
      // remove this promise from pending promise rejection table.
      //
      // For more details see:
      // https://github.com/denoland/deno/issues/4908
      // https://v8.dev/features/top-level-await#module-execution-order
      let maybe_value = module.evaluate(scope, context);

      // Update status after evaluating.
      status = module.get_status();

      if let Some(value) = maybe_value {
        assert!(
          status == v8::ModuleStatus::Evaluated
            || status == v8::ModuleStatus::Errored
        );
        let promise = v8::Local::<v8::Promise>::try_from(value)
          .expect("Expected to get promise as module evaluation result");
        let promise_id = promise.get_identity_hash();
        let mut core_state = core_state_rc.borrow_mut();
        if let Some(mut handle) =
          core_state.pending_promise_exceptions.remove(&promise_id)
        {
          handle.reset(scope);
        }
      } else {
        assert!(status == v8::ModuleStatus::Errored);
      }
    }

    match status {
      v8::ModuleStatus::Evaluated => Ok(()),
      v8::ModuleStatus::Errored => {
        let exception = module.get_exception();
        exception_to_err_result(scope, exception)
          .map_err(|err| attach_handle_to_error(scope, err, exception))
      }
      other => panic!("Unexpected module status {:?}", other),
    }
  }

  fn dyn_import_error(
    &mut self,
    id: ModuleLoadId,
    err: ErrBox,
  ) -> Result<(), ErrBox> {
    let state_rc = Self::state(self);
    let mut state = state_rc.borrow_mut();

    let core_state_rc = CoreIsolate::state(self);
    let core_state = core_state_rc.borrow();

    let mut hs = v8::HandleScope::new(&mut self.0);
    let scope = hs.enter();
    let context = core_state.global_context.get(scope).unwrap();
    let mut cs = v8::ContextScope::new(scope, context);
    let scope = cs.enter();

    drop(core_state);

    let mut resolver_handle = state
      .dyn_import_map
      .remove(&id)
      .expect("Invalid dyn import id");
    let resolver = resolver_handle.get(scope).unwrap();
    resolver_handle.reset(scope);

    drop(state);

    let exception = err
      .downcast_ref::<ErrWithV8Handle>()
      .and_then(|err| err.get_handle().get(scope))
      .unwrap_or_else(|| {
        let message = err.to_string();
        let message = v8::String::new(scope, &message).unwrap();
        v8::Exception::type_error(scope, message)
      });

    resolver.reject(context, exception).unwrap();
    scope.isolate().run_microtasks();
    Ok(())
  }

  fn dyn_import_done(
    &mut self,
    id: ModuleLoadId,
    mod_id: ModuleId,
  ) -> Result<(), ErrBox> {
    let state_rc = Self::state(self);

    let core_state_rc = CoreIsolate::state(self);

    debug!("dyn_import_done {} {:?}", id, mod_id);
    assert!(mod_id != 0);
    let mut hs = v8::HandleScope::new(&mut self.0);
    let scope = hs.enter();
    let context = {
      let core_state = core_state_rc.borrow();
      core_state.global_context.get(scope).unwrap()
    };
    let mut cs = v8::ContextScope::new(scope, context);
    let scope = cs.enter();

    let mut resolver_handle = {
      let mut state = state_rc.borrow_mut();
      state
        .dyn_import_map
        .remove(&id)
        .expect("Invalid dyn import id")
    };
    let resolver = resolver_handle.get(scope).unwrap();
    resolver_handle.reset(scope);

    let module = {
      let state = state_rc.borrow();
      let info = state
        .modules
        .get_info(mod_id)
        .expect("Dyn import module info not found");
      // Resolution success
      info.handle.get(scope).unwrap()
    };
    assert_eq!(module.get_status(), v8::ModuleStatus::Evaluated);

    let module_namespace = module.get_module_namespace();
    resolver.resolve(context, module_namespace).unwrap();
    scope.isolate().run_microtasks();
    Ok(())
  }

  fn prepare_dyn_imports(
    &mut self,
    cx: &mut Context,
  ) -> Poll<Result<(), ErrBox>> {
    let state_rc = Self::state(self);

    loop {
      let r = {
        let mut state = state_rc.borrow_mut();
        state.preparing_dyn_imports.poll_next_unpin(cx)
      };
      match r {
        Poll::Pending | Poll::Ready(None) => {
          // There are no active dynamic import loaders, or none are ready.
          return Poll::Ready(Ok(()));
        }
        Poll::Ready(Some(prepare_poll)) => {
          let dyn_import_id = prepare_poll.0;
          let prepare_result = prepare_poll.1;

          match prepare_result {
            Ok(load) => {
              let state = state_rc.borrow_mut();
              state.pending_dyn_imports.push(load.into_future());
            }
            Err(err) => {
              self.dyn_import_error(dyn_import_id, err)?;
            }
          }
        }
      }
    }
  }

  fn poll_dyn_imports(&mut self, cx: &mut Context) -> Poll<Result<(), ErrBox>> {
    let state_rc = Self::state(self);
    loop {
      let poll_result = {
        let mut state = state_rc.borrow_mut();
        state.pending_dyn_imports.poll_next_unpin(cx)
      };

      match poll_result {
        Poll::Pending | Poll::Ready(None) => {
          // There are no active dynamic import loaders, or none are ready.
          return Poll::Ready(Ok(()));
        }
        Poll::Ready(Some(load_stream_poll)) => {
          let maybe_result = load_stream_poll.0;
          let mut load = load_stream_poll.1;
          let dyn_import_id = load.id;

          if let Some(load_stream_result) = maybe_result {
            match load_stream_result {
              Ok(info) => {
                // A module (not necessarily the one dynamically imported) has been
                // fetched. Create and register it, and if successful, poll for the
                // next recursive-load event related to this dynamic import.
                match self.register_during_load(info, &mut load) {
                  Ok(()) => {
                    // Keep importing until it's fully drained
                    let state = state_rc.borrow_mut();
                    state.pending_dyn_imports.push(load.into_future());
                  }
                  Err(err) => self.dyn_import_error(dyn_import_id, err)?,
                }
              }
              Err(err) => {
                // A non-javascript error occurred; this could be due to a an invalid
                // module specifier, or a problem with the source map, or a failure
                // to fetch the module source code.
                self.dyn_import_error(dyn_import_id, err)?
              }
            }
          } else {
            // The top-level module from a dynamic import has been instantiated.
            // Load is done.
            let module_id = load.root_module_id.unwrap();
            self.mod_instantiate(module_id)?;
            match self.mod_evaluate(module_id) {
              Ok(()) => self.dyn_import_done(dyn_import_id, module_id)?,
              Err(err) => self.dyn_import_error(dyn_import_id, err)?,
            };
          }
        }
      }
    }
  }

  fn register_during_load(
    &mut self,
    info: ModuleSource,
    load: &mut RecursiveModuleLoad,
  ) -> Result<(), ErrBox> {
    let ModuleSource {
      code,
      module_url_specified,
      module_url_found,
    } = info;

    let is_main =
      load.state == LoadState::LoadingRoot && !load.is_dynamic_import();
    let referrer_specifier =
      ModuleSpecifier::resolve_url(&module_url_found).unwrap();

    let state_rc = Self::state(self);
    // #A There are 3 cases to handle at this moment:
    // 1. Source code resolved result have the same module name as requested
    //    and is not yet registered
    //     -> register
    // 2. Source code resolved result have a different name as requested:
    //   2a. The module with resolved module name has been registered
    //     -> alias
    //   2b. The module with resolved module name has not yet been registered
    //     -> register & alias

    // If necessary, register an alias.
    if module_url_specified != module_url_found {
      let mut state = state_rc.borrow_mut();
      state
        .modules
        .alias(&module_url_specified, &module_url_found);
    }

    let maybe_mod_id = {
      let state = state_rc.borrow();
      state.modules.get_id(&module_url_found)
    };

    let module_id = match maybe_mod_id {
      Some(id) => {
        // Module has already been registered.
        debug!(
          "Already-registered module fetched again: {}",
          module_url_found
        );
        id
      }
      // Module not registered yet, do it now.
      None => self.mod_new(is_main, &module_url_found, &code)?,
    };

    // Now we must iterate over all imports of the module and load them.
    let imports = {
      let state_rc = Self::state(self);
      let state = state_rc.borrow();
      state.modules.get_children(module_id).unwrap().clone()
    };

    for module_specifier in imports {
      let is_registered = {
        let state_rc = Self::state(self);
        let state = state_rc.borrow();
        state.modules.is_registered(&module_specifier)
      };
      if !is_registered {
        load
          .add_import(module_specifier.to_owned(), referrer_specifier.clone());
      }
    }

    // If we just finished loading the root module, store the root module id.
    if load.state == LoadState::LoadingRoot {
      load.root_module_id = Some(module_id);
      load.state = LoadState::LoadingImports;
    }

    if load.pending.is_empty() {
      load.state = LoadState::Done;
    }

    Ok(())
  }

  /// Asynchronously load specified module and all of it's dependencies
  ///
  /// User must call `Isolate::mod_evaluate` with returned `ModuleId`
  /// manually after load is finished.
  pub async fn load_module(
    &mut self,
    specifier: &ModuleSpecifier,
    code: Option<String>,
  ) -> Result<ModuleId, ErrBox> {
    self.shared_init();
    let loader = {
      let state_rc = Self::state(self);
      let state = state_rc.borrow();
      state.loader.clone()
    };

    let load = RecursiveModuleLoad::main(&specifier.to_string(), code, loader);
    let (_load_id, prepare_result) = load.prepare().await;

    let mut load = prepare_result?;

    while let Some(info_result) = load.next().await {
      let info = info_result?;
      self.register_during_load(info, &mut load)?;
    }

    let root_id = load.root_module_id.expect("Root module id empty");
    self.mod_instantiate(root_id).map(|_| root_id)
  }

  pub fn snapshot(&mut self) -> v8::StartupData {
    let state_rc = Self::state(self);
    std::mem::take(&mut state_rc.borrow_mut().modules);
    CoreIsolate::snapshot(self)
  }

  pub fn state(isolate: &v8::Isolate) -> Rc<RefCell<EsIsolateState>> {
    let s = isolate.get_slot::<Rc<RefCell<EsIsolateState>>>().unwrap();
    s.clone()
  }
}

impl Future for EsIsolate {
  type Output = Result<(), ErrBox>;

  fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
    let es_isolate = self.get_mut();

    let state_rc = Self::state(es_isolate);

    {
      let state = state_rc.borrow();
      state.waker.register(cx.waker());
    }

    let has_preparing = {
      let state = state_rc.borrow();
      !state.preparing_dyn_imports.is_empty()
    };
    if has_preparing {
      let poll_imports = es_isolate.prepare_dyn_imports(cx)?;
      assert!(poll_imports.is_ready());
    }

    let has_pending = {
      let state = state_rc.borrow();
      !state.pending_dyn_imports.is_empty()
    };
    if has_pending {
      let poll_imports = es_isolate.poll_dyn_imports(cx)?;
      assert!(poll_imports.is_ready());
    }

    match ready!(es_isolate.0.poll_unpin(cx)) {
      Ok(()) => {
        let state = state_rc.borrow();
        if state.pending_dyn_imports.is_empty()
          && state.preparing_dyn_imports.is_empty()
        {
          Poll::Ready(Ok(()))
        } else {
          Poll::Pending
        }
      }
      Err(e) => Poll::Ready(Err(e)),
    }
  }
}

impl EsIsolateState {
  // Called by V8 during `Isolate::mod_instantiate`.
  pub fn module_resolve_cb(
    &mut self,
    specifier: &str,
    referrer_id: ModuleId,
  ) -> ModuleId {
    let referrer = self.modules.get_name(referrer_id).unwrap();
    let specifier = self
      .loader
      .resolve(specifier, referrer, false)
      .expect("Module should have been already resolved");
    self.modules.get_id(specifier.as_str()).unwrap_or(0)
  }

  // Called by V8 during `Isolate::mod_instantiate`.
  pub fn dyn_import_cb(
    &mut self,
    resolver_handle: v8::Global<v8::PromiseResolver>,
    specifier: &str,
    referrer: &str,
  ) {
    debug!("dyn_import specifier {} referrer {} ", specifier, referrer);

    let load = RecursiveModuleLoad::dynamic_import(
      specifier,
      referrer,
      self.loader.clone(),
    );
    self.dyn_import_map.insert(load.id, resolver_handle);
    self.waker.wake();
    let fut = load.prepare().boxed_local();
    self.preparing_dyn_imports.push(fut);
  }
}

#[cfg(test)]
pub mod tests {
  use super::*;
  use crate::core_isolate::tests::run_in_task;
  use crate::core_isolate::CoreIsolateState;
  use crate::js_check;
  use crate::modules::ModuleSourceFuture;
  use crate::ops::*;
  use crate::ZeroCopyBuf;
  use std::io;
  use std::sync::atomic::{AtomicUsize, Ordering};
  use std::sync::Arc;

  #[test]
  fn test_mods() {
    #[derive(Default)]
    struct ModsLoader {
      pub count: Arc<AtomicUsize>,
    }

    impl ModuleLoader for ModsLoader {
      fn resolve(
        &self,
        specifier: &str,
        referrer: &str,
        _is_main: bool,
      ) -> Result<ModuleSpecifier, ErrBox> {
        self.count.fetch_add(1, Ordering::Relaxed);
        assert_eq!(specifier, "./b.js");
        assert_eq!(referrer, "file:///a.js");
        let s = ModuleSpecifier::resolve_import(specifier, referrer).unwrap();
        Ok(s)
      }

      fn load(
        &self,
        _module_specifier: &ModuleSpecifier,
        _maybe_referrer: Option<ModuleSpecifier>,
        _is_dyn_import: bool,
      ) -> Pin<Box<ModuleSourceFuture>> {
        unreachable!()
      }
    }

    let loader = Rc::new(ModsLoader::default());
    let resolve_count = loader.count.clone();
    let dispatch_count = Arc::new(AtomicUsize::new(0));
    let dispatch_count_ = dispatch_count.clone();

    let mut isolate = EsIsolate::new(loader, StartupData::None, false);

    let dispatcher = move |_state: &mut CoreIsolateState,
                           control: &[u8],
                           _zero_copy: &mut [ZeroCopyBuf]|
          -> Op {
      dispatch_count_.fetch_add(1, Ordering::Relaxed);
      assert_eq!(control.len(), 1);
      assert_eq!(control[0], 42);
      let buf = vec![43u8, 0, 0, 0].into_boxed_slice();
      Op::Async(futures::future::ready(buf).boxed())
    };

    isolate.register_op("test", dispatcher);

    js_check(isolate.execute(
      "setup.js",
      r#"
        function assert(cond) {
          if (!cond) {
            throw Error("assert");
          }
        }
        "#,
    ));

    assert_eq!(dispatch_count.load(Ordering::Relaxed), 0);

    let specifier_a = "file:///a.js".to_string();
    let mod_a = isolate
      .mod_new(
        true,
        &specifier_a,
        r#"
        import { b } from './b.js'
        if (b() != 'b') throw Error();
        let control = new Uint8Array([42]);
        Deno.core.send(1, control);
      "#,
      )
      .unwrap();
    assert_eq!(dispatch_count.load(Ordering::Relaxed), 0);

    let state_rc = EsIsolate::state(&isolate);
    {
      let state = state_rc.borrow();
      let imports = state.modules.get_children(mod_a);
      assert_eq!(
        imports,
        Some(&vec![ModuleSpecifier::resolve_url("file:///b.js").unwrap()])
      );
    }
    let mod_b = isolate
      .mod_new(false, "file:///b.js", "export function b() { return 'b' }")
      .unwrap();
    {
      let state = state_rc.borrow();
      let imports = state.modules.get_children(mod_b).unwrap();
      assert_eq!(imports.len(), 0);
    }

    js_check(isolate.mod_instantiate(mod_b));
    assert_eq!(dispatch_count.load(Ordering::Relaxed), 0);
    assert_eq!(resolve_count.load(Ordering::SeqCst), 1);

    js_check(isolate.mod_instantiate(mod_a));
    assert_eq!(dispatch_count.load(Ordering::Relaxed), 0);

    js_check(isolate.mod_evaluate(mod_a));
    assert_eq!(dispatch_count.load(Ordering::Relaxed), 1);
  }

  #[test]
  fn dyn_import_err() {
    #[derive(Clone, Default)]
    struct DynImportErrLoader {
      pub count: Arc<AtomicUsize>,
    }

    impl ModuleLoader for DynImportErrLoader {
      fn resolve(
        &self,
        specifier: &str,
        referrer: &str,
        _is_main: bool,
      ) -> Result<ModuleSpecifier, ErrBox> {
        self.count.fetch_add(1, Ordering::Relaxed);
        assert_eq!(specifier, "/foo.js");
        assert_eq!(referrer, "file:///dyn_import2.js");
        let s = ModuleSpecifier::resolve_import(specifier, referrer).unwrap();
        Ok(s)
      }

      fn load(
        &self,
        _module_specifier: &ModuleSpecifier,
        _maybe_referrer: Option<ModuleSpecifier>,
        _is_dyn_import: bool,
      ) -> Pin<Box<ModuleSourceFuture>> {
        async { Err(ErrBox::from(io::Error::from(io::ErrorKind::NotFound))) }
          .boxed()
      }
    }

    // Test an erroneous dynamic import where the specified module isn't found.
    run_in_task(|cx| {
      let loader = Rc::new(DynImportErrLoader::default());
      let count = loader.count.clone();
      let mut isolate = EsIsolate::new(loader, StartupData::None, false);

      js_check(isolate.execute(
        "file:///dyn_import2.js",
        r#"
        (async () => {
          await import("/foo.js");
        })();
        "#,
      ));

      assert_eq!(count.load(Ordering::Relaxed), 0);
      // We should get an error here.
      let result = isolate.poll_unpin(cx);
      if let Poll::Ready(Ok(_)) = result {
        unreachable!();
      }
      assert_eq!(count.load(Ordering::Relaxed), 2);
    })
  }

  #[derive(Clone, Default)]
  struct DynImportOkLoader {
    pub prepare_load_count: Arc<AtomicUsize>,
    pub resolve_count: Arc<AtomicUsize>,
    pub load_count: Arc<AtomicUsize>,
  }

  impl ModuleLoader for DynImportOkLoader {
    fn resolve(
      &self,
      specifier: &str,
      referrer: &str,
      _is_main: bool,
    ) -> Result<ModuleSpecifier, ErrBox> {
      let c = self.resolve_count.fetch_add(1, Ordering::Relaxed);
      assert!(c < 4);
      assert_eq!(specifier, "./b.js");
      assert_eq!(referrer, "file:///dyn_import3.js");
      let s = ModuleSpecifier::resolve_import(specifier, referrer).unwrap();
      Ok(s)
    }

    fn load(
      &self,
      specifier: &ModuleSpecifier,
      _maybe_referrer: Option<ModuleSpecifier>,
      _is_dyn_import: bool,
    ) -> Pin<Box<ModuleSourceFuture>> {
      self.load_count.fetch_add(1, Ordering::Relaxed);
      let info = ModuleSource {
        module_url_specified: specifier.to_string(),
        module_url_found: specifier.to_string(),
        code: "export function b() { return 'b' }".to_owned(),
      };
      async move { Ok(info) }.boxed()
    }

    fn prepare_load(
      &self,
      _load_id: ModuleLoadId,
      _module_specifier: &ModuleSpecifier,
      _maybe_referrer: Option<String>,
      _is_dyn_import: bool,
    ) -> Pin<Box<dyn Future<Output = Result<(), ErrBox>>>> {
      self.prepare_load_count.fetch_add(1, Ordering::Relaxed);
      async { Ok(()) }.boxed_local()
    }
  }

  #[test]
  fn dyn_import_ok() {
    run_in_task(|cx| {
      let loader = Rc::new(DynImportOkLoader::default());
      let prepare_load_count = loader.prepare_load_count.clone();
      let resolve_count = loader.resolve_count.clone();
      let load_count = loader.load_count.clone();
      let mut isolate = EsIsolate::new(loader, StartupData::None, false);

      // Dynamically import mod_b
      js_check(isolate.execute(
        "file:///dyn_import3.js",
        r#"
          (async () => {
            let mod = await import("./b.js");
            if (mod.b() !== 'b') {
              throw Error("bad1");
            }
            // And again!
            mod = await import("./b.js");
            if (mod.b() !== 'b') {
              throw Error("bad2");
            }
          })();
          "#,
      ));

      // First poll runs `prepare_load` hook.
      assert!(match isolate.poll_unpin(cx) {
        Poll::Pending => true,
        _ => false,
      });
      assert_eq!(prepare_load_count.load(Ordering::Relaxed), 1);

      // Second poll actually loads modules into the isolate.
      assert!(match isolate.poll_unpin(cx) {
        Poll::Ready(Ok(_)) => true,
        _ => false,
      });
      assert_eq!(resolve_count.load(Ordering::Relaxed), 4);
      assert_eq!(load_count.load(Ordering::Relaxed), 2);
      assert!(match isolate.poll_unpin(cx) {
        Poll::Ready(Ok(_)) => true,
        _ => false,
      });
      assert_eq!(resolve_count.load(Ordering::Relaxed), 4);
      assert_eq!(load_count.load(Ordering::Relaxed), 2);
    })
  }

  #[test]
  fn dyn_import_borrow_mut_error() {
    // https://github.com/denoland/deno/issues/6054
    run_in_task(|cx| {
      let loader = Rc::new(DynImportOkLoader::default());
      let prepare_load_count = loader.prepare_load_count.clone();
      let mut isolate = EsIsolate::new(loader, StartupData::None, false);
      js_check(isolate.execute(
        "file:///dyn_import3.js",
        r#"
          (async () => {
            let mod = await import("./b.js");
            if (mod.b() !== 'b') {
              throw Error("bad");
            }
            // Now do any op
            Deno.core.ops();
          })();
          "#,
      ));
      // First poll runs `prepare_load` hook.
      let _ = isolate.poll_unpin(cx);
      assert_eq!(prepare_load_count.load(Ordering::Relaxed), 1);
      // Second poll triggers error
      let _ = isolate.poll_unpin(cx);
    })
  }

  #[test]
  fn es_snapshot() {
    #[derive(Default)]
    struct ModsLoader;

    impl ModuleLoader for ModsLoader {
      fn resolve(
        &self,
        specifier: &str,
        referrer: &str,
        _is_main: bool,
      ) -> Result<ModuleSpecifier, ErrBox> {
        assert_eq!(specifier, "file:///main.js");
        assert_eq!(referrer, ".");
        let s = ModuleSpecifier::resolve_import(specifier, referrer).unwrap();
        Ok(s)
      }

      fn load(
        &self,
        _module_specifier: &ModuleSpecifier,
        _maybe_referrer: Option<ModuleSpecifier>,
        _is_dyn_import: bool,
      ) -> Pin<Box<ModuleSourceFuture>> {
        unreachable!()
      }
    }

    let loader = std::rc::Rc::new(ModsLoader::default());
    let mut runtime_isolate = EsIsolate::new(loader, StartupData::None, true);

    let specifier = ModuleSpecifier::resolve_url("file:///main.js").unwrap();
    let source_code = "Deno.core.print('hello\\n')".to_string();

    let module_id = futures::executor::block_on(
      runtime_isolate.load_module(&specifier, Some(source_code)),
    )
    .unwrap();

    js_check(runtime_isolate.mod_evaluate(module_id));

    let _snapshot = runtime_isolate.snapshot();
  }
}
