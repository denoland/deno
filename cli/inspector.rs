// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

// The documentation for the inspector API is sparse, but these are helpful:
// https://chromedevtools.github.io/devtools-protocol/
// https://hyperandroid.com/2020/02/12/v8-inspector-from-an-embedder-standpoint/

#![allow(dead_code)]
#![allow(warnings)]

use crate::ErrBox;
use deno_core::v8;
use futures;
use futures::channel;
use futures::channel::mpsc;
use futures::executor;
use futures::future;
use futures::future::IntoFuture;
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
use std::ops::Deref;
use std::ops::DerefMut;
use std::pin::Pin;
use std::ptr;
use std::ptr::null;
use std::ptr::NonNull;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::sync::Mutex;
use std::task::Context;
use std::task::Poll;
use std::task::Waker;
use std::thread;
use uuid::Uuid;
use warp;
use warp::filters::ws;
use warp::Filter;

/// Stored in a UUID hashmap, used by WS server. Clonable.
type InspectorInfo = mpsc::UnboundedSender<ws::WebSocket>;

/// Owned by GlobalState.
pub struct InspectorServer {
  address: SocketAddrV4,
  thread_handle: Option<thread::JoinHandle<()>>,
  new_inspector_tx: Option<mpsc::UnboundedSender<InspectorInfo>>,
}

impl InspectorServer {
  pub fn new(host: &str, brk: bool) -> Self {
    if brk {
      todo!("--inspect-brk not yet supported");
    }
    let address = host.parse::<SocketAddrV4>().unwrap();
    let (new_inspector_tx, new_inspector_rx) =
      mpsc::unbounded::<InspectorInfo>();
    let thread_handle = thread::spawn(move || {
      crate::tokio_util::run_basic(server(address, new_inspector_rx));
    });
    Self {
      address,
      thread_handle: Some(thread_handle),
      new_inspector_tx: Some(new_inspector_tx),
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
    let (new_session_tx, new_session_rx) = mpsc::unbounded::<ws::WebSocket>();

    let inspector = crate::inspector::DenoInspector::new(scope, new_session_rx);

    self
      .new_inspector_tx
      .as_ref()
      .unwrap()
      .unbounded_send(new_session_tx)
      .unwrap_or_else(|_| {
        panic!("sending message to inspector server thread failed");
      });

    inspector
  }
}

impl Drop for InspectorServer {
  fn drop(&mut self) {
    self.new_inspector_tx.take();
    self.thread_handle.take().unwrap().join().unwrap();
    panic!("TODO: this drop is never called");
  }
}

fn websocket_debugger_url(address: SocketAddrV4, uuid: &Uuid) -> String {
  format!("ws://{}:{}/ws/{}", address.ip(), address.port(), uuid)
}

async fn server(
  address: SocketAddrV4,
  mut new_inspector_rx: mpsc::UnboundedReceiver<InspectorInfo>,
) {
  let inspector_map = HashMap::<Uuid, InspectorInfo>::new();
  let inspector_map = Arc::new(std::sync::Mutex::new(inspector_map));

  let inspector_map_ = inspector_map.clone();
  let new_inspector_handler = new_inspector_rx
    .map(|new_session_tx| {
      let uuid = Uuid::new_v4();
      inspector_map_
        .lock()
        .unwrap()
        .insert(uuid, new_session_tx)
        .map(|_| panic!("Inspector UUID already in map"));
      eprintln!(
        "Debugger listening on {}",
        websocket_debugger_url(address, &uuid)
      );
    })
    .fold((), |_, _| future::ready(()));

  let inspector_map_ = inspector_map.clone();
  let websocket_route = warp::path("ws")
    .and(warp::path::param())
    .and_then(move |uuid: String| {
      let r = Uuid::parse_str(&uuid)
        .ok()
        .and_then(|uuid| {
          inspector_map_.lock().unwrap().get(&uuid).map(Clone::clone)
        })
        .ok_or_else(warp::reject::not_found);
      future::ready(r)
    })
    .and(warp::ws())
    .map(
      |new_session_tx: mpsc::UnboundedSender<_>, ws: warp::ws::Ws| {
        ws.on_upgrade(move |websocket| async move {
          let _ = new_session_tx.unbounded_send(websocket);
        })
      },
    );
  let inspector_map_ = inspector_map.clone();
  let json_list_route =
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

  let version_route = warp::path!("json" / "version").map(|| {
    warp::reply::json(&json!({
      "Browser": format!("Deno/{}", crate::version::DENO),
      "Protocol-Version": "1.3",
      "V8-Version": crate::version::v8(),
    }))
  });

  let routes = websocket_route.or(version_route).or(json_list_route);
  let (_, web_handler) = warp::serve(routes)
    .try_bind_ephemeral(address)
    .unwrap_or_else(|e| {
      eprintln!("Cannot start inspector server: {}", e);
      std::process::exit(1);
    });

  future::join(new_inspector_handler, web_handler).await;
}

#[derive(Clone, Copy)]
enum PollState {
  Idle,
  Woken,
  Polling,
  Parked,
  Dropped,
}

#[derive(Clone)]
enum PollEntry<'a> {
  Future(&'a Waker),
  Pause,
  Other,
}

pub struct DenoInspector {
  v8_inspector_client: v8::inspector::V8InspectorClientBase,
  v8_inspector: v8::UniqueRef<v8::inspector::V8Inspector>,
  session_handler: RefCell<Pin<Box<dyn Future<Output = ()>>>>,
  inspector_waker: Arc<InspectorWaker>,
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
    self
      .inspector_waker
      .update(|w| w.state = PollState::Dropped);
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
    let _ = self.poll_session_handler(PollEntry::Pause);
  }

