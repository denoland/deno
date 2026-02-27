// Copyright 2018-2025 the Deno authors. MIT license.

//! The documentation for the inspector API is sparse, but these are helpful:
//! <https://chromedevtools.github.io/devtools-protocol/>
//! <https://web.archive.org/web/20210918052901/https://hyperandroid.com/2020/02/12/v8-inspector-from-an-embedder-standpoint/>

use crate::futures::channel::mpsc;
use crate::futures::channel::mpsc::UnboundedReceiver;
use crate::futures::channel::mpsc::UnboundedSender;
use crate::futures::channel::oneshot;
use crate::futures::prelude::*;
use crate::futures::stream::FuturesUnordered;
use crate::futures::stream::StreamExt;
use crate::futures::task;
use crate::serde_json::json;

use parking_lot::Mutex;
use std::cell::Cell;
use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::c_void;
use std::mem::take;
use std::pin::Pin;
use std::ptr::NonNull;
use std::rc::Rc;
use std::sync::Arc;
use std::task::Context;
use std::task::Poll;
use std::thread;

#[derive(Debug)]
pub enum InspectorMsgKind {
  Notification,
  Message(i32),
}

#[derive(Debug)]
pub struct InspectorMsg {
  pub kind: InspectorMsgKind,
  pub content: String,
}

impl InspectorMsg {
  /// Create a notification message from a JSON value
  pub fn notification(content: serde_json::Value) -> Self {
    Self {
      kind: InspectorMsgKind::Notification,
      content: content.to_string(),
    }
  }
}

// TODO(bartlomieju): remove this
pub type SessionProxySender = UnboundedSender<InspectorMsg>;
// TODO(bartlomieju): remove this
pub type SessionProxyReceiver = UnboundedReceiver<String>;

/// Channels for an inspector session
pub enum InspectorSessionChannels {
  /// Regular inspector session with bidirectional communication
  Regular {
    tx: SessionProxySender,
    rx: SessionProxyReceiver,
  },
  /// Worker inspector session with separate channels for main<->worker communication
  Worker {
    /// Main thread sends commands TO worker
    main_to_worker_tx: UnboundedSender<String>,
    /// Worker sends responses/events TO main thread
    worker_to_main_rx: UnboundedReceiver<InspectorMsg>,
    /// Worker URL for identification
    worker_url: String,
  },
}

/// Encapsulates channels and metadata for creating an inspector session
pub struct InspectorSessionProxy {
  pub channels: InspectorSessionChannels,
  pub kind: InspectorSessionKind,
}

/// Creates a pair of inspector session proxies for worker-main communication.
pub fn create_worker_inspector_session_pair(
  worker_url: String,
) -> (InspectorSessionProxy, InspectorSessionProxy) {
  let (worker_to_main_tx, worker_to_main_rx) =
    mpsc::unbounded::<InspectorMsg>();
  let (main_to_worker_tx, main_to_worker_rx) = mpsc::unbounded::<String>();

  let main_side = InspectorSessionProxy {
    channels: InspectorSessionChannels::Worker {
      main_to_worker_tx,
      worker_to_main_rx,
      worker_url,
    },
    kind: InspectorSessionKind::NonBlocking {
      wait_for_disconnect: false,
    },
  };

  let worker_side = InspectorSessionProxy {
    channels: InspectorSessionChannels::Regular {
      tx: worker_to_main_tx,
      rx: main_to_worker_rx,
    },
    kind: InspectorSessionKind::NonBlocking {
      wait_for_disconnect: false,
    },
  };

  (main_side, worker_side)
}

pub type InspectorSessionSend = Box<dyn Fn(InspectorMsg)>;

#[derive(Clone, Copy, Debug)]
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
/// is used for REPL or coverage collection.
pub struct JsRuntimeInspector {
  v8_inspector: Rc<v8::inspector::V8Inspector>,
  new_session_tx: UnboundedSender<InspectorSessionProxy>,
  deregister_tx: RefCell<Option<oneshot::Sender<()>>>,
  state: Rc<JsRuntimeInspectorState>,
}

impl Drop for JsRuntimeInspector {
  fn drop(&mut self) {
    // Since the waker is cloneable, it might outlive the inspector itself.
    // Set the poll state to 'dropped' so it doesn't attempt to request an
    // interrupt from the isolate.
    self
      .state
      .waker
      .update(|w| w.poll_state = PollState::Dropped);

    // V8 automatically deletes all sessions when an `V8Inspector` instance is
    // deleted, however InspectorSession also has a drop handler that cleans
    // up after itself. To avoid a double free, make sure the inspector is
    // dropped last.
    self.state.sessions.borrow_mut().drop_sessions();

    // Notify counterparty that this instance is being destroyed. Ignoring
    // result because counterparty waiting for the signal might have already
    // dropped the other end of channel.
    if let Some(deregister_tx) = self.deregister_tx.borrow_mut().take() {
      let _ = deregister_tx.send(());
    }
  }
}

