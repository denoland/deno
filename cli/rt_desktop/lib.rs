// Copyright 2018-2026 the Deno authors. MIT license.

//! Desktop runtime for Deno (libdenort).
//!
//! This is a cdylib that exports the WEF C ABI (wef_runtime_init,
//! wef_runtime_start, wef_runtime_shutdown) and boots the full Deno
//! standalone runtime. A WEF backend (CEF, WebView, Servo) loads this
//! shared library and provides the browser/window layer.
//!
//! The user's code uses `Deno.serve()` or `export default { fetch }`
//! to serve an HTTP app. The desktop runtime starts it on a local port
//! and navigates the webview to it.

use std::borrow::Cow;
use std::collections::HashSet;
use std::env;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicU32;
use std::sync::atomic::Ordering;

use deno_core::anyhow::Context;
use deno_core::anyhow::bail;
use deno_core::error::AnyError;
use deno_core::serde_json;
use deno_core::v8;
use deno_lib::util::result::js_error_downcast_ref;
use deno_lib::version::otel_runtime_config;
use deno_runtime::fmt_errors::format_js_error;
use deno_terminal::colors;
use denort::desktop::DesktopApi;
use denort::run::RunOptions;

/// Allocate a random available port by binding to port 0.
fn allocate_random_port() -> std::io::Result<u16> {
  let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
  Ok(listener.local_addr()?.port())
}

/// WEF-backed implementation of [`denort::desktop::DesktopApi`].
struct WefDesktopApi {
  event_tx: tokio::sync::mpsc::UnboundedSender<
    deno_runtime::ops::desktop::DesktopEvent,
  >,
  pending_responses: deno_runtime::ops::desktop::PendingBindResponses,
  closed_windows: Arc<Mutex<HashSet<u32>>>,
}

impl WefDesktopApi {
  /// Set up all event handlers on a newly created window, wiring events
  /// into the shared event channel.
  fn setup_window_events(&self, window: wef::Window) -> wef::Window {
    let kb_tx = self.event_tx.clone();
    let mouse_click_tx = self.event_tx.clone();
    let mouse_move_tx = self.event_tx.clone();
    let wheel_tx = self.event_tx.clone();
    let cursor_tx = self.event_tx.clone();
    let focus_tx = self.event_tx.clone();
    let resize_tx = self.event_tx.clone();
    let move_tx = self.event_tx.clone();
    let close_tx = self.event_tx.clone();
    let closed_windows = self.closed_windows.clone();

    window
      .on_keyboard_event(move |ev| {
        let _ =
          kb_tx.send(deno_runtime::ops::desktop::DesktopEvent::KeyboardEvent {
            window_id: ev.window_id,
            r#type: match ev.state {
              wef::KeyState::Pressed => "keydown".to_string(),
              wef::KeyState::Released => "keyup".to_string(),
            },
            key: ev.key,
            code: ev.code,
            shift: ev.modifiers.shift,
            control: ev.modifiers.control,
            alt: ev.modifiers.alt,
            meta: ev.modifiers.meta,
            repeat: ev.repeat,
          });
      })
      .on_mouse_click(move |ev| {
        let _ = mouse_click_tx.send(
          deno_runtime::ops::desktop::DesktopEvent::MouseClick {
            window_id: ev.window_id,
            state: match ev.state {
              wef::MouseButtonState::Pressed => "pressed".to_string(),
              wef::MouseButtonState::Released => "released".to_string(),
            },
            button: match ev.button {
              wef::MouseButton::Left => 0,
              wef::MouseButton::Middle => 1,
              wef::MouseButton::Right => 2,
              wef::MouseButton::Back => 3,
              wef::MouseButton::Forward => 4,
              wef::MouseButton::Other(n) => n,
            },
            client_x: ev.x,
            client_y: ev.y,
            shift: ev.modifiers.shift,
            control: ev.modifiers.control,
            alt: ev.modifiers.alt,
            meta: ev.modifiers.meta,
            click_count: ev.click_count,
          },
        );
      })
      .on_mouse_move(move |ev| {
        let _ = mouse_move_tx.send(
          deno_runtime::ops::desktop::DesktopEvent::MouseMove {
            window_id: ev.window_id,
            client_x: ev.x,
            client_y: ev.y,
            shift: ev.modifiers.shift,
            control: ev.modifiers.control,
            alt: ev.modifiers.alt,
            meta: ev.modifiers.meta,
          },
        );
      })
      .on_wheel(move |ev| {
        let _ =
          wheel_tx.send(deno_runtime::ops::desktop::DesktopEvent::Wheel {
            window_id: ev.window_id,
            delta_x: ev.delta_x,
            delta_y: ev.delta_y,
            delta_mode: match ev.delta_mode {
              wef::WheelDeltaMode::Pixel => 0,
              wef::WheelDeltaMode::Line => 1,
              wef::WheelDeltaMode::Page => 2,
            },
            client_x: ev.x,
            client_y: ev.y,
            shift: ev.modifiers.shift,
            control: ev.modifiers.control,
            alt: ev.modifiers.alt,
            meta: ev.modifiers.meta,
          });
      })
      .on_cursor_enter_leave(move |ev| {
        let _ = cursor_tx.send(
          deno_runtime::ops::desktop::DesktopEvent::CursorEnterLeave {
            window_id: ev.window_id,
            entered: ev.entered,
            client_x: ev.x,
            client_y: ev.y,
            shift: ev.modifiers.shift,
            control: ev.modifiers.control,
            alt: ev.modifiers.alt,
            meta: ev.modifiers.meta,
          },
        );
      })
      .on_focused(move |ev| {
        let _ = focus_tx.send(
          deno_runtime::ops::desktop::DesktopEvent::FocusChanged {
            window_id: ev.window_id,
            focused: ev.focused,
          },
        );
      })
      .on_resize(move |ev| {
        let _ = resize_tx.send(
          deno_runtime::ops::desktop::DesktopEvent::WindowResize {
            window_id: ev.window_id,
            width: ev.width,
            height: ev.height,
          },
        );
      })
      .on_move(move |ev| {
        let _ =
          move_tx.send(deno_runtime::ops::desktop::DesktopEvent::WindowMove {
            window_id: ev.window_id,
            x: ev.x,
            y: ev.y,
          });
      })
      .on_close_requested(move |ev| {
        closed_windows.lock().unwrap().insert(ev.window_id);
        let _ = close_tx.send(
          deno_runtime::ops::desktop::DesktopEvent::CloseRequested {
            window_id: ev.window_id,
          },
        );
      })
  }
}