  fn quit_message_loop_on_pause(&mut self) {
    self.inspector_waker.update(|w| w.on_pause = false);
  }

  fn run_if_waiting_for_debugger(&mut self, context_group_id: i32) {
    assert_eq!(context_group_id, DenoInspectorSession::CONTEXT_GROUP_ID);
  }
}

/// DenoInspector implements a Future so that it can poll for incoming messages
/// from the WebSocket server. Since a Worker ownes a DenoInspector, and because
/// a Worker is a Future too, Worker::poll will call this.
impl Future for DenoInspector {
  type Output = ();
  fn poll(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<()> {
    self
      .poll_session_handler(PollEntry::Future(cx.waker()))
      .unwrap()
  }
}

impl DenoInspector {
  const CONTEXT_GROUP_ID: i32 = 1;

  pub fn new(
    scope: &mut impl v8::InIsolate,
    new_session_rx: impl Stream<Item = ws::WebSocket> + 'static,
  ) -> Box<Self> {
    let mut self_ = new_box_with(|address| {
      let v8_inspector_client =
        v8::inspector::V8InspectorClientBase::new::<Self>();

      let mut v8_inspector =
        v8::inspector::V8Inspector::create(scope, unsafe { &mut *address });

      let session_handler =
        Self::create_session_handler(address, new_session_rx);

      let inspector_waker =
        InspectorWaker::new(scope.isolate().thread_safe_handle());

      Self {
        v8_inspector_client,
        v8_inspector,
        session_handler,
        inspector_waker,
      }
    });

    self_.register_current_context(scope);
    self_.poll_session_handler(PollEntry::Other).unwrap();

    self_
  }

  fn register_current_context(&mut self, scope: &mut impl v8::InIsolate) {
    let mut scope = v8::HandleScope::new(scope);
    let scope = scope.enter();
    if let Some(context) = scope.get_current_context() {
      let empty_view = v8::inspector::StringView::empty();
      self.context_created(context, Self::CONTEXT_GROUP_ID, &empty_view);
    }
  }

  fn create_session_handler(
    self_: *mut Self,
    new_session_rx: impl Stream<Item = ws::WebSocket> + 'static,
  ) -> RefCell<Pin<Box<dyn Future<Output = ()>>>> {
    let fut = new_session_rx
      .for_each_concurrent(None, move |websocket| async move {
        let session =
          DenoInspectorSession::new(unsafe { &mut *self_ }, websocket);
        eprintln!("Inspector session started.");
        match session.await {
          Ok(_) => eprintln!("Inspector session ended."),
          Err(err) => eprintln!("Inspector session ended: {}.", err),
        };
      })
      .boxed_local();
    RefCell::new(fut)
  }

