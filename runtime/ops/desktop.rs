// Copyright 2018-2026 the Deno authors. MIT license.

//! Desktop window management ops for `deno compile --desktop`.
//!
//! These ops are included in the V8 snapshot so their external references
//! are stable. When `DesktopApi` is not present in OpState (non-desktop
//! builds), the ops silently no-op.

use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::OnceLock;
use std::sync::atomic::AtomicU32;
use std::sync::atomic::Ordering;

use deno_core::FromV8;
use deno_core::OpState;
use deno_core::ToV8;
use deno_core::cppgc::SameObject;
use deno_core::op2;
use deno_core::serde_json;
use deno_core::v8;

/// Thread-safe intermediate value type for crossing the WEF ↔ Deno boundary.
/// Converts directly to V8 values without going through serde.
pub enum DesktopValue {
  Null,
  Bool(bool),
  Int(i32),
  Double(f64),
  String(String),
  List(Vec<DesktopValue>),
  Dict(Vec<(String, DesktopValue)>),
  Binary(Vec<u8>),
}

impl<'a> ToV8<'a> for DesktopValue {
  type Error = std::convert::Infallible;

  fn to_v8(
    self,
    scope: &mut v8::PinScope<'a, '_>,
  ) -> Result<v8::Local<'a, v8::Value>, Self::Error> {
    Ok(match self {
      DesktopValue::Null => v8::null(scope).into(),
      DesktopValue::Bool(b) => v8::Boolean::new(scope, b).into(),
      DesktopValue::Int(i) => v8::Integer::new(scope, i).into(),
      DesktopValue::Double(d) => v8::Number::new(scope, d).into(),
      DesktopValue::String(s) => v8::String::new(scope, &s).unwrap().into(),
      DesktopValue::List(l) => {
        let arr = v8::Array::new(scope, l.len() as i32);
        for (i, v) in l.into_iter().enumerate() {
          let val = v.to_v8(scope)?;
          arr.set_index(scope, i as u32, val);
        }
        arr.into()
      }
      DesktopValue::Dict(d) => {
        let obj = v8::Object::new(scope);
        for (k, v) in d {
          let key: v8::Local<v8::Value> =
            v8::String::new(scope, &k).unwrap().into();
          let val = v.to_v8(scope)?;
          obj.set(scope, key, val);
        }
        obj.into()
      }
      DesktopValue::Binary(b) => {
        let len = b.len();
        let store = v8::ArrayBuffer::new_backing_store_from_vec(b);
        let ab = v8::ArrayBuffer::with_backing_store(scope, &store.into());
        v8::Uint8Array::new(scope, ab, 0, len).unwrap().into()
      }
    })
  }
}

/// Wraps a `Result<DesktopValue, DesktopValue>` from `execute_js`.
/// Converts to `{ ok: true, value }` or `{ ok: false, value }`.
pub struct ExecuteJsResult(pub Result<DesktopValue, DesktopValue>);

impl<'a> ToV8<'a> for ExecuteJsResult {
  type Error = std::convert::Infallible;

  fn to_v8(
    self,
    scope: &mut v8::PinScope<'a, '_>,
  ) -> Result<v8::Local<'a, v8::Value>, Self::Error> {
    let obj = v8::Object::new(scope);

    let ok_key: v8::Local<v8::Value> =
      v8::String::new(scope, "ok").unwrap().into();
    let value_key: v8::Local<v8::Value> =
      v8::String::new(scope, "value").unwrap().into();

    let (ok, val) = match self.0 {
      Ok(v) => (true, v.to_v8(scope)?),
      Err(v) => (false, v.to_v8(scope)?),
    };

    obj.set(scope, ok_key, v8::Boolean::new(scope, ok).into());
    obj.set(scope, value_key, val);
    Ok(obj.into())
  }
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenDevtoolsOptions {
  pub renderer: Option<bool>,
  pub deno: Option<bool>,
}

