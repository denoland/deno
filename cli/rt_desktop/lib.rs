// Copyright 2018-2026 the Deno authors. MIT license.

//! Desktop runtime for Deno (libdenort).
//!
//! This is a cdylib that exports the Laufey C ABI (laufey_runtime_init,
//! laufey_runtime_start, laufey_runtime_shutdown) and boots the full Deno
//! standalone runtime. A Laufey backend (CEF, WebView, Servo) loads this
//! shared library and provides the browser/window layer.
//!
//! The user's code uses `Deno.serve()` or `export default { fetch }`
//! to serve an HTTP app. The desktop runtime starts it on a local port
//! and navigates the webview to it.

use std::borrow::Cow;
use std::collections::HashMap;
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
use deno_lib::util::net::allocate_random_port;
use deno_lib::util::result::js_error_downcast_ref;
use deno_lib::version::otel_runtime_config;
use deno_runtime::fmt_errors::format_js_error;
use deno_terminal::colors;
use denort::desktop::DesktopApi;
use denort::run::RunOptions;

/// Compile-time check: the laufey crate we're linking against must use the
/// same C ABI version as the prebuilt backend we ship. If laufey bumps
/// `LAUFEY_API_VERSION` without our side updating, `init_api` would reject the
/// backend at startup with `-2`. Catching the mismatch at `cargo build` time
/// makes the failure mode obvious instead of "the desktop app silently won't
/// launch".
const _: () = assert!(
  laufey::LAUFEY_API_VERSION == 25,
  "LAUFEY_API_VERSION mismatch: update this assert and the prebuilt backend release pin in cli/tools/desktop.rs when laufey bumps its API version",
);

/// Laufey-backed implementation of [`denort::desktop::DesktopApi`].
struct WefDesktopApi {
  event_tx: deno_runtime::ops::desktop::DesktopEventTx,
  pending_responses: deno_runtime::ops::desktop::PendingBindResponses,
  closed_windows: Arc<Mutex<HashSet<u32>>>,
  /// IDs of every window currently displayed. Shared with the HMR reload
  /// callback so it can refresh all windows, not just the initial one.
  open_windows: Arc<Mutex<HashSet<u32>>>,
  trays: Arc<Mutex<HashMap<u32, laufey::TrayIcon>>>,
  notifications: Arc<Mutex<HashMap<u32, laufey::NotificationHandle>>>,
  /// Singleton for the unified-mux DevTools window. Without this, every
  /// `openDevtools()` call would spawn another DevTools window.
  devtools_window: Mutex<Option<u32>>,
}

