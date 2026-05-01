// Copyright 2018-2026 the Deno authors. MIT license.

//! Desktop window management ops for `deno compile --desktop`.
//!
//! These ops are included in the V8 snapshot so their external references
//! are stable. When `DesktopApi` is not present in OpState (non-desktop
//! builds), the ops silently no-op.

use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::Path;
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
  #[serde(rename_all = "camelCase")]
  DockMenuClick { id: String },
  #[serde(rename_all = "camelCase")]
  DockReopen { has_visible_windows: bool },
  #[serde(rename_all = "camelCase")]
  TrayClick { tray_id: u32 },
  #[serde(rename_all = "camelCase")]
  TrayDoubleClick { tray_id: u32 },
  #[serde(rename_all = "camelCase")]
  TrayMenuClick { tray_id: u32, id: String },
}

/// Capacity of the runtime-bound event channel. A misbehaving renderer could
/// otherwise flood mouse-move / wheel events fast enough to OOM the runtime
/// (the channel was previously unbounded). When full, low-priority events
/// (motion / wheel) are dropped via `try_send` and a warning is logged.
const DESKTOP_EVENT_CHANNEL_CAPACITY: usize = 1024;

type DesktopEventRx =
  tokio::sync::Mutex<tokio::sync::mpsc::Receiver<DesktopEvent>>;
pub type DesktopEventTx = tokio::sync::mpsc::Sender<DesktopEvent>;

pub struct DesktopEventReceiver(pub Arc<DesktopEventRx>);
#[derive(Clone)]
pub struct DesktopEventSender(pub DesktopEventTx);

impl DesktopEventSender {
  /// Send an event, dropping it on backpressure rather than blocking or
  /// allocating. Use this for high-frequency events (mouse move, wheel).
  pub fn try_send(&self, event: DesktopEvent) {
    if let Err(tokio::sync::mpsc::error::TrySendError::Full(_)) =
      self.0.try_send(event)
    {
      // Log once per overflow burst would be ideal, but a plain warn is fine
      // here — this only fires on pathological event rates.
      log::warn!(
        "desktop event channel full; dropping event (renderer producing events faster than runtime can drain)"
      );
    }
  }
}

pub fn create_desktop_event_channel()
-> (DesktopEventSender, DesktopEventReceiver) {
  let (tx, rx) = tokio::sync::mpsc::channel(DESKTOP_EVENT_CHANNEL_CAPACITY);
  (
    DesktopEventSender(tx),
    DesktopEventReceiver(Arc::new(tokio::sync::Mutex::new(rx))),
  )
}

/// A pending call from the webview to a bound Deno function.
pub struct PendingBindCall {
  pub name: String,
  pub args: serde_json::Value,
  pub response: tokio::sync::oneshot::Sender<Result<serde_json::Value, String>>,
}

type PendingBindResponsesMap =
  HashMap<u32, tokio::sync::oneshot::Sender<Result<serde_json::Value, String>>>;

#[derive(Clone)]
pub struct PendingBindResponses(
  pub Arc<std::sync::Mutex<PendingBindResponsesMap>>,
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

  /// Set a short text badge on the app's dock / taskbar icon. An empty
  /// string clears the badge.
  fn set_dock_badge(&self, text: &str);
  /// Bounce the dock icon (macOS) or the closest native analog. `critical`
  /// maps to a continuous bounce; otherwise a single bounce.
  fn bounce_dock(&self, critical: bool);
  /// Set a custom right-click menu on the app's dock icon (macOS only).
  /// `None` clears any menu previously set.
  fn set_dock_menu(&self, menu: Option<Vec<MenuItem>>);
  /// Show or hide the app's dock icon (macOS activation policy).
  fn set_dock_visible(&self, visible: bool);

  /// Returns `0` if the backend doesn't support tray icons.
  fn create_tray(&self) -> u32;
  /// Destroy a tray icon previously created with `create_tray`.
  fn destroy_tray(&self, tray_id: u32);
  /// Set the tray icon image from PNG-encoded bytes.
  fn set_tray_icon(&self, tray_id: u32, png_bytes: &[u8]);
  /// Set the tray icon used in OS dark mode. `None` clears it.
  fn set_tray_icon_dark(&self, tray_id: u32, png_bytes: Option<&[u8]>);
  /// Set the tooltip shown on hover. `None` clears it.
  fn set_tray_tooltip(&self, tray_id: u32, text: Option<&str>);
  /// Set the right-click context menu on the tray icon. `None` clears
  /// any menu previously set.
  fn set_tray_menu(&self, tray_id: u32, menu: Option<Vec<MenuItem>>);
}