/// A single event type that flows from the WEF backend to the Deno runtime.
#[derive(Debug, serde::Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum DesktopEvent {
  #[serde(rename_all = "camelCase")]
  AppMenuClick { window_id: u32, id: String },
  #[serde(rename_all = "camelCase")]
  ContextMenuClick { window_id: u32, id: String },
  #[serde(rename_all = "camelCase")]
  KeyboardEvent {
    window_id: u32,
    r#type: String,
    key: String,
    code: String,
    shift: bool,
    control: bool,
    alt: bool,
    meta: bool,
    repeat: bool,
  },
  #[serde(rename_all = "camelCase")]
  BindCall {
    window_id: u32,
    name: String,
    args: serde_json::Value,
    call_id: u32,
  },
  #[serde(rename_all = "camelCase")]
  MouseClick {
    window_id: u32,
    state: String,
    button: i32,
    client_x: f64,
    client_y: f64,
    shift: bool,
    control: bool,
    alt: bool,
    meta: bool,
    click_count: i32,
  },
  #[serde(rename_all = "camelCase")]
  MouseMove {
    window_id: u32,
    client_x: f64,
    client_y: f64,
    shift: bool,
    control: bool,
    alt: bool,
    meta: bool,
  },
  #[serde(rename_all = "camelCase")]
  Wheel {
    window_id: u32,
    delta_x: f64,
    delta_y: f64,
    delta_mode: i32,
    client_x: f64,
    client_y: f64,
    shift: bool,
    control: bool,
    alt: bool,
    meta: bool,
  },
  #[serde(rename_all = "camelCase")]
  CursorEnterLeave {
    window_id: u32,
    entered: bool,
    client_x: f64,
    client_y: f64,
    shift: bool,
    control: bool,
    alt: bool,
    meta: bool,
  },
  #[serde(rename_all = "camelCase")]
  FocusChanged { window_id: u32, focused: bool },
  #[serde(rename_all = "camelCase")]
  WindowResize {
    window_id: u32,
    width: i32,
    height: i32,
  },
  #[serde(rename_all = "camelCase")]
  WindowMove { window_id: u32, x: i32, y: i32 },
  #[serde(rename_all = "camelCase")]
  CloseRequested { window_id: u32 },
  #[serde(rename_all = "camelCase")]
  RuntimeError {
    message: String,
    stack: Option<String>,
  },
}

pub struct DesktopEventReceiver(
  pub Arc<tokio::sync::Mutex<tokio::sync::mpsc::UnboundedReceiver<DesktopEvent>>>,
);
pub struct DesktopEventSender(
  pub tokio::sync::mpsc::UnboundedSender<DesktopEvent>,
);

pub fn create_desktop_event_channel()
-> (DesktopEventSender, DesktopEventReceiver) {
  let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
  (DesktopEventSender(tx), DesktopEventReceiver(Arc::new(tokio::sync::Mutex::new(rx))))
}

/// A pending call from the webview to a bound Deno function.
pub struct PendingBindCall {
  pub name: String,
  pub args: serde_json::Value,
  pub response: tokio::sync::oneshot::Sender<Result<serde_json::Value, String>>,
}

#[derive(Clone)]
pub struct PendingBindResponses(
  pub  Arc<
    std::sync::Mutex<
      HashMap<
        u32,
        tokio::sync::oneshot::Sender<Result<serde_json::Value, String>>,
      >,
    >,
  >,
);

impl PendingBindResponses {
  pub fn new() -> Self {
    Self(Arc::new(std::sync::Mutex::new(HashMap::new())))
  }
}

static BIND_CALL_COUNTER: AtomicU32 = AtomicU32::new(1);

/// Assign a call_id for a bind call and register its response sender.
/// Returns the call_id to embed in the `DesktopEvent::BindCall`.
pub fn register_bind_call(
  responses: &PendingBindResponses,
  response: tokio::sync::oneshot::Sender<Result<serde_json::Value, String>>,
) -> u32 {
  let call_id = BIND_CALL_COUNTER.fetch_add(1, Ordering::Relaxed);
  responses.0.lock().unwrap().insert(call_id, response);
  call_id
}

