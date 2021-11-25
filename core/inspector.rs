// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

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
use crate::futures::future::Future;
use crate::futures::prelude::*;
use crate::futures::stream::FuturesUnordered;
use crate::futures::stream::StreamExt;
use crate::futures::task;
use crate::futures::task::Context;
use crate::futures::task::Poll;
use crate::serde_json;
use anyhow::Error;
use log::debug;
use parking_lot::Mutex;
use serde::Deserialize;
use serde::Serialize;
use serde_json::value::RawValue;
use std::cell::BorrowMutError;
use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::c_void;
use std::iter::Peekable;
use std::mem::replace;
use std::mem::take;
use std::mem::MaybeUninit;
use std::pin::Pin;
use std::ptr;
use std::ptr::NonNull;
use std::rc::Rc;
use std::str::Chars;
use std::sync::Arc;
use std::thread;

/// If first argument is `None` then it's a notification, otherwise
/// it's a message.
pub type SessionProxySender = UnboundedSender<(Option<i32>, String)>;
pub type SessionProxyReceiver = UnboundedReceiver<Vec<u8>>;

/// Encapsulates an UnboundedSender/UnboundedReceiver pair that together form
/// a duplex channel for sending/receiving messages in V8 session.
pub struct InspectorSessionProxy {
  pub tx: SessionProxySender,
  pub rx: SessionProxyReceiver,
}

#[derive(Clone, Copy)]
enum PollState {
  Idle,
  Woken,
  Polling,
  Parked,
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
    take(&mut *self.sessions.borrow_mut());

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

  fn base_mut(&mut self) -> &mut v8::inspector::V8InspectorClientBase {
    &mut self.v8_inspector_client
  }

  fn run_message_loop_on_pause(&mut self, context_group_id: i32) {
    assert_eq!(context_group_id, JsRuntimeInspector::CONTEXT_GROUP_ID);
    self.flags.borrow_mut().on_pause = true;
    let _ = self.poll_sessions(None);
  }

  fn quit_message_loop_on_pause(&mut self) {
    self.flags.borrow_mut().on_pause = false;
  }

  fn run_if_waiting_for_debugger(&mut self, context_group_id: i32) {
    assert_eq!(context_group_id, JsRuntimeInspector::CONTEXT_GROUP_ID);
    self.flags.borrow_mut().session_handshake_done = true;
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
    self.poll_sessions(Some(cx)).unwrap()
  }
}

impl JsRuntimeInspector {
  /// Currently Deno supports only a single context in `JsRuntime`
  /// and thus it's id is provided as an associated contant.
  const CONTEXT_GROUP_ID: i32 = 1;

  pub fn new(
    isolate: &mut v8::OwnedIsolate,
    context: v8::Global<v8::Context>,
  ) -> Box<Self> {
    let scope = &mut v8::HandleScope::new(isolate);

    let (new_session_tx, new_session_rx) =
      mpsc::unbounded::<InspectorSessionProxy>();

    let v8_inspector_client =
      v8::inspector::V8InspectorClientBase::new::<Self>();

    let flags = InspectorFlags::new();
    let waker = InspectorWaker::new(scope.thread_safe_handle());

    // Create JsRuntimeInspector instance.
    let mut self_ = Box::new(Self {
      v8_inspector_client,
      v8_inspector: Default::default(),
      sessions: Default::default(),
      new_session_tx,
      flags,
      waker,
      deregister_tx: None,
    });
    self_.v8_inspector = Rc::new(RefCell::new(
      v8::inspector::V8Inspector::create(scope, &mut *self_).into(),
    ));
    self_.sessions =
      SessionContainer::new(self_.v8_inspector.clone(), new_session_rx);

    // Tell the inspector about the global context.
    let context = v8::Local::new(scope, context);
    let context_name = v8::inspector::StringView::from(&b"global context"[..]);
    self_
      .v8_inspector
      .borrow_mut()
      .as_mut()
      .unwrap()
      .context_created(context, Self::CONTEXT_GROUP_ID, context_name);

    // Poll the session handler so we will get notified whenever there is
    // new_incoming debugger activity.
    let _ = self_.poll_sessions(None).unwrap();

    self_
  }

