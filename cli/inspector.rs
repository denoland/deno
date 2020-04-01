// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

// The documentation for the inspector API is sparse, but these are helpful:
// https://chromedevtools.github.io/devtools-protocol/
// https://hyperandroid.com/2020/02/12/v8-inspector-from-an-embedder-standpoint/

#![allow(clippy::option_map_unit_fn)]

use core::convert::Infallible as Never; // Alias for the future `!` type.
use deno_core;
use deno_core::v8;
use futures::channel::mpsc;
use futures::channel::mpsc::UnboundedReceiver;
use futures::channel::mpsc::UnboundedSender;
use futures::channel::oneshot;
use futures::future::Future;
use futures::prelude::*;
use futures::select;
use futures::task;
use futures::task::Poll;
use std::cell::BorrowMutError;
use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::c_void;
use std::mem::replace;
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
use warp;
use warp::filters::ws;
use warp::filters::ws::WebSocket;
use warp::Filter;

struct InspectorServer {
  host: SocketAddr,
  register_inspector_tx: UnboundedSender<InspectorInfo>,
}

impl InspectorServer {
  // Returns the global InspectorServer instance. If the server is not yet
  // running, this function starts it.
  pub fn global(host: SocketAddr) -> &'static InspectorServer {
    let instance = unsafe {
      static mut INSTANCE: Option<InspectorServer> = None;
      static INIT: Once = Once::new();
      INIT.call_once(|| {
        INSTANCE.replace(Self::new(host));
      });
      INSTANCE.as_ref().unwrap()
    };
    assert_eq!(host, instance.host);
    instance
  }

  fn new(host: SocketAddr) -> Self {
    let (register_inspector_tx, register_inspector_rx) =
      mpsc::unbounded::<InspectorInfo>();
    thread::spawn(move || {
      crate::tokio_util::run_basic(server(host, register_inspector_rx))
    });
    Self {
      host,
      register_inspector_tx,
    }
  }

  pub fn register_inspector(&self, info: InspectorInfo) {
    self.register_inspector_tx.unbounded_send(info).unwrap();
  }
}

/// Inspector information that is sent from the isolate thread to the server
/// thread when a new inspector is created.
pub struct InspectorInfo {
  uuid: Uuid,
  thread_name: Option<String>,
  new_session_tx: UnboundedSender<WebSocket>,
  canary_rx: oneshot::Receiver<Never>,
}

impl InspectorInfo {
  fn get_websocket_debugger_url(&self, host: &SocketAddr) -> String {
    format!("ws://{}/ws/{}", host, &self.uuid)
  }

  fn get_frontend_url(&self, host: &SocketAddr) -> String {
    format!(
      "chrome-devtools://devtools/bundled/inspector.html?v8only=true&ws={}/ws/{}",
      host, &self.uuid
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
      eprintln!("Inspector listening at {}", info.get_frontend_url(&host));
      inspector_map_
        .lock()
        .unwrap()
        .insert(info.uuid, info)
        .map(|_| panic!("Inspector UUID already in map"));
    })
    .collect::<()>();

