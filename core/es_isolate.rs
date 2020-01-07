// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

// Do not add any dependency to modules.rs!
// modules.rs is complex and should remain decoupled from isolate.rs to keep the
// Isolate struct from becoming too bloating for users who do not need
// asynchronous module loading.

use rusty_v8 as v8;

use crate::any_error::ErrBox;
use crate::bindings;
use futures::future::Future;
use futures::future::FutureExt;
use futures::ready;
use futures::stream::FuturesUnordered;
use futures::stream::IntoStream;
use futures::stream::Stream;
use futures::stream::StreamExt;
use futures::stream::StreamFuture;
use futures::stream::TryStream;
use futures::stream::TryStreamExt;
use futures::task::AtomicWaker;
use libc::c_void;
use std::collections::HashMap;
use std::fmt;
use std::ops::{Deref, DerefMut};
use std::option::Option;
use std::pin::Pin;
use std::sync::Arc;
use std::task::Context;
use std::task::Poll;

use crate::isolate::Isolate;
use crate::isolate::StartupData;

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

#[derive(Debug, Eq, PartialEq)]
pub enum RecursiveLoadEvent {
  Fetch(SourceCodeInfo),
  Instantiate(ModuleId),
}

pub trait ImportStream: TryStream {
  fn register(
    &mut self,
    source_code_info: SourceCodeInfo,
    isolate: &mut EsIsolate,
  ) -> Result<(), ErrBox>;
}

type DynImportStream = Box<
  dyn ImportStream<
      Ok = RecursiveLoadEvent,
      Error = ErrBox,
      Item = Result<RecursiveLoadEvent, ErrBox>,
    > + Send
    + Unpin,
>;

type DynImportFn = dyn Fn(DynImportId, &str, &str) -> DynImportStream;

/// Wraps DynImportStream to include the DynImportId, so that it doesn't
/// need to be exposed.
#[derive(Debug)]
struct DynImport {
  pub id: DynImportId,
  pub inner: DynImportStream,
}

impl fmt::Debug for DynImportStream {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "DynImportStream(..)")
  }
}

impl Stream for DynImport {
  type Item = Result<(DynImportId, RecursiveLoadEvent), (DynImportId, ErrBox)>;

  fn poll_next(
    self: Pin<&mut Self>,
    cx: &mut Context,
  ) -> Poll<Option<Self::Item>> {
    let self_inner = self.get_mut();
    let result = ready!(self_inner.inner.try_poll_next_unpin(cx)).unwrap();
    match result {
      Ok(event) => Poll::Ready(Some(Ok((self_inner.id, event)))),
      Err(e) => Poll::Ready(Some(Err((self_inner.id, e)))),
    }
  }
}

impl ImportStream for DynImport {
  fn register(
    &mut self,
    source_code_info: SourceCodeInfo,
    isolate: &mut EsIsolate,
  ) -> Result<(), ErrBox> {
    self.inner.register(source_code_info, isolate)
  }
}

pub struct ModuleInfo {
  pub main: bool,
  pub name: String,
  pub handle: v8::Global<v8::Module>,
  pub import_specifiers: Vec<String>,
}

/// A single execution context of JavaScript. Corresponds roughly to the "Web
/// Worker" concept in the DOM. An Isolate is a Future that can be used with
/// Tokio.  The Isolate future complete when there is an error or when all
/// pending ops have completed.
///
/// Ops are created in JavaScript by calling Deno.core.dispatch(), and in Rust
/// by implementing dispatcher function that takes control buffer and optional zero copy buffer
/// as arguments. An async Op corresponds exactly to a Promise in JavaScript.
pub struct EsIsolate {
  core_isolate: Box<Isolate>,

  mods_: HashMap<ModuleId, ModuleInfo>,
  pub(crate) next_dyn_import_id: DynImportId,
  pub(crate) dyn_import_map:
    HashMap<DynImportId, v8::Global<v8::PromiseResolver>>,
  pub(crate) resolve_context: *mut c_void,

  pending_dyn_imports: FuturesUnordered<StreamFuture<IntoStream<DynImport>>>,
  dyn_import: Option<Arc<DynImportFn>>,
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
      let mut hs = v8::HandleScope::new(&mut locker);
      let scope = hs.enter();
      for module in self.mods_.values_mut() {
        module.handle.reset(scope);
      }
      for handle in self.dyn_import_map.values_mut() {
        handle.reset(scope);
      }
    }
  }
}

/// Called during mod_instantiate() to resolve imports.
type ResolveFn<'a> = dyn FnMut(&str, ModuleId) -> ModuleId + 'a;

