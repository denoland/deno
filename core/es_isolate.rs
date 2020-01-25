// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

// This module provides higher level implementation of Isolate that
// supports asynchronous loading and executution of ES Modules.
// The isolate.rs should never depend on this module.

use rusty_v8 as v8;

use crate::any_error::ErrBox;
use crate::bindings;
use futures::future::Future;
use futures::future::FutureExt;
use futures::ready;
use futures::stream::FuturesUnordered;
use futures::stream::StreamExt;
use futures::stream::StreamFuture;
use futures::task::AtomicWaker;
use libc::c_void;
use std::collections::HashMap;
use std::ops::{Deref, DerefMut};
use std::option::Option;
use std::pin::Pin;
use std::sync::Arc;
use std::task::Context;
use std::task::Poll;

use crate::isolate::Isolate;
use crate::isolate::StartupData;
use crate::module_specifier::ModuleSpecifier;
use crate::modules::LoadState;
use crate::modules::Loader;
use crate::modules::Modules;
use crate::modules::RecursiveModuleLoad;

pub type ModuleId = i32;
pub type DynImportId = i32;
/// Represent result of fetching the source code of a module. Found module URL
/// might be different from specified URL used for loading due to redirections
/// (like HTTP 303). E.G. Both https://example.com/a.ts and
/// https://example.com/b.ts may point to https://example.com/c.ts
/// By keeping track of specified and found URL we can alias modules and avoid
/// recompiling the same code 3 times.
#[derive(Debug, Eq, PartialEq)]
pub struct SourceCodeInfo {
  pub code: String,
  pub module_url_specified: String,
  pub module_url_found: String,
}

/// More specialized version of `Isolate` that provides loading
/// and execution of ES Modules.
///
/// Creating `EsIsolate` requires to pass `loader` argument
/// that implements `Loader` trait - that way actual resolution and
/// loading of modules can be customized by the implementor.
pub struct EsIsolate {
  core_isolate: Box<Isolate>,
  loader: Arc<Box<dyn Loader + Unpin>>,
  pub modules: Modules,
  pub(crate) next_dyn_import_id: DynImportId,
  pub(crate) dyn_import_map:
    HashMap<DynImportId, v8::Global<v8::PromiseResolver>>,

  pending_dyn_imports: FuturesUnordered<StreamFuture<RecursiveModuleLoad>>,
  waker: AtomicWaker,
}

impl Deref for EsIsolate {
  type Target = Isolate;

  fn deref(&self) -> &Self::Target {
    &self.core_isolate
  }
}

impl DerefMut for EsIsolate {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.core_isolate
  }
}

unsafe impl Send for EsIsolate {}

impl Drop for EsIsolate {
  fn drop(&mut self) {
    let isolate = self.core_isolate.v8_isolate.as_ref().unwrap();
    // Clear persistent handles we own.
    {
      let mut locker = v8::Locker::new(&isolate);
      let mut hs = v8::HandleScope::new(locker.enter());
      let scope = hs.enter();
      for module in self.modules.info.values_mut() {
        module.handle.reset(scope);
      }
      for handle in self.dyn_import_map.values_mut() {
        handle.reset(scope);
      }
    }
  }
}

impl EsIsolate {
  pub fn new(
    loader: Box<dyn Loader + Unpin>,
    startup_data: StartupData,
    will_snapshot: bool,
  ) -> Box<Self> {
    let mut core_isolate = Isolate::new(startup_data, will_snapshot);
    {
      let isolate = core_isolate.v8_isolate.as_mut().unwrap();
      isolate.set_host_initialize_import_meta_object_callback(
        bindings::host_initialize_import_meta_object_callback,
      );
      isolate.set_host_import_module_dynamically_callback(
        bindings::host_import_module_dynamically_callback,
      );
    }

    let es_isolate = Self {
      modules: Modules::new(),
      loader: Arc::new(loader),
      core_isolate,
      next_dyn_import_id: 0,
      dyn_import_map: HashMap::new(),
      pending_dyn_imports: FuturesUnordered::new(),
      waker: AtomicWaker::new(),
    };

    let mut boxed_es_isolate = Box::new(es_isolate);
    {
      let es_isolate_ptr: *mut Self = Box::into_raw(boxed_es_isolate);
      boxed_es_isolate = unsafe { Box::from_raw(es_isolate_ptr) };
      unsafe {
        let v8_isolate = boxed_es_isolate.v8_isolate.as_mut().unwrap();
        v8_isolate.set_data(1, es_isolate_ptr as *mut c_void);
      };
    }
    boxed_es_isolate
  }

