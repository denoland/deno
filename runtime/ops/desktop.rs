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

/// A single event type that flows from the laufey backend to the Deno runtime.
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
  /// A deep link (`<scheme>://...`) was opened and routed to this app, either
  /// at launch (cold start) or while already running. Carries the full URL.
  #[serde(rename_all = "camelCase")]
  OpenUrl { url: String },
  #[serde(rename_all = "camelCase")]
  TrayClick { tray_id: u32 },
  #[serde(rename_all = "camelCase")]
  TrayDoubleClick { tray_id: u32 },
  #[serde(rename_all = "camelCase")]
  TrayMenuClick { tray_id: u32, id: String },
  #[serde(rename_all = "camelCase")]
  NotificationShow { notification_id: u32 },
  #[serde(rename_all = "camelCase")]
  NotificationClick { notification_id: u32 },
  #[serde(rename_all = "camelCase")]
  NotificationClose { notification_id: u32 },
  #[serde(rename_all = "camelCase")]
  NotificationError { notification_id: u32 },
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
    Self::default()
  }
}

impl Default for PendingBindResponses {
  fn default() -> Self {
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
/// runtime (denort_desktop) to bridge to the laufey backend.
///
/// All per-window methods take a `window_id` identifying the target window.
pub trait DesktopApi: Send + Sync + 'static {
  /// Create a new window with the given dimensions and return its ID.
  ///
  /// `frameless` drops the title bar and standard window chrome.
  /// `no_activate` makes the window a floating, non-activating utility panel
  /// (used for tray / menu-bar popovers): it floats above normal windows and
  /// does not steal key focus from the foreground app when shown.
  /// `transparent` gives the window a transparent background so the page's own
  /// alpha composites against whatever is behind the window. These are all
  /// creation-time properties and cannot be changed afterwards.
  fn create_window(
    &self,
    width: i32,
    height: i32,
    frameless: bool,
    no_activate: bool,
    transparent_titlebar: bool,
    transparent: bool,
  ) -> u32;
  /// Apply creation-time flags to the pre-created initial window when it is
  /// adopted by the first `BrowserWindow`.
  ///
  /// `frameless` / `no_activate` / `transparent_titlebar` / `transparent` can
  /// only be set at OS-window construction, but the initial window is created
  /// eagerly at startup (before user JS runs) so framework apps that never
  /// construct a `BrowserWindow` still get a window. When the first
  /// `BrowserWindow` requests any of these flags, the backend recreates the
  /// initial window with them and returns the new window id (which the runtime
  /// uses for navigation / HMR). The default implementation is a no-op
  /// returning the original id, so backends without an eager initial window are
  /// unaffected.
  #[allow(clippy::too_many_arguments, reason = "window init flags")]
  fn reinit_initial_window(
    &self,
    initial_window_id: u32,
    _width: i32,
    _height: i32,
    _frameless: bool,
    _no_activate: bool,
    _transparent_titlebar: bool,
    _transparent: bool,
  ) -> u32 {
    initial_window_id
  }
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

  /// Overall window opacity in `0.0..=1.0` (1.0 == fully opaque). Fades the
  /// whole window uniformly (chrome included), unlike the `transparent`
  /// creation flag which honors the page's per-pixel alpha.
  fn get_window_opacity(&self, window_id: u32) -> f64;
  fn set_window_opacity(&self, window_id: u32, opacity: f64);

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

  /// Best-effort fetch of the OS-level window/display handles for the
  /// given window. Returning `Err` instead of panicking matters because
  /// this trait method is reachable from a v8 op handler but its
  /// implementation is invoked across the laufey C ABI; an unwind through
  /// that boundary would be UB.
  fn get_raw_window_handle(
    &self,
    window_id: u32,
  ) -> Result<
    (
      raw_window_handle::RawWindowHandle,
      raw_window_handle::RawDisplayHandle,
    ),
    deno_error::JsErrorBox,
  >;

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
  /// Show a modal confirm dialog. Blocks the calling thread until the
  /// user dismisses it; the platform's modal run loop pumps OS events
  /// while the dialog is up so other windows continue to render and
  /// respond.
  fn confirm(&self, title: &str, message: &str) -> bool;
  /// Show a modal prompt dialog. Returns the entered text on confirm,
  /// `None` on cancel. Blocking semantics as `confirm`.
  fn prompt(
    &self,
    title: &str,
    message: &str,
    default_value: &str,
  ) -> Option<String>;

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
  /// The tray icon's screen rectangle `(x, y, width, height)` in the same
  /// top-left-origin coordinate space as window positions, or `None` if the
  /// icon has no on-screen position yet or the backend can't report it. Used
  /// to anchor a popover window under the icon.
  fn get_tray_bounds(&self, tray_id: u32) -> Option<(i32, i32, i32, i32)>;

  /// Show an OS notification. Returns the notification id (`0` if the
  /// backend doesn't support system notifications). Events for this
  /// notification (`Show`, `Click`, `Close`, `Error`) are delivered via
  /// the desktop event channel keyed by the returned id.
  fn show_notification(
    &self,
    title: &str,
    body: Option<&str>,
    icon: Option<&[u8]>,
    tag: Option<&str>,
    silent: Option<bool>,
    require_interaction: Option<bool>,
  ) -> u32;
  /// Dismiss a notification previously shown via `show_notification`.
  /// No-op if the id is unknown or already dismissed.
  fn close_notification(&self, notification_id: u32);

  /// Request OS authorization to show notifications. If the user has not
  /// yet decided, this triggers a system prompt; otherwise the cached
  /// decision is returned without a re-prompt. The callback fires on the
  /// UI thread with one of [`PermissionState::Granted`],
  /// [`PermissionState::Denied`], [`PermissionState::Prompt`] (rare —
  /// happens if the user dismissed the prompt without deciding) or
  /// [`PermissionState::Unsupported`] (backend / platform has no
  /// permission model — e.g. an unbundled macOS process, Linux libnotify).
  fn request_notification_permission(
    &self,
    cb: Box<dyn FnOnce(PermissionState) + Send + 'static>,
  );
  /// Query the current authorization state without prompting. Same status
  /// codes as [`request_notification_permission`].
  fn query_notification_permission(
    &self,
    cb: Box<dyn FnOnce(PermissionState) + Send + 'static>,
  );
}

/// Authorization state for a capability that the OS (or a runtime
/// component) gates. Mirrors the Web Permissions API state set with an
/// extra `Unsupported` variant for environments where the capability has
/// no permission model at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionState {
  Granted,
  Denied,
  Prompt,
  Unsupported,
}

