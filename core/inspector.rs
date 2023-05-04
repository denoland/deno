// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

//! The documentation for the inspector API is sparse, but these are helpful:
//! <https://chromedevtools.github.io/devtools-protocol/>
//! <https://hyperandroid.com/2020/02/12/v8-inspector-from-an-embedder-standpoint/>

use crate::error::generic_error;
use crate::futures::channel::mpsc;
use crate::futures::channel::mpsc::UnboundedReceiver;
use crate::futures::channel::mpsc::UnboundedSender;
use crate::futures::channel::oneshot;
use crate::futures::future::select;
use crate::futures::future::Either;
use crate::futures::prelude::*;
use crate::futures::stream::SelectAll;
use crate::futures::stream::StreamExt;
use crate::futures::task;
use crate::futures::task::Context;
use crate::futures::task::Poll;
use crate::serde_json;
use crate::serde_json::json;
use crate::serde_json::Value;
use anyhow::Error;
use parking_lot::Mutex;
use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::c_void;
use std::mem::replace;
use std::mem::take;
use std::mem::MaybeUninit;
use std::pin::Pin;
use std::ptr;
use std::ptr::NonNull;
use std::rc::Rc;
use std::sync::Arc;
use v8::HandleScope;

pub enum InspectorMsgKind {
  Notification,
  Message(i32),
}
pub struct InspectorMsg {
  pub kind: InspectorMsgKind,
  pub content: String,
}
pub type SessionProxySender = UnboundedSender<InspectorMsg>;
pub type SessionProxyReceiver = UnboundedReceiver<String>;

/// Encapsulates an UnboundedSender/UnboundedReceiver pair that together form
/// a duplex channel for sending/receiving messages in V8 session.
pub struct InspectorSessionProxy {
  pub tx: SessionProxySender,
  pub rx: SessionProxyReceiver,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum PollState {
  // Inspector is not being polled at this moment, it's waiting for more events
  // from the inspector.
  Idle,
  // `InspectorWaker` has been called - either explicitly by outside code
  // (like WS server), or from one of the futures we were polling.
  Woken,
  // Inspector is being polled asynchronously from the owning runtime.
  Polling,
  // Inspector is being polled synchronously, possibly in a reentrant way
  // (e.g. from a callback invoked by V8).
  SyncPolling,
  // Inspector has been dropped already, but wakers might outlive the inspector
  // so make sure nothing gets woken at this point.
  Dropped,
}

/// This structure is used responsible for providing inspector interface
/// to the `JsRuntime`.
///
/// It stores an instance of `v8::inspector::V8Inspector` and additionally
/// implements `v8::inspector::V8InspectorClientImpl`.
///
/// After creating this structure it's possible to connect multiple sessions
/// to the inspector, in case of Deno it's either: a "websocket session" that
/// provides integration with Chrome Devtools, or an "in-memory session" that
/// is used for REPL or converage collection.
pub struct JsRuntimeInspector {
  v8_inspector_client: v8::inspector::V8InspectorClientBase,
  v8_inspector: Rc<RefCell<v8::UniquePtr<v8::inspector::V8Inspector>>>,
  new_session_tx: UnboundedSender<InspectorSessionProxy>,
  sessions: RefCell<SessionContainer>,
  flags: RefCell<InspectorFlags>,
  waker: Arc<InspectorWaker>,
  deregister_tx: Option<oneshot::Sender<()>>,
  is_dispatching_message: RefCell<bool>,
}

impl Drop for JsRuntimeInspector {
  fn drop(&mut self) {
    // Since the waker is cloneable, it might outlive the inspector itself.
    // Set the poll state to 'dropped' so it doesn't attempt to request an
    // interrupt from the isolate.
    self.waker.update(|w| w.poll_state = PollState::Dropped);

    // V8 automatically deletes all sessions when an `V8Inspector` instance is
    // deleted, however InspectorSession also has a drop handler that cleans
    // up after itself. To avoid a double free, make sure the inspector is
    // dropped last.
    self.sessions.borrow_mut().drop_sessions();

    // Notify counterparty that this instance is being destroyed. Ignoring
    // result because counterparty waiting for the signal might have already
    // dropped the other end of channel.
    if let Some(deregister_tx) = self.deregister_tx.take() {
      let _ = deregister_tx.send(());
    }
  }
}

impl v8::inspector::V8InspectorClientImpl for JsRuntimeInspector {
  fn base(&self) -> &v8::inspector::V8InspectorClientBase {
    &self.v8_inspector_client
  }

