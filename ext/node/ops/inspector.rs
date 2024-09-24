use deno_core::futures::channel::mpsc;
use deno_core::futures::StreamExt;
use deno_core::op2;
use deno_core::unsync::spawn;
use deno_core::v8;
use deno_core::v8::inspector::ChannelBase;
use deno_core::v8::inspector::ChannelImpl;
use deno_core::v8::inspector::StringBuffer;
use deno_core::v8::inspector::StringView;
use deno_core::v8::inspector::V8Inspector;
use deno_core::v8::inspector::V8InspectorClientBase;
use deno_core::v8::inspector::V8InspectorClientImpl;
use deno_core::v8::inspector::V8InspectorClientTrustLevel;
use deno_core::v8::inspector::V8InspectorSession;
use deno_core::GarbageCollected;
use deno_core::OpState;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::task::Context;
use std::task::Poll;

struct Inspector {
  _client: Box<InspectorClient>,
  inspector: v8::UniqueRef<V8Inspector>,
  sessions: HashMap<u32, InspectorSession>,
}

impl Inspector {
  fn create(isolate: &mut v8::Isolate) -> Self {
    let mut client = Box::new(InspectorClient::new());
    let inspector = V8Inspector::create(isolate, &mut *client);
    Self {
      _client: client,
      inspector,
      sessions: HashMap::new(),
    }
  }

  fn connect(
    &mut self,
    receive: Box<dyn Fn(String)>,
  ) -> mpsc::UnboundedSender<String> {
    let (dispatch_tx, dispatch_rx) = mpsc::unbounded();
    let mut channel = Box::new(InspectorChannel::new(receive));
    let state = StringView::empty();
    let session = self.inspector.connect(
      1,
      &mut *channel,
      state,
      V8InspectorClientTrustLevel::FullyTrusted,
    );
    let session = InspectorSession {
      _channel: channel,
      session,
      dispatch_rx,
    };
    self.sessions.insert(0, session);
    dispatch_tx
  }

  fn poll(&mut self, cx: &mut Context) -> Poll<()> {
    self.sessions.retain(|_, session| loop {
      match session.dispatch_rx.poll_next_unpin(cx) {
        Poll::Ready(Some(message)) => {
          let message = StringView::from(message.as_bytes());
          session.session.dispatch_protocol_message(message);
          continue;
        }
        Poll::Ready(None) => break false,
        Poll::Pending => break true,
      }
    });
    // fixme: need connected waker here
    Poll::Pending
  }

  fn wait_for_session_and_break_on_next_statement(&self) {}

  fn context_created(&mut self, context: v8::Local<v8::Context>) {
    self.inspector.context_created(
      context,
      1,
      StringView::from("main realm".as_bytes()),
      StringView::from(r#"{"isDefault": true}"#.as_bytes()),
    );
  }
}

struct InspectorClient {
  base: V8InspectorClientBase,
}

impl InspectorClient {
  fn new() -> Self {
    InspectorClient {
      base: V8InspectorClientBase::new::<Self>(),
    }
  }
}

impl V8InspectorClientImpl for InspectorClient {
  fn base(&self) -> &V8InspectorClientBase {
    &self.base
  }

  fn base_mut(&mut self) -> &mut V8InspectorClientBase {
    &mut self.base
  }

  unsafe fn base_ptr(this: *const Self) -> *const V8InspectorClientBase {
    &(*this).base
  }

  fn run_message_loop_on_pause(&mut self, _context_group_id: i32) {}
}

struct InspectorChannel {
  base: ChannelBase,
  receive: Box<dyn Fn(String)>,
}

impl InspectorChannel {
  fn new(receive: Box<dyn Fn(String)>) -> Self {
    Self {
      base: ChannelBase::new::<Self>(),
      receive,
    }
  }
}

impl ChannelImpl for InspectorChannel {
  fn base(&self) -> &ChannelBase {
    &self.base
  }

  fn base_mut(&mut self) -> &mut ChannelBase {
    &mut self.base
  }

  unsafe fn base_ptr(this: *const Self) -> *const ChannelBase {
    &(*this).base
  }

  fn send_response(
    &mut self,
    _call_id: i32,
    message: v8::UniquePtr<StringBuffer>,
  ) {
    (self.receive)(message.unwrap().string().to_string());
  }

  fn send_notification(&mut self, message: v8::UniquePtr<StringBuffer>) {
    (self.receive)(message.unwrap().string().to_string());
  }

  fn flush_protocol_notifications(&mut self) {}
}

struct InspectorSession {
  _channel: Box<InspectorChannel>,
  session: v8::UniqueRef<V8InspectorSession>,
  dispatch_rx: mpsc::UnboundedReceiver<String>,
}

#[op2(fast)]
pub fn op_inspector_open() {}

#[op2(fast)]
pub fn op_inspector_close() {}

#[op2(fast)]
pub fn op_inspector_wait(state: &OpState) -> bool {
  match state.try_borrow::<Rc<RefCell<Inspector>>>() {
    Some(inspector) => {
      inspector
        .borrow_mut()
        .wait_for_session_and_break_on_next_statement();
      true
    }
    None => false,
  }
}

struct JSInspectorSession {
  sender: RefCell<Option<mpsc::UnboundedSender<String>>>,
}

impl GarbageCollected for JSInspectorSession {}

#[op2]
#[cppgc]
pub fn op_inspector_connect<'s>(
  isolate: *mut v8::Isolate,
  scope: &mut v8::HandleScope<'s>,
  state: &mut OpState,
  callback: v8::Local<v8::Function>,
  is_main_thread: bool,
) -> JSInspectorSession {
  if is_main_thread {
    // TODO: totally different api
    panic!()
  }

  let context = scope.get_current_context();

  {
    let mut inspector = Inspector::create(scope);

    inspector.context_created(context);

    let inspector = Rc::new(RefCell::new(inspector));
    state.put(inspector.clone());

    let inspector = Rc::downgrade(&inspector);
    spawn(async move {
      std::future::poll_fn(|cx| {
        if let Some(inspector) = inspector.upgrade() {
          let mut inspector = inspector.borrow_mut();
          inspector.poll(cx)
        } else {
          Poll::Ready(())
        }
      })
      .await;
    });
  }

  let callback = v8::Global::new(scope, callback);
  let context = v8::Global::new(scope, context);

  let tx = {
    let mut inspector = state.borrow::<Rc<RefCell<Inspector>>>().borrow_mut();
    inspector.connect(Box::new(move |message| {
      // SAFETY: Inspector only runs when Isolate is alive.
      let isolate = unsafe { &mut *isolate };
      let scope = &mut v8::HandleScope::new(isolate);
      let context = v8::Local::new(scope, context.clone());
      let scope = &mut v8::ContextScope::new(scope, context);
      let callback = v8::Local::new(scope, callback.clone());
      if let Some(message) = v8::String::new(scope, &message) {
        let this = v8::undefined(scope);
        callback.call(scope, this.into(), &[message.into()]);
      }
    }))
  };

  JSInspectorSession {
    sender: RefCell::new(Some(tx)),
  }
}

#[op2(fast)]
pub fn op_inspector_dispatch(
  #[cppgc] session: &JSInspectorSession,
  #[string] message: String,
) {
  if let Some(sender) = &*session.sender.borrow() {
    let _ = sender.unbounded_send(message);
  }
}

#[op2(fast)]
pub fn op_inspector_disconnect(#[cppgc] session: &JSInspectorSession) {
  drop(session.sender.borrow_mut().take());
}
