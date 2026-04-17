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
pub use deno_runtime::ops::desktop::MenuItem;

/// JS code that exposes desktop APIs via `Deno.BrowserWindow` and `Deno.desktop`.
pub const DESKTOP_JS: &str = r#"
(() => {
  const internals = Deno[Deno.internal];
  const {
    BrowserWindow,
    op_desktop_init,
    op_desktop_recv_event,
    op_desktop_resolve_bind_call,
    op_desktop_reject_bind_call,
    op_desktop_alert,
    op_desktop_confirm,
    op_desktop_prompt,
  } = internals.core.ops;
  const BrowserWindowPrototype = BrowserWindow.prototype;
  Object.setPrototypeOf(BrowserWindowPrototype, EventTarget.prototype);

  class UIEvent extends Event {
    #detail = 0;
    #view = null;

    get detail() { return this.#detail; }
    get view() { return this.#view; }

    constructor(type, init = {}) {
      super(type, init);
      this.#detail = init.detail ?? 0;
      this.#view = init.view ?? null;
    }
  }

  class FocusEvent extends UIEvent {
    #relatedTarget = null;

    get relatedTarget() { return this.#relatedTarget; }

    constructor(type, init = {}) {
      super(type, init);
      this.#relatedTarget = init.relatedTarget ?? null;
    }
  }

  class KeyboardEvent extends UIEvent {
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

  class MouseEvent extends UIEvent {
    #button = 0;
    #clientX = 0;
    #clientY = 0;
    #ctrlKey = false;
    #shiftKey = false;
    #altKey = false;
    #metaKey = false;

    get button() { return this.#button; }
    get clientX() { return this.#clientX; }
    get clientY() { return this.#clientY; }
    get screenX() { return this.#clientX; }
    get screenY() { return this.#clientY; }
    get ctrlKey() { return this.#ctrlKey; }
    get shiftKey() { return this.#shiftKey; }
    get altKey() { return this.#altKey; }
    get metaKey() { return this.#metaKey; }

    constructor(type, init = {}) {
      super(type, init);
      this.#button = init.button ?? 0;
      this.#clientX = init.clientX ?? 0;
      this.#clientY = init.clientY ?? 0;
      this.#ctrlKey = init.ctrlKey ?? false;
      this.#shiftKey = init.shiftKey ?? false;
      this.#altKey = init.altKey ?? false;
      this.#metaKey = init.metaKey ?? false;
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

  class WheelEvent extends MouseEvent {
    #deltaX = 0;
    #deltaY = 0;
    #deltaZ = 0;
    #deltaMode = 0;

    get deltaX() { return this.#deltaX; }
    get deltaY() { return this.#deltaY; }
    get deltaZ() { return this.#deltaZ; }
    get deltaMode() { return this.#deltaMode; }

    constructor(type, init = {}) {
      super(type, init);
      this.#deltaX = init.deltaX ?? 0;
      this.#deltaY = init.deltaY ?? 0;
      this.#deltaZ = init.deltaZ ?? 0;
      this.#deltaMode = init.deltaMode ?? 0;
    }
  }

  op_desktop_init(
    internals.webidlBrand,
    internals.setEventTargetData,
  );

  // Window registry: windowId -> BrowserWindow instance.
  const windows = new Map();
  const nativeConstructor = BrowserWindow;
  const OrigBW = function(...args) {
    const instance = new nativeConstructor(...args);
    const windowId = instance.windowId;
    windows.set(windowId, instance);
    return instance;
  };
  Object.setPrototypeOf(OrigBW, nativeConstructor);
  Object.setPrototypeOf(OrigBW.prototype, nativeConstructor.prototype);
  Deno.BrowserWindow = OrigBW;

  internals.defineEventHandler(BrowserWindowPrototype, "keydown");
  internals.defineEventHandler(BrowserWindowPrototype, "keyup");
  internals.defineEventHandler(BrowserWindowPrototype, "mousedown");
  internals.defineEventHandler(BrowserWindowPrototype, "mouseup");
  internals.defineEventHandler(BrowserWindowPrototype, "click");
  internals.defineEventHandler(BrowserWindowPrototype, "dblclick");
  internals.defineEventHandler(BrowserWindowPrototype, "mousemove");
  internals.defineEventHandler(BrowserWindowPrototype, "wheel");
  internals.defineEventHandler(BrowserWindowPrototype, "mouseenter");
  internals.defineEventHandler(BrowserWindowPrototype, "mouseleave");
  internals.defineEventHandler(BrowserWindowPrototype, "focus");
  internals.defineEventHandler(BrowserWindowPrototype, "blur");
  internals.defineEventHandler(BrowserWindowPrototype, "resize");
  internals.defineEventHandler(BrowserWindowPrototype, "move");
  internals.defineEventHandler(BrowserWindowPrototype, "close");
  internals.defineEventHandler(BrowserWindowPrototype, "menuclick");
  internals.defineEventHandler(BrowserWindowPrototype, "contextmenuclick");

  // Per-window bind callback registry: windowId -> Map<name, fn>
  const windowBindCallbacks = new Map();

  // Binding-call correlation: when --inspect is active, both the Deno
  // and renderer consoles emit matching console.debug messages so the
  // developer can trace a binding call across isolates.
  const bindingTrace = typeof Deno.env?.get === "function"
    && Deno.env.get("DENO_DESKTOP_MUX_WS") != null;

  const nativeBind = BrowserWindowPrototype.bind;
  BrowserWindowPrototype.bind = function(name, fn) {
    const windowId = this.windowId;
    if (!windowBindCallbacks.has(windowId)) {
      windowBindCallbacks.set(windowId, new Map());
    }
    windowBindCallbacks.get(windowId).set(name, fn.bind(this));
    nativeBind.call(this, name);

    // Inject a renderer-side wrapper that emits console.debug around
    // every binding call. The wrapper waits for the native binding to
    // appear (CEF registers it asynchronously via IPC) and then
    // replaces it with a logging shim.
    if (bindingTrace) {
      const escapedName = JSON.stringify(name);
      this.executeJs(`(function() {
        var n = ${escapedName};
        var seq = 0;
        function tryWrap() {
          if (typeof window.bindings === "undefined" || typeof window.bindings[n] !== "function") {
            setTimeout(tryWrap, 10);
            return;
          }
          var orig = window.bindings[n];
          if (orig.__bindTrace) return;
          window.bindings[n] = async function() {
            var id = ++seq;
            var args = Array.prototype.slice.call(arguments);
            console.debug("[binding:call]", n, ":" + id, args);
            try {
              var result = await orig.apply(this, arguments);
              console.debug("[binding:return]", n, ":" + id, result);
              return result;
            } catch (e) {
              console.debug("[binding:error]", n, ":" + id, e);
              throw e;
            }
          };
          window.bindings[n].__bindTrace = true;
        }
        tryWrap();
      })();`);
    }
  };

  const nativeUnbind = BrowserWindowPrototype.unbind;
  BrowserWindowPrototype.unbind = function(name) {
    const windowId = this.windowId;
    const callbacks = windowBindCallbacks.get(windowId);
    if (callbacks) callbacks.delete(name);
    nativeUnbind.call(this, name);
  };

  function alert(message = "Alert") {
    op_desktop_alert("", String(message));
  }

  function confirm(message = "Confirm") {
    return op_desktop_confirm(String(message));
  }

  function prompt(message = "Prompt", defaultValue) {
    return op_desktop_prompt(String(message), defaultValue != null ? String(defaultValue) : null);
  }


  Object.defineProperties(globalThis, {
    alert: internals.core.propWritable(alert),
    confirm: internals.core.propWritable(confirm),
    prompt: internals.core.propWritable(prompt),
    UIEvent: internals.core.propNonEnumerable(UIEvent),
    FocusEvent: internals.core.propNonEnumerable(FocusEvent),
    KeyboardEvent: internals.core.propNonEnumerable(KeyboardEvent),
    MouseEvent: internals.core.propNonEnumerable(MouseEvent),
    WheelEvent: internals.core.propNonEnumerable(WheelEvent),
  });

  // Start polling loops immediately. Use core.unrefOpPromise so these
  // pending ops don't block event loop completion (e.g. the pre-module
  // tick used by HMR, or module evaluation with top-level await).
  const { unrefOpPromise } = internals.core;

  // Single polling loop for all native desktop events.
  (async () => {
    while (true) {
      try {
        const p = op_desktop_recv_event();
        unrefOpPromise(p);
        const ev = await p;
        if (ev == null) break;
        switch (ev.kind) {
          case "appMenuClick": {
            const target = windows.get(ev.windowId);
            if (!target) break;
            target.dispatchEvent(new CustomEvent("menuclick", { detail: { id: ev.id } }));
            break;
          }
          case "contextMenuClick": {
            const target = windows.get(ev.windowId);
            if (!target) break;
            target.dispatchEvent(new CustomEvent("contextmenuclick", { detail: { id: ev.id } }));
            break;
          }
          case "keyboardEvent": {
            const target = windows.get(ev.windowId);
            if (!target) break;
            target.dispatchEvent(new KeyboardEvent(ev.type, {
              key: ev.key,
              code: ev.code,
              shiftKey: ev.shift,
              ctrlKey: ev.control,
              altKey: ev.alt,
              metaKey: ev.meta,
              repeat: ev.repeat,
            }));
            break;
          }
          case "bindCall": {
            const callbacks = windowBindCallbacks.get(ev.windowId);
            const fn_ = callbacks?.get(ev.name);
            if (!fn_) {
              op_desktop_reject_bind_call(ev.callId, "No callback bound for: " + ev.name);
              break;
            }
            // Run async so it doesn't block the event loop
            (async () => {
              try {
                const args = Array.isArray(ev.args) ? ev.args : [];
                if (bindingTrace) {
                  console.debug("[binding:call]", ev.name, ":" + ev.callId, args);
                }
                const result = await fn_(...args);
                if (bindingTrace) {
                  console.debug("[binding:return]", ev.name, ":" + ev.callId, result);
                }
                op_desktop_resolve_bind_call(ev.callId, result ?? null);
              } catch (e) {
                if (bindingTrace) {
                  console.debug("[binding:error]", ev.name, ":" + ev.callId, String(e));
                }
                op_desktop_reject_bind_call(ev.callId, String(e));
              }
            })();
            break;
          }
          case "mouseClick": {
            const target = windows.get(ev.windowId);
            if (!target) break;
            const init = {
              button: ev.button,
              clientX: ev.clientX,
              clientY: ev.clientY,
              ctrlKey: ev.control,
              shiftKey: ev.shift,
              altKey: ev.alt,
              metaKey: ev.meta,
              detail: ev.clickCount,
            };
            if (ev.state === "pressed") {
              target.dispatchEvent(new MouseEvent("mousedown", init));
            } else {
              target.dispatchEvent(new MouseEvent("mouseup", init));
              if (ev.button === 0) {
                target.dispatchEvent(new MouseEvent("click", init));
                if (ev.clickCount >= 2) {
                  target.dispatchEvent(new MouseEvent("dblclick", init));
                }
              }
            }
            break;
          }
          case "mouseMove": {
            const target = windows.get(ev.windowId);
            if (!target) break;
            target.dispatchEvent(new MouseEvent("mousemove", {
              clientX: ev.clientX,
              clientY: ev.clientY,
              ctrlKey: ev.control,
              shiftKey: ev.shift,
              altKey: ev.alt,
              metaKey: ev.meta,
            }));
            break;
          }
          case "wheel": {
            const target = windows.get(ev.windowId);
            if (!target) break;
            target.dispatchEvent(new WheelEvent("wheel", {
              deltaX: ev.deltaX,
              deltaY: ev.deltaY,
              deltaMode: ev.deltaMode,
              clientX: ev.clientX,
              clientY: ev.clientY,
              ctrlKey: ev.control,
              shiftKey: ev.shift,
              altKey: ev.alt,
              metaKey: ev.meta,
            }));
            break;
          }
          case "cursorEnterLeave": {
            const target = windows.get(ev.windowId);
            if (!target) break;
            const init = {
              clientX: ev.clientX,
              clientY: ev.clientY,
              ctrlKey: ev.control,
              shiftKey: ev.shift,
              altKey: ev.alt,
              metaKey: ev.meta,
            };
            target.dispatchEvent(new MouseEvent(
              ev.entered ? "mouseenter" : "mouseleave", init));
            break;
          }
          case "focusChanged": {
            const target = windows.get(ev.windowId);
            if (!target) break;
            target.dispatchEvent(new FocusEvent(ev.focused ? "focus" : "blur"));
            break;
          }
          case "windowResize": {
            const target = windows.get(ev.windowId);
            if (!target) break;
            target.dispatchEvent(new CustomEvent("resize", {
              detail: { width: ev.width, height: ev.height },
            }));
            break;
          }
          case "windowMove": {
            const target = windows.get(ev.windowId);
            if (!target) break;
            target.dispatchEvent(new CustomEvent("move", {
              detail: { x: ev.x, y: ev.y },
            }));
            break;
          }
          case "closeRequested": {
            const target = windows.get(ev.windowId);
            if (!target) break;
            target.dispatchEvent(new Event("close"));
            break;
          }
          case "runtimeError": {
            dispatchEvent(new ErrorEvent("error", {
              message: ev.message,
              error: new Error(ev.message),
            }));
            break;
          }
        }
      } catch (e) {
        console.error("Desktop event loop error:", e?.stack ?? e);
      }
    }
  })();
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

/// JS code that initializes error reporting. Installs `"error"` and
/// `"unhandledrejection"` listeners that show a native alert and
/// optionally POST error reports to a configured URL.
pub fn desktop_error_reporting_js(
  url: Option<&str>,
  version: Option<&str>,
) -> String {
  format!(
    r#"(() => {{
  const {{ op_desktop_alert, op_desktop_send_error_report }} = Deno[Deno.internal].core.ops;
  const _errorReportingUrl = {url};
  const _appVersion = {version};

  function handleError(message, stack) {{
    if (_errorReportingUrl) {{
      const body = JSON.stringify({{
        version: 1,
        message: String(message),
        stack: stack ?? null,
        appVersion: _appVersion,
        timestamp: new Date().toISOString(),
        platform: Deno.build.os,
        arch: Deno.build.arch,
      }});
      op_desktop_send_error_report(_errorReportingUrl, body);
    }}

    try {{
      op_desktop_alert("Application Error", String(message));
    }} catch (_) {{}}
  }}

  addEventListener("error", (ev) => {{
    if (ev.defaultPrevented) return;
    const err = ev.error;
    handleError(
      err?.message ?? ev.message ?? "Unknown error",
      err?.stack ?? null,
    );
  }});

  addEventListener("unhandledrejection", (ev) => {{
    if (ev.defaultPrevented) return;
    const err = ev.reason;
    handleError(
      err?.message ?? String(err ?? "Unhandled promise rejection"),
      err?.stack ?? null,
    );
  }});
}})();
"#,
    url = match url {
      Some(u) =>
        format!("\"{}\"", u.replace('\\', "\\\\").replace('"', "\\\"")),
      None => "null".to_string(),
    },
    version = match version {
      Some(v) =>
        format!("\"{}\"", v.replace('\\', "\\\\").replace('"', "\\\"")),
      None => "null".to_string(),
    },
  )
}

pub use deno_runtime::ops::desktop::DesktopEvent;
pub use deno_runtime::ops::desktop::DesktopEventReceiver;
pub use deno_runtime::ops::desktop::DesktopEventSender;
pub use deno_runtime::ops::desktop::InitialWindowId;
pub use deno_runtime::ops::desktop::PendingBindCall;
pub use deno_runtime::ops::desktop::PendingBindResponses;
pub use deno_runtime::ops::desktop::create_desktop_event_channel;
pub use deno_runtime::ops::desktop::register_bind_call;

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
}