impl denort::desktop::DesktopApi for WefDesktopApi {
  fn create_window(&self, width: i32, height: i32) -> u32 {
    let window = wef::Window::new(width, height);
    let window = self.setup_window_events(window);
    window.id()
  }

  fn close_window(&self, window_id: u32) {
    self.closed_windows.lock().unwrap().insert(window_id);
    wef::Window::from_id(window_id).close();
  }

  fn is_closed(&self, window_id: u32) -> bool {
    self.closed_windows.lock().unwrap().contains(&window_id)
  }

  fn set_title(&self, window_id: u32, title: &str) {
    wef::Window::from_id(window_id).set_title(title);
  }

  fn get_window_size(&self, window_id: u32) -> (i32, i32) {
    wef::Window::from_id(window_id).get_size()
  }

  fn set_window_size(&self, window_id: u32, width: i32, height: i32) {
    wef::Window::from_id(window_id).set_size(width, height);
  }

  fn get_window_position(&self, window_id: u32) -> (i32, i32) {
    wef::Window::from_id(window_id).get_position()
  }

  fn set_window_position(&self, window_id: u32, x: i32, y: i32) {
    wef::Window::from_id(window_id).set_position(x, y);
  }

  fn is_resizable(&self, window_id: u32) -> bool {
    wef::Window::from_id(window_id).get_resizable()
  }

  fn set_resizable(&self, window_id: u32, resizable: bool) {
    wef::Window::from_id(window_id).set_resizable(resizable);
  }

  fn is_always_on_top(&self, window_id: u32) -> bool {
    wef::Window::from_id(window_id).get_always_on_top()
  }

  fn set_always_on_top(&self, window_id: u32, always_on_top: bool) {
    wef::Window::from_id(window_id).set_always_on_top(always_on_top);
  }

  fn is_visible(&self, window_id: u32) -> bool {
    wef::Window::from_id(window_id).get_visible()
  }

  fn show(&self, window_id: u32) {
    wef::Window::from_id(window_id).show();
  }

  fn hide(&self, window_id: u32) {
    wef::Window::from_id(window_id).hide();
  }

  fn focus(&self, window_id: u32) {
    wef::Window::from_id(window_id).focus();
  }

  fn open_devtools(&self, window_id: u32, renderer: bool, deno: bool) {
    if let Ok(mux) = env::var("DENO_DESKTOP_MUX_WS") {
      let (endpoint, frontend) = match (renderer, deno) {
        (true, true) => ("/unified", "inspector.html"),
        (true, false) => ("/cef", "inspector.html"),
        (false, true) => ("/deno", "js_app.html"),
        (false, false) => unreachable!(),
      };
      let url = format!("http://{mux}/devtools/{frontend}?ws={mux}{endpoint}");
      log::info!(
        "[desktop] openDevtools(renderer={renderer}, deno={deno}) → {url}"
      );
      let window = wef::Window::new(1200, 800);
      window.set_title("Deno Desktop DevTools");
      window.navigate(&url);
      let _ = self.setup_window_events(window);
      return;
    }
    wef::Window::from_id(window_id).open_devtools();
  }

  fn execute_js(
    &self,
    window_id: u32,
    script: &str,
    callback: Box<
      dyn FnOnce(
          Result<
            deno_runtime::ops::desktop::DesktopValue,
            deno_runtime::ops::desktop::DesktopValue,
          >,
        ) + Send
        + 'static,
    >,
  ) {
    wef::Window::from_id(window_id).execute_js(
      script,
      Some(move |result: Result<wef::Value, wef::Value>| {
        callback(match result {
          Ok(val) => Ok(wef_value_to_desktop_value(val)),
          Err(err) => Err(wef_value_to_desktop_value(err)),
        });
      }),
    );
  }

  fn bind(&self, window_id: u32, name: &str) {
    let tx = self.event_tx.clone();
    let responses = self.pending_responses.clone();
    let name_owned = name.to_string();
    wef::Window::from_id(window_id).add_binding_async(name, move |js_call| {
      let tx = tx.clone();
      let responses = responses.clone();
      let name = name_owned.clone();
      async move {
        let args: Vec<serde_json::Value> =
          js_call.args.iter().map(wef_value_to_json).collect();
        let (resp_tx, resp_rx) = tokio::sync::oneshot::channel();
        let call_id =
          deno_runtime::ops::desktop::register_bind_call(&responses, resp_tx);
        let event = deno_runtime::ops::desktop::DesktopEvent::BindCall {
          window_id: js_call.window_id,
          name,
          args: serde_json::Value::Array(args),
          call_id,
        };
        if tx.send(event).is_err() {
          js_call
            .reject(wef::Value::String("event channel closed".to_string()));
          return;
        }
        match resp_rx.await {
          Ok(Ok(result)) => {
            js_call.resolve(json_to_wef_value(&result));
          }
          Ok(Err(error)) => {
            js_call.reject(wef::Value::String(error));
          }
          Err(_) => {
            js_call.reject(wef::Value::String(
              "bind response channel dropped".to_string(),
            ));
          }
        }
      }
    });
  }

  fn unbind(&self, window_id: u32, name: &str) {
    wef::Window::from_id(window_id).unbind(name);
  }

  fn navigate(&self, window_id: u32, url: &str) {
    wef::Window::from_id(window_id).navigate(url);
  }