/// Stores the window ID of the initial window created during runtime init.
/// The first `BrowserWindow` constructor takes this ID to wrap the existing
/// window; subsequent constructors create new windows.
pub struct InitialWindowId(pub std::sync::Mutex<Option<u32>>);

struct BrowserWindow {
  api: Arc<dyn DesktopApi>,
  window_id: u32,
  surface: SameObject<deno_webgpu::byow::UnsafeWindowSurface>,
  /// Set when JS has taken a `getNativeWindow()` surface. Once a webgpu
  /// surface holds the underlying raw window handles, destroying the OS
  /// window underneath it would dangle those handles in wgpu-internal state
  /// (use-after-free at present). We refuse to destroy the window in that
  /// case and only hide it; the surface keeps the window alive until JS
  /// releases the BrowserWindow (cppgc) and with it the surface.
  surface_taken: std::cell::Cell<bool>,
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
      surface_taken: std::cell::Cell::new(false),
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
    if self.surface_taken.get() {
      // A WebGPU surface is referencing this window's native handles.
      // Destroying the OS window now would dangle those handles. Hide
      // instead; cleanup happens when the BrowserWindow is GC'd.
      log::warn!(
        "BrowserWindow.close(): a WebGPU surface is still attached; hiding window instead of destroying it"
      );
      self.api.hide(self.window_id);
      return;
    }
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
    self.surface_taken.set(true);

