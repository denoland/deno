// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

//! The documentation for the inspector API is sparse, but these are helpful:
//! https://chromedevtools.github.io/devtools-protocol/
//! https://hyperandroid.com/2020/02/12/v8-inspector-from-an-embedder-standpoint/

use core::convert::Infallible as Never; // Alias for the future `!` type.
use deno_core::v8;
use futures::channel::mpsc;
use futures::channel::mpsc::UnboundedReceiver;
use futures::channel::mpsc::UnboundedSender;
use futures::channel::oneshot;
use futures::future::Future;
use futures::prelude::*;
use futures::select;
use futures::stream::FuturesUnordered;
use futures::task;
use futures::task::Context;
use futures::task::Poll;
use std::cell::BorrowMutError;
use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::c_void;
use std::mem::replace;
use std::mem::take;
use std::mem::MaybeUninit;
use std::net::SocketAddr;
use std::ops::Deref;
use std::ops::DerefMut;
use std::pin::Pin;
use std::process;
use std::ptr;
use std::ptr::NonNull;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::Once;
use std::thread;
use uuid::Uuid;
use warp::filters::ws;
use warp::Filter;

struct InspectorServer {
  host: SocketAddr,
  register_inspector_tx: UnboundedSender<InspectorInfo>,
  _thread_handle: thread::JoinHandle<()>,
}

impl InspectorServer {
  /// Registers an Inspector instance with the inspector server. If the server
  /// is not running yet, it'll  be started first.
  pub fn register_inspector(info: InspectorInfo) {
    let self_ = Self::global(&info.host);
    self_.register_inspector_tx.unbounded_send(info).unwrap();
  }

  /// Returns the global InspectorServer instance. If the server is not yet
  /// running, this function starts it.
  fn global(host: &SocketAddr) -> &'static InspectorServer {
    let instance = unsafe {
      static mut INSTANCE: Option<InspectorServer> = None;
      static INIT: Once = Once::new();
      INIT.call_once(|| {
        INSTANCE.replace(Self::new(*host));
      });
      INSTANCE.as_ref().unwrap()
    };
    // We only start a single server, so all inspectors must bind to the same
    // host:port combination.
    assert_eq!(host, &instance.host);
    instance
  }

  fn new(host: SocketAddr) -> Self {
    let (register_inspector_tx, register_inspector_rx) =
      mpsc::unbounded::<InspectorInfo>();
    let thread_handle = thread::spawn(move || {
      crate::tokio_util::run_basic(server(host, register_inspector_rx))
    });
    Self {
      host,
      register_inspector_tx,
      _thread_handle: thread_handle,
    }
  }
}

/// Inspector information that is sent from the isolate thread to the server
/// thread when a new inspector is created.
struct InspectorInfo {
  host: SocketAddr,
  uuid: Uuid,
  thread_name: Option<String>,
  new_websocket_tx: UnboundedSender<WebSocketProxy>,
  canary_rx: oneshot::Receiver<Never>,
}

impl InspectorInfo {
  fn get_json_metadata(&self) -> serde_json::Value {
    json!({
      "description": "deno",
      "devtoolsFrontendUrl": self.get_frontend_url(),
      "faviconUrl": "https://deno.land/favicon.ico",
      "id": self.uuid.to_string(),
      "title": self.get_title(),
      "type": "deno",
      // TODO(ry): "url": "file://",
      "webSocketDebuggerUrl": self.get_websocket_debugger_url(),
    })
  }

  fn get_websocket_debugger_url(&self) -> String {
    format!("ws://{}/ws/{}", &self.host, &self.uuid)
  }

  fn get_frontend_url(&self) -> String {
    format!(
      "chrome-devtools://devtools/bundled/inspector.html?v8only=true&ws={}/ws/{}",
      &self.host, &self.uuid
    )
  }

