#![allow(unused)]

use rusty_v8 as v8;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};
use v8::inspector::Client;
use v8::inspector::StringBuffer;
use v8::inspector::StringView;
use v8::inspector::{ChannelBase, ChannelImpl};
use v8::inspector::{ClientBase, ClientImpl};
use v8::inspector::{V8Inspector, V8InspectorSession};
use v8::int;
use v8::platform::{TaskBase, TaskImpl};
use v8::UniquePtr;
use v8::UniqueRef;

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
  base: ClientBase,
  session: Option<UniqueRef<V8InspectorSession>>,
  frontend: Option<InspectorFrontend>,
  inspector: Option<UniqueRef<V8Inspector>>,
  terminated: bool,
}

impl ClientImpl for InspectorClient {
  fn base(&self) -> &ClientBase {
    &self.base
  }

  fn base_mut(&mut self) -> &mut ClientBase {
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
  pub fn new(
    isolate: &mut v8::Isolate,
    context: v8::Local<v8::Context>,
  ) -> Self {
    let mut frontend = InspectorFrontend::new();

    let mut client = Self {
      base: ClientBase::new::<Self>(),
      session: None,
      inspector: None,
      frontend: None,
      terminated: false,
    };

    let mut inspector = V8Inspector::create(isolate, &mut client);
    let context_group_id = 1;
    let empty_view = StringView::empty();
    let mut buffer = StringBuffer::create(&empty_view).unwrap();
    let session =
      inspector.connect(context_group_id, &mut frontend, &mut buffer);
    // let context_info = V8ContextInfo::new();
    inspector.context_created(context, context_group_id, &mut buffer);
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
    let string_view = StringView::from(reason);
    let mut reason_buffer = StringBuffer::create(&string_view).unwrap();
    let mut reason_buffer2 = StringBuffer::create(&string_view).unwrap();
    eprintln!("before break");
    session.schedule_pause_on_next_statement(
      &mut reason_buffer,
      &mut reason_buffer2,
    );
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
