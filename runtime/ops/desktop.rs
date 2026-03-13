// Copyright 2018-2026 the Deno authors. MIT license.

//! Desktop window management ops for `deno compile --desktop`.
//!
//! These ops are included in the V8 snapshot so their external references
//! are stable. When `DesktopApi` is not present in OpState (non-desktop
//! builds), the ops silently no-op.

use std::sync::Arc;

use deno_core::OpState;
use deno_core::op2;

/// Channel for receiving menu click events in the Deno runtime.
pub struct MenuClickReceiver(pub tokio::sync::mpsc::UnboundedReceiver<String>);
pub struct MenuClickSender(pub tokio::sync::mpsc::UnboundedSender<String>);

pub fn create_menu_click_channel() -> (MenuClickSender, MenuClickReceiver) {
  let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
  (MenuClickSender(tx), MenuClickReceiver(rx))
}

/// Trait for desktop window operations. Implemented by the desktop
/// runtime (denort_desktop) to bridge to the WEF backend.
pub trait DesktopApi: Send + Sync + 'static {
  fn set_title(&self, title: &str);
  fn set_window_size(&self, width: i32, height: i32);
  fn navigate(&self, url: &str);
  fn execute_js(&self, script: &str);
  fn quit(&self);
  fn set_application_menu(&self, template_json: &str);
}

fn try_api(state: &OpState) -> Option<Arc<dyn DesktopApi>> {
  state.try_borrow::<Arc<dyn DesktopApi>>().map(|a| a.clone())
}

#[op2(fast)]
pub fn op_desktop_set_title(state: &mut OpState, #[string] title: &str) {
  if let Some(api) = try_api(state) {
    api.set_title(title);
  }
}

#[op2(fast)]
pub fn op_desktop_set_size(
  state: &mut OpState,
  #[smi] width: i32,
  #[smi] height: i32,
) {
  if let Some(api) = try_api(state) {
    api.set_window_size(width, height);
  }
}

#[op2(fast)]
pub fn op_desktop_navigate(state: &mut OpState, #[string] url: &str) {
  if let Some(api) = try_api(state) {
    api.navigate(url);
  }
}

#[op2(fast)]
pub fn op_desktop_execute_js(state: &mut OpState, #[string] script: &str) {
  if let Some(api) = try_api(state) {
    api.execute_js(script);
  }
}

#[op2(fast)]
pub fn op_desktop_close(state: &mut OpState) {
  if let Some(api) = try_api(state) {
    api.quit();
  }
}

#[op2(fast)]
pub fn op_desktop_set_application_menu(
  state: &mut OpState,
  #[string] template_json: &str,
) {
  if let Some(api) = try_api(state) {
    api.set_application_menu(template_json);
  }
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
    op_desktop_set_title,
    op_desktop_set_size,
    op_desktop_navigate,
    op_desktop_execute_js,
    op_desktop_close,
    op_desktop_set_application_menu,
    op_desktop_apply_patch,
    op_desktop_confirm_update,
    op_desktop_recv_menu_click,
  ],
);