#[derive(Clone)]
struct JsRuntimeInspectorState {
  isolate_ptr: v8::UnsafeRawIsolatePtr,
  context: v8::Global<v8::Context>,
  flags: Rc<RefCell<InspectorFlags>>,
  waker: Arc<InspectorWaker>,
  sessions: Rc<RefCell<SessionContainer>>,
  is_dispatching_message: Rc<RefCell<bool>>,
  pending_worker_messages: Arc<Mutex<Vec<(String, String)>>>,
  nodeworker_enabled: Rc<Cell<bool>>,
  auto_attach_enabled: Rc<Cell<bool>>,
  discover_targets_enabled: Rc<Cell<bool>>,
}

struct JsRuntimeInspectorClient(Rc<JsRuntimeInspectorState>);

impl v8::inspector::V8InspectorClientImpl for JsRuntimeInspectorClient {
  fn run_message_loop_on_pause(&self, context_group_id: i32) {
    assert_eq!(context_group_id, JsRuntimeInspector::CONTEXT_GROUP_ID);
    self.0.flags.borrow_mut().on_pause = true;
    let _ = self.0.poll_sessions(None);
  }

  fn quit_message_loop_on_pause(&self) {
    self.0.flags.borrow_mut().on_pause = false;
  }

  fn run_if_waiting_for_debugger(&self, context_group_id: i32) {
    assert_eq!(context_group_id, JsRuntimeInspector::CONTEXT_GROUP_ID);
    self.0.flags.borrow_mut().waiting_for_session = false;
  }

  fn ensure_default_context_in_group(
    &self,
    context_group_id: i32,
  ) -> Option<v8::Local<'_, v8::Context>> {
    assert_eq!(context_group_id, JsRuntimeInspector::CONTEXT_GROUP_ID);
    let context = self.0.context.clone();
    let mut isolate =
      unsafe { v8::Isolate::from_raw_isolate_ptr(self.0.isolate_ptr) };
    let isolate = &mut isolate;
    v8::callback_scope!(unsafe scope, isolate);
    let local = v8::Local::new(scope, context);
    Some(unsafe { local.extend_lifetime_unchecked() })
  }

  fn resource_name_to_url(
    &self,
    resource_name: &v8::inspector::StringView,
  ) -> Option<v8::UniquePtr<v8::inspector::StringBuffer>> {
    let resource_name = resource_name.to_string();
    let url = url::Url::from_file_path(resource_name).ok()?;
    let src_view = v8::inspector::StringView::from(url.as_str().as_bytes());
    Some(v8::inspector::StringBuffer::create(src_view))
  }
}