  let inspector_map_ = inspector_map_.clone();
  let mut deregister_inspector_handler = future::poll_fn(|cx| {
    inspector_map_
      .lock()
      .unwrap()
      .retain(|_, info| info.canary_rx.poll_unpin(cx) == Poll::Pending);
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
            inspector_map_
              .lock()
              .unwrap()
              .get(&uuid)
              .map(|info| info.new_session_tx.clone())
              .map(|new_session_tx| {
                ws.on_upgrade(move |websocket| async move {
                  let _ = new_session_tx.unbounded_send(websocket);
                })
              })
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
    let json_values = inspector_map_
      .lock()
      .unwrap()
      .values()
      .map(|info| {
        let title = format!(
          "[{}] deno{}",
          process::id(),
          info
            .thread_name
            .as_ref()
            .map(|n| format!(" - {}", n))
            .unwrap_or_default()
        );
        json!({
          "description": "deno",
          "devtoolsFrontendUrl": info.get_frontend_url(&host),
          "faviconUrl": "https://deno.land/favicon.ico",
          "id": info.uuid.to_string(),
          "title": title,
          "type": "deno",
          "url": "file://",
          "webSocketDebuggerUrl": info.get_websocket_debugger_url(&host),
        })
      })
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
    _ = deregister_inspector_handler => panic!(),
    _ = server_handler => panic!(),
  }
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum RunMode {
  Running,
  OnPause,
  WaitingForDebugger,
}

#[derive(Clone, Copy)]
enum PollState {
  Idle,
  Woken,
  Polling,
  Parked,
  Dropped,
}

enum PollEntry<'a> {
  InspectorCreated(bool),
  FuturePolled(&'a task::Waker),
  Pause,
  Interrupt,
}

pub struct DenoInspector {
  v8_inspector_client: v8::inspector::V8InspectorClientBase,
  v8_inspector: v8::UniqueRef<v8::inspector::V8Inspector>,
  session_handler: RefCell<Pin<Box<dyn Future<Output = ()>>>>,
  inspector_waker: Arc<InspectorWaker>,
  _canary_tx: oneshot::Sender<Never>,
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
    self
      .inspector_waker
      .update(|w| w.poll_state = PollState::Dropped);
    // V8 automatically deletes all sessions when an Inspector instance is
    // deleted, however InspectorSession also has a drop handler that cleans
    // up after itself. To avoid a double free, make sure the inspector is
    // dropped last.
    replace(
      &mut *self.session_handler.borrow_mut(),
      async {}.boxed_local(),
    );
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
    self
      .inspector_waker
      .update(|w| w.run_mode = RunMode::Running);
  }

  fn run_if_waiting_for_debugger(&mut self, context_group_id: i32) {
    assert_eq!(context_group_id, DenoInspectorSession::CONTEXT_GROUP_ID);
    let was_waiting_for_debugger = self.inspector_waker.update(|w| {
      if let RunMode::WaitingForDebugger = w.run_mode {
        w.run_mode = RunMode::Running;
        true
      } else {
        false
      }
    });
    if was_waiting_for_debugger {
      schedule_pause_on_next_statement(&mut self.v8_inspector);
    }
  }
}

/// DenoInspector implements a Future so that it can poll for incoming messages
/// from the WebSocket server. Since a Worker ownes a DenoInspector, and because
/// a Worker is a Future too, Worker::poll will call this.
impl Future for DenoInspector {
  type Output = ();
  fn poll(self: Pin<&mut Self>, cx: &mut task::Context) -> Poll<()> {
    self
      .poll_session_handler(PollEntry::FuturePolled(cx.waker()))
      .unwrap()
  }
}

impl DenoInspector {
  const CONTEXT_GROUP_ID: i32 = 1;

  pub fn new(
    isolate: &mut deno_core::Isolate,
    host: SocketAddr,
    wait_for_debugger: bool,
  ) -> Box<Self> {
    let deno_core::Isolate {
      v8_isolate,
      global_context,
      ..
    } = isolate;

    let v8_isolate = v8_isolate.as_mut().unwrap();
    let mut hs = v8::HandleScope::new(v8_isolate);
    let scope = hs.enter();

    let (new_session_tx, new_session_rx) = mpsc::unbounded::<WebSocket>();
    let (canary_tx, canary_rx) = oneshot::channel::<Never>();

    // Create DenoInspector instance.
    let mut self_ = new_box_with(|self_ptr| {
      let v8_inspector_client =
        v8::inspector::V8InspectorClientBase::new::<Self>();
      let v8_inspector =
        v8::inspector::V8Inspector::create(scope, unsafe { &mut *self_ptr });
      let session_handler =
        Self::create_session_handler(self_ptr, new_session_rx);
      let inspector_waker =
        InspectorWaker::new(scope.isolate().thread_safe_handle());
      Self {
        v8_inspector_client,
        v8_inspector,
        session_handler,
        inspector_waker,
        _canary_tx: canary_tx,
      }
    });

    // Tell the inspector about the global context.
    let context = global_context.get(scope).unwrap();
    let context_name = v8::inspector::StringView::from(&b"global context"[..]);
    self_.context_created(context, Self::CONTEXT_GROUP_ID, &context_name);

    // Register this inspector with the server thread.
    // Note: poll_session_handler() might block if we need to wait for a
    // debugger front-end to connect. Therefore the server thread must to be
    // nofified *before* polling.
    let server = InspectorServer::global(host);
    let info = InspectorInfo {
      uuid: Uuid::new_v4(),
      thread_name: thread::current().name().map(|n| n.to_owned()),
      new_session_tx,
      canary_rx,
    };
    server.register_inspector(info);

    // Poll the session handler so we will get notified whenever there is
    // incoming debugger activity.
    let _ = self_
      .poll_session_handler(PollEntry::InspectorCreated(wait_for_debugger))
      .unwrap();

    self_
  }

  fn create_session_handler(
    self_: *mut Self,
    new_session_rx: impl Stream<Item = WebSocket> + 'static,
  ) -> RefCell<Pin<Box<dyn Future<Output = ()>>>> {
    let fut = new_session_rx
      .for_each_concurrent(None, move |websocket| {
        DenoInspectorSession::new(unsafe { &mut *self_ }, websocket)
      })
      .boxed_local();
    RefCell::new(fut)
  }

