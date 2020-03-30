// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

#![allow(dead_code)]
#![allow(warnings)]

use deno_core::v8;
use futures;
use futures::channel;
use futures::channel::mpsc::{unbounded, UnboundedReceiver, UnboundedSender};
use futures::executor;
use futures::future;
use futures::sink;
use futures::stream::Forward;
use futures::stream::FuturesOrdered;
use futures::stream::FuturesUnordered;
use futures::stream::SplitSink;
use futures::stream::SplitStream;
use futures::task;
use futures::task::AtomicWaker;
use futures::FutureExt;
use futures::SinkExt;
use futures::Stream;
use futures::StreamExt;
use futures::TryFutureExt;
use futures::TryStreamExt;
use std::collections::HashMap;
use std::ffi::c_void;
use std::future::Future;
use std::mem::replace;
use std::mem::MaybeUninit;
use std::net::SocketAddrV4;
use std::pin::Pin;
use std::ptr;
use std::ptr::NonNull;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::sync::Mutex;
use std::task::Context;
use std::task::Poll;
use std::task::Waker;
use std::thread;
use tokio;
use tokio::sync::mpsc;
use tokio::sync::mpsc::error::TryRecvError;
use uuid::Uuid;
use warp;
use warp::filters::ws;
use warp::Filter;

const CONTEXT_GROUP_ID: i32 = 1;

/// Owned by GloalState, this channel end can be used by any isolate thread
/// to register it's inspector with the inspector server.
type ServerMsgTx = mpsc::UnboundedSender<ServerMsg>;
// Owned by the inspector server thread, used to to receive information about
// new isolates.
type ServerMsgRx = mpsc::UnboundedReceiver<ServerMsg>;
// These messages can be sent from any thread to the server thread.
enum ServerMsg {
  AddInspector(InspectorInfo),
}

/// Owned by the web socket server. Relays incoming websocket connections and
/// messages to the isolate/inspector thread.
type FrontendToInspectorTx = mpsc::UnboundedSender<FrontendToInspectorMsg>;
/// Owned by the isolate/worker. Receives incoming websocket connections and
/// messages from the inspector server thread.
type FrontendToInspectorRx = mpsc::UnboundedReceiver<FrontendToInspectorMsg>;
/// Messages sent over the FrontendToInspectorTx/FrontendToInspectorRx channel.
pub enum FrontendToInspectorMsg {
  WsConnection {
    session_uuid: Uuid,
    websocket: ws::WebSocket,
  },
  //WsIncoming {
  //  session_uuid: Uuid,
  //  msg: ws::Message,
  //},
}

/// Owned by the deno inspector session, used to forward messages from the
/// inspector channel on the isolate thread to the websocket that is owned by
/// the inspector server.
type SessionToFrontendTx = mpsc::UnboundedSender<ws::Message>;
/// Owned by the inspector server. Messages arriving on this channel, coming
/// from the inspector session on the isolate thread are forwarded over the
/// websocket to the devtools frontend.
type SessionToFrontendRx = mpsc::UnboundedReceiver<ws::Message>;

type FrontendToSessionTx = mpsc::UnboundedSender<ws::Message>;
type FrontendToSessionRx = mpsc::UnboundedReceiver<ws::Message>;

/// Stored in a UUID hashmap, used by WS server. Clonable.
#[derive(Clone)]
struct InspectorInfo {
  uuid: Uuid,
  connections_tx: mpsc::UnboundedSender<ws::WebSocket>,
}

/// Owned by GlobalState.
pub struct InspectorServer {
  address: SocketAddrV4,
  thread_handle: Option<std::thread::JoinHandle<()>>,
  server_msg_tx: Option<ServerMsgTx>,
}

impl InspectorServer {
  pub fn new(host: &str, brk: bool) -> Self {
    if brk {
      todo!("--inspect-brk not yet supported");
    }
    let address = host.parse::<SocketAddrV4>().unwrap();
    let (server_msg_tx, server_msg_rx) = mpsc::unbounded_channel::<ServerMsg>();
    let thread_handle = std::thread::spawn(move || {
      crate::tokio_util::run_basic(server(address, server_msg_rx));
    });
    Self {
      address,
      thread_handle: Some(thread_handle),
      server_msg_tx: Some(server_msg_tx),
    }
  }

