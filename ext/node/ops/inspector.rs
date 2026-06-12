// Copyright 2018-2026 the Deno authors. MIT license.

use std::cell::RefCell;
use std::net::IpAddr;
use std::net::SocketAddr;
use std::net::ToSocketAddrs;
use std::rc::Rc;

use deno_core::GarbageCollected;
use deno_core::InspectorMsg;
use deno_core::InspectorSessionKind;
use deno_core::JsRuntimeInspector;
use deno_core::ModuleSpecifier;
use deno_core::OpState;
use deno_core::op2;
use deno_core::serde_json;
use deno_core::v8;
use deno_inspector_server::InspectPublishUid;
use deno_inspector_server::InspectorServerUrl;
use deno_inspector_server::create_inspector_server;
use deno_inspector_server::stop_inspector_server;
use deno_permissions::PermissionsContainer;

#[op2(fast)]
pub fn op_inspector_enabled(state: &OpState) -> bool {
  // If there's `InspectorServerUrl` then inspector is enabled, this
  // will change once `op_inspector_open` will be implemented
  state.try_borrow::<InspectorServerUrl>().is_some()
}

/// Returns the port the inspector server is listening on, or 0 if the
/// inspector server has not been started. Used to back Node.js'
/// `process.debugPort` so it reflects the actual bound port (which is
/// important when `--inspect=...:0` requests an ephemeral port).
#[op2(fast)]
pub fn op_inspector_port(state: &OpState) -> u32 {
  let Some(url) = state.try_borrow::<InspectorServerUrl>() else {
    return 0;
  };
  // URL looks like `ws://host:port/<uuid>` (or `wss://...`). Parse out
  // the port. We don't depend on the `url` crate here to keep the op
  // tiny; the format is fixed by `get_websocket_debugger_url`.
  let s = url.0.as_str();
  let Some(after_scheme) = s.split_once("://").map(|(_, rest)| rest) else {
    return 0;
  };
  let host_and_port = after_scheme.split('/').next().unwrap_or("");
  let port_str = match host_and_port.rsplit_once(':') {
    Some((_, p)) => p,
    None => return 0,
  };
  port_str.parse::<u16>().map(|p| p as u32).unwrap_or(0)
}

#[op2(stack_trace)]
pub fn op_inspector_open(
  state: &mut OpState,
  port: Option<u16>,
  #[string] host: Option<String>,
  wait_for_session: bool,
) -> Result<(), InspectorOpenError> {
  const DEFAULT_HOST: IpAddr =
    IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1));
  const DEFAULT_PORT: u16 = 9229;

  let host_ip: IpAddr = match &host {
    Some(h) => {
      if let Ok(ip) = h.parse::<IpAddr>() {
        ip
      } else {
        // Resolve hostnames like "localhost" via DNS to match Node's
        // `inspector.open(port, host)` behavior. Prefer IPv4 results.
        let addrs = (h.as_str(), 0u16)
          .to_socket_addrs()
          .map_err(|e| {
            InspectorOpenError::InvalidHost(format!(
              "Invalid inspector host '{}': {}",
              h, e
            ))
          })?
          .collect::<Vec<_>>();
        addrs
          .iter()
          .find(|a| a.is_ipv4())
          .or_else(|| addrs.first())
          .map(|a| a.ip())
          .ok_or_else(|| {
            InspectorOpenError::InvalidHost(format!(
              "Could not resolve inspector host '{}'",
              h
            ))
          })?
      }
    }
    None => DEFAULT_HOST,
  };
  let port = port.unwrap_or(DEFAULT_PORT);
  let addr = SocketAddr::new(host_ip, port);

  state
    .borrow_mut::<PermissionsContainer>()
    .check_net_listen(&(host_ip.to_string(), Some(port)), "inspector.open")?;

  let server =
    create_inspector_server(addr, "deno", InspectPublishUid::default())?;

  let inspector = state.borrow::<Rc<JsRuntimeInspector>>().clone();
  let main_module = state.borrow::<ModuleSpecifier>().to_string();

  let inspector_url =
    server.register_inspector(main_module, inspector, wait_for_session);
  state.put(inspector_url);

  Ok(())
}