/// Used internally by Isolate::mod_instantiate to wrap ResolveFn and
/// encapsulate pointer casts.
pub struct ResolveContext<'a> {
  pub resolve_fn: &'a mut ResolveFn<'a>,
}

impl<'a> ResolveContext<'a> {
  #[inline]
  fn as_raw_ptr(&mut self) -> *mut c_void {
    self as *mut _ as *mut c_void
  }

  #[allow(clippy::missing_safety_doc)]
  #[inline]
  pub(crate) unsafe fn from_raw_ptr(ptr: *mut c_void) -> &'a mut Self {
    &mut *(ptr as *mut _)
  }
}

impl EsIsolate {
  pub fn new(startup_data: StartupData, will_snapshot: bool) -> Box<Self> {
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
      core_isolate,
      mods_: HashMap::new(),
      next_dyn_import_id: 0,
      dyn_import_map: HashMap::new(),
      resolve_context: std::ptr::null_mut(),
      dyn_import: None,
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

    let mut hs = v8::HandleScope::new(&mut locker);
    let scope = hs.enter();
    assert!(!self.core_isolate.global_context.is_empty());
    let mut context = self.core_isolate.global_context.get(scope).unwrap();
    context.enter();

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
      context.exit();
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
    self.mods_.insert(
      id,
      ModuleInfo {
        main,
        name: name.to_string(),
        import_specifiers,
        handle,
      },
    );
    context.exit();
    id
  }

  /// Low-level module creation.
  pub fn mod_new(
    &mut self,
    main: bool,
    name: &str,
    source: &str,
  ) -> Result<ModuleId, ErrBox> {
    let id = self.mod_new2(main, name, source);
    self.core_isolate.check_last_exception().map(|_| id)
  }

  pub fn mod_get_imports(&self, id: ModuleId) -> Vec<String> {
    let info = self.get_module_info(id).unwrap();
    let len = info.import_specifiers.len();
    let mut out = Vec::new();
    for i in 0..len {
      let info = self.get_module_info(id).unwrap();
      let specifier = info.import_specifiers.get(i).unwrap().to_string();
      out.push(specifier);
    }
    out
  }

  fn mod_instantiate2(&mut self, mut ctx: ResolveContext<'_>, id: ModuleId) {
    self.resolve_context = ctx.as_raw_ptr();
    let isolate = self.core_isolate.v8_isolate.as_ref().unwrap();
    let mut locker = v8::Locker::new(isolate);
    let mut hs = v8::HandleScope::new(&mut locker);
    let scope = hs.enter();
    assert!(!self.core_isolate.global_context.is_empty());
    let mut context = self.core_isolate.global_context.get(scope).unwrap();
    context.enter();
    let mut try_catch = v8::TryCatch::new(scope);
    let tc = try_catch.enter();

    let maybe_info = self.get_module_info(id);

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

    context.exit();
    self.resolve_context = std::ptr::null_mut();
  }
  /// Instanciates a ES module
  ///
  /// ErrBox can be downcast to a type that exposes additional information about
  /// the V8 exception. By default this type is CoreJSError, however it may be a
  /// different type if Isolate::set_js_error_create() has been used.
  pub fn mod_instantiate(
    &mut self,
    id: ModuleId,
    resolve_fn: &mut ResolveFn,
  ) -> Result<(), ErrBox> {
    let ctx = ResolveContext { resolve_fn };
    self.mod_instantiate2(ctx, id);
    self.core_isolate.check_last_exception()
  }

  /// Evaluates an already instantiated ES module.
  ///
  /// ErrBox can be downcast to a type that exposes additional information about
  /// the V8 exception. By default this type is CoreJSError, however it may be a
  /// different type if Isolate::set_js_error_create() has been used.
  pub fn mod_evaluate(&mut self, id: ModuleId) -> Result<(), ErrBox> {
    self.shared_init();
    let isolate = self.core_isolate.v8_isolate.as_ref().unwrap();
    let mut locker = v8::Locker::new(isolate);
    let mut hs = v8::HandleScope::new(&mut locker);
    let scope = hs.enter();
    assert!(!self.core_isolate.global_context.is_empty());
    let mut context = self.core_isolate.global_context.get(scope).unwrap();
    context.enter();

    let info = self.get_module_info(id).expect("ModuleInfo not found");
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

    context.exit();

    self.core_isolate.check_last_exception()
  }

  pub fn get_module_info(&self, id: ModuleId) -> Option<&ModuleInfo> {
    if id == 0 {
      return None;
    }
    self.mods_.get(&id)
  }

