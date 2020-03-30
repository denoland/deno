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
use std::cell::BorrowMutError;
use std::cell::RefCell;
use std::cell::RefMut;
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

/// Thread-safe state associated with a DenoInspector instance.
struct InspectorStateInner {
  woken: bool,
  blocked: bool,
  paused: bool,
  interrupt_requested: bool,
  inspector: *const DenoInspector,
  isolate_handle: v8::IsolateHandle,
  isolate_thread: thread::Thread,
  parent_waker: Option<task::Waker>,
}

unsafe impl Send for InspectorStateInner {}

struct InspectorState(Mutex<Option<InspectorStateInner>>);

impl InspectorState {
  fn new(state: InspectorStateInner) -> Arc<Self> {
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

impl task::ArcWake for InspectorState {
  fn wake_by_ref(arc_self: &Arc<Self>) {
    arc_self.map(|state| {
      state.woken = true;
      if state.blocked {
        // Wake the isolate thread.
        assert_ne!(state.isolate_thread.id(), thread::current().id());
        state.blocked = false;
        state.isolate_thread.unpark();
      } else {
        // Wake the main event loop.
        state.parent_waker.take().map(|w| w.wake());
        // Interrupt the isolate if it is running.
        state.interrupt_requested = state.interrupt_requested
          || state.isolate_handle.request_interrupt(
            DenoInspector::poll_interrupt,
            state.inspector as *const c_void as *mut c_void,
          );
      }
    });
  }
}

pub struct DenoInspector {
  client: v8::inspector::V8InspectorClientBase,
  waker_state: Arc<InspectorState>,
  handler: RefCell<Pin<Box<dyn Future<Output = ()>>>>,
}

impl DenoInspector {
  pub fn new(
    scope: &mut impl v8::InIsolate,
    connections_rx: impl Stream<Item = ws::WebSocket> + 'static,
  ) -> Box<Self> {
    let mut self_ = new_box_with(|address| {
      let client = v8::inspector::V8InspectorClientBase::new::<Self>();

      let waker_state = InspectorState::new(InspectorStateInner {
        woken: false,
        blocked: false,
        paused: false,
        interrupt_requested: false,
        inspector: address,
        isolate_handle: scope.isolate().thread_safe_handle(),
        isolate_thread: thread::current(),
        parent_waker: None,
      });

      let mut v8_inspector =
        v8::inspector::V8Inspector::create(scope, unsafe { &mut *address });

      let mut scope = v8::HandleScope::new(scope);
      let scope = scope.enter();
      if let Some(context) = scope.get_current_context() {
        let empty_view = v8::inspector::StringView::empty();
        v8_inspector.context_created(context, CONTEXT_GROUP_ID, &empty_view);
      }

      let handler =
        connections_rx.for_each_concurrent(None, move |websocket| {
          DenoInspectorSession::new(&mut v8_inspector, websocket)
            .unwrap_or_else(|err| {
              eprintln!("Inspector session disconnected: {:?}", err)
            })
        });
      let handler = RefCell::new(handler.boxed_local());

      Self {
        client,
        waker_state,
        handler,
      }
    });

    self_.try_poll().expect("Inspector could not be polled");

    self_
  }

  extern "C" fn poll_interrupt(
    _isolate: &mut v8::Isolate,
    self_ptr: *mut c_void,
  ) {
    let self_ = unsafe { &*(self_ptr as *const _ as *const Self) };
    self_.waker_state.map(|state| {
      state.interrupt_requested = false;
    });
    let _ = self_.try_poll();
  }

  fn try_poll(&self) -> Result<Poll<()>, BorrowMutError> {
    let Self {
      handler,
      waker_state,
      ..
    } = self;
    // If try_borrow_mut() fails, this means we've attempted to re-enter
    // the poll function. This is expected under certain circumstances, however
    // if we actually went through with it it would lead to a crash.
    let mut handler = handler.try_borrow_mut()?;
    let poll_result = Self::do_poll(handler.as_mut(), waker_state);
    Ok(poll_result)
  }

  fn do_poll(
    mut handler: Pin<&mut dyn Future<Output = ()>>,
    waker_state: &Arc<InspectorState>,
  ) -> Poll<()> {
    let waker_ref = task::waker_ref(&waker_state);
    let mut cx = Context::from_waker(&waker_ref);

    loop {
      waker_state.map(|state| {
        state.woken = false;
      });

      let result = handler.poll_unpin(&mut cx);

      let (should_continue, should_block) = waker_state
        .map(|state| {
          if state.woken {
            (true, false)
          } else if state.paused {
            assert_eq!(state.isolate_thread.id(), thread::current().id());
            state.blocked = true;
            (true, true)
          } else {
            (false, false)
          }
        })
        .unwrap();

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
    self_
      .waker_state
      .map(|state| {
        let parent_waker = cx.waker();
        match &mut state.parent_waker {
          Some(pw) if pw.will_wake(parent_waker) => {}
          pw => {
            pw.replace(parent_waker.clone());
          }
        };
      })
      .unwrap();
    self_.try_poll().expect("Inspector could not be polled")
  }
}

impl Drop for DenoInspector {
  fn drop(&mut self) {
    self.waker_state.drop_inspector();
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
    self.waker_state.map(|state| state.paused = true);
    let _ = self.try_poll();
    eprintln!("!!! run_message_loop_on_pause END");
  }

  fn quit_message_loop_on_pause(&mut self) {
    eprintln!("!!! quit_message_loop_on_pause");
    self.waker_state.map(|state| state.paused = false);
  }

  fn run_if_waiting_for_debugger(&mut self, context_group_id: i32) {
    assert_eq!(context_group_id, CONTEXT_GROUP_ID);
  }
}

struct DenoInspectorSession {
  channel: v8::inspector::ChannelBase,
  session: v8::UniqueRef<v8::inspector::V8InspectorSession>,
  // Internal channel/queue that temporarily stores messages sent by V8 to
  // the front-end, before they are sent over the websocket.
  tx: UnboundedSender<Result<ws::Message, warp::Error>>,
  rx: SplitStream<ws::WebSocket>,
  tx_pump: Forward<
    UnboundedReceiver<Result<ws::Message, warp::Error>>,
    SplitSink<ws::WebSocket, ws::Message>,
  >,
}

impl DenoInspectorSession {
  pub fn new(
    inspector: &mut v8::inspector::V8Inspector,
    websocket: ws::WebSocket,
  ) -> Box<Self> {
    new_box_with(move |channel_address| {
      let (mut channel_tx, mut channel_rx) =
        unbounded::<Result<ws::Message, warp::Error>>();
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

    match (rx_poll, tx_poll) {
      (Ready(r1), Ready(r2)) => Ready(r1.and(r2)),
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