  unsafe fn base_ptr(
    this: *const Self,
  ) -> *const v8::inspector::V8InspectorClientBase
  where
    Self: Sized,
  {
    // SAFETY: this pointer is valid for the whole lifetime of inspector
    unsafe { std::ptr::addr_of!((*this).v8_inspector_client) }
  }

  fn base_mut(&mut self) -> &mut v8::inspector::V8InspectorClientBase {
    &mut self.v8_inspector_client
  }

  /// This method id called when a breakpoint is triggered, eg. using `debugger` statement. In that case
  /// inspector sends `Debugger.paused` notification. Nested message loop should be run and process all
  /// sent protocol commands until `quit_message_loop_on_pause` is called. After that execution will
  /// return to inspector and then JavaScript execution will resume.
  fn run_message_loop_on_pause(&mut self, context_group_id: i32) {
    assert_eq!(context_group_id, JsRuntimeInspector::CONTEXT_GROUP_ID);
    self.flags.borrow_mut().on_pause = true;
    self.poll_sessions_sync();
    assert!(
      !self.flags.borrow().on_pause,
      "V8InspectorClientImpl::run_message_loop_on_pause returned before quit_message_loop_on_pause was called"
    );
  }

  fn quit_message_loop_on_pause(&mut self) {
    let mut flags = self.flags.borrow_mut();
    assert!(flags.on_pause);
    flags.on_pause = false;
  }

  fn run_if_waiting_for_debugger(&mut self, context_group_id: i32) {
    assert_eq!(context_group_id, JsRuntimeInspector::CONTEXT_GROUP_ID);
    self.flags.borrow_mut().waiting_for_session = false;
  }
}

/// Polling `JsRuntimeInspector` allows inspector to accept new incoming
/// connections and "pump" messages in different sessions.
///
/// It should be polled on tick of event loop, ie. in `JsRuntime::poll_event_loop`
/// function.
impl Future for JsRuntimeInspector {
  type Output = ();
  fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<()> {
    // Here we actually want to set up waker so we are notified when new
    // messages arrive. Note that other call sites might want to reenter
    // and pump sessions synchronously.
    self.poll_sessions(cx)
  }
}

impl JsRuntimeInspector {
  /// Currently Deno supports only a single context in `JsRuntime`
  /// and thus it's id is provided as an associated contant.
  const CONTEXT_GROUP_ID: i32 = 1;

  pub fn new(
    isolate: &mut v8::OwnedIsolate,
    context: v8::Global<v8::Context>,
    is_main: bool,
  ) -> Rc<RefCell<Self>> {
    let scope = &mut v8::HandleScope::new(isolate);

    let (new_session_tx, new_session_rx) =
      mpsc::unbounded::<InspectorSessionProxy>();

    let v8_inspector_client =
      v8::inspector::V8InspectorClientBase::new::<Self>();

    let waker = InspectorWaker::new(scope.thread_safe_handle());

    // Create JsRuntimeInspector instance.
    let self__ = Rc::new(RefCell::new(Self {
      v8_inspector_client,
      v8_inspector: Default::default(),
      sessions: RefCell::new(SessionContainer::temporary_placeholder()),
      new_session_tx,
      flags: Default::default(),
      waker,
      deregister_tx: None,
      is_dispatching_message: Default::default(),
    }));
    let mut self_ = self__.borrow_mut();
    self_.v8_inspector = Rc::new(RefCell::new(
      v8::inspector::V8Inspector::create(scope, &mut *self_).into(),
    ));
    self_.sessions = RefCell::new(SessionContainer::new(
      self_.v8_inspector.clone(),
      new_session_rx,
    ));

    // Tell the inspector about the global context.
    let context = v8::Local::new(scope, context);
    let context_name = v8::inspector::StringView::from(&b"global context"[..]);
    // NOTE(bartlomieju): this is what Node.js does and it turns out some
    // debuggers (like VSCode) rely on this information to disconnect after
    // program completes
    let aux_data = if is_main {
      r#"{"isDefault": true}"#
    } else {
      r#"{"isDefault": false}"#
    };
    let aux_data_view = v8::inspector::StringView::from(aux_data.as_bytes());
    self_
      .v8_inspector
      .borrow_mut()
      .as_mut()
      .unwrap()
      .context_created(
        context,
        Self::CONTEXT_GROUP_ID,
        context_name,
        aux_data_view,
      );

    self_.poll_sessions_sync();
    drop(self_);

    self__
  }

