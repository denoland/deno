// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

// The documentation for the inspector API is sparse, but these are helpful:
// https://chromedevtools.github.io/devtools-protocol/
// https://hyperandroid.com/2020/02/12/v8-inspector-from-an-embedder-standpoint/

use deno_core::v8;
use futures;
use futures::executor;
use futures::future;
use futures::FutureExt;
use futures::SinkExt;
use futures::StreamExt;
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
/// Owned by the inspector server thread, used to to receive information about
/// new isolates.
type ServerMsgRx = mpsc::UnboundedReceiver<ServerMsg>;
/// These messages can be sent from any thread to the server thread.
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
    session_to_frontend_tx: SessionToFrontendTx,
  },
  WsIncoming {
    session_uuid: Uuid,
    msg: ws::Message,
  },
}

/// Owned by the deno inspector session, used to forward messages from the
/// inspector channel on the isolate thread to the websocket that is owned by
/// the inspector server.
type SessionToFrontendTx = mpsc::UnboundedSender<ws::Message>;
/// Owned by the inspector server. Messages arriving on this channel, coming
/// from the inspector session on the isolate thread are forwarded over the
/// websocket to the devtools frontend.
type SessionToFrontendRx = mpsc::UnboundedReceiver<ws::Message>;

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
    );

    info!(
      "Debugger listening on {}",
      websocket_debugger_url(address, &uuid)
    );

    server_msg_tx
      .send(ServerMsg::AddInspector(InspectorInfo {
        uuid,
        frontend_to_inspector_tx,
        inspector_handle: DenoInspectorHandle::new(
          &inspector,
          v8_isolate.thread_safe_handle(),
        ),
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
      ws.on_upgrade(move |socket| async move {
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

        // send a message back so register_worker can return...
        let (mut ws_tx, mut ws_rx) = socket.split();

        let (session_to_frontend_tx, mut session_to_frontend_rx): (
          SessionToFrontendTx,
          SessionToFrontendRx,
        ) = mpsc::unbounded_channel();

        // Not to be confused with the WS's uuid...
        let session_uuid = Uuid::new_v4();

        inspector_info
          .frontend_to_inspector_tx
          .send(FrontendToInspectorMsg::WsConnection {
            session_to_frontend_tx,
            session_uuid,
          })
          .unwrap_or_else(|_| {
            panic!("sending message to frontend_to_inspector_tx failed");
          });

        inspector_info.inspector_handle.interrupt();

        let pump_to_inspector = async {
          while let Some(Ok(msg)) = ws_rx.next().await {
            inspector_info
              .frontend_to_inspector_tx
              .send(FrontendToInspectorMsg::WsIncoming { msg, session_uuid })
              .unwrap_or_else(|_| {
                panic!("sending message to frontend_to_inspector_tx failed");
              });

            inspector_info.inspector_handle.interrupt();
          }
        };

        let pump_from_session = async {
          while let Some(msg) = session_to_frontend_rx.next().await {
            ws_tx.send(msg).await.ok();
          }
        };

        future::join(pump_to_inspector, pump_from_session).await;
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
      "Protocol-Version": "1.3",
      "V8-Version": crate::version::v8(),
    }))
  });

  let routes = websocket.or(version).or(json_list);
  let (_, web_handler) = warp::serve(routes)
    .try_bind_ephemeral(address)
    .unwrap_or_else(|e| {
      eprintln!("Cannot start inspector server: {}", e);
      std::process::exit(1);
    });

  future::join(msg_handler, web_handler).await;
}

pub struct DenoInspector {
  client: v8::inspector::V8InspectorClientBase,
  inspector: v8::UniqueRef<v8::inspector::V8Inspector>,
  pub sessions: HashMap<Uuid, Box<DenoInspectorSession>>,
  frontend_to_inspector_rx: FrontendToInspectorRx,
  paused: bool,
  interrupted: Arc<AtomicBool>,
}