impl JsRuntimeInspectorState {
  #[allow(clippy::result_unit_err)]
  pub fn poll_sessions(
    &self,
    mut invoker_cx: Option<&mut Context>,
  ) -> Result<Poll<()>, ()> {
    // The futures this function uses do not have re-entrant poll() functions.
    // However it is can happen that poll_sessions() gets re-entered, e.g.
    // when an interrupt request is honored while the inspector future is polled
    // by the task executor. We let the caller know by returning some error.
    let Ok(mut sessions) = self.sessions.try_borrow_mut() else {
      return Err(());
    };

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
        if let Some(session) = sessions.handshake.take() {
          let mut fut =
            pump_inspector_session_messages(session.clone()).boxed_local();
          // Only add to established if the future is still pending.
          // If the channel is already closed (e.g., worker terminated quickly),
          // the future may complete immediately on first poll. Pushing a
          // completed future to FuturesUnordered would cause a panic when
          // polled again.
          if fut.poll_unpin(cx).is_pending() {
            sessions.established.push(fut);
          }
          let id = sessions.next_local_id;
          sessions.next_local_id += 1;
          sessions.local.insert(id, session);

          // Track the first session as the main session for Target events
          if sessions.main_session_id.is_none() {
            sessions.main_session_id = Some(id);
          }

          continue;
        }

        // Accept new connections.
        if let Poll::Ready(Some(session_proxy)) =
          sessions.session_rx.poll_next_unpin(cx)
        {
          match session_proxy.channels {
            InspectorSessionChannels::Worker {
              main_to_worker_tx,
              worker_to_main_rx,
              worker_url,
            } => {
              // Get the next local ID for this worker
              let worker_id = sessions.next_local_id;
              sessions.next_local_id += 1;

              sessions.register_worker_session(worker_id, worker_url.clone());

              // Register the worker channels
              sessions.register_worker_channels(
                worker_id,
                main_to_worker_tx,
                worker_to_main_rx,
              );

              // Notify the main session about worker target creation/attachment.
              //
              // discover_targets_enabled: Set to true when the debugger calls
              // Target.setDiscoverTargets. When enabled, the inspector sends
              // Target.targetCreated events for new workers.
              //
              // auto_attach_enabled: Set to true when the debugger calls
              // Target.setAutoAttach. When enabled, the inspector automatically
              // attaches to new workers and sends Target.attachedToTarget events.
              if let Some(main_id) = sessions.main_session_id
                && let Some(main_session) = sessions.local.get(&main_id)
                && let Some(ts) =
                  sessions.target_sessions.get(&format!("{}", worker_id))
              {
                if self.discover_targets_enabled.get() {
                  (main_session.state.send)(InspectorMsg::notification(
                    json!({
                      "method": "Target.targetCreated",
                      "params": { "targetInfo": ts.target_info(false) }
                    }),
                  ));
                }

                if self.auto_attach_enabled.get() {
                  ts.attached.set(true);
                  (main_session.state.send)(InspectorMsg::notification(
                    json!({
                      "method": "Target.attachedToTarget",
                      "params": {
                        "sessionId": ts.session_id,
                        "targetInfo": ts.target_info(true),
                        "waitingForDebugger": false
                      }
                    }),
                  ));
                }
              }

              continue;
            }
            InspectorSessionChannels::Regular { tx, rx } => {
              // Normal session (not a worker)
              let session = InspectorSession::new(
                sessions.v8_inspector.as_ref().unwrap().clone(),
                self.is_dispatching_message.clone(),
                Box::new(move |msg| {
                  let _ = tx.unbounded_send(msg);
                }),
                Some(rx),
                session_proxy.kind,
                self.sessions.clone(),
                self.pending_worker_messages.clone(),
                self.nodeworker_enabled.clone(),
                self.auto_attach_enabled.clone(),
                self.discover_targets_enabled.clone(),
              );

              let prev = sessions.handshake.replace(session);
              assert!(prev.is_none());
              continue;
            }
          }
        }

        // Poll worker message channels - forward messages from workers to main session
        if let Some(main_id) = sessions.main_session_id {
          // Get main session send function before mutably iterating over target_sessions
          let main_session_send =
            sessions.local.get(&main_id).map(|s| s.state.send.clone());

          if let Some(send) = main_session_send {
            let mut has_worker_message = false;
            let mut terminated_workers = Vec::new();

            for target_session in sessions.target_sessions.values() {
              // Skip sessions that haven't had their channels registered
              if !target_session.has_channels() {
                continue;
              }
              match target_session.poll_from_worker(cx) {
                Poll::Ready(Some(msg)) => {
                  // CDP Flattened Session Mode: Add sessionId at top level
                  // Used by Chrome DevTools (when NodeWorker.enable has NOT been called)
                  // VSCode uses NodeWorker.receivedMessageFromWorker instead
                  if !self.nodeworker_enabled.get() {
                    if let Ok(mut parsed) =
                      serde_json::from_str::<serde_json::Value>(&msg.content)
                    {
                      if let Some(obj) = parsed.as_object_mut() {
                        obj.insert(
                          "sessionId".to_string(),
                          json!(target_session.session_id),
                        );
                        let flattened_msg = parsed.to_string();

                        // Send the flattened response with sessionId at top level
                        send(InspectorMsg {
                          kind: msg.kind,
                          content: flattened_msg,
                        });
                      }
                    } else {
                      // Fallback: send as-is if not valid JSON
                      send(msg);
                    }
                  } else {
                    let wrapped_nodeworker = json!({
                      "method": "NodeWorker.receivedMessageFromWorker",
                      "params": {
                        "sessionId": target_session.session_id,
                        "message": msg.content,
                        "workerId": target_session.target_id
                      }
                    });

                    send(InspectorMsg {
                      kind: InspectorMsgKind::Notification,
                      content: wrapped_nodeworker.to_string(),
                    });
                  }

                  has_worker_message = true;
                }
                Poll::Ready(None) => {
                  // Worker channel closed - worker has terminated
                  // Notify debugger based on which protocol is in use
                  if self.nodeworker_enabled.get() {
                    // VSCode / Node.js style
                    send(InspectorMsg::notification(json!({
                      "method": "NodeWorker.detachedFromWorker",
                      "params": {
                        "sessionId": target_session.session_id
                      }
                    })));
                  } else if self.auto_attach_enabled.get()
                    || self.discover_targets_enabled.get()
                  {
                    // Chrome DevTools style
                    send(InspectorMsg::notification(json!({
                      "method": "Target.targetDestroyed",
                      "params": {
                        "targetId": target_session.target_id
                      }
                    })));
                  }
                  terminated_workers.push(target_session.session_id.clone());
                }
                Poll::Pending => {}
              }
            }

            // Clean up terminated worker sessions
            for session_id in terminated_workers {
              sessions.target_sessions.remove(&session_id);
              has_worker_message = true; // Trigger re-poll
            }

            if has_worker_message {
              continue;
            }
          }
        }

        // Poll established sessions.
        match sessions.established.poll_next_unpin(cx) {
          Poll::Ready(Some(())) => {
            continue;
          }
          Poll::Ready(None) => {
            break;
          }
          Poll::Pending => {
            break;
          }
        };
      }

      let should_block = {
        let flags = self.flags.borrow();
        flags.on_pause || flags.waiting_for_session
      };

      // Process any queued NodeWorker messages after polling completes
      // Drain from the thread-safe Mutex queue (doesn't require borrowing sessions)
      let pending_messages: Vec<(String, String)> = {
        let mut queue = self.pending_worker_messages.lock();
        queue.drain(..).collect()
      };

