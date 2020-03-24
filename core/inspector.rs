#![allow(unused)]

use rusty_v8 as v8;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};
use v8::inspector::StringBuffer;
use v8::inspector::StringView;
use v8::inspector::V8InspectorClient;
use v8::inspector::{ChannelBase, ChannelImpl};
use v8::inspector::{V8Inspector, V8InspectorSession};
use v8::inspector::{V8InspectorClientBase, V8InspectorClientImpl};
use v8::UniquePtr;
use v8::UniqueRef;
use v8::{TaskBase, TaskImpl};

pub use std::os::raw::c_int as int;

#[derive(Clone)]
pub struct InspectorHandle {
  pub tx: Arc<Mutex<Sender<String>>>,
  pub rx: Arc<Mutex<Receiver<String>>>,
}

impl InspectorHandle {
  pub fn new(tx: Sender<String>, rx: Receiver<String>) -> Self {
    InspectorHandle {
      tx: Arc::new(Mutex::new(tx)),
      rx: Arc::new(Mutex::new(rx)),
    }
  }
}

// Using repr(C) to preserve field ordering and test that everything works
// when the ChannelBase field is not the first element of the struct.
#[repr(C)]
pub struct InspectorFrontend {
  base: ChannelBase,
  // TODO: deno_isolate: Isolate,
}

impl ChannelImpl for InspectorFrontend {
  fn base(&self) -> &ChannelBase {
    &self.base
  }

  fn base_mut(&mut self) -> &mut ChannelBase {
    &mut self.base
  }

  fn send_response(
    &mut self,
    call_id: i32,
    mut message: UniquePtr<StringBuffer>,
  ) {
    // deno_isolate.inspector_message_cb(message)
    todo!()
  }

  fn send_notification(&mut self, mut message: UniquePtr<StringBuffer>) {
    // deno_isolate.inspector_message_cb(message)
    todo!()
  }

  fn flush_protocol_notifications(&mut self) {
    // pass
    todo!()
  }
}

impl InspectorFrontend {
  pub fn new() -> Self {
    Self {
      base: ChannelBase::new::<Self>(),
    }
  }
}

#[repr(C)]
pub struct InspectorClient {
  base: V8InspectorClientBase,
  session: Option<UniqueRef<V8InspectorSession>>,
  frontend: Option<InspectorFrontend>,
  inspector: Option<UniqueRef<V8Inspector>>,
  terminated: bool,
}

impl V8InspectorClientImpl for InspectorClient {
  fn base(&self) -> &V8InspectorClientBase {
    &self.base
  }

  fn base_mut(&mut self) -> &mut V8InspectorClientBase {
    &mut self.base
  }

  fn run_message_loop_on_pause(&mut self, context_group_id: int) {
    // while !self.terminated {
    // self.deno_isolate.inspector_block_recv();
    // }
    todo!()
  }

  fn quit_message_loop_on_pause(&mut self) {
    todo!()
  }

  fn run_if_waiting_for_debugger(&mut self, context_group_id: int) {
    todo!()
  }
}

impl InspectorClient {
  pub fn new<P>(scope: &mut P, context: v8::Local<v8::Context>) -> Self
  where
    P: v8::InIsolate,
  {
    let mut frontend = InspectorFrontend::new();

    let mut client = Self {
      base: V8InspectorClientBase::new::<Self>(),
      session: None,
      inspector: None,
      frontend: None,
      terminated: false,
    };

    let mut inspector = V8Inspector::create(scope, &mut client);
    let context_group_id = 1;
    let empty_view = StringView::empty();
    let mut buffer = StringBuffer::create(&empty_view).unwrap();

    let state = b"";
    let state_view = StringView::from(&state[..]);

    let session =
      inspector.connect(context_group_id, &mut frontend, &state_view);
    // let context_info = V8ContextInfo::new();
    inspector.context_created(context, context_group_id, &state_view);
    client.frontend = Some(frontend);
    client.session = Some(session);
    client.inspector = Some(inspector);
    client
  }

  pub fn get_session(&mut self) -> &mut UniqueRef<V8InspectorSession> {
    self.session.as_mut().unwrap()
  }

  pub fn schedule_pause_on_next_statement(&mut self) {
    eprintln!("pause on next statement");
    let session = self.session.as_mut().unwrap();
    let reason = &"Break on start".to_string().into_bytes()[..];
    let mut string_view = StringView::from(reason);
    let mut string_view2 = StringView::from(reason);
    //let mut reason_buffer = StringBuffer::create(&string_view).unwrap();
    //let mut reason_buffer2 = StringBuffer::create(&string_view).unwrap();
    eprintln!("before break");
    session
      .schedule_pause_on_next_statement(&mut string_view, &mut string_view2);
    eprintln!("after break");
  }
}

pub struct DispatchOnInspectorBackendTask {
  base: TaskBase,
}

impl TaskImpl for DispatchOnInspectorBackendTask {
  fn base(&self) -> &TaskBase {
    &self.base
  }

  fn base_mut(&mut self) -> &mut TaskBase {
    &mut self.base
  }

  fn run(&mut self) {
    todo!()
  }
}

impl DispatchOnInspectorBackendTask {
  pub fn new() -> Self {
    Self {
      base: TaskBase::new::<Self>(),
    }
  }
}