/// Stores the window ID of the initial window created during runtime init.
/// The first `BrowserWindow` constructor takes this ID to wrap the existing
/// window; subsequent constructors create new windows.
pub struct InitialWindowId(pub std::sync::Mutex<Option<u32>>);

/// The compiled app's name (from deno.json `desktop.app.name`, falling back to
/// the output file name). Used as the default window title so a window the app
/// doesn't explicitly title shows the app name instead of the backend's
/// internal default (`laufey_webview`).
pub struct DesktopAppName(pub String);

struct BrowserWindow {
  api: Arc<dyn DesktopApi>,
  window_id: u32,
  surface: SameObject<deno_canvas::byow::UnsafeWindowSurface>,
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

    let width = options.as_ref().and_then(|o| o.width).unwrap_or(800);
    let height = options.as_ref().and_then(|o| o.height).unwrap_or(600);
    let frameless = options.as_ref().and_then(|o| o.frameless).unwrap_or(false);
    let no_activate = options
      .as_ref()
      .and_then(|o| o.no_activate)
      .unwrap_or(false);
    let transparent_titlebar = options
      .as_ref()
      .and_then(|o| o.transparent_titlebar)
      .unwrap_or(false);
    let transparent = options
      .as_ref()
      .and_then(|o| o.transparent)
      .unwrap_or(false);

    // Use the initial window if this is the first BrowserWindow, otherwise
    // create a new one. The initial window is created eagerly at startup with
    // default (framed / activating) flags, so if this first window requests a
    // creation-time flag we ask the backend to recreate it accordingly and use
    // the (possibly new) window id.
    let window_id = match state
      .try_borrow::<InitialWindowId>()
      .and_then(|iw| iw.0.lock().unwrap().take())
    {
      Some(initial_window_id)
        if frameless || no_activate || transparent_titlebar || transparent =>
      {
        api.reinit_initial_window(
          initial_window_id,
          width,
          height,
          frameless,
          no_activate,
          transparent_titlebar,
          transparent,
        )
      }
      Some(initial_window_id) => initial_window_id,
      None => api.create_window(
        width,
        height,
        frameless,
        no_activate,
        transparent_titlebar,
        transparent,
      ),
    };

    // Default the window title to the app name when the app doesn't set one,
    // so the window shows e.g. "MyApp" instead of the backend's internal
    // default (`laufey_webview`). An explicit `title` option below overrides
    // this, and a page that sets `document.title` overrides it at the OS level.
    if options.as_ref().and_then(|o| o.title.as_ref()).is_none()
      && let Some(name) = state.try_borrow::<DesktopAppName>()
      && !name.0.is_empty()
    {
      api.set_title(window_id, &name.0);
    }

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
      if let Some(opacity) = options.opacity {
        api.set_window_opacity(window_id, opacity);
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

  // Keep the native primitive separate from DESKTOP_JS's public `bind`
  // wrapper. The deferred fast-call upgrade may replace this symbol-backed
  // method without overwriting the wrapper.
  #[fast]
  #[symbol("Deno_privateDesktopBind")]
  fn bind(&self, #[string] name: &str) {
    self.api.bind(self.window_id, name);
  }

  #[fast]
  #[symbol("Deno_privateDesktopUnbind")]
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
  fn get_opacity(&self) -> f64 {
    self.api.get_window_opacity(self.window_id)
  }

  #[fast]
  fn set_opacity(&self, opacity: f64) {
    self.api.set_window_opacity(self.window_id, opacity);
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

    // Hoisted out of the `surface.try_get` closure so the
    // `get_raw_window_handle` failure path can bubble before we ever
    // touch wgpu (and can't unwind across the laufey C ABI).
    let (win_handle, display_handle) = api.get_raw_window_handle(window_id)?;

    let result = self.surface.try_get(scope, move |_| {
      // SAFETY: The raw handles are valid for the lifetime of the OS window.
      // `BrowserWindow.close()` is suppressed (downgraded to hide) once a
      // surface has been taken (`surface_taken`), and the OS window outlives
      // both the cached `SameObject<UnsafeWindowSurface>` and the
      // BrowserWindow itself, so the handles remain valid for the surface's
      // lifetime.
      let surface_id = unsafe {
        instance
          .instance_create_surface(Some(display_handle), win_handle, None)
          .map_err(|e| {
            deno_error::JsErrorBox::generic(format!(
              "failed to create wgpu surface: {e}"
            ))
          })?
      };
      let (width, height) = api.get_window_size(window_id);
      Ok::<_, deno_error::JsErrorBox>(deno_canvas::byow::UnsafeWindowSurface {
        data: std::rc::Rc::new(RefCell::new(
          deno_webgpu::canvas::SurfaceData {
            id: surface_id,
            width: width as u32,
            height: height as u32,
            instance,
          },
        )),
        active_context: Default::default(),
      })
    })?;
    // Only suppress close() once the surface is actually live. If
    // surface creation failed above, the window is still safe to close.
    self.surface_taken.set(true);
    Ok(result)
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
  opacity: Option<f64>,
  frameless: Option<bool>,
  no_activate: Option<bool>,
  transparent_titlebar: Option<bool>,
  transparent: Option<bool>,
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

#[allow(
  clippy::disallowed_methods,
  reason = "privileged auto-update op writes the live dylib outside any user's sandbox by design"
)]
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
/// 32-byte public key and base64-encoded 64-byte signature. Inner pure
/// function so it's directly callable from unit tests (the `#[op2]`
/// wrapper replaces the surface name with an `OpDecl`).
fn verify_ed25519_b64(
  public_key_b64: &str,
  signature_b64: &str,
  message: &[u8],
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

/// Verify an Ed25519 signature over `message` using the base64-encoded
/// 32-byte public key and base64-encoded 64-byte signature. Used by the JS
/// auto-update path to validate `latest.json` before fetching any patch.
#[op2(fast)]
pub fn op_desktop_verify_ed25519(
  #[string] public_key_b64: &str,
  #[string] signature_b64: &str,
  #[buffer] message: &[u8],
) -> bool {
  verify_ed25519_b64(public_key_b64, signature_b64, message)
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

#[allow(
  clippy::disallowed_methods,
  reason = "privileged auto-update sentinel write next to the dylib, outside any user sandbox"
)]
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
  if let Some(responses) = state.try_borrow::<PendingBindResponses>()
    && let Some(tx) = responses.0.lock().unwrap().remove(&call_id)
  {
    let _ = tx.send(Ok(result));
  }
}