  fn mod_new2(&mut self, main: bool, name: &str, source: &str) -> ModuleId {
    let isolate = self.core_isolate.v8_isolate.as_ref().unwrap();
    let mut locker = v8::Locker::new(&isolate);

    let mut hs = v8::HandleScope::new(locker.enter());
    let scope = hs.enter();
    assert!(!self.core_isolate.global_context.is_empty());
    let context = self.core_isolate.global_context.get(scope).unwrap();
    let mut cs = v8::ContextScope::new(scope, context);
    let scope = cs.enter();

    let name_str = v8::String::new(scope, name).unwrap();
    let source_str = v8::String::new(scope, source).unwrap();

    let origin = bindings::module_origin(scope, name_str);
    let source = v8::script_compiler::Source::new(source_str, &origin);

    let mut try_catch = v8::TryCatch::new(scope);
    let tc = try_catch.enter();

    let maybe_module = v8::script_compiler::compile_module(&isolate, source);

    if tc.has_caught() {
      assert!(maybe_module.is_none());
      self.core_isolate.handle_exception(
        scope,
        context,
        tc.exception().unwrap(),
      );
      return 0;
    }
    let module = maybe_module.unwrap();
    let id = module.get_identity_hash();

    let mut import_specifiers: Vec<String> = vec![];
    for i in 0..module.get_module_requests_length() {
      let specifier = module.get_module_request(i);
      import_specifiers.push(specifier.to_rust_string_lossy(scope));
    }

    let mut handle = v8::Global::<v8::Module>::new();
    handle.set(scope, module);
    self
      .modules
      .register(id, name, main, handle, import_specifiers);
    id
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
    let id = self.mod_new2(main, name, source);
    self.core_isolate.check_last_exception().map(|_| id)
  }

  fn mod_instantiate2(&mut self, id: ModuleId) {
    let isolate = self.core_isolate.v8_isolate.as_ref().unwrap();
    let mut locker = v8::Locker::new(isolate);
    let mut hs = v8::HandleScope::new(locker.enter());
    let scope = hs.enter();
    assert!(!self.core_isolate.global_context.is_empty());
    let context = self.core_isolate.global_context.get(scope).unwrap();
    let mut cs = v8::ContextScope::new(scope, context);
    let mut try_catch = v8::TryCatch::new(cs.enter());
    let tc = try_catch.enter();

    let maybe_info = self.modules.get_info(id);

    if maybe_info.is_none() {
      return;
    }

    let module_handle = &maybe_info.unwrap().handle;
    let mut module = module_handle.get(scope).unwrap();

    if module.get_status() == v8::ModuleStatus::Errored {
      return;
    }

    let maybe_ok =
      module.instantiate_module(context, bindings::module_resolve_callback);
    assert!(maybe_ok.is_some() || tc.has_caught());

    if tc.has_caught() {
      self.core_isolate.handle_exception(
        scope,
        context,
        tc.exception().unwrap(),
      );
    }
  }

  /// Instanciates a ES module
  ///
  /// ErrBox can be downcast to a type that exposes additional information about
  /// the V8 exception. By default this type is CoreJSError, however it may be a
  /// different type if Isolate::set_js_error_create() has been used.
  fn mod_instantiate(&mut self, id: ModuleId) -> Result<(), ErrBox> {
    self.mod_instantiate2(id);
    self.core_isolate.check_last_exception()
  }

  fn mod_evaluate2(&mut self, id: ModuleId) {
    let isolate = self.core_isolate.v8_isolate.as_ref().unwrap();
    let mut locker = v8::Locker::new(isolate);
    let mut hs = v8::HandleScope::new(locker.enter());
    let scope = hs.enter();
    assert!(!self.core_isolate.global_context.is_empty());
    let context = self.core_isolate.global_context.get(scope).unwrap();
    let mut cs = v8::ContextScope::new(scope, context);
    let scope = cs.enter();

    let info = self.modules.get_info(id).expect("ModuleInfo not found");
    let mut module = info.handle.get(scope).expect("Empty module handle");
    let mut status = module.get_status();

    if status == v8::ModuleStatus::Instantiated {
      let ok = module.evaluate(scope, context).is_some();
      // Update status after evaluating.
      status = module.get_status();
      if ok {
        assert!(
          status == v8::ModuleStatus::Evaluated
            || status == v8::ModuleStatus::Errored
        );
      } else {
        assert!(status == v8::ModuleStatus::Errored);
      }
    }

    match status {
      v8::ModuleStatus::Evaluated => {
        self.core_isolate.last_exception_handle.reset(scope);
        self.core_isolate.last_exception.take();
      }
      v8::ModuleStatus::Errored => {
        self.core_isolate.handle_exception(
          scope,
          context,
          module.get_exception(),
        );
      }
      other => panic!("Unexpected module status {:?}", other),
    };
  }