impl DenoInspector {
  pub fn new<P>(
    scope: &mut P,
    context: v8::Local<v8::Context>,
    frontend_to_inspector_rx: FrontendToInspectorRx,
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
      sessions: HashMap::new(),
      frontend_to_inspector_rx,
      paused: false,
      interrupted: Arc::new(AtomicBool::new(false)),
    });

    let empty_view = v8::inspector::StringView::empty();
    deno_inspector.inspector.context_created(
      context,
      CONTEXT_GROUP_ID,
      &empty_view,
    );

    deno_inspector
  }

  pub fn connect(
    &mut self,
    session_uuid: Uuid,
    session_to_frontend_tx: SessionToFrontendTx,
  ) {
    let session =
      DenoInspectorSession::new(&mut self.inspector, session_to_frontend_tx);
    self.sessions.insert(session_uuid, session);
  }

  fn dispatch_frontend_to_inspector_msg(
    &mut self,
    msg: FrontendToInspectorMsg,
  ) {
    match msg {
      FrontendToInspectorMsg::WsConnection {
        session_uuid,
        session_to_frontend_tx,
      } => {
        self.connect(session_uuid, session_to_frontend_tx);
      }
      FrontendToInspectorMsg::WsIncoming { session_uuid, msg } => {
        if let Some(deno_session) = self.sessions.get_mut(&session_uuid) {
          deno_session.dispatch_protocol_message(msg)
        } else {
          info!("Unknown inspector session {}. msg {:?}", session_uuid, msg);
        }
      }
    };
  }

  extern "C" fn poll_interrupt(
    _isolate: &mut v8::Isolate,
    self_ptr: *mut c_void,
  ) {
    let self_ = unsafe { &mut *(self_ptr as *mut Self) };
    let _ = self_.poll_without_waker();
  }

  fn poll_without_waker(&mut self) -> Poll<<Self as Future>::Output> {
    loop {
      match self.frontend_to_inspector_rx.try_recv() {
        Ok(msg) => self.dispatch_frontend_to_inspector_msg(msg),
        Err(TryRecvError::Closed) => break Poll::Ready(()),
        Err(TryRecvError::Empty)
          if self.interrupted.swap(false, Ordering::AcqRel) => {}
        Err(TryRecvError::Empty) => break Poll::Pending,
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

#[derive(Clone)]
struct DenoInspectorHandle {
  deno_inspector_ptr: *mut c_void,
  isolate_handle: v8::IsolateHandle,
  interrupted: Arc<AtomicBool>,
}

impl DenoInspectorHandle {
  pub fn new(
    deno_inspector: &DenoInspector,
    isolate_handle: v8::IsolateHandle,
  ) -> Self {
    Self {
      deno_inspector_ptr: deno_inspector as *const DenoInspector
        as *const c_void as *mut c_void,
      isolate_handle,
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

/// sub-class of v8::inspector::Channel
pub struct DenoInspectorSession {
  channel: v8::inspector::ChannelBase,
  session: v8::UniqueRef<v8::inspector::V8InspectorSession>,
  session_to_frontend_tx: SessionToFrontendTx,
}

impl DenoInspectorSession {
  pub fn new(
    inspector: &mut v8::inspector::V8Inspector,
    session_to_frontend_tx: SessionToFrontendTx,
  ) -> Box<Self> {
    new_box_with(|address| {
      let empty_view = v8::inspector::StringView::empty();
      Self {
        channel: v8::inspector::ChannelBase::new::<Self>(),
        session: inspector.connect(
          CONTEXT_GROUP_ID,
          // Todo(piscisaureus): V8Inspector::connect() should require that
          // the 'channel'  argument cannot move.
          unsafe { &mut *address },
          &empty_view,
        ),
        session_to_frontend_tx,
      }
    })
  }

  pub fn dispatch_protocol_message(&mut self, ws_msg: ws::Message) {
    let bytes = ws_msg.as_bytes();
    let string_view = v8::inspector::StringView::from(bytes);
    self.session.dispatch_protocol_message(&string_view);
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
    let ws_msg = v8_to_ws_msg(message);
    self.session_to_frontend_tx.send(ws_msg).unwrap();
  }

  fn send_notification(
    &mut self,
    message: v8::UniquePtr<v8::inspector::StringBuffer>,
  ) {
    let ws_msg = v8_to_ws_msg(message);
    self.session_to_frontend_tx.send(ws_msg).unwrap();
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