#[op2(fast)]
fn op_desktop_reject_bind_call(
  state: &mut OpState,
  #[smi] call_id: u32,
  #[string] error: String,
) {
  if let Some(responses) = state.try_borrow::<PendingBindResponses>()
    && let Some(tx) = responses.0.lock().unwrap().remove(&call_id)
  {
    let _ = tx.send(Err(error));
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

#[allow(
  clippy::disallowed_methods,
  reason = "best-effort panic-hook error-report append; path is operator-configured via `error_reporting_url` and FileSystem trait isn't reachable from a panic hook"
)]
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
/// Accepts only `file://` and `https://`. Plain `http://` is rejected:
/// error reports usually carry stack traces and runtime context, so
/// anyone on-path could read them. A bare path (or any unparseable
/// string) is also rejected — previously such inputs were silently
/// treated as a local file path, which let a malformed metadata field
/// land error reports at an attacker-chosen location on disk.
pub fn send_error_report(url: &str, body: &str) {
  let Ok(parsed) = deno_core::url::Url::parse(url) else {
    log::warn!(
      "desktop: error_reporting_url is not a valid URL ({:?}); dropping report",
      url,
    );
    return;
  };

  match parsed.scheme() {
    "file" => {
      // `url_to_file_path` rejects `file://host/...` URLs (non-local),
      // so a local path is the only way to reach `append_to_file`.
      let Ok(path) = deno_path_util::url_to_file_path(&parsed) else {
        log::warn!(
          "desktop: error_reporting_url file:// URL is not a local path ({:?}); dropping report",
          url,
        );
        return;
      };
      append_to_file(&path, body);
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
        "desktop: refusing to send error report over '{other}' (file:// or https:// only); dropping report",
      );
    }
  }
}

#[op2(fast)]
fn op_desktop_send_error_report(state: &mut OpState, #[string] body: &str) {
  // The report destination is operator config — it is baked into the app at
  // build time (`error_reporting_url`) and stored in `ERROR_REPORT_CONFIG`.
  // It is deliberately NOT accepted from JS: this op is exposed on
  // `core.ops` and survives `removeImportedOps()`, so any (untrusted) code
  // in the runtime can call it. Trusting a caller-supplied URL would turn
  // this into an unrestricted file-append (`file://`) or network-POST
  // (`https://`) primitive that bypasses the `--allow-write`/`--allow-net`
  // permission checks every other fs/net op performs.
  let Some((url, _)) = error_report_config() else {
    // No reporting URL configured (e.g. plain `deno run`, or a desktop app
    // that didn't set one) — there is nowhere to send, so do nothing.
    return;
  };
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
  // Sync op: web `confirm()` returns a boolean, not a Promise. The
  // backend's `confirm` blocks the calling thread inside the platform's
  // modal run loop (NSAlert runModal / MessageBoxW / gtk_dialog_run /
  // rfd) which itself pumps OS events, so other windows stay responsive
  // while the dialog is up.
  match state.try_borrow::<Arc<dyn DesktopApi>>() {
    Some(api) => api.confirm("", message),
    None => false,
  }
}

#[op2]
#[string]
fn op_desktop_prompt(
  state: &mut OpState,
  #[string] message: &str,
  #[string] default_value: Option<String>,
) -> Option<String> {
  // See `op_desktop_confirm` for the sync-blocking rationale.
  match state.try_borrow::<Arc<dyn DesktopApi>>() {
    Some(api) => {
      api.prompt("", message, default_value.as_deref().unwrap_or(""))
    }
    None => None,
  }
}

fn permission_state_to_web_string(state: PermissionState) -> &'static str {
  // Web Permissions API state values; `Notification.requestPermission`
  // additionally maps `Prompt` → `"default"` per the Notifications spec.
  match state {
    PermissionState::Granted => "granted",
    PermissionState::Denied => "denied",
    PermissionState::Prompt => "prompt",
    PermissionState::Unsupported => "unsupported",
  }
}

