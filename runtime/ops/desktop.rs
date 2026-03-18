// Copyright 2018-2026 the Deno authors. MIT license.

//! Desktop window management ops for `deno compile --desktop`.
//!
//! These ops are included in the V8 snapshot so their external references
//! are stable. When `DesktopApi` is not present in OpState (non-desktop
//! builds), the ops silently no-op.

use std::borrow::Cow;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::AtomicU32;
use std::sync::atomic::Ordering;

use deno_core::FromV8;
use deno_core::OpState;
use deno_core::ToV8;
use deno_core::op2;
use deno_core::serde_json;
use deno_core::v8;

/// Channel for receiving menu click events in the Deno runtime.
pub struct MenuClickReceiver(pub tokio::sync::mpsc::UnboundedReceiver<String>);
pub struct MenuClickSender(pub tokio::sync::mpsc::UnboundedSender<String>);

pub fn create_menu_click_channel() -> (MenuClickSender, MenuClickReceiver) {
  let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
  (MenuClickSender(tx), MenuClickReceiver(rx))
}

#[derive(Debug, Clone, ToV8)]
pub struct KeyboardEventData {
  /// "keydown" or "keyup"
  pub r#type: &'static str,
  pub key: String,
  pub code: String,
  pub shift: bool,
  pub control: bool,
  pub alt: bool,
  pub meta: bool,
  pub repeat: bool,
}

pub struct KeyboardEventReceiver(
  pub tokio::sync::mpsc::UnboundedReceiver<KeyboardEventData>,
);
pub struct KeyboardEventSender(
  pub tokio::sync::mpsc::UnboundedSender<KeyboardEventData>,
);

pub fn create_keyboard_event_channel(
) -> (KeyboardEventSender, KeyboardEventReceiver) {
  let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
  (KeyboardEventSender(tx), KeyboardEventReceiver(rx))
}

/// A pending call from the webview to a bound Deno function.
pub struct PendingBindCall {
  pub name: String,
  pub args: serde_json::Value,
  pub response:
    tokio::sync::oneshot::Sender<Result<serde_json::Value, String>>,
}

pub struct BindCallReceiver(
  pub tokio::sync::mpsc::UnboundedReceiver<PendingBindCall>,
);
pub struct BindCallSender(
  pub tokio::sync::mpsc::UnboundedSender<PendingBindCall>,
);

pub fn create_bind_call_channel() -> (BindCallSender, BindCallReceiver) {
  let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
  (BindCallSender(tx), BindCallReceiver(rx))
}

pub struct PendingBindResponses(
  pub HashMap<
    u32,
    tokio::sync::oneshot::Sender<Result<serde_json::Value, String>>,
  >,
);

impl PendingBindResponses {
  pub fn new() -> Self {
    Self(HashMap::new())
  }
}

static BIND_CALL_COUNTER: AtomicU32 = AtomicU32::new(1);

/// Trait for desktop window operations. Implemented by the desktop
/// runtime (denort_desktop) to bridge to the WEF backend.
pub trait DesktopApi: Send + Sync + 'static {
  fn set_title(&self, title: &str);

  fn get_window_size(&self) -> (i32, i32);
  fn set_window_size(&self, width: i32, height: i32);

  fn get_window_position(&self) -> (i32, i32);
  fn set_window_position(&self, width: i32, height: i32);

  fn is_resizeable(&self) -> bool;
  fn set_resizeable(&self, resizeable: bool);

  fn is_always_on_top(&self) -> bool;
  fn set_always_on_top(&self, always_on_top: bool);
  fn is_visible(&self) -> bool;
  fn show(&self);
  fn hide(&self);
  fn focus(&self);

  fn bind(&self, name: &str);
  fn unbind(&self, name: &str);

  fn navigate(&self, url: &str);
  fn execute_js<'a>(&self, scope: &'a mut v8::PinScope<'a, '_>, script: &str) -> Pin<Box<dyn Future<Output = Result<v8::Local<'a, v8::Value>, v8::Local<'a, v8::Value>>> + 'a>>;
  fn quit(&self);
  fn set_application_menu(&self, template_json: &str);
}

struct BrowserWindow {
  api: Arc<dyn DesktopApi>,
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
    if let Some(options) = options {
      if let Some(title) = options.title {
        api.set_title(&title);
      }
      api.set_window_size(
        options.width.unwrap_or(800),
        options.height.unwrap_or(600),
      );
      if let Some(resizable) = options.resizable {
        api.set_resizeable(resizable);
      }
      if let Some(always_on_top) = options.always_on_top {
        api.set_always_on_top(always_on_top);
      }
    }

    let window = BrowserWindow { api };
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

  #[fast]
  fn bind(&self, #[string] name: &str) {
    self.api.bind(name);
  }

  #[fast]
  fn unbind(&self, #[string] name: &str) {
    self.api.unbind(name);
  }

  #[fast]
  fn set_title(&self, #[string] title: &str) {
    self.api.set_title(title);
  }

  fn get_size(&self) -> (i32, i32) {
    self.api.get_window_size()
  }