/// Trait for desktop window operations. Implemented by the desktop
/// runtime (denort_desktop) to bridge to the WEF backend.
///
/// All per-window methods take a `window_id` identifying the target window.
pub trait DesktopApi: Send + Sync + 'static {
  /// Create a new window with the given dimensions and return its ID.
  fn create_window(&self, width: i32, height: i32) -> u32;
  /// Close a specific window.
  fn close_window(&self, window_id: u32);
  /// Returns true if the given window has been closed (either via
  /// `close_window` or because the OS window was destroyed).
  fn is_closed(&self, window_id: u32) -> bool;

  fn set_title(&self, window_id: u32, title: &str);

  fn get_window_size(&self, window_id: u32) -> (i32, i32);
  fn set_window_size(&self, window_id: u32, width: i32, height: i32);

  fn get_window_position(&self, window_id: u32) -> (i32, i32);
  fn set_window_position(&self, window_id: u32, x: i32, y: i32);

  fn is_resizable(&self, window_id: u32) -> bool;
  fn set_resizable(&self, window_id: u32, resizable: bool);

  fn is_always_on_top(&self, window_id: u32) -> bool;
  fn set_always_on_top(&self, window_id: u32, always_on_top: bool);
  fn is_visible(&self, window_id: u32) -> bool;
  fn show(&self, window_id: u32);
  fn hide(&self, window_id: u32);
  fn focus(&self, window_id: u32);

  fn bind(&self, window_id: u32, name: &str);
  fn unbind(&self, window_id: u32, name: &str);

  fn navigate(&self, window_id: u32, url: &str);
  fn quit(&self);
  fn set_application_menu(&self, window_id: u32, menu: Vec<MenuItem>);
  fn show_context_menu(
    &self,
    window_id: u32,
    x: i32,
    y: i32,
    menu: Vec<MenuItem>,
  );

  /// Returns the raw window and display handles for the given window.
  fn get_raw_window_handle(
    &self,
    window_id: u32,
  ) -> (
    raw_window_handle::RawWindowHandle,
    raw_window_handle::RawDisplayHandle,
  );

  fn open_devtools(&self, window_id: u32, renderer: bool, deno: bool);

  fn execute_js(
    &self,
    window_id: u32,
    script: &str,
    callback: Box<
      dyn FnOnce(Result<DesktopValue, DesktopValue>) + Send + 'static,
    >,
  );

  fn alert(&self, title: &str, message: &str);
  fn confirm(
    &self,
    title: &str,
    message: &str,
    callback: Box<dyn FnOnce(bool) + Send + 'static>,
  );
  fn prompt(
    &self,
    title: &str,
    message: &str,
    default_value: &str,
    callback: Box<dyn FnOnce(Option<String>) + Send + 'static>,
  );
}

/// Stores the window ID of the initial window created during runtime init.
/// The first `BrowserWindow` constructor takes this ID to wrap the existing
/// window; subsequent constructors create new windows.
pub struct InitialWindowId(pub std::sync::Mutex<Option<u32>>);

struct BrowserWindow {
  api: Arc<dyn DesktopApi>,
  window_id: u32,
  surface: SameObject<deno_webgpu::byow::UnsafeWindowSurface>,
}

// SAFETY: we're sure this can be GCed
unsafe impl deno_core::GarbageCollected for BrowserWindow {
  fn trace(&self, _visitor: &mut deno_core::v8::cppgc::Visitor) {}

  fn get_name(&self) -> &'static std::ffi::CStr {
    c"BrowserWindow"
  }
}

impl deno_core::Resource for BrowserWindow {
  fn name(&self) -> Cow<'_, str> {
    "BrowserWindow".into()
  }
}

struct EventTargetSetup {
  brand: v8::Global<v8::Value>,
  set_event_target_data: v8::Global<v8::Value>,
}