#[op2]
#[string]
async fn op_desktop_request_notification_permission(
  state: std::rc::Rc<std::cell::RefCell<OpState>>,
) -> String {
  let api = {
    let s = state.borrow();
    s.try_borrow::<Arc<dyn DesktopApi>>().cloned()
  };
  let Some(api) = api else {
    // No backend wired up (snapshot build or non-desktop runtime).
    return "unsupported".to_string();
  };
  let (tx, rx) = tokio::sync::oneshot::channel::<PermissionState>();
  api.request_notification_permission(Box::new(move |state| {
    let _ = tx.send(state);
  }));
  // If the backend forgets to invoke the callback (programmer error in a
  // hypothetical custom backend), the channel drops and `recv` returns
  // `Err` — surface that as "unsupported" so JS gets a stable result.
  permission_state_to_web_string(
    rx.await.unwrap_or(PermissionState::Unsupported),
  )
  .to_string()
}

#[op2]
#[string]
async fn op_desktop_query_notification_permission(
  state: std::rc::Rc<std::cell::RefCell<OpState>>,
) -> String {
  let api = {
    let s = state.borrow();
    s.try_borrow::<Arc<dyn DesktopApi>>().cloned()
  };
  let Some(api) = api else {
    return "unsupported".to_string();
  };
  let (tx, rx) = tokio::sync::oneshot::channel::<PermissionState>();
  api.query_notification_permission(Box::new(move |state| {
    let _ = tx.send(state);
  }));
  permission_state_to_web_string(
    rx.await.unwrap_or(PermissionState::Unsupported),
  )
  .to_string()
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

  #[serde]
  fn get_bounds(&self) -> Option<TrayBounds> {
    self
      .api
      .get_tray_bounds(self.tray_id)
      .map(|(x, y, width, height)| TrayBounds {
        x,
        y,
        width,
        height,
      })
  }

  // DESKTOP_JS exposes the public `destroy` wrapper that also updates its tray
  // registry. Keep the native primitive on a distinct symbol-backed slot.
  #[fast]
  #[symbol("Deno_privateDesktopTrayDestroy")]
  fn destroy(&self) {
    self.api.destroy_tray(self.tray_id);
  }
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct TrayBounds {
  x: i32,
  y: i32,
  width: i32,
  height: i32,
}

struct Notification {
  api: Arc<dyn DesktopApi>,
  notification_id: u32,
  title: String,
  body: String,
  icon: String,
  tag: String,
  dir: String,
  lang: String,
  badge: String,
  silent: Option<bool>,
  require_interaction: bool,
  data: v8::Global<v8::Value>,
}

// SAFETY: we're sure this can be GCed
unsafe impl deno_core::GarbageCollected for Notification {
  fn trace(&self, _visitor: &mut deno_core::v8::cppgc::Visitor) {}

  fn get_name(&self) -> &'static std::ffi::CStr {
    c"Notification"
  }
}

impl deno_core::Resource for Notification {
  fn name(&self) -> Cow<'_, str> {
    "Notification".into()
  }
}

#[derive(FromV8)]
struct NotificationConstructorOptions {
  body: Option<String>,
  icon: Option<String>,
  tag: Option<String>,
  dir: Option<String>,
  lang: Option<String>,
  badge: Option<String>,
  silent: Option<bool>,
  require_interaction: Option<bool>,
  data: Option<v8::Global<v8::Value>>,
}

#[op2]
impl Notification {
  #[constructor]
  fn new(
    state: &OpState,
    scope: &mut v8::PinScope<'_, '_>,
    #[string] title: String,
    #[scoped] options: Option<NotificationConstructorOptions>,
    #[buffer] icon_bytes: Option<&[u8]>,
  ) -> v8::Global<v8::Value> {
    let api = state
      .try_borrow::<Arc<dyn DesktopApi>>()
      .expect("desktop mode enabled")
      .clone();

    let options = options.unwrap_or(NotificationConstructorOptions {
      body: None,
      icon: None,
      tag: None,
      dir: None,
      lang: None,
      badge: None,
      silent: None,
      require_interaction: None,
      data: None,
    });

    let notification_id = api.show_notification(
      &title,
      options.body.as_deref(),
      icon_bytes,
      options.tag.as_deref(),
      options.silent,
      options.require_interaction,
    );

    let data = options.data.unwrap_or_else(|| {
      let null: v8::Local<v8::Value> = v8::null(scope).into();
      v8::Global::new(scope, null)
    });

    let notification = Notification {
      api,
      notification_id,
      title,
      body: options.body.unwrap_or_default(),
      icon: options.icon.unwrap_or_default(),
      tag: options.tag.unwrap_or_default(),
      dir: options.dir.unwrap_or_else(|| "auto".to_string()),
      lang: options.lang.unwrap_or_default(),
      badge: options.badge.unwrap_or_default(),
      silent: options.silent,
      require_interaction: options.require_interaction.unwrap_or(false),
      data,
    };
    let notification = deno_core::cppgc::make_cppgc_object(scope, notification);
    let event_target_setup = state.borrow::<EventTargetSetup>();
    let webidl_brand = v8::Local::new(scope, event_target_setup.brand.clone());
    notification.set(scope, webidl_brand, webidl_brand);
    let set_event_target_data =
      v8::Local::new(scope, event_target_setup.set_event_target_data.clone())
        .cast::<v8::Function>();
    let null = v8::null(scope);
    set_event_target_data.call(scope, null.into(), &[notification.into()]);
    let notification = notification.cast::<v8::Value>();

    v8::Global::new(scope, notification)
  }

  #[getter]
  fn notification_id(&self) -> u32 {
    self.notification_id
  }

  #[getter]
  #[string]
  fn title(&self) -> String {
    self.title.clone()
  }

  #[getter]
  #[string]
  fn body(&self) -> String {
    self.body.clone()
  }

  #[getter]
  #[string]
  fn icon(&self) -> String {
    self.icon.clone()
  }

  #[getter]
  #[string]
  fn tag(&self) -> String {
    self.tag.clone()
  }