  pub fn is_dispatching_message(&self) -> bool {
    *self.is_dispatching_message.borrow()
  }

  pub fn context_destroyed(
    &mut self,
    scope: &mut HandleScope,
    context: v8::Global<v8::Context>,
  ) {
    let context = v8::Local::new(scope, context);
    self
      .v8_inspector
      .borrow_mut()
      .as_mut()
      .unwrap()
      .context_destroyed(context);
  }

  pub fn exception_thrown(
    &self,
    scope: &mut HandleScope,
    exception: v8::Local<'_, v8::Value>,
    in_promise: bool,
  ) {
    let context = scope.get_current_context();
    let message = v8::Exception::create_message(scope, exception);
    let stack_trace = message.get_stack_trace(scope).unwrap();
    let mut v8_inspector_ref = self.v8_inspector.borrow_mut();
    let v8_inspector = v8_inspector_ref.as_mut().unwrap();
    let stack_trace = v8_inspector.create_stack_trace(stack_trace);
    v8_inspector.exception_thrown(
      context,
      if in_promise {
        v8::inspector::StringView::from("Uncaught (in promise)".as_bytes())
      } else {
        v8::inspector::StringView::from("Uncaught".as_bytes())
      },
      exception,
      v8::inspector::StringView::from("".as_bytes()),
      v8::inspector::StringView::from("".as_bytes()),
      0,
      0,
      stack_trace,
      0,
    );
  }

  pub fn has_active_sessions(&self) -> bool {
    self.sessions.borrow().has_active_sessions()
  }

  pub fn has_blocking_sessions(&self) -> bool {
    self.sessions.borrow().has_blocking_sessions()
  }

  fn poll_sessions_sync(&self) {
    let (prev_poll_state, mut prev_task_waker) = self.waker.update(|w| {
      let prev_poll_state = replace(&mut w.poll_state, PollState::SyncPolling);
      assert!(prev_poll_state != PollState::SyncPolling);

      let prev_task_waker = w.task_waker.take();

      (prev_poll_state, prev_task_waker)
    });

    futures::executor::block_on(futures::future::poll_fn(|cx| {
      self.poll_sessions_inner(cx);

      // Block the thread if either the `on_pause` or the `waiting_for_session`.
      // is set. Otherwise, return `Ready(_)` to make `block_on()` return.
      let flags = self.flags.borrow();
      if flags.on_pause || flags.waiting_for_session {
        Poll::Pending
      } else {
        Poll::Ready(())
      }
    }));

    // Restore the previous poll state.
    self.waker.update(|w| {
      let replaced = replace(&mut w.poll_state, prev_poll_state);
      assert_eq!(replaced, PollState::SyncPolling);
    });

    // The `block_on(...)` call above must have created a new `Waker` that will
    // now be registered with `sessions.session_rx` and `sessions.established`.
    // This has the consequence that when either of those streams transitions
    // from `Pending` to `Ready`, they'll wake that (stale) waker, and the
    // inspector task won't get notified. To avoid a hang, explicitly wake the
    // inspector task here; when it gets polled, it will re-register the right
    // waker (the `InspectorWaker`) with those streams.
    if let Some(waker) = prev_task_waker.take() {
      waker.wake();
    }
  }

  fn poll_sessions(&self, invoker_cx: &mut Context) -> Poll<()> {
    self.waker.update(|w| {
      match w.poll_state {
        PollState::Idle | PollState::Woken => {
          w.poll_state = PollState::Polling;
          w.inspector_ptr = Some(NonNull::from(self));
        }
        s => unreachable!("state in poll_sessions {:#?}", s),
      };
    });

    // Create a new Context object that will make downstream futures
    // use the InspectorWaker when they are ready to be polled again.
    let waker_ref = task::waker_ref(&self.waker);
    let cx = &mut Context::from_waker(&waker_ref);

    loop {
      self.poll_sessions_inner(cx);

      {
        let flags = self.flags.borrow();
        assert!(!flags.on_pause);
        assert!(!flags.waiting_for_session);
      }

      let new_poll_state = self.waker.update(|w| {
        match w.poll_state {
          PollState::Woken => {
            // The inspector got woken up before the last round of polling was
            // even over, so we need to do another round.
            w.poll_state = PollState::Polling;
          }
          PollState::Polling => {
            // Since all streams were polled until they all yielded `Pending`,
            // there's nothing else we can do right now.
            w.poll_state = PollState::Idle;
            // Capture the waker that, when used, will get the inspector polled.
            w.task_waker.replace(invoker_cx.waker().clone());
          }
          _ => unreachable!(),
        };
        w.poll_state
      });

      match new_poll_state {
        PollState::Idle => break Poll::Pending,
        PollState::Polling => continue, // Poll the session handler again.
        _ => unreachable!(),
      };
    }
  }