#[op2]
impl BrowserWindow {
  #[constructor]
  fn new(
    state: &OpState,
    scope: &mut v8::PinScope<'_, '_>,
    #[scoped] options: Option<BrowserWindowOptions>,
  ) -> v8::Global<v8::Value> {
    let api = state
      .try_borrow::<Arc<dyn DesktopApi>>()
      .expect("desktop mode enabled")
      .clone();

    // Use the initial window if this is the first BrowserWindow,
    // otherwise create a new one.
    let window_id = state
      .try_borrow::<InitialWindowId>()
      .and_then(|iw| iw.0.lock().unwrap().take())
      .unwrap_or_else(|| {
        let width = options.as_ref().and_then(|o| o.width).unwrap_or(800);
        let height = options.as_ref().and_then(|o| o.height).unwrap_or(600);
        api.create_window(width, height)
      });

    if let Some(options) = &options {
      if let Some(title) = &options.title {
        api.set_title(window_id, title);
      }
      api.set_window_size(
        window_id,
        options.width.unwrap_or(800),
        options.height.unwrap_or(600),
      );
      if let (Some(x), Some(y)) = (options.x, options.y) {
        api.set_window_position(window_id, x, y);
      }
      if let Some(resizable) = options.resizable {
        api.set_resizable(window_id, resizable);
      }
      if let Some(always_on_top) = options.always_on_top {
        api.set_always_on_top(window_id, always_on_top);
      }
    }

    let window = BrowserWindow {
      api,
      window_id,
      surface: SameObject::new(),
    };
    let window = deno_core::cppgc::make_cppgc_object(scope, window);
    let event_target_setup = state.borrow::<EventTargetSetup>();
    let webidl_brand = v8::Local::new(scope, event_target_setup.brand.clone());
    window.set(scope, webidl_brand, webidl_brand);
    let set_event_target_data =
      v8::Local::new(scope, event_target_setup.set_event_target_data.clone())
        .cast::<v8::Function>();
    let null = v8::null(scope);
    set_event_target_data.call(scope, null.into(), &[window.into()]);
    let window = window.cast::<v8::Value>();

    v8::Global::new(scope, window)
  }

  #[getter]
  fn window_id(&self) -> u32 {
    self.window_id
  }

  #[fast]
  fn bind(&self, #[string] name: &str) {
    self.api.bind(self.window_id, name);
  }

  #[fast]
  fn unbind(&self, #[string] name: &str) {
    self.api.unbind(self.window_id, name);
  }

  #[fast]
  fn set_title(&self, #[string] title: &str) {
    self.api.set_title(self.window_id, title);
  }

  fn get_size(&self) -> (i32, i32) {
    self.api.get_window_size(self.window_id)
  }

  #[fast]
  fn set_size(&self, #[smi] width: i32, #[smi] height: i32) {
    self.api.set_window_size(self.window_id, width, height);
  }

  fn get_position(&self) -> (i32, i32) {
    self.api.get_window_position(self.window_id)
  }

  #[fast]
  fn set_position(&self, #[smi] x: i32, #[smi] y: i32) {
    self.api.set_window_position(self.window_id, x, y);
  }

  #[fast]
  fn is_resizable(&self) -> bool {
    self.api.is_resizable(self.window_id)
  }

  #[fast]
  fn set_resizable(&self, resizable: bool) {
    self.api.set_resizable(self.window_id, resizable);
  }

  #[fast]
  fn is_always_on_top(&self) -> bool {
    self.api.is_always_on_top(self.window_id)
  }

  #[fast]
  fn set_always_on_top(&self, always_on_top: bool) {
    self.api.set_always_on_top(self.window_id, always_on_top);
  }

  #[fast]
  fn is_closed(&self) -> bool {
    self.api.is_closed(self.window_id)
  }

  #[fast]
  fn close(&self) {
    self.api.close_window(self.window_id);
  }

  #[fast]
  fn is_visible(&self) -> bool {
    self.api.is_visible(self.window_id)
  }

  #[fast]
  fn show(&self) {
    self.api.show(self.window_id);
  }

  #[fast]
  fn hide(&self) {
    self.api.hide(self.window_id);
  }

  #[fast]
  fn focus(&self) {
    self.api.focus(self.window_id);
  }

  #[fast]
  fn navigate(&self, #[string] url: &str) {
    self.api.navigate(self.window_id, url);
  }

  fn open_devtools(
    &self,
    #[serde] options: Option<OpenDevtoolsOptions>,
  ) -> Result<(), deno_error::JsErrorBox> {
    let (renderer, deno) = match options {
      Some(opts) => (opts.renderer.unwrap_or(true), opts.deno.unwrap_or(true)),
      None => (true, true),
    };
    if !renderer && !deno {
      return Err(deno_error::JsErrorBox::type_error(
        "At least one of 'renderer' or 'deno' must be true",
      ));
    }
    self.api.open_devtools(self.window_id, renderer, deno);
    Ok(())
  }