impl WefDesktopApi {
  /// Set up all event handlers on a newly created window, wiring events
  /// into the shared event channel.
  fn setup_window_events(&self, window: laufey::Window) -> laufey::Window {
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
    let open_windows_on_close = self.open_windows.clone();

    window
      .on_keyboard_event(move |ev| {
        let _ = kb_tx.try_send(
          deno_runtime::ops::desktop::DesktopEvent::KeyboardEvent {
            window_id: ev.window_id,
            r#type: match ev.state {
              laufey::KeyState::Pressed => "keydown".to_string(),
              laufey::KeyState::Released => "keyup".to_string(),
            },
            key: ev.key,
            code: ev.code,
            shift: ev.modifiers.shift,
            control: ev.modifiers.control,
            alt: ev.modifiers.alt,
            meta: ev.modifiers.meta,
            repeat: ev.repeat,
          },
        );
      })
      .on_mouse_click(move |ev| {
        let _ = mouse_click_tx.try_send(
          deno_runtime::ops::desktop::DesktopEvent::MouseClick {
            window_id: ev.window_id,
            state: match ev.state {
              laufey::MouseButtonState::Pressed => "pressed".to_string(),
              laufey::MouseButtonState::Released => "released".to_string(),
            },
            button: match ev.button {
              laufey::MouseButton::Left => 0,
              laufey::MouseButton::Middle => 1,
              laufey::MouseButton::Right => 2,
              laufey::MouseButton::Back => 3,
              laufey::MouseButton::Forward => 4,
              laufey::MouseButton::Other(n) => n,
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
        let _ = mouse_move_tx.try_send(
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
          wheel_tx.try_send(deno_runtime::ops::desktop::DesktopEvent::Wheel {
            window_id: ev.window_id,
            delta_x: ev.delta_x,
            delta_y: ev.delta_y,
            delta_mode: match ev.delta_mode {
              laufey::WheelDeltaMode::Pixel => 0,
              laufey::WheelDeltaMode::Line => 1,
              laufey::WheelDeltaMode::Page => 2,
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
        let _ = cursor_tx.try_send(
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
        let _ = focus_tx.try_send(
          deno_runtime::ops::desktop::DesktopEvent::FocusChanged {
            window_id: ev.window_id,
            focused: ev.focused,
          },
        );
      })
      .on_resize(move |ev| {
        let _ = resize_tx.try_send(
          deno_runtime::ops::desktop::DesktopEvent::WindowResize {
            window_id: ev.window_id,
            width: ev.width,
            height: ev.height,
          },
        );
      })
      .on_move(move |ev| {
        let _ = move_tx.try_send(
          deno_runtime::ops::desktop::DesktopEvent::WindowMove {
            window_id: ev.window_id,
            x: ev.x,
            y: ev.y,
          },
        );
      })
      .on_close_requested(move |ev| {
        closed_windows.lock().unwrap().insert(ev.window_id);
        open_windows_on_close.lock().unwrap().remove(&ev.window_id);
        let _ = close_tx.try_send(
          deno_runtime::ops::desktop::DesktopEvent::CloseRequested {
            window_id: ev.window_id,
          },
        );
      })
  }
}

impl denort::desktop::DesktopApi for WefDesktopApi {
  fn create_window(
    &self,
    width: i32,
    height: i32,
    frameless: bool,
    no_activate: bool,
    transparent_titlebar: bool,
  ) -> u32 {
    let window = laufey::Window::new_with_options(
      width,
      height,
      laufey::WindowOptions {
        frameless,
        no_activate,
        transparent_titlebar,
      },
    );
    let window = self.setup_window_events(window);
    let id = window.id();
    self.open_windows.lock().unwrap().insert(id);
    id
  }

  fn close_window(&self, window_id: u32) {
    self.closed_windows.lock().unwrap().insert(window_id);
    self.open_windows.lock().unwrap().remove(&window_id);
    laufey::Window::from_id(window_id).close();
  }

  fn is_closed(&self, window_id: u32) -> bool {
    self.closed_windows.lock().unwrap().contains(&window_id)
  }

  fn set_title(&self, window_id: u32, title: &str) {
    laufey::Window::from_id(window_id).set_title(title);
  }

  fn get_window_size(&self, window_id: u32) -> (i32, i32) {
    laufey::Window::from_id(window_id).get_size()
  }

  fn set_window_size(&self, window_id: u32, width: i32, height: i32) {
    laufey::Window::from_id(window_id).set_size(width, height);
  }

  fn get_window_position(&self, window_id: u32) -> (i32, i32) {
    laufey::Window::from_id(window_id).get_position()
  }

  fn set_window_position(&self, window_id: u32, x: i32, y: i32) {
    laufey::Window::from_id(window_id).set_position(x, y);
  }

  fn is_resizable(&self, window_id: u32) -> bool {
    laufey::Window::from_id(window_id).get_resizable()
  }

  fn set_resizable(&self, window_id: u32, resizable: bool) {
    laufey::Window::from_id(window_id).set_resizable(resizable);
  }

  fn is_always_on_top(&self, window_id: u32) -> bool {
    laufey::Window::from_id(window_id).get_always_on_top()
  }

  fn set_always_on_top(&self, window_id: u32, always_on_top: bool) {
    laufey::Window::from_id(window_id).set_always_on_top(always_on_top);
  }

  fn is_visible(&self, window_id: u32) -> bool {
    laufey::Window::from_id(window_id).get_visible()
  }

  fn show(&self, window_id: u32) {
    laufey::Window::from_id(window_id).show();
  }

  fn hide(&self, window_id: u32) {
    laufey::Window::from_id(window_id).hide();
  }

  fn focus(&self, window_id: u32) {
    laufey::Window::from_id(window_id).focus();
  }

  fn open_devtools(&self, window_id: u32, renderer: bool, deno: bool) {
    if let Ok(mux) = env::var("DENO_DESKTOP_MUX_WS") {
      // Reuse an existing DevTools window when one is already open, so
      // repeated `openDevtools()` calls don't pile up windows.
      if let Some(id) = *self.devtools_window.lock().unwrap() {
        if !self.closed_windows.lock().unwrap().contains(&id) {
          laufey::Window::from_id(id).focus();
          return;
        }
      }

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
      let window = laufey::Window::new(1200, 800);
      window.set_title("Deno Desktop DevTools");
      window.navigate(&url);
      let window = self.setup_window_events(window);
      let id = window.id();
      // Track for HMR reload + the singleton check above.
      self.open_windows.lock().unwrap().insert(id);
      *self.devtools_window.lock().unwrap() = Some(id);
      return;
    }
    laufey::Window::from_id(window_id).open_devtools();
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
    laufey::Window::from_id(window_id).execute_js(
      script,
      Some(move |result: Result<laufey::Value, laufey::Value>| {
        callback(match result {
          Ok(val) => Ok(laufey_value_to_desktop_value(val)),
          Err(err) => Err(laufey_value_to_desktop_value(err)),
        });
      }),
    );
  }

  fn bind(&self, window_id: u32, name: &str) {
    let tx = self.event_tx.clone();
    let responses = self.pending_responses.clone();
    let name_owned = name.to_string();
    laufey::Window::from_id(window_id).add_binding_async(
      name,
      move |js_call| {
        let tx = tx.clone();
        let responses = responses.clone();
        let name = name_owned.clone();
        async move {
          let args: Vec<serde_json::Value> =
            js_call.args.iter().map(laufey_value_to_json).collect();
          let (resp_tx, resp_rx) = tokio::sync::oneshot::channel();
          let call_id =
            deno_runtime::ops::desktop::register_bind_call(&responses, resp_tx);
          let event = deno_runtime::ops::desktop::DesktopEvent::BindCall {
            window_id: js_call.window_id,
            name,
            args: serde_json::Value::Array(args),
            call_id,
          };
          if let Err(err) = tx.try_send(event) {
            let msg = match err {
              tokio::sync::mpsc::error::TrySendError::Full(_) => {
                "event channel saturated".to_string()
              }
              tokio::sync::mpsc::error::TrySendError::Closed(_) => {
                "event channel closed".to_string()
              }
            };
            js_call.reject(laufey::Value::String(msg));
            return;
          }
          match resp_rx.await {
            Ok(Ok(result)) => {
              js_call.resolve(json_to_laufey_value(&result));
            }
            Ok(Err(error)) => {
              js_call.reject(laufey::Value::String(error));
            }
            Err(_) => {
              js_call.reject(laufey::Value::String(
                "bind response channel dropped".to_string(),
              ));
            }
          }
        }
      },
    );
  }

  fn unbind(&self, window_id: u32, name: &str) {
    laufey::Window::from_id(window_id).unbind(name);
  }

  fn navigate(&self, window_id: u32, url: &str) {
    laufey::Window::from_id(window_id).navigate(url);
  }

  fn quit(&self) {
    laufey::quit();
  }

  fn set_application_menu(
    &self,
    window_id: u32,
    menu: Vec<denort::desktop::MenuItem>,
  ) {
    let menu = menu
      .into_iter()
      .map(desktop_menu_item_to_laufey_menu_item)
      .collect::<Vec<_>>();
    let tx = self.event_tx.clone();
    laufey::Window::from_id(window_id).set_menu(&menu, move |id: &str| {
      let _ =
        tx.try_send(deno_runtime::ops::desktop::DesktopEvent::AppMenuClick {
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
      .map(desktop_menu_item_to_laufey_menu_item)
      .collect::<Vec<_>>();
    let tx = self.event_tx.clone();
    laufey::Window::from_id(window_id).show_context_menu(
      x,
      y,
      &menu,
      move |id: &str| {
        let _ = tx.try_send(
          deno_runtime::ops::desktop::DesktopEvent::ContextMenuClick {
            window_id,
            id: id.to_string(),
          },
        );
      },
    );
  }

  fn get_raw_window_handle(
    &self,
    window_id: u32,
  ) -> Result<
    (
      raw_window_handle::RawWindowHandle,
      raw_window_handle::RawDisplayHandle,
    ),
    deno_error::JsErrorBox,
  > {
    let window = laufey::Window::from_id(window_id);
    let handle_type = window.get_window_handle_type();
    let raw_win = window.get_window_handle();
    let raw_display = window.get_display_handle();

    let null_window =
      || deno_error::JsErrorBox::generic("Laufey returned a null window handle");
    let null_display =
      || deno_error::JsErrorBox::generic("Laufey returned a null display handle");

    match handle_type {
      laufey::LAUFEY_WINDOW_HANDLE_APPKIT => {
        use raw_window_handle::*;
        let win = RawWindowHandle::AppKit(AppKitWindowHandle::new(
          std::ptr::NonNull::new(raw_win).ok_or_else(null_window)?,
        ));
        let display = RawDisplayHandle::AppKit(AppKitDisplayHandle::new());
        Ok((win, display))
      }
      laufey::LAUFEY_WINDOW_HANDLE_WIN32 => {
        use raw_window_handle::*;
        let mut handle = Win32WindowHandle::new(
          std::num::NonZeroIsize::new(raw_win as isize)
            .ok_or_else(null_window)?,
        );
        handle.hinstance =
          std::num::NonZeroIsize::new(raw_display as isize).map(|v| v.into());
        let win = RawWindowHandle::Win32(handle);
        let display = RawDisplayHandle::Windows(WindowsDisplayHandle::new());
        Ok((win, display))
      }
      laufey::LAUFEY_WINDOW_HANDLE_X11 => {
        use raw_window_handle::*;
        let win = RawWindowHandle::Xlib(XlibWindowHandle::new(raw_win as _));
        let display = RawDisplayHandle::Xlib(XlibDisplayHandle::new(
          std::ptr::NonNull::new(raw_display),
          0,
        ));
        Ok((win, display))
      }
      laufey::LAUFEY_WINDOW_HANDLE_WAYLAND => {
        use raw_window_handle::*;
        let win = RawWindowHandle::Wayland(WaylandWindowHandle::new(
          std::ptr::NonNull::new(raw_win).ok_or_else(null_window)?,
        ));
        let display = RawDisplayHandle::Wayland(WaylandDisplayHandle::new(
          std::ptr::NonNull::new(raw_display).ok_or_else(null_display)?,
        ));
        Ok((win, display))
      }
      other => Err(deno_error::JsErrorBox::generic(format!(
        "unknown Laufey window handle type: {other}",
      ))),
    }
  }

  fn alert(&self, title: &str, message: &str) {
    laufey::alert(title, message);
  }

  fn confirm(&self, title: &str, message: &str) -> bool {
    laufey::confirm(title, message)
  }

  fn prompt(
    &self,
    title: &str,
    message: &str,
    default_value: &str,
  ) -> Option<String> {
    laufey::prompt(title, message, default_value)
  }

  fn set_dock_badge(&self, text: &str) {
    laufey::set_dock_badge(if text.is_empty() { None } else { Some(text) });
  }

  fn bounce_dock(&self, critical: bool) {
    laufey::bounce_dock(if critical {
      laufey::DockBounceType::Critical
    } else {
      laufey::DockBounceType::Informational
    });
  }

  fn set_dock_menu(&self, menu: Option<Vec<denort::desktop::MenuItem>>) {
    match menu {
      Some(menu) => {
        let menu = menu
          .into_iter()
          .map(desktop_menu_item_to_laufey_menu_item)
          .collect::<Vec<_>>();
        let tx = self.event_tx.clone();
        laufey::set_dock_menu(&menu, move |id: &str| {
          let _ = tx.try_send(
            deno_runtime::ops::desktop::DesktopEvent::DockMenuClick {
              id: id.to_string(),
            },
          );
        });
      }
      None => laufey::clear_dock_menu(),
    }
  }

  fn set_dock_visible(&self, visible: bool) {
    laufey::set_dock_visible(visible);
  }

  fn create_tray(&self) -> u32 {
    let tray = laufey::TrayIcon::new();
    let tray_id = tray.id();
    if tray_id == 0 {
      return 0;
    }
    let click_tx = self.event_tx.clone();
    let tray = tray.on_click(move || {
      let _ = click_tx.try_send(
        deno_runtime::ops::desktop::DesktopEvent::TrayClick { tray_id },
      );
    });
    let dblclick_tx = self.event_tx.clone();
    tray.set_double_click_handler(move || {
      let _ = dblclick_tx.try_send(
        deno_runtime::ops::desktop::DesktopEvent::TrayDoubleClick { tray_id },
      );
    });
    self.trays.lock().unwrap().insert(tray_id, tray);
    tray_id
  }

  fn destroy_tray(&self, tray_id: u32) {
    self.trays.lock().unwrap().remove(&tray_id);
  }

  fn set_tray_icon(&self, tray_id: u32, png_bytes: &[u8]) {
    if let Some(tray) = self.trays.lock().unwrap().get(&tray_id) {
      tray.set_icon(png_bytes);
    }
  }

  fn set_tray_icon_dark(&self, tray_id: u32, png_bytes: Option<&[u8]>) {
    if let Some(tray) = self.trays.lock().unwrap().get(&tray_id) {
      tray.set_icon_dark(png_bytes.unwrap_or(&[]));
    }
  }

  fn set_tray_tooltip(&self, tray_id: u32, text: Option<&str>) {
    if let Some(tray) = self.trays.lock().unwrap().get(&tray_id) {
      tray.set_tooltip(text);
    }
  }

  fn set_tray_menu(
    &self,
    tray_id: u32,
    menu: Option<Vec<denort::desktop::MenuItem>>,
  ) {
    let trays = self.trays.lock().unwrap();
    let Some(tray) = trays.get(&tray_id) else {
      return;
    };
    match menu {
      Some(menu) => {
        let menu = menu
          .into_iter()
          .map(desktop_menu_item_to_laufey_menu_item)
          .collect::<Vec<_>>();
        let tx = self.event_tx.clone();
        tray.set_menu(&menu, move |id: &str| {
          let _ = tx.try_send(
            deno_runtime::ops::desktop::DesktopEvent::TrayMenuClick {
              tray_id,
              id: id.to_string(),
            },
          );
        });
      }
      None => tray.clear_menu(),
    }
  }

  fn get_tray_bounds(&self, tray_id: u32) -> Option<(i32, i32, i32, i32)> {
    let trays = self.trays.lock().unwrap();
    trays.get(&tray_id)?.get_bounds()
  }

  fn show_notification(
    &self,
    title: &str,
    body: Option<&str>,
    icon: Option<&[u8]>,
    tag: Option<&str>,
    silent: Option<bool>,
    require_interaction: Option<bool>,
  ) -> u32 {
    let mut builder = laufey::Notification::new(title);
    if let Some(body) = body {
      builder = builder.body(body);
    }
    if let Some(icon) = icon {
      builder = builder.icon(icon.to_vec());
    }
    if let Some(tag) = tag {
      builder = builder.tag(tag);
    }
    if let Some(silent) = silent {
      builder = builder.silent(silent);
    }
    if let Some(require) = require_interaction {
      builder = builder.require_interaction(require);
    }

    // The laufey handler closure receives only the event; it needs the
    // notification id to route the event through the desktop channel.
    // We can't know the id until `on_event` returns, so we capture it
    // through a shared slot populated immediately after.
    let id_slot: Arc<std::sync::OnceLock<u32>> =
      Arc::new(std::sync::OnceLock::new());
    let id_for_handler = id_slot.clone();
    let tx = self.event_tx.clone();
    let notifications = self.notifications.clone();

    let handle = builder.on_event(move |event| {
      let Some(&nid) = id_for_handler.get() else {
        return;
      };
      use laufey::NotificationEvent;
      let desktop_event = match event {
        NotificationEvent::Shown => {
          deno_runtime::ops::desktop::DesktopEvent::NotificationShow {
            notification_id: nid,
          }
        }
        NotificationEvent::Clicked => {
          deno_runtime::ops::desktop::DesktopEvent::NotificationClick {
            notification_id: nid,
          }
        }
        NotificationEvent::Closed => {
          deno_runtime::ops::desktop::DesktopEvent::NotificationClose {
            notification_id: nid,
          }
        }
        // The Web Notification API has no "action" event in window context;
        // surface action button clicks as a click event for compatibility.
        NotificationEvent::Action(_) => {
          deno_runtime::ops::desktop::DesktopEvent::NotificationClick {
            notification_id: nid,
          }
        }
      };
      let is_terminal = matches!(event, laufey::NotificationEvent::Closed);
      let _ = tx.try_send(desktop_event);
      if is_terminal {
        notifications.lock().unwrap().remove(&nid);
      }
    });

    let id = handle.id();
    if id == 0 {
      // Backend doesn't support notifications. Emit a synthetic error
      // event so the user can observe the failure.
      let _ = self.event_tx.try_send(
        deno_runtime::ops::desktop::DesktopEvent::NotificationError {
          notification_id: 0,
        },
      );
      return 0;
    }
    let _ = id_slot.set(id);
    self.notifications.lock().unwrap().insert(id, handle);
    id
  }

  fn close_notification(&self, notification_id: u32) {
    if let Some(handle) =
      self.notifications.lock().unwrap().get(&notification_id)
    {
      handle.close();
    }
  }

  fn request_notification_permission(
    &self,
    cb: Box<
      dyn FnOnce(deno_runtime::ops::desktop::PermissionState) + Send + 'static,
    >,
  ) {
    laufey::request_permission(
      laufey::PermissionKind::Notifications,
      move |status| cb(map_permission_status(status)),
    );
  }

  fn query_notification_permission(
    &self,
    cb: Box<
      dyn FnOnce(deno_runtime::ops::desktop::PermissionState) + Send + 'static,
    >,
  ) {
    laufey::query_permission(
      laufey::PermissionKind::Notifications,
      move |status| cb(map_permission_status(status)),
    );
  }
}

fn map_permission_status(
  status: laufey::PermissionStatus,
) -> deno_runtime::ops::desktop::PermissionState {
  use deno_runtime::ops::desktop::PermissionState;
  match status {
    laufey::PermissionStatus::Granted => PermissionState::Granted,
    laufey::PermissionStatus::Denied => PermissionState::Denied,
    laufey::PermissionStatus::Prompt => PermissionState::Prompt,
    laufey::PermissionStatus::Unsupported => PermissionState::Unsupported,
  }
}

fn desktop_menu_item_to_laufey_menu_item(
  item: denort::desktop::MenuItem,
) -> laufey::MenuItem {
  match item {
    denort::desktop::MenuItem::Item {
      label,
      id,
      accelerator,
      enabled,
    } => laufey::MenuItem::Item {
      label,
      id,
      accelerator,
      enabled,
    },
    denort::desktop::MenuItem::Submenu { label, items } => {
      laufey::MenuItem::Submenu {
        label,
        items: items
          .into_iter()
          .map(desktop_menu_item_to_laufey_menu_item)
          .collect(),
      }
    }
    denort::desktop::MenuItem::Separator => laufey::MenuItem::Separator,
    denort::desktop::MenuItem::Role { role } => {
      laufey::MenuItem::Role { role }
    }
  }
}

#[allow(dead_code)]
fn laufey_value_to_v8<'a>(
  scope: &v8::PinScope<'a, '_>,
  val: laufey::Value,
) -> v8::Local<'a, v8::Value> {
  match val {
    laufey::Value::Null => v8::null(scope).into(),
    laufey::Value::Bool(bool) => v8::Boolean::new(scope, bool).into(),
    laufey::Value::Int(int) => v8::Integer::new(scope, int).into(),
    laufey::Value::Double(double) => v8::Number::new(scope, double).into(),
    laufey::Value::String(str) => {
      v8::String::new(scope, &str).unwrap().into()
    }
    laufey::Value::List(list) => {
      let elements = list
        .into_iter()
        .map(|v| laufey_value_to_v8(scope, v))
        .collect::<Vec<_>>();
      v8::Array::new_with_elements(scope, &elements).into()
    }
    laufey::Value::Dict(dict) => {
      let mut names = Vec::with_capacity(dict.len());
      let mut values = Vec::with_capacity(dict.len());

      for (k, v) in dict {
        names.push(v8::String::new(scope, &k).unwrap().into());
        values.push(laufey_value_to_v8(scope, v));
      }

      let prototype = v8::null(scope).into();
      v8::Object::with_prototype_and_properties(
        scope, prototype, &names, &values,
      )
      .into()
    }
    laufey::Value::Binary(bin) => {
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
fn v8_to_laufey_value<'a>(
  scope: &v8::PinScope<'a, '_>,
  val: v8::Local<'a, v8::Value>,
) -> laufey::Value {
  if val.is_null_or_undefined() {
    laufey::Value::Null
  } else if val.is_boolean() {
    laufey::Value::Bool(val.boolean_value(scope))
  } else if val.is_int32() {
    laufey::Value::Int(val.int32_value(scope).unwrap_or(0))
  } else if val.is_number() {
    laufey::Value::Double(val.number_value(scope).unwrap_or(0.0))
  } else if val.is_string() {
    let s = val.to_rust_string_lossy(scope);
    laufey::Value::String(s)
  } else if val.is_array_buffer_view() {
    let view: v8::Local<v8::ArrayBufferView> = val.try_into().unwrap();
    let len = view.byte_length();
    let mut buf = vec![0u8; len];
    view.copy_contents(&mut buf);
    laufey::Value::Binary(buf)
  } else if val.is_array() {
    let arr: v8::Local<v8::Array> = val.try_into().unwrap();
    let len = arr.length();
    let mut list = Vec::with_capacity(len as usize);
    for i in 0..len {
      if let Some(elem) = arr.get_index(scope, i) {
        list.push(v8_to_laufey_value(scope, elem));
      }
    }
    laufey::Value::List(list)
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
            map.insert(key_str, v8_to_laufey_value(scope, value));
          }
        }
      }
    }
    laufey::Value::Dict(map)
  } else {
    // Fallback: coerce to string
    laufey::Value::String(val.to_rust_string_lossy(scope))
  }
}

/// Promote this dylib's symbols to the global symbol scope so that
/// native addons loaded via `dlopen` (e.g. next-swc.node) can resolve
/// NAPI function symbols from our library.
///
/// By default, Laufey loads this dylib without `RTLD_GLOBAL`, so its symbols
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

  // SAFETY:
  // - `dladdr` reads the metadata of the function passed in; a known
  //   function pointer in this dylib is always a valid argument.
  // - `dlopen` with RTLD_NOLOAD doesn't load anything new — it only
  //   bumps the refcount of an already-loaded image, then sets
  //   RTLD_GLOBAL on its symbols. We pass the path returned by
  //   `dladdr` (our own dylib) which is guaranteed loaded since
  //   we're executing in it. On NULL return there's nothing for us
  //   to clean up — the global-symbol-promotion just didn't happen
  //   and any NAPI addon needing those symbols will fail with a
  //   clear "symbol not found" later.
  unsafe {
    let mut info: DlInfo = std::mem::zeroed();
    let addr = promote_dylib_symbols_to_global as *const std::ffi::c_void;
    if dladdr(addr, &mut info) != 0 && !info.dli_fname.is_null() {
      let handle =
        dlopen(info.dli_fname, RTLD_LAZY | RTLD_NOLOAD | RTLD_GLOBAL);
      if handle.is_null() {
        log::debug!(
          "[desktop] dlopen(self, RTLD_NOLOAD|RTLD_GLOBAL) returned NULL; \
           NAPI symbols will not be globally visible"
        );
      }
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

    // Hard-link (or copy) the live dylib aside as a backup *without*
    // unlinking the original. If the subsequent swap-in of the update
    // fails (perms, disk full, EXDEV), the running dylib is still in place
    // — previously a rename-then-rename pair could leave the app with no
    // dylib at all on rename #2 failure.
    let backup_ok = std::fs::hard_link(dylib_path, &backup_path).is_ok()
      || std::fs::copy(dylib_path, &backup_path).is_ok();
    if !backup_ok {
      eprintln!("[desktop] could not stage backup, skipping update");
      return false;
    }

    if std::fs::rename(&update_path, dylib_path).is_err() {
      // Rename failed (cross-filesystem / perms / etc.). Fall back to a
      // temp-then-rename copy so the dylib is never observed half-written:
      // copy the update to `<dylib>.update.tmp` on the same filesystem as
      // the dylib, then atomic-rename into place. Only on full success do
      // we consume the staged `.update`.
      let tmp_path = dylib_path.with_extension(format!("{}.update.tmp", ext));
      let copy_ok = std::fs::copy(&update_path, &tmp_path).is_ok()
        && std::fs::rename(&tmp_path, dylib_path).is_ok();
      if copy_ok {
        let _ = std::fs::remove_file(&update_path);
      } else {
        // Couldn't apply the update by rename or copy. Leave `.update` in
        // place so the next launch retries, drop the stale `.tmp`, and
        // delete the unused `.backup` — otherwise the next launch would
        // see backup-without-sentinel and trigger a spurious "rollback"
        // even though we never swapped anything in.
        let _ = std::fs::remove_file(&tmp_path);
        let _ = std::fs::remove_file(&backup_path);
        eprintln!(
          "[desktop] failed to apply staged update; will retry on next launch"
        );
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

laufey::main!(|| {
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
  // forked workers and run them headless (no Laufey window).
  //
  // A forked worker is recognized by the *combination* of:
  //   1. argv shaped like `<exe> run [flags…] script.js …` (i.e.
  //      `extract_fork_script_path` returns `Some`), OR
  //   2. argv shaped like `<exe> run …` *and* one of the worker env
  //      vars set by the parent dev server (NODE_CHANNEL_FD,
  //      NEXT_PRIVATE_WORKER).
  //
  // The bare env-var check used to be enough, but a user shell that
  // already had NODE_CHANNEL_FD set (e.g. running inside another forked
  // process, Jest, pnpm) would silently take the headless path and
  // never show a window. Requiring the `run` argv shape rules that out:
  // the Laufey backend never invokes us with `run` as argv[1].
  let args: Vec<_> = env::args_os().collect();
  let argv_run = args
    .get(1)
    .and_then(|a| a.to_str())
    .map(|s| s == "run")
    .unwrap_or(false);
  let is_worker = extract_fork_script_path(&args).is_some()
    || (argv_run
      && (env::var("NODE_CHANNEL_FD").is_ok()
        || env::var("NEXT_PRIVATE_WORKER").is_ok()));
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

  laufey::set_js_namespace("bindings");

  // Allocate the desktop serve port and publish it via DENO_SERVE_ADDRESS
  // BEFORE the tokio runtime is built. Once the runtime spins up its
  // mio IO thread (and, optionally, the inspector server thread),
  // `setenv` is no longer thread-safe on glibc — Rust 1.81+ marks it
  // unsafe for that reason. We're still single-threaded up to here:
  // the worker-fork path has already returned, and the init calls
  // above (init_logging, mark_standalone, rustls install_default,
  // set_js_namespace) don't spawn threads.
  let desktop_serve_port = match allocate_random_port() {
    Ok(p) => p,
    Err(e) => {
      eprintln!("[desktop] failed to allocate serve port: {}", e);
      return;
    }
  };
  // SAFETY: see the block comment above — single-threaded at this point.
  unsafe {
    std::env::set_var(
      "DENO_SERVE_ADDRESS",
      format!("tcp:127.0.0.1:{}", desktop_serve_port),
    );
  }

  // Read the embedded standalone section, extract the VFS, and chdir
  // into the extraction dir — all BEFORE the tokio runtime starts.
  // chdir is process-wide; doing it after the runtime build (and any
  // worker / async tasks it spawns) would race with code that resolves
  // relative paths.
  let args: Vec<_> = env::args_os().collect();
  let data = match denort::binary::extract_standalone_with_finder(
    Cow::Owned(args),
    find_section_in_dylib,
  ) {
    Ok(data) => data,
    Err(e) => {
      eprintln!("[desktop] failed to read standalone section: {:?}", e);
      return;
    }
  };
  if data.metadata.self_extracting.is_some() {
    if let Err(e) =
      denort::binary::extract_vfs_to_disk(&data.vfs, &data.root_path)
    {
      eprintln!("[desktop] failed to extract VFS: {:?}", e);
      return;
    }
    // Frameworks like Next.js look for build output (e.g. .next/)
    // relative to CWD.
    if let Err(e) = std::env::set_current_dir(&data.root_path) {
      eprintln!(
        "[desktop] failed to chdir to {}: {}",
        data.root_path.display(),
        e
      );
      return;
    }
  }

  let rt = tokio::runtime::Builder::new_current_thread()
    .enable_all()
    .build()
    .unwrap();

  rt.block_on(async {
    eprintln!("[desktop] run_desktop starting");
    match run_desktop(update_rolled_back, desktop_serve_port, data).await {
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
          laufey::alert(
            "Application Error",
            error_string.trim_start_matches("error: "),
          );
        }
      }
    }
  });
});

/// Run as a headless worker (no Laufey window). Used when a framework dev
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

    match denort::run::run_with_options(
      Arc::new(sys.clone()),
      sys,
      data,
      options,
    )
    .await
    {
      Ok(exit_code) => {
        eprintln!(
          "[worker] run_with_options completed with exit code: {}",
          exit_code
        );
      }
      Err(error) => {
        let error_string = match js_error_downcast_ref(&error) {
          Some(js_error) => format_js_error(js_error, None),
          None => format!("{:?}", error),
        };
        eprintln!("[worker] run_with_options error: {}", error_string);
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

/// Convert a laufey::Value to a DesktopValue for direct V8 conversion.
fn laufey_value_to_desktop_value(
  v: laufey::Value,
) -> deno_runtime::ops::desktop::DesktopValue {
  use deno_runtime::ops::desktop::DesktopValue;
  match v {
    laufey::Value::Null => DesktopValue::Null,
    laufey::Value::Bool(b) => DesktopValue::Bool(b),
    laufey::Value::Int(i) => DesktopValue::Int(i),
    laufey::Value::Double(d) => DesktopValue::Double(d),
    laufey::Value::String(s) => DesktopValue::String(s),
    laufey::Value::List(l) => DesktopValue::List(
      l.into_iter().map(laufey_value_to_desktop_value).collect(),
    ),
    laufey::Value::Dict(d) => DesktopValue::Dict(
      d.into_iter()
        .map(|(k, v)| (k, laufey_value_to_desktop_value(v)))
        .collect(),
    ),
    laufey::Value::Binary(b) => DesktopValue::Binary(b),
  }
}

/// Convert a laufey::Value to a serde_json::Value for channel transport.
fn laufey_value_to_json(v: &laufey::Value) -> serde_json::Value {
  match v {
    laufey::Value::Null => serde_json::Value::Null,
    laufey::Value::Bool(b) => serde_json::Value::Bool(*b),
    laufey::Value::Int(i) => serde_json::json!(*i),
    laufey::Value::Double(d) => serde_json::json!(*d),
    laufey::Value::String(s) => serde_json::Value::String(s.clone()),
    laufey::Value::List(l) => {
      serde_json::Value::Array(l.iter().map(laufey_value_to_json).collect())
    }
    laufey::Value::Dict(d) => {
      let mut map = serde_json::Map::new();
      for (k, v) in d {
        map.insert(k.clone(), laufey_value_to_json(v));
      }
      serde_json::Value::Object(map)
    }
    laufey::Value::Binary(b) => serde_json::json!(b),
  }
}

/// Convert a serde_json::Value to a laufey::Value for the menu template.
fn json_to_laufey_value(v: &serde_json::Value) -> laufey::Value {
  match v {
    serde_json::Value::Null => laufey::Value::Null,
    serde_json::Value::Bool(b) => laufey::Value::Bool(*b),
    serde_json::Value::Number(n) => {
      if let Some(i) = n.as_i64() {
        laufey::Value::Int(i as i32)
      } else {
        laufey::Value::Double(n.as_f64().unwrap_or(0.0))
      }
    }
    serde_json::Value::String(s) => laufey::Value::String(s.clone()),
    serde_json::Value::Array(arr) => {
      laufey::Value::List(arr.iter().map(json_to_laufey_value).collect())
    }
    serde_json::Value::Object(obj) => {
      let mut map = std::collections::HashMap::new();
      for (k, v) in obj {
        map.insert(k.clone(), json_to_laufey_value(v));
      }
      laufey::Value::Dict(map)
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

async fn run_desktop(
  update_rolled_back: bool,
  desktop_serve_port: u16,
  data: denort::binary::StandaloneData,
) -> Result<(), AnyError> {
  // Make the error reporting URL available to the panic hook.
  if let Some(ref url) = data.metadata.error_reporting_url {
    deno_runtime::ops::desktop::set_error_report_config(
      url.clone(),
      data.metadata.app_version.clone(),
    );
  }

  // The VFS extract + chdir for self-extracting bundles happens in
  // `laufey::main!` before the tokio runtime is built — chdir is
  // process-wide and isn't safe to do once async tasks are running.
  let sys = if data.metadata.self_extracting.is_some() {
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

  // Wire up the Deno-side inspector when launched under
  // `deno desktop --inspect[-brk|-wait]`. The parent process binds the
  // user-visible port and runs a multiplexer that fronts both this
  // inspector and the CEF renderer's debug port; we just listen on the
  // internal port that the parent allocated for us.
  // Surface a malformed value loudly: previously a typoed port silently
  // disabled the inspector and the user wondered why DevTools showed
  // nothing. Bail rather than no-op so the failure is visible.
  let inspect_internal_port = match env::var(
    "DENO_DESKTOP_INSPECT_INTERNAL_PORT",
  ) {
    Ok(s) => match s.parse::<std::net::SocketAddr>() {
      Ok(addr) => Some(addr),
      Err(e) => {
        bail!(
          "DENO_DESKTOP_INSPECT_INTERNAL_PORT={s:?} is not a valid SocketAddr: {e}"
        );
      }
    },
    Err(_) => None,
  };
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

  // DENO_SERVE_ADDRESS is published by `laufey::main!` before the
  // tokio runtime is built — see the comment there for why we can't
  // do it from here. `desktop_serve_port` is the port we put into it.

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
  let initial_window_id_for_navigate = initial_window_id.clone();

  // Track every live window so HMR can refresh secondary windows too,
  // not just the initial one. The same Arc is handed to WefDesktopApi
  // below; create_window / close_window keep it in sync.
  let open_windows: Arc<Mutex<HashSet<u32>>> =
    Arc::new(Mutex::new(HashSet::new()));
  let open_windows_for_api = open_windows.clone();
  let open_windows_for_hmr = open_windows.clone();

  let hmr_on_reload: Option<denort::hmr::HmrReloadCallback> =
    if hmr_watch_dir.is_some() && !is_framework_dev {
      Some(Box::new(move || {
        let ids: Vec<u32> = open_windows_for_hmr
          .lock()
          .unwrap()
          .iter()
          .copied()
          .collect();
        for id in ids {
          laufey::Window::from_id(id).execute_js::<fn(
            Result<laufey::Value, laufey::Value>,
          )>("location.reload()", None);
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
      rolled_back: update_rolled_back, // from laufey::main! startup check
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
        open_windows: open_windows_for_api.clone(),
        trays: Arc::new(Mutex::new(HashMap::new())),
        notifications: Arc::new(Mutex::new(HashMap::new())),
        devtools_window: Mutex::new(None),
      };

      // Forward macOS dock-reopen callbacks (clicking the dock icon while
      // no windows are visible) into the shared event channel so JS can
      // observe them as `Deno.dock` "reopen" events.
      {
        let reopen_tx = event_tx.0.clone();
        laufey::on_dock_reopen(move |has_visible_windows| {
          let _ = reopen_tx.try_send(
            deno_runtime::ops::desktop::DesktopEvent::DockReopen {
              has_visible_windows,
            },
          );
        });
      }

      // Create the initial window and wire up event handlers.
      let window_id = api.create_window(800, 600, false, false, false);
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

  // Run the Deno runtime and Laufey event loop concurrently.
  // We spawn the runtime first, wait for the server to be ready,
  // then navigate the webview.
  let url = format!("http://127.0.0.1:{}", desktop_serve_port);
  eprintln!("[desktop] starting runtime and laufey event loop");
  let run_fut =
    denort::run::run_with_options(Arc::new(sys.clone()), sys, data, run_opts);
  let laufey_fut = laufey::run();

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
              laufey::Window::from_id(id).navigate(&url);
              return;
            }
          }
        }
      }
      tokio::time::sleep(std::time::Duration::from_millis(250)).await;
    }
    log::warn!("Server not ready after 15s, navigating anyway");
    let id = initial_window_id_for_navigate.load(Ordering::Acquire);
    laufey::Window::from_id(id).navigate(&url);
  };

  // Hold the JoinHandle so we can abort it when the runtime / Laufey
  // event loop ends — otherwise the navigate poll keeps trying for up
  // to 15s after window close, writing warnings to a stderr that may
  // already be torn down.
  let navigate_handle = tokio::spawn(navigate_fut);

  tokio::select! {
    result = run_fut => {
      match result {
        Ok(exit_code) => {
          eprintln!("[desktop] Deno runtime exited with code {}", exit_code);
        }
        Err(err) => {
          eprintln!("[desktop] Deno runtime error: {:?}", err);
          navigate_handle.abort();
          return Err(err);
        }
      }
    }
    _ = laufey_fut => {
      eprintln!("[desktop] Laufey event loop ended (window closed)");
    }
  }
  navigate_handle.abort();

  Ok(())
}

#[cfg(test)]
mod tests {
  use std::ffi::OsString;

  use deno_runtime::ops::desktop::PermissionState;

  use super::desktop_menu_item_to_laufey_menu_item;
  use super::extract_fork_script_path;
  use super::json_to_laufey_value;
  use super::map_permission_status;
  use super::laufey_value_to_desktop_value;
  use super::laufey_value_to_json;

  // --- extract_fork_script_path ---
  //
  // Framework dev servers (e.g. Next.js) call `child_process.fork()` to
  // re-exec this binary as a worker process. We detect that pattern by
  // argv shape: `<exe> run [flags…] script.js …`. Mis-classifying the
  // main launch as a worker would mean no window appears at all.

  fn args(parts: &[&str]) -> Vec<OsString> {
    parts.iter().map(|s| OsString::from(*s)).collect()
  }

  #[test]
  fn fork_argv_shape_returns_script_url() {
    let v =
      extract_fork_script_path(&args(&["myapp", "run", "/tmp/worker.js"]));
    let url = v.expect("argv with `run <abs-script>` is a worker");
    assert!(url.as_str().ends_with("worker.js"));
    assert_eq!(url.scheme(), "file");
  }

  #[test]
  fn fork_argv_skips_flags_before_script() {
    let v = extract_fork_script_path(&args(&[
      "myapp",
      "run",
      "--allow-net",
      "-A",
      "/tmp/worker.js",
    ]));
    assert!(v.is_some(), "flags before the script path must be skipped");
  }

  #[test]
  fn fork_argv_rejects_non_run_subcommand() {
    // Only `<exe> run …` is the worker shape. The main desktop launch
    // never has a subcommand at argv[1] — laufey passes no argv at all.
    assert!(extract_fork_script_path(&args(&["myapp"])).is_none());
    assert!(
      extract_fork_script_path(&args(&["myapp", "task", "build"])).is_none()
    );
    assert!(extract_fork_script_path(&args(&["myapp", "test"])).is_none());
  }

  #[test]
  fn fork_argv_run_with_only_flags_is_not_a_fork() {
    // `<exe> run --help` has no script path — don't claim this is a
    // worker.
    let v = extract_fork_script_path(&args(&["myapp", "run", "--help"]));
    assert!(
      v.is_none(),
      "argv with only flags after `run` is not a fork worker"
    );
  }

  #[test]
  fn fork_argv_bare_run_is_not_a_fork() {
    // `<exe> run` with no further args isn't enough to be a fork.
    assert!(extract_fork_script_path(&args(&["myapp", "run"])).is_none());
  }

  // --- map_permission_status ---

  #[test]
  fn permission_status_maps_one_to_one() {
    // Every laufey PermissionStatus must round-trip to the matching
    // deno PermissionState. Off-by-one swaps here would surface in JS
    // as "granted shows up as denied" — silent and very confusing.
    assert!(matches!(
      map_permission_status(laufey::PermissionStatus::Granted),
      PermissionState::Granted
    ));
    assert!(matches!(
      map_permission_status(laufey::PermissionStatus::Denied),
      PermissionState::Denied
    ));
    assert!(matches!(
      map_permission_status(laufey::PermissionStatus::Prompt),
      PermissionState::Prompt
    ));
    assert!(matches!(
      map_permission_status(laufey::PermissionStatus::Unsupported),
      PermissionState::Unsupported
    ));
  }

  // --- desktop_menu_item_to_laufey_menu_item ---

  #[test]
  fn menu_item_conversion_preserves_fields() {
    let item = denort::desktop::MenuItem::Item {
      label: "Save".into(),
      id: Some("file.save".into()),
      accelerator: Some("CmdOrCtrl+S".into()),
      enabled: true,
    };
    match desktop_menu_item_to_laufey_menu_item(item) {
      laufey::MenuItem::Item {
        label,
        id,
        accelerator,
        enabled,
      } => {
        assert_eq!(label, "Save");
        assert_eq!(id.as_deref(), Some("file.save"));
        assert_eq!(accelerator.as_deref(), Some("CmdOrCtrl+S"));
        assert!(enabled);
      }
      _ => panic!("expected Item"),
    }
  }

  #[test]
  fn menu_item_conversion_recurses_into_submenus() {
    let item = denort::desktop::MenuItem::Submenu {
      label: "File".into(),
      items: vec![
        denort::desktop::MenuItem::Item {
          label: "Open".into(),
          id: Some("open".into()),
          accelerator: None,
          enabled: false,
        },
        denort::desktop::MenuItem::Separator,
        denort::desktop::MenuItem::Role {
          role: "quit".into(),
        },
      ],
    };
    let converted = desktop_menu_item_to_laufey_menu_item(item);
    let laufey::MenuItem::Submenu { items, .. } = converted else {
      panic!("expected Submenu");
    };
    assert_eq!(items.len(), 3);
    // First child is an Item with enabled=false preserved.
    match &items[0] {
      laufey::MenuItem::Item { enabled, label, .. } => {
        assert_eq!(label, "Open");
        assert!(!enabled, "enabled=false must propagate through recursion");
      }
      _ => panic!("first child should be Item"),
    }
    assert!(matches!(items[1], laufey::MenuItem::Separator));
    match &items[2] {
      laufey::MenuItem::Role { role } => assert_eq!(role, "quit"),
      _ => panic!("third child should be Role"),
    }
  }

  // --- laufey_value_to_desktop_value / laufey_value_to_json / json_to_laufey_value ---

  #[test]
  fn laufey_value_to_desktop_value_covers_every_variant() {
    use deno_runtime::ops::desktop::DesktopValue;
    assert!(matches!(
      laufey_value_to_desktop_value(laufey::Value::Null),
      DesktopValue::Null
    ));
    assert!(matches!(
      laufey_value_to_desktop_value(laufey::Value::Bool(true)),
      DesktopValue::Bool(true)
    ));
    assert!(matches!(
      laufey_value_to_desktop_value(laufey::Value::Int(7)),
      DesktopValue::Int(7)
    ));
    assert!(matches!(
      laufey_value_to_desktop_value(laufey::Value::Double(1.5)),
      DesktopValue::Double(d) if d == 1.5
    ));
    match laufey_value_to_desktop_value(laufey::Value::String("hi".into())) {
      DesktopValue::String(s) => assert_eq!(s, "hi"),
      _ => panic!(),
    }
    match laufey_value_to_desktop_value(laufey::Value::Binary(vec![1, 2, 3])) {
      DesktopValue::Binary(b) => assert_eq!(b, vec![1, 2, 3]),
      _ => panic!(),
    }
  }

  #[test]
  fn laufey_value_to_desktop_value_recurses() {
    use deno_runtime::ops::desktop::DesktopValue;
    let v = laufey::Value::List(vec![
      laufey::Value::Int(1),
      laufey::Value::List(vec![laufey::Value::String("nested".into())]),
    ]);
    let dv = laufey_value_to_desktop_value(v);
    let DesktopValue::List(outer) = dv else {
      panic!("outer must be List")
    };
    assert_eq!(outer.len(), 2);
    let DesktopValue::List(inner) = &outer[1] else {
      panic!("second element must be a nested List")
    };
    match &inner[0] {
      DesktopValue::String(s) => assert_eq!(s, "nested"),
      _ => panic!("inner element should be String"),
    }
  }

  #[test]
  fn laufey_value_to_json_roundtrip() {
    use std::collections::HashMap;
    let mut dict = HashMap::new();
    dict.insert("name".to_string(), laufey::Value::String("ada".into()));
    dict.insert("count".to_string(), laufey::Value::Int(42));
    dict.insert("ok".to_string(), laufey::Value::Bool(true));
    let v = laufey::Value::Dict(dict);
    let j = laufey_value_to_json(&v);
    assert_eq!(j["name"], "ada");
    assert_eq!(j["count"], 42);
    assert_eq!(j["ok"], true);

    // Round-trip back through json_to_laufey_value to confirm symmetry on
    // the simple types.
    let back = json_to_laufey_value(&j);
    let laufey::Value::Dict(d) = back else {
      panic!("round-trip must yield Dict")
    };
    match d.get("name") {
      Some(laufey::Value::String(s)) => assert_eq!(s, "ada"),
      _ => panic!("name must round-trip as String"),
    }
    match d.get("count") {
      // json_to_laufey_value maps integer numbers to Int via as_i64() — so
      // it should land in the Int branch.
      Some(laufey::Value::Int(42)) => {}
      _ => panic!("count round-trip should yield laufey::Value::Int(42)"),
    }
  }

  #[test]
  fn json_to_laufey_value_distinguishes_int_and_double() {
    let n_int = serde_json::json!(42);
    let n_float = serde_json::json!(1.5);
    assert!(matches!(
      json_to_laufey_value(&n_int),
      laufey::Value::Int(42)
    ));
    assert!(matches!(
      json_to_laufey_value(&n_float),
      laufey::Value::Double(d) if d == 1.5
    ));
  }

  // --- apply_pending_update ---
  //
  // State machine: presence/absence of `<dylib>.update`, `.backup`, and
  // `.update-ok` sentinel decides what apply_pending_update does next.
  // The four combinations have non-obvious effects (commit / rollback /
  // cleanup / no-op) and the consequences of getting them wrong are
  // serious (booting a half-applied dylib, or boot-looping on rollback).
  // Tests below exercise each branch using a tempdir as the live dylib
  // location.

  use super::apply_pending_update;

  fn touch(path: &std::path::Path, content: &str) {
    std::fs::write(path, content).unwrap();
  }

  fn read(path: &std::path::Path) -> String {
    std::fs::read_to_string(path).unwrap()
  }

  fn paths(
    tmp: &std::path::Path,
  ) -> (
    std::path::PathBuf,
    std::path::PathBuf,
    std::path::PathBuf,
    std::path::PathBuf,
  ) {
    let dylib = tmp.join("app.dylib");
    let update = tmp.join("app.dylib.update");
    let backup = tmp.join("app.dylib.backup");
    let sentinel = tmp.join("app.dylib.update-ok");
    (dylib, update, backup, sentinel)
  }

  #[test]
  fn pending_update_no_files_is_noop() {
    let tmp = tempfile::tempdir().unwrap();
    let (dylib, _, _, _) = paths(tmp.path());
    touch(&dylib, "live");
    let rolled_back = apply_pending_update(&dylib);
    assert!(!rolled_back, "no .update / .backup → no rollback");
    assert_eq!(read(&dylib), "live", "dylib must be untouched");
  }

  #[test]
  fn pending_update_swaps_in_new_dylib_when_update_exists() {
    let tmp = tempfile::tempdir().unwrap();
    let (dylib, update, backup, sentinel) = paths(tmp.path());
    touch(&dylib, "old");
    touch(&update, "new");
    let rolled_back = apply_pending_update(&dylib);

    assert!(!rolled_back, "applying a fresh update is not a rollback");
    assert_eq!(
      read(&dylib),
      "new",
      "dylib must now hold the update contents"
    );
    assert!(backup.exists(), "live dylib must be preserved as .backup");
    assert_eq!(read(&backup), "old", "backup must be the *previous* dylib");
    // Stale sentinel from a prior update is cleared so the next launch
    // can detect THIS update's success/failure.
    assert!(!sentinel.exists());
    // The staged update file is consumed.
    assert!(!update.exists());
  }

  #[test]
  fn pending_update_clears_stale_sentinel_before_swap() {
    let tmp = tempfile::tempdir().unwrap();
    let (dylib, update, backup, sentinel) = paths(tmp.path());
    touch(&dylib, "old");
    touch(&update, "new");
    touch(&sentinel, "ok"); // sentinel left over from a previous launch
    touch(&backup, "very-old"); // and a backup, also stale
    apply_pending_update(&dylib);
    // After applying a new update: backup is the just-replaced 'old',
    // sentinel was cleared. The previous backup ("very-old") must not
    // persist, or rollback would resurrect a two-version-old dylib.
    assert_eq!(read(&backup), "old");
    assert!(!sentinel.exists());
  }

  #[test]
  fn pending_update_rolls_back_when_sentinel_missing() {
    // .backup present + no sentinel = previous update booted but crashed
    // before writing the sentinel. Rollback to backup.
    let tmp = tempfile::tempdir().unwrap();
    let (dylib, _, backup, _) = paths(tmp.path());
    touch(&dylib, "new-but-broken");
    touch(&backup, "old-but-known-good");
    let rolled_back = apply_pending_update(&dylib);
    assert!(rolled_back, "missing sentinel must trigger rollback");
    assert_eq!(
      read(&dylib),
      "old-but-known-good",
      "rollback must restore the backup contents over the live dylib"
    );
  }

  #[test]
  fn pending_update_cleans_up_after_successful_boot() {
    // .backup + .update-ok sentinel = previous update was applied AND
    // booted at least once. Clean up; we don't need to keep the backup
    // around forever.
    let tmp = tempfile::tempdir().unwrap();
    let (dylib, _, backup, sentinel) = paths(tmp.path());
    touch(&dylib, "new");
    touch(&backup, "old");
    touch(&sentinel, "ok");
    let rolled_back = apply_pending_update(&dylib);
    assert!(!rolled_back);
    assert!(!backup.exists(), "successful-boot path must delete .backup");
    assert!(
      !sentinel.exists(),
      "successful-boot path must delete sentinel"
    );
    assert_eq!(read(&dylib), "new", "dylib untouched on cleanup path");
  }

  #[test]
  fn pending_update_with_only_sentinel_is_noop() {
    // Sentinel without backup or update — orphaned. Don't roll back
    // (there's nothing to roll back to) and don't touch the dylib.
    let tmp = tempfile::tempdir().unwrap();
    let (dylib, _, _, sentinel) = paths(tmp.path());
    touch(&dylib, "live");
    touch(&sentinel, "ok");
    let rolled_back = apply_pending_update(&dylib);
    assert!(!rolled_back);
    assert_eq!(read(&dylib), "live");
  }

  #[test]
  fn json_to_laufey_value_handles_nested_arrays_and_objects() {
    let j = serde_json::json!({
      "list": [1, 2, 3],
      "nested": {"key": "value"},
      "null": null,
    });
    let v = json_to_laufey_value(&j);
    let laufey::Value::Dict(d) = v else {
      panic!("expected Dict")
    };
    assert!(matches!(d.get("null"), Some(laufey::Value::Null)));
    match d.get("list") {
      Some(laufey::Value::List(items)) => assert_eq!(items.len(), 3),
      _ => panic!("list must convert to List"),
    }
    match d.get("nested") {
      Some(laufey::Value::Dict(inner)) => match inner.get("key") {
        Some(laufey::Value::String(s)) => assert_eq!(s, "value"),
        _ => panic!("nested.key must be String"),
      },
      _ => panic!("nested must convert to Dict"),
    }
  }
}
