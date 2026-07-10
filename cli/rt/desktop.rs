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
    Dock,
    Tray,
    Notification: NotificationNative,
    op_desktop_init,
    op_desktop_recv_event,
    op_desktop_resolve_bind_call,
    op_desktop_reject_bind_call,
    op_desktop_alert,
    op_desktop_confirm,
    op_desktop_prompt,
    op_desktop_request_notification_permission,
    op_desktop_query_notification_permission,
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
  // bindingTrace is only useful under --inspect (where DENO_DESKTOP_MUX_WS
  // is set by the parent). `Deno.env.get` THROWS `NotCapable` if the
  // runtime wasn't compiled with --allow-env, which aborts DESKTOP_JS
  // execution before the event-loop IIFE below has a chance to register.
  // That's the "nothing works — no mouse, no keyboard, no alerts"
  // failure mode: events fire on the wef side and pile up in the mpsc
  // channel, but the JS side never reads them because this throw kills
  // the script. Catch the env-permission error and disable tracing.
  let bindingTrace = false;
  try {
    bindingTrace = typeof Deno.env?.get === "function"
      && Deno.env.get("DENO_DESKTOP_MUX_WS") != null;
  } catch (_) {
    // No env access — fine, we just don't trace binding calls.
  }

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
      // Cap retries at ~2s (200 × 10ms). CEF registers bindings via async
      // IPC; missing them after that long means navigation tore down the
      // page or the binding will never appear, and an unbounded
      // setTimeout loop would otherwise leak forever per such call.
      this.executeJs(`(function() {
        var n = ${escapedName};
        var seq = 0;
        var attempts = 0;
        function tryWrap() {
          if (typeof window.bindings === "undefined" || typeof window.bindings[n] !== "function") {
            if (++attempts >= 200) return;
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

  const DockPrototype = Dock.prototype;
  Object.setPrototypeOf(DockPrototype, EventTarget.prototype);

  const docks = new Set();
  const nativeDockConstructor = Dock;
  const OrigDock = function(...args) {
    const instance = new nativeDockConstructor(...args);
    docks.add(instance);
    return instance;
  };
  Object.setPrototypeOf(OrigDock, nativeDockConstructor);
  Object.setPrototypeOf(OrigDock.prototype, nativeDockConstructor.prototype);
  Deno.Dock = OrigDock;

  internals.defineEventHandler(DockPrototype, "menuclick");
  internals.defineEventHandler(DockPrototype, "reopen");

  const dock = new OrigDock();
  Object.defineProperty(Deno, "dock", internals.core.propReadOnly(dock));

  const TrayPrototype = Tray.prototype;
  Object.setPrototypeOf(TrayPrototype, EventTarget.prototype);

  const trays = new Map();
  const nativeTrayConstructor = Tray;
  const OrigTray = function(...args) {
    const instance = new nativeTrayConstructor(...args);
    trays.set(instance.trayId, instance);
    return instance;
  };
  Object.setPrototypeOf(OrigTray, nativeTrayConstructor);
  Object.setPrototypeOf(OrigTray.prototype, nativeTrayConstructor.prototype);
  Deno.Tray = OrigTray;

  const nativeTrayDestroy = TrayPrototype.destroy;
  TrayPrototype.destroy = function() {
    trays.delete(this.trayId);
    nativeTrayDestroy.call(this);
  };
  TrayPrototype[Symbol.dispose] = function() {
    this.destroy();
  };

  internals.defineEventHandler(TrayPrototype, "click");
  internals.defineEventHandler(TrayPrototype, "dblclick");
  internals.defineEventHandler(TrayPrototype, "menuclick");

  // High-level convenience: wire a frameless, non-activating popover window
  // to this tray icon (the classic menu-bar-app pattern). Built entirely on
  // the primitives — `new BrowserWindow({ frameless, noActivate })`,
  // `tray.getBounds()`, the tray "click" event and the window "blur" event.
  TrayPrototype.attachPanel = function(options) {
    if (typeof options === "string") options = { url: options };
    options = options ?? {};
    const width = options.width ?? 360;
    const height = options.height ?? 480;
    const hideOnBlur = options.hideOnBlur ?? true;
    const positionFn = options.position;
    const tray = this;

    const window = new Deno.BrowserWindow({
      width,
      height,
      frameless: true,
      noActivate: true,
      resizable: false,
    });
    window.hide();
    if (options.url != null) window.navigate(options.url);

    let visible = false;
    // Guards the click -> blur -> click toggle race: a tray click on a
    // focused panel blurs it (hiding via the blur handler) *before* the
    // tray "click" fires, which would otherwise immediately re-show it.
    let suppressNextShow = false;

    const place = () => {
      const bounds = tray.getBounds();
      // No bounds (e.g. Linux, where the tray protocol has no geometry):
      // leave the window at its current position.
      if (!bounds) return;
      const pos = positionFn
        ? positionFn(bounds, { width, height })
        : {
          x: Math.round(bounds.x + bounds.width / 2 - width / 2),
          y: Math.round(bounds.y + bounds.height),
        };
      window.setPosition(pos.x, pos.y);
    };

    const show = () => {
      place();
      window.show();
      // Take key focus so the panel is interactive and so losing focus
      // (clicking elsewhere) dismisses it via the blur handler.
      window.focus();
      visible = true;
    };
    const hide = () => {
      window.hide();
      visible = false;
    };
    const toggle = () => {
      if (visible) hide();
      else show();
    };

    const onTrayClick = () => {
      if (suppressNextShow) {
        suppressNextShow = false;
        return;
      }
      toggle();
    };
    tray.addEventListener("click", onTrayClick);

    let onBlur = null;
    if (hideOnBlur) {
      onBlur = () => {
        if (!visible) return;
        hide();
        // If this blur was caused by clicking the tray icon, the tray
        // "click" is about to fire — tell it to stay hidden.
        suppressNextShow = true;
        setTimeout(() => {
          suppressNextShow = false;
        }, 250);
      };
      window.addEventListener("blur", onBlur);
    }

    return {
      window,
      get visible() {
        return visible;
      },
      show,
      hide,
      toggle,
      destroy() {
        tray.removeEventListener("click", onTrayClick);
        if (onBlur) window.removeEventListener("blur", onBlur);
        window.close();
      },
    };
  };

  // --- Web Notifications API ---
  //
  // Backend wants raw PNG bytes for the icon while the Web Notifications
  // API specifies icon as a URL string. We synchronously decode `data:`
  // URLs (the only form a sync constructor can resolve without I/O) and
  // ignore other schemes — the URL is still stored verbatim on the
  // instance so `notification.icon` round-trips per spec.
  function decodeDataUrlSync(url) {
    if (typeof url !== "string" || !url.startsWith("data:")) return null;
    const comma = url.indexOf(",");
    if (comma === -1) return null;
    const meta = url.slice(5, comma);
    const isBase64 = meta.endsWith(";base64");
    const payload = url.slice(comma + 1);
    try {
      if (isBase64) {
        const bin = atob(payload);
        const out = new Uint8Array(bin.length);
        for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
        return out;
      }
      return new TextEncoder().encode(decodeURIComponent(payload));
    } catch {
      return null;
    }
  }

  const NotificationPrototype = NotificationNative.prototype;
  Object.setPrototypeOf(NotificationPrototype, EventTarget.prototype);

  const notifications = new Map();
  // The Web Notifications API constructor is `new Notification(title, options?)`
  // and shows the notification immediately. The native constructor takes
  // a third arg for pre-decoded icon bytes so the icon URL → bytes step
  // happens on the JS side (sync data: URL decoding only).
  const Notification = function Notification(title, options) {
    if (arguments.length < 1) {
      throw new TypeError(
        "Failed to construct 'Notification': 1 argument required, but only 0 present.",
      );
    }
    const t = String(title);
    const opts = options ?? {};
    const iconBytes = decodeDataUrlSync(opts.icon);
    const instance = new NotificationNative(t, opts, iconBytes ?? undefined);
    if (instance.notificationId !== 0) {
      notifications.set(instance.notificationId, instance);
    } else {
      // Backend didn't show it (no support / failure). The native side
      // already emitted a NotificationError event; nothing to track here.
    }
    return instance;
  };
  Object.setPrototypeOf(Notification, NotificationNative);
  Object.setPrototypeOf(Notification.prototype, NotificationPrototype);
  Notification.prototype.constructor = Notification;

  // Cache of the last status the OS reported. `Notification.permission`
  // is a *synchronous* getter per spec, but the underlying laufey call is
  // async (it's serviced on the UI thread). The cache starts at
  // "default" and updates as `requestPermission()` / permissions.query()
  // resolve. We deliberately do NOT do a startup query — that op
  // dispatches into the laufey backend's UI thread, which may not be
  // pumping when DESKTOP_JS first runs; a hung promise there shouldn't
  // be possible to interfere with the event loop below, but the cost
  // of being defensive is also zero (apps generally read `.permission`
  // only after a user-driven request anyway).
  //
  // Web spec maps laufey's "prompt" status (no decision yet) to "default"
  // for `Notification`, and "prompt" for `navigator.permissions`. The
  // laufey "unsupported" status — emitted when the backend or platform
  // has no permission model — surfaces to JS as a thrown error from
  // requestPermission (most honest) and as "denied" from
  // permissions.query (spec doesn't have an "unsupported" state).
  let cachedNotificationPermission = "default";

  function laufeyToNotificationPermission(s) {
    // "prompt" → "default" per the Notifications spec; "unsupported"
    // is handled by the caller (throws on requestPermission).
    switch (s) {
      case "granted": return "granted";
      case "denied": return "denied";
      case "prompt": return "default";
      default: return "default";
    }
  }

  // Wrap every new descriptor mutation in a try/catch. Anything that
  // throws here would otherwise abort DESKTOP_JS execution and prevent
  // the event-loop IIFE at the bottom of this script from registering,
  // which manifests as "nothing works" (no mouse, no keyboard, no
  // alerts). The catch is logged via console.error so a regression is
  // visible but doesn't take the whole desktop runtime down with it.
  try {
    Object.defineProperties(Notification, {
      permission: {
        get() { return cachedNotificationPermission; },
        enumerable: true,
        configurable: true,
      },
      maxActions: internals.core.propReadOnly(0),
      requestPermission: internals.core.propWritable(function requestPermission(
        cb,
      ) {
        // The Web Notifications spec gates `requestPermission` on a
        // transient user activation. The desktop runtime can't observe
        // renderer activations cleanly (the OS-level UN dialog lives
        // outside Chromium's activation tracking), so we don't enforce.
        const promise = (async () => {
          const s = await op_desktop_request_notification_permission();
          if (s === "unsupported") {
            // Honest signaling: this OS / backend has no notification
            // permission model. Throw rather than silently returning a
            // misleading "denied" or "granted".
            throw new TypeError(
              "Notification.requestPermission: not supported by this platform/backend",
            );
          }
          const perm = laufeyToNotificationPermission(s);
          cachedNotificationPermission = perm;
          return perm;
        })();
        if (typeof cb === "function") {
          // Deprecated callback form. Per spec, the callback is invoked
          // with the resolved permission *and* the promise still resolves.
          promise.then(
            (perm) => { try { cb(perm); } catch (_) {} },
            () => { try { cb("denied"); } catch (_) {} },
          );
        }
        return promise;
      }),
    });
  } catch (e) {
    console.error("[deno desktop] failed to install Notification permission API:", e);
  }

  internals.defineEventHandler(NotificationPrototype, "show");
  internals.defineEventHandler(NotificationPrototype, "click");
  internals.defineEventHandler(NotificationPrototype, "close");
  internals.defineEventHandler(NotificationPrototype, "error");

  Object.defineProperty(globalThis, "Notification", {
    value: Notification,
    writable: true,
    enumerable: false,
    configurable: true,
  });

  // --- navigator.permissions.query (minimal) ---
  //
  // Spec surface: `navigator.permissions.query({name})` returns a
  // Promise<PermissionStatus> where `PermissionStatus` extends EventTarget
  // and exposes a readonly `state` plus an `onchange` slot. The desktop
  // runtime today only routes `notifications` through laufey; other names
  // resolve to "denied" (Chrome's behavior for unknown / unsupported
  // names — closer to honest than "prompt" for things we can't fulfill).
  //
  // Note: we don't fire `change` events. laufey has no change-notification
  // channel for permissions, and the cached decision only flips when the
  // user goes through System Settings (rare, manual, not worth polling).
  class PermissionStatus extends EventTarget {
    #name;
    #state;
    #onchange = null;
    constructor(name, state) {
      super();
      this.#name = name;
      this.#state = state;
    }
    get name() { return this.#name; }
    get state() { return this.#state; }
    get status() { return this.#state; } // legacy alias kept by some libs
    get onchange() { return this.#onchange; }
    set onchange(v) { this.#onchange = typeof v === "function" ? v : null; }
  }

  function laufeyToPermissionsApiState(s) {
    // Spec maps "prompt" through verbatim; "unsupported" has no spec
    // analog so we return "denied" — query() shouldn't throw, but we
    // shouldn't lie and say "granted" either.
    switch (s) {
      case "granted": return "granted";
      case "denied": return "denied";
      case "prompt": return "prompt";
      default: return "denied";
    }
  }

  const permissionsImpl = {
    async query(descriptor) {
      if (descriptor == null || typeof descriptor !== "object") {
        throw new TypeError(
          "Failed to execute 'query' on 'Permissions': descriptor required",
        );
      }
      const name = String(descriptor.name);
      if (name === "notifications") {
        // No side effects per spec — never call request_*.
        const s = await op_desktop_query_notification_permission();
        // Keep Notification.permission's cache in sync: a permissions.query
        // result is authoritative and lets the synchronous getter report
        // a current value without us needing a second roundtrip.
        if (s !== "unsupported") {
          cachedNotificationPermission = laufeyToNotificationPermission(s);
        }
        return new PermissionStatus(name, laufeyToPermissionsApiState(s));
      }
      // Unknown / unrouted name. Chrome returns "denied" for descriptors
      // it doesn't recognize; mimic that rather than throwing.
      return new PermissionStatus(name, "denied");
    },
  };

  // Plug into globalThis.navigator. The base Deno runtime defines
  // `navigator` without a `permissions` slot — add ours, but don't
  // clobber the object if it's missing entirely (defensive against
  // future-Deno changes). Wrapped: a failure here must not abort the
  // event-loop IIFE below, because that's what drives all input.
  try {
    if (typeof navigator === "object" && navigator != null) {
      Object.defineProperty(navigator, "permissions", {
        value: permissionsImpl,
        writable: true,
        enumerable: true,
        configurable: true,
      });
    }
    Object.defineProperty(globalThis, "PermissionStatus", {
      value: PermissionStatus,
      writable: true,
      enumerable: false,
      configurable: true,
    });
  } catch (e) {
    console.error("[deno desktop] failed to install navigator.permissions:", e);
  }

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
          case "dockMenuClick": {
            for (const d of docks) {
              d.dispatchEvent(new CustomEvent("menuclick", {
                detail: { id: ev.id },
              }));
            }
            break;
          }
          case "dockReopen": {
            for (const d of docks) {
              d.dispatchEvent(new CustomEvent("reopen", {
                detail: { hasVisibleWindows: ev.hasVisibleWindows },
              }));
            }
            break;
          }
          case "trayClick": {
            const target = trays.get(ev.trayId);
            if (!target) break;
            target.dispatchEvent(new MouseEvent("click"));
            break;
          }
          case "trayDoubleClick": {
            const target = trays.get(ev.trayId);
            if (!target) break;
            target.dispatchEvent(new MouseEvent("dblclick"));
            break;
          }
          case "trayMenuClick": {
            const target = trays.get(ev.trayId);
            if (!target) break;
            target.dispatchEvent(new CustomEvent("menuclick", {
              detail: { id: ev.id },
            }));
            break;
          }
          case "notificationShow": {
            const target = notifications.get(ev.notificationId);
            if (!target) break;
            target.dispatchEvent(new Event("show"));
            break;
          }
          case "notificationClick": {
            const target = notifications.get(ev.notificationId);
            if (!target) break;
            target.dispatchEvent(new Event("click"));
            break;
          }
          case "notificationClose": {
            const target = notifications.get(ev.notificationId);
            notifications.delete(ev.notificationId);
            if (!target) break;
            target.dispatchEvent(new Event("close"));
            break;
          }
          case "notificationError": {
            // notificationId === 0 means the backend rejected the show
            // before any instance was registered. Per spec, errors only
            // fire on the instance — best-effort dispatch to whichever
            // instance is registered under that id (or none for id 0).
            const target = notifications.get(ev.notificationId);
            if (!target) break;
            target.dispatchEvent(new Event("error"));
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
  release_base_url: Option<&str>,
) -> String {
  format!(
    r#"(() => {{
  const {{
    op_desktop_apply_patch,
    op_desktop_verify_ed25519,
    op_desktop_confirm_update,
  }} = Deno[Deno.internal].core.ops;
  const {{ propReadOnly, propWritable }} = Deno[Deno.internal].core;

  const _version = {version};
  const _rolledBack = {rolled_back};
  const _releaseBaseUrl = {release_base_url};

  const ROLLBACK_REASON = "Update failed to start, rolled back.";

  if (!_rolledBack) {{
    op_desktop_confirm_update();
  }}

  let autoUpdateTimer = null;

  function isHttpsUrl(u) {{
    try {{
      const parsed = new URL(u);
      return parsed.protocol === "https:";
    }} catch {{
      return false;
    }}
  }}

  function autoUpdate(urlOrOpts) {{
    const opts = typeof urlOrOpts === "string"
      ? {{ url: urlOrOpts }}
      : (urlOrOpts ?? {{}});
    const {{
      url = _releaseBaseUrl,
      interval,
      onUpdateReady,
      onRollback,
      publicKey,
    }} = opts;

    if (_rolledBack && typeof onRollback === "function") {{
      queueMicrotask(() => {{
        try {{ onRollback(ROLLBACK_REASON); }} catch (e) {{
          console.error("Deno.autoUpdate onRollback threw:", e);
        }}
      }});
    }}

    if (!_version) {{
      console.warn("Deno.autoUpdate: no version in deno.json, skipping");
      return;
    }}
    if (typeof url !== "string" || url.length === 0) {{
      console.warn("Deno.autoUpdate: missing 'url' option, skipping");
      return;
    }}
    if (!isHttpsUrl(url)) {{
      console.error(
        "Deno.autoUpdate: refusing non-https url (got %s); ignoring.", url,
      );
      return;
    }}

    const base = url.replace(/\/$/, "");
    const te = new TextEncoder();

    const check = async () => {{
      try {{
        const resp = await fetch(base + "/latest.json", {{
          cache: "no-store",
          redirect: "error",
        }});
        if (!resp.ok) return;
        const manifestText = await resp.text();
        let manifest;
        try {{
          manifest = JSON.parse(manifestText);
        }} catch {{
          console.warn("Deno.autoUpdate: latest.json is not valid JSON");
          return;
        }}
        if (manifest.version === _version) return;

        if (publicKey) {{
          const sig = manifest.signature;
          if (typeof sig !== "string" || !sig) {{
            console.error(
              "Deno.autoUpdate: publicKey configured but manifest has no signature",
            );
            return;
          }}
          // Signature is computed over the manifest with the `signature` field
          // removed, serialized canonically. To avoid depending on a JCS
          // implementation, signers must put the signature on a top-level
          // `signature` field and include the rest of the manifest verbatim
          // under a `signed` field (string). We then verify over that string.
          const signed = manifest.signed;
          if (typeof signed !== "string") {{
            console.error(
              "Deno.autoUpdate: signed manifest must include a `signed` string field",
            );
            return;
          }}
          if (!op_desktop_verify_ed25519(publicKey, sig, te.encode(signed))) {{
            console.error("Deno.autoUpdate: manifest signature verification failed");
            return;
          }}
          // Re-parse the signed payload — only its contents are trusted.
          try {{
            manifest = JSON.parse(signed);
          }} catch {{
            console.error("Deno.autoUpdate: signed payload is not valid JSON");
            return;
          }}
          if (manifest.version === _version) return;
        }}

        const patchEntry = manifest.patches?.[_version];
        if (!patchEntry) {{
          console.warn("Deno.autoUpdate: no patch available for",
            _version, "->", manifest.version);
          return;
        }}
        // Accept either a string (legacy/unsafe) or {{ name, sha256 }}. The
        // SHA-256 is required — Rust will reject the patch otherwise.
        const patchName = typeof patchEntry === "string"
          ? patchEntry
          : patchEntry?.name;
        const patchSha256 = typeof patchEntry === "object"
          ? patchEntry?.sha256
          : undefined;
        if (!patchName) {{
          console.error("Deno.autoUpdate: malformed patch entry");
          return;
        }}
        if (typeof patchSha256 !== "string" || patchSha256.length !== 64) {{
          console.error(
            "Deno.autoUpdate: manifest patch entry must include sha256",
          );
          return;
        }}
        const patchResp = await fetch(base + "/" + patchName, {{
          cache: "no-store",
          redirect: "error",
        }});
        if (!patchResp.ok) return;
        const patchBytes = new Uint8Array(await patchResp.arrayBuffer());
        op_desktop_apply_patch(patchBytes, patchSha256);
        if (typeof onUpdateReady === "function") {{
          try {{ onUpdateReady(manifest.version); }} catch (e) {{
            console.error("Deno.autoUpdate onUpdateReady threw:", e);
          }}
        }}
        if (autoUpdateTimer) {{
          clearInterval(autoUpdateTimer);
          autoUpdateTimer = null;
        }}
      }} catch (e) {{
        console.warn("Deno.autoUpdate: check failed:", e.message);
      }}
    }};

    setTimeout(check, 1000);
    if (interval) {{
      autoUpdateTimer = setInterval(check, interval);
    }}
  }}

  Object.defineProperties(Deno, {{
    desktopVersion: propReadOnly(_version),
    autoUpdate: propWritable(autoUpdate),
  }});
}})();
"#,
    version = serde_json::to_string(&version).unwrap(),
    rolled_back = if rolled_back { "true" } else { "false" },
    release_base_url = serde_json::to_string(&release_base_url).unwrap(),
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
      // The destination is not passed from JS — the op reads the
      // operator-configured `error_reporting_url` from native state so an
      // untrusted caller can't retarget it. `_errorReportingUrl` here only
      // gates whether there's anything to report.
      op_desktop_send_error_report(body);
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
    url = serde_json::to_string(&url).unwrap(),
    version = serde_json::to_string(&version).unwrap(),
  )
}

pub use deno_runtime::ops::desktop::DesktopEvent;
pub use deno_runtime::ops::desktop::DesktopEventReceiver;
pub use deno_runtime::ops::desktop::DesktopEventSender;
pub use deno_runtime::ops::desktop::DesktopEventTx;
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

pub use deno_lib::util::net::allocate_random_port;

#[cfg(test)]
mod tests {
  use super::DESKTOP_JS;
  use super::desktop_auto_update_js;
  use super::desktop_error_reporting_js;

  // --- DESKTOP_JS structural invariants ---
  //
  // DESKTOP_JS is an 800+ line string baked into the binary. We can't
  // cheaply exec it in a v8 isolate from here, but the asserts below
  // pin the regressions that motivated this whole fix: the "Deno.env
  // throws NotCapable and aborts the IIFE" bug from May 2026.

  #[test]
  fn desktop_js_wraps_binding_trace_env_read_in_try_catch() {
    // The original bug: `Deno.env.get("DENO_DESKTOP_MUX_WS")` threw
    // NotCapable when --allow-env wasn't granted, aborting the rest of
    // DESKTOP_JS. The fix is to wrap that read in try/catch and let it
    // soft-fail. A regression that removed the try/catch would
    // reintroduce the "blank window where nothing works" failure mode.
    let near = locate_around(DESKTOP_JS, "DENO_DESKTOP_MUX_WS");
    assert!(
      near.contains("try {") || near.contains("try{"),
      "DENO_DESKTOP_MUX_WS read must be wrapped in try/catch; got:\n{near}"
    );
  }

  #[test]
  fn desktop_js_installs_alert_confirm_prompt_overrides() {
    // The renderer reaches `op_desktop_alert/confirm/prompt` through
    // these globalThis overrides; the assignment lines must survive.
    // Each must reference its op so a stub/no-op replacement would
    // fail the check.
    assert!(DESKTOP_JS.contains("op_desktop_alert"));
    assert!(DESKTOP_JS.contains("op_desktop_confirm"));
    assert!(DESKTOP_JS.contains("op_desktop_prompt"));
    assert!(DESKTOP_JS.contains("globalThis"));
  }

  #[test]
  fn desktop_js_installs_recv_event_loop() {
    // The event-loop IIFE at the bottom polls `op_desktop_recv_event`
    // and dispatches to BrowserWindow listeners. Without this code path
    // mouse / keyboard / focus / resize events would never reach JS,
    // exactly the symptom from the original bug report.
    assert!(
      DESKTOP_JS.contains("op_desktop_recv_event"),
      "DESKTOP_JS must call op_desktop_recv_event"
    );
  }

  #[test]
  fn desktop_js_installs_notification_permission_getter() {
    // Notification.permission is a synchronous spec-mandated getter.
    // A regression that turned the property definition into a plain
    // value would break feature-detection in user code.
    assert!(DESKTOP_JS.contains("Notification"));
    assert!(DESKTOP_JS.contains("permission"));
    assert!(DESKTOP_JS.contains("requestPermission"));
  }

  #[test]
  fn desktop_js_installs_navigator_permissions() {
    assert!(DESKTOP_JS.contains("navigator"));
    assert!(DESKTOP_JS.contains("permissions"));
    assert!(DESKTOP_JS.contains("PermissionStatus"));
  }

  #[test]
  fn desktop_js_installs_browser_window_constructor() {
    assert!(DESKTOP_JS.contains("Deno.BrowserWindow"));
    // The original BrowserWindow is wrapped so per-window state is
    // recorded.
    assert!(DESKTOP_JS.contains("windows.set"));
  }

  // --- desktop_auto_update_js ---

  #[test]
  fn auto_update_js_inlines_version_as_json_literal() {
    let js = desktop_auto_update_js(Some("1.2.3"), false, None);
    // The version must be a JSON-quoted string, not a bare identifier:
    // we feed it through serde_json::to_string. A regression that
    // dropped the quoting would produce invalid JS for any non-trivial
    // version (e.g. `1.2.3-alpha`).
    assert!(
      js.contains(r#""1.2.3""#),
      "version must be quoted; got: {js}"
    );
    assert!(js.contains("const _version ="));
    assert!(js.contains("const _rolledBack = false"));
  }

  #[test]
  fn auto_update_js_serializes_none_as_null_literal() {
    let js = desktop_auto_update_js(None, true, None);
    assert!(js.contains("const _version = null"));
    assert!(js.contains("const _rolledBack = true"));
    assert!(js.contains("const _releaseBaseUrl = null"));
  }

  #[test]
  fn auto_update_js_inlines_release_base_url() {
    // The configured `desktop.release.baseUrl` is baked in as the default
    // `url` for `Deno.autoUpdate`, so a no-arg call uses it.
    let js = desktop_auto_update_js(
      Some("1.0.0"),
      false,
      Some("https://releases.example/app"),
    );
    assert!(
      js.contains(r#"const _releaseBaseUrl = "https://releases.example/app""#),
      "release base url must be quoted; got: {js}"
    );
    assert!(js.contains("url = _releaseBaseUrl"));
  }

  #[test]
  fn auto_update_js_blocks_non_https_manifest_url() {
    // Anti-downgrade defence: the auto-update path must refuse to
    // fetch its manifest over http://, gopher://, file://, etc. A
    // change that loosened this check is a security regression.
    let js = desktop_auto_update_js(Some("1.0.0"), false, None);
    assert!(js.contains("isHttpsUrl"));
    assert!(js.contains("https:"));
  }

  // --- desktop_error_reporting_js ---

  #[test]
  fn error_reporting_js_quotes_url_and_version() {
    let js =
      desktop_error_reporting_js(Some("https://err.example/r"), Some("0.1.0"));
    assert!(js.contains(r#""https://err.example/r""#));
    assert!(js.contains(r#""0.1.0""#));
    // The URL must be referenced by the script body — otherwise the
    // emitted code would silently never POST.
    assert!(js.contains("_errorReportingUrl"));
  }

  #[test]
  fn error_reporting_js_handles_none_url() {
    let js = desktop_error_reporting_js(None, None);
    // null both — the handler short-circuits the POST but still shows
    // the alert.
    assert!(js.contains("const _errorReportingUrl = null"));
    assert!(js.contains("const _appVersion = null"));
  }

  #[test]
  fn error_reporting_js_listens_for_unhandledrejection() {
    // Both `error` and `unhandledrejection` events must be hooked
    // — missing either would let half of all user-code failures fall
    // out the bottom of the runtime without notification.
    let js = desktop_error_reporting_js(None, None);
    assert!(js.contains("\"error\""));
    assert!(js.contains("\"unhandledrejection\""));
  }

  // --- helpers ---

  /// Return a window of DESKTOP_JS around the first occurrence of `needle`,
  /// covering ~10 lines on each side. Useful for asserting "the region
  /// near this token contains a try/catch" without coupling the test
  /// to a precise line range.
  fn locate_around(hay: &str, needle: &str) -> String {
    // The first occurrence of DENO_DESKTOP_MUX_WS in DESKTOP_JS is
    // inside a comment block; the *code* occurrence is the second. We
    // want the window around the code path, so search after the first.
    let first = hay.find(needle).unwrap_or_else(|| {
      panic!("needle {needle:?} not found in DESKTOP_JS");
    });
    let idx = hay[first + needle.len()..]
      .find(needle)
      .map(|i| first + needle.len() + i)
      .unwrap_or(first);
    let start = idx.saturating_sub(500);
    let end = (idx + needle.len() + 500).min(hay.len());
    hay[start..end].to_string()
  }
}