#[op2(fast)]
pub fn op_inspector_close(state: &mut OpState) {
  stop_inspector_server();
  state.try_take::<InspectorServerUrl>();
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

#[op2(nofast, reentrant)]
pub fn op_inspector_emit_protocol_event(
  state: Rc<RefCell<OpState>>,
  scope: &mut v8::PinScope<'_, '_>,
  #[string] event_name: String,
  #[string] params: String,
) {
  let inspector = {
    let state = state.borrow();
    state.try_borrow::<Rc<JsRuntimeInspector>>().cloned()
  };
  let Some(inspector) = inspector else {
    return;
  };

  let needs_initiator = event_name == "Network.requestWillBeSent"
    || event_name == "Network.webSocketCreated";
  let needs_has_post_data = event_name == "Network.requestWillBeSent";
  let needs_capture = matches!(
    event_name.as_str(),
    "Network.requestWillBeSent"
      | "Network.responseReceived"
      | "Network.loadingFinished"
      | "Network.loadingFailed"
      | "Network.dataReceived"
      | "Network.dataSent"
  );

  if !needs_initiator && !needs_has_post_data && !needs_capture {
    inspector.broadcast_to_sessions(&event_name, &params);
    return;
  }

  let Ok(mut parsed) = serde_json::from_str::<serde_json::Value>(&params)
  else {
    inspector.broadcast_to_sessions(&event_name, &params);
    return;
  };

  if needs_initiator {
    let initiator = capture_initiator(scope);
    parsed
      .as_object_mut()
      .unwrap()
      .insert("initiator".to_string(), initiator);
  }

  if needs_has_post_data
    && let Some(request) =
      parsed.get_mut("request").and_then(|r| r.as_object_mut())
  {
    request
      .entry("hasPostData")
      .or_insert(serde_json::Value::Bool(false));
  }

  let should_broadcast = if needs_capture {
    inspector.capture_network_event(&event_name, &parsed)
  } else {
    true
  };

  if should_broadcast {
    let augmented = serde_json::to_string(&parsed).unwrap();
    inspector.broadcast_to_sessions(&event_name, &augmented);
  }
}

fn capture_initiator(scope: &mut v8::PinScope<'_, '_>) -> serde_json::Value {
  let Some(stack_trace) = v8::StackTrace::current_stack_trace(scope, 10) else {
    return serde_json::json!({ "type": "other" });
  };

  let frame_count = stack_trace.get_frame_count();
  let mut call_frames = Vec::new();

  for i in 0..frame_count {
    let Some(frame) = stack_trace.get_frame(scope, i) else {
      continue;
    };
    // Skip internal frames (ext:, node: prefixes)
    if let Some(script_name) = frame.get_script_name(scope) {
      let name = script_name.to_rust_string_lossy(scope);
      if name.starts_with("ext:") || name.starts_with("node:") {
        continue;
      }
    }

    let function_name = frame
      .get_function_name(scope)
      .map(|n| n.to_rust_string_lossy(scope))
      .unwrap_or_default();
    let url = frame
      .get_script_name(scope)
      .map(|n| n.to_rust_string_lossy(scope))
      .map(|s| {
        // CJS code sees `__filename` as an OS filesystem path (with
        // backslashes on Windows), not a `file://` URL. Convert here so
        // that test frame lookups like
        // `findFrameInInitiator(__filename, initiator)` match.
        if s.starts_with("file://")
          && let Ok(parsed) = ModuleSpecifier::parse(&s)
          && let Ok(path) = deno_path_util::url_to_file_path(&parsed)
        {
          return path.to_string_lossy().into_owned();
        }
        s
      })
      .unwrap_or_default();
    let line_number = frame.get_line_number().saturating_sub(1); // CDP uses 0-based
    let column_number = frame.get_column().saturating_sub(1);

    call_frames.push(serde_json::json!({
      "functionName": function_name,
      "scriptId": "",
      "url": url,
      "lineNumber": line_number,
      "columnNumber": column_number,
    }));
  }

  // Always emit `type: "script"` even when `call_frames` is empty.
  // Returning `{type: "other"}` for the empty case (as the original code
  // did) tripped node_compat tests that assert `initiator.type ===
  // "script"` whenever the inspector saw a JS-originated request - which
  // is true here by construction, since we only reach this function from
  // `op_inspector_emit_protocol_event` called from JS-land emitters.
  serde_json::json!({
    "type": "script",
    "stack": {
      "callFrames": call_frames,
    },
  })
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
pub enum InspectorOpenError {
  #[class(inherit)]
  #[error(transparent)]
  Permission(
    #[from]
    #[inherit]
    deno_permissions::PermissionCheckError,
  ),
  #[class(inherit)]
  #[error(transparent)]
  Server(
    #[from]
    #[inherit]
    deno_inspector_server::InspectorServerError,
  ),
  #[class(generic)]
  #[error("{0}")]
  InvalidHost(String),
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

/// Like `op_inspector_connect`, but skips the `--allow-sys=inspector`
/// permission check. Only intended for the `node:repl` polyfill, which
/// uses a private inspector session to evaluate the in-flight input with
/// `Runtime.evaluate({ throwOnSideEffect: true })` for the inline preview.
/// The session can only inspect the current runtime, which user code can
/// already do via other JS APIs, so no permission is required.
#[op2]
#[cppgc]
pub fn op_node_repl_inspector_connect<'s>(
  isolate: &v8::Isolate,
  scope: &mut v8::PinScope<'s, '_>,
  state: &mut OpState,
  callback: v8::Local<'s, v8::Function>,
) -> JSInspectorSession {
  let context = scope.get_current_context();
  let context = v8::Global::new(scope, context);
  let callback = v8::Global::new(scope, callback);

  let inspector = state.borrow::<Rc<JsRuntimeInspector>>().clone();

  // SAFETY: just grabbing the raw pointer
  let isolate = unsafe { isolate.as_raw_isolate_ptr() };

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

  JSInspectorSession {
    session: RefCell::new(Some(session)),
  }
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