  #[fast]
  fn reload(&self) {
    self
      .api
      .execute_js(self.window_id, "location.reload()", Box::new(|_| {}));
  }

  async fn execute_js(
    &self,
    #[string] script: String,
  ) -> Result<ExecuteJsResult, deno_error::JsErrorBox> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    self.api.execute_js(
      self.window_id,
      &script,
      Box::new(move |result| {
        let _ = tx.send(result);
      }),
    );
    let result = rx.await.map_err(|_| {
      deno_error::JsErrorBox::generic("execute_js callback dropped")
    })?;
    Ok(ExecuteJsResult(result))
  }

  fn set_application_menu(&self, #[serde] menu: Vec<MenuItem>) {
    self.api.set_application_menu(self.window_id, menu);
  }

  fn show_context_menu(
    &self,
    #[smi] x: i32,
    #[smi] y: i32,
    #[serde] menu: Vec<MenuItem>,
  ) {
    self.api.show_context_menu(self.window_id, x, y, menu);
  }

  fn get_native_window(
    &self,
    state: &OpState,
    scope: &mut v8::PinScope<'_, '_>,
  ) -> Result<v8::Global<v8::Object>, deno_error::JsErrorBox> {
    let instance = state
      .try_borrow::<deno_webgpu::Instance>()
      .ok_or_else(|| {
        deno_error::JsErrorBox::type_error(
          "Cannot create surface outside of WebGPU context. Did you forget to call `navigator.gpu.requestAdapter()`?",
        )
      })?
      .clone();

    let api = self.api.clone();
    let window_id = self.window_id;

    Ok(self.surface.get(scope, move |_| {
      let (win_handle, display_handle) = api.get_raw_window_handle(window_id);

      // SAFETY: The raw handles are valid for the lifetime of the window,
      // which is guaranteed by the BrowserWindow preventing the window from
      // being destroyed while this surface exists.
      let surface_id = unsafe {
        instance
          .instance_create_surface(display_handle, win_handle, None)
          .expect("failed to create surface")
      };

      let (width, height) = api.get_window_size(window_id);

      deno_webgpu::byow::UnsafeWindowSurface {
        id: surface_id,
        width: RefCell::new(width as u32),
        height: RefCell::new(height as u32),
        context: SameObject::new(),
      }
    }))
  }
}

#[derive(FromV8)]
struct BrowserWindowOptions {
  title: Option<String>,
  width: Option<i32>,
  height: Option<i32>,
  x: Option<i32>,
  y: Option<i32>,
  resizable: Option<bool>,
  always_on_top: Option<bool>,
}

#[derive(Clone, Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MenuItem {
  Item {
    label: String,
    id: Option<String>,
    accelerator: Option<String>,
    enabled: bool,
  },
  Submenu {
    label: String,
    items: Vec<MenuItem>,
  },
  Separator,
  Role {
    role: String,
  },
}

/// State for the auto-update system, placed into OpState at init.
pub struct AutoUpdateState {
  /// Path to the currently running dylib on disk.
  pub dylib_path: std::path::PathBuf,
  /// App version from metadata (deno.json `version` field).
  pub app_version: Option<String>,
  /// Whether we rolled back from a failed update on this launch.
  pub rolled_back: bool,
}

#[op2(fast)]
pub fn op_desktop_apply_patch(
  state: &mut OpState,
  #[buffer] patch_bytes: &[u8],
) -> Result<(), deno_error::JsErrorBox> {
  let update_state =
    state.try_borrow::<AutoUpdateState>().ok_or_else(|| {
      deno_error::JsErrorBox::generic("Auto-update state not initialized")
    })?;
  let dylib_path = &update_state.dylib_path;

  let original = std::fs::read(dylib_path).map_err(|e| {
    deno_error::JsErrorBox::generic(format!(
      "Failed to read dylib at {}: {}",
      dylib_path.display(),
      e
    ))
  })?;

  let patcher = qbsdiff::Bspatch::new(patch_bytes).map_err(|e| {
    deno_error::JsErrorBox::generic(format!("Invalid patch: {}", e))
  })?;
  let target_size = patcher.hint_target_size() as usize;
  let mut patched = Vec::with_capacity(target_size);
  patcher
    .apply(&original, std::io::Cursor::new(&mut patched))
    .map_err(|e| {
      deno_error::JsErrorBox::generic(format!("bspatch failed: {}", e))
    })?;

  let update_path = dylib_path.with_extension(format!(
    "{}.update",
    dylib_path.extension().unwrap_or_default().to_string_lossy()
  ));
  std::fs::write(&update_path, &patched).map_err(|e| {
    deno_error::JsErrorBox::generic(format!(
      "Failed to write update to {}: {}",
      update_path.display(),
      e
    ))
  })?;

  log::info!(
    "Update written to {}. Will be applied on next launch.",
    update_path.display()
  );

  Ok(())
}