  /// Evaluates an already instantiated ES module.
  ///
  /// ErrBox can be downcast to a type that exposes additional information about
  /// the V8 exception. By default this type is CoreJSError, however it may be a
  /// different type if Isolate::set_js_error_create() has been used.
  pub fn mod_evaluate(&mut self, id: ModuleId) -> Result<(), ErrBox> {
    self.shared_init();
    self.mod_evaluate2(id);
    self.core_isolate.check_last_exception()
  }

  // Called by V8 during `Isolate::mod_instantiate`.
  pub fn module_resolve_cb(
    &mut self,
    specifier: &str,
    referrer_id: ModuleId,
  ) -> ModuleId {
    let referrer = self.modules.get_name(referrer_id).unwrap();
    // We should have already resolved and Ready this module, so
    // resolve() will not fail this time.
    let specifier = self
      .modules
      .get_cached_specifier(specifier, &referrer)
      .expect("Module should already be resolved");
    self.modules.get_id(specifier.as_str()).unwrap_or(0)
  }

  // Called by V8 during `Isolate::mod_instantiate`.
  pub fn dyn_import_cb(
    &mut self,
    specifier: &str,
    referrer: &str,
    id: DynImportId,
  ) {
    debug!("dyn_import specifier {} referrer {} ", specifier, referrer);

    let load = RecursiveModuleLoad::dynamic_import(
      id,
      specifier,
      referrer,
      self.loader.clone(),
    );
    self.waker.wake();
    self.pending_dyn_imports.push(load.into_future());
  }

  fn dyn_import_error(
    &mut self,
    id: DynImportId,
    error: Option<String>,
  ) -> Result<(), ErrBox> {
    debug!("dyn_import_error {} {:?}", id, error);
    assert!(
      error.is_some() || !self.core_isolate.last_exception_handle.is_empty()
    );
    let isolate = self.core_isolate.v8_isolate.as_ref().unwrap();
    let mut locker = v8::Locker::new(isolate);
    let mut hs = v8::HandleScope::new(locker.enter());
    let scope = hs.enter();
    assert!(!self.core_isolate.global_context.is_empty());
    let context = self.core_isolate.global_context.get(scope).unwrap();
    let mut cs = v8::ContextScope::new(scope, context);
    let scope = cs.enter();

    let mut resolver_handle = self
      .dyn_import_map
      .remove(&id)
      .expect("Invalid dyn import id");
    let mut resolver = resolver_handle.get(scope).unwrap();
    resolver_handle.reset(scope);
    // Resolution error.
    if let Some(error_str) = error {
      let msg = v8::String::new(scope, &error_str).unwrap();
      let e = v8::Exception::type_error(scope, msg);
      resolver.reject(context, e).unwrap();
    } else {
      let e = self.core_isolate.last_exception_handle.get(scope).unwrap();
      self.core_isolate.last_exception_handle.reset(scope);
      self.core_isolate.last_exception.take();
      resolver.reject(context, e).unwrap();
    }

    scope.isolate().run_microtasks();
    self.core_isolate.check_last_exception()
  }

  fn dyn_import_done(
    &mut self,
    id: DynImportId,
    mod_id: ModuleId,
  ) -> Result<(), ErrBox> {
    debug!("dyn_import_done {} {:?}", id, mod_id);
    assert!(mod_id != 0);
    let isolate = self.core_isolate.v8_isolate.as_ref().unwrap();
    let mut locker = v8::Locker::new(isolate);
    let mut hs = v8::HandleScope::new(locker.enter());
    let scope = hs.enter();
    assert!(!self.core_isolate.global_context.is_empty());
    let context = self.core_isolate.global_context.get(scope).unwrap();
    let mut cs = v8::ContextScope::new(scope, context);
    let scope = cs.enter();

    let mut resolver_handle = self
      .dyn_import_map
      .remove(&id)
      .expect("Invalid dyn import id");
    let mut resolver = resolver_handle.get(scope).unwrap();
    resolver_handle.reset(scope);
    let info = self
      .modules
      .get_info(mod_id)
      .expect("Dyn import module info not found");
    // Resolution success
    let mut module = info.handle.get(scope).unwrap();
    assert_eq!(module.get_status(), v8::ModuleStatus::Evaluated);
    let module_namespace = module.get_module_namespace();
    resolver.resolve(context, module_namespace).unwrap();
    scope.isolate().run_microtasks();
    self.core_isolate.check_last_exception()
  }