  fn get_title(&self) -> String {
    format!(
      "[{}] deno{}",
      process::id(),
      self
        .thread_name
        .as_ref()
        .map(|n| format!(" - {}", n))
        .unwrap_or_default()
    )
  }
}

async fn server(
  host: SocketAddr,
  register_inspector_rx: UnboundedReceiver<InspectorInfo>,
) {
  // TODO: `inspector_map` in an Rc<RefCell<T>> instead. This is currently not
  // possible because warp requires all filters to implement Send, which should
  // not be necessary because we are using a single-threaded runtime.
  let inspector_map = HashMap::<Uuid, InspectorInfo>::new();
  let inspector_map = Arc::new(Mutex::new(inspector_map));

  let inspector_map_ = inspector_map.clone();
  let mut register_inspector_handler = register_inspector_rx
    .map(|info| {
      eprintln!(
        "Debugger listening on {}",
        info.get_websocket_debugger_url()
      );
      let mut g = inspector_map_.lock().unwrap();
      if g.insert(info.uuid, info).is_some() {
        panic!("Inspector UUID already in map");
      }
    })
    .collect::<()>();

  let inspector_map_ = inspector_map_.clone();
  let mut deregister_inspector_handler = future::poll_fn(|cx| {
    let mut g = inspector_map_.lock().unwrap();
    g.retain(|_, info| info.canary_rx.poll_unpin(cx) == Poll::Pending);
    Poll::<Never>::Pending
  })
  .fuse();

  let inspector_map_ = inspector_map.clone();
  let websocket_route = warp::path("ws")
    .and(warp::path::param())
    .and(warp::ws())
    .and_then(move |uuid: String, ws: warp::ws::Ws| {
      future::ready(
        Uuid::parse_str(&uuid)
          .ok()
          .and_then(|uuid| {
            let g = inspector_map_.lock().unwrap();
            g.get(&uuid).map(|info| info.new_websocket_tx.clone()).map(
              |new_websocket_tx| {
                ws.on_upgrade(move |websocket| async move {
                  let (proxy, pump) = create_websocket_proxy(websocket);
                  let _ = new_websocket_tx.unbounded_send(proxy);
                  pump.await;
                })
              },
            )
          })
          .ok_or_else(warp::reject::not_found),
      )
    });

  let json_version_route = warp::path!("json" / "version").map(|| {
    warp::reply::json(&json!({
      "Browser": format!("Deno/{}", crate::version::DENO),
      "Protocol-Version": "1.3",
      "V8-Version": crate::version::v8(),
    }))
  });

  let inspector_map_ = inspector_map.clone();
  let json_list_route = warp::path("json").map(move || {
    let g = inspector_map_.lock().unwrap();
    let json_values = g
      .values()
      .map(|info| info.get_json_metadata())
      .collect::<Vec<_>>();
    warp::reply::json(&json!(json_values))
  });

  let server_routes =
    websocket_route.or(json_version_route).or(json_list_route);
  let mut server_handler = warp::serve(server_routes)
    .try_bind_ephemeral(host)
    .map(|(_, fut)| fut)
    .unwrap_or_else(|err| {
      eprintln!("Cannot start inspector server: {}.", err);
      process::exit(1);
    })
    .fuse();

  select! {
    _ = register_inspector_handler => (),
    _ = deregister_inspector_handler => unreachable!(),
    _ = server_handler => unreachable!(),
  }
}

type WebSocketProxySender = UnboundedSender<ws::Message>;
type WebSocketProxyReceiver =
  UnboundedReceiver<Result<ws::Message, warp::Error>>;

/// Encapsulates an UnboundedSender/UnboundedReceiver pair that together form
/// a duplex channel for sending/receiving websocket messages.
struct WebSocketProxy {
  tx: WebSocketProxySender,
  rx: WebSocketProxyReceiver,
}

impl WebSocketProxy {
  pub fn split(self) -> (WebSocketProxySender, WebSocketProxyReceiver) {
    (self.tx, self.rx)
  }
}