  fn poll_session_handler(
    &self,
    entry: PollEntry,
  ) -> Result<Poll<()>, BorrowMutError> {
    // The session handler's poll() function is not re-entrant. However it is
    // possible that poll_session_handler() gets re-entered, for example when an
    // interrupt request is honored while the inspector future is polled by
    // the task executor. When this happens, return an error.
    let mut session_handler = self.session_handler.try_borrow_mut()?;

    self.inspector_waker.update(|w| {
      // Update the run mode if we got here after receiving a 'pause' event,
      // or when the isolate has just started and we need to wait for a
      // debugger session to connect.
      match entry {
        PollEntry::InspectorCreated(wait) if wait => {
          w.run_mode = RunMode::WaitingForDebugger
        }
        PollEntry::Pause => w.run_mode = RunMode::OnPause,
        _ => {}
      }
      // Set state to 'polling'.
      match w.poll_state {
        PollState::Idle | PollState::Woken => w.poll_state = PollState::Polling,
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
            match w.poll_state {
              PollState::Woken => {
                // The inspector was woken while the session handler was being
                // polled, so we poll it another time.
                w.poll_state = PollState::Polling;
              }
              PollState::Polling if w.run_mode == RunMode::Running => {
                // The session handler doesn't need to be polled any longer, and
                // there's no reason to block (execution is not paused), so
                // we're going to return from the poll_session_handler()
                // function.
                w.poll_state = PollState::Idle;
                // Register the task waker that can be used to wake the parent
                // task that will poll the inspector future.
                if let PollEntry::FuturePolled(task_waker) = entry {
                  w.task_waker.replace(task_waker.clone());
                }
                // Register the host of the inspector which allows the waker
                // to request an interrupt from the isolate.
                w.inspector_ptr = NonNull::new(self as *const _ as *mut Self);
              }
              PollState::Polling if w.run_mode != RunMode::Running => {
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
            PollState::Idle => break Ok(result), // Yield to task.
            PollState::Polling => {} // Poll the session handler again.
            PollState::Parked => thread::park(), // Park the thread.
            _ => unreachable!(),
          };
        }
        Poll::Ready(_) => break Ok(result), // Session has ended.
      }
    }
  }
}

struct InspectorWakerInner {
  run_mode: RunMode,
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
      run_mode: RunMode::Running,
      poll_state: PollState::Idle,
      task_waker: None,
      parked_thread: None,
      inspector_ptr: None,
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
      match w.poll_state {
        PollState::Idle => {
          // Wake the task, if any, that has polled the Inspector future last.
          w.task_waker.take().map(|waker| waker.wake());
          // Request an interrupt from the isolate if it's running and there's
          // not unhandled interrupt request in flight.
          w.inspector_ptr
            .take()
            .map(|ptr| ptr.as_ptr() as *mut c_void)
            .map(|arg| {
              w.isolate_handle.request_interrupt(handle_interrupt, arg);
            });
          extern "C" fn handle_interrupt(
            _isolate: &mut v8::Isolate,
            arg: *mut c_void,
          ) {
            let inspector = unsafe { &*(arg as *mut DenoInspector) };
            let _ = inspector.poll_session_handler(PollEntry::Interrupt);
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
  message_handler: Pin<Box<dyn Future<Output = ()> + 'static>>,
  // Internal channel/queue that temporarily stores messages sent by V8 to
  // the front-end, before they are sent over the websocket.
  outbound_queue_tx:
    UnboundedSender<v8::UniquePtr<v8::inspector::StringBuffer>>,
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
    v8_inspector: &mut v8::inspector::V8Inspector,
    websocket: WebSocket,
  ) -> Box<Self> {
    new_box_with(move |self_ptr| {
      let v8_channel = v8::inspector::ChannelBase::new::<Self>();

      let empty_view = v8::inspector::StringView::empty();
      let v8_session = v8_inspector.connect(
        Self::CONTEXT_GROUP_ID,
        // Todo(piscisaureus): V8Inspector::connect() should require that
        // the 'v8_channel' argument cannot move.
        unsafe { &mut *self_ptr },
        &empty_view,
      );

      let (outbound_queue_tx, outbound_queue_rx) =
        mpsc::unbounded::<v8::UniquePtr<v8::inspector::StringBuffer>>();

      let message_handler =
        Self::create_message_handler(self_ptr, websocket, outbound_queue_rx);

      Self {
        v8_channel,
        v8_session,
        message_handler,
        outbound_queue_tx,
      }
    })
  }

  fn create_message_handler(
    self_: *mut Self,
    websocket: WebSocket,
    outbound_queue_rx: UnboundedReceiver<
      v8::UniquePtr<v8::inspector::StringBuffer>,
    >,
  ) -> Pin<Box<dyn Future<Output = ()> + 'static>> {
    let (websocket_tx, websocket_rx) = websocket.split();

    // Receive messages from the websocket and dispatch them to the V8 session.
    let inbound_pump = websocket_rx
      .map_ok(move |msg| {
        let msg = msg.as_bytes();
        let msg = v8::inspector::StringView::from(msg);
        unsafe { &mut *self_ }.dispatch_protocol_message(&msg);
      })
      .try_collect::<()>();

    // Convert and forward messages from the outbound message queue to the
    // websocket.
    let outbound_pump = outbound_queue_rx
      .map(move |msg| {
        let msg = msg.unwrap().string().to_string();
        let msg = ws::Message::text(msg);
        Ok(msg)
      })
      .forward(websocket_tx);

    let disconnect_future = future::try_join(inbound_pump, outbound_pump);

    async move {
      eprintln!("Inspector session started.");
      match disconnect_future.await {
        Ok(_) => eprintln!("Inspector session ended."),
        Err(err) => eprintln!("Inspector session ended: {}.", err),
      };
    }
    .boxed_local()
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
    let _ = self.outbound_queue_tx.unbounded_send(message);
  }

  fn send_notification(
    &mut self,
    message: v8::UniquePtr<v8::inspector::StringBuffer>,
  ) {
    let _ = self.outbound_queue_tx.unbounded_send(message);
  }

  fn flush_protocol_notifications(&mut self) {}
}

impl Future for DenoInspectorSession {
  type Output = ();
  fn poll(
    mut self: Pin<&mut Self>,
    cx: &mut task::Context,
  ) -> Poll<Self::Output> {
    self.message_handler.poll_unpin(cx)
  }
}

fn new_box_with<T>(new_fn: impl FnOnce(*mut T) -> T) -> Box<T> {
  let b = Box::new(MaybeUninit::<T>::uninit());
  let p = Box::into_raw(b) as *mut T;
  unsafe { ptr::write(p, new_fn(p)) };
  unsafe { Box::from_raw(p) }
}

// TODO: REMOVE ASAP! This is a total hack to work around a missing binding
// in rusty_v8, `V8InspectorSession::schedulePauseOnNextStatement`.
use schedule_pause_on_next_statement_hack::schedule_pause_on_next_statement;
mod schedule_pause_on_next_statement_hack {
  use super::*;
  pub fn schedule_pause_on_next_statement(
    v8_inspector: &mut v8::inspector::V8Inspector,
  ) {
    let mut session = HelperSession::new(v8_inspector);
    let messages = &[
      r#"{"id":1,"method":"Debugger.enable"}"#,
      r#"{"id":2,"method":"Debugger.pause"}"#,
    ];
    for msg in messages {
      let msg = v8::inspector::StringView::from(msg.as_bytes());
      session.v8_session.dispatch_protocol_message(&msg);
    }
  }
  struct HelperSession {
    v8_channel: v8::inspector::ChannelBase,
    v8_session: v8::UniqueRef<v8::inspector::V8InspectorSession>,
  }
  impl HelperSession {
    fn new(v8_inspector: &mut v8::inspector::V8Inspector) -> Box<Self> {
      new_box_with(move |self_ptr| {
        let v8_channel = v8::inspector::ChannelBase::new::<Self>();
        let empty_view = v8::inspector::StringView::empty();
        let v8_session = v8_inspector.connect(
          DenoInspectorSession::CONTEXT_GROUP_ID,
          unsafe { &mut *self_ptr },
          &empty_view,
        );
        Self {
          v8_channel,
          v8_session,
        }
      })
    }
  }
  impl v8::inspector::ChannelImpl for HelperSession {
    fn base(&self) -> &v8::inspector::ChannelBase {
      &self.v8_channel
    }
    fn base_mut(&mut self) -> &mut v8::inspector::ChannelBase {
      &mut self.v8_channel
    }
    fn send_response(
      &mut self,
      _call_id: i32,
      _message: v8::UniquePtr<v8::inspector::StringBuffer>,
    ) {
    }
    fn send_notification(
      &mut self,
      _message: v8::UniquePtr<v8::inspector::StringBuffer>,
    ) {
    }
    fn flush_protocol_notifications(&mut self) {}
  }
}
