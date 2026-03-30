// Copyright 2018-2026 the Deno authors. MIT license.

use std::cell::Cell;
use std::cell::RefCell;
use std::rc::Rc;

use deno_core::CppgcBase;
use deno_core::CppgcInherits;
use deno_core::GarbageCollected;
use deno_core::OpState;
use deno_core::ResourceId;
use deno_core::error::ResourceError;
use deno_core::op2;
use deno_core::uv_compat;
use deno_core::uv_compat::uv_handle_t;
use deno_core::v8;

// ---------------------------------------------------------------------------
// GlobalHandle — mirrors Node's BaseObject::persistent_handle_ weak/strong
// switching.
//
// In Node, BaseObject holds a v8::Global that can be switched between strong
// (prevents GC of the JS object) and weak (allows GC, triggering cleanup).
// With cppgc the C++ object's lifetime is managed by the GC, but we still
// need to hold a reference back to the JS wrapper object for use in
// callbacks. A strong Global would create a reference cycle (JS -> cppgc
// -> Global -> JS) and leak. A Weak reference allows the GC to collect
// the pair when nothing else references them.
//
// The handle starts Strong after construction (the active libuv handle
// should keep the object alive). It should be made Weak when the handle
// is closed or no longer actively referenced.
// ---------------------------------------------------------------------------

/// A reference to a JS object that can switch between strong (GC root),
/// weak (allows collection), or empty. This mirrors Node's pattern of
/// calling `MakeWeak()` / `ClearWeak()` on `BaseObject::persistent_handle_`.
#[derive(Default)]
pub enum GlobalHandle<T> {
  Strong(v8::Global<T>),
  Weak(v8::Weak<T>),
  #[default]
  None,
}

impl<T> GlobalHandle<T>
where
  v8::Global<T>: v8::Handle<Data = T>,
{
  /// Create a new strong handle.
  pub fn new_strong(scope: &mut v8::PinScope, value: v8::Local<T>) -> Self {
    GlobalHandle::Strong(v8::Global::new(scope, value))
  }

  /// Create a new weak handle.
  pub fn new_weak(scope: &mut v8::PinScope, value: v8::Local<T>) -> Self {
    GlobalHandle::Weak(v8::Weak::new(scope, value))
  }

  /// Make the handle weak, allowing the GC to collect the JS object.
  /// Mirrors Node's `BaseObject::MakeWeak()`.
  pub fn make_weak(&mut self, scope: &mut v8::PinScope) {
    match std::mem::take(self) {
      GlobalHandle::Strong(global) => {
        let local = v8::Local::new(scope, &global);
        *self = GlobalHandle::Weak(v8::Weak::new(scope, local));
      }
      other => *self = other,
    }
  }

  /// Make the handle strong, preventing the GC from collecting the JS object.
  /// Mirrors Node's `BaseObject::ClearWeak()`.
  pub fn make_strong(&mut self, scope: &mut v8::PinScope) {
    match std::mem::take(self) {
      GlobalHandle::Weak(weak) => {
        if let Some(local) = weak.to_local(scope) {
          *self = GlobalHandle::Strong(v8::Global::new(scope, local));
        }
        // If already collected, stays None
      }
      other => *self = other,
    }
  }

  /// Get a Global clone if the reference is still alive.
  /// Returns None if empty or if the weak reference has been collected.
  pub fn to_global(&self, scope: &mut v8::PinScope) -> Option<v8::Global<T>> {
    match self {
      GlobalHandle::Strong(global) => Some(global.clone()),
      GlobalHandle::Weak(weak) => weak
        .to_local(scope)
        .map(|local| v8::Global::new(scope, local)),
      GlobalHandle::None => None,
    }
  }

  /// Returns true if this is a weak reference.
  pub fn is_weak(&self) -> bool {
    matches!(self, GlobalHandle::Weak(_))
  }

  /// Returns true if this handle is empty.
  pub fn is_none(&self) -> bool {
    matches!(self, GlobalHandle::None)
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum ProviderType {
  None = 0,
  DirHandle,
  DnsChannel,
  EldHistogram,
  FileHandle,
  FileHandleCloseReq,
  FixedSizeBlobCopy,
  FsEventWrap,
  FsReqCallback,
  FsReqPromise,
  GetAddrInfoReqWrap,
  GetNameInfoReqWrap,
  HeapSnapshot,
  Http2Session,
  Http2Stream,
  Http2Ping,
  Http2Settings,
  HttpIncomingMessage,
  HttpClientRequest,
  JsStream,
  JsUdpWrap,
  MessagePort,
  PipeConnectWrap,
  PipeServerWrap,
  PipeWrap,
  ProcessWrap,
  Promise,
  QueryWrap,
  ShutdownWrap,
  SignalWrap,
  StatWatcher,
  StreamPipe,
  TcpConnectWrap,
  TcpServerWrap,
  TcpWrap,
  TtyWrap,
  UdpSendWrap,
  UdpWrap,
  SigIntWatchdog,
  Worker,
  WorkerHeapSnapshot,
  WriteWrap,
  Zlib,
}

impl From<ProviderType> for i32 {
  fn from(provider: ProviderType) -> Self {
    provider as i32
  }
}

pub use deno_core::uv_compat::AsyncId;

fn next_async_id(state: &mut OpState) -> i64 {
  state.borrow_mut::<AsyncId>().next()
}

#[op2(fast)]
pub fn op_node_new_async_id(state: &mut OpState) -> f64 {
  next_async_id(state) as f64
}

#[derive(CppgcBase)]
#[repr(C)]
pub struct AsyncWrap {
  provider: i32,
  async_id: i64,
}

// SAFETY: we're sure this can be GCed
unsafe impl GarbageCollected for AsyncWrap {
  fn trace(&self, _visitor: &mut deno_core::v8::cppgc::Visitor) {}

  fn get_name(&self) -> &'static std::ffi::CStr {
    c"AsyncWrap"
  }
}

impl AsyncWrap {
  pub(crate) fn create(state: &mut OpState, provider: i32) -> Self {
    let async_id = next_async_id(state);

    Self { provider, async_id }
  }
}

#[op2(base)]
impl AsyncWrap {
  #[getter]
  fn provider(&self) -> i32 {
    self.provider
  }

  #[fast]
  fn get_async_id(&self) -> f64 {
    self.async_id as f64
  }

  #[fast]
  fn get_provider_type(&self) -> i32 {
    self.provider
  }
}

#[derive(Copy, Clone, PartialEq, Eq, Default)]
enum State {
  #[default]
  Initialized,
  Closing,
  Closed,
}

/// A handle to a libuv resource. `New` stores a raw pointer to a `uv_handle_t`
/// whose lifetime is managed by the owning cppgc object (e.g. `TTY`). The
/// pointer is valid as long as the handle has not been closed via `uv_close`.
/// This mirrors Node's approach where `HandleWrap` stores a `uv_handle_t*`
/// that becomes null after close.
#[derive(PartialEq, Eq)]
pub enum Handle {
  Old(ResourceId),
  New(*const uv_handle_t),
}

#[derive(CppgcBase, CppgcInherits)]
#[cppgc_inherits_from(AsyncWrap)]
#[repr(C)]
pub struct HandleWrap {
  base: AsyncWrap,
  handle: Option<Handle>,
  state: Rc<Cell<State>>,
}

// SAFETY: we're sure this can be GCed
unsafe impl GarbageCollected for HandleWrap {
  fn trace(&self, _visitor: &mut deno_core::v8::cppgc::Visitor) {}

  fn get_name(&self) -> &'static std::ffi::CStr {
    c"HandleWrap"
  }
}