  /// Accepts incoming connections from inspector clients, and polls established
  /// inspector sessions for messages that need to be dispatched to V8. This
  /// function will repeatedly poll its innner streams and will not return until
  /// they all yield `Pending` or have ended.
  fn poll_sessions_inner(&self, cx: &mut Context) {
    loop {
      let mut sessions = self.sessions.borrow_mut();

      // Accept new connections.
      let poll_result = sessions.session_rx.poll_next_unpin(cx);
      match poll_result {
        Poll::Ready(Some(session_proxy)) => {
          let session = InspectorSession::new(
            self.v8_inspector.clone(),
            session_proxy,
            false,
          );
          sessions.established.push(session);
          // `session_rx` needs to be polled repeatedly until it is `Pending`.
          continue;
        }
        Poll::Ready(None) => unreachable!(), // `session_rx` should never end.
        Poll::Pending => {}
      }

      // Poll established inspector sessions.
      let poll_result = sessions.established.poll_next_unpin(cx);
      if let Poll::Ready(Some(session_stream_item)) = poll_result {
        let (v8_session_ptr, msg) = session_stream_item;
        // Don't hold the borrow on sessions while dispatching a message, as it
        // might result in a call to `poll_sessions_sync`.
        drop(sessions);
        InspectorSession::dispatch_message(v8_session_ptr, msg);
        // Loop around. We need to keep polling established sessions and
        // accepting new ones until eventually everything is `Pending`.
        continue;
      }

      break;
    }
  }

  /// This function blocks the thread until at least one inspector client has
  /// established a websocket connection.
  pub fn wait_for_session(&mut self) {
    loop {
      match self.sessions.get_mut().established.iter_mut().next() {
        Some(_session) => {
          self.flags.get_mut().waiting_for_session = false;
          break;
        }
        None => {
          self.flags.get_mut().waiting_for_session = true;
          self.poll_sessions_sync();
        }
      };
    }
  }

  /// This function blocks the thread until at least one inspector client has
  /// established a websocket connection.
  ///
  /// After that, it instructs V8 to pause at the next statement.
  /// Frontend must send "Runtime.runIfWaitingForDebugger" message to resume
  /// execution.
  pub fn wait_for_session_and_break_on_next_statement(&mut self) {
    loop {
      match self.sessions.get_mut().established.iter_mut().next() {
        Some(session) => {
          break session.break_on_next_statement();
        }
        None => {
          self.flags.get_mut().waiting_for_session = true;
          self.poll_sessions_sync();
        }
      };
    }
  }

  /// Obtain a sender for proxy channels.
  pub fn get_session_sender(&self) -> UnboundedSender<InspectorSessionProxy> {
    self.new_session_tx.clone()
  }

  /// Create a channel that notifies the frontend when inspector is dropped.
  ///
  /// NOTE: Only a single handler is currently available.
  pub fn add_deregister_handler(&mut self) -> oneshot::Receiver<()> {
    let (tx, rx) = oneshot::channel::<()>();
    let prev = self.deregister_tx.replace(tx);
    assert!(
      prev.is_none(),
      "Only a single deregister handler is allowed"
    );
    rx
  }