  fn quit(&self) {
    wef::quit();
  }

  fn set_application_menu(
    &self,
    window_id: u32,
    menu: Vec<denort::desktop::MenuItem>,
  ) {
    let menu = menu
      .into_iter()
      .map(desktop_menu_item_to_wef_menu_item)
      .collect::<Vec<_>>();
    let tx = self.event_tx.clone();
    wef::Window::from_id(window_id).set_menu(&menu, move |id: &str| {
      let _ = tx.send(deno_runtime::ops::desktop::DesktopEvent::AppMenuClick {
        window_id,
        id: id.to_string(),
      });
    });
  }

  fn show_context_menu(
    &self,
    window_id: u32,
    x: i32,
    y: i32,
    menu: Vec<denort::desktop::MenuItem>,
  ) {
    let menu = menu
      .into_iter()
      .map(desktop_menu_item_to_wef_menu_item)
      .collect::<Vec<_>>();
    let tx = self.event_tx.clone();
    wef::Window::from_id(window_id).show_context_menu(
      x,
      y,
      &menu,
      move |id: &str| {
        let _ =
          tx.send(deno_runtime::ops::desktop::DesktopEvent::ContextMenuClick {
            window_id,
            id: id.to_string(),
          });
      },
    );
  }

  fn get_raw_window_handle(
    &self,
    window_id: u32,
  ) -> (
    raw_window_handle::RawWindowHandle,
    raw_window_handle::RawDisplayHandle,
  ) {
    let window = wef::Window::from_id(window_id);
    let handle_type = window.get_window_handle_type();
    let raw_win = window.get_window_handle();
    let raw_display = window.get_display_handle();

    match handle_type {
      wef::WEF_WINDOW_HANDLE_APPKIT => {
        use raw_window_handle::*;
        let win = RawWindowHandle::AppKit(AppKitWindowHandle::new(
          std::ptr::NonNull::new(raw_win).expect("null window handle"),
        ));
        let display = RawDisplayHandle::AppKit(AppKitDisplayHandle::new());
        (win, display)
      }
      wef::WEF_WINDOW_HANDLE_WIN32 => {
        use raw_window_handle::*;
        let mut handle = Win32WindowHandle::new(
          std::num::NonZeroIsize::new(raw_win as isize)
            .expect("null window handle"),
        );
        handle.hinstance =
          std::num::NonZeroIsize::new(raw_display as isize).map(|v| v.into());
        let win = RawWindowHandle::Win32(handle);
        let display = RawDisplayHandle::Windows(WindowsDisplayHandle::new());
        (win, display)
      }
      wef::WEF_WINDOW_HANDLE_X11 => {
        use raw_window_handle::*;
        let win = RawWindowHandle::Xlib(XlibWindowHandle::new(raw_win as _));
        let display = RawDisplayHandle::Xlib(XlibDisplayHandle::new(
          std::ptr::NonNull::new(raw_display),
          0,
        ));
        (win, display)
      }
      wef::WEF_WINDOW_HANDLE_WAYLAND => {
        use raw_window_handle::*;
        let win = RawWindowHandle::Wayland(WaylandWindowHandle::new(
          std::ptr::NonNull::new(raw_win).expect("null window handle"),
        ));
        let display = RawDisplayHandle::Wayland(WaylandDisplayHandle::new(
          std::ptr::NonNull::new(raw_display).expect("null display handle"),
        ));
        (win, display)
      }
      _ => panic!("unknown window handle type: {handle_type}"),
    }
  }

  fn alert(&self, title: &str, message: &str) {
    wef::alert(title, message);
  }

  fn confirm(
    &self,
    title: &str,
    message: &str,
    callback: Box<dyn FnOnce(bool) + Send + 'static>,
  ) {
    wef::confirm(title, message, callback);
  }

  fn prompt(
    &self,
    title: &str,
    message: &str,
    default_value: &str,
    callback: Box<dyn FnOnce(Option<String>) + Send + 'static>,
  ) {
    wef::prompt(title, message, default_value, callback);
  }
}

fn desktop_menu_item_to_wef_menu_item(
  item: denort::desktop::MenuItem,
) -> wef::MenuItem {
  match item {
    denort::desktop::MenuItem::Item {
      label,
      id,
      accelerator,
      enabled,
    } => wef::MenuItem::Item {
      label,
      id,
      accelerator,
      enabled,
    },
    denort::desktop::MenuItem::Submenu { label, items } => {
      wef::MenuItem::Submenu {
        label,
        items: items
          .into_iter()
          .map(desktop_menu_item_to_wef_menu_item)
          .collect(),
      }
    }
    denort::desktop::MenuItem::Separator => wef::MenuItem::Separator,
    denort::desktop::MenuItem::Role { role } => wef::MenuItem::Role { role },
  }
}

#[allow(dead_code)]
fn wef_value_to_v8<'a>(
  scope: &v8::PinScope<'a, '_>,
  val: wef::Value,
) -> v8::Local<'a, v8::Value> {
  match val {
    wef::Value::Null => v8::null(scope).into(),
    wef::Value::Bool(bool) => v8::Boolean::new(scope, bool).into(),
    wef::Value::Int(int) => v8::Integer::new(scope, int).into(),
    wef::Value::Double(double) => v8::Number::new(scope, double).into(),
    wef::Value::String(str) => v8::String::new(scope, &str).unwrap().into(),
    wef::Value::List(list) => {
      let elements = list
        .into_iter()
        .map(|v| wef_value_to_v8(scope, v))
        .collect::<Vec<_>>();
      v8::Array::new_with_elements(scope, &elements).into()
    }
    wef::Value::Dict(dict) => {
      let mut names = Vec::with_capacity(dict.len());
      let mut values = Vec::with_capacity(dict.len());

      for (k, v) in dict {
        names.push(v8::String::new(scope, &k).unwrap().into());
        values.push(wef_value_to_v8(scope, v));
      }

      let prototype = v8::null(scope).into();
      v8::Object::with_prototype_and_properties(
        scope, prototype, &names, &values,
      )
      .into()
    }
    wef::Value::Binary(bin) => {
      let len = bin.len();
      let backing_store = v8::ArrayBuffer::new_backing_store_from_vec(bin);
      let backing_store = backing_store.make_shared();
      let ab = v8::ArrayBuffer::with_backing_store(scope, &backing_store);
      let uint8_array = v8::Uint8Array::new(scope, ab, 0, len).unwrap();
      uint8_array.into()
    }
  }
}