  /// Each worker/isolate to be debugged should call this exactly one.
  /// Called from worker's thread
  pub fn add_inspector(
    &self,
    isolate: &mut deno_core::Isolate,
  ) -> Box<DenoInspector> {
    let deno_core::Isolate {
      v8_isolate,
      global_context,
      ..
    } = isolate;
    let v8_isolate = v8_isolate.as_mut().unwrap();
    let isolate_handle = v8_isolate.thread_safe_handle();

    let mut hs = v8::HandleScope::new(v8_isolate);
    let scope = hs.enter();
    let context = global_context.get(scope).unwrap();
    let mut cs = v8::ContextScope::new(scope, context);
    let scope = cs.enter();
    let uuid = Uuid::new_v4();
    let (tx, rx) = mpsc::unbounded_channel::<ws::WebSocket>();

    let inspector = crate::inspector::DenoInspector::new(scope, rx);

    self
      .server_msg_tx
      .as_ref()
      .unwrap()
      .send(ServerMsg::AddInspector(InspectorInfo {
        uuid,
        connections_tx: tx,
      }))
      .unwrap_or_else(|_| {
        panic!("sending message to inspector server thread failed");
      });

    eprintln!(
      "Debugger listening on {}",
      websocket_debugger_url(self.address, &uuid)
    );

    inspector
  }
}

impl Drop for InspectorServer {
  fn drop(&mut self) {
    self.server_msg_tx.take();
    self.thread_handle.take().unwrap().join().unwrap();
    panic!("TODO: this drop is never called");
  }
}

fn websocket_debugger_url(address: SocketAddrV4, uuid: &Uuid) -> String {
  format!("ws://{}:{}/ws/{}", address.ip(), address.port(), uuid)
}

async fn server(address: SocketAddrV4, mut server_msg_rx: ServerMsgRx) {
  let inspector_map = HashMap::<Uuid, InspectorInfo>::new();
  let inspector_map = Arc::new(std::sync::Mutex::new(inspector_map));

  let inspector_map_ = inspector_map.clone();
  let msg_handler = async move {
    while let Some(msg) = server_msg_rx.next().await {
      match msg {
        ServerMsg::AddInspector(inspector_info) => {
          let existing = inspector_map_
            .lock()
            .unwrap()
            .insert(inspector_info.uuid, inspector_info);
          if existing.is_some() {
            panic!("UUID already in map");
          }
        }
      };
    }
  };

  let inspector_map_ = inspector_map.clone();
  let websocket = warp::path("ws")
    .and(warp::path::param())
    .and(warp::ws())
    .map(move |uuid: String, ws: warp::ws::Ws| {
      let inspector_map__ = inspector_map_.clone();
      ws.on_upgrade(move |websocket| async move {
        let inspector_info = {
          if let Ok(uuid) = Uuid::parse_str(&uuid) {
            let g = inspector_map__.lock().unwrap();
            if let Some(inspector_info) = g.get(&uuid) {
              inspector_info.clone()
            } else {
              return;
            }
          } else {
            return;
          }
        };

        inspector_info
          .connections_tx
          .send(websocket)
          .unwrap_or_else(|_| {
            panic!("sending message to frontend_to_inspector_tx failed");
          });

        // inspector_info.inspector_handle.interrupt();

        //let pump_to_inspector = async {
        //  while let Some(Ok(msg)) = ws_rx.next().await {
        //    inspector_info
        //      .frontend_to_inspector_tx
        //      .send(FrontendToInspectorMsg::WsIncoming { msg, session_uuid })
        //      .unwrap_or_else(|_| {
        //        panic!("sending message to frontend_to_inspector_tx failed");
        //      });
        //
        //    inspector_info.inspector_handle.interrupt();
        //  }
        //};
      })
    });

  let inspector_map_ = inspector_map.clone();
  let json_list =
    warp::path("json")
      .map(move || {
        let g = inspector_map_.lock().unwrap();
        let json_values: Vec<serde_json::Value> = g.iter().map(|(uuid, _)| {
          let url = websocket_debugger_url(address, uuid);
          json!({
            "description": "deno",
            "devtoolsFrontendUrl": format!("chrome-devtools://devtools/bundled/js_app.html?experiments=true&v8only=true&ws={}", url),
            "faviconUrl": "https://deno.land/favicon.ico",
            "id": uuid.to_string(),
            "title": format!("deno[{}]", std::process::id()),
            "type": "deno",
            "url": "file://",
            "webSocketDebuggerUrl": url,
          })
        }).collect();
        warp::reply::json(&json!(json_values))
      });

  let version = warp::path!("json" / "version").map(|| {
    warp::reply::json(&json!({
      "Browser": format!("Deno/{}", crate::version::DENO),
      // TODO upgrade to 1.3? https://chromedevtools.github.io/devtools-protocol/
      "Protocol-Version": "1.3",
      "V8-Version": crate::version::v8(),
    }))
  });

  let routes = websocket.or(version).or(json_list);
  let web_handler = warp::serve(routes).bind(address);

  future::join(msg_handler, web_handler).await;
}