  fn poll_session_handler(
    &self,
    mut entry: PollEntry,
  ) -> Result<Poll<()>, BorrowMutError> {
    // The session handler's poll() function is not re-entrant. However it is
    // possible that poll_session_handler() gets re-entered, for example when an
    // interrupt request is honored while the inspector future is polled by
    // the task executor. When this happens, return an error.
    let mut session_handler = self.session_handler.try_borrow_mut()?;

    self.inspector_waker.update(|w| {
      // Set 'on_pause' flag if this function was called by the
      // run_message_loop_on_pause() function.
      match entry {
        PollEntry::Pause => w.on_pause = true,
        _ => {}
      };
      // Set state to 'polling'.
      match w.state {
        PollState::Idle | PollState::Woken => w.state = PollState::Polling,
        _ => unreachable!(),
      };
    });

    // Create a new task::Context object that will make downstream futures
    // use the InspectorWaker when they are ready to be polled again.
    let waker_ref = task::waker_ref(&self.inspector_waker);
    let mut cx = task::Context::from_waker(&waker_ref);

    loop {
      let result = session_handler.as_mut().poll_unpin(&mut cx);

      match result {
        Poll::Pending => {
          let new_state = self.inspector_waker.update(|w| {
            match w.state {
              PollState::Woken => {
                // The inspector was woken while the session handler was being
                // polled, so we poll it another time.
                w.state = PollState::Polling;
              }
              PollState::Polling if !w.on_pause => {
                // The session handler doesn't need to be polled any longer, and
                // there's no reason to block (execution is not paused), so
                // we're going to return from the poll_session_handler()
                // function.
                w.state = PollState::Idle;
                // Register the task waker that can be used to wake the parent
                // task that will poll the inspector future.
                if let PollEntry::Future(task_waker) = entry {
                  w.task_waker.replace(task_waker.clone());
                }
                // Register the address of the inspector which allows the waker
                // to request an interrupt from the isolate.
                w.inspector_address =
                  NonNull::new(self as *const _ as *mut Self);
              }
              PollState::Polling if w.on_pause => {
                // Isolate execution has been paused but there are no more
                // events to process, so this thread will be parked. Therefore,
                // store the current thread handle in the waker so it knows
                // which thread to unpark when new events arrive.
                w.state = PollState::Parked;
                w.parked_thread.replace(thread::current());
              }
              _ => unreachable!(),
            };
            w.state
          });
          match new_state {
            PollState::Idle => break Ok(Poll::Pending), // Yield to task.
            PollState::Polling => {} // Poll the session handler again.
            PollState::Parked => thread::park(), // Park the thread.
            _ => unreachable!(),
          };
        }
        Poll::Ready(r) => break Ok(Poll::Ready(r)), // Session has ended.
      }
    }
  }
}

unsafe impl Send for InspectorWakerInner {}

struct InspectorWakerInner {
  state: PollState,
  on_pause: bool,
  task_waker: Option<task::Waker>,
  parked_thread: Option<thread::Thread>,
  inspector_address: Option<NonNull<DenoInspector>>,
  isolate_handle: v8::IsolateHandle,
}

struct InspectorWaker(Mutex<InspectorWakerInner>);

impl InspectorWaker {
  fn new(isolate_handle: v8::IsolateHandle) -> Arc<Self> {
    let inner = InspectorWakerInner {
      state: PollState::Idle,
      on_pause: false,
      task_waker: None,
      parked_thread: None,
      inspector_address: None,
      isolate_handle,
    };
    Arc::new(Self(Mutex::new(inner)))
  }

  fn update<F, R>(&self, f: F) -> R
  where
    F: FnOnce(&mut InspectorWakerInner) -> R,
  {
    let mut guard = self.0.lock().unwrap();
    f(&mut guard)
  }
}

impl task::ArcWake for InspectorWaker {
  fn wake_by_ref(arc_self: &Arc<Self>) {
    arc_self.update(|w| {
      match w.state {
        PollState::Idle => {
          // Wake the task, if any, that has polled the Inspector future last.
          w.task_waker.take().map(|waker| waker.wake());
          // Request an interrupt from the isolate if it's running and there's
          // not unhandled interrupt request in flight.
          w.inspector_address
            .take()
            .map(|address| address.as_ptr() as *mut c_void)
            .map(|arg| {
              w.isolate_handle.request_interrupt(handle_interrupt, arg);
            });
          extern "C" fn handle_interrupt(
            isolate: &mut v8::Isolate,
            arg: *mut c_void,
          ) {
            let inspector = unsafe { &*(arg as *mut DenoInspector) };
            inspector.poll_session_handler(PollEntry::Other);
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
      w.state = PollState::Woken;
    });
  }
}

struct DenoInspectorSession {
  channel: v8::inspector::ChannelBase,
  session: v8::UniqueRef<v8::inspector::V8InspectorSession>,
  // Internal channel/queue that temporarily stores messages sent by V8 to
  // the front-end, before they are sent over the websocket.
  tx: mpsc::UnboundedSender<Result<ws::Message, warp::Error>>,
  rx: SplitStream<ws::WebSocket>,
  tx_pump: Forward<
    mpsc::UnboundedReceiver<Result<ws::Message, warp::Error>>,
    SplitSink<ws::WebSocket, ws::Message>,
  >,
}

impl DenoInspectorSession {
  const CONTEXT_GROUP_ID: i32 = 1;

  pub fn new(
    inspector: &mut v8::inspector::V8Inspector,
    websocket: ws::WebSocket,
  ) -> Box<Self> {
    new_box_with(move |channel_address| {
      let (mut channel_tx, mut channel_rx) =
        mpsc::unbounded::<Result<ws::Message, warp::Error>>();
      let (mut websocket_tx, websocket_rx) = websocket.split();
      let mut tx_pump = channel_rx.forward(websocket_tx);

      let empty_view = v8::inspector::StringView::empty();

      let session = inspector.connect(
        Self::CONTEXT_GROUP_ID,
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