#[op2]
#[serde]
async fn op_desktop_recv_event(
  state: std::rc::Rc<std::cell::RefCell<OpState>>,
) -> Option<DesktopEvent> {
  let rx = {
    let s = state.borrow();
    s.try_borrow::<DesktopEventReceiver>().map(|r| r.0.clone())
  };
  if let Some(rx) = rx {
    rx.lock().await.recv().await
  } else {
    std::future::pending().await
  }
}

#[op2(fast)]
pub fn op_desktop_confirm_update(state: &mut OpState) {
  if let Some(s) = state.try_borrow::<AutoUpdateState>() {
    let ext = s
      .dylib_path
      .extension()
      .unwrap_or_default()
      .to_string_lossy();
    let sentinel = s.dylib_path.with_extension(format!("{}.update-ok", ext));
    let _ = std::fs::write(&sentinel, b"ok");
  }
}

#[op2]
fn op_desktop_resolve_bind_call(
  state: &mut OpState,
  #[smi] call_id: u32,
  #[serde] result: serde_json::Value,
) {
  if let Some(responses) = state.try_borrow::<PendingBindResponses>() {
    if let Some(tx) = responses.0.lock().unwrap().remove(&call_id) {
      let _ = tx.send(Ok(result));
    }
  }
}

#[op2(fast)]
fn op_desktop_reject_bind_call(
  state: &mut OpState,
  #[smi] call_id: u32,
  #[string] error: String,
) {
  if let Some(responses) = state.try_borrow::<PendingBindResponses>() {
    if let Some(tx) = responses.0.lock().unwrap().remove(&call_id) {
      let _ = tx.send(Err(error));
    }
  }
}

#[op2(fast)]
pub fn op_desktop_init(
  state: &mut OpState,
  scope: &mut v8::PinScope<'_, '_>,
  webidl_brand: v8::Local<v8::Value>,
  set_event_target_data: v8::Local<v8::Value>,
) {
  state.put(EventTargetSetup {
    brand: v8::Global::new(scope, webidl_brand),
    set_event_target_data: v8::Global::new(scope, set_event_target_data),
  });
}

#[op2(fast)]
fn op_desktop_alert(
  state: &mut OpState,
  #[string] title: &str,
  #[string] message: &str,
) {
  if let Some(api) = state.try_borrow::<Arc<dyn DesktopApi>>() {
    api.alert(title, message);
  }
}

struct ErrorReportConfig {
  url: String,
  app_version: Option<String>,
}

static ERROR_REPORT_CONFIG: OnceLock<ErrorReportConfig> = OnceLock::new();

/// Store the error reporting URL and app version so the panic hook can
/// send reports without access to OpState.
pub fn set_error_report_config(url: String, app_version: Option<String>) {
  let _ = ERROR_REPORT_CONFIG.set(ErrorReportConfig { url, app_version });
}

/// Returns the error reporting URL and app version, if configured.
pub fn error_report_config() -> Option<(&'static str, Option<&'static str>)> {
  ERROR_REPORT_CONFIG
    .get()
    .map(|c| (c.url.as_str(), c.app_version.as_deref()))
}