  fn poll_dyn_imports(&mut self, cx: &mut Context) -> Poll<Result<(), ErrBox>> {
    loop {
      match self.pending_dyn_imports.poll_next_unpin(cx) {
        Poll::Pending | Poll::Ready(None) => {
          // There are no active dynamic import loaders, or none are ready.
          return Poll::Ready(Ok(()));
        }
        Poll::Ready(Some(load_stream_poll)) => {
          let maybe_result = load_stream_poll.0;
          let mut load = load_stream_poll.1;
          let dyn_import_id = load.dyn_import_id.unwrap();

          if let Some(load_stream_result) = maybe_result {
            match load_stream_result {
              Ok(info) => {
                // A module (not necessarily the one dynamically imported) has been
                // fetched. Create and register it, and if successful, poll for the
                // next recursive-load event related to this dynamic import.
                match self.register_during_load(info, &mut load) {
                  Ok(()) => {
                    // Keep importing until it's fully drained
                    self.pending_dyn_imports.push(load.into_future());
                  }
                  Err(err) => self
                    .dyn_import_error(dyn_import_id, Some(err.to_string()))?,
                }
              }
              Err(err) => {
                // A non-javascript error occurred; this could be due to a an invalid
                // module specifier, or a problem with the source map, or a failure
                // to fetch the module source code.
                self.dyn_import_error(dyn_import_id, Some(err.to_string()))?
              }
            }
          } else {
            // The top-level module from a dynamic import has been instantiated.
            // Load is done.
            let module_id = load.root_module_id.unwrap();
            self.mod_instantiate(module_id)?;
            if let Ok(()) = self.mod_evaluate(module_id) {
              self.dyn_import_done(dyn_import_id, module_id)?
            } else {
              self.dyn_import_error(dyn_import_id, None)?
            }
          }
        }
      }
    }
  }