#[allow(dead_code)]
fn v8_to_wef_value<'a>(
  scope: &v8::PinScope<'a, '_>,
  val: v8::Local<'a, v8::Value>,
) -> wef::Value {
  if val.is_null_or_undefined() {
    wef::Value::Null
  } else if val.is_boolean() {
    wef::Value::Bool(val.boolean_value(scope))
  } else if val.is_int32() {
    wef::Value::Int(val.int32_value(scope).unwrap_or(0))
  } else if val.is_number() {
    wef::Value::Double(val.number_value(scope).unwrap_or(0.0))
  } else if val.is_string() {
    let s = val.to_rust_string_lossy(scope);
    wef::Value::String(s)
  } else if val.is_array_buffer_view() {
    let view: v8::Local<v8::ArrayBufferView> = val.try_into().unwrap();
    let len = view.byte_length();
    let mut buf = vec![0u8; len];
    view.copy_contents(&mut buf);
    wef::Value::Binary(buf)
  } else if val.is_array() {
    let arr: v8::Local<v8::Array> = val.try_into().unwrap();
    let len = arr.length();
    let mut list = Vec::with_capacity(len as usize);
    for i in 0..len {
      if let Some(elem) = arr.get_index(scope, i) {
        list.push(v8_to_wef_value(scope, elem));
      }
    }
    wef::Value::List(list)
  } else if val.is_object() {
    let obj: v8::Local<v8::Object> = val.try_into().unwrap();
    let mut map = std::collections::HashMap::new();
    if let Some(names) =
      obj.get_own_property_names(scope, v8::GetPropertyNamesArgs::default())
    {
      for i in 0..names.length() {
        if let Some(key) = names.get_index(scope, i) {
          let key_str = key.to_rust_string_lossy(scope);
          if let Some(value) = obj.get(scope, key) {
            map.insert(key_str, v8_to_wef_value(scope, value));
          }
        }
      }
    }
    wef::Value::Dict(map)
  } else {
    // Fallback: coerce to string
    wef::Value::String(val.to_rust_string_lossy(scope))
  }
}

/// Promote this dylib's symbols to the global symbol scope so that
/// native addons loaded via `dlopen` (e.g. next-swc.node) can resolve
/// NAPI function symbols from our library.
///
/// By default, WEF loads this dylib without `RTLD_GLOBAL`, so its symbols
/// are only visible within the dylib itself. NAPI addons use
/// `-undefined dynamic_lookup` (macOS) and expect NAPI symbols to be
/// in the global symbol table. Re-opening ourselves with `RTLD_GLOBAL`
/// promotes our exports to global scope.
#[cfg(unix)]
fn promote_dylib_symbols_to_global() {
  #[repr(C)]
  struct DlInfo {
    dli_fname: *const std::ffi::c_char,
    dli_fbase: *mut std::ffi::c_void,
    dli_sname: *const std::ffi::c_char,
    dli_saddr: *mut std::ffi::c_void,
  }
  unsafe extern "C" {
    fn dladdr(
      addr: *const std::ffi::c_void,
      info: *mut DlInfo,
    ) -> std::ffi::c_int;
    fn dlopen(
      path: *const std::ffi::c_char,
      flags: std::ffi::c_int,
    ) -> *mut std::ffi::c_void;
  }
  const RTLD_LAZY: std::ffi::c_int = 0x1;
  const RTLD_NOLOAD: std::ffi::c_int = 0x10;
  const RTLD_GLOBAL: std::ffi::c_int = 0x8;

  unsafe {
    let mut info: DlInfo = std::mem::zeroed();
    // Use a function in our dylib as a reference address
    let addr = promote_dylib_symbols_to_global as *const std::ffi::c_void;
    if dladdr(addr, &mut info) != 0 && !info.dli_fname.is_null() {
      // Re-open our own dylib with RTLD_GLOBAL to promote symbols
      dlopen(info.dli_fname, RTLD_LAZY | RTLD_NOLOAD | RTLD_GLOBAL);
    }
  }
}

/// Get the filesystem path of this dylib using `dladdr`.
#[cfg(unix)]
fn get_dylib_path() -> Option<PathBuf> {
  #[repr(C)]
  struct DlInfo {
    dli_fname: *const std::ffi::c_char,
    dli_fbase: *mut std::ffi::c_void,
    dli_sname: *const std::ffi::c_char,
    dli_saddr: *mut std::ffi::c_void,
  }
  unsafe extern "C" {
    fn dladdr(
      addr: *const std::ffi::c_void,
      info: *mut DlInfo,
    ) -> std::ffi::c_int;
  }
  unsafe {
    let mut info: DlInfo = std::mem::zeroed();
    let addr = get_dylib_path as *const std::ffi::c_void;
    if dladdr(addr, &mut info) != 0 && !info.dli_fname.is_null() {
      let c_str = std::ffi::CStr::from_ptr(info.dli_fname);
      Some(PathBuf::from(c_str.to_string_lossy().into_owned()))
    } else {
      None
    }
  }
}

