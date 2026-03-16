// Copyright 2018-2026 the Deno authors. MIT license.

//! Desktop window management ops for `deno compile --desktop`.
//!
//! These ops are included in the V8 snapshot so their external references
//! are stable. When `DesktopApi` is not present in OpState (non-desktop
//! builds), the ops silently no-op.

use std::borrow::Cow;
use std::sync::Arc;

use deno_core::{FromV8};
use deno_core::{ToV8};
use deno_core::OpState;
use deno_core::op2;
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

  fn bind(
    &self,
    name: &str,
    scope: &mut v8::PinScope<'_, '_>,
    this: v8::Global<v8::Object>,
    cb: v8::Local<v8::Function>,
  );
  fn unbind(&self, name: &str);

  fn navigate(&self, url: &str);
  fn execute_js(&self, scope: &mut v8::PinScope<'_, '_>, script: &str) -> Result<v8::Local<v8::Value>, v8::Local<v8::Value>>;
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

#[op2]
impl BrowserWindow {
  #[constructor]
  #[cppgc]
  fn new(
    state: &OpState,
    #[scoped] options: Option<BrowserWindowOptions>,
  ) -> BrowserWindow {
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
    }

    BrowserWindow { api }
  }

  #[fast]
  fn bind(
    &self,
    scope: &mut v8::PinScope<'_, '_>,
    #[this] this: v8::Global<v8::Object>,
    #[string] name: &str,
    cb: v8::Local<v8::Function>,
  ) {
    self.api.bind(name, scope, this, cb);
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

  fn execute_js(&self, scope: &mut v8::PinScope<'_, '_>, #[string] script: &str) -> Result<v8::Local<v8::Value>, v8::Local<v8::Value>> {
    self.api.execute_js(scope, script)
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
  if let Some(mut rx) = rx {
    let result = rx.0.recv().await;
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

deno_core::extension!(
  deno_desktop,
  ops = [
    op_desktop_apply_patch,
    op_desktop_confirm_update,
    op_desktop_recv_menu_click,
    op_desktop_recv_keyboard_event,
  ],
  objects = [BrowserWindow,],
);