      for (session_id, message) in pending_messages {
        if let Some(target_session) = sessions.target_sessions.get(&session_id)
        {
          target_session.send_to_worker(message);
        }
      }

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
            w.inspector_state_ptr = NonNull::new(self as *const _ as *mut Self);
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
        PollState::Idle => break,            // Yield to task.
        PollState::Polling => continue,      // Poll the session handler again.
        PollState::Parked => thread::park(), // Park the thread.
        _ => unreachable!(),
      };
    }

    Ok(Poll::Pending)
  }
}

impl JsRuntimeInspector {
  /// Currently Deno supports only a single context in `JsRuntime`
  /// and thus it's id is provided as an associated constant.
  const CONTEXT_GROUP_ID: i32 = 1;

  pub fn new(
    isolate_ptr: v8::UnsafeRawIsolatePtr,
    scope: &mut v8::PinScope,
    context: v8::Local<v8::Context>,
    is_main_runtime: bool,
    worker_id: Option<u32>,
  ) -> Rc<Self> {
    let (new_session_tx, new_session_rx) =
      mpsc::unbounded::<InspectorSessionProxy>();

    let waker = InspectorWaker::new(scope.thread_safe_handle());
    let state = Rc::new(JsRuntimeInspectorState {
      waker,
      flags: Default::default(),
      isolate_ptr,
      context: v8::Global::new(scope, context),
      sessions: Rc::new(
        RefCell::new(SessionContainer::temporary_placeholder()),
      ),
      is_dispatching_message: Default::default(),
      pending_worker_messages: Arc::new(Mutex::new(Vec::new())),
      nodeworker_enabled: Rc::new(Cell::new(false)),
      auto_attach_enabled: Rc::new(Cell::new(false)),
      discover_targets_enabled: Rc::new(Cell::new(false)),
    });
    let client = Box::new(JsRuntimeInspectorClient(state.clone()));
    let v8_inspector_client = v8::inspector::V8InspectorClient::new(client);
    let v8_inspector = Rc::new(v8::inspector::V8Inspector::create(
      scope,
      v8_inspector_client,
    ));

    *state.sessions.borrow_mut() =
      SessionContainer::new(v8_inspector.clone(), new_session_rx);

    // Tell the inspector about the main realm.
    let context_name_bytes = if is_main_runtime {
      &b"main realm"[..]
    } else {
      &format!("worker [{}]", worker_id.unwrap_or(1)).into_bytes()
    };

    let context_name = v8::inspector::StringView::from(context_name_bytes);
    // NOTE(bartlomieju): this is what Node.js does and it turns out some
    // debuggers (like VSCode) rely on this information to disconnect after
    // program completes.
    // The auxData structure should match {isDefault: boolean, type: 'default'|'isolated'|'worker'}
    // For Chrome DevTools to properly show workers in the execution context dropdown.
    let aux_data = if is_main_runtime {
      r#"{"isDefault": true, "type": "default"}"#
    } else {
      r#"{"isDefault": false, "type": "worker"}"#
    };
    let aux_data_view = v8::inspector::StringView::from(aux_data.as_bytes());
    v8_inspector.context_created(
      context,
      Self::CONTEXT_GROUP_ID,
      context_name,
      aux_data_view,
    );

    // Poll the session handler so we will get notified whenever there is
    // new incoming debugger activity.
    let _ = state.poll_sessions(None).unwrap();

    Rc::new(Self {
      v8_inspector,
      state,
      new_session_tx,
      deregister_tx: RefCell::new(None),
    })
  }

  pub fn is_dispatching_message(&self) -> bool {
    *self.state.is_dispatching_message.borrow()
  }

  pub fn context_destroyed(
    &self,
    scope: &mut v8::PinScope<'_, '_>,
    context: v8::Global<v8::Context>,
  ) {
    let context = v8::Local::new(scope, context);
    self.v8_inspector.context_destroyed(context);
  }