  #[getter]
  #[string]
  fn dir(&self) -> String {
    self.dir.clone()
  }

  #[getter]
  #[string]
  fn lang(&self) -> String {
    self.lang.clone()
  }

  #[getter]
  #[string]
  fn badge(&self) -> String {
    self.badge.clone()
  }

  #[getter]
  fn silent<'a>(
    &self,
    scope: &mut v8::PinScope<'a, '_>,
  ) -> v8::Local<'a, v8::Value> {
    match self.silent {
      Some(b) => v8::Boolean::new(scope, b).into(),
      None => v8::null(scope).into(),
    }
  }

  #[fast]
  #[getter]
  fn require_interaction(&self) -> bool {
    self.require_interaction
  }

  #[getter]
  fn data(&self) -> v8::Global<v8::Value> {
    self.data.clone()
  }

  #[fast]
  fn close(&self) {
    if self.notification_id != 0 {
      self.api.close_notification(self.notification_id);
    }
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
    op_desktop_request_notification_permission,
    op_desktop_query_notification_permission,
  ],
  objects = [BrowserWindow, Dock, Tray, Notification],
);

#[cfg(test)]
mod tests {
  use deno_core::serde_json;
  use deno_core::serde_json::json;

  use super::BrowserWindow;
  use super::DesktopEvent;
  use super::PendingBindResponses;
  use super::PermissionState;
  use super::Tray;
  use super::dylib_magic_ok;
  use super::permission_state_to_web_string;
  use super::register_bind_call;
  use super::verify_ed25519_b64;

  // These tests pin the wire format that the DESKTOP_JS event-loop
  // IIFE consumes. Changing any of these shapes is a breaking change
  // for in-renderer event listeners — the assertions below should
  // fail if you change a field name or remove a `#[serde(rename_all =
  // "camelCase")]` so you find out at test time, not at runtime in the
  // packaged app.

  #[test]
  fn js_wrapped_desktop_methods_use_private_symbols() {
    for (object, methods) in [
      (
        BrowserWindow::DECL,
        &["Deno_privateDesktopBind", "Deno_privateDesktopUnbind"][..],
      ),
      (Tray::DECL, &["Deno_privateDesktopTrayDestroy"][..]),
    ] {
      for name in methods {
        let method = object
          .methods
          .iter()
          .find(|method| method.name == *name)
          .unwrap_or_else(|| panic!("missing method {name}"));
        assert!(
          method.symbol_for,
          "{name} must not share a string property with its DESKTOP_JS wrapper"
        );
        let _ = method.fast_fn();
      }
    }

    for (object, public_names) in [
      (BrowserWindow::DECL, &["bind", "unbind"][..]),
      (Tray::DECL, &["destroy"][..]),
    ] {
      for name in public_names {
        assert!(
          object.methods.iter().all(|method| method.name != *name),
          "native method must not collide with the public {name} wrapper"
        );
      }
    }
  }

  #[test]
  fn app_menu_click_wire_shape() {
    let v = serde_json::to_value(DesktopEvent::AppMenuClick {
      window_id: 7,
      id: "file.quit".to_string(),
    })
    .unwrap();
    assert_eq!(
      v,
      json!({
        "kind": "appMenuClick",
        "windowId": 7,
        "id": "file.quit",
      })
    );
  }

  #[test]
  fn keyboard_event_camelcases_and_keeps_type() {
    let v = serde_json::to_value(DesktopEvent::KeyboardEvent {
      window_id: 1,
      r#type: "keydown".to_string(),
      key: "a".to_string(),
      code: "KeyA".to_string(),
      shift: true,
      control: false,
      alt: false,
      meta: true,
      repeat: false,
    })
    .unwrap();
    // `type` (a Rust keyword, written `r#type`) must serialize as
    // `"type"` — the renderer reads it as `e.type` per Web spec.
    assert_eq!(v["type"], "keydown");
    assert_eq!(v["kind"], "keyboardEvent");
    assert_eq!(v["windowId"], 1);
    assert_eq!(v["shift"], true);
    assert_eq!(v["meta"], true);
  }

  #[test]
  fn mouse_click_uses_client_xy() {
    let v = serde_json::to_value(DesktopEvent::MouseClick {
      window_id: 1,
      state: "released".to_string(),
      button: 0,
      client_x: 10.5,
      client_y: 20.25,
      shift: false,
      control: false,
      alt: false,
      meta: false,
      click_count: 1,
    })
    .unwrap();
    assert_eq!(v["kind"], "mouseClick");
    // The renderer reads `e.clientX` / `e.clientY` per the Web spec.
    // Snake-case names here would silently break the JS side.
    assert_eq!(v["clientX"], 10.5);
    assert_eq!(v["clientY"], 20.25);
    assert_eq!(v["clickCount"], 1);
  }

  #[test]
  fn window_resize_wire_shape() {
    let v = serde_json::to_value(DesktopEvent::WindowResize {
      window_id: 1,
      width: 800,
      height: 600,
    })
    .unwrap();
    assert_eq!(
      v,
      json!({
        "kind": "windowResize",
        "windowId": 1,
        "width": 800,
        "height": 600,
      })
    );
  }

  #[test]
  fn dock_reopen_camelcase_payload() {
    let v = serde_json::to_value(DesktopEvent::DockReopen {
      has_visible_windows: true,
    })
    .unwrap();
    assert_eq!(v["kind"], "dockReopen");
    assert_eq!(v["hasVisibleWindows"], true);
    // The snake_case variant must NOT exist — DESKTOP_JS reads the
    // camelCased name.
    assert!(v.get("has_visible_windows").is_none());
  }