  pub fn has_active_sessions(&self) -> bool {
    let sessions = self.sessions.borrow();
    !sessions.established.is_empty() || sessions.handshake.is_some()
  }

  fn poll_sessions(
    &self,
    mut invoker_cx: Option<&mut Context>,
  ) -> Result<Poll<()>, BorrowMutError> {
    // The futures this function uses do not have re-entrant poll() functions.
    // However it is can happpen that poll_sessions() gets re-entered, e.g.
    // when an interrupt request is honored while the inspector future is polled
    // by the task executor. We let the caller know by returning some error.
    let mut sessions = self.sessions.try_borrow_mut()?;

    self.waker.update(|w| {
      match w.poll_state {
        PollState::Idle | PollState::Woken => w.poll_state = PollState::Polling,
        _ => unreachable!(),
      };
    });

    // Create a new Context object that will make downstream futures
    // use the InspectorWaker when they are ready to be polled again.
    let waker_ref = task::waker_ref(&self.waker);
    let cx = &mut Context::from_waker(&waker_ref);

    loop {
      loop {
        // Do one "handshake" with a newly connected session at a time.
        if let Some(session) = &mut sessions.handshake {
          let poll_result = session.poll_unpin(cx);
          let handshake_done =
            replace(&mut self.flags.borrow_mut().session_handshake_done, false);
          match poll_result {
            Poll::Pending if handshake_done => {
              let session = sessions.handshake.take().unwrap();
              sessions.established.push(session);
              take(&mut self.flags.borrow_mut().waiting_for_session);
            }
            Poll::Ready(_) => sessions.handshake = None,
            Poll::Pending => break,
          };
        }

        // Accept new connections.
        match sessions.new_incoming.poll_next_unpin(cx) {
          Poll::Ready(Some(session)) => {
            let prev = sessions.handshake.replace(session);
            assert!(prev.is_none());
            continue;
          }
          Poll::Ready(None) => {}
          Poll::Pending => {}
        }

        // Poll established sessions.
        match sessions.established.poll_next_unpin(cx) {
          Poll::Ready(Some(_)) => continue,
          Poll::Ready(None) => break,
          Poll::Pending => break,
        };
      }

      let should_block = sessions.handshake.is_some()
        || self.flags.borrow().on_pause
        || self.flags.borrow().waiting_for_session;

      let new_state = self.waker.update(|w| {
        match w.poll_state {
          PollState::Woken => {
            // The inspector was woken while the session handler was being
            // polled, so we poll it another time.
            w.poll_state = PollState::Polling;
          }
          PollState::Polling if !should_block => {
            // The session handler doesn't need to be polled any longer, and
            // there's no reason to block (execution is not paused), so this
            // function is about to return.
            w.poll_state = PollState::Idle;
            // Register the task waker that can be used to wake the parent
            // task that will poll the inspector future.
            if let Some(cx) = invoker_cx.take() {
              w.task_waker.replace(cx.waker().clone());
            }
            // Register the address of the inspector, which allows the waker
            // to request an interrupt from the isolate.
            w.inspector_ptr = NonNull::new(self as *const _ as *mut Self);
          }
          PollState::Polling if should_block => {
            // Isolate execution has been paused but there are no more
            // events to process, so this thread will be parked. Therefore,
            // store the current thread handle in the waker so it knows
            // which thread to unpark when new events arrive.
            w.poll_state = PollState::Parked;
            w.parked_thread.replace(thread::current());
          }
          _ => unreachable!(),
        };
        w.poll_state
      });
      match new_state {
        PollState::Idle => break Ok(Poll::Pending), // Yield to task.
        PollState::Polling => {} // Poll the session handler again.
        PollState::Parked => thread::park(), // Park the thread.
        _ => unreachable!(),
      };
    }
  }