  #[fast]
  fn set_size(&self, #[smi] width: i32, #[smi] height: i32) {
    self.api.set_window_size(width, height);
  }

  fn get_position(&self) -> (i32, i32) {
    self.api.get_window_position()
  }

  #[fast]
  fn set_position(&self, #[smi] x: i32, #[smi] y: i32) {
    self.api.set_window_position(x, y);
  }

  #[fast]
  fn is_resizeable(&self) -> bool {
    self.api.is_resizeable()
  }

  #[fast]
  fn set_resizeable(&self, resizeable: bool) {
    self.api.set_resizeable(resizeable);
  }

  #[fast]
  fn is_always_on_top(&self) -> bool {
    self.api.is_always_on_top()
  }

  #[fast]
  fn set_always_on_top(&self, always_on_top: bool) {
    self.api.set_always_on_top(always_on_top);
  }

  #[fast]
  fn is_closed(&self) -> bool {
    todo!("implement")
  }

  #[fast]
  fn close(&self) {
    self.api.quit();
  }

  #[fast]
  fn is_visible(&self) -> bool {
    self.api.is_visible()
  }

  #[fast]
  fn show(&self) {
    self.api.show();
  }

  #[fast]
  fn hide(&self) {
    self.api.hide();
  }

  #[fast]
  fn focus(&self) {
    self.api.focus();
  }

  #[fast]
  fn navigate(&self, #[string] url: &str) {
    self.api.navigate(url);
  }

  #[fast]
  fn reload(&self) {
    todo!("implement")
  }

  #[fast]
  fn execute_js(&self, #[string] _script: &str) {
    todo!("implement execute_js — async + v8 scope borrowing needs a different approach")
    /*
        self.api.execute_js(scope, script).await
     */
  }

  #[fast]
  fn set_application_menu(&self, #[string] template_json: &str) {
    // TODO
    self.api.set_application_menu(template_json);
  }
}

#[derive(FromV8)]
struct BrowserWindowOptions {
  title: Option<String>,
  width: Option<i32>,
  height: Option<i32>,
  x: Option<u64>,
  y: Option<u64>,
  resizable: Option<bool>,
  always_on_top: Option<bool>,
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
#[string]
async fn op_desktop_recv_menu_click(
  state: std::rc::Rc<std::cell::RefCell<OpState>>,
) -> Option<String> {
  let rx = {
    let mut s = state.borrow_mut();
    s.try_take::<MenuClickReceiver>()
  };
  if let Some(mut rx) = rx {
    let result = rx.0.recv().await;
    state.borrow_mut().put(rx);
    result
  } else {
    // No receiver — desktop not initialized, pend forever
    std::future::pending().await
  }
}

#[op2]
async fn op_desktop_recv_keyboard_event(
  state: std::rc::Rc<std::cell::RefCell<OpState>>,
) -> Option<KeyboardEventData> {
  let rx = {
    let mut s = state.borrow_mut();
    s.try_take::<KeyboardEventReceiver>()
  };
  dbg!(rx.is_some());
  if let Some(mut rx) = rx {
    let result = rx.0.recv().await;
    dbg!(&result);
    state.borrow_mut().put(rx);
    result
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

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct BindCallInfo {
  name: String,
  args: serde_json::Value,
  call_id: u32,
}

#[op2]
#[serde]
async fn op_desktop_recv_bind_call(
  state: std::rc::Rc<std::cell::RefCell<OpState>>,
) -> Option<BindCallInfo> {
  let rx = {
    let mut s = state.borrow_mut();
    s.try_take::<BindCallReceiver>()
  };
  if let Some(mut rx) = rx {
    let result = rx.0.recv().await;
    state.borrow_mut().put(rx);
    if let Some(call) = result {
      let call_id = BIND_CALL_COUNTER.fetch_add(1, Ordering::Relaxed);
      {
        let mut s = state.borrow_mut();
        let responses = s.borrow_mut::<PendingBindResponses>();
        responses.0.insert(call_id, call.response);
      }
      Some(BindCallInfo {
        name: call.name,
        args: call.args,
        call_id,
      })
    } else {
      None
    }
  } else {
    std::future::pending().await
  }
}

#[op2]
fn op_desktop_resolve_bind_call(
  state: &mut OpState,
  #[smi] call_id: u32,
  #[serde] result: serde_json::Value,
) {
  if let Some(responses) = state.try_borrow_mut::<PendingBindResponses>() {
    if let Some(tx) = responses.0.remove(&call_id) {
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
  if let Some(responses) = state.try_borrow_mut::<PendingBindResponses>() {
    if let Some(tx) = responses.0.remove(&call_id) {
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


deno_core::extension!(
  deno_desktop,
  ops = [
    op_desktop_apply_patch,
    op_desktop_confirm_update,
    op_desktop_init,
    op_desktop_recv_menu_click,
    op_desktop_recv_keyboard_event,
    op_desktop_recv_bind_call,
    op_desktop_resolve_bind_call,
    op_desktop_reject_bind_call,
  ],
  objects = [BrowserWindow,],
);