/*
struct DenoInspectorWaker {
  default_loop_waker: AtomicWaker,
  paused_loop_waker: AtomicWaker,
  isolate_handle: v8::IsolateHandle,
  interrupt_requested: AtomicBool,
  waiting_for_poll: AtomicBool
  waker: Option<Waker>
}

impl DenoInspectorWaker {
  pub fn new(isolate_handle: v8::IsolateHandle) -> Arc<Self> {
    let mut arc_self = Arc::new(Self {
      default_loop_waker: AtomicWaker::new(),
      paused_loop_waker: AtomicWaker::new(),
      isolate_handle,
      interrupt_requested: AtomicBool::new(false),
      waiting_for_poll: AtomicBool::new(false),
      waker: None
    });

    arc_self.waker = task::waker(arc_self.clone());
    std::mem::forget(arc_self.clone());
    assert_eq!(Arc::strong_count(&arc_self), 1);

    arc_self
  }

  pub fn begin_poll(&self) -> bool {
    self.waiting_for_poll.swap(false, Ordering::AcqRel);
  }

  pub fn set_default_loop_waker(&self, waker: &Waker) {
    self.default_loop_waker.register(waker);
  }

  pub fn set_paused_loop_waker(&self, waker: &Waker) {
    self.paused_loop_waker.register(waker);
  }

  pub fn waker(arc_self: &Arc<Self>) -> Waker {
    task::waker(arc_self.clone)
  }
}

impl ArcWake for DenoInspectorWaker {
  fn wake_by_ref(arc_self: &Arc<Self>) {

  }
}
*/

/// Thread-safe state associated with a DenoInspector instance.
struct InspectorStateInner {
  woken: bool,
  polling: bool,
  blocked: bool,
  paused: bool,
  interrupt_requested: bool,
  inspector: *mut DenoInspector,
  isolate_handle: v8::IsolateHandle,
  isolate_thread: thread::Thread,
  parent_waker: Option<task::Waker>,
}

unsafe impl Send for InspectorStateInner {}

struct InspectorState(Mutex<Option<InspectorStateInner>>);

impl InspectorState {
  fn new_arc(state: InspectorStateInner) -> Arc<Self> {
    Arc::new(Self(Mutex::new(Some(state))))
  }

  fn map<F, R>(&self, f: F) -> Option<R>
  where
    F: FnOnce(&mut InspectorStateInner) -> R,
  {
    let mut guard = self.0.lock().unwrap();
    guard.as_mut().map(f)
  }

  fn drop_inspector(&self) {
    let mut guard = self.0.lock().unwrap();
    guard
      .take()
      .expect("InspectorState::drop_inspector() called twice");
  }
}

fn debug_state(label: &'static str, state: &InspectorStateInner) {
  return;
  let main_thread = thread::current().id() == state.isolate_thread.id();
  eprintln!(
    r#"{}:
  main_thread: {:?}
  woken: {:?}
  polling: {:?}
  blocked: {:?}
  paused: {:?}
  interrupt_requested: {:?}"#,
    label,
    main_thread,
    state.woken,
    state.polling,
    state.blocked,
    state.paused,
    state.interrupt_requested
  );
}