  /// This function blocks the thread until at least one inspector client has
  /// established a websocket connection and successfully completed the
  /// handshake. After that, it instructs V8 to pause at the next statement.
  pub fn wait_for_session_and_break_on_next_statement(&mut self) {
    loop {
      match self.sessions.get_mut().established.iter_mut().next() {
        Some(session) => break session.break_on_next_statement(),
        None => {
          self.flags.get_mut().waiting_for_session = true;
          let _ = self.poll_sessions(None).unwrap();
        }
      };
    }
  }

  /// Obtain a sender for proxy channels.
  ///
  /// After a proxy is sent inspector will wait for a "handshake".
  /// Frontend must send "Runtime.runIfWaitingForDebugger" message to
  /// complete the handshake.
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
    // sessions, so it doesn't need to go through the session sender and handshake
    // phase.
    let inspector_session =
      InspectorSession::new(self.v8_inspector.clone(), proxy);
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
  session_handshake_done: bool,
  on_pause: bool,
}

impl InspectorFlags {
  fn new() -> RefCell<Self> {
    let self_ = Self::default();
    RefCell::new(self_)
  }
}

/// A helper structure that helps coordinate sessions during different
/// parts of their lifecycle.
struct SessionContainer {
  new_incoming: Pin<Box<dyn Stream<Item = Box<InspectorSession>> + 'static>>,
  handshake: Option<Box<InspectorSession>>,
  established: FuturesUnordered<Box<InspectorSession>>,
}

impl SessionContainer {
  fn new(
    v8_inspector: Rc<RefCell<v8::UniquePtr<v8::inspector::V8Inspector>>>,
    new_session_rx: UnboundedReceiver<InspectorSessionProxy>,
  ) -> RefCell<Self> {
    let new_incoming = new_session_rx
      .map(move |session_proxy| {
        InspectorSession::new(v8_inspector.clone(), session_proxy)
      })
      .boxed_local();
    let self_ = Self {
      new_incoming,
      ..Default::default()
    };
    RefCell::new(self_)
  }
}

impl Default for SessionContainer {
  fn default() -> Self {
    Self {
      new_incoming: stream::empty().boxed_local(),
      handshake: None,
      established: FuturesUnordered::new(),
    }
  }
}

struct InspectorWakerInner {
  poll_state: PollState,
  task_waker: Option<task::Waker>,
  parked_thread: Option<thread::Thread>,
  inspector_ptr: Option<NonNull<JsRuntimeInspector>>,
  isolate_handle: v8::IsolateHandle,
}

unsafe impl Send for InspectorWakerInner {}

struct InspectorWaker(Mutex<InspectorWakerInner>);