/// Creates a future that proxies messages sent and received on a warp WebSocket
/// to a UnboundedSender/UnboundedReceiver pair. We need this to sidestep
/// Tokio's task budget, which causes issues when DenoInspector::poll_sessions()
/// needs to block the thread because JavaScript execution is paused.
///
/// This works because UnboundedSender/UnboundedReceiver are implemented in the
/// 'futures' crate, therefore they can't participate in Tokio's cooperative
/// task yielding.
///
/// A tuple is returned, where the first element is a duplex channel that can
/// be used to send/receive messages on the websocket, and the second element
/// is a future that does the forwarding.
fn create_websocket_proxy(
  websocket: ws::WebSocket,
) -> (WebSocketProxy, impl Future<Output = ()> + Send) {
  // The 'outbound' channel carries messages sent to the websocket.
  let (outbound_tx, outbound_rx) = mpsc::unbounded();

  // The 'inbound' channel carries messages received from the websocket.
  let (inbound_tx, inbound_rx) = mpsc::unbounded();

  let proxy = WebSocketProxy {
    tx: outbound_tx,
    rx: inbound_rx,
  };

  // The pump future takes care of forwarding messages between the websocket
  // and channels. It resolves to () when either side disconnects, ignoring any
  // errors.
  let pump = async move {
    let (websocket_tx, websocket_rx) = websocket.split();

    let outbound_pump =
      outbound_rx.map(Ok).forward(websocket_tx).map_err(|_| ());

    let inbound_pump = websocket_rx
      .map(|msg| inbound_tx.unbounded_send(msg))
      .map_err(|_| ())
      .try_collect::<()>();

    let _ = future::try_join(outbound_pump, inbound_pump).await;
  };

  (proxy, pump)
}

#[derive(Clone, Copy)]
enum PollState {
  Idle,
  Woken,
  Polling,
  Parked,
  Dropped,
}

pub struct DenoInspector {
  v8_inspector_client: v8::inspector::V8InspectorClientBase,
  v8_inspector: v8::UniqueRef<v8::inspector::V8Inspector>,
  sessions: RefCell<InspectorSessions>,
  flags: RefCell<InspectorFlags>,
  waker: Arc<InspectorWaker>,
  _canary_tx: oneshot::Sender<Never>,
  pub debugger_url: String,
}

impl Deref for DenoInspector {
  type Target = v8::inspector::V8Inspector;
  fn deref(&self) -> &Self::Target {
    &self.v8_inspector
  }
}

impl DerefMut for DenoInspector {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.v8_inspector
  }
}

impl Drop for DenoInspector {
  fn drop(&mut self) {
    // Since the  waker is cloneable, it might outlive the inspector itself.
    // Set the poll state to 'dropped' so it doesn't attempt to request an
    // interrupt from the isolate.
    self.waker.update(|w| w.poll_state = PollState::Dropped);
    // V8 automatically deletes all sessions when an Inspector instance is
    // deleted, however InspectorSession also has a drop handler that cleans
    // up after itself. To avoid a double free, make sure the inspector is
    // dropped last.
    take(&mut *self.sessions.borrow_mut());
  }
}

impl v8::inspector::V8InspectorClientImpl for DenoInspector {
  fn base(&self) -> &v8::inspector::V8InspectorClientBase {
    &self.v8_inspector_client
  }

  fn base_mut(&mut self) -> &mut v8::inspector::V8InspectorClientBase {
    &mut self.v8_inspector_client
  }

  fn run_message_loop_on_pause(&mut self, context_group_id: i32) {
    assert_eq!(context_group_id, DenoInspectorSession::CONTEXT_GROUP_ID);
    self.flags.borrow_mut().on_pause = true;
    let _ = self.poll_sessions(None);
  }

  fn quit_message_loop_on_pause(&mut self) {
    self.flags.borrow_mut().on_pause = false;
  }