  pub fn set_dyn_import<F>(&mut self, f: F)
  where
    F: Fn(DynImportId, &str, &str) -> DynImportStream + Send + Sync + 'static,
  {
    self.dyn_import = Some(Arc::new(f));
  }

  pub fn dyn_import_cb(
    &mut self,
    specifier: &str,
    referrer: &str,
    id: DynImportId,
  ) {
    debug!("dyn_import specifier {} referrer {} ", specifier, referrer);

    if let Some(ref f) = self.dyn_import {
      let inner = f(id, specifier, referrer);
      let stream = DynImport { inner, id };
      self.waker.wake();
      self
        .pending_dyn_imports
        .push(stream.into_stream().into_future());
    } else {
      panic!("dyn_import callback not set")
    }
  }

  fn dyn_import_done(
    &mut self,
    id: DynImportId,
    result: Result<ModuleId, Option<String>>,
  ) -> Result<(), ErrBox> {
    debug!("dyn_import_done {} {:?}", id, result);
    let (mod_id, maybe_err_str) = match result {
      Ok(mod_id) => (mod_id, None),
      Err(None) => (0, None),
      Err(Some(err_str)) => (0, Some(err_str)),
    };

    assert!(
      (mod_id == 0 && maybe_err_str.is_some())
        || (mod_id != 0 && maybe_err_str.is_none())
        || (mod_id == 0 && !self.core_isolate.last_exception_handle.is_empty())
    );

    let isolate = self.core_isolate.v8_isolate.as_ref().unwrap();
    let mut locker = v8::Locker::new(isolate);
    let mut hs = v8::HandleScope::new(&mut locker);
    let scope = hs.enter();
    assert!(!self.core_isolate.global_context.is_empty());
    let mut context = self.core_isolate.global_context.get(scope).unwrap();
    context.enter();

    // TODO(ry) error on bad import_id.
    let mut resolver_handle = self.dyn_import_map.remove(&id).unwrap();
    // Resolve.
    let mut resolver = resolver_handle.get(scope).unwrap();
    resolver_handle.reset(scope);

    let maybe_info = self.get_module_info(mod_id);

    if let Some(info) = maybe_info {
      // Resolution success
      let mut module = info.handle.get(scope).unwrap();
      assert_eq!(module.get_status(), v8::ModuleStatus::Evaluated);
      let module_namespace = module.get_module_namespace();
      resolver.resolve(context, module_namespace).unwrap();
    } else {
      // Resolution error.
      if let Some(error_str) = maybe_err_str {
        let msg = v8::String::new(scope, &error_str).unwrap();
        let isolate = context.get_isolate();
        isolate.enter();
        let e = v8::type_error(scope, msg);
        isolate.exit();
        resolver.reject(context, e).unwrap();
      } else {
        let e = self.core_isolate.last_exception_handle.get(scope).unwrap();
        self.core_isolate.last_exception_handle.reset(scope);
        self.core_isolate.last_exception.take();
        resolver.reject(context, e).unwrap();
      }
    }

    isolate.run_microtasks();

    context.exit();
    self.core_isolate.check_last_exception()
  }