  /// Create a local inspector session that can be used on
  /// the same thread as the isolate.
  pub fn create_local_session(&self) -> LocalInspectorSession {
    // The 'outbound' channel carries messages sent to the session.
    let (outbound_tx, outbound_rx) = mpsc::unbounded();

    // The 'inbound' channel carries messages received from the session.
    let (inbound_tx, inbound_rx) = mpsc::unbounded();

    let proxy = InspectorSessionProxy {
      tx: outbound_tx,
      rx: inbound_rx,
    };

    // InspectorSessions for a local session is added directly to the "established"
    // sessions, so it doesn't need to go through the session sender.
    let inspector_session =
      InspectorSession::new(self.v8_inspector.clone(), proxy, true);
    self
      .sessions
      .borrow_mut()
      .established
      .push(inspector_session);
    take(&mut self.flags.borrow_mut().waiting_for_session);

    LocalInspectorSession::new(inbound_tx, outbound_rx)
  }
}

#[derive(Default)]
struct InspectorFlags {
  waiting_for_session: bool,
  on_pause: bool,
}

/// A helper structure that helps coordinate sessions during different
/// parts of their lifecycle.
struct SessionContainer {
  v8_inspector: Rc<RefCell<v8::UniquePtr<v8::inspector::V8Inspector>>>,
  session_rx: UnboundedReceiver<InspectorSessionProxy>,
  established: SelectAll<Box<InspectorSession>>,
}

impl SessionContainer {
  fn new(
    v8_inspector: Rc<RefCell<v8::UniquePtr<v8::inspector::V8Inspector>>>,
    new_session_rx: UnboundedReceiver<InspectorSessionProxy>,
  ) -> Self {
    Self {
      v8_inspector,
      session_rx: new_session_rx,
      established: SelectAll::new(),
    }
  }

  /// V8 automatically deletes all sessions when an `V8Inspector` instance is
  /// deleted, however InspectorSession also has a drop handler that cleans
  /// up after itself. To avoid a double free, we need to manually drop
  /// all sessions before dropping the inspector instance.
  fn drop_sessions(&mut self) {
    self.v8_inspector = Default::default();
    self.established.clear();
  }

  fn has_active_sessions(&self) -> bool {
    !self.established.is_empty()
  }

  fn has_blocking_sessions(&self) -> bool {
    self.established.iter().any(|s| s.blocking)
  }

  /// A temporary placeholder that should be used before actual
  /// instance of V8Inspector is created. It's used in favor
  /// of `Default` implementation to signal that it's not meant
  /// for actual use.
  fn temporary_placeholder() -> Self {
    let (_tx, rx) = mpsc::unbounded::<InspectorSessionProxy>();
    Self {
      v8_inspector: Default::default(),
      session_rx: rx,
      established: SelectAll::new(),
    }
  }
}

struct InspectorWakerInner {
  poll_state: PollState,
  task_waker: Option<task::Waker>,
  inspector_ptr: Option<NonNull<JsRuntimeInspector>>,
  isolate_handle: v8::IsolateHandle,
}

// SAFETY: unsafe trait must have unsafe implementation
unsafe impl Send for InspectorWakerInner {}

struct InspectorWaker(Mutex<InspectorWakerInner>);

impl InspectorWaker {
  fn new(isolate_handle: v8::IsolateHandle) -> Arc<Self> {
    let inner = InspectorWakerInner {
      poll_state: PollState::Idle,
      task_waker: None,
      inspector_ptr: None,
      isolate_handle,
    };
    Arc::new(Self(Mutex::new(inner)))
  }

  fn update<F, R>(&self, update_fn: F) -> R
  where
    F: FnOnce(&mut InspectorWakerInner) -> R,
  {
    let mut g = self.0.lock();
    update_fn(&mut g)
  }
}

extern "C" fn handle_interrupt(_isolate: &mut v8::Isolate, arg: *mut c_void) {
  // SAFETY: `InspectorWaker` is owned by `JsRuntimeInspector`, so the
  // pointer to the latter is valid as long as waker is alive.
  let inspector = unsafe { &*(arg as *mut JsRuntimeInspector) };
  inspector.poll_sessions_sync();
}

impl task::ArcWake for InspectorWaker {
  fn wake_by_ref(arc_self: &Arc<Self>) {
    arc_self.update(|w| {
      // Determine whether, given the current poll state, waking up is possible
      // and necessary. If it is, change the poll state to `Woken`.
      match w.poll_state {
        PollState::Idle | PollState::Polling => w.poll_state = PollState::Woken,
        PollState::Woken => {} // Even if already woken, schedule an interrupt.
        PollState::Dropped => return, // Don't do anything.
        PollState::SyncPolling => panic!("wake() called while sync polling"),
      };

      // Wake the task, if any, that has polled the Inspector future last.
      if let Some(waker) = w.task_waker.take() {
        waker.wake()
      }

      // Request an interrupt from the isolate, if the isolate is currently
      // running and there isn't already an interrupt request in flight.
      if let Some(arg) = w
        .inspector_ptr
        .take()
        .map(|ptr| ptr.cast::<c_void>().as_ptr())
      {
        w.isolate_handle.request_interrupt(handle_interrupt, arg);
      }
    });
  }
}

/// An inspector session that proxies messages to concrete "transport layer",
/// eg. Websocket or another set of channels.
struct InspectorSession {
  v8_channel: v8::inspector::ChannelBase,
  v8_session: v8::UniqueRef<v8::inspector::V8InspectorSession>,
  proxy: InspectorSessionProxy,
  // Describes if session should keep event loop alive, eg. a local REPL
  // session should keep event loop alive, but a Websocket session shouldn't.
  blocking: bool,
}

impl InspectorSession {
  const CONTEXT_GROUP_ID: i32 = 1;

