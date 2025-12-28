// Copyright 2018-2025 the Deno authors. MIT license.

use std::cell::RefCell;
use std::rc::Rc;

use deno_core::GarbageCollected;
use deno_core::InspectorMsg;
use deno_core::InspectorSessionKind;
use deno_core::JsRuntimeInspector;
use deno_core::OpState;
use deno_core::op2;
use deno_core::v8;
use deno_error::JsErrorBox;
use deno_permissions::PermissionsContainer;

pub struct InspectorServerUrl(pub String);

#[op2(fast)]
pub fn op_inspector_enabled() -> bool {
  // TODO: hook up to InspectorServer
  false
}

#[op2(stack_trace)]
pub fn op_inspector_open(
  _state: &mut OpState,
  _port: Option<u16>,
  #[string] _host: Option<String>,
) -> Result<(), JsErrorBox> {
  // TODO: hook up to InspectorServer
  /*
  let server = state.borrow_mut::<InspectorServer>();
  if let Some(host) = host {
    server.set_host(host);
  }
  if let Some(port) = port {
    server.set_port(port);
  }
  state
    .borrow_mut::<P>()
    .check_net((server.host(), Some(server.port())), "inspector.open")?;
  */

  Ok(())
}

#[op2(fast)]
pub fn op_inspector_close() {
  // TODO: hook up to InspectorServer
}

#[op2]
#[string]
pub fn op_inspector_url(
  state: &mut OpState,
) -> Result<Option<String>, InspectorConnectError> {
  state
    .borrow_mut::<PermissionsContainer>()
    .check_sys("inspector", "inspector.url")?;

  Ok(
    state
      .try_borrow::<InspectorServerUrl>()
      .map(|url| url.0.to_string()),
  )
}

#[op2(fast)]
pub fn op_inspector_wait(state: &OpState) -> bool {
  match state.try_borrow::<Rc<JsRuntimeInspector>>() {
    Some(inspector) => {
      inspector.wait_for_session_and_break_on_next_statement();
      true
    }
    None => false,
  }
}

#[op2(fast)]
pub fn op_inspector_emit_protocol_event(
  #[string] _event_name: String,
  #[string] _params: String,
) {
  // TODO: inspector channel & protocol notifications
}

struct JSInspectorSession {
  session: RefCell<Option<deno_core::LocalInspectorSession>>,
}

// SAFETY: we're sure this can be GCed
unsafe impl GarbageCollected for JSInspectorSession {
  fn trace(&self, _visitor: &mut deno_core::v8::cppgc::Visitor) {}

  fn get_name(&self) -> &'static std::ffi::CStr {
    c"JSInspectorSession"
  }
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum InspectorConnectError {
  #[class(inherit)]
  #[error(transparent)]
  Permission(
    #[from]
    #[inherit]
    deno_permissions::PermissionCheckError,
  ),
  #[class(generic)]
  #[error("connectToMainThread not supported")]
  ConnectToMainThreadUnsupported,
}

#[op2(stack_trace)]
#[cppgc]
pub fn op_inspector_connect<'s>(
  isolate: &v8::Isolate,
  scope: &mut v8::PinScope<'s, '_>,
  state: &mut OpState,
  connect_to_main_thread: bool,
  callback: v8::Local<'s, v8::Function>,
) -> Result<JSInspectorSession, InspectorConnectError> {
  state
    .borrow_mut::<PermissionsContainer>()
    .check_sys("inspector", "inspector.Session.connect")?;

  if connect_to_main_thread {
    return Err(InspectorConnectError::ConnectToMainThreadUnsupported);
  }

  let context = scope.get_current_context();
  let context = v8::Global::new(scope, context);
  let callback = v8::Global::new(scope, callback);

  let inspector = state.borrow::<Rc<JsRuntimeInspector>>().clone();

  // SAFETY: just grabbing the raw pointer
  let isolate = unsafe { isolate.as_raw_isolate_ptr() };

  // The inspector connection does not keep the event loop alive but
  // when the inspector sends a message to the frontend, the JS that
  // that runs may keep the event loop alive so we have to call back
  // synchronously, instead of using the usual LocalInspectorSession
  // UnboundedReceiver<InspectorMsg> API.
  let callback = Box::new(move |message: InspectorMsg| {
    // SAFETY: This function is called directly by the inspector, so
    //   1) The isolate is still valid
    //   2) We are on the same thread as the Isolate
    let mut isolate = unsafe { v8::Isolate::from_raw_isolate_ptr(isolate) };
    v8::callback_scope!(unsafe let scope, &mut isolate);
    let context = v8::Local::new(scope, context.clone());
    let scope = &mut v8::ContextScope::new(scope, context);
    v8::tc_scope!(let scope, scope);
    let recv = v8::undefined(scope);
    if let Some(message) = v8::String::new(scope, &message.content) {
      let callback = v8::Local::new(scope, callback.clone());
      callback.call(scope, recv.into(), &[message.into()]);
    }
  });

  let session = JsRuntimeInspector::create_local_session(
    inspector,
    callback,
    InspectorSessionKind::NonBlocking {
      wait_for_disconnect: false,
    },
  );

  Ok(JSInspectorSession {
    session: RefCell::new(Some(session)),
  })
}

#[op2(fast, reentrant)]
pub fn op_inspector_dispatch(
  #[cppgc] inspector: &JSInspectorSession,
  #[string] message: String,
) {
  if let Some(session) = &mut *inspector.session.borrow_mut() {
    session.dispatch(message);
  }
}

#[op2(fast)]
pub fn op_inspector_disconnect(#[cppgc] inspector: &JSInspectorSession) {
  inspector.session.borrow_mut().take();
}