  fn poll_dyn_imports(&mut self, cx: &mut Context) -> Poll<Result<(), ErrBox>> {
    use RecursiveLoadEvent::*;
    loop {
      match self.pending_dyn_imports.poll_next_unpin(cx) {
        Poll::Pending | Poll::Ready(None) => {
          // There are no active dynamic import loaders, or none are ready.
          return Poll::Ready(Ok(()));
        }
        Poll::Ready(Some((
          Some(Ok((dyn_import_id, Fetch(source_code_info)))),
          mut stream,
        ))) => {
          // A module (not necessarily the one dynamically imported) has been
          // fetched. Create and register it, and if successful, poll for the
          // next recursive-load event related to this dynamic import.
          match stream.get_mut().register(source_code_info, self) {
            Ok(()) => self.pending_dyn_imports.push(stream.into_future()),
            Err(err) => {
              self.dyn_import_done(dyn_import_id, Err(Some(err.to_string())))?
            }
          }
        }
        Poll::Ready(Some((
          Some(Ok((dyn_import_id, Instantiate(module_id)))),
          _,
        ))) => {
          // The top-level module from a dynamic import has been instantiated.
          match self.mod_evaluate(module_id) {
            Ok(()) => self.dyn_import_done(dyn_import_id, Ok(module_id))?,
            Err(..) => self.dyn_import_done(dyn_import_id, Err(None))?,
          }
        }
        Poll::Ready(Some((Some(Err((dyn_import_id, err))), _))) => {
          // A non-javascript error occurred; this could be due to a an invalid
          // module specifier, or a problem with the source map, or a failure
          // to fetch the module source code.
          self.dyn_import_done(dyn_import_id, Err(Some(err.to_string())))?
        }
        Poll::Ready(Some((None, _))) => unreachable!(),
      }
    }
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
  use crate::isolate::PinnedBuf;
  use crate::ops::*;
  use std::io;
  use std::sync::atomic::{AtomicUsize, Ordering};
  use std::sync::Mutex;

  pub fn setup() -> (Box<EsIsolate>, Arc<AtomicUsize>) {
    let dispatch_count = Arc::new(AtomicUsize::new(0));
    let dispatch_count_ = dispatch_count.clone();

    let mut isolate = EsIsolate::new(StartupData::None, false);

    let dispatcher =
      move |control: &[u8], _zero_copy: Option<PinnedBuf>| -> CoreOp {
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
    (isolate, dispatch_count)
  }

  #[test]
  fn test_mods() {
    let (mut isolate, dispatch_count) = setup();
    let mod_a = isolate
      .mod_new(
        true,
        "a.js",
        r#"
        import { b } from 'b.js'
        if (b() != 'b') throw Error();
        let control = new Uint8Array([42]);
        Deno.core.send(1, control);
      "#,
      )
      .unwrap();
    assert_eq!(dispatch_count.load(Ordering::Relaxed), 0);

    let imports = isolate.mod_get_imports(mod_a);
    assert_eq!(imports, vec!["b.js".to_string()]);
    let mod_b = isolate
      .mod_new(false, "b.js", "export function b() { return 'b' }")
      .unwrap();
    let imports = isolate.mod_get_imports(mod_b);
    assert_eq!(imports.len(), 0);

    let resolve_count = Arc::new(AtomicUsize::new(0));
    let resolve_count_ = resolve_count.clone();

    let mut resolve = move |specifier: &str, _referrer: ModuleId| -> ModuleId {
      resolve_count_.fetch_add(1, Ordering::SeqCst);
      assert_eq!(specifier, "b.js");
      mod_b
    };

    js_check(isolate.mod_instantiate(mod_b, &mut resolve));
    assert_eq!(dispatch_count.load(Ordering::Relaxed), 0);
    assert_eq!(resolve_count.load(Ordering::SeqCst), 0);

    js_check(isolate.mod_instantiate(mod_a, &mut resolve));
    assert_eq!(dispatch_count.load(Ordering::Relaxed), 0);
    assert_eq!(resolve_count.load(Ordering::SeqCst), 1);

    js_check(isolate.mod_evaluate(mod_a));
    assert_eq!(dispatch_count.load(Ordering::Relaxed), 1);
    assert_eq!(resolve_count.load(Ordering::SeqCst), 1);
  }

  struct MockImportStream(Vec<Result<RecursiveLoadEvent, ErrBox>>);

  impl Stream for MockImportStream {
    type Item = Result<RecursiveLoadEvent, ErrBox>;

    fn poll_next(
      self: Pin<&mut Self>,
      _cx: &mut Context,
    ) -> Poll<Option<Self::Item>> {
      let inner = self.get_mut();
      let event = if inner.0.is_empty() {
        None
      } else {
        Some(inner.0.remove(0))
      };
      Poll::Ready(event)
    }
  }

  impl ImportStream for MockImportStream {
    fn register(
      &mut self,
      module_data: SourceCodeInfo,
      isolate: &mut EsIsolate,
    ) -> Result<(), ErrBox> {
      let id = isolate.mod_new(
        false,
        &module_data.module_url_found,
        &module_data.code,
      )?;
      println!(
        "MockImportStream register {} {}",
        id, module_data.module_url_found
      );
      Ok(())
    }
  }

  #[test]
  fn dyn_import_err() {
    // Test an erroneous dynamic import where the specified module isn't found.
    run_in_task(|cx| {
      let count = Arc::new(AtomicUsize::new(0));
      let count_ = count.clone();
      let mut isolate = EsIsolate::new(StartupData::None, false);
      isolate.set_dyn_import(move |_, specifier, referrer| {
        count_.fetch_add(1, Ordering::Relaxed);
        assert_eq!(specifier, "foo.js");
        assert_eq!(referrer, "dyn_import2.js");
        let err = io::Error::from(io::ErrorKind::NotFound);
        let stream = MockImportStream(vec![Err(err.into())]);
        Box::new(stream)
      });
      js_check(isolate.execute(
        "dyn_import2.js",
        r#"
        (async () => {
          await import("foo.js");
        })();
        "#,
      ));
      assert_eq!(count.load(Ordering::Relaxed), 1);

      // We should get an error here.
      let result = isolate.poll_unpin(cx);
      if let Poll::Ready(Ok(_)) = result {
        unreachable!();
      }
    })
  }

  #[test]
  fn dyn_import_err2() {
    use std::convert::TryInto;
    // Import multiple modules to demonstrate that after failed dynamic import
    // another dynamic import can still be run
    run_in_task(|cx| {
      let count = Arc::new(AtomicUsize::new(0));
      let count_ = count.clone();
      let mut isolate = EsIsolate::new(StartupData::None, false);
      isolate.set_dyn_import(move |_, specifier, referrer| {
        let c = count_.fetch_add(1, Ordering::Relaxed);
        match c {
          0 => assert_eq!(specifier, "foo1.js"),
          1 => assert_eq!(specifier, "foo2.js"),
          2 => assert_eq!(specifier, "foo3.js"),
          _ => unreachable!(),
        }
        assert_eq!(referrer, "dyn_import_error.js");

        let source_code_info = SourceCodeInfo {
          module_url_specified: specifier.to_owned(),
          module_url_found: specifier.to_owned(),
          code: "# not valid JS".to_owned(),
        };
        let stream = MockImportStream(vec![
          Ok(RecursiveLoadEvent::Fetch(source_code_info)),
          Ok(RecursiveLoadEvent::Instantiate(c.try_into().unwrap())),
        ]);
        Box::new(stream)
      });

      js_check(isolate.execute(
        "dyn_import_error.js",
        r#"
        (async () => {
          await import("foo1.js");
        })();
        (async () => {
          await import("foo2.js");
        })();
        (async () => {
          await import("foo3.js");
        })();
        "#,
      ));

      assert_eq!(count.load(Ordering::Relaxed), 3);
      // Now each poll should return error
      assert!(match isolate.poll_unpin(cx) {
        Poll::Ready(Err(_)) => true,
        _ => false,
      });
      assert!(match isolate.poll_unpin(cx) {
        Poll::Ready(Err(_)) => true,
        _ => false,
      });
      assert!(match isolate.poll_unpin(cx) {
        Poll::Ready(Err(_)) => true,
        _ => false,
      });
    })
  }

  #[test]
  fn dyn_import_ok() {
    run_in_task(|cx| {
      let count = Arc::new(AtomicUsize::new(0));
      let count_ = count.clone();

      // Sometimes Rust is really annoying.
      let mod_b = Arc::new(Mutex::new(0));
      let mod_b2 = mod_b.clone();

      let mut isolate = EsIsolate::new(StartupData::None, false);
      isolate.set_dyn_import(move |_id, specifier, referrer| {
        let c = count_.fetch_add(1, Ordering::Relaxed);
        match c {
          0 => assert_eq!(specifier, "foo1.js"),
          1 => assert_eq!(specifier, "foo2.js"),
          _ => unreachable!(),
        }
        assert_eq!(referrer, "dyn_import3.js");
        let mod_id = *mod_b2.lock().unwrap();
        let source_code_info = SourceCodeInfo {
          module_url_specified: "foo.js".to_owned(),
          module_url_found: "foo.js".to_owned(),
          code: "".to_owned(),
        };
        let stream = MockImportStream(vec![
          Ok(RecursiveLoadEvent::Fetch(source_code_info)),
          Ok(RecursiveLoadEvent::Instantiate(mod_id)),
        ]);
        Box::new(stream)
      });

      // Instantiate mod_b
      {
        let mut mod_id = mod_b.lock().unwrap();
        *mod_id = isolate
          .mod_new(false, "b.js", "export function b() { return 'b' }")
          .unwrap();
        let mut resolve = move |_specifier: &str,
                                _referrer: ModuleId|
              -> ModuleId { unreachable!() };
        js_check(isolate.mod_instantiate(*mod_id, &mut resolve));
      }
      // Dynamically import mod_b
      js_check(isolate.execute(
        "dyn_import3.js",
        r#"
          (async () => {
            let mod = await import("foo1.js");
            if (mod.b() !== 'b') {
              throw Error("bad1");
            }
            // And again!
            mod = await import("foo2.js");
            if (mod.b() !== 'b') {
              throw Error("bad2");
            }
          })();
          "#,
      ));

      assert_eq!(count.load(Ordering::Relaxed), 1);
      assert!(match isolate.poll_unpin(cx) {
        Poll::Ready(Ok(_)) => true,
        _ => false,
      });
      assert_eq!(count.load(Ordering::Relaxed), 2);
      assert!(match isolate.poll_unpin(cx) {
        Poll::Ready(Ok(_)) => true,
        _ => false,
      });
      assert_eq!(count.load(Ordering::Relaxed), 2);
    })
  }
}