impl HandleWrap {
  pub(crate) fn create(base: AsyncWrap, handle: Option<Handle>) -> Self {
    Self {
      base,
      handle,
      state: Rc::new(Cell::new(State::Initialized)),
    }
  }

  fn is_alive(&self) -> bool {
    self.state.get() != State::Closed
  }
}

static ON_CLOSE_STR: deno_core::FastStaticString =
  deno_core::ascii_str!("_onClose");

#[op2(inherit = AsyncWrap, base)]
impl HandleWrap {
  #[constructor]
  #[cppgc]
  fn new(
    state: &mut OpState,
    #[smi] provider: i32,
    #[smi] handle: Option<ResourceId>,
  ) -> HandleWrap {
    HandleWrap::create(
      AsyncWrap::create(state, provider),
      handle.map(Handle::Old),
    )
  }

  // Ported from Node.js
  //
  // https://github.com/nodejs/node/blob/038d82980ab26cd79abe4409adc2fecad94d7c93/src/handle_wrap.cc#L65-L85
  #[reentrant]
  fn close(
    &self,
    op_state: Rc<RefCell<OpState>>,
    #[this] this: v8::Global<v8::Object>,
    scope: &mut v8::PinScope<'_, '_>,
    #[scoped] cb: Option<v8::Global<v8::Function>>,
  ) -> Result<(), ResourceError> {
    if self.state.get() != State::Initialized {
      return Ok(());
    }

    let state = self.state.clone();
    // This effectively mimicks Node's OnClose callback.
    //
    // https://github.com/nodejs/node/blob/038d82980ab26cd79abe4409adc2fecad94d7c93/src/handle_wrap.cc#L135-L157
    let on_close = move |scope: &mut v8::PinScope<'_, '_>| {
      assert!(state.get() == State::Closing);
      state.set(State::Closed);

      // Workaround for https://github.com/denoland/deno/pull/24656
      //
      // We need to delay 'cb' at least 2 ticks to avoid "close" event happening before "error"
      // event in net.Socket.
      //
      // This is a temporary solution. We should support async close like `uv_close`.
      if let Some(cb) = cb {
        let recv = v8::undefined(scope);
        cb.open(scope).call(scope, recv.into(), &[]);
      }
    };

    // For new-style handles (uv_compat), call uv_compat::uv_close to
    // properly shut down the libuv handle (e.g. close FDs for TTY).
    // Without this, the libuv handle cleanup never runs and resources
    // like PTY master file descriptors are leaked.
    if let Some(Handle::New(handle)) = &self.handle {
      // SAFETY: handle is a valid uv_handle_t pointer set during
      // construction and remains live while HandleWrap is alive.
      unsafe {
        uv_compat::uv_close(handle.cast_mut(), None);
      }
    }

    uv_close(scope, op_state, this, on_close);
    self.state.set(State::Closing);

    Ok(())
  }