/// Manages pending updates and rollback on startup.
///
/// Uses a sentinel file (`.update-ok`) to detect if the last update
/// booted successfully:
///
/// - `.update` exists → apply it (current → `.backup`, `.update` → current)
/// - `.backup` exists but `.update-ok` doesn't → last update crashed, rollback
/// - `.backup` exists and `.update-ok` exists → previous update succeeded, clean up
///
/// Returns `true` if a rollback occurred (so we can dispatch an event in JS).
fn apply_pending_update(dylib_path: &Path) -> bool {
  let ext = dylib_path.extension().unwrap_or_default().to_string_lossy();
  let update_path = dylib_path.with_extension(format!("{}.update", ext));
  let backup_path = dylib_path.with_extension(format!("{}.backup", ext));
  let sentinel_path = dylib_path.with_extension(format!("{}.update-ok", ext));

  if update_path.exists() {
    // New update pending — apply it.
    // Remove stale sentinel so we can detect if *this* update fails.
    let _ = std::fs::remove_file(&sentinel_path);
    let _ = std::fs::remove_file(&backup_path);
    if std::fs::rename(dylib_path, &backup_path).is_ok() {
      if std::fs::rename(&update_path, dylib_path).is_err() {
        // Failed to move update into place — rollback immediately.
        let _ = std::fs::rename(&backup_path, dylib_path);
      }
    }
    return false;
  }

  if backup_path.exists() && !sentinel_path.exists() {
    // Last update didn't write the sentinel → it crashed. Rollback.
    eprintln!("[desktop] Last update failed to start, rolling back...");
    let _ = std::fs::rename(&backup_path, dylib_path);
    return true;
  }

  if backup_path.exists() && sentinel_path.exists() {
    // Previous update booted fine — clean up backup and sentinel.
    let _ = std::fs::remove_file(&backup_path);
    let _ = std::fs::remove_file(&sentinel_path);
  }

  false
}

wef::main!(|| {
  // Apply any pending update before anything else.
  #[cfg(unix)]
  let update_rolled_back = {
    match std::panic::catch_unwind(|| {
      if let Some(ref dylib_path) = get_dylib_path() {
        eprintln!("[desktop] dylib path: {:?}", dylib_path);
        apply_pending_update(dylib_path)
      } else {
        eprintln!("[desktop] could not determine dylib path");
        false
      }
    }) {
      Ok(v) => v,
      Err(e) => {
        eprintln!("[desktop] update check panicked: {:?}", e);
        false
      }
    }
  };
  #[cfg(not(unix))]
  let update_rolled_back = false;

  // Make NAPI symbols visible to native addons (e.g. next-swc).
  #[cfg(unix)]
  promote_dylib_symbols_to_global();

  // Guard against re-entry: when a framework dev server (e.g. Next.js)
  // forks child/worker processes, they re-execute this dylib. Detect
  // forked workers and run them headless (no WEF window) by checking
  // for NODE_CHANNEL_FD (set by Node's child_process.fork()) or
  // NEXT_PRIVATE_WORKER (set by Next.js specifically).
  let args: Vec<_> = env::args_os().collect();
  let is_worker = env::var("NODE_CHANNEL_FD").is_ok()
    || env::var("NEXT_PRIVATE_WORKER").is_ok()
    || extract_fork_script_path(&args).is_some();
  if is_worker {
    run_headless_worker();
    return;
  }

  // Set up panic hook for desktop error reporting.  The error reporting
  // URL is only known after binary metadata is parsed (in run_desktop),
  // so the hook reads from a global that gets set later.
  {
    let orig_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
      use deno_runtime::ops::desktop::error_report_config;
      use deno_runtime::ops::desktop::send_error_report;

      if let Some((url, app_version)) = error_report_config() {
        let message = if let Some(s) =
          panic_info.payload().downcast_ref::<&str>()
        {
          (*s).to_string()
        } else if let Some(s) = panic_info.payload().downcast_ref::<String>() {
          s.clone()
        } else {
          "Deno runtime panicked".to_string()
        };

        let location = panic_info
          .location()
          .map(|l| format!("at {}:{}:{}", l.file(), l.line(), l.column()));

        let body = deno_core::serde_json::json!({
          "version": 1,
          "message": message,
          "stack": location,
          "appVersion": app_version,
          "platform": env::consts::OS,
          "arch": env::consts::ARCH,
        });
        send_error_report(url, &body.to_string());
      }

      eprintln!(
        "\n============================================================"
      );
      eprintln!("Deno has panicked. This is a bug in Deno. Please report this");
      eprintln!("at https://github.com/denoland/deno/issues/new.");
      eprintln!();
      eprintln!("Platform: {} {}", env::consts::OS, env::consts::ARCH);
      eprintln!();

      orig_hook(panic_info);
      deno_runtime::exit(1);
    }));
  }

  denort::init_logging(None, None);

  deno_runtime::deno_permissions::mark_standalone();

  rustls::crypto::aws_lc_rs::default_provider()
    .install_default()
    .unwrap();

  wef::set_js_namespace("bindings");

  let rt = tokio::runtime::Builder::new_current_thread()
    .enable_all()
    .build()
    .unwrap();

  rt.block_on(async {
    eprintln!("[desktop] run_desktop starting");
    match run_desktop(update_rolled_back).await {
      Ok(()) => eprintln!("[desktop] run_desktop completed OK"),
      Err(error) => {
        let is_js_error = js_error_downcast_ref(&error).is_some();
        let error_string = match js_error_downcast_ref(&error) {
          Some(js_error) => format_js_error(js_error, None),
          None => format!("{:?}", error),
        };
        log::error!(
          "{}: {}",
          colors::red_bold("error"),
          error_string.trim_start_matches("error: ")
        );
        // Only show native alert for non-JS errors (startup crashes).
        // JS errors are already handled by the error reporting JS listener.
        if !is_js_error {
          wef::alert(
            "Application Error",
            error_string.trim_start_matches("error: "),
          );
        }
      }
    }
  });
});

