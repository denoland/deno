// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

// The documentation for the inspector API is sparse, but these are helpful:
// https://chromedevtools.github.io/devtools-protocol/
// https://hyperandroid.com/2020/02/12/v8-inspector-from-an-embedder-standpoint/

#![allow(clippy::option_map_unit_fn)]

use core::convert::Infallible as Never; // Alias for the future `!` type.
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
use std::cmp::max;
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
use std::thread;
use uuid::Uuid;
use warp;
use warp::filters::ws;
use warp::filters::ws::WebSocket;
use warp::Filter;

/// Owned by GlobalState.
pub struct InspectorServer {
  thread_handle: Option<thread::JoinHandle<()>>,
  register_inspector_tx: Option<UnboundedSender<InspectorInfo>>,
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
  fn get_websocket_debugger_url(&self, address: &SocketAddr) -> String {
    format!("ws://{}/ws/{}", address, &self.uuid)
  }

  fn get_frontend_url(&self, address: &SocketAddr) -> String {
    format!(
      "devtools://devtools/bundled/inspector.html?remotefrontend=true&v8only=true&ws={}/ws/{}",
      address, &self.uuid
    )
  }
}

impl InspectorServer {
  pub fn new(host: &str, brk: bool) -> Self {
    if brk {
      todo!("--inspect-brk not yet supported");
    }
    let address = host.parse::<SocketAddr>().unwrap();
    let (register_inspector_tx, register_inspector_rx) =
      mpsc::unbounded::<InspectorInfo>();
    let thread_handle = thread::spawn(move || {
      crate::tokio_util::run_basic(server(address, register_inspector_rx))
    });
    Self {
      thread_handle: Some(thread_handle),
      register_inspector_tx: Some(register_inspector_tx),
    }
  }

  /// Each worker/isolate to be debugged should call this exactly one.
  /// Called from worker's thread.
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
    let mut cs = v8::ContextScope::new(scope, context);
    let scope = cs.enter();

    let uuid = Uuid::new_v4();
    let thread_name = thread::current().name().map(|n| n.to_owned());
    let (new_session_tx, new_session_rx) = mpsc::unbounded::<WebSocket>();
    let (canary_tx, canary_rx) = oneshot::channel::<Never>();
    let inspector = DenoInspector::new(scope, new_session_rx, canary_tx);

    let inspector_info = InspectorInfo {
      uuid,
      thread_name,
      new_session_tx,
      canary_rx,
    };

    self
      .register_inspector_tx
      .as_ref()
      .unwrap()
      .unbounded_send(inspector_info)
      .unwrap_or_else(|_| {
        panic!("sending message to inspector server thread failed");
      });

    inspector
  }
}

impl Drop for InspectorServer {
  fn drop(&mut self) {
    self.register_inspector_tx.take();
    self.thread_handle.take().unwrap().join().unwrap();
    panic!("TODO: this drop is never called");
  }
}

async fn server(
  address: SocketAddr,
  register_inspector_rx: UnboundedReceiver<InspectorInfo>,
) {
  // TODO: wrap `address` and `inspector_map` in an Rc<RefCell<T>> instead.
  // This is currently not possible because warp requires all filters to
  // implement Send, which should not be necessary because we are using a
  // single-threaded runtime.
  let address = Arc::new(Mutex::new(address));
  let inspector_map = HashMap::<Uuid, InspectorInfo>::new();
  let inspector_map = Arc::new(Mutex::new(inspector_map));

  let address_ = address.clone();
  let inspector_map_ = inspector_map.clone();
  let mut register_inspector_handler = register_inspector_rx
    .map(|info| {
      eprintln!(
        "Inspector listening at {}",
        info.get_frontend_url(&*address_.lock().unwrap())
      );
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

  let address_ = address.clone();
  let inspector_map_ = inspector_map.clone();
  let json_list_route = warp::path("json").map(move || {
    let address = *address_.lock().unwrap();
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
          "devtoolsFrontendUrl": info.get_frontend_url(&address),
          "faviconUrl": "https://deno.land/favicon.ico",
          "id": info.uuid.to_string(),
          "title": title,
          "type": "deno",
          "url": "file://",
          "webSocketDebuggerUrl": info.get_websocket_debugger_url(&address),
        })
      })
      .collect::<Vec<_>>();
    warp::reply::json(&json!(json_values))
  });

  let server_routes =
    websocket_route.or(json_version_route).or(json_list_route);

  let mut server_handler = {
    let address = &mut *address.lock().unwrap();
    let mut port = address.port();
    let max_port = max(port + 64, u16::max_value() - 1);
    let mut first_error = None;
    loop {
      let server = warp::serve(server_routes.clone());
      match server.try_bind_ephemeral(*address) {
        Ok((_, fut)) => break fut,
        Err(err) => {
          let err = first_error.get_or_insert(err);
          if port < max_port {
            port += 1;
            address.set_port(port);
          } else {
            eprintln!("Cannot start inspector server: {}.", err);
            process::exit(1);
          }
        }
      };
    }
  }
  .fuse();

  select! {
    _ = register_inspector_handler => (),
    _ = deregister_inspector_handler => panic!(),
    _ = server_handler => panic!(),
  }
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
  Future(&'a task::Waker),
  Pause,
  Other,
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
      .update(|w| w.state = PollState::Dropped);
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
  fn poll(self: Pin<&mut Self>, cx: &mut task::Context) -> Poll<()> {
    self
      .poll_session_handler(PollEntry::Future(cx.waker()))
      .unwrap()
  }
}

impl DenoInspector {
  const CONTEXT_GROUP_ID: i32 = 1;

  pub fn new(
    scope: &mut impl v8::InIsolate,
    new_session_rx: impl Stream<Item = WebSocket> + 'static,
    canary_tx: oneshot::Sender<Never>,
  ) -> Box<Self> {
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

    self_.register_current_context(scope);
    let _ = self_.poll_session_handler(PollEntry::Other).unwrap();

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
      // Set 'on_pause' flag if this function was called by the
      // run_message_loop_on_pause() function.
      if let PollEntry::Pause = entry {
        w.on_pause = true;
      }
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
                w.inspector_ptr = NonNull::new(self as *const _ as *mut Self);
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
  state: PollState,
  on_pause: bool,
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
      state: PollState::Idle,
      on_pause: false,
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
      match w.state {
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
            let _ = inspector.poll_session_handler(PollEntry::Other);
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