impl task::ArcWake for InspectorState {
  fn wake_by_ref(arc_self: &Arc<Self>) {
    arc_self.map(|state| {
      debug_state("wake_by_ref before", state);
      state.woken = true;
      if state.blocked {
        // Wake the isolate thread.
        assert_ne!(state.isolate_thread.id(), thread::current().id());
        state.blocked = false;
        state.isolate_thread.unpark();
      } else if state.polling {
        // The thread is already polling, nothing further to do.
      } else {
        // Wake the main event loop.
        state.parent_waker.take().map(|w| w.wake());
        // Interrupt the isolate if it is running.
        state.interrupt_requested = state.interrupt_requested
          || state.isolate_handle.request_interrupt(
            DenoInspector::poll_interrupt,
            state.inspector as *mut c_void,
          );
      }
      debug_state("wake_by_ref after", state);
    });
  }
}

pub struct DenoInspector {
  //inspector: v8::UniqueRef<v8::inspector::V8Inspector>,
  client: v8::inspector::V8InspectorClientBase,
  state: Arc<InspectorState>,
  waker: Waker,
  handler: Pin<Box<dyn Future<Output = ()>>>,
}

impl DenoInspector {
  pub fn new(
    scope: &mut impl v8::InIsolate,
    connections_rx: impl Stream<Item = ws::WebSocket> + 'static,
  ) -> Box<Self> {
    let mut self_ = new_box_with(|address| {
      let client = v8::inspector::V8InspectorClientBase::new::<Self>();

      let state = InspectorState::new_arc(InspectorStateInner {
        woken: false,
        polling: false,
        blocked: false,
        paused: false,
        interrupt_requested: false,
        inspector: address,
        isolate_handle: scope.isolate().thread_safe_handle(),
        isolate_thread: thread::current(),
        parent_waker: None,
      });

      let waker = task::waker(state.clone());

      let handler = async {
        unimplemented!(); // Temporary placeholder.
      }
      .boxed_local();

      Self {
        //inspector,
        client,
        state,
        waker,
        handler,
      }
    });

    let mut v8_inspector =
      v8::inspector::V8Inspector::create(scope, &mut *self_);

    let mut scope = v8::HandleScope::new(scope);
    let scope = scope.enter();
    if let Some(context) = scope.get_current_context() {
      let empty_view = v8::inspector::StringView::empty();
      v8_inspector.context_created(context, CONTEXT_GROUP_ID, &empty_view);
    }

    self_.handler = connections_rx
      .for_each_concurrent(None, move |websocket| {
        DenoInspectorSession::new(&mut v8_inspector, websocket).unwrap_or_else(
          |err| eprintln!("Inspector session disconnected: {:?}", err),
        )
      })
      .boxed_local();

    self_.poll_impl(None);

    self_
  }

  extern "C" fn poll_interrupt(
    _isolate: &mut v8::Isolate,
    self_ptr: *mut c_void,
  ) {
    let self_ = unsafe { &mut *(self_ptr as *mut Self) };
    let re_entry = self_
      .state
      .map(|state| {
        state.interrupt_requested = false;
        state.polling
      })
      .unwrap();
    if !re_entry {
      let _ = self_.poll_impl(None);
    }
  }

  fn poll_impl(&mut self, parent_waker: Option<&Waker>) -> Poll<()> {
    let mut cx = Context::from_waker(&self.waker);
    loop {
      self.state.map(|state| {
        debug_state("poll loop pre-update start", state);
        state.woken = false;
        state.polling = true;
        debug_state("poll loop pre-update end", state);
      });

      let result = self.handler.poll_unpin(&mut cx);

      let (should_continue, should_block) = self
        .state
        .map(|state| {
          debug_state("poll loop post-update start", state);
          if state.woken {
            (true, false)
          } else if state.paused {
            assert_eq!(state.isolate_thread.id(), thread::current().id());
            state.blocked = true;
            (true, true)
          } else {
            state.polling = false;
            parent_waker.map(|w| state.parent_waker.replace(w.clone()));
            (false, false)
          }
        })
        .unwrap();
      self
        .state
        .map(|state| debug_state("poll loop post-update end", state));

      if !should_continue {
        break result;
      } else if should_block {
        thread::park();
      }
    }
  }
}

