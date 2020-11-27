// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use super::config::Config;
use super::diagnostics::DiagnosticCollection;
use super::diagnostics::DiagnosticSource;
use super::diagnostics::DiagnosticVec;
use super::memory_cache::MemoryCache;
use super::task_pool::TaskPool;
use super::tsc;
use super::utils::notification_is;

use crate::js;

use crossbeam_channel::select;
use crossbeam_channel::unbounded;
use crossbeam_channel::Receiver;
use crossbeam_channel::Sender;
use deno_core::JsRuntime;
use deno_core::ModuleSpecifier;
use lsp_server::Message;
use lsp_server::Notification;
use lsp_server::Request;
use lsp_server::RequestId;
use lsp_server::Response;
use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;
use std::sync::RwLock;
use std::time::Instant;

type ReqHandler = fn(&mut ServerState, Response);
type ReqQueue = lsp_server::ReqQueue<(String, Instant), ReqHandler>;

pub enum Event {
  Message(Message),
  Task(Task),
}

impl fmt::Debug for Event {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    let debug_verbose_not =
      |notification: &Notification, f: &mut fmt::Formatter| {
        f.debug_struct("Notification")
          .field("method", &notification.method)
          .finish()
      };

    match self {
      Event::Message(Message::Notification(notification)) => {
        if notification_is::<lsp_types::notification::DidOpenTextDocument>(
          notification,
        ) || notification_is::<lsp_types::notification::DidChangeTextDocument>(
          notification,
        ) {
          return debug_verbose_not(notification, f);
        }
      }
      Event::Task(Task::Response(response)) => {
        return f
          .debug_struct("Response")
          .field("id", &response.id)
          .field("error", &response.error)
          .finish();
      }
      _ => (),
    }
    match self {
      Event::Message(it) => fmt::Debug::fmt(it, f),
      Event::Task(it) => fmt::Debug::fmt(it, f),
    }
  }
}

pub struct Handle<H, C> {
  pub handle: H,
  pub receiver: C,
}

#[allow(unused)]
#[derive(Eq, PartialEq, Copy, Clone)]
pub enum Status {
  Loading,
  Ready,
  Invalid,
  NeedsReload,
}

impl Default for Status {
  fn default() -> Self {
    Status::Loading
  }
}

#[derive(Debug)]
pub enum Task {
  Diagnostics((DiagnosticSource, DiagnosticVec)),
  Response(Response),
}

#[derive(Debug, Clone)]
pub struct DocumentData {
  pub version: Option<i32>,
}

impl DocumentData {
  pub fn new(version: i32) -> Self {
    DocumentData {
      version: Some(version),
    }
  }
}

/// An immutable snapshot of the server state at a point in time.
#[derive(Debug, Clone, Default)]
pub struct ServerStateSnapshot {
  pub config: Config,
  pub diagnostics: DiagnosticCollection,
  pub doc_data: HashMap<ModuleSpecifier, DocumentData>,
  pub file_cache: Arc<RwLock<MemoryCache>>,
}

pub struct ServerState {
  pub config: Config,
  pub diagnostics: DiagnosticCollection,
  pub doc_data: HashMap<ModuleSpecifier, DocumentData>,
  pub file_cache: Arc<RwLock<MemoryCache>>,
  req_queue: ReqQueue,
  sender: Sender<Message>,
  pub shutdown_requested: bool,
  pub status: Status,
  pub tasks: Handle<TaskPool<Task>, Receiver<Task>>,
  pub ts_runtime: JsRuntime,
}

impl ServerState {
  pub fn new(sender: Sender<Message>, config: Config) -> Self {
    let tasks = {
      let (sender, receiver) = unbounded();
      let handle = TaskPool::new(sender);
      Handle { handle, receiver }
    };
    let ts_runtime = tsc::start(js::compiler_isolate_init(), false)
      .expect("could not start tsc");

    Self {
      config,
      diagnostics: Default::default(),
      doc_data: HashMap::new(),
      file_cache: Arc::new(RwLock::new(Default::default())),
      req_queue: Default::default(),
      sender,
      shutdown_requested: false,
      status: Default::default(),
      tasks,
      ts_runtime,
    }
  }

  pub fn cancel(&mut self, request_id: RequestId) {
    if let Some(response) = self.req_queue.incoming.cancel(request_id) {
      self.send(response.into());
    }
  }

  pub fn complete_request(&mut self, response: Response) {
    let handler = self.req_queue.outgoing.complete(response.id.clone());
    handler(self, response)
  }

  pub fn next_event(&self, inbox: &Receiver<Message>) -> Option<Event> {
    select! {
      recv(inbox) -> msg => msg.ok().map(Event::Message),
      recv(self.tasks.receiver) -> task => Some(Event::Task(task.unwrap())),
    }
  }

  /// Handle any changes and return a `bool` that indicates if there were
  /// important changes to the state.
  pub fn process_changes(&mut self) -> bool {
    let mut file_cache = self.file_cache.write().unwrap();
    let changed_files = file_cache.take_changes();
    // other processing of changed files should be done here as needed
    !changed_files.is_empty()
  }

  pub fn register_request(&mut self, request: &Request, received: Instant) {
    self
      .req_queue
      .incoming
      .register(request.id.clone(), (request.method.clone(), received));
  }

  pub fn respond(&mut self, response: Response) {
    if let Some((_, _)) = self.req_queue.incoming.complete(response.id.clone())
    {
      self.send(response.into());
    }
  }

  fn send(&mut self, message: Message) {
    self.sender.send(message).unwrap()
  }

  pub fn send_notification<N: lsp_types::notification::Notification>(
    &mut self,
    params: N::Params,
  ) {
    let notification = Notification::new(N::METHOD.to_string(), params);
    self.send(notification.into());
  }

  pub fn send_request<R: lsp_types::request::Request>(
    &mut self,
    params: R::Params,
    handler: ReqHandler,
  ) {
    let request =
      self
        .req_queue
        .outgoing
        .register(R::METHOD.to_string(), params, handler);
    self.send(request.into());
  }

  pub fn snapshot(&self) -> ServerStateSnapshot {
    ServerStateSnapshot {
      config: self.config.clone(),
      diagnostics: self.diagnostics.clone(),
      doc_data: self.doc_data.clone(),
      file_cache: Arc::clone(&self.file_cache),
    }
  }

  pub fn transition(&mut self, new_status: Status) {
    self.status = new_status;
  }
}