  fn register_during_load(
    &mut self,
    info: SourceCodeInfo,
    load: &mut RecursiveModuleLoad,
  ) -> Result<(), ErrBox> {
    let SourceCodeInfo {
      code,
      module_url_specified,
      module_url_found,
    } = info;

    let is_main =
      load.state == LoadState::LoadingRoot && !load.is_dynamic_import();
    let referrer_name = &module_url_found.to_string();
    let referrer_specifier =
      ModuleSpecifier::resolve_url(referrer_name).unwrap();

    // #A There are 3 cases to handle at this moment:
    // 1. Source code resolved result have the same module name as requested
    //    and is not yet registered
    //     -> register
    // 2. Source code resolved result have a different name as requested:
    //   2a. The module with resolved module name has been registered
    //     -> alias
    //   2b. The module with resolved module name has not yet been registerd
    //     -> register & alias

    // If necessary, register an alias.
    if module_url_specified != module_url_found {
      self.modules.alias(&module_url_specified, &module_url_found);
    }

    let module_id = match self.modules.get_id(&module_url_found) {
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
    let imports = self
      .modules
      .get_info(module_id)
      .unwrap()
      .import_specifiers
      .clone();
    for import in imports {
      let module_specifier = self.loader.resolve(
        &import,
        referrer_name,
        false,
        load.is_dynamic_import(),
      )?;
      self
        .modules
        .cache_specifier(&import, referrer_name, &module_specifier);
      let module_name = module_specifier.as_str();

      if !self.modules.is_registered(module_name) {
        load.add_import(module_specifier, referrer_specifier.clone());
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
    specifier: &str,
    code: Option<String>,
  ) -> Result<ModuleId, ErrBox> {
    let mut load =
      RecursiveModuleLoad::main(specifier, code, self.loader.clone());

    while let Some(info_result) = load.next().await {
      let info = info_result?;
      self.register_during_load(info, &mut load)?;
    }

    let root_id = load.root_module_id.expect("Root module id empty");
    self.mod_instantiate(root_id).map(|_| root_id)
  }
}

impl Future for EsIsolate {
  type Output = Result<(), ErrBox>;

  fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
    let inner = self.get_mut();

    inner.waker.register(cx.waker());

    // If there are any pending dyn_import futures, do those first.
    if !inner.pending_dyn_imports.is_empty() {
      let poll_imports = inner.poll_dyn_imports(cx)?;
      assert!(poll_imports.is_ready());
    }

    match ready!(inner.core_isolate.poll_unpin(cx)) {
      Ok(()) => {
        if inner.pending_dyn_imports.is_empty() {
          Poll::Ready(Ok(()))
        } else {
          Poll::Pending
        }
      }
      Err(e) => Poll::Ready(Err(e)),
    }
  }
}

#[cfg(test)]
pub mod tests {
  use super::*;
  use crate::isolate::js_check;
  use crate::isolate::tests::run_in_task;
  use crate::isolate::ZeroCopyBuf;
  use crate::modules::SourceCodeInfoFuture;
  use crate::ops::*;
  use std::io;
  use std::sync::atomic::{AtomicUsize, Ordering};

  #[test]
  fn test_mods() {
    #[derive(Clone, Default)]
    struct ModsLoader {
      pub count: Arc<AtomicUsize>,
    }

    impl Loader for ModsLoader {
      fn resolve(
        &self,
        specifier: &str,
        referrer: &str,
        _is_main: bool,
        _is_dyn_import: bool,
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
      ) -> Pin<Box<SourceCodeInfoFuture>> {
        unreachable!()
      }
    }

    let loader = Box::new(ModsLoader::default());
    let resolve_count = loader.count.clone();
    let dispatch_count = Arc::new(AtomicUsize::new(0));
    let dispatch_count_ = dispatch_count.clone();

    let mut isolate = EsIsolate::new(loader, StartupData::None, false);

    let dispatcher =
      move |control: &[u8], _zero_copy: Option<ZeroCopyBuf>| -> CoreOp {
        dispatch_count_.fetch_add(1, Ordering::Relaxed);
        assert_eq!(control.len(), 1);
        assert_eq!(control[0], 42);
        let buf = vec![43u8, 0, 0, 0].into_boxed_slice();
        Op::Async(futures::future::ok(buf).boxed())
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

    let imports = isolate
      .modules
      .get_info(mod_a)
      .unwrap()
      .import_specifiers
      .clone();
    let specifier_b = "./b.js".to_string();
    assert_eq!(imports, vec![specifier_b.clone()]);
    let mod_b = isolate
      .mod_new(false, "file:///b.js", "export function b() { return 'b' }")
      .unwrap();
    let imports = isolate
      .modules
      .get_info(mod_b)
      .unwrap()
      .import_specifiers
      .clone();
    assert_eq!(imports.len(), 0);

    let module_specifier =
      ModuleSpecifier::resolve_import(&specifier_b, &specifier_a).unwrap();
    isolate.modules.cache_specifier(
      &specifier_b,
      &specifier_a,
      &module_specifier,
    );
    js_check(isolate.mod_instantiate(mod_b));
    assert_eq!(dispatch_count.load(Ordering::Relaxed), 0);
    assert_eq!(resolve_count.load(Ordering::SeqCst), 0);

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

    impl Loader for DynImportErrLoader {
      fn resolve(
        &self,
        specifier: &str,
        referrer: &str,
        _is_main: bool,
        _is_dyn_import: bool,
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
      ) -> Pin<Box<SourceCodeInfoFuture>> {
        async { Err(ErrBox::from(io::Error::from(io::ErrorKind::NotFound))) }
          .boxed()
      }
    }

    // Test an erroneous dynamic import where the specified module isn't found.
    run_in_task(|cx| {
      let loader = Box::new(DynImportErrLoader::default());
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
      assert_eq!(count.load(Ordering::Relaxed), 1);
    })
  }

  #[test]
  fn dyn_import_err2() {
    #[derive(Clone, Default)]
    struct DynImportErr2Loader {
      pub count: Arc<AtomicUsize>,
    }

    impl Loader for DynImportErr2Loader {
      fn resolve(
        &self,
        specifier: &str,
        referrer: &str,
        _is_main: bool,
        _is_dyn_import: bool,
      ) -> Result<ModuleSpecifier, ErrBox> {
        let c = self.count.fetch_add(1, Ordering::Relaxed);
        match c {
          0 => assert_eq!(specifier, "/foo1.js"),
          1 => assert_eq!(specifier, "/foo2.js"),
          2 => assert_eq!(specifier, "/foo3.js"),
          _ => unreachable!(),
        }
        assert_eq!(referrer, "file:///dyn_import_error.js");
        let s = ModuleSpecifier::resolve_import(specifier, referrer).unwrap();
        Ok(s)
      }

      fn load(
        &self,
        specifier: &ModuleSpecifier,
        _maybe_referrer: Option<ModuleSpecifier>,
      ) -> Pin<Box<SourceCodeInfoFuture>> {
        let info = SourceCodeInfo {
          module_url_specified: specifier.to_string(),
          module_url_found: specifier.to_string(),
          code: "# not valid JS".to_owned(),
        };
        async move { Ok(info) }.boxed()
      }
    }

    // Import multiple modules to demonstrate that after failed dynamic import
    // another dynamic import can still be run
    run_in_task(|cx| {
      let loader = Box::new(DynImportErr2Loader::default());
      let loader1 = loader.clone();
      let mut isolate = EsIsolate::new(loader, StartupData::None, false);

      js_check(isolate.execute(
        "file:///dyn_import_error.js",
        r#"
        (async () => {
          await import("/foo1.js");
        })();
        (async () => {
          await import("/foo2.js");
        })();
        (async () => {
          await import("/foo3.js");
        })();
        "#,
      ));

      assert_eq!(loader1.count.load(Ordering::Relaxed), 0);
      // Now each poll should return error
      assert!(match isolate.poll_unpin(cx) {
        Poll::Ready(Err(_)) => true,
        _ => false,
      });
      assert_eq!(loader1.count.load(Ordering::Relaxed), 1);
      assert!(match isolate.poll_unpin(cx) {
        Poll::Ready(Err(_)) => true,
        _ => false,
      });
      assert_eq!(loader1.count.load(Ordering::Relaxed), 2);
      assert!(match isolate.poll_unpin(cx) {
        Poll::Ready(Err(_)) => true,
        _ => false,
      });
      assert_eq!(loader1.count.load(Ordering::Relaxed), 3);
    })
  }

  #[test]
  fn dyn_import_ok() {
    #[derive(Clone, Default)]
    struct DynImportOkLoader {
      pub resolve_count: Arc<AtomicUsize>,
      pub load_count: Arc<AtomicUsize>,
    }

    impl Loader for DynImportOkLoader {
      fn resolve(
        &self,
        specifier: &str,
        referrer: &str,
        _is_main: bool,
        _is_dyn_import: bool,
      ) -> Result<ModuleSpecifier, ErrBox> {
        let c = self.resolve_count.fetch_add(1, Ordering::Relaxed);
        match c {
          0 => assert_eq!(specifier, "./b.js"),
          1 => assert_eq!(specifier, "./b.js"),
          _ => unreachable!(),
        }
        assert_eq!(referrer, "file:///dyn_import3.js");
        let s = ModuleSpecifier::resolve_import(specifier, referrer).unwrap();
        Ok(s)
      }

      fn load(
        &self,
        specifier: &ModuleSpecifier,
        _maybe_referrer: Option<ModuleSpecifier>,
      ) -> Pin<Box<SourceCodeInfoFuture>> {
        self.load_count.fetch_add(1, Ordering::Relaxed);
        let info = SourceCodeInfo {
          module_url_specified: specifier.to_string(),
          module_url_found: specifier.to_string(),
          code: "export function b() { return 'b' }".to_owned(),
        };
        async move { Ok(info) }.boxed()
      }
    }

    run_in_task(|cx| {
      let loader = Box::new(DynImportOkLoader::default());
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

      assert!(match isolate.poll_unpin(cx) {
        Poll::Ready(Ok(_)) => true,
        _ => false,
      });
      assert_eq!(resolve_count.load(Ordering::Relaxed), 2);
      assert_eq!(load_count.load(Ordering::Relaxed), 2);
      assert!(match isolate.poll_unpin(cx) {
        Poll::Ready(Ok(_)) => true,
        _ => false,
      });
      assert_eq!(resolve_count.load(Ordering::Relaxed), 2);
      assert_eq!(load_count.load(Ordering::Relaxed), 2);
    })
  }
}