impl Future for DenoInspector {
  type Output = ();

  fn poll(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<()> {
    let mut self_ = self.as_mut();
    eprintln!("POLL!");
    self_.poll_impl(Some(cx.waker()))
  }
}

impl Drop for DenoInspector {
  fn drop(&mut self) {
    self.state.drop_inspector();
  }
}

impl v8::inspector::V8InspectorClientImpl for DenoInspector {
  fn base(&self) -> &v8::inspector::V8InspectorClientBase {
    &self.client
  }

  fn base_mut(&mut self) -> &mut v8::inspector::V8InspectorClientBase {
    &mut self.client
  }

  fn run_message_loop_on_pause(&mut self, context_group_id: i32) {
    eprintln!("!!! run_message_loop_on_pause START");

    assert_eq!(context_group_id, CONTEXT_GROUP_ID);
    let re_entry = self
      .state
      .map(|state| {
        state.paused = true;
        state.polling
      })
      .unwrap();
    if !re_entry {
      self.poll_impl(None);
    }

    eprintln!("!!! run_message_loop_on_pause END");
  }

  fn quit_message_loop_on_pause(&mut self) {
    eprintln!("!!! quit_message_loop_on_pause");
    self.state.map(|state| state.paused = false);
  }

  fn run_if_waiting_for_debugger(&mut self, context_group_id: i32) {
    assert_eq!(context_group_id, CONTEXT_GROUP_ID);
  }
}

/*
  fn poll_without_waker(&mut self) -> Poll<<Self as Future>::Output> {
    self.interrupt_depth += 1;
    eprintln!("+ START interrupt depth={}", self.interrupt_depth);
    let _ = loop {
      match self.frontend_to_inspector_rx.try_recv() {
        Ok(msg) => self.dispatch_frontend_to_inspector_msg(msg),
        Err(TryRecvError::Closed) => break Poll::Ready(()),
        Err(TryRecvError::Empty) => break Poll::Pending,
      }
    };
    let cx = &mut Context::from_waker(&self.interrupting_waker);
    let r = loop {
      match self.sessions2.poll_next_unpin(cx) {
        Poll::Ready(Some(_)) => {}
        _ => break Poll::Pending,
      }
    };
    eprintln!("- END interrupt depth={}", self.interrupt_depth);
    self.interrupt_depth -= 1;
    r
  }
}

/// DenoInspector implements a Future so that it can poll for incoming messages
/// from the WebSocket server. Since a Worker ownes a DenoInspector, and because
/// a Worker is a Future too, Worker::poll will call this.
impl Future for DenoInspector {
  type Output = ();

  fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
    let self_ = self.get_mut();

    let waker = InterruptingWaker::new(&self_, cx.waker());
    self_.interrupting_waker = waker.clone();
    let cx = &mut Context::from_waker(&waker);

    eprintln!("poll sessions2");
    let _ = loop {
      match self_.sessions2.poll_next_unpin(cx) {
        Poll::Ready(Some(_)) => {}
        _ => break (), //Poll::Pending,
      }
    };
    loop {
      match self_.frontend_to_inspector_rx.poll_recv(cx) {
        Poll::Ready(Some(msg)) => self_.dispatch_frontend_to_inspector_msg(msg),
        Poll::Ready(None) => break Poll::Ready(()),
        Poll::Pending if self_.interrupted.swap(false, Ordering::AcqRel) => {}
        Poll::Pending => break Poll::Pending,
      }
    }
  }
}

impl v8::inspector::V8InspectorClientImpl for DenoInspector {
  fn base(&self) -> &v8::inspector::V8InspectorClientBase {
    &self.client
  }

  fn base_mut(&mut self) -> &mut v8::inspector::V8InspectorClientBase {
    &mut self.client
  }

  fn run_message_loop_on_pause(&mut self, context_group_id: i32) {
    eprintln!("!!! run_message_loop_on_pause START");

    assert_eq!(context_group_id, CONTEXT_GROUP_ID);
    assert!(!self.paused);
    self.paused = true;

    // Creating a new executor and calling block_on generally causes a panic.
    // In this case it works because the outer executor is provided by tokio
    // and the one created here comes from the 'futures' crate, and they don't
    // see each other.
    let dispatch_messages_while_paused =
      future::poll_fn(|cx| match self.poll_unpin(cx) {
        Poll::Pending if self.paused => Poll::Pending,
        _ => Poll::Ready(()),
      });
    executor::block_on(dispatch_messages_while_paused);

    eprintln!("!!! run_message_loop_on_pause END");
  }

  fn quit_message_loop_on_pause(&mut self) {
    self.paused = false;
    eprintln!("!!! quit_message_loop_on_pause");
  }

  fn run_if_waiting_for_debugger(&mut self, context_group_id: i32) {
    assert_eq!(context_group_id, CONTEXT_GROUP_ID);
  }
}

struct InterruptingWaker {
  inspector_handle: DenoInspectorHandle,
  downstream_waker: Waker,
}

impl InterruptingWaker {
  pub fn new(
    deno_inspector: &DenoInspector,
    downstream_waker: &Waker,
  ) -> std::task::Waker {
    let w = Self {
      inspector_handle: DenoInspectorHandle::new(deno_inspector),
      downstream_waker: downstream_waker.clone(),
    };
    futures::task::waker(Arc::new(w))
  }
}

impl futures::task::ArcWake for InterruptingWaker {
  fn wake_by_ref(arc_self: &Arc<Self>) {
    eprintln!("wake!");
    arc_self.downstream_waker.wake_by_ref();
    arc_self.inspector_handle.interrupt();
  }
}

struct NoopWaker;

impl NoopWaker {
  pub fn new() -> std::task::Waker {
    futures::task::waker(Arc::new(Self))
  }
}

impl futures::task::ArcWake for NoopWaker {
  fn wake_by_ref(_: &Arc<Self>) {}
}

#[derive(Clone)]
struct DenoInspectorHandle {
  deno_inspector_ptr: *mut c_void,
  isolate_handle: v8::IsolateHandle,
  interrupted: Arc<AtomicBool>,
}

impl DenoInspectorHandle {
  pub fn new(deno_inspector: &DenoInspector) -> Self {
    Self {
      deno_inspector_ptr: deno_inspector as *const DenoInspector
        as *const c_void as *mut c_void,
      isolate_handle: deno_inspector.isolate_handle.clone(),
      interrupted: deno_inspector.interrupted.clone(),
    }
  }

  pub fn interrupt(&self) {
    if !self.interrupted.swap(true, Ordering::AcqRel) {
      eprintln!("Request interrupt");
      self.isolate_handle.request_interrupt(
        DenoInspector::poll_interrupt,
        self.deno_inspector_ptr,
      );
    }
  }
}

unsafe impl Send for DenoInspectorHandle {}
unsafe impl Sync for DenoInspectorHandle {}

fn handle_session(
  inspector: &'_ mut v8::inspector::V8Inspector,
  websocket: ws::WebSocket,
) -> Pin<Box<dyn Future<Output = ()>>> {
  let mut session = DenoInspectorSession::new(inspector, websocket);
  session
    .unwrap_or_else(|err| eprintln!("Debugger disconnected: {:?}", err))
    .boxed_local()
}
*/

type MessageResult = Result<ws::Message, warp::Error>;

struct DenoInspectorSession {
  channel: v8::inspector::ChannelBase,
  session: v8::UniqueRef<v8::inspector::V8InspectorSession>,
  // Internal channel/queue that temporarily stores messages sent by V8 to
  // the front-end, before they are sent over the websocket.
  tx: UnboundedSender<MessageResult>,
  rx: SplitStream<ws::WebSocket>,
  tx_pump: Forward<
    UnboundedReceiver<MessageResult>,
    SplitSink<ws::WebSocket, ws::Message>,
  >,
}

impl DenoInspectorSession {
  pub fn new(
    inspector: &mut v8::inspector::V8Inspector,
    websocket: ws::WebSocket,
  ) -> Box<Self> {
    new_box_with(move |channel_address| {
      let (mut channel_tx, mut channel_rx) = unbounded::<MessageResult>();
      let (mut websocket_tx, websocket_rx) = websocket.split();
      let mut tx_pump = channel_rx.forward(websocket_tx);

      let empty_view = v8::inspector::StringView::empty();

      let session = inspector.connect(
        CONTEXT_GROUP_ID,
        // Todo(piscisaureus): V8Inspector::connect() should require that
        // the 'channel' argument cannot move.
        unsafe { &mut *channel_address },
        &empty_view,
      );

      Self {
        channel: v8::inspector::ChannelBase::new::<Self>(),
        session,
        tx: channel_tx,
        rx: websocket_rx,
        tx_pump,
      }
    })
  }