  #[test]
  fn runtime_error_omits_stack_when_none() {
    let with_stack = serde_json::to_value(DesktopEvent::RuntimeError {
      message: "boom".to_string(),
      stack: Some("at foo".to_string()),
    })
    .unwrap();
    assert_eq!(with_stack["message"], "boom");
    assert_eq!(with_stack["stack"], "at foo");

    let no_stack = serde_json::to_value(DesktopEvent::RuntimeError {
      message: "boom".to_string(),
      stack: None,
    })
    .unwrap();
    // None should serialize as JSON null (not be omitted), matching
    // what the JS handler currently expects.
    assert_eq!(no_stack["stack"], serde_json::Value::Null);
  }

  #[test]
  fn notification_variants_share_field_name() {
    // All four notification variants must use the same `notificationId`
    // key so the JS handler can route by `kind` alone.
    for ev in [
      DesktopEvent::NotificationShow {
        notification_id: 42,
      },
      DesktopEvent::NotificationClick {
        notification_id: 42,
      },
      DesktopEvent::NotificationClose {
        notification_id: 42,
      },
      DesktopEvent::NotificationError {
        notification_id: 42,
      },
    ] {
      let v = serde_json::to_value(&ev).unwrap();
      assert_eq!(v["notificationId"], 42, "for variant {ev:?}");
    }
  }

  // --- Every remaining DesktopEvent variant gets a kind pin ---

  fn kind_of(ev: DesktopEvent) -> String {
    serde_json::to_value(ev).unwrap()["kind"]
      .as_str()
      .expect("kind must be a string")
      .to_string()
  }

  #[test]
  fn every_variant_has_camelcase_kind() {
    // The kind discriminator is what DESKTOP_JS switches on. A
    // misspelled or accidentally renamed variant would make its events
    // silently no-op in the renderer. Pin every kind name.
    assert_eq!(
      kind_of(DesktopEvent::AppMenuClick {
        window_id: 0,
        id: "".into()
      }),
      "appMenuClick"
    );
    assert_eq!(
      kind_of(DesktopEvent::ContextMenuClick {
        window_id: 0,
        id: "".into()
      }),
      "contextMenuClick"
    );
    assert_eq!(
      kind_of(DesktopEvent::KeyboardEvent {
        window_id: 0,
        r#type: "".into(),
        key: "".into(),
        code: "".into(),
        shift: false,
        control: false,
        alt: false,
        meta: false,
        repeat: false,
      }),
      "keyboardEvent"
    );
    assert_eq!(
      kind_of(DesktopEvent::BindCall {
        window_id: 0,
        name: "".into(),
        args: serde_json::Value::Null,
        call_id: 0,
      }),
      "bindCall"
    );
    assert_eq!(
      kind_of(DesktopEvent::MouseClick {
        window_id: 0,
        state: "".into(),
        button: 0,
        client_x: 0.0,
        client_y: 0.0,
        shift: false,
        control: false,
        alt: false,
        meta: false,
        click_count: 0,
      }),
      "mouseClick"
    );
    assert_eq!(
      kind_of(DesktopEvent::MouseMove {
        window_id: 0,
        client_x: 0.0,
        client_y: 0.0,
        shift: false,
        control: false,
        alt: false,
        meta: false,
      }),
      "mouseMove"
    );
    assert_eq!(
      kind_of(DesktopEvent::Wheel {
        window_id: 0,
        delta_x: 0.0,
        delta_y: 0.0,
        delta_mode: 0,
        client_x: 0.0,
        client_y: 0.0,
        shift: false,
        control: false,
        alt: false,
        meta: false,
      }),
      "wheel"
    );
    assert_eq!(
      kind_of(DesktopEvent::CursorEnterLeave {
        window_id: 0,
        entered: false,
        client_x: 0.0,
        client_y: 0.0,
        shift: false,
        control: false,
        alt: false,
        meta: false,
      }),
      "cursorEnterLeave"
    );
    assert_eq!(
      kind_of(DesktopEvent::FocusChanged {
        window_id: 0,
        focused: false
      }),
      "focusChanged"
    );
    assert_eq!(
      kind_of(DesktopEvent::WindowResize {
        window_id: 0,
        width: 0,
        height: 0
      }),
      "windowResize"
    );
    assert_eq!(
      kind_of(DesktopEvent::WindowMove {
        window_id: 0,
        x: 0,
        y: 0
      }),
      "windowMove"
    );
    assert_eq!(
      kind_of(DesktopEvent::CloseRequested { window_id: 0 }),
      "closeRequested"
    );
    assert_eq!(
      kind_of(DesktopEvent::RuntimeError {
        message: "".into(),
        stack: None
      }),
      "runtimeError"
    );
    assert_eq!(
      kind_of(DesktopEvent::DockMenuClick { id: "".into() }),
      "dockMenuClick"
    );
    assert_eq!(
      kind_of(DesktopEvent::DockReopen {
        has_visible_windows: false
      }),
      "dockReopen"
    );
    assert_eq!(kind_of(DesktopEvent::TrayClick { tray_id: 0 }), "trayClick");
    assert_eq!(
      kind_of(DesktopEvent::TrayDoubleClick { tray_id: 0 }),
      "trayDoubleClick"
    );
    assert_eq!(
      kind_of(DesktopEvent::TrayMenuClick {
        tray_id: 0,
        id: "".into()
      }),
      "trayMenuClick"
    );
    assert_eq!(
      kind_of(DesktopEvent::NotificationShow { notification_id: 0 }),
      "notificationShow"
    );
    assert_eq!(
      kind_of(DesktopEvent::NotificationClick { notification_id: 0 }),
      "notificationClick"
    );
    assert_eq!(
      kind_of(DesktopEvent::NotificationClose { notification_id: 0 }),
      "notificationClose"
    );
    assert_eq!(
      kind_of(DesktopEvent::NotificationError { notification_id: 0 }),
      "notificationError"
    );
  }