/// Run as a headless worker (no WEF window). Used when a framework dev
/// server forks child processes that re-execute this dylib.
fn run_headless_worker() {
  denort::init_logging(None, None);
  deno_runtime::deno_permissions::mark_standalone();
  rustls::crypto::aws_lc_rs::default_provider()
    .install_default()
    .unwrap();

  let rt = tokio::runtime::Builder::new_current_thread()
    .enable_all()
    .build()
    .unwrap();

  rt.block_on(async {
    let args: Vec<_> = env::args_os().collect();
    eprintln!("[worker] args: {:?}", args);
    eprintln!("[worker] cwd: {:?}", env::current_dir());

    // Detect if this is a child_process.fork() invocation.
    // fork() translates args to: ["run", "-A", "--unstable-...", "script.js", ...]
    // Extract the script path so the forked worker runs the correct module
    // instead of the embedded entrypoint.
    let fork_module = extract_fork_script_path(&args);
    eprintln!("[worker] fork_module: {:?}", fork_module);

    let data = match denort::binary::extract_standalone_with_finder(
      Cow::Owned(args),
      find_section_in_dylib,
    ) {
      Ok(data) => data,
      Err(e) => {
        log::error!("Worker failed to load standalone data: {:?}", e);
        return;
      }
    };

    denort::load_env_vars(&data.metadata.env_vars_from_env_file);

    let sys = if data.metadata.self_extracting.is_some() {
      // VFS should already be extracted by the parent process.
      // In dev mode, keep the source directory as CWD (inherited from parent).
      // In production mode, set CWD to extraction directory.
      if env::var("DENO_DESKTOP_DEV").is_err() {
        let _ = std::env::set_current_dir(&data.root_path);
      }
      denort::file_system::DenoRtSys::new_self_extracting(data.vfs.clone())
    } else {
      denort::file_system::DenoRtSys::new(data.vfs.clone())
    };

    let options = denort::run::RunOptions {
      override_main_module: fork_module,
      ..Default::default()
    };

    eprintln!("[worker] starting run_with_options");

    // Log worker output to a file for debugging since the parent may
    // call process.exit() and kill our stderr.
    let log_path = std::env::temp_dir().join("deno_desktop_worker.log");
    let log_msg = format!("[worker] log file: {:?}\n", log_path);
    eprint!("{}", log_msg);
    let _ = std::fs::write(&log_path, &log_msg);

    match denort::run::run_with_options(
      Arc::new(sys.clone()),
      sys,
      data,
      options,
    )
    .await
    {
      Ok(exit_code) => {
        let msg = format!(
          "[worker] run_with_options completed with exit code: {}\n",
          exit_code
        );
        eprint!("{}", msg);
        let _ = std::fs::OpenOptions::new()
          .append(true)
          .open(&log_path)
          .and_then(|mut f| std::io::Write::write_all(&mut f, msg.as_bytes()));
      }
      Err(error) => {
        let error_string = match js_error_downcast_ref(&error) {
          Some(js_error) => format_js_error(js_error, None),
          None => format!("{:?}", error),
        };
        let msg =
          format!("[worker] run_with_options error: {}\n", error_string);
        eprint!("{}", msg);
        let _ = std::fs::OpenOptions::new()
          .append(true)
          .open(&log_path)
          .and_then(|mut f| std::io::Write::write_all(&mut f, msg.as_bytes()));
        log::error!(
          "{}: {}",
          colors::red_bold("error"),
          error_string.trim_start_matches("error: ")
        );
      }
    }
    eprintln!("[worker] block_on finished");
  });
  eprintln!("[worker] run_headless_worker returning");
}

/// Extract the script path from fork'd process arguments.
///
/// When `child_process.fork(scriptPath)` is called, the args are translated
/// to Deno CLI args: `["<exe>", "run", "-A", "--unstable-...", "script.js", ...]`
/// This function finds the script path (first non-flag arg after "run").
fn extract_fork_script_path(
  args: &[std::ffi::OsString],
) -> Option<deno_core::url::Url> {
  let args: Vec<String> = args
    .iter()
    .filter_map(|a| a.to_str().map(String::from))
    .collect();

  // Skip argv[0] (the executable), expect "run" as the subcommand
  if args.len() < 3 || args[1] != "run" {
    return None;
  }

  // Find the first arg after "run" that isn't a flag
  for arg in &args[2..] {
    if arg.starts_with('-') {
      continue;
    }
    // This is the script path
    let path = PathBuf::from(arg);
    let path = if path.is_absolute() {
      path
    } else {
      env::current_dir().ok()?.join(path)
    };
    return deno_core::url::Url::from_file_path(path).ok();
  }
  None
}

/// Convert a wef::Value to a DesktopValue for direct V8 conversion.
fn wef_value_to_desktop_value(
  v: wef::Value,
) -> deno_runtime::ops::desktop::DesktopValue {
  use deno_runtime::ops::desktop::DesktopValue;
  match v {
    wef::Value::Null => DesktopValue::Null,
    wef::Value::Bool(b) => DesktopValue::Bool(b),
    wef::Value::Int(i) => DesktopValue::Int(i),
    wef::Value::Double(d) => DesktopValue::Double(d),
    wef::Value::String(s) => DesktopValue::String(s),
    wef::Value::List(l) => DesktopValue::List(
      l.into_iter().map(wef_value_to_desktop_value).collect(),
    ),
    wef::Value::Dict(d) => DesktopValue::Dict(
      d.into_iter()
        .map(|(k, v)| (k, wef_value_to_desktop_value(v)))
        .collect(),
    ),
    wef::Value::Binary(b) => DesktopValue::Binary(b),
  }
}

/// Convert a wef::Value to a serde_json::Value for channel transport.
fn wef_value_to_json(v: &wef::Value) -> serde_json::Value {
  match v {
    wef::Value::Null => serde_json::Value::Null,
    wef::Value::Bool(b) => serde_json::Value::Bool(*b),
    wef::Value::Int(i) => serde_json::json!(*i),
    wef::Value::Double(d) => serde_json::json!(*d),
    wef::Value::String(s) => serde_json::Value::String(s.clone()),
    wef::Value::List(l) => {
      serde_json::Value::Array(l.iter().map(wef_value_to_json).collect())
    }
    wef::Value::Dict(d) => {
      let mut map = serde_json::Map::new();
      for (k, v) in d {
        map.insert(k.clone(), wef_value_to_json(v));
      }
      serde_json::Value::Object(map)
    }
    wef::Value::Binary(b) => serde_json::json!(b),
  }
}