  fn dispatch_inbound(&mut self, msg: ws::Message) {
    let bytes = msg.as_bytes();
    let string_view = v8::inspector::StringView::from(bytes);
    self.session.dispatch_protocol_message(&string_view);
  }

  fn send_outbound(&mut self, msg: v8::UniquePtr<v8::inspector::StringBuffer>) {
    let mut msg = msg.unwrap();
    let msg = msg.string().to_string();
    eprintln!("tx: {}", &msg);
    let msg = ws::Message::text(msg);
    let msg = Ok(msg);
    self
      .tx
      .unbounded_send(msg)
      .expect("unbounded_send() failed");
  }
}

impl Future for DenoInspectorSession {
  type Output = Result<(), warp::Error>;

  fn poll(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
    use Poll::*;
    let mut self_ = self.as_mut();

    let rx_poll = loop {
      match self_.rx.poll_next_unpin(cx) {
        Ready(Some(Ok(msg))) => self_.dispatch_inbound(msg),
        Ready(None) => break Ready(Ok(())),
        Ready(Some(Err(e))) => break Ready(Err(e)),
        Pending => break Pending,
      }
    };

    let tx_poll = self_.tx_pump.poll_unpin(cx);

    let r = match (rx_poll, tx_poll) {
      (Ready(r1), Ready(r2)) => Ready(r1.and(r2)),
      _ => Pending,
    };

    eprintln!("Client state: {:?}", r);
    r
  }
}

impl v8::inspector::ChannelImpl for DenoInspectorSession {
  fn base(&self) -> &v8::inspector::ChannelBase {
    &self.channel
  }

