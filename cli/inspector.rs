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
use futures::FutureExt;
use futures::SinkExt;
use futures::StreamExt;
use futures::TryFutureExt;
use futures::TryStreamExt;
use std::collections::HashMap;
use std::ffi::c_void;
use std::future::Future;
use std::mem::MaybeUninit;
use std::net::SocketAddrV4;
use std::pin::Pin;
use std::ptr;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::task::Context;
use std::task::Poll;
use std::task::Waker;
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
  frontend_to_inspector_tx: FrontendToInspectorTx,
  inspector_handle: DenoInspectorHandle,
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

    let server_msg_tx = self.server_msg_tx.as_ref().unwrap().clone();
    let address = self.address;
    let (frontend_to_inspector_tx, frontend_to_inspector_rx) =
      mpsc::unbounded_channel::<FrontendToInspectorMsg>();
    let uuid = Uuid::new_v4();

    let inspector = crate::inspector::DenoInspector::new(
      scope,
      context,
      frontend_to_inspector_rx,
      isolate_handle,
    );

    eprintln!(
      "Debugger listening on {}",
      websocket_debugger_url(address, &uuid)
    );

    server_msg_tx
      .send(ServerMsg::AddInspector(InspectorInfo {
        uuid,
        frontend_to_inspector_tx,
        inspector_handle: DenoInspectorHandle::new(&inspector),
      }))
      .unwrap_or_else(|_| {
        panic!("sending message to inspector server thread failed");
      });

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

        // Not to be confused with the WS's uuid...
        let session_uuid = Uuid::new_v4();

        inspector_info
          .frontend_to_inspector_tx
          .send(FrontendToInspectorMsg::WsConnection {
            session_uuid,
            websocket,
          })
          .unwrap_or_else(|_| {
            panic!("sending message to frontend_to_inspector_tx failed");
          });

        inspector_info.inspector_handle.interrupt();

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

pub struct DenoInspector {
  client: v8::inspector::V8InspectorClientBase,
  inspector: v8::UniqueRef<v8::inspector::V8Inspector>,
  sessions2: FuturesUnordered<Pin<Box<dyn Future<Output = ()>>>>,
  frontend_to_inspector_rx: FrontendToInspectorRx,
  paused: bool,
  interrupted: Arc<AtomicBool>,
  isolate_handle: v8::IsolateHandle,
  interrupting_waker: std::task::Waker,
}

impl DenoInspector {
  pub fn new<P>(
    scope: &mut P,
    context: v8::Local<v8::Context>,
    frontend_to_inspector_rx: FrontendToInspectorRx,
    isolate_handle: v8::IsolateHandle,
  ) -> Box<Self>
  where
    P: v8::InIsolate,
  {
    let mut deno_inspector = new_box_with(|address| Self {
      client: v8::inspector::V8InspectorClientBase::new::<Self>(),
      // TODO(piscisaureus): V8Inspector::create() should require that
      // the 'client' argument cannot move.
      inspector: v8::inspector::V8Inspector::create(scope, unsafe {
        &mut *address
      }),
      sessions2: FuturesUnordered::new(),
      frontend_to_inspector_rx,
      paused: false,
      interrupted: Arc::new(AtomicBool::new(false)),
      isolate_handle,
      interrupting_waker: NoopWaker::new(),
    });
    deno_inspector.interrupting_waker =
      InterruptingWaker::new(&deno_inspector, &NoopWaker::new());

    let empty_view = v8::inspector::StringView::empty();
    deno_inspector.inspector.context_created(
      context,
      CONTEXT_GROUP_ID,
      &empty_view,
    );

    deno_inspector
  }

  fn dispatch_frontend_to_inspector_msg(
    &mut self,
    msg: FrontendToInspectorMsg,
  ) {
    match msg {
      FrontendToInspectorMsg::WsConnection {
        session_uuid,
        websocket,
      } => self
        .sessions2
        .push(handle_session(&mut self.inspector, websocket).boxed_local()),
    };
  }

  extern "C" fn poll_interrupt(
    _isolate: &mut v8::Isolate,
    self_ptr: *mut c_void,
  ) {
    eprintln!("Interrupt!");
    let self_ = unsafe { &mut *(self_ptr as *mut Self) };
    let _ = self_.poll_without_waker();
  }

  fn poll_without_waker(&mut self) -> Poll<<Self as Future>::Output> {
    eprintln!("interrupted poll");
    let _ = loop {
      match self.frontend_to_inspector_rx.try_recv() {
        Ok(msg) => self.dispatch_frontend_to_inspector_msg(msg),
        Err(TryRecvError::Closed) => break Poll::Ready(()),
        Err(TryRecvError::Empty) => break Poll::Pending,
      }
    };
    let cx = &mut Context::from_waker(&self.interrupting_waker);
    loop {
      match self.sessions2.poll_next_unpin(cx) {
        Poll::Ready(Some(_)) => {}
        _ if self.interrupted.swap(false, Ordering::AcqRel) => {}
        _ => break Poll::Pending,
      }
    }
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
  }

  fn quit_message_loop_on_pause(&mut self) {
    self.paused = false;
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
    let (mut channel_tx, mut channel_rx) = unbounded::<MessageResult>();
    let (mut websocket_tx, websocket_rx) = websocket.split();
    let mut tx_pump = channel_rx.forward(websocket_tx);

    let empty_view = v8::inspector::StringView::empty();

    new_box_with(move |address| {
      Self {
        channel: v8::inspector::ChannelBase::new::<Self>(),
        session: inspector.connect(
          CONTEXT_GROUP_ID,
          // Todo(piscisaureus): V8Inspector::connect() should require that
          // the 'channel' argument cannot move.
          unsafe { &mut *address },
          &empty_view,
        ),
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

    let r = loop {
      match self_.rx.poll_next_unpin(cx) {
        Ready(Some(Ok(msg))) => self_.dispatch_inbound(msg),
        Ready(None) => break Ready(Ok(())),
        Ready(Some(Err(e))) => break Ready(Err(e)),
        Pending => break Pending,
      }
    };

    let s = self_.tx_pump.poll_unpin(cx);

    match (r, s) {
      (v @ Ready(Err(_)), _) => v,
      (_, v @ Ready(Err(_))) => v,
      (v @ Ready(Ok(_)), Ready(Ok(_))) => v,
      _ => Pending,
    }
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
