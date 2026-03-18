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
  const internals = Deno[Deno.internal];
  const {
    BrowserWindow,
    op_desktop_init,
    op_desktop_recv_menu_click,
    op_desktop_recv_keyboard_event,
    op_desktop_recv_bind_call,
    op_desktop_resolve_bind_call,
    op_desktop_reject_bind_call,
  } = internals.core.ops;
  const BrowserWindowPrototype = BrowserWindow.prototype;
  Object.setPrototypeOf(BrowserWindowPrototype, EventTarget.prototype);

  class KeyboardEvent extends Event {
    #key = "";
    #code = "";
    #location = 0;
    #ctrlKey = false;
    #shiftKey = false;
    #altKey = false;
    #metaKey = false;
    #repeat = false;
    #isComposing = false;

    get key() { return this.#key; }
    get code() { return this.#code; }
    get location() { return this.#location; }
    get ctrlKey() { return this.#ctrlKey; }
    get shiftKey() { return this.#shiftKey; }
    get altKey() { return this.#altKey; }
    get metaKey() { return this.#metaKey; }
    get repeat() { return this.#repeat; }
    get isComposing() { return this.#isComposing; }

    constructor(type, init = {}) {
      super(type, init);
      this.#key = init.key ?? "";
      this.#code = init.code ?? "";
      this.#location = init.location ?? 0;
      this.#ctrlKey = init.ctrlKey ?? false;
      this.#shiftKey = init.shiftKey ?? false;
      this.#altKey = init.altKey ?? false;
      this.#metaKey = init.metaKey ?? false;
      this.#repeat = init.repeat ?? false;
      this.#isComposing = init.isComposing ?? false;
    }

    getModifierState(key) {
      switch (key) {
        case "Alt": return this.#altKey;
        case "Control": return this.#ctrlKey;
        case "Meta": return this.#metaKey;
        case "Shift": return this.#shiftKey;
        default: return false;
      }
    }
  }
  globalThis.KeyboardEvent = internals.core.propNonEnumerable(KeyboardEvent);

  op_desktop_init(
    internals.webidlBrand,
    internals.setEventTargetData,
  );

  // Track the active BrowserWindow instance for event dispatch.
  let activeWindow = null;
  const nativeConstructor = BrowserWindow;
  const OrigBW = function(...args) {
    const instance = new nativeConstructor(...args);
    activeWindow = instance;
    return instance;
  };
  Object.setPrototypeOf(OrigBW, nativeConstructor);
  Object.setPrototypeOf(OrigBW.prototype, nativeConstructor.prototype);
  Deno.BrowserWindow = OrigBW;

  internals.defineEventHandler(BrowserWindowPrototype, "keydown");
  internals.defineEventHandler(BrowserWindowPrototype, "keyup");

  // Bind callback registry: name -> bound function
  const bindCallbacks = new Map();

  const nativeBind = BrowserWindowPrototype.bind;
  BrowserWindowPrototype.bind = function(name, fn) {
    bindCallbacks.set(name, fn.bind(this));
    nativeBind.call(this, name);
  };

  const nativeUnbind = BrowserWindowPrototype.unbind;
  BrowserWindowPrototype.unbind = function(name) {
    bindCallbacks.delete(name);
    nativeUnbind.call(this, name);
  };

  // Defer start so the pending op doesn't block the pre-module event loop tick used by HMR.
  addEventListener("load", () => {
    // Poll for menu click events from the native side and dispatch
    // them as CustomEvents on globalThis.
    (async () => {
      while (true) {
        const id = await op_desktop_recv_menu_click();
        if (id == null) break;
        dispatchEvent(new CustomEvent("menuclick", { detail: { id } }));
      }
    })();
    // Poll for keyboard events from the native side and dispatch
    // them on the active BrowserWindow instance.
    (async () => {
      while (true) {
        const ev = await op_desktop_recv_keyboard_event();
        if (ev == null) break;
        const target = activeWindow;
        if (!target) continue;
        target.dispatchEvent(new KeyboardEvent(ev.type, {
          key: ev.key,
          code: ev.code,
          shiftKey: ev.shift,
          ctrlKey: ev.control,
          altKey: ev.alt,
          metaKey: ev.meta,
          repeat: ev.repeat,
        }));
      }
    })();
    // Poll for bind calls from the webview and dispatch to JS callbacks.
    (async () => {
      while (true) {
        const call = await op_desktop_recv_bind_call();
        if (call == null) break;
        const fn_ = bindCallbacks.get(call.name);
        if (!fn_) {
          op_desktop_reject_bind_call(call.callId, "No callback bound for: " + call.name);
          continue;
        }
        try {
          const args = Array.isArray(call.args) ? call.args : [];
          const result = await fn_(...args);
          op_desktop_resolve_bind_call(call.callId, result ?? null);
        } catch (e) {
          op_desktop_reject_bind_call(call.callId, String(e));
        }
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
      Some(v) =>
        format!("\"{}\"", v.replace('\\', "\\\\").replace('"', "\\\"")),
      None => "null".to_string(),
    },
    rolled_back = if rolled_back { "true" } else { "false" },
  )
}

pub use deno_runtime::ops::desktop::BindCallReceiver;
pub use deno_runtime::ops::desktop::BindCallSender;
pub use deno_runtime::ops::desktop::KeyboardEventSender;
pub use deno_runtime::ops::desktop::MenuClickSender;
pub use deno_runtime::ops::desktop::PendingBindCall;
pub use deno_runtime::ops::desktop::PendingBindResponses;
pub use deno_runtime::ops::desktop::create_bind_call_channel;
pub use deno_runtime::ops::desktop::create_keyboard_event_channel;
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
  // Create keyboard event channel so op_desktop_recv_keyboard_event can work
  let (kb_tx, kb_rx) = create_keyboard_event_channel();
  state.put(kb_rx);
  state.put(kb_tx);
  // Initialize pending bind responses map
  state.put(PendingBindResponses::new());
}
