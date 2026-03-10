// Copyright 2018-2026 the Deno authors. MIT license.

//! Desktop window management extension for `deno compile --desktop`.
//!
//! Exposes `Deno.desktop.*` APIs that control the native window
//! (title, size, navigation, JS execution in the webview, quit).
//!
//! The actual implementation is provided via [`DesktopApi`] which is
//! placed into the OpState by the caller.

use deno_core::Extension;
use deno_core::OpState;
use deno_core::op2;

/// Trait for desktop window operations. Implemented by the desktop
/// runtime to bridge to the WEF backend.
pub trait DesktopApi: Send + Sync + 'static {
  fn set_title(&self, title: &str);
  fn set_window_size(&self, width: i32, height: i32);
  fn navigate(&self, url: &str);
  fn execute_js(&self, script: &str);
  fn quit(&self);
}

fn get_api(state: &OpState) -> &dyn DesktopApi {
  &**state.borrow::<Box<dyn DesktopApi>>()
}

#[op2(fast)]
fn op_desktop_set_title(state: &OpState, #[string] title: &str) {
  get_api(state).set_title(title);
}

#[op2(fast)]
fn op_desktop_set_window_size(state: &OpState, width: i32, height: i32) {
  get_api(state).set_window_size(width, height);
}

#[op2(fast)]
fn op_desktop_navigate(state: &OpState, #[string] url: &str) {
  get_api(state).navigate(url);
}

#[op2(fast)]
fn op_desktop_execute_js(state: &OpState, #[string] script: &str) {
  get_api(state).execute_js(script);
}

#[op2(fast)]
fn op_desktop_quit(state: &OpState) {
  get_api(state).quit();
}

deno_core::extension!(
  deno_desktop,
  ops = [
    op_desktop_set_title,
    op_desktop_set_window_size,
    op_desktop_navigate,
    op_desktop_execute_js,
    op_desktop_quit,
  ],
);

/// JS code that sets up `Deno.desktop.*` APIs.
pub const DESKTOP_JS: &str = r#"
(() => {
  const ops = Deno[Deno.internal].core.ops;

  class BrowserWindow {
    setTitle(title) { ops.op_desktop_set_title(title); }
    setSize(width, height) { ops.op_desktop_set_window_size(width, height); }
    navigate(url) { ops.op_desktop_navigate(url); }
    executeJs(script) { ops.op_desktop_execute_js(script); }
    close() { ops.op_desktop_quit(); }
  }

  Deno.desktop = {
    BrowserWindow,
    mainWindow: new BrowserWindow(),
  };
})();
"#;

/// Create the desktop extension with the given API implementation.
pub fn init_extension(api: Box<dyn DesktopApi>) -> Extension {
  let mut ext = self::deno_desktop::init();
  ext.op_state_fn = Some(Box::new(move |state: &mut deno_core::OpState| {
    state.put::<Box<dyn DesktopApi>>(api);
  }));
  ext
}