  // --- BindCall.args round-trip ---

  #[test]
  fn bind_call_args_passes_through_arbitrary_json() {
    // BindCall carries a serde_json::Value as `args`. We must serialize
    // it transparently (not nested under "args.value" or with a Some()
    // wrapper) so the renderer sees exactly what was passed.
    let ev = DesktopEvent::BindCall {
      window_id: 1,
      name: "greet".into(),
      args: json!([{"name": "ada", "n": 42}]),
      call_id: 7,
    };
    let v = serde_json::to_value(&ev).unwrap();
    assert_eq!(v["args"][0]["name"], "ada");
    assert_eq!(v["args"][0]["n"], 42);
    assert_eq!(v["callId"], 7);
    assert_eq!(v["windowId"], 1);
  }

  // --- permission_state_to_web_string ---

  #[test]
  fn permission_state_strings_match_web_api() {
    // These exact strings are surfaced to JS via `Notification.permission`
    // and `navigator.permissions.query(...).state`. The Web Permissions
    // API specifies "granted" / "denied" / "prompt" verbatim; renaming
    // any of them silently breaks feature detection in user code.
    assert_eq!(
      permission_state_to_web_string(PermissionState::Granted),
      "granted"
    );
    assert_eq!(
      permission_state_to_web_string(PermissionState::Denied),
      "denied"
    );
    assert_eq!(
      permission_state_to_web_string(PermissionState::Prompt),
      "prompt"
    );
    // "unsupported" is wef-specific (the spec has no such state); DESKTOP_JS
    // maps it to a TypeError throw from requestPermission.
    assert_eq!(
      permission_state_to_web_string(PermissionState::Unsupported),
      "unsupported"
    );
  }

  // --- dylib_magic_ok ---

  #[test]
  fn dylib_magic_accepts_native_formats() {
    // 32/64-bit Mach-O, both endians.
    assert!(dylib_magic_ok(&[0xFE, 0xED, 0xFA, 0xCE]));
    assert!(dylib_magic_ok(&[0xFE, 0xED, 0xFA, 0xCF]));
    assert!(dylib_magic_ok(&[0xCE, 0xFA, 0xED, 0xFE]));
    assert!(dylib_magic_ok(&[0xCF, 0xFA, 0xED, 0xFE]));
    // Fat Mach-O (universal binary).
    assert!(dylib_magic_ok(&[0xCA, 0xFE, 0xBA, 0xBE]));
    assert!(dylib_magic_ok(&[0xCA, 0xFE, 0xBA, 0xBF]));
    // ELF (Linux).
    assert!(dylib_magic_ok(&[0x7F, b'E', b'L', b'F']));
    // PE/COFF (Windows): starts with "MZ".
    assert!(dylib_magic_ok(b"MZ\x90\x00rest_of_pe_header"));
  }

  #[test]
  fn dylib_magic_rejects_non_binaries() {
    // Plain text — what a malformed bspatch result might decode to.
    assert!(!dylib_magic_ok(b"not a dylib"));
    // Empty / too short.
    assert!(!dylib_magic_ok(b""));
    assert!(!dylib_magic_ok(b"M"));
    assert!(!dylib_magic_ok(b"MZ"));
    assert!(!dylib_magic_ok(b"MZ\x90"));
    // Random gibberish.
    assert!(!dylib_magic_ok(&[0xDE, 0xAD, 0xBE, 0xEF]));
    // The wrapper check is the last line of defence — failure here means
    // we'd write garbage as the staged dylib.
  }

  // --- op_desktop_verify_ed25519 ---
  //
  // The op is the trust anchor for auto-update: the manifest is signed
  // and verified against a baked-in pubkey before any patch hash is
  // trusted. A regression that returns true for invalid input would
  // turn the whole auto-update path into "fetch + apply arbitrary code".

  fn keypair_from_seed(
    seed: &[u8; 32],
  ) -> (ed25519_dalek::SigningKey, ed25519_dalek::VerifyingKey) {
    let sk = ed25519_dalek::SigningKey::from_bytes(seed);
    let vk = sk.verifying_key();
    (sk, vk)
  }

  fn b64(bytes: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(bytes)
  }

  #[test]
  fn verify_ed25519_accepts_real_signature() {
    use ed25519_dalek::Signer;
    let (sk, vk) = keypair_from_seed(&[1u8; 32]);
    let message = b"deno desktop update v1.2.3";
    let sig = sk.sign(message);
    let ok =
      verify_ed25519_b64(&b64(&vk.to_bytes()), &b64(&sig.to_bytes()), message);
    assert!(ok, "signature over message must verify");
  }

  #[test]
  fn verify_ed25519_rejects_tampered_message() {
    use ed25519_dalek::Signer;
    let (sk, vk) = keypair_from_seed(&[1u8; 32]);
    let original = b"deno desktop update v1.2.3";
    let sig = sk.sign(original);
    // Flip a single byte of the message — a correct verifier must reject.
    let tampered = b"deno desktop update v1.2.4";
    let ok =
      verify_ed25519_b64(&b64(&vk.to_bytes()), &b64(&sig.to_bytes()), tampered);
    assert!(!ok, "tampered message must fail verification");
  }

  #[test]
  fn verify_ed25519_rejects_wrong_key() {
    use ed25519_dalek::Signer;
    let (sk_a, _) = keypair_from_seed(&[1u8; 32]);
    let (_, vk_b) = keypair_from_seed(&[2u8; 32]);
    let message = b"hi";
    let sig = sk_a.sign(message);
    let ok = verify_ed25519_b64(
      &b64(&vk_b.to_bytes()),
      &b64(&sig.to_bytes()),
      message,
    );
    assert!(!ok, "signature from key A must NOT verify under key B");
  }