/// Convert a serde_json::Value to a wef::Value for the menu template.
fn json_to_wef_value(v: &serde_json::Value) -> wef::Value {
  match v {
    serde_json::Value::Null => wef::Value::Null,
    serde_json::Value::Bool(b) => wef::Value::Bool(*b),
    serde_json::Value::Number(n) => {
      if let Some(i) = n.as_i64() {
        wef::Value::Int(i as i32)
      } else {
        wef::Value::Double(n.as_f64().unwrap_or(0.0))
      }
    }
    serde_json::Value::String(s) => wef::Value::String(s.clone()),
    serde_json::Value::Array(arr) => {
      wef::Value::List(arr.iter().map(json_to_wef_value).collect())
    }
    serde_json::Value::Object(obj) => {
      let mut map = std::collections::HashMap::new();
      for (k, v) in obj {
        map.insert(k.clone(), json_to_wef_value(v));
      }
      wef::Value::Dict(map)
    }
  }
}

/// Find the embedded data section in this dylib (not the main executable).
fn find_section_in_dylib() -> Result<&'static [u8], AnyError> {
  match libsui::find_section_in_current_image("d3n0l4nd")
    .context("Failed reading standalone binary section from dylib.")
  {
    Ok(Some(data)) => Ok(data),
    Ok(None) => {
      bail!("Could not find standalone binary section in dylib.")
    }
    Err(err) => Err(err),
  }
}