  pub fn new(
    v8_inspector_rc: Rc<RefCell<v8::UniquePtr<v8::inspector::V8Inspector>>>,
    session_proxy: InspectorSessionProxy,
    blocking: bool,
  ) -> Box<Self> {
    new_box_with(move |self_ptr| {
      // TODO(bartlomieju): channel should probably be a separate struct
      let v8_channel = v8::inspector::ChannelBase::new::<Self>();
      let mut v8_inspector = v8_inspector_rc.borrow_mut();
      let v8_inspector_ptr = v8_inspector.as_mut().unwrap();
      // TODO(piscisaureus): safety comment
      #[allow(clippy::undocumented_unsafe_blocks)]
      let v8_session = v8_inspector_ptr.connect(
        Self::CONTEXT_GROUP_ID,
        // Todo(piscisaureus): V8Inspector::connect() should require that
        // the 'v8_channel' argument cannot move.
        unsafe { &mut *self_ptr },
        v8::inspector::StringView::empty(),
        v8::inspector::V8InspectorClientTrustLevel::FullyTrusted,
      );

      Self {
        v8_channel,
        v8_session,
        proxy: session_proxy,
        blocking,
      }
    })
  }

  // Dispatch message to V8 session
  fn dispatch_message(
    v8_session_ptr: *mut v8::inspector::V8InspectorSession,
    msg: String,
  ) {
    let msg = v8::inspector::StringView::from(msg.as_bytes());
    // SAFETY: `InspectorSession` is the only owner of `v8_session_ptr`, so
    // the pointer is valid for as long the struct.
    unsafe {
      (*v8_session_ptr).dispatch_protocol_message(msg);
    };
  }

  fn send_message(
    &self,
    msg_kind: InspectorMsgKind,
    msg: v8::UniquePtr<v8::inspector::StringBuffer>,
  ) {
    let msg = msg.unwrap().string().to_string();
    let _ = self.proxy.tx.unbounded_send(InspectorMsg {
      kind: msg_kind,
      content: msg,
    });
  }

  pub fn break_on_next_statement(&mut self) {
    let reason = v8::inspector::StringView::from(&b"debugCommand"[..]);
    let detail = v8::inspector::StringView::empty();
    // TODO(bartlomieju): use raw `*mut V8InspectorSession` pointer, as this
    // reference may become aliased.
    (*self.v8_session).schedule_pause_on_next_statement(reason, detail);
  }
}

impl v8::inspector::ChannelImpl for InspectorSession {
  fn base(&self) -> &v8::inspector::ChannelBase {
    &self.v8_channel
  }

  unsafe fn base_ptr(this: *const Self) -> *const v8::inspector::ChannelBase
  where
    Self: Sized,
  {
    // SAFETY: this pointer is valid for the whole lifetime of inspector
    unsafe { std::ptr::addr_of!((*this).v8_channel) }
  }

  fn base_mut(&mut self) -> &mut v8::inspector::ChannelBase {
    &mut self.v8_channel
  }

  fn send_response(
    &mut self,
    call_id: i32,
    message: v8::UniquePtr<v8::inspector::StringBuffer>,
  ) {
    self.send_message(InspectorMsgKind::Message(call_id), message);
  }

  fn send_notification(
    &mut self,
    message: v8::UniquePtr<v8::inspector::StringBuffer>,
  ) {
    self.send_message(InspectorMsgKind::Notification, message);
  }