  // Ported from Node.js
  //
  // https://github.com/nodejs/node/blob/038d82980ab26cd79abe4409adc2fecad94d7c93/src/handle_wrap.cc#L58-L62
  #[fast]
  fn has_ref(&self, state: &mut OpState) -> bool {
    if let Some(handle) = &self.handle {
      return match handle {
        Handle::Old(resource_id) => state.has_ref(*resource_id),
        // SAFETY: handle is a valid uv_handle_t pointer set during construction and remains live while HandleWrap is alive.
        Handle::New(handle) => unsafe { uv_compat::uv_has_ref(*handle) != 0 },
      };
    }

    true
  }

  // Ported from Node.js
  //
  // https://github.com/nodejs/node/blob/038d82980ab26cd79abe4409adc2fecad94d7c93/src/handle_wrap.cc#L40-L46
  #[fast]
  #[rename("ref")]
  fn ref_method(&self, state: &mut OpState) {
    if self.is_alive()
      && let Some(handle) = &self.handle
    {
      match handle {
        Handle::Old(resource_id) => state.uv_ref(*resource_id),
        // SAFETY: handle is a valid uv_handle_t pointer set during construction and remains live while HandleWrap is alive.
        Handle::New(handle) => unsafe { uv_compat::uv_ref(handle.cast_mut()) },
      }
    }
  }

  // Ported from Node.js
  //
  // https://github.com/nodejs/node/blob/038d82980ab26cd79abe4409adc2fecad94d7c93/src/handle_wrap.cc#L49-L55
  #[fast]
  fn unref(&self, state: &mut OpState) {
    if self.is_alive()
      && let Some(handle) = &self.handle
    {
      match handle {
        Handle::Old(resource_id) => state.uv_unref(*resource_id),
        // SAFETY: handle is a valid uv_handle_t pointer set during construction and remains live while HandleWrap is alive.
        Handle::New(handle) => unsafe {
          uv_compat::uv_unref(handle.cast_mut())
        },
      }
    }
  }
}

fn uv_close<F>(
  scope: &mut v8::PinScope<'_, '_>,
  op_state: Rc<RefCell<OpState>>,
  this: v8::Global<v8::Object>,
  on_close: F,
) where
  F: FnOnce(&mut v8::PinScope<'_, '_>) + 'static,
{
  // Call _onClose() on the JS handles. Not needed for Rust handles.
  let this = v8::Local::new(scope, this);
  let on_close_str = ON_CLOSE_STR.v8_string(scope).unwrap();
  let onclose = this.get(scope, on_close_str.into());

  if let Some(onclose) = onclose
    && let Ok(fn_) = v8::Local::<v8::Function>::try_from(onclose)
  {
    fn_.call(scope, this.into(), &[]);
  }

  op_state
    .borrow()
    .borrow::<deno_core::V8TaskSpawner>()
    .spawn(on_close);
}

#[cfg(test)]
mod tests {
  use std::future::poll_fn;
  use std::task::Poll;

  use deno_core::JsRuntime;
  use deno_core::RuntimeOptions;

  async fn js_test(source_code: &'static str) {
    deno_core::extension!(
      test_ext,
      objects = [super::AsyncWrap, super::HandleWrap,],
      state = |state| {
        state.put::<super::AsyncId>(super::AsyncId::default());
      }
    );

    let mut runtime = JsRuntime::new(RuntimeOptions {
      extensions: vec![test_ext::init()],
      ..Default::default()
    });

    poll_fn(move |cx| {
      runtime
        .execute_script("file://handle_wrap_test.js", source_code)
        .unwrap();

      let result = runtime.poll_event_loop(cx, Default::default());
      assert!(matches!(result, Poll::Ready(Ok(()))));
      Poll::Ready(())
    })
    .await;
  }

  #[tokio::test]
  async fn test_handle_wrap() {
    js_test(
      r#"
        const { HandleWrap } = Deno.core.ops;

        let called = false;
        class MyHandleWrap extends HandleWrap {
          constructor() {
            super(0, null);
          }

          _onClose() {
            called = true;
          }
        }

        const handleWrap = new MyHandleWrap();
        handleWrap.close();

        if (!called) {
          throw new Error("HandleWrap._onClose was not called");
        }
      "#,
    )
    .await;
  }
}