async fn run_desktop(update_rolled_back: bool) -> Result<(), AnyError> {
  let args: Vec<_> = env::args_os().collect();
  let data = denort::binary::extract_standalone_with_finder(
    Cow::Owned(args),
    find_section_in_dylib,
  )?;

  // Make the error reporting URL available to the panic hook.
  if let Some(ref url) = data.metadata.error_reporting_url {
    deno_runtime::ops::desktop::set_error_report_config(
      url.clone(),
      data.metadata.app_version.clone(),
    );
  }

  let sys = if data.metadata.self_extracting.is_some() {
    denort::binary::extract_vfs_to_disk(&data.vfs, &data.root_path)?;
    // Set CWD to extraction directory so frameworks like Next.js
    // can find their build output (e.g. .next/) relative to CWD.
    std::env::set_current_dir(&data.root_path)?;
    denort::file_system::DenoRtSys::new_self_extracting(data.vfs.clone())
  } else {
    denort::file_system::DenoRtSys::new(data.vfs.clone())
  };

  deno_runtime::deno_telemetry::init(
    &sys,
    otel_runtime_config(),
    data.metadata.otel_config.clone(),
  )?;
  denort::init_logging(
    data.metadata.log_level,
    Some(data.metadata.otel_config.clone()),
  );
  denort::load_env_vars(&data.metadata.env_vars_from_env_file);

  let desktop_serve_port = allocate_random_port()?;

  // Wire up the Deno-side inspector when launched under
  // `deno desktop --inspect[-brk|-wait]`. The parent process binds the
  // user-visible port and runs a multiplexer that fronts both this
  // inspector and the CEF renderer's debug port; we just listen on the
  // internal port that the parent allocated for us.
  let inspect_internal_port = env::var("DENO_DESKTOP_INSPECT_INTERNAL_PORT")
    .ok()
    .and_then(|s| s.parse::<std::net::SocketAddr>().ok());
  let inspect_brk = env::var("DENO_DESKTOP_INSPECT_BRK").is_ok();
  let inspect_wait = env::var("DENO_DESKTOP_INSPECT_WAIT").is_ok();
  if let Some(addr) = inspect_internal_port {
    deno_runtime::deno_inspector_server::create_inspector_server(
      addr,
      "deno-desktop",
      // Don't print the ws:// URL ourselves — DevTools attaches via the
      // parent mux's user-visible port.
      deno_runtime::deno_inspector_server::InspectPublishUid {
        console: false,
        http: true,
      },
    )?;
    log::debug!("[desktop] inspector server bound on {addr}");
  }

  // Set DENO_SERVE_ADDRESS so Deno.serve() and Node http servers
  // automatically bind to the desktop port.
  #[allow(clippy::undocumented_unsafe_blocks)]
  unsafe {
    std::env::set_var(
      "DENO_SERVE_ADDRESS",
      format!("tcp:127.0.0.1:{}", desktop_serve_port),
    );
  }

  // Enable HMR if DENO_DESKTOP_HMR is set to a directory path
  // (set by `deno compile --desktop --hmr`).
  let hmr_watch_dir = env::var("DENO_DESKTOP_HMR").ok().map(PathBuf::from);

  // Framework dev servers handle their own HMR via websocket.
  // For non-framework apps, V8-level HMR reloads the webview.
  let is_framework_dev = env::var("DENO_DESKTOP_DEV").is_ok();

  // In dev mode, restore CWD to the source directory so the framework
  // dev server watches the original source files, not the extracted VFS.
  if is_framework_dev {
    if let Ok(source_dir) = env::var("DENO_DESKTOP_HMR") {
      std::env::set_current_dir(&source_dir)?;
    }
  }

  // Shared initial window ID for navigate_fut and HMR reload.
  let initial_window_id = Arc::new(AtomicU32::new(0));
  let initial_window_id_for_hmr = initial_window_id.clone();
  let initial_window_id_for_navigate = initial_window_id.clone();

  let hmr_on_reload: Option<denort::hmr::HmrReloadCallback> =
    if hmr_watch_dir.is_some() && !is_framework_dev {
      Some(Box::new(move || {
        let id = initial_window_id_for_hmr.load(Ordering::Acquire);
        if id != 0 {
          wef::Window::from_id(id)
            .execute_js::<fn(Result<wef::Value, wef::Value>)>(
              "location.reload()",
              None,
            );
        }
      }))
    } else {
      None
    };

  // Desktop extension: provides Deno.desktop.* APIs + auto-update
  #[cfg(unix)]
  let auto_update_state = get_dylib_path().map(|p| {
    denort::desktop::AutoUpdateState {
      dylib_path: p,
      app_version: data.metadata.app_version.clone(),
      rolled_back: update_rolled_back, // from wef::main! startup check
    }
  });
  #[cfg(not(unix))]
  let auto_update_state: Option<denort::desktop::AutoUpdateState> = None;

  let auto_update_version = auto_update_state
    .as_ref()
    .and_then(|s| s.app_version.clone());
  let auto_update_rolled_back =
    auto_update_state.as_ref().is_some_and(|s| s.rolled_back);

  let run_opts = RunOptions {
    auto_serve: true,
    serve_port: Some(desktop_serve_port),
    serve_host: Some("127.0.0.1".to_string()),
    hmr_watch_dir: if is_framework_dev {
      None
    } else {
      hmr_watch_dir
    },
    hmr_on_reload,
    op_state_init: Some(Box::new(move |state| {
      let (event_tx, event_rx) =
        denort::desktop::create_desktop_event_channel();
      let pending_responses = denort::desktop::PendingBindResponses::new();
      let api = WefDesktopApi {
        event_tx: event_tx.0.clone(),
        pending_responses: pending_responses.clone(),
        closed_windows: Arc::new(Mutex::new(HashSet::new())),
      };

      // Create the initial window and wire up event handlers.
      let window_id = api.create_window(800, 600);
      initial_window_id.store(window_id, Ordering::Release);

      denort::desktop::init_desktop_state(
        state,
        Box::new(api),
        auto_update_state,
      );
      state.put(event_rx);
      state.put(event_tx);
      state.put(pending_responses);
      state.put(denort::desktop::InitialWindowId(std::sync::Mutex::new(
        Some(window_id),
      )));
    })),
    override_main_module: None,
    auto_update_version,
    auto_update_rolled_back,
    error_reporting_url: data.metadata.error_reporting_url.clone(),
    inspect_brk,
    inspect_wait,
    is_inspecting: inspect_internal_port.is_some(),
  };

  // Run the Deno runtime and WEF event loop concurrently.
  // We spawn the runtime first, wait for the server to be ready,
  // then navigate the webview.
  let url = format!("http://127.0.0.1:{}", desktop_serve_port);
  eprintln!("[desktop] starting runtime and wef event loop");
  let run_fut =
    denort::run::run_with_options(Arc::new(sys.clone()), sys, data, run_opts);
  let wef_fut = wef::run();

  // Wait for the server to be ready, then navigate the initial window.
  // Do a full HTTP request instead of just a TCP connect — frameworks
  // like Vite accept connections before they're ready to serve.
  let wait_for_debugger = inspect_brk || inspect_wait;
  let mux_addr = env::var("DENO_DESKTOP_MUX_WS").ok();
  let navigate_fut = async move {
    use tokio::io::AsyncReadExt;
    use tokio::io::AsyncWriteExt;

    // When --inspect-wait or --inspect-brk: block until a DevTools
    // client has connected to the mux. This prevents the renderer
    // from racing ahead while the developer is still opening DevTools.
    if wait_for_debugger {
      if let Some(ref mux) = mux_addr {
        eprintln!("[desktop] Waiting for debugger to attach on ws://{mux} …");
        loop {
          if let Ok(mut stream) =
            tokio::net::TcpStream::connect(mux.as_str()).await
          {
            let req = format!(
              "GET /debugger-attached HTTP/1.1\r\nHost: {mux}\r\nConnection: close\r\n\r\n",
            );
            if stream.write_all(req.as_bytes()).await.is_ok() {
              let mut buf = vec![0u8; 128];
              if let Ok(n) = stream.read(&mut buf).await {
                let resp = String::from_utf8_lossy(&buf[..n]);
                if resp.starts_with("HTTP/1.1 200")
                  || resp.starts_with("HTTP/1.0 200")
                {
                  eprintln!("[desktop] Debugger attached");
                  break;
                }
              }
            }
          }
          tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        }
      }
    }

    for i in 0..60 {
      if let Ok(mut stream) =
        tokio::net::TcpStream::connect(("127.0.0.1", desktop_serve_port)).await
      {
        let req = format!(
          "GET / HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nConnection: close\r\n\r\n",
          desktop_serve_port
        );
        if stream.write_all(req.as_bytes()).await.is_ok() {
          let mut buf = vec![0u8; 256];
          if let Ok(n) = stream.read(&mut buf).await {
            let response = String::from_utf8_lossy(&buf[..n]);
            if response.starts_with("HTTP/1.1 2")
              || response.starts_with("HTTP/1.1 3")
              || response.starts_with("HTTP/1.0 2")
              || response.starts_with("HTTP/1.0 3")
            {
              eprintln!(
                "[desktop] Server ready after {} attempts, navigating to {}",
                i + 1,
                &url
              );
              let id = initial_window_id_for_navigate.load(Ordering::Acquire);
              wef::Window::from_id(id).navigate(&url);
              return;
            }
          }
        }
      }
      tokio::time::sleep(std::time::Duration::from_millis(250)).await;
    }
    log::warn!("Server not ready after 15s, navigating anyway");
    let id = initial_window_id_for_navigate.load(Ordering::Acquire);
    wef::Window::from_id(id).navigate(&url);
  };

  tokio::spawn(navigate_fut);

  tokio::select! {
    result = run_fut => {
      match result {
        Ok(exit_code) => {
          eprintln!("[desktop] Deno runtime exited with code {}", exit_code);
        }
        Err(err) => {
          eprintln!("[desktop] Deno runtime error: {:?}", err);
          return Err(err);
        }
      }
    }
    _ = wef_fut => {
      eprintln!("[desktop] WEF event loop ended (window closed)");
    }
  }

  Ok(())
}
