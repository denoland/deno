// Copyright 2018-2026 the Deno authors. MIT license.

//! Desktop window management for `deno compile --desktop`.
//!
//! The ops are defined in `deno_runtime::ops::desktop` and included in the
//! V8 snapshot. This module re-exports the key types and provides the JS
//! initialization code.

use std::sync::Arc;

use deno_core::OpState;

// Re-export from runtime so denort_desktop can use them.
pub use deno_runtime::ops::desktop::AutoUpdateState;
pub use deno_runtime::ops::desktop::DesktopApi;

/// JS code that exposes desktop APIs via `Deno.BrowserWindow` and `Deno.desktop`.
pub const DESKTOP_JS: &str = r#"
(() => {
  const {
    op_desktop_set_title,
    op_desktop_set_size,
    op_desktop_navigate,
    op_desktop_execute_js,
    op_desktop_close,
    op_desktop_set_application_menu,
    op_desktop_recv_menu_click,
  } = Deno[Deno.internal].core.ops;

  class BrowserWindow {
    constructor(options = {}) {
      if (options.title) op_desktop_set_title(options.title);
      if (options.width && options.height) op_desktop_set_size(options.width, options.height);
    }
    setTitle(title) { op_desktop_set_title(title); }
    setSize(width, height) { op_desktop_set_size(width, height); }
    navigate(url) { op_desktop_navigate(url); }
    executeJs(script) { op_desktop_execute_js(script); }
    close() { op_desktop_close(); }
    setApplicationMenu(templateJson) { op_desktop_set_application_menu(templateJson); }
  }
  Deno.BrowserWindow = BrowserWindow;

  // Poll for menu click events from the native side and dispatch
  // them as CustomEvents on globalThis.  Defer start so the pending
  // op doesn't block the pre-module event loop tick used by HMR.
  addEventListener("load", () => {
    (async () => {
      while (true) {
        const id = await op_desktop_recv_menu_click();
        if (id == null) break;
        dispatchEvent(new CustomEvent("menuclick", { detail: { id } }));
      }
    })();
  }, { once: true });
})();
"#;

/// JS code that initializes auto-update APIs. Executed separately so
/// version and rollback state can be baked in as literals.
pub fn desktop_auto_update_js(
  version: Option<&str>,
  rolled_back: bool,
) -> String {
  format!(
    r#"(() => {{
  const {{
    op_desktop_apply_patch,
    op_desktop_confirm_update,
  }} = Deno[Deno.internal].core.ops;

  const _version = {version};
  const _rolledBack = {rolled_back};

  if (_rolledBack) {{
    queueMicrotask(() => {{
      dispatchEvent(
        new CustomEvent("desktop-update-rollback", {{
          detail: {{ reason: "Update failed to start, rolled back." }},
        }}),
      );
    }});
  }} else {{
    op_desktop_confirm_update();
  }}

  let autoUpdateTimer = null;

  Deno.desktop = {{
    get version() {{ return _version; }},

    autoUpdate(urlOrOpts) {{
      const opts = typeof urlOrOpts === "string"
        ? {{ url: urlOrOpts }}
        : urlOrOpts;
      const {{ url, interval }} = opts;
      if (!_version) {{
        console.warn("Deno.desktop.autoUpdate: no version in deno.json, skipping");
        return;
      }}

      const check = async () => {{
        try {{
          const manifestUrl = url.replace(/\/$/, "") + "/latest.json";
          const resp = await fetch(manifestUrl);
          if (!resp.ok) return;
          const manifest = await resp.json();
          if (manifest.version === _version) return;

          const patchName = manifest.patches?.[_version];
          if (patchName) {{
            const patchUrl = url.replace(/\/$/, "") + "/" + patchName;
            const patchResp = await fetch(patchUrl);
            if (patchResp.ok) {{
              const patchBytes = new Uint8Array(await patchResp.arrayBuffer());
              op_desktop_apply_patch(patchBytes);
              dispatchEvent(
                new CustomEvent("desktop-update-ready", {{
                  detail: {{ version: manifest.version }},
                }}),
              );
              if (autoUpdateTimer) clearInterval(autoUpdateTimer);
              return;
            }}
          }}
          console.warn("Deno.desktop.autoUpdate: no patch available for",
            _version, "->", manifest.version);
        }} catch (e) {{
          console.warn("Deno.desktop.autoUpdate: check failed:", e.message);
        }}
      }};

      setTimeout(check, 1000);
      if (interval) {{
        autoUpdateTimer = setInterval(check, interval);
      }}
    }},
  }};
}})();
"#,
    version = match version {
      Some(v) => format!("\"{}\"", v.replace('\\', "\\\\").replace('"', "\\\"")),
      None => "null".to_string(),
    },
    rolled_back = if rolled_back { "true" } else { "false" },
  )
}

pub use deno_runtime::ops::desktop::MenuClickSender;
pub use deno_runtime::ops::desktop::create_menu_click_channel;

/// Place the DesktopApi and optional AutoUpdateState into OpState.
/// The ops are already registered in the snapshot; this just provides
/// the runtime implementation.
pub fn init_desktop_state(
  state: &mut OpState,
  api: Box<dyn DesktopApi>,
  auto_update: Option<AutoUpdateState>,
) {
  let api: Arc<dyn DesktopApi> = Arc::from(api);
  state.put::<Arc<dyn DesktopApi>>(api);
  if let Some(au) = auto_update {
    state.put::<AutoUpdateState>(au);
  }
  // Create menu click channel so op_desktop_recv_menu_click can work
  let (tx, rx) = create_menu_click_channel();
  state.put(rx);
  state.put(tx);
}