    Ok(self.surface.get(scope, move |_| {
      let (win_handle, display_handle) = api.get_raw_window_handle(window_id);

      // SAFETY: The raw handles are valid for the lifetime of the OS window.
      // `BrowserWindow.close()` is suppressed (downgraded to hide) once a
      // surface has been taken (`surface_taken`), and the OS window outlives
      // both the cached `SameObject<UnsafeWindowSurface>` and the
      // BrowserWindow itself, so the handles remain valid for the surface's
      // lifetime.
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

/// Hex-decoded length of a SHA-256 digest.
const SHA256_HEX_LEN: usize = 64;

fn dylib_magic_ok(bytes: &[u8]) -> bool {
  if bytes.len() < 4 {
    return false;
  }
  let m = &bytes[..4];
  // Mach-O (32/64 BE/LE), Mach-O fat, ELF, PE/COFF (MZ).
  matches!(
    m,
    [0xFE, 0xED, 0xFA, 0xCE]
      | [0xFE, 0xED, 0xFA, 0xCF]
      | [0xCE, 0xFA, 0xED, 0xFE]
      | [0xCF, 0xFA, 0xED, 0xFE]
      | [0xCA, 0xFE, 0xBA, 0xBE]
      | [0xCA, 0xFE, 0xBA, 0xBF]
      | [0x7F, b'E', b'L', b'F']
  ) || m.starts_with(b"MZ")
}

#[op2(fast)]
pub fn op_desktop_apply_patch(
  state: &mut OpState,
  #[buffer] patch_bytes: &[u8],
  #[string] expected_sha256: &str,
) -> Result<(), deno_error::JsErrorBox> {
  let update_state =
    state.try_borrow::<AutoUpdateState>().ok_or_else(|| {
      deno_error::JsErrorBox::generic("Auto-update state not initialized")
    })?;
  let dylib_path = &update_state.dylib_path;

  // Verify the patch bytes against the SHA-256 declared in the manifest before
  // we trust them with `bspatch`. Without this, anyone who can MITM the patch
  // download (or compromise the release host) could deliver arbitrary native
  // code. The hash itself is only as trustworthy as the manifest delivery
  // (TLS) and, when configured, the manifest signature checked in JS.
  let expected_sha256 = expected_sha256.trim().to_ascii_lowercase();
  if expected_sha256.len() != SHA256_HEX_LEN
    || !expected_sha256.chars().all(|c| c.is_ascii_hexdigit())
  {
    return Err(deno_error::JsErrorBox::generic(
      "Auto-update: manifest is missing a valid SHA-256 patch hash",
    ));
  }
  let actual_sha256 = {
    use sha2::Digest;
    faster_hex::hex_string(&sha2::Sha256::digest(patch_bytes)).to_lowercase()
  };
  if actual_sha256 != expected_sha256 {
    return Err(deno_error::JsErrorBox::generic(format!(
      "Auto-update: patch SHA-256 mismatch (expected {expected_sha256}, got {actual_sha256})"
    )));
  }

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

  // Sanity-check the patched bytes look like a real native binary. This
  // doesn't make the file safe to load (the hash check above does that), but
  // it catches a malformed or empty payload before we stage the swap and
  // shrinks the window where rename(2) into place could fail and leave the
  // app without a working dylib.
  if !dylib_magic_ok(&patched) {
    return Err(deno_error::JsErrorBox::generic(
      "Auto-update: patched dylib does not look like a native binary",
    ));
  }

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

/// Verify an Ed25519 signature over `message` using the base64-encoded
/// 32-byte public key and base64-encoded 64-byte signature. Used by the JS
/// auto-update path to validate `latest.json` before fetching any patch.
#[op2(fast)]
pub fn op_desktop_verify_ed25519(
  #[string] public_key_b64: &str,
  #[string] signature_b64: &str,
  #[buffer] message: &[u8],
) -> bool {
  use base64::Engine;
  let engine = base64::engine::general_purpose::STANDARD;
  let Ok(pk_bytes) = engine.decode(public_key_b64.trim()) else {
    return false;
  };
  let Ok(sig_bytes) = engine.decode(signature_b64.trim()) else {
    return false;
  };
  let Ok(pk_arr): Result<[u8; 32], _> = pk_bytes.as_slice().try_into() else {
    return false;
  };
  let Ok(sig_arr): Result<[u8; 64], _> = sig_bytes.as_slice().try_into() else {
    return false;
  };
  let Ok(verifying_key) = ed25519_dalek::VerifyingKey::from_bytes(&pk_arr)
  else {
    return false;
  };
  let signature = ed25519_dalek::Signature::from_bytes(&sig_arr);
  use ed25519_dalek::Verifier;
  verifying_key.verify(message, &signature).is_ok()
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

/// Stash of the `OpState` HTTP client for the panic-hook path. The panic
/// hook can't reach `OpState`, so we capture a client when the runtime
/// initializes error reporting and reuse it from both code paths. This
/// keeps a single TLS configuration (the user's roots) — earlier the panic
/// path constructed an ad-hoc `reqwest`/`fetch` client that bypassed it.
static ERROR_REPORT_CLIENT: OnceLock<deno_fetch::Client> = OnceLock::new();

/// Capture the OpState HTTP client for use by the panic hook.
pub fn set_error_report_client(client: deno_fetch::Client) {
  let _ = ERROR_REPORT_CLIENT.set(client);
}

fn append_to_file(path: &Path, body: &str) {
  let mut line = body.to_string();
  line.push('\n');
  let _ = std::fs::OpenOptions::new()
    .create(true)
    .append(true)
    .open(path)
    .and_then(|mut f| std::io::Write::write_all(&mut f, line.as_bytes()));
}

fn post_error_report(client: deno_fetch::Client, url: String, body: String) {
  let _ = std::thread::spawn(move || {
    let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
      .enable_io()
      .enable_time()
      .build()
    else {
      return;
    };
    runtime.block_on(async move {
      let Ok(uri) = url.parse::<http::Uri>() else {
        return;
      };
      let mut req = http::Request::new(deno_fetch::ReqBody::full(body.into()));
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

/// Send a JSON error report to the given URL. Best-effort — never panics.
/// Accepts `file://` (or a bare path) and `https://`. Plain `http://` is
/// rejected: error reports usually carry stack traces and runtime context,
/// so anyone on-path could read them.
pub fn send_error_report(url: &str, body: &str) {
  let parsed = deno_core::url::Url::parse(url);
  let Ok(parsed) = parsed else {
    append_to_file(Path::new(url), body);
    return;
  };

  match parsed.scheme() {
    "file" => {
      if let Ok(path) = parsed.to_file_path() {
        append_to_file(&path, body);
      }
    }
    "https" => {
      let Some(client) = ERROR_REPORT_CLIENT.get().cloned() else {
        log::warn!(
          "desktop: error-report HTTP client not initialized; dropping report"
        );
        return;
      };
      post_error_report(client, parsed.to_string(), body.to_string());
    }
    other => {
      log::warn!(
        "desktop: refusing to send error report over '{other}' (https only); dropping report",
      );
    }
  }
}

#[op2(fast)]
fn op_desktop_send_error_report(
  state: &mut OpState,
  #[string] url: &str,
  #[string] body: &str,
) {
  // Make sure the panic-hook path has a client too. The OpState client is
  // the one configured with the user's TLS roots/permissions, so we share
  // it across both code paths instead of creating an ad-hoc client.
  if ERROR_REPORT_CLIENT.get().is_none()
    && let Ok(client) = deno_fetch::get_or_create_client_from_state(state)
  {
    set_error_report_client(client);
  }
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

struct Dock {
  api: Arc<dyn DesktopApi>,
}

// SAFETY: we're sure this can be GCed
unsafe impl deno_core::GarbageCollected for Dock {
  fn trace(&self, _visitor: &mut deno_core::v8::cppgc::Visitor) {}

  fn get_name(&self) -> &'static std::ffi::CStr {
    c"Dock"
  }
}

impl deno_core::Resource for Dock {
  fn name(&self) -> Cow<'_, str> {
    "Dock".into()
  }
}

#[op2]
impl Dock {
  #[constructor]
  fn new(
    state: &OpState,
    scope: &mut v8::PinScope<'_, '_>,
  ) -> v8::Global<v8::Value> {
    let api = state
      .try_borrow::<Arc<dyn DesktopApi>>()
      .expect("desktop mode enabled")
      .clone();

    let dock = Dock { api };
    let dock = deno_core::cppgc::make_cppgc_object(scope, dock);
    let event_target_setup = state.borrow::<EventTargetSetup>();
    let webidl_brand = v8::Local::new(scope, event_target_setup.brand.clone());
    dock.set(scope, webidl_brand, webidl_brand);
    let set_event_target_data =
      v8::Local::new(scope, event_target_setup.set_event_target_data.clone())
        .cast::<v8::Function>();
    let null = v8::null(scope);
    set_event_target_data.call(scope, null.into(), &[dock.into()]);
    let dock = dock.cast::<v8::Value>();

    v8::Global::new(scope, dock)
  }

  #[fast]
  fn set_badge(&self, #[string] text: &str) {
    self.api.set_dock_badge(text);
  }

  #[fast]
  fn bounce(&self, critical: bool) {
    self.api.bounce_dock(critical);
  }

  fn set_menu(&self, #[serde] menu: Option<Vec<MenuItem>>) {
    self.api.set_dock_menu(menu);
  }

  #[fast]
  fn set_visible(&self, visible: bool) {
    self.api.set_dock_visible(visible);
  }
}

struct Tray {
  api: Arc<dyn DesktopApi>,
  tray_id: u32,
}

// SAFETY: we're sure this can be GCed
unsafe impl deno_core::GarbageCollected for Tray {
  fn trace(&self, _visitor: &mut deno_core::v8::cppgc::Visitor) {}

  fn get_name(&self) -> &'static std::ffi::CStr {
    c"Tray"
  }
}

impl deno_core::Resource for Tray {
  fn name(&self) -> Cow<'_, str> {
    "Tray".into()
  }
}

#[op2]
impl Tray {
  #[constructor]
  fn new(
    state: &OpState,
    scope: &mut v8::PinScope<'_, '_>,
  ) -> v8::Global<v8::Value> {
    let api = state
      .try_borrow::<Arc<dyn DesktopApi>>()
      .expect("desktop mode enabled")
      .clone();

    let tray_id = api.create_tray();
    let tray = Tray { api, tray_id };
    let tray = deno_core::cppgc::make_cppgc_object(scope, tray);
    let event_target_setup = state.borrow::<EventTargetSetup>();
    let webidl_brand = v8::Local::new(scope, event_target_setup.brand.clone());
    tray.set(scope, webidl_brand, webidl_brand);
    let set_event_target_data =
      v8::Local::new(scope, event_target_setup.set_event_target_data.clone())
        .cast::<v8::Function>();
    let null = v8::null(scope);
    set_event_target_data.call(scope, null.into(), &[tray.into()]);
    let tray = tray.cast::<v8::Value>();

    v8::Global::new(scope, tray)
  }

  #[getter]
  fn tray_id(&self) -> u32 {
    self.tray_id
  }

  #[fast]
  fn set_icon(&self, #[buffer] png_bytes: &[u8]) {
    self.api.set_tray_icon(self.tray_id, png_bytes);
  }

  fn set_icon_dark(&self, #[buffer] png_bytes: Option<&[u8]>) {
    self.api.set_tray_icon_dark(self.tray_id, png_bytes);
  }

  fn set_tooltip(&self, #[string] text: Option<String>) {
    self.api.set_tray_tooltip(self.tray_id, text.as_deref());
  }

  fn set_menu(&self, #[serde] menu: Option<Vec<MenuItem>>) {
    self.api.set_tray_menu(self.tray_id, menu);
  }

  #[fast]
  fn destroy(&self) {
    self.api.destroy_tray(self.tray_id);
  }
}

deno_core::extension!(
  deno_desktop,
  ops = [
    op_desktop_apply_patch,
    op_desktop_verify_ed25519,
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
  objects = [BrowserWindow, Dock, Tray],
);