  fn base_mut(&mut self) -> &mut v8::inspector::ChannelBase {
    &mut self.channel
  }

  fn send_response(
    &mut self,
    _call_id: i32,
    message: v8::UniquePtr<v8::inspector::StringBuffer>,
  ) {
    self.send_outbound(message);
  }

  fn send_notification(
    &mut self,
    message: v8::UniquePtr<v8::inspector::StringBuffer>,
  ) {
    self.send_outbound(message);
  }

  fn flush_protocol_notifications(&mut self) {}
}

// TODO impl From or Into
fn v8_to_ws_msg(
  message: v8::UniquePtr<v8::inspector::StringBuffer>,
) -> ws::Message {
  let mut x = message.unwrap();
  let s = x.string().to_string();
  ws::Message::text(s)
}

fn new_box_with<T>(new_fn: impl FnOnce(*mut T) -> T) -> Box<T> {
  let b = Box::new(MaybeUninit::<T>::uninit());
  let p = Box::into_raw(b) as *mut T;
  unsafe { ptr::write(p, new_fn(p)) };
  unsafe { Box::from_raw(p) }
}

fn new_arc_with<T>(new_fn: impl FnOnce(*mut T) -> T) -> Box<T> {
  let b = Box::new(MaybeUninit::<T>::uninit());
  let p = Box::into_raw(b) as *mut T;
  unsafe { ptr::write(p, new_fn(p)) };
  unsafe { Box::from_raw(p) }
}