  #[test]
  fn verify_ed25519_rejects_malformed_inputs() {
    let message = b"hi";
    // Empty key.
    assert!(!verify_ed25519_b64("", &b64(&[0u8; 64]), message));
    // Empty sig.
    assert!(!verify_ed25519_b64(&b64(&[0u8; 32]), "", message));
    // Wrong-length key.
    assert!(!verify_ed25519_b64(
      &b64(&[0u8; 31]),
      &b64(&[0u8; 64]),
      message
    ));
    assert!(!verify_ed25519_b64(
      &b64(&[0u8; 33]),
      &b64(&[0u8; 64]),
      message
    ));
    // Wrong-length sig.
    assert!(!verify_ed25519_b64(
      &b64(&[0u8; 32]),
      &b64(&[0u8; 63]),
      message
    ));
    assert!(!verify_ed25519_b64(
      &b64(&[0u8; 32]),
      &b64(&[0u8; 65]),
      message
    ));
    // Invalid base64.
    assert!(!verify_ed25519_b64(
      "!!! not base64 !!!",
      &b64(&[0u8; 64]),
      message
    ));
    assert!(!verify_ed25519_b64(
      &b64(&[0u8; 32]),
      "@@@ not base64 @@@",
      message
    ));
  }

  // --- register_bind_call + PendingBindResponses ---

  // The op_desktop_resolve_bind_call / op_desktop_reject_bind_call ops
  // both reduce to map.remove(&call_id).map(|tx| tx.send(...)). We
  // exercise the underlying state machine directly here — the ops
  // themselves are #[op2(fast)] wrappers and aren't callable from a
  // unit test, but the bug surface is the map manipulation, not the
  // tiny op2 wrapper.

  fn resolve(responses: &PendingBindResponses, id: u32, v: serde_json::Value) {
    if let Some(tx) = responses.0.lock().unwrap().remove(&id) {
      let _ = tx.send(Ok(v));
    }
  }

  fn reject(responses: &PendingBindResponses, id: u32, e: String) {
    if let Some(tx) = responses.0.lock().unwrap().remove(&id) {
      let _ = tx.send(Err(e));
    }
  }

  #[tokio::test]
  async fn bind_call_resolve_round_trips_value() {
    let responses = PendingBindResponses::new();
    let (tx, rx) = tokio::sync::oneshot::channel();
    let id = register_bind_call(&responses, tx);
    // The renderer resolves with a JSON value.
    resolve(&responses, id, serde_json::json!({"ok": true, "n": 42}));
    let v = rx.await.expect("oneshot recv").expect("Ok variant");
    assert_eq!(v["ok"], true);
    assert_eq!(v["n"], 42);
    // After resolve, the map entry is gone.
    assert!(
      responses.0.lock().unwrap().is_empty(),
      "responses map must be drained after resolve"
    );
  }

  #[tokio::test]
  async fn bind_call_reject_delivers_error_string() {
    let responses = PendingBindResponses::new();
    let (tx, rx) = tokio::sync::oneshot::channel();
    let id = register_bind_call(&responses, tx);
    reject(&responses, id, "binding threw".to_string());
    let e = rx.await.unwrap().expect_err("must be Err");
    assert_eq!(e, "binding threw");
    assert!(responses.0.lock().unwrap().is_empty());
  }

  #[test]
  fn bind_call_ids_are_unique_across_concurrent_registers() {
    // The id counter is a single AtomicU32 shared across calls.
    // Registering many at once must produce distinct ids — duplicates
    // would silently route a renderer response to the wrong pending
    // call.
    let responses = PendingBindResponses::new();
    let ids: Vec<u32> = (0..50)
      .map(|_| {
        let (tx, _rx) = tokio::sync::oneshot::channel();
        register_bind_call(&responses, tx)
      })
      .collect();
    let mut seen: std::collections::HashSet<u32> =
      std::collections::HashSet::new();
    for id in &ids {
      assert!(seen.insert(*id), "duplicate bind call id: {id}");
    }
    // All 50 are registered in the map.
    assert_eq!(responses.0.lock().unwrap().len(), 50);
  }

  #[test]
  fn bind_call_unknown_id_resolve_is_noop() {
    let responses = PendingBindResponses::new();
    // No entry registered — resolve with a random id must not panic
    // and must not affect any state.
    resolve(&responses, 999_999, serde_json::Value::Null);
    reject(&responses, 999_999, "x".to_string());
    assert!(responses.0.lock().unwrap().is_empty());
  }

  #[tokio::test]
  async fn bind_call_dropped_receiver_doesnt_panic_resolve() {
    // The renderer may give up on a bind call before the Deno side
    // resolves it (window closed). The resolve path uses `let _ = tx.send(...)`
    // explicitly because the receiver might be gone; we pin that
    // behaviour here so a future refactor doesn't reintroduce a
    // .unwrap() that would crash the runtime.
    let responses = PendingBindResponses::new();
    let (tx, rx) =
      tokio::sync::oneshot::channel::<Result<serde_json::Value, String>>();
    let id = register_bind_call(&responses, tx);
    drop(rx);
    resolve(&responses, id, serde_json::Value::Null);
    // If we reach this line without panicking, the test passes.
  }

  #[test]
  fn verify_ed25519_trims_whitespace_on_b64_inputs() {
    use ed25519_dalek::Signer;
    let (sk, vk) = keypair_from_seed(&[1u8; 32]);
    let message = b"trim me";
    let sig = sk.sign(message);
    let pk = format!("  {}\n", b64(&vk.to_bytes()));
    let sg = format!("\t{}\n", b64(&sig.to_bytes()));
    // The op trims the base64 before decoding so manifest JSON with
    // pretty-printed whitespace (or trailing newlines from `\n` literals)
    // still verifies.
    assert!(verify_ed25519_b64(&pk, &sg, message));
  }
}