/// Send a JSON error report to the given URL.  Handles `file://`,
/// `http://`, and `https://` URLs.  Best-effort — never panics.
/// Safe to call from a panic hook (creates its own tokio runtime for HTTP).
pub fn send_error_report(url: &str, body: &str) {
  let Ok(parsed) = deno_core::url::Url::parse(url) else {
    // Not a valid URL — treat as a file path.
    let mut line = body.to_string();
    line.push('\n');
    let _ = std::fs::OpenOptions::new()
      .create(true)
      .append(true)
      .open(url)
      .and_then(|mut f| std::io::Write::write_all(&mut f, line.as_bytes()));
    return;
  };

  match parsed.scheme() {
    "file" => {
      if let Ok(path) = parsed.to_file_path() {
        let mut line = body.to_string();
        line.push('\n');
        let _ = std::fs::OpenOptions::new()
          .create(true)
          .append(true)
          .open(path)
          .and_then(|mut f| std::io::Write::write_all(&mut f, line.as_bytes()));
      }
    }
    "http" | "https" => {
      let url_str = parsed.to_string();
      let body = body.to_string();
      let _ = std::thread::spawn(move || {
        let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
          .enable_io()
          .enable_time()
          .build()
        else {
          return;
        };
        runtime.block_on(async move {
          let Ok(client) =
            deno_fetch::create_http_client("deno-desktop", Default::default())
          else {
            return;
          };
          let Ok(uri) = url_str.parse::<http::Uri>() else {
            return;
          };
          let mut req =
            http::Request::new(deno_fetch::ReqBody::full(body.into()));
          *req.method_mut() = http::Method::POST;
          *req.uri_mut() = uri;
          req.headers_mut().insert(
            http::header::CONTENT_TYPE,
            http::HeaderValue::from_static("application/json"),
          );
          let _ = client.send(req).await;
        });
      })
      .join();
    }
    _ => {}
  }
}

#[op2(fast)]
fn op_desktop_send_error_report(
  state: &mut OpState,
  #[string] url: &str,
  #[string] body: &str,
) {
  let parsed = deno_core::url::Url::parse(url);

  // For HTTP(S) URLs, prefer the client from OpState (has user's TLS config).
  if let Ok(ref parsed) = parsed {
    if matches!(parsed.scheme(), "http" | "https") {
      if let Ok(client) = deno_fetch::get_or_create_client_from_state(state) {
        let url_str = parsed.to_string();
        let body = body.to_string();
        let _ = std::thread::spawn(move || {
          let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
            .enable_io()
            .enable_time()
            .build()
          else {
            return;
          };
          runtime.block_on(async move {
            let Ok(uri) = url_str.parse::<http::Uri>() else {
              return;
            };
            let mut req =
              http::Request::new(deno_fetch::ReqBody::full(body.into()));
            *req.method_mut() = http::Method::POST;
            *req.uri_mut() = uri;
            req.headers_mut().insert(
              http::header::CONTENT_TYPE,
              http::HeaderValue::from_static("application/json"),
            );
            let _ = client.send(req).await;
          });
        })
        .join();
        return;
      }
    }
  }

  // Fall back to the standalone sender for file URLs and non-HTTP.
  send_error_report(url, body);
}

#[op2(fast)]
fn op_desktop_confirm(state: &mut OpState, #[string] message: &str) -> bool {
  if let Some(api) = state.try_borrow::<Arc<dyn DesktopApi>>() {
    let (tx, rx) = std::sync::mpsc::channel();
    api.confirm(
      "",
      message,
      Box::new(move |result| {
        let _ = tx.send(result);
      }),
    );
    rx.recv().unwrap_or(false)
  } else {
    false
  }
}

#[op2]
#[string]
fn op_desktop_prompt(
  state: &mut OpState,
  #[string] message: &str,
  #[string] default_value: Option<String>,
) -> Option<String> {
  if let Some(api) = state.try_borrow::<Arc<dyn DesktopApi>>() {
    let (tx, rx) = std::sync::mpsc::channel();
    api.prompt(
      "",
      message,
      default_value.as_deref().unwrap_or(""),
      Box::new(move |result| {
        let _ = tx.send(result);
      }),
    );
    rx.recv().unwrap_or(None)
  } else {
    None
  }
}

deno_core::extension!(
  deno_desktop,
  ops = [
    op_desktop_apply_patch,
    op_desktop_confirm_update,
    op_desktop_init,
    op_desktop_recv_event,
    op_desktop_resolve_bind_call,
    op_desktop_reject_bind_call,
    op_desktop_alert,
    op_desktop_confirm,
    op_desktop_prompt,
    op_desktop_send_error_report,
  ],
  objects = [BrowserWindow,],
);