impl InspectorWaker {
  fn new(isolate_handle: v8::IsolateHandle) -> Arc<Self> {
    let inner = InspectorWakerInner {
      poll_state: PollState::Idle,
      task_waker: None,
      parked_thread: None,
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

impl task::ArcWake for InspectorWaker {
  fn wake_by_ref(arc_self: &Arc<Self>) {
    arc_self.update(|w| {
      match w.poll_state {
        PollState::Idle => {
          // Wake the task, if any, that has polled the Inspector future last.
          if let Some(waker) = w.task_waker.take() {
            waker.wake()
          }
          // Request an interrupt from the isolate if it's running and there's
          // not unhandled interrupt request in flight.
          if let Some(arg) = w
            .inspector_ptr
            .take()
            .map(|ptr| ptr.as_ptr() as *mut c_void)
          {
            w.isolate_handle.request_interrupt(handle_interrupt, arg);
          }
          extern "C" fn handle_interrupt(
            _isolate: &mut v8::Isolate,
            arg: *mut c_void,
          ) {
            let inspector = unsafe { &*(arg as *mut JsRuntimeInspector) };
            let _ = inspector.poll_sessions(None);
          }
        }
        PollState::Parked => {
          // Unpark the isolate thread.
          let parked_thread = w.parked_thread.take().unwrap();
          assert_ne!(parked_thread.id(), thread::current().id());
          parked_thread.unpark();
        }
        _ => {}
      };
      w.poll_state = PollState::Woken;
    });
  }
}

/// An inspector session that proxies messages to concrete "transport layer",
/// eg. Websocket or another set of channels.
struct InspectorSession {
  v8_channel: v8::inspector::ChannelBase,
  v8_session: Rc<RefCell<v8::UniqueRef<v8::inspector::V8InspectorSession>>>,
  proxy: InspectorSessionProxy,
}

impl InspectorSession {
  const CONTEXT_GROUP_ID: i32 = 1;

  pub fn new(
    v8_inspector_rc: Rc<RefCell<v8::UniquePtr<v8::inspector::V8Inspector>>>,
    session_proxy: InspectorSessionProxy,
  ) -> Box<Self> {
    new_box_with(move |self_ptr| {
      let v8_channel = v8::inspector::ChannelBase::new::<Self>();
      let mut v8_inspector = v8_inspector_rc.borrow_mut();
      let v8_inspector_ptr = v8_inspector.as_mut().unwrap();
      let v8_session = Rc::new(RefCell::new(v8_inspector_ptr.connect(
        Self::CONTEXT_GROUP_ID,
        // Todo(piscisaureus): V8Inspector::connect() should require that
        // the 'v8_channel' argument cannot move.
        unsafe { &mut *self_ptr },
        v8::inspector::StringView::empty(),
      )));

      Self {
        v8_channel,
        v8_session,
        proxy: session_proxy,
      }
    })
  }

  // Dispatch message to V8 session
  #[allow(unused)]
  fn dispatch_message(&mut self, msg: Vec<u8>) {
    let msg = v8::inspector::StringView::from(msg.as_slice());
    let mut v8_session = self.v8_session.borrow_mut();
    let v8_session_ptr = v8_session.as_mut();
    v8_session_ptr.dispatch_protocol_message(msg);
  }

  fn send_message(
    &self,
    maybe_call_id: Option<i32>,
    msg: v8::UniquePtr<v8::inspector::StringBuffer>,
  ) {
    let msg = msg.unwrap().string().to_string();
    let _ = self.proxy.tx.unbounded_send((maybe_call_id, msg));
  }

  pub fn break_on_next_statement(&mut self) {
    let reason = v8::inspector::StringView::from(&b"debugCommand"[..]);
    let detail = v8::inspector::StringView::empty();
    self
      .v8_session
      .borrow_mut()
      .as_mut()
      .schedule_pause_on_next_statement(reason, detail);
  }
}

impl v8::inspector::ChannelImpl for InspectorSession {
  fn base(&self) -> &v8::inspector::ChannelBase {
    &self.v8_channel
  }

  fn base_mut(&mut self) -> &mut v8::inspector::ChannelBase {
    &mut self.v8_channel
  }

  fn send_response(
    &mut self,
    call_id: i32,
    message: v8::UniquePtr<v8::inspector::StringBuffer>,
  ) {
    self.send_message(Some(call_id), message);
  }

  fn send_notification(
    &mut self,
    message: v8::UniquePtr<v8::inspector::StringBuffer>,
  ) {
    self.send_message(None, message);
  }

  fn flush_protocol_notifications(&mut self) {}
}

/// This is a "pump" future takes care of receiving messages and dispatching
/// them to the inspector. It resolves when receiver closes.
impl Future for InspectorSession {
  type Output = ();
  fn poll(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
    while let Poll::Ready(maybe_msg) = self.proxy.rx.poll_next_unpin(cx) {
      if let Some(msg) = maybe_msg {
        let msg = v8::inspector::StringView::from(msg.as_slice());
        let mut v8_session = self.v8_session.borrow_mut();
        let v8_session_ptr = v8_session.as_mut();
        v8_session_ptr.dispatch_protocol_message(msg);
      } else {
        return Poll::Ready(());
      }
    }

    Poll::Pending
  }
}

#[derive(Debug, Deserialize)]
struct V8InspectorMessage {
  id: i32,
  result: Option<Box<serde_json::value::RawValue>>,
  error: Option<V8Error>,
}

#[derive(Debug, Deserialize)]
struct V8Error {
  code: i32,
  message: LossyString,
}

#[derive(Deserialize)]
pub struct V8InspectorNotification {
  pub method: String,
  pub params: Box<RawValue>,
}

/// A local inspector session that can be used to send and receive protocol messages directly on
/// the same thread as an isolate.
pub struct LocalInspectorSession {
  v8_session_tx: UnboundedSender<Vec<u8>>,
  v8_session_rx: UnboundedReceiver<(Option<i32>, String)>,
  response_tx_map: HashMap<i32, oneshot::Sender<V8InspectorMessage>>,
  next_message_id: i32,
  notification_queue: Vec<V8InspectorNotification>,
}

impl LocalInspectorSession {
  pub fn new(
    v8_session_tx: UnboundedSender<Vec<u8>>,
    v8_session_rx: UnboundedReceiver<(Option<i32>, String)>,
  ) -> Self {
    let response_tx_map = HashMap::new();
    let next_message_id = 0;

    let notification_queue = Vec::new();

    Self {
      v8_session_tx,
      v8_session_rx,
      response_tx_map,
      next_message_id,
      notification_queue,
    }
  }

  pub fn notifications(&mut self) -> Vec<V8InspectorNotification> {
    self.notification_queue.split_off(0)
  }

  pub async fn post_message<T: Serialize>(
    &mut self,
    method: &'static str,
    params: Option<T>,
  ) -> Result<Box<RawValue>, Error> {
    let id = self.next_message_id;
    self.next_message_id += 1;

    let (response_tx, mut response_rx) =
      oneshot::channel::<V8InspectorMessage>();
    self.response_tx_map.insert(id, response_tx);

    #[derive(Serialize)]
    struct Request<T> {
      id: i32,
      method: &'static str,
      params: Option<T>,
    }

    let message = Request { id, method, params };

    let raw_message = serde_json::to_string(&message).unwrap();
    debug!("Sending message: {}", raw_message);
    self
      .v8_session_tx
      .unbounded_send(raw_message.as_bytes().to_vec())
      .unwrap();

    loop {
      let receive_fut = self.receive_from_v8_session().boxed_local();
      match select(receive_fut, &mut response_rx).await {
        Either::Left(_) => continue,
        Either::Right((result, _)) => {
          let response = result?;
          match (response.result, response.error) {
            (Some(result), None) => {
              return Ok(result);
            }
            (None, Some(err)) => {
              let message: String = err.message.into();
              return Err(generic_error(message));
            }
            _ => panic!("Invalid response"),
          }
        }
      }
    }
  }

  async fn receive_from_v8_session(&mut self) {
    let (maybe_call_id, message) = self.v8_session_rx.next().await.unwrap();
    debug!("Recv message: {}", message);
    // If there's no call_id then it's a notification
    if let Some(call_id) = maybe_call_id {
      let message: V8InspectorMessage = match serde_json::from_str(&message) {
        Ok(v) => v,
        Err(e) => {
          panic!("Could not parse inspector message: {}", e)
        }
      };
      self
        .response_tx_map
        .remove(&call_id)
        .unwrap()
        .send(message)
        .unwrap();
    } else {
      let message = serde_json::from_str(&message).unwrap();
      self.notification_queue.push(message);
    }
  }
}

fn new_box_with<T>(new_fn: impl FnOnce(*mut T) -> T) -> Box<T> {
  let b = Box::new(MaybeUninit::<T>::uninit());
  let p = Box::into_raw(b) as *mut T;
  unsafe { ptr::write(p, new_fn(p)) };
  unsafe { Box::from_raw(p) }
}

/// A wrapper type for `String` that has a serde::Deserialize implementation
/// that will deserialize lossily (i.e. using replacement characters for
/// invalid UTF-8 sequences).
pub struct LossyString(String);

static ESCAPE: [bool; 256] = {
  const CT: bool = true; // control character \x00..=\x1F
  const QU: bool = true; // quote \x22
  const BS: bool = true; // backslash \x5C
  const __: bool = false; // allow unescaped
  [
    //   1   2   3   4   5   6   7   8   9   A   B   C   D   E   F
    CT, CT, CT, CT, CT, CT, CT, CT, CT, CT, CT, CT, CT, CT, CT, CT, // 0
    CT, CT, CT, CT, CT, CT, CT, CT, CT, CT, CT, CT, CT, CT, CT, CT, // 1
    __, __, QU, __, __, __, __, __, __, __, __, __, __, __, __, __, // 2
    __, __, __, __, __, __, __, __, __, __, __, __, __, __, __, __, // 3
    __, __, __, __, __, __, __, __, __, __, __, __, __, __, __, __, // 4
    __, __, __, __, __, __, __, __, __, __, __, __, BS, __, __, __, // 5
    __, __, __, __, __, __, __, __, __, __, __, __, __, __, __, __, // 6
    __, __, __, __, __, __, __, __, __, __, __, __, __, __, __, __, // 7
    __, __, __, __, __, __, __, __, __, __, __, __, __, __, __, __, // 8
    __, __, __, __, __, __, __, __, __, __, __, __, __, __, __, __, // 9
    __, __, __, __, __, __, __, __, __, __, __, __, __, __, __, __, // A
    __, __, __, __, __, __, __, __, __, __, __, __, __, __, __, __, // B
    __, __, __, __, __, __, __, __, __, __, __, __, __, __, __, __, // C
    __, __, __, __, __, __, __, __, __, __, __, __, __, __, __, __, // D
    __, __, __, __, __, __, __, __, __, __, __, __, __, __, __, __, // E
    __, __, __, __, __, __, __, __, __, __, __, __, __, __, __, __, // F
  ]
};

impl<'de> serde::Deserialize<'de> for LossyString {
  fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
  where
    D: serde::Deserializer<'de>,
  {
    macro_rules! tri {
      ($opt:expr) => {
        match $opt {
          Some(v) => v,
          None => {
            return Err(serde::de::Error::custom("Unexpected end of string"))
          }
        }
      };
    }

    let val = Box::<RawValue>::deserialize(deserializer)?;
    let mut chars = val.get().chars().peekable();

    if tri!(chars.next()) != '"' {
      return Err(serde::de::Error::custom("Expected string"));
    }

    let mut str = String::new();

    loop {
      let ch = tri!(chars.next());
      if !ESCAPE[ch as usize] {
        str.push(ch);
        continue;
      }
      match ch {
        '"' => {
          break;
        }
        '\\' => match parse_escape(&mut chars, &mut str) {
          Ok(_) => {}
          Err(err) => return Err(serde::de::Error::custom(err)),
        },
        _ => {
          return Err(serde::de::Error::custom("Constrol character in string"));
        }
      }
    }

    if chars.next() != None {
      return Err(serde::de::Error::custom("Trailing characters in string"));
    }

    Ok(LossyString(str))
  }
}

static HEX: [u8; 256] = {
  const __: u8 = 255; // not a hex digit
  [
    //   1   2   3   4   5   6   7   8   9   A   B   C   D   E   F
    __, __, __, __, __, __, __, __, __, __, __, __, __, __, __, __, // 0
    __, __, __, __, __, __, __, __, __, __, __, __, __, __, __, __, // 1
    __, __, __, __, __, __, __, __, __, __, __, __, __, __, __, __, // 2
    00, 01, 02, 03, 04, 05, 06, 07, 08, 09, __, __, __, __, __, __, // 3
    __, 10, 11, 12, 13, 14, 15, __, __, __, __, __, __, __, __, __, // 4
    __, __, __, __, __, __, __, __, __, __, __, __, __, __, __, __, // 5
    __, 10, 11, 12, 13, 14, 15, __, __, __, __, __, __, __, __, __, // 6
    __, __, __, __, __, __, __, __, __, __, __, __, __, __, __, __, // 7
    __, __, __, __, __, __, __, __, __, __, __, __, __, __, __, __, // 8
    __, __, __, __, __, __, __, __, __, __, __, __, __, __, __, __, // 9
    __, __, __, __, __, __, __, __, __, __, __, __, __, __, __, __, // A
    __, __, __, __, __, __, __, __, __, __, __, __, __, __, __, __, // B
    __, __, __, __, __, __, __, __, __, __, __, __, __, __, __, __, // C
    __, __, __, __, __, __, __, __, __, __, __, __, __, __, __, __, // D
    __, __, __, __, __, __, __, __, __, __, __, __, __, __, __, __, // E
    __, __, __, __, __, __, __, __, __, __, __, __, __, __, __, __, // F
  ]
};

fn decode_hex_val(val: char) -> Option<u16> {
  let n = HEX[val as usize] as u16;
  if n == 255 {
    None
  } else {
    Some(n)
  }
}

fn decode_hex_escape(
  chars: &mut Peekable<Chars<'_>>,
) -> Result<u16, &'static str> {
  let mut n = 0;
  for _ in 0..4 {
    let ch = match chars.next() {
      Some(ch) => ch,
      None => return Err("Unexpected end of string"),
    };
    match decode_hex_val(ch) {
      None => return Err("Invalid hex escape"),
      Some(val) => {
        n = (n << 4) + val;
      }
    }
  }
  Ok(n)
}

fn parse_escape(
  chars: &mut Peekable<Chars<'_>>,
  str: &mut String,
) -> Result<(), &'static str> {
  macro_rules! tri {
    ($opt:expr) => {
      match $opt {
        Some(v) => v,
        None => return Err("Unexpected end of string"),
      }
    };
  }

  let ch = tri!(chars.next());

  match ch {
    '"' => str.push('"'),
    '\\' => str.push('\\'),
    '/' => str.push('/'),
    'b' => str.push('\x08'),
    'f' => str.push('\x0c'),
    'n' => str.push('\n'),
    'r' => str.push('\r'),
    't' => str.push('\t'),
    'u' => {
      let c = match decode_hex_escape(chars)? {
        0xDC00..=0xDFFF => {
          str.push('\u{FFFD}');
          return Ok(());
        }

        // Non-BMP characters are encoded as a sequence of two hex
        // escapes, representing UTF-16 surrogates. If deserializing a
        // utf-8 string the surrogates are required to be paired,
        // whereas deserializing a byte string accepts lone surrogates.
        n1 @ 0xD800..=0xDBFF => {
          if *tri!(chars.peek()) == '\\' {
            chars.next();
          } else {
            str.push('\u{FFFD}');
            return Ok(());
          }

          if *tri!(chars.peek()) == 'u' {
            chars.next();
          } else {
            str.push('\u{FFFD}');
            return parse_escape(chars, str);
          }

          let n2 = decode_hex_escape(chars)?;

          if n2 < 0xDC00 || n2 > 0xDFFF {
            str.push('\u{FFFD}');
          }

          let n =
            (((n1 - 0xD800) as u32) << 10 | (n2 - 0xDC00) as u32) + 0x1_0000;

          match char::from_u32(n) {
            Some(c) => c,
            None => {
              str.push('\u{FFFD}');
              return Ok(());
            }
          }
        }

        n => match char::from_u32(n as u32) {
          Some(c) => c,
          None => {
            str.push('\u{FFFD}');
            return Ok(());
          }
        },
      };

      str.push(c);
    }
    _ => {
      return Err("Invalid escape sequence");
    }
  }

  Ok(())
}

impl From<LossyString> for String {
  fn from(lossy_string: LossyString) -> Self {
    lossy_string.0
  }
}

impl std::ops::Deref for LossyString {
  type Target = String;
  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl std::fmt::Debug for LossyString {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{}", self.0)
  }
}