  fn run_if_waiting_for_debugger(&mut self, context_group_id: i32) {
    assert_eq!(context_group_id, DenoInspectorSession::CONTEXT_GROUP_ID);
    self.flags.borrow_mut().session_handshake_done = true;
  }
}

/// DenoInspector implements a Future so that it can poll for new incoming
/// connections and messages from the WebSocket server. The Worker that owns
/// this DenoInspector will call our poll function from Worker::poll().
impl Future for DenoInspector {
  type Output = ();
  fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<()> {
    self.poll_sessions(Some(cx)).unwrap()
  }
}

impl DenoInspector {
  const CONTEXT_GROUP_ID: i32 = 1;

  pub fn new(
    isolate: &mut deno_core::CoreIsolate,
    host: SocketAddr,
  ) -> Box<Self> {
    let core_state_rc = deno_core::CoreIsolate::state(isolate);
    let core_state = core_state_rc.borrow();

    let mut hs = v8::HandleScope::new(isolate);
    let scope = hs.enter();

    let (new_websocket_tx, new_websocket_rx) =
      mpsc::unbounded::<WebSocketProxy>();
    let (canary_tx, canary_rx) = oneshot::channel::<Never>();

    let info = InspectorInfo {
      host,
      uuid: Uuid::new_v4(),
      thread_name: thread::current().name().map(|n| n.to_owned()),
      new_websocket_tx,
      canary_rx,
    };

    // Create DenoInspector instance.
    let mut self_ = new_box_with(|self_ptr| {
      let v8_inspector_client =
        v8::inspector::V8InspectorClientBase::new::<Self>();
      let v8_inspector =
        v8::inspector::V8Inspector::create(scope, unsafe { &mut *self_ptr });

      let sessions = InspectorSessions::new(self_ptr, new_websocket_rx);
      let flags = InspectorFlags::new();
      let waker = InspectorWaker::new(scope.isolate().thread_safe_handle());

      Self {
        v8_inspector_client,
        v8_inspector,
        sessions,
        flags,
        waker,
        _canary_tx: canary_tx,
        debugger_url: info.get_websocket_debugger_url(),
      }
    });

    // Tell the inspector about the global context.
    let context = core_state.global_context.get(scope).unwrap();
    let context_name = v8::inspector::StringView::from(&b"global context"[..]);
    self_.context_created(context, Self::CONTEXT_GROUP_ID, context_name);

    // Register this inspector with the server thread.
    InspectorServer::register_inspector(info);

    // Poll the session handler so we will get notified whenever there is
    // new_incoming debugger activity.
    let _ = self_.poll_sessions(None).unwrap();

    self_
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

struct InspectorSessions {
  new_incoming:
    Pin<Box<dyn Stream<Item = Box<DenoInspectorSession>> + 'static>>,
  handshake: Option<Box<DenoInspectorSession>>,
  established: FuturesUnordered<Box<DenoInspectorSession>>,
}

impl InspectorSessions {
  fn new(
    inspector_ptr: *mut DenoInspector,
    new_websocket_rx: UnboundedReceiver<WebSocketProxy>,
  ) -> RefCell<Self> {
    let new_incoming = new_websocket_rx
      .map(move |websocket| DenoInspectorSession::new(inspector_ptr, websocket))
      .boxed_local();
    let self_ = Self {
      new_incoming,
      ..Default::default()
    };
    RefCell::new(self_)
  }
}

impl Default for InspectorSessions {
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
  inspector_ptr: Option<NonNull<DenoInspector>>,
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
    let mut g = self.0.lock().unwrap();
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
            let inspector = unsafe { &*(arg as *mut DenoInspector) };
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

struct DenoInspectorSession {
  v8_channel: v8::inspector::ChannelBase,
  v8_session: v8::UniqueRef<v8::inspector::V8InspectorSession>,
  websocket_tx: WebSocketProxySender,
  websocket_rx_handler: Pin<Box<dyn Future<Output = ()> + 'static>>,
}

impl Deref for DenoInspectorSession {
  type Target = v8::inspector::V8InspectorSession;
  fn deref(&self) -> &Self::Target {
    &self.v8_session
  }
}

impl DerefMut for DenoInspectorSession {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.v8_session
  }
}

impl DenoInspectorSession {
  const CONTEXT_GROUP_ID: i32 = 1;

  pub fn new(
    inspector_ptr: *mut DenoInspector,
    websocket: WebSocketProxy,
  ) -> Box<Self> {
    new_box_with(move |self_ptr| {
      let v8_channel = v8::inspector::ChannelBase::new::<Self>();
      let v8_session = unsafe { &mut *inspector_ptr }.connect(
        Self::CONTEXT_GROUP_ID,
        // Todo(piscisaureus): V8Inspector::connect() should require that
        // the 'v8_channel' argument cannot move.
        unsafe { &mut *self_ptr },
        v8::inspector::StringView::empty(),
      );

      let (websocket_tx, websocket_rx) = websocket.split();
      let websocket_rx_handler =
        Self::receive_from_websocket(self_ptr, websocket_rx);

      Self {
        v8_channel,
        v8_session,
        websocket_tx,
        websocket_rx_handler,
      }
    })
  }

  /// Returns a future that receives messages from the websocket and dispatches
  /// them to the V8 session.
  fn receive_from_websocket(
    self_ptr: *mut Self,
    websocket_rx: WebSocketProxyReceiver,
  ) -> Pin<Box<dyn Future<Output = ()> + 'static>> {
    async move {
      eprintln!("Debugger session started.");

      let result = websocket_rx
        .map_ok(move |msg| {
          let msg = msg.as_bytes();
          let msg = v8::inspector::StringView::from(msg);
          unsafe { &mut *self_ptr }.dispatch_protocol_message(msg);
        })
        .try_collect::<()>()
        .await;

      match result {
        Ok(_) => eprintln!("Debugger session ended."),
        Err(err) => eprintln!("Debugger session ended: {}.", err),
      };
    }
    .boxed_local()
  }

  fn send_to_websocket(&self, msg: v8::UniquePtr<v8::inspector::StringBuffer>) {
    let msg = msg.unwrap().string().to_string();
    let msg = ws::Message::text(msg);
    let _ = self.websocket_tx.unbounded_send(msg);
  }

  pub fn break_on_next_statement(&mut self) {
    let reason = v8::inspector::StringView::from(&b"debugCommand"[..]);
    let detail = v8::inspector::StringView::empty();
    self.schedule_pause_on_next_statement(reason, detail);
  }
}

impl v8::inspector::ChannelImpl for DenoInspectorSession {
  fn base(&self) -> &v8::inspector::ChannelBase {
    &self.v8_channel
  }

  fn base_mut(&mut self) -> &mut v8::inspector::ChannelBase {
    &mut self.v8_channel
  }

  fn send_response(
    &mut self,
    _call_id: i32,
    message: v8::UniquePtr<v8::inspector::StringBuffer>,
  ) {
    self.send_to_websocket(message);
  }

  fn send_notification(
    &mut self,
    message: v8::UniquePtr<v8::inspector::StringBuffer>,
  ) {
    self.send_to_websocket(message);
  }

  fn flush_protocol_notifications(&mut self) {}
}

impl Future for DenoInspectorSession {
  type Output = ();
  fn poll(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
    self.websocket_rx_handler.poll_unpin(cx)
  }
}

fn new_box_with<T>(new_fn: impl FnOnce(*mut T) -> T) -> Box<T> {
  let b = Box::new(MaybeUninit::<T>::uninit());
  let p = Box::into_raw(b) as *mut T;
  unsafe { ptr::write(p, new_fn(p)) };
  unsafe { Box::from_raw(p) }
}