  fn flush_protocol_notifications(&mut self) {}
}

impl Stream for InspectorSession {
  type Item = (*mut v8::inspector::V8InspectorSession, String);

  fn poll_next(
    self: Pin<&mut Self>,
    cx: &mut Context,
  ) -> Poll<Option<Self::Item>> {
    let inner = self.get_mut();
    if let Poll::Ready(maybe_msg) = inner.proxy.rx.poll_next_unpin(cx) {
      if let Some(msg) = maybe_msg {
        return Poll::Ready(Some((&mut *inner.v8_session, msg)));
      } else {
        return Poll::Ready(None);
      }
    }

    Poll::Pending
  }
}

/// A local inspector session that can be used to send and receive protocol messages directly on
/// the same thread as an isolate.
pub struct LocalInspectorSession {
  v8_session_tx: UnboundedSender<String>,
  v8_session_rx: UnboundedReceiver<InspectorMsg>,
  response_tx_map: HashMap<i32, oneshot::Sender<serde_json::Value>>,
  next_message_id: i32,
  notification_tx: UnboundedSender<Value>,
  notification_rx: Option<UnboundedReceiver<Value>>,
}

impl LocalInspectorSession {
  pub fn new(
    v8_session_tx: UnboundedSender<String>,
    v8_session_rx: UnboundedReceiver<InspectorMsg>,
  ) -> Self {
    let response_tx_map = HashMap::new();
    let next_message_id = 0;

    let (notification_tx, notification_rx) = mpsc::unbounded::<Value>();

    Self {
      v8_session_tx,
      v8_session_rx,
      response_tx_map,
      next_message_id,
      notification_tx,
      notification_rx: Some(notification_rx),
    }
  }

  pub fn take_notification_rx(&mut self) -> UnboundedReceiver<Value> {
    self.notification_rx.take().unwrap()
  }

  pub async fn post_message<T: serde::Serialize>(
    &mut self,
    method: &str,
    params: Option<T>,
  ) -> Result<serde_json::Value, Error> {
    let id = self.next_message_id;
    self.next_message_id += 1;

    let (response_tx, mut response_rx) =
      oneshot::channel::<serde_json::Value>();
    self.response_tx_map.insert(id, response_tx);

    let message = json!({
        "id": id,
        "method": method,
        "params": params,
    });

    let stringified_msg = serde_json::to_string(&message).unwrap();
    self.v8_session_tx.unbounded_send(stringified_msg).unwrap();

    loop {
      let receive_fut = self.receive_from_v8_session().boxed_local();
      match select(receive_fut, &mut response_rx).await {
        Either::Left(_) => continue,
        Either::Right((result, _)) => {
          let response = result?;
          if let Some(error) = response.get("error") {
            return Err(generic_error(error.to_string()));
          }

          let result = response.get("result").unwrap().clone();
          return Ok(result);
        }
      }
    }
  }

  async fn receive_from_v8_session(&mut self) {
    let inspector_msg = self.v8_session_rx.next().await.unwrap();
    if let InspectorMsgKind::Message(msg_id) = inspector_msg.kind {
      let message: serde_json::Value =
        match serde_json::from_str(&inspector_msg.content) {
          Ok(v) => v,
          Err(error) => match error.classify() {
            serde_json::error::Category::Syntax => json!({
              "id": msg_id,
              "result": {
                "result": {
                  "type": "error",
                  "description": "Unterminated string literal",
                  "value": "Unterminated string literal",
                },
                "exceptionDetails": {
                  "exceptionId": 0,
                  "text": "Unterminated string literal",
                  "lineNumber": 0,
                  "columnNumber": 0
                },
              },
            }),
            _ => panic!("Could not parse inspector message"),
          },
        };

      self
        .response_tx_map
        .remove(&msg_id)
        .unwrap()
        .send(message)
        .unwrap();
    } else {
      let message = serde_json::from_str(&inspector_msg.content).unwrap();
      // Ignore if the receiver has been dropped.
      let _ = self.notification_tx.unbounded_send(message);
    }
  }
}

fn new_box_with<T>(new_fn: impl FnOnce(*mut T) -> T) -> Box<T> {
  let b = Box::new(MaybeUninit::<T>::uninit());
  let p = Box::into_raw(b) as *mut T;
  // SAFETY: memory layout for `T` is ensured on first line of this function
  unsafe {
    ptr::write(p, new_fn(p));
    Box::from_raw(p)
  }
}