  pub fn exception_thrown(
    &self,
    scope: &mut v8::PinScope<'_, '_>,
    exception: v8::Local<'_, v8::Value>,
    in_promise: bool,
  ) {
    let context = scope.get_current_context();
    let message = v8::Exception::create_message(scope, exception);
    let stack_trace = message.get_stack_trace(scope);
    let stack_trace = self.v8_inspector.create_stack_trace(stack_trace);
    self.v8_inspector.exception_thrown(
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

  pub fn sessions_state(&self) -> SessionsState {
    self.state.sessions.borrow().sessions_state()
  }

  pub fn poll_sessions_from_event_loop(&self, cx: &mut Context) {
    let _ = self.state.poll_sessions(Some(cx)).unwrap();
  }

  /// This function blocks the thread until at least one inspector client has
  /// established a websocket connection.
  pub fn wait_for_session(&self) {
    loop {
      if let Some(_session) =
        self.state.sessions.borrow_mut().local.values().next()
      {
        self.state.flags.borrow_mut().waiting_for_session = false;
        break;
      } else {
        self.state.flags.borrow_mut().waiting_for_session = true;
        let _ = self.state.poll_sessions(None).unwrap();
      }
    }
  }

  /// This function blocks the thread until at least one inspector client has
  /// established a websocket connection.
  ///
  /// After that, it instructs V8 to pause at the next statement.
  /// Frontend must send "Runtime.runIfWaitingForDebugger" message to resume
  /// execution.
  pub fn wait_for_session_and_break_on_next_statement(&self) {
    loop {
      if let Some(session) =
        self.state.sessions.borrow_mut().local.values().next()
      {
        break session.break_on_next_statement();
      } else {
        self.state.flags.borrow_mut().waiting_for_session = true;
        let _ = self.state.poll_sessions(None).unwrap();
      }
    }
  }

  /// Obtain a sender for proxy channels.
  pub fn get_session_sender(&self) -> UnboundedSender<InspectorSessionProxy> {
    self.new_session_tx.clone()
  }

  /// Create a channel that notifies the frontend when inspector is dropped.
  ///
  /// NOTE: Only a single handler is currently available.
  pub fn add_deregister_handler(&self) -> oneshot::Receiver<()> {
    let maybe_deregister_tx = self.deregister_tx.borrow_mut().take();
    if let Some(deregister_tx) = maybe_deregister_tx
      && !deregister_tx.is_canceled()
    {
      panic!("Inspector deregister handler already exists and is alive.");
    }
    let (tx, rx) = oneshot::channel::<()>();
    self.deregister_tx.borrow_mut().replace(tx);
    rx
  }

  pub fn create_local_session(
    inspector: Rc<JsRuntimeInspector>,
    callback: InspectorSessionSend,
    kind: InspectorSessionKind,
  ) -> LocalInspectorSession {
    let (session_id, sessions) = {
      let sessions = inspector.state.sessions.clone();

      let inspector_session = InspectorSession::new(
        inspector.v8_inspector.clone(),
        inspector.state.is_dispatching_message.clone(),
        callback,
        None,
        kind,
        sessions.clone(),
        inspector.state.pending_worker_messages.clone(),
        inspector.state.nodeworker_enabled.clone(),
        inspector.state.auto_attach_enabled.clone(),
        inspector.state.discover_targets_enabled.clone(),
      );

      let session_id = {
        let mut s = sessions.borrow_mut();
        let id = s.next_local_id;
        s.next_local_id += 1;
        assert!(s.local.insert(id, inspector_session).is_none());
        id
      };

      take(&mut inspector.state.flags.borrow_mut().waiting_for_session);
      (session_id, sessions)
    };

    LocalInspectorSession::new(session_id, sessions)
  }
}

#[derive(Default)]
struct InspectorFlags {
  waiting_for_session: bool,
  on_pause: bool,
}

#[derive(Debug)]
pub struct SessionsState {
  pub has_active: bool,
  pub has_blocking: bool,
  pub has_nonblocking: bool,
  pub has_nonblocking_wait_for_disconnect: bool,
}

/// A helper structure that helps coordinate sessions during different
/// parts of their lifecycle.
pub struct SessionContainer {
  v8_inspector: Option<Rc<v8::inspector::V8Inspector>>,
  session_rx: UnboundedReceiver<InspectorSessionProxy>,
  handshake: Option<Rc<InspectorSession>>,
  established: FuturesUnordered<InspectorSessionPumpMessages>,
  next_local_id: i32,
  local: HashMap<i32, Rc<InspectorSession>>,

  target_sessions: HashMap<String, Rc<TargetSession>>, // sessionId -> TargetSession
  main_session_id: Option<i32>, // The first session that should receive Target events
  next_worker_id: u32, // Sequential ID for worker display naming (1, 2, 3, ...)
}

struct MainWorkerChannels {
  main_to_worker_tx: UnboundedSender<String>,
  worker_to_main_rx: UnboundedReceiver<InspectorMsg>,
}

/// Represents a CDP Target session (e.g., a worker)
struct TargetSession {
  target_id: String,
  session_id: String,
  local_session_id: i32,
  /// Sequential worker ID for display (1, 2, 3, ...) - independent from session IDs
  worker_id: u32,
  main_worker_channels: RefCell<Option<MainWorkerChannels>>,
  url: String,
  /// Track if we've already sent attachedToTarget for this session
  attached: Cell<bool>,
}

impl TargetSession {
  /// Get a display title for the worker using Node.js style naming
  /// e.g., "worker [1]", "worker [2]"
  fn title(&self) -> String {
    format!("worker [{}]", self.worker_id)
  }

  /// Send a message to the worker (main → worker direction)
  fn send_to_worker(&self, message: String) {
    if let Some(channels) = self.main_worker_channels.borrow().as_ref() {
      let _ = channels.main_to_worker_tx.unbounded_send(message);
    }
  }

  /// Returns true if worker channels have been registered
  fn has_channels(&self) -> bool {
    self.main_worker_channels.borrow().is_some()
  }

  /// Poll for messages from the worker (worker → main direction).
  /// Panics if channels have not been registered yet - caller should
  /// check has_channels() first.
  fn poll_from_worker(&self, cx: &mut Context) -> Poll<Option<InspectorMsg>> {
    self
      .main_worker_channels
      .borrow_mut()
      .as_mut()
      .expect("poll_from_worker called before channels were registered")
      .worker_to_main_rx
      .poll_next_unpin(cx)
  }
}

impl SessionContainer {
  fn new(
    v8_inspector: Rc<v8::inspector::V8Inspector>,
    new_session_rx: UnboundedReceiver<InspectorSessionProxy>,
  ) -> Self {
    Self {
      v8_inspector: Some(v8_inspector),
      session_rx: new_session_rx,
      handshake: None,
      established: FuturesUnordered::new(),
      next_local_id: 1,
      local: HashMap::new(),

      target_sessions: HashMap::new(),
      main_session_id: None,
      next_worker_id: 1, // Workers are numbered starting from 1
    }
  }

  /// V8 automatically deletes all sessions when an `V8Inspector` instance is
  /// deleted, however InspectorSession also has a drop handler that cleans
  /// up after itself. To avoid a double free, we need to manually drop
  /// all sessions before dropping the inspector instance.
  fn drop_sessions(&mut self) {
    self.v8_inspector = Default::default();
    self.handshake.take();
    self.established.clear();
    self.local.clear();
  }

  fn sessions_state(&self) -> SessionsState {
    SessionsState {
      has_active: !self.established.is_empty()
        || self.handshake.is_some()
        || !self.local.is_empty(),
      has_blocking: self
        .local
        .values()
        .any(|s| matches!(s.state.kind, InspectorSessionKind::Blocking)),
      has_nonblocking: self.local.values().any(|s| {
        matches!(s.state.kind, InspectorSessionKind::NonBlocking { .. })
      }),
      has_nonblocking_wait_for_disconnect: self.local.values().any(|s| {
        matches!(
          s.state.kind,
          InspectorSessionKind::NonBlocking {
            wait_for_disconnect: true
          }
        )
      }),
    }
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
      handshake: None,
      established: FuturesUnordered::new(),
      next_local_id: 1,
      local: HashMap::new(),

      target_sessions: HashMap::new(),
      main_session_id: None,
      next_worker_id: 1,
    }
  }

  pub fn dispatch_message_from_frontend(
    &mut self,
    session_id: i32,
    message: String,
  ) {
    let session = self.local.get(&session_id).unwrap();
    session.dispatch_message(message);
  }

  /// Register a worker session and return the assigned worker ID
  fn register_worker_session(
    &mut self,
    local_session_id: i32,
    worker_url: String,
  ) -> u32 {
    // Assign a sequential worker ID for display purposes
    let worker_id = self.next_worker_id;
    self.next_worker_id += 1;

    // Use the local_session_id for internal session routing
    let target_id = format!("{}", local_session_id);
    let session_id = format!("{}", local_session_id);

    let target_session = Rc::new(TargetSession {
      target_id: target_id.clone(),
      session_id: session_id.clone(),
      local_session_id,
      worker_id,
      main_worker_channels: RefCell::new(None),
      url: worker_url.clone(),
      attached: Cell::new(false),
    });
    self
      .target_sessions
      .insert(session_id.clone(), target_session.clone());

    worker_id
  }

  /// Register the communication channels for a worker's V8 inspector
  /// This is called from the worker side to establish bidirectional communication
  pub fn register_worker_channels(
    &mut self,
    local_session_id: i32,
    main_to_worker_tx: UnboundedSender<String>,
    worker_to_main_rx: UnboundedReceiver<InspectorMsg>,
  ) -> bool {
    // Find the target session for this local session ID
    for target_session in self.target_sessions.values() {
      if target_session.local_session_id == local_session_id {
        *target_session.main_worker_channels.borrow_mut() =
          Some(MainWorkerChannels {
            main_to_worker_tx,
            worker_to_main_rx,
          });
        return true;
      }
    }
    false
  }
}

struct InspectorWakerInner {
  poll_state: PollState,
  task_waker: Option<task::Waker>,
  parked_thread: Option<thread::Thread>,
  inspector_state_ptr: Option<NonNull<JsRuntimeInspectorState>>,
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
      parked_thread: None,
      inspector_state_ptr: None,
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
            .inspector_state_ptr
            .take()
            .map(|ptr| ptr.as_ptr() as *mut c_void)
          {
            w.isolate_handle.request_interrupt(handle_interrupt, arg);
          }
          unsafe extern "C" fn handle_interrupt(
            _isolate: v8::UnsafeRawIsolatePtr,
            arg: *mut c_void,
          ) {
            // SAFETY: `InspectorWaker` is owned by `JsRuntimeInspector`, so the
            // pointer to the latter is valid as long as waker is alive.
            let inspector_state =
              unsafe { &*(arg as *mut JsRuntimeInspectorState) };
            let _ = inspector_state.poll_sessions(None);
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

#[derive(Clone, Copy, Debug)]
pub enum InspectorSessionKind {
  Blocking,
  NonBlocking { wait_for_disconnect: bool },
}

#[derive(Clone)]
struct InspectorSessionState {
  is_dispatching_message: Rc<RefCell<bool>>,
  send: Rc<InspectorSessionSend>,
  rx: Rc<RefCell<Option<SessionProxyReceiver>>>,
  // Describes if session should keep event loop alive, eg. a local REPL
  // session should keep event loop alive, but a Websocket session shouldn't.
  kind: InspectorSessionKind,
  sessions: Rc<RefCell<SessionContainer>>,
  // Thread-safe queue for NodeWorker messages that need to be sent to workers
  pending_worker_messages: Arc<Mutex<Vec<(String, String)>>>,
  // Track whether NodeWorker.enable has been called (enables VSCode-style worker debugging)
  nodeworker_enabled: Rc<Cell<bool>>,
  // Track whether Target.setAutoAttach has been called (enables worker auto-attach)
  auto_attach_enabled: Rc<Cell<bool>>,
  // Track whether Target.setDiscoverTargets has been called (enables target discovery)
  discover_targets_enabled: Rc<Cell<bool>>,
}

/// An inspector session that proxies messages to concrete "transport layer",
/// eg. Websocket or another set of channels.
struct InspectorSession {
  v8_session: v8::inspector::V8InspectorSession,
  state: InspectorSessionState,
}

impl InspectorSession {
  const CONTEXT_GROUP_ID: i32 = 1;

  #[allow(clippy::too_many_arguments)]
  pub fn new(
    v8_inspector: Rc<v8::inspector::V8Inspector>,
    is_dispatching_message: Rc<RefCell<bool>>,
    send: InspectorSessionSend,
    rx: Option<SessionProxyReceiver>,
    kind: InspectorSessionKind,
    sessions: Rc<RefCell<SessionContainer>>,
    pending_worker_messages: Arc<Mutex<Vec<(String, String)>>>,
    nodeworker_enabled: Rc<Cell<bool>>,
    auto_attach_enabled: Rc<Cell<bool>>,
    discover_targets_enabled: Rc<Cell<bool>>,
  ) -> Rc<Self> {
    let state = InspectorSessionState {
      is_dispatching_message,
      send: Rc::new(send),
      rx: Rc::new(RefCell::new(rx)),
      kind,
      sessions,
      pending_worker_messages,
      nodeworker_enabled,
      auto_attach_enabled,
      discover_targets_enabled,
    };

    let v8_session = v8_inspector.connect(
      Self::CONTEXT_GROUP_ID,
      v8::inspector::Channel::new(Box::new(state.clone())),
      v8::inspector::StringView::empty(),
      v8::inspector::V8InspectorClientTrustLevel::FullyTrusted,
    );

    Rc::new(Self { v8_session, state })
  }

  // Dispatch message to V8 session
  fn dispatch_message(&self, msg: String) {
    *self.state.is_dispatching_message.borrow_mut() = true;
    let msg = v8::inspector::StringView::from(msg.as_bytes());
    self.v8_session.dispatch_protocol_message(msg);
    *self.state.is_dispatching_message.borrow_mut() = false;
  }

  pub fn break_on_next_statement(&self) {
    let reason = v8::inspector::StringView::from(&b"debugCommand"[..]);
    let detail = v8::inspector::StringView::empty();
    self
      .v8_session
      .schedule_pause_on_next_statement(reason, detail);
  }

  /// Queue a message to be sent to a worker
  fn queue_worker_message(&self, session_id: &str, message: String) {
    self
      .state
      .pending_worker_messages
      .lock()
      .push((session_id.to_string(), message));
  }

  /// Notify all registered workers via a callback
  fn notify_workers<F>(&self, mut f: F)
  where
    F: FnMut(&TargetSession, &dyn Fn(InspectorMsg)) + 'static,
  {
    let sessions = self.state.sessions.clone();
    let send = self.state.send.clone();
    deno_core::unsync::spawn(async move {
      let sessions = sessions.borrow();
      for ts in sessions.target_sessions.values() {
        f(ts, &|msg| send(msg));
      }
    });
  }
}

impl InspectorSessionState {
  fn send_message(
    &self,
    msg_kind: InspectorMsgKind,
    msg: v8::UniquePtr<v8::inspector::StringBuffer>,
  ) {
    let msg = msg.unwrap().string().to_string();
    (self.send)(InspectorMsg {
      kind: msg_kind,
      content: msg,
    });
  }
}

impl v8::inspector::ChannelImpl for InspectorSessionState {
  fn send_response(
    &self,
    call_id: i32,
    message: v8::UniquePtr<v8::inspector::StringBuffer>,
  ) {
    self.send_message(InspectorMsgKind::Message(call_id), message);
  }

  fn send_notification(
    &self,
    message: v8::UniquePtr<v8::inspector::StringBuffer>,
  ) {
    self.send_message(InspectorMsgKind::Notification, message);
  }

  fn flush_protocol_notifications(&self) {}
}
type InspectorSessionPumpMessages = Pin<Box<dyn Future<Output = ()>>>;
/// Helper to extract a string param from CDP params
fn get_str_param(params: &Option<serde_json::Value>, key: &str) -> String {
  params
    .as_ref()
    .and_then(|p| p.get(key))
    .and_then(|v| v.as_str())
    .unwrap_or_default()
    .to_owned()
}

/// Helper to extract a bool param from CDP params
fn get_bool_param(params: &Option<serde_json::Value>, key: &str) -> bool {
  params
    .as_ref()
    .and_then(|p| p.get(key))
    .and_then(|v| v.as_bool())
    .unwrap_or(false)
}

impl TargetSession {
  /// Build target info JSON for CDP events
  fn target_info(&self, attached: bool) -> serde_json::Value {
    json!({
      "targetId": self.target_id,
      "type": "node_worker",
      "title": self.title(),
      "url": self.url,
      "attached": attached,
      "canAccessOpener": true
    })
  }

  /// Build worker info JSON for NodeWorker events
  fn worker_info(&self) -> serde_json::Value {
    json!({
      "workerId": self.target_id,
      "type": "node_worker",
      "title": self.title(),
      "url": self.url
    })
  }
}

async fn pump_inspector_session_messages(session: Rc<InspectorSession>) {
  let mut rx = session.state.rx.borrow_mut().take().unwrap();

  while let Some(msg) = rx.next().await {
    let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&msg) else {
      session.dispatch_message(msg);
      continue;
    };

    // CDP Flattened Session Mode: route messages with top-level sessionId to workers
    if let Some(session_id) = parsed.get("sessionId").and_then(|s| s.as_str()) {
      let mut worker_msg = parsed.clone();
      if let Some(obj) = worker_msg.as_object_mut() {
        obj.remove("sessionId");
        session.queue_worker_message(session_id, worker_msg.to_string());
      }
      continue;
    }

    let Some(method) = parsed.get("method").and_then(|m| m.as_str()) else {
      session.dispatch_message(msg);
      continue;
    };

    let params = parsed.get("params").cloned();
    let msg_id = parsed.get("id").cloned();

    match method {
      "NodeWorker.enable" => {
        session.state.nodeworker_enabled.set(true);
        session.notify_workers(|ts, send| {
          send(InspectorMsg::notification(json!({
            "method": "NodeWorker.attachedToWorker",
            "params": {
              "sessionId": ts.session_id,
              "workerInfo": ts.worker_info(),
              "waitingForDebugger": false
            }
          })));
        });
      }
      "NodeWorker.sendMessageToWorker" | "Target.sendMessageToTarget" => {
        session.queue_worker_message(
          &get_str_param(&params, "sessionId"),
          get_str_param(&params, "message"),
        );
      }
      "Target.setDiscoverTargets" => {
        let discover = get_bool_param(&params, "discover");
        session.state.discover_targets_enabled.set(discover);

        if discover {
          session.notify_workers(|ts, send| {
            send(InspectorMsg::notification(json!({
              "method": "Target.targetCreated",
              "params": { "targetInfo": ts.target_info(false) }
            })));
          });
        }
      }
      "Target.setAutoAttach" => {
        let auto_attach = get_bool_param(&params, "autoAttach");
        let send = session.state.send.clone();
        let sessions = session.state.sessions.clone();
        session.state.auto_attach_enabled.set(auto_attach);
        if auto_attach {
          deno_core::unsync::spawn(async move {
            let sessions = sessions.borrow();
            for ts in sessions.target_sessions.values() {
              if ts.attached.replace(true) {
                continue; // Skip if already attached
              }
              send(InspectorMsg::notification(json!({
                "method": "Target.attachedToTarget",
                "params": {
                  "sessionId": ts.session_id,
                  "targetInfo": ts.target_info(true),
                  "waitingForDebugger": false
                }
              })));
            }
          });
        }
      }
      _ => {
        session.dispatch_message(msg);
        continue;
      }
    }

    // Send response after handling the command
    if let Some(id) = msg_id {
      let call_id = id.as_i64().unwrap_or(0) as i32;
      (session.state.send)(InspectorMsg {
        kind: InspectorMsgKind::Message(call_id),
        content: json!({
          "id": id,
          "result": {}
        })
        .to_string(),
      });
    }
  }
}

/// A local inspector session that can be used to send and receive protocol messages directly on
/// the same thread as an isolate.
///
/// Does not provide any abstraction over CDP messages.
pub struct LocalInspectorSession {
  sessions: Rc<RefCell<SessionContainer>>,
  session_id: i32,
}

impl LocalInspectorSession {
  pub fn new(session_id: i32, sessions: Rc<RefCell<SessionContainer>>) -> Self {
    Self {
      sessions,
      session_id,
    }
  }

  pub fn dispatch(&mut self, msg: String) {
    self
      .sessions
      .borrow_mut()
      .dispatch_message_from_frontend(self.session_id, msg);
  }

  pub fn post_message<T: serde::Serialize>(
    &mut self,
    id: i32,
    method: &str,
    params: Option<T>,
  ) {
    let message = json!({
        "id": id,
        "method": method,
        "params": params,
    });

    let stringified_msg = serde_json::to_string(&message).unwrap();
    self.dispatch(stringified_msg);
  }
}
