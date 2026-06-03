// Comprehensive desktop feature test — exercises every BrowserWindow API.

// ── State ──────────────────────────────────────────────────────────────────

const testResults = new Map<
  string,
  { pass: boolean; detail: string }
>();
const eventLog: Record<string, unknown>[] = [];
let mouseMoveCount = 0;
let eventSeq = 0;

function record(name: string, pass: boolean, detail = "") {
  testResults.set(name, { pass, detail });
}

function pushEvent(entry: Record<string, unknown>) {
  entry.ts = Date.now();
  entry.seq = ++eventSeq;
  eventLog.push(entry);
  if (eventLog.length > 200) eventLog.shift();
}

// ── Window ─────────────────────────────────────────────────────────────────

const win = new Deno.BrowserWindow({
  title: "Desktop Feature Test",
  width: 1100,
  height: 800,
  x: 50,
  y: 50,
  resizable: true,
  alwaysOnTop: false,
});

// ── Auto Tests (sync properties) ───────────────────────────────────────────

function runAutoTests() {
  // windowId
  try {
    const id = win.windowId;
    record("windowId", typeof id === "number" && id >= 0, `${id}`);
  } catch (e) {
    record("windowId", false, String(e));
  }

  // isResizable / setResizable
  try {
    const before = win.isResizable();
    win.setResizable(false);
    const after = win.isResizable();
    win.setResizable(true);
    record(
      "resizable",
      before === true && after === false,
      `before=${before} after=${after}`,
    );
  } catch (e) {
    record("resizable", false, String(e));
  }

  // isAlwaysOnTop / setAlwaysOnTop
  try {
    const before = win.isAlwaysOnTop();
    win.setAlwaysOnTop(true);
    const after = win.isAlwaysOnTop();
    win.setAlwaysOnTop(false);
    record(
      "alwaysOnTop",
      before === false && after === true,
      `before=${before} after=${after}`,
    );
  } catch (e) {
    record("alwaysOnTop", false, String(e));
  }

  // isVisible / hide / show
  try {
    const before = win.isVisible();
    win.hide();
    const hidden = win.isVisible();
    win.show();
    record(
      "visibility",
      before === true && hidden === false,
      `before=${before} hidden=${hidden}`,
    );
  } catch (e) {
    record("visibility", false, String(e));
  }

  // setTitle
  try {
    win.setTitle("Test Title Changed");
    win.setTitle("Desktop Feature Test");
    record("setTitle", true, "no crash");
  } catch (e) {
    record("setTitle", false, String(e));
  }

  // getSize (known WEF bug: may return [0,0])
  try {
    const size = win.getSize();
    const isArr = Array.isArray(size) && size.length === 2;
    record(
      "getSize",
      isArr,
      `[${size}]${size[0] === 0 ? " (WEF cross-thread bug)" : ""}`,
    );
  } catch (e) {
    record("getSize", false, String(e));
  }

  // setSize
  try {
    win.setSize(1100, 800);
    record("setSize", true, "no crash");
  } catch (e) {
    record("setSize", false, String(e));
  }

  // getPosition (known WEF bug: may return [0,0])
  try {
    const pos = win.getPosition();
    const isArr = Array.isArray(pos) && pos.length === 2;
    record(
      "getPosition",
      isArr,
      `[${pos}]${pos[0] === 0 ? " (WEF cross-thread bug)" : ""}`,
    );
  } catch (e) {
    record("getPosition", false, String(e));
  }

  // setPosition
  try {
    win.setPosition(50, 50);
    record("setPosition", true, "no crash");
  } catch (e) {
    record("setPosition", false, String(e));
  }

  // focus
  try {
    win.focus();
    record("focus", true, "no crash");
  } catch (e) {
    record("focus", false, String(e));
  }

  // openDevtools — also verifies the singleton fix (#21): a second
  // call should NOT spawn another DevTools window. We can't observe
  // the window count from JS, but we can at least verify both calls
  // return cleanly.
  try {
    win.openDevtools();
    win.openDevtools();
    record("openDevtools", true, "called twice, singleton ⇒ one window");
  } catch (e) {
    record("openDevtools", false, String(e));
  }

  // isClosed — used to be `todo!` in Rust, now backed by the
  // closed_windows set on the WefDesktopApi.
  try {
    const closed = win.isClosed();
    record(
      "isClosed",
      closed === false,
      `live window reports closed=${closed}`,
    );
  } catch (e) {
    record("isClosed", false, String(e));
  }
}

runAutoTests();

// ── Async Tests (executeJs) ────────────────────────────────────────────────

async function runAsyncTests() {
  // executeJs: simple arithmetic
  try {
    const r = await win.executeJs("1 + 1");
    // r is { ok: boolean, value: any }
    const ok = (r as any).ok === true && (r as any).value === 2;
    record("executeJs:arithmetic", ok, JSON.stringify(r));
  } catch (e) {
    record("executeJs:arithmetic", false, String(e));
  }

  // executeJs: error
  try {
    const r = await win.executeJs("throw new Error('test error')");
    const ok = (r as any).ok === false;
    record("executeJs:error", ok, JSON.stringify(r));
  } catch (e) {
    // if it throws instead of returning { ok: false }, that's also valid
    record("executeJs:error", true, `threw: ${e}`);
  }

  // executeJs: complex value
  try {
    const r = await win.executeJs("({a: 1, b: [2, 3]})");
    const v = (r as any).ok !== undefined ? (r as any).value : r;
    const ok = v && typeof v === "object" && v.a === 1 && Array.isArray(v.b);
    record("executeJs:complex", ok, JSON.stringify(r));
  } catch (e) {
    record("executeJs:complex", false, String(e));
  }

  // executeJs: string value
  try {
    const r = await win.executeJs("document.title");
    const v = (r as any).ok !== undefined ? (r as any).value : r;
    record("executeJs:string", typeof v === "string", JSON.stringify(r));
  } catch (e) {
    record("executeJs:string", false, String(e));
  }
}

// Run after a short delay to let the page load
setTimeout(() => runAsyncTests(), 1500);

// ── Bindings ───────────────────────────────────────────────────────────────

// deno-lint-ignore no-explicit-any
win.bind("echo", async (...args: any[]) => {
  return args.length === 1 ? args[0] : args;
});

// deno-lint-ignore no-explicit-any
win.bind("add", async (a: any, b: any) => {
  return (a as number) + (b as number);
});

win.bind("getTestResults", async () => {
  return Object.fromEntries(testResults);
});

win.bind("getEventLog", async () => {
  return eventLog.slice(-100);
});

win.bind("triggerAutoTests", async () => {
  runAutoTests();
  await runAsyncTests();
  return Object.fromEntries(testResults);
});

// Context menu items
const contextMenuItems: Deno.MenuItem[] = [
  { item: { label: "Option A", id: "ctx-a", enabled: true } },
  { item: { label: "Option B", id: "ctx-b", enabled: true } },
  { separator: null } as unknown as Deno.MenuItem,
  {
    submenu: {
      label: "More",
      items: [
        { item: { label: "Option C", id: "ctx-c", enabled: true } },
        { item: { label: "Option D", id: "ctx-d", enabled: true } },
      ],
    },
  },
];

// deno-lint-ignore no-explicit-any
win.bind("triggerContextMenu", async (x: any, y: any) => {
  win.showContextMenu(x as number, y as number, contextMenuItems);
  return null;
});

// Handle right-click from Deno side (WEF may not forward contextmenu to webview)
win.addEventListener("mousedown", (e) => {
  if (e.button === 2) {
    win.showContextMenu(e.clientX, e.clientY, contextMenuItems);
  }
});

win.bind("showAlert", async (msg: unknown) => {
  alert(String(msg));
  return "done";
});

win.bind("showConfirm", async (msg: unknown) => {
  return confirm(String(msg));
});

win.bind("showPrompt", async (msg: unknown, def: unknown) => {
  return prompt(String(msg), def != null ? String(def) : undefined);
});

// ── Dock (singleton at Deno.dock) ───────────────────────────────────────────

// macOS: NSDockTile badge / NSApp.requestUserAttention. Other platforms
// degrade to taskbar/title-prefix tweaks per the WEF backend; the JS
// shape is the same.

win.bind("dockSetBadge", async (text: unknown) => {
  Deno.dock.setBadge(String(text));
  return true;
});

win.bind("dockClearBadge", async () => {
  Deno.dock.setBadge("");
  return true;
});

win.bind("dockBounce", async (critical: unknown) => {
  Deno.dock.bounce(critical === true);
  return true;
});

const dockMenu: Deno.MenuItem[] = [
  { item: { label: "Dock Item A", id: "dock-a", enabled: true } },
  { item: { label: "Dock Item B", id: "dock-b", enabled: true } },
  { separator: null } as unknown as Deno.MenuItem,
  { item: { label: "Disabled Item", id: "dock-disabled", enabled: false } },
];

win.bind("dockSetMenu", async () => {
  Deno.dock.setMenu(dockMenu);
  return true;
});

win.bind("dockHide", async () => {
  Deno.dock.setVisible(false);
  return true;
});

win.bind("dockShow", async () => {
  Deno.dock.setVisible(true);
  return true;
});

Deno.dock.addEventListener("menuclick", (e: CustomEvent) => {
  pushEvent({ type: "dockmenuclick", id: e.detail.id });
});

Deno.dock.addEventListener("reopen", (e: CustomEvent) => {
  pushEvent({
    type: "dockreopen",
    hasVisibleWindows: e.detail.hasVisibleWindows,
  });
});

// ── Tray ────────────────────────────────────────────────────────────────────

// A visible tray icon, generated at runtime so we don't bundle an image
// file: a filled black circle on a transparent field, encoded as a valid
// RGBA PNG. macOS renders it as a template image (adapts to the menu bar's
// light/dark appearance); other platforms show it as-is.
function makeTrayIconPng(size: number): Uint8Array {
  const rowLen = 1 + size * 4;
  const raw = new Uint8Array(rowLen * size);
  const center = (size - 1) / 2;
  const radius = size / 2 - 1;
  for (let y = 0; y < size; y++) {
    const rowStart = y * rowLen;
    raw[rowStart] = 0; // filter: none
    for (let x = 0; x < size; x++) {
      const dx = x - center;
      const dy = y - center;
      const inside = dx * dx + dy * dy <= radius * radius;
      const p = rowStart + 1 + x * 4;
      raw[p + 3] = inside ? 0xff : 0x00; // opaque black inside, else clear
    }
  }

  const crcTable = new Uint32Array(256);
  for (let n = 0; n < 256; n++) {
    let v = n;
    for (let k = 0; k < 8; k++) v = v & 1 ? 0xedb88320 ^ (v >>> 1) : v >>> 1;
    crcTable[n] = v >>> 0;
  }
  const crc32 = (data: Uint8Array): number => {
    let v = 0xffffffff;
    for (let i = 0; i < data.length; i++) {
      v = crcTable[(v ^ data[i]) & 0xff] ^ (v >>> 8);
    }
    return (v ^ 0xffffffff) >>> 0;
  };

  // zlib stream: header + one stored deflate block + adler32 checksum.
  const len = raw.length;
  const zlib = new Uint8Array(2 + 5 + len + 4);
  zlib[0] = 0x78;
  zlib[1] = 0x01;
  zlib[2] = 0x01; // BFINAL=1, BTYPE=00 (stored)
  zlib[3] = len & 0xff;
  zlib[4] = (len >> 8) & 0xff;
  const nlen = ~len & 0xffff;
  zlib[5] = nlen & 0xff;
  zlib[6] = (nlen >> 8) & 0xff;
  zlib.set(raw, 7);
  let a = 1;
  let b = 0;
  for (let i = 0; i < len; i++) {
    a = (a + raw[i]) % 65521;
    b = (b + a) % 65521;
  }
  const adler = ((b << 16) | a) >>> 0;
  const ao = 7 + len;
  zlib[ao] = (adler >>> 24) & 0xff;
  zlib[ao + 1] = (adler >>> 16) & 0xff;
  zlib[ao + 2] = (adler >>> 8) & 0xff;
  zlib[ao + 3] = adler & 0xff;

  const chunk = (type: string, data: Uint8Array): Uint8Array => {
    const typeBytes = new TextEncoder().encode(type);
    const body = new Uint8Array(typeBytes.length + data.length);
    body.set(typeBytes, 0);
    body.set(data, typeBytes.length);
    const out = new Uint8Array(4 + body.length + 4);
    const dv = new DataView(out.buffer);
    dv.setUint32(0, data.length);
    out.set(body, 4);
    dv.setUint32(4 + body.length, crc32(body));
    return out;
  };

  const ihdr = new Uint8Array(13);
  const idv = new DataView(ihdr.buffer);
  idv.setUint32(0, size);
  idv.setUint32(4, size);
  ihdr[8] = 8; // bit depth
  ihdr[9] = 6; // color type: RGBA

  const sig = new Uint8Array([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);
  const parts = [
    sig,
    chunk("IHDR", ihdr),
    chunk("IDAT", zlib),
    chunk("IEND", new Uint8Array(0)),
  ];
  let total = 0;
  for (const p of parts) total += p.length;
  const png = new Uint8Array(total);
  let off = 0;
  for (const p of parts) {
    png.set(p, off);
    off += p.length;
  }
  return png;
}

const TRAY_ICON_PNG = makeTrayIconPng(16);

const trayMenu: Deno.MenuItem[] = [
  { item: { label: "Tray Item A", id: "tray-a", enabled: true } },
  { item: { label: "Tray Item B", id: "tray-b", enabled: true } },
  { separator: null } as unknown as Deno.MenuItem,
  { item: { label: "Quit", id: "tray-quit", enabled: true } },
];

let activeTray: Deno.Tray | null = null;
let trayPanel: Deno.TrayPanel | null = null;

// Self-contained content for the tray popover panel (no server route needed).
const PANEL_HTML =
  `<!DOCTYPE html><html><head><meta charset="utf-8"></head><body style="margin:0;` +
  `height:100vh;display:flex;flex-direction:column;align-items:center;` +
  `justify-content:center;gap:6px;font-family:system-ui,sans-serif;` +
  `background:#1e293b;color:#fff">` +
  `<div style="font-size:18px;font-weight:600">Tray Panel</div>` +
  `<div style="font-size:12px;opacity:.7">frameless &middot; non-activating</div>` +
  `<div style="font-size:11px;opacity:.5">click outside to dismiss</div>` +
  `</body></html>`;
const PANEL_URL = "data:text/html;charset=utf-8," +
  encodeURIComponent(PANEL_HTML);

win.bind("trayCreate", async () => {
  if (activeTray) return { ok: true, trayId: activeTray.trayId, reused: true };
  const tray = new Deno.Tray();
  tray.setIcon(TRAY_ICON_PNG);
  tray.setTooltip("Desktop Feature Test");
  tray.setMenu(trayMenu);
  tray.addEventListener("click", () => {
    pushEvent({ type: "trayclick", trayId: tray.trayId });
  });
  tray.addEventListener("dblclick", () => {
    pushEvent({ type: "traydblclick", trayId: tray.trayId });
  });
  tray.addEventListener("menuclick", (e: CustomEvent) => {
    pushEvent({
      type: "traymenuclick",
      trayId: tray.trayId,
      id: e.detail.id,
    });
  });
  activeTray = tray;
  return { ok: true, trayId: tray.trayId, reused: false };
});

win.bind("trayDestroy", async () => {
  if (!activeTray) return { ok: false, reason: "no active tray" };
  if (trayPanel) {
    trayPanel.destroy();
    trayPanel = null;
  }
  activeTray.destroy();
  activeTray = null;
  return { ok: true };
});

win.bind("traySetTooltip", async (text: unknown) => {
  if (!activeTray) return { ok: false, reason: "no active tray" };
  activeTray.setTooltip(String(text));
  return { ok: true };
});

// Tray.getBounds(): the icon's screen rectangle, used to anchor a popover.
// null on platforms that can't report it (Linux).
win.bind("trayGetBounds", async () => {
  if (!activeTray) return { ok: false, reason: "no active tray" };
  return { ok: true, bounds: activeTray.getBounds() };
});

// Tray.attachPanel(): the menu-bar-app convenience — a frameless,
// non-activating popover that toggles on tray click and hides on blur.
win.bind("trayAttachPanel", async () => {
  if (!activeTray) return { ok: false, reason: "no active tray" };
  if (trayPanel) return { ok: true, reused: true };
  trayPanel = activeTray.attachPanel({
    url: PANEL_URL,
    width: 280,
    height: 160,
  });
  return { ok: true, reused: false, windowId: trayPanel.window.windowId };
});

win.bind("trayPanelToggle", async () => {
  if (!trayPanel) return { ok: false, reason: "no panel attached" };
  trayPanel.toggle();
  return { ok: true, visible: trayPanel.visible };
});

win.bind("trayDetachPanel", async () => {
  if (!trayPanel) return { ok: false, reason: "no panel attached" };
  trayPanel.destroy();
  trayPanel = null;
  return { ok: true };
});

// ── Notifications (Web Notifications API) ──────────────────────────────────

// `Notification` is the standard Web Notifications API. The desktop runtime
// routes it through the backend's native UN/snore/winrt path. Permission
// state is cached on `Notification.permission`; `requestPermission()` is the
// async spec entry point. Events: show/click/close/error.

// Track active notifications by a JS-side id so the UI can close them. The
// underlying instance is what owns the resource — we just keep references.
const activeNotifications = new Map<number, Notification>();
let notificationCounter = 0;

win.bind("notificationPermission", async () => {
  // `Notification.permission` is a cached synchronous getter. Hit
  // `navigator.permissions.query` so we observe the *current* OS state
  // (this also refreshes the cache). The query path additionally lets
  // us distinguish "unsupported" (process not bundled / wrong bundle id)
  // from a real "denied" — the Notifications spec collapses those, but
  // the permissions API surfaces the distinction via the "unsupported"
  // wef status before it gets remapped.
  const status = await (navigator as unknown as {
    permissions: {
      query: (d: { name: string }) => Promise<{ state: string }>;
    };
  }).permissions.query({ name: "notifications" });
  return {
    cached: Notification.permission,
    permissionsApi: status.state,
  };
});

win.bind("notificationRequestPermission", async () => {
  try {
    const perm = await Notification.requestPermission();
    return { ok: true, permission: perm };
  } catch (e) {
    return { ok: false, error: String(e) };
  }
});

win.bind(
  "notificationShow",
  // deno-lint-ignore no-explicit-any
  async (opts: any) => {
    const o = (opts ?? {}) as {
      title?: string;
      body?: string;
      tag?: string;
      requireInteraction?: boolean;
      silent?: boolean;
      icon?: string;
    };
    const title = o.title ?? "Desktop Feature Test";
    try {
      const n = new Notification(title, {
        body: o.body ?? "Notification body text",
        tag: o.tag,
        requireInteraction: o.requireInteraction === true,
        silent: o.silent === true,
        icon: o.icon,
      });
      const localId = ++notificationCounter;
      activeNotifications.set(localId, n);
      // Wire up the four spec events. We log to the same event log used by
      // the rest of the dashboard so the report picks them up automatically.
      n.addEventListener("show", () => {
        pushEvent({ type: "notification:show", localId, title: n.title });
      });
      n.addEventListener("click", () => {
        pushEvent({ type: "notification:click", localId, title: n.title });
      });
      n.addEventListener("close", () => {
        pushEvent({ type: "notification:close", localId, title: n.title });
        activeNotifications.delete(localId);
      });
      n.addEventListener("error", () => {
        pushEvent({ type: "notification:error", localId, title: n.title });
        activeNotifications.delete(localId);
      });
      return {
        ok: true,
        localId,
        notificationId: (n as unknown as { notificationId: number })
          .notificationId,
        title: n.title,
        permission: Notification.permission,
      };
    } catch (e) {
      return { ok: false, error: String(e) };
    }
  },
);

win.bind("notificationCloseLast", async () => {
  const keys = [...activeNotifications.keys()];
  if (keys.length === 0) return { ok: false, reason: "no active notification" };
  const id = keys[keys.length - 1];
  const n = activeNotifications.get(id)!;
  n.close();
  activeNotifications.delete(id);
  return { ok: true, localId: id };
});

win.bind("notificationCloseAll", async () => {
  for (const n of activeNotifications.values()) n.close();
  const count = activeNotifications.size;
  activeNotifications.clear();
  return { ok: true, closed: count };
});

// ── Secondary window (multi-window) ─────────────────────────────────────────

// Exercises issue #4 (HMR all-windows reload tracks every window) and
// the open_windows / closed_windows bookkeeping (#21).

let secondWin: Deno.BrowserWindow | null = null;

win.bind("secondWindowOpen", async () => {
  if (secondWin && !secondWin.isClosed()) {
    secondWin.focus();
    return { ok: true, reused: true, windowId: secondWin.windowId };
  }
  secondWin = new Deno.BrowserWindow({
    title: "Secondary Window",
    width: 600,
    height: 400,
    x: 200,
    y: 200,
  });
  secondWin.addEventListener("close", () => {
    pushEvent({ type: "secondwin:close", windowId: secondWin?.windowId });
  });
  // Point it at a tiny inline page so we have something to navigate.
  await new Promise((r) => setTimeout(r, 200));
  await secondWin.executeJs(
    "document.body.style = 'background:#222;color:#fff;font-family:sans-serif;padding:20px';" +
      "document.body.textContent = 'Secondary window — close me to test isClosed()'",
  );
  return { ok: true, reused: false, windowId: secondWin.windowId };
});

win.bind("secondWindowClose", async () => {
  if (!secondWin) return { ok: false, reason: "no second window" };
  secondWin.close();
  return { ok: true };
});

win.bind("secondWindowStatus", async () => {
  if (!secondWin) return { exists: false };
  return {
    exists: true,
    windowId: secondWin.windowId,
    isClosed: secondWin.isClosed(),
  };
});

// ── Window navigate / reload ────────────────────────────────────────────────

// Tests the BrowserWindow.navigate(url) and reload() ops. Reload uses
// `location.reload()` under the hood; navigate replaces the current
// document.

win.bind("windowReload", async () => {
  win.reload();
  return true;
});

let savedHomeUrl: string | null = null;
win.bind("windowNavigate", async (url: unknown) => {
  win.navigate(String(url));
  return true;
});
win.bind("windowSetHome", async (url: unknown) => {
  savedHomeUrl = String(url);
  return true;
});
win.bind("windowGoHome", async () => {
  if (!savedHomeUrl) return { ok: false, reason: "no home url stashed" };
  win.navigate(savedHomeUrl);
  return { ok: true };
});

// ── Event Listeners ────────────────────────────────────────────────────────

for (const type of ["keydown", "keyup"] as const) {
  win.addEventListener(type, (e) => {
    pushEvent({
      type,
      key: e.key,
      code: e.code,
      ctrl: e.ctrlKey,
      shift: e.shiftKey,
      alt: e.altKey,
      meta: e.metaKey,
      repeat: e.repeat,
    });
  });
}

for (const type of ["mousedown", "mouseup", "click", "dblclick"] as const) {
  win.addEventListener(type, (e) => {
    pushEvent({
      type,
      button: e.button,
      x: e.clientX,
      y: e.clientY,
    });
  });
}

win.addEventListener("mousemove", (e) => {
  mouseMoveCount++;
  if (mouseMoveCount % 20 === 0) {
    pushEvent({ type: "mousemove", x: e.clientX, y: e.clientY });
  }
});

for (const type of ["mouseenter", "mouseleave"] as const) {
  win.addEventListener(type, () => {
    pushEvent({ type });
  });
}

win.addEventListener("wheel", (e) => {
  pushEvent({
    type: "wheel",
    deltaX: e.deltaX,
    deltaY: e.deltaY,
    deltaMode: e.deltaMode,
  });
});

for (const type of ["focus", "blur"] as const) {
  win.addEventListener(type, () => {
    pushEvent({ type });
  });
}

win.addEventListener("resize", (e: CustomEvent) => {
  pushEvent({
    type: "resize",
    width: e.detail.width,
    height: e.detail.height,
  });
});

win.addEventListener("move", (e: CustomEvent) => {
  pushEvent({ type: "move", x: e.detail.x, y: e.detail.y });
});

win.addEventListener("menuclick", (e: CustomEvent) => {
  pushEvent({ type: "menuclick", id: e.detail.id });
});

win.addEventListener("contextmenuclick", (e: CustomEvent) => {
  pushEvent({ type: "contextmenuclick", id: e.detail.id });
});

// ── Application Menu ───────────────────────────────────────────────────────

win.setApplicationMenu([
  // On macOS the first submenu becomes the application menu (label
  // is replaced with the app name).  Put standard app roles here.
  {
    submenu: {
      label: "App",
      items: [
        { role: { role: "quit" } },
      ],
    },
  },
  {
    submenu: {
      label: "File",
      items: [
        {
          item: {
            label: "Test Action",
            id: "test-action",
            accelerator: "CmdOrCtrl+T",
            enabled: true,
          },
        },
      ],
    },
  },
  {
    submenu: {
      label: "Edit",
      items: [
        { role: { role: "copy" } },
        { role: { role: "paste" } },
        { role: { role: "cut" } },
      ],
    },
  },
  {
    submenu: {
      label: "Test",
      items: [
        {
          item: {
            label: "Action 1",
            id: "action-1",
            enabled: true,
          },
        },
        {
          item: {
            label: "Action 2",
            id: "action-2",
            enabled: true,
          },
        },
        { separator: null } as unknown as Deno.MenuItem,
        {
          item: {
            label: "Disabled Item",
            id: "disabled",
            enabled: false,
          },
        },
      ],
    },
  },
]);

// ── HTML Dashboard ─────────────────────────────────────────────────────────

const html = `<!DOCTYPE html>
<html>
<head>
<meta charset="utf-8">
<title>Desktop Feature Test</title>
<style>
  * { box-sizing: border-box; margin: 0; padding: 0; }
  body {
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
    background: #1a1a2e; color: #e0e0e0; padding: 16px;
    font-size: 13px; line-height: 1.5;
  }
  h1 { font-size: 20px; margin-bottom: 4px; color: #fff; }
  h2 { font-size: 14px; margin-bottom: 8px; color: #aaa; border-bottom: 1px solid #333; padding-bottom: 4px; }
  .header { display: flex; justify-content: space-between; align-items: center; margin-bottom: 12px; }
  .header-right { display: flex; gap: 8px; }
  button {
    background: #2d2d4a; border: 1px solid #444; color: #e0e0e0;
    padding: 4px 10px; border-radius: 4px; cursor: pointer; font-size: 12px;
  }
  button:hover { background: #3d3d5a; }
  .grid { display: grid; grid-template-columns: minmax(0, 1fr) minmax(0, 1fr); gap: 10px; }
  .card {
    background: #16213e; border: 1px solid #333; border-radius: 6px; padding: 10px;
  }
  .card.full { grid-column: 1 / -1; }
  table { width: 100%; border-collapse: collapse; font-size: 12px; }
  td, th { padding: 2px 6px; text-align: left; }
  .pass { color: #4caf50; } .pass::before { content: "PASS "; font-weight: bold; }
  .fail { color: #f44336; } .fail::before { content: "FAIL "; font-weight: bold; }
  .pending { color: #888; } .pending::before { content: "-- "; }
  .log { background: #0d1117; border-radius: 4px; padding: 6px; max-height: 120px; overflow-y: auto; font-family: monospace; font-size: 11px; white-space: pre-wrap; }
  .test-area {
    background: #0d1117; border: 2px dashed #444; border-radius: 6px;
    min-height: 60px; display: flex; align-items: center; justify-content: center;
    color: #666; cursor: crosshair; user-select: none; margin-bottom: 6px;
  }
  .count { display: inline-block; background: #333; border-radius: 10px; padding: 0 6px; font-size: 11px; margin-left: 6px; }
  .report {
    background: #0d1117; border-radius: 6px; padding: 12px; margin-top: 12px;
    font-family: monospace; font-size: 12px; white-space: pre-wrap;
  }
  .btn-row { display: flex; gap: 6px; margin-bottom: 6px; }
</style>
</head>
<body>
  <div class="header">
    <h1>Desktop Feature Test</h1>
    <div class="header-right">
      <button onclick="rerunAutoTests()">Re-run Auto Tests</button>
      <button onclick="generateReport()">Generate Report</button>
    </div>
  </div>

  <div class="grid">
    <!-- Window Properties (auto) -->
    <div class="card">
      <h2>Window Properties</h2>
      <div id="props-results" class="log">Loading...</div>
    </div>

    <!-- executeJs (auto) -->
    <div class="card">
      <h2>executeJs</h2>
      <div id="exec-results" class="log">Loading...</div>
    </div>

    <!-- Bindings (auto) -->
    <div class="card">
      <h2>Bindings Roundtrip</h2>
      <div id="bind-results" class="log">Running...</div>
    </div>

    <!-- App Menu (manual) -->
    <div class="card">
      <h2>App Menu <span class="count" id="menu-count">0</span></h2>
      <p style="color:#888;margin-bottom:4px">Click menu items: File > Test Action, Test > Action 1/2</p>
      <div id="menu-log" class="log">Waiting for menu clicks...</div>
    </div>

    <!-- Keyboard (manual) -->
    <div class="card">
      <h2>Keyboard <span class="count" id="key-count">0</span></h2>
      <p style="color:#888;margin-bottom:4px">Press any key</p>
      <div id="key-log" class="log">Waiting for key events...</div>
    </div>

    <!-- Mouse (manual) -->
    <div class="card">
      <h2>Mouse <span class="count" id="mouse-count">0</span></h2>
      <div class="test-area" id="mouse-area">Click / double-click here</div>
      <div id="mouse-log" class="log">Waiting for mouse events...</div>
    </div>

    <!-- Wheel (manual) -->
    <div class="card">
      <h2>Wheel <span class="count" id="wheel-count">0</span></h2>
      <p style="color:#888;margin-bottom:4px">Scroll anywhere in the window</p>
      <div id="wheel-log" class="log">Waiting for wheel events...</div>
    </div>

    <!-- Focus/Blur (manual) -->
    <div class="card">
      <h2>Focus / Blur <span class="count" id="focus-count">0</span></h2>
      <p style="color:#888;margin-bottom:4px">Click outside the window, then back</p>
      <div id="focus-log" class="log">Waiting for focus events...</div>
    </div>

    <!-- Resize/Move (manual) -->
    <div class="card">
      <h2>Resize / Move <span class="count" id="winev-count">0</span></h2>
      <p style="color:#888;margin-bottom:4px">Resize or drag the window</p>
      <div id="winev-log" class="log">Waiting for resize/move events...</div>
    </div>

    <!-- Dialogs (manual) -->
    <div class="card">
      <h2>Dialogs</h2>
      <div class="btn-row">
        <button onclick="testAlert()">Alert</button>
        <button onclick="testConfirm()">Confirm</button>
        <button onclick="testPrompt()">Prompt</button>
      </div>
      <div id="dialog-log" class="log">Click a button above</div>
    </div>

    <!-- Context Menu (manual) -->
    <div class="card">
      <h2>Context Menu <span class="count" id="ctx-count">0</span></h2>
      <div class="test-area" id="ctx-area" oncontextmenu="return false;">Right-click here</div>
      <div id="ctx-log" class="log">Waiting for context menu clicks...</div>
    </div>

    <!-- Dock (singleton at Deno.dock) -->
    <div class="card">
      <h2>Dock <span class="count" id="dock-count">0</span></h2>
      <div class="btn-row">
        <input id="dock-badge-input" placeholder="badge text" value="3" style="background:#0d1117;border:1px solid #444;color:#e0e0e0;padding:2px 6px;border-radius:4px;width:80px;font-size:12px"/>
        <button onclick="dockSetBadge()">Set Badge</button>
        <button onclick="dockClearBadge()">Clear Badge</button>
      </div>
      <div class="btn-row">
        <button onclick="dockBounce(false)">Bounce</button>
        <button onclick="dockBounce(true)">Bounce Critical</button>
        <button onclick="dockSetMenu()">Set Dock Menu</button>
      </div>
      <div class="btn-row">
        <button onclick="dockHide()">Hide Dock</button>
        <button onclick="dockShow()">Show Dock</button>
      </div>
      <div id="dock-log" class="log">macOS-only for most ops; other platforms degrade.</div>
    </div>

    <!-- Tray -->
    <div class="card">
      <h2>Tray <span class="count" id="tray-count">0</span></h2>
      <div class="btn-row">
        <button onclick="trayCreate()">Create Tray</button>
        <button onclick="trayDestroy()">Destroy Tray</button>
        <button onclick="traySetTooltip()">Set Tooltip</button>
      </div>
      <div class="btn-row">
        <button onclick="trayGetBounds()">Get Bounds</button>
        <button onclick="trayAttachPanel()">Attach Panel</button>
        <button onclick="trayPanelToggle()">Toggle Panel</button>
        <button onclick="trayDetachPanel()">Detach Panel</button>
      </div>
      <div id="tray-log" class="log">Create the tray, then Attach Panel and click the tray icon to toggle the popover.</div>
    </div>

    <!-- Notifications (Web Notifications API) -->
    <div class="card">
      <h2>Notifications <span class="count" id="notif-count">0</span></h2>
      <div class="btn-row">
        <button onclick="notifRequestPermission()">Request Permission</button>
        <button onclick="notifQueryPermission()">Query Permission</button>
        <span id="notif-perm" style="color:#888;align-self:center;margin-left:6px">perm: ?</span>
      </div>
      <div class="btn-row">
        <button onclick="notifShow()">Show</button>
        <button onclick="notifShowPersistent()">Show (requireInteraction)</button>
        <button onclick="notifShowSilent()">Show (silent)</button>
      </div>
      <div class="btn-row">
        <button onclick="notifShowTagged()">Show (tag=&quot;reuse&quot;)</button>
        <button onclick="notifCloseLast()">Close Last</button>
        <button onclick="notifCloseAll()">Close All</button>
      </div>
      <div id="notif-log" class="log">Click Request Permission first, then Show. Events: show/click/close/error.</div>
    </div>

    <!-- Secondary Window (multi-window) -->
    <div class="card">
      <h2>Secondary Window <span class="count" id="secondwin-count">0</span></h2>
      <div class="btn-row">
        <button onclick="secondWindowOpen()">Open Window</button>
        <button onclick="secondWindowClose()">Close (op)</button>
        <button onclick="secondWindowStatus()">Status</button>
      </div>
      <div id="secondwin-log" class="log">Tests multi-window tracking + isClosed().</div>
    </div>

    <!-- Window navigate / reload -->
    <div class="card">
      <h2>Navigate / Reload</h2>
      <div class="btn-row">
        <button onclick="windowReload()">reload()</button>
        <button onclick="windowNavigateBlank()">navigate(blank page)</button>
        <button onclick="windowGoHome()">navigate(home)</button>
      </div>
      <div id="navigate-log" class="log">reload() goes through location.reload(); navigate() replaces the document.</div>
    </div>

  </div>

  <div id="report" class="report" style="display:none"></div>

<script>
  // ── Binding roundtrip tests ──────────────────────────────────────────────
  const bindResults = [];

  async function runBindTests() {
    const tests = [
      { label: 'echo("hello")', fn: () => bindings.echo("hello"), expect: "hello" },
      { label: 'echo(null)', fn: () => bindings.echo(null), expect: null },
      { label: 'echo(true)', fn: () => bindings.echo(true), expect: true },
      { label: 'echo(42)', fn: () => bindings.echo(42), expect: 42 },
      { label: 'echo([1,"two"])', fn: () => bindings.echo([1, "two"]), expect: [1, "two"] },
      { label: 'echo({a:1})', fn: () => bindings.echo({ a: 1 }), expect: { a: 1 } },
      { label: 'add(2, 3)', fn: () => bindings.add(2, 3), expect: 5 },
    ];
    const el = document.getElementById("bind-results");
    let out = "";
    for (const t of tests) {
      try {
        const result = await t.fn();
        const pass = JSON.stringify(result) === JSON.stringify(t.expect);
        out += (pass ? "PASS" : "FAIL") + " " + t.label +
          " => " + JSON.stringify(result) + "\\n";
        bindResults.push({ label: t.label, pass, result });
      } catch (e) {
        out += "FAIL " + t.label + " threw: " + e + "\\n";
        bindResults.push({ label: t.label, pass: false, error: String(e) });
      }
    }
    el.textContent = out;
  }

  // ── Render auto-test results ─────────────────────────────────────────────

  async function renderAutoResults() {
    try {
      const results = await bindings.getTestResults();
      const propKeys = [
        "windowId", "resizable", "alwaysOnTop", "visibility",
        "setTitle", "getSize", "setSize", "getPosition", "setPosition",
        "focus", "openDevtools", "isClosed"
      ];
      const execKeys = [
        "executeJs:arithmetic", "executeJs:error",
        "executeJs:complex", "executeJs:string"
      ];

      let propsHtml = "";
      for (const k of propKeys) {
        const r = results[k];
        if (r) {
          const cls = r.pass ? "pass" : "fail";
          propsHtml += '<span class="' + cls + '">' + k + "</span>  " + r.detail + "\\n";
        } else {
          propsHtml += '<span class="pending">' + k + "</span>\\n";
        }
      }
      document.getElementById("props-results").innerHTML = propsHtml;

      let execHtml = "";
      for (const k of execKeys) {
        const r = results[k];
        if (r) {
          const cls = r.pass ? "pass" : "fail";
          execHtml += '<span class="' + cls + '">' + k.replace("executeJs:", "") + "</span>  " + r.detail + "\\n";
        } else {
          execHtml += '<span class="pending">' + k.replace("executeJs:", "") + "</span>\\n";
        }
      }
      document.getElementById("exec-results").innerHTML = execHtml;
    } catch (e) {
      document.getElementById("props-results").textContent = "Error: " + e;
    }
  }

  // ── Poll event log ───────────────────────────────────────────────────────

  const eventCounts = {};
  function updateEventCounts(events) {
    for (const e of events) {
      eventCounts[e.type] = (eventCounts[e.type] || 0) + 1;
    }
  }

  function updateCount(id, types) {
    const n = types.reduce((s, t) => s + (eventCounts[t] || 0), 0);
    document.getElementById(id).textContent = n;
  }

  function renderEvents(elId, events, types) {
    const filtered = events.filter(e => types.includes(e.type)).slice(-8);
    const el = document.getElementById(elId);
    if (filtered.length === 0) return;
    el.textContent = filtered.map(e => {
      const { type, ts, ...rest } = e;
      return type + " " + JSON.stringify(rest);
    }).join("\\n");
  }

  let lastSeq = 0;
  async function pollEvents() {
    try {
      const events = await bindings.getEventLog();
      if (events.length === 0) return;
      const newEvents = events.filter(e => e.seq > lastSeq);
      if (newEvents.length === 0) return;
      lastSeq = events[events.length - 1].seq;
      updateEventCounts(newEvents);

      renderEvents("key-log", events, ["keydown", "keyup"]);
      updateCount("key-count", ["keydown", "keyup"]);

      renderEvents("mouse-log", events, ["mousedown", "mouseup", "click", "dblclick", "mousemove", "mouseenter", "mouseleave"]);
      updateCount("mouse-count", ["mousedown", "mouseup", "click", "dblclick", "mousemove"]);

      renderEvents("wheel-log", events, ["wheel"]);
      updateCount("wheel-count", ["wheel"]);

      renderEvents("focus-log", events, ["focus", "blur"]);
      updateCount("focus-count", ["focus", "blur"]);

      renderEvents("winev-log", events, ["resize", "move"]);
      updateCount("winev-count", ["resize", "move"]);

      renderEvents("menu-log", events, ["menuclick"]);
      updateCount("menu-count", ["menuclick"]);

      renderEvents("ctx-log", events, ["contextmenuclick"]);
      updateCount("ctx-count", ["contextmenuclick"]);

      renderEvents("dock-log", events, ["dockmenuclick", "dockreopen"]);
      updateCount("dock-count", ["dockmenuclick", "dockreopen"]);

      renderEvents("tray-log", events, [
        "trayclick", "traydblclick", "traymenuclick"
      ]);
      updateCount("tray-count", [
        "trayclick", "traydblclick", "traymenuclick"
      ]);

      renderEvents("secondwin-log", events, ["secondwin:close"]);
      updateCount("secondwin-count", ["secondwin:close"]);

      renderEvents("notif-log", events, [
        "notification:show", "notification:click",
        "notification:close", "notification:error"
      ]);
      updateCount("notif-count", [
        "notification:show", "notification:click",
        "notification:close", "notification:error"
      ]);
    } catch (_) {}
  }

  // ── Context menu ─────────────────────────────────────────────────────────

  document.getElementById("ctx-area").addEventListener("contextmenu", async (e) => {
    e.preventDefault();
    await bindings.triggerContextMenu(e.clientX, e.clientY);
  });

  // ── Dialogs ──────────────────────────────────────────────────────────────

  async function testAlert() {
    const el = document.getElementById("dialog-log");
    el.textContent = "Showing alert...";
    await bindings.showAlert("Test alert message");
    el.textContent = "alert() completed (dismissed by user)";
  }

  async function testConfirm() {
    const el = document.getElementById("dialog-log");
    el.textContent = "Showing confirm...";
    const result = await bindings.showConfirm("Do you confirm?");
    el.textContent = "confirm() returned: " + JSON.stringify(result);
  }

  async function testPrompt() {
    const el = document.getElementById("dialog-log");
    el.textContent = "Showing prompt...";
    const result = await bindings.showPrompt("Enter something:", "default value");
    el.textContent = "prompt() returned: " + JSON.stringify(result);
  }

  // ── Dock ─────────────────────────────────────────────────────────────────

  async function dockSetBadge() {
    const text = document.getElementById("dock-badge-input").value || "";
    await bindings.dockSetBadge(text);
    document.getElementById("dock-log").textContent =
      "setBadge(" + JSON.stringify(text) + ") OK";
  }
  async function dockClearBadge() {
    await bindings.dockClearBadge();
    document.getElementById("dock-log").textContent = "setBadge('') OK";
  }
  async function dockBounce(critical) {
    await bindings.dockBounce(critical);
    document.getElementById("dock-log").textContent =
      "bounce(critical=" + critical + ") OK";
  }
  async function dockSetMenu() {
    await bindings.dockSetMenu();
    document.getElementById("dock-log").textContent =
      "setMenu OK — right-click the dock icon";
  }
  async function dockHide() {
    await bindings.dockHide();
    document.getElementById("dock-log").textContent = "setVisible(false) OK";
  }
  async function dockShow() {
    await bindings.dockShow();
    document.getElementById("dock-log").textContent = "setVisible(true) OK";
  }

  // ── Tray ─────────────────────────────────────────────────────────────────

  async function trayCreate() {
    const r = await bindings.trayCreate();
    document.getElementById("tray-log").textContent =
      "trayCreate => " + JSON.stringify(r);
  }
  async function trayDestroy() {
    const r = await bindings.trayDestroy();
    document.getElementById("tray-log").textContent =
      "trayDestroy => " + JSON.stringify(r);
  }
  async function traySetTooltip() {
    const text = "Tray @ " + new Date().toLocaleTimeString();
    const r = await bindings.traySetTooltip(text);
    document.getElementById("tray-log").textContent =
      "setTooltip(" + JSON.stringify(text) + ") => " + JSON.stringify(r);
  }
  async function trayGetBounds() {
    const r = await bindings.trayGetBounds();
    document.getElementById("tray-log").textContent =
      "getBounds => " + JSON.stringify(r);
  }
  async function trayAttachPanel() {
    const r = await bindings.trayAttachPanel();
    document.getElementById("tray-log").textContent =
      "attachPanel => " + JSON.stringify(r) +
      " — now click the tray icon to toggle it";
  }
  async function trayPanelToggle() {
    const r = await bindings.trayPanelToggle();
    document.getElementById("tray-log").textContent =
      "toggle => " + JSON.stringify(r);
  }
  async function trayDetachPanel() {
    const r = await bindings.trayDetachPanel();
    document.getElementById("tray-log").textContent =
      "detachPanel => " + JSON.stringify(r);
  }

  // ── Notifications ────────────────────────────────────────────────────────

  function setNotifPerm(p) {
    document.getElementById("notif-perm").textContent = "perm: " + p;
  }

  async function notifQueryPermission() {
    const p = await bindings.notificationPermission();
    // p = { cached, permissionsApi }. The permissionsApi field is what
    // we just read from wef; "cached" is what Notification.permission
    // returns synchronously (only updates after a successful query/request).
    setNotifPerm(p.permissionsApi);
    document.getElementById("notif-log").textContent =
      "Notification.permission (cached) => " + p.cached + "\\n" +
      "navigator.permissions.query => " + p.permissionsApi +
      (p.permissionsApi === "denied"
        ? "\\n[hint] macOS UN caches denied per CFBundleIdentifier." +
          " Under 'deno desktop' the bundle id is io.wef.cef (CEF backend)" +
          " or io.wef.webview (webview backend); fix at" +
          " System Settings > Notifications."
        : "");
  }
  async function notifRequestPermission() {
    document.getElementById("notif-log").textContent =
      "Requesting permission...";
    const r = await bindings.notificationRequestPermission();
    if (r.ok) setNotifPerm(r.permission);
    document.getElementById("notif-log").textContent =
      "requestPermission => " + JSON.stringify(r);
  }
  async function notifShow() {
    const r = await bindings.notificationShow({
      title: "Desktop Feature Test",
      body: "Hello from the Notification API @ " + new Date().toLocaleTimeString(),
    });
    if (r.ok && r.permission) setNotifPerm(r.permission);
    document.getElementById("notif-log").textContent =
      "show => " + JSON.stringify(r);
  }
  async function notifShowPersistent() {
    const r = await bindings.notificationShow({
      title: "Persistent Notification",
      body: "requireInteraction=true — won't auto-dismiss",
      requireInteraction: true,
    });
    if (r.ok && r.permission) setNotifPerm(r.permission);
    document.getElementById("notif-log").textContent =
      "show(persistent) => " + JSON.stringify(r);
  }
  async function notifShowSilent() {
    const r = await bindings.notificationShow({
      title: "Silent Notification",
      body: "silent=true — no sound",
      silent: true,
    });
    if (r.ok && r.permission) setNotifPerm(r.permission);
    document.getElementById("notif-log").textContent =
      "show(silent) => " + JSON.stringify(r);
  }
  async function notifShowTagged() {
    // Two notifications with the same tag — on platforms that honor it,
    // the second should replace the first instead of stacking.
    const r1 = await bindings.notificationShow({
      title: "Tagged #1",
      body: "tag='reuse' — should be replaced by the next one",
      tag: "reuse",
    });
    await new Promise(r => setTimeout(r, 600));
    const r2 = await bindings.notificationShow({
      title: "Tagged #2",
      body: "tag='reuse' — replaces #1 on platforms that honor tags",
      tag: "reuse",
    });
    if (r2.ok && r2.permission) setNotifPerm(r2.permission);
    document.getElementById("notif-log").textContent =
      "show(tag) #1 => " + JSON.stringify(r1) + "\\n" +
      "show(tag) #2 => " + JSON.stringify(r2);
  }
  async function notifCloseLast() {
    const r = await bindings.notificationCloseLast();
    document.getElementById("notif-log").textContent =
      "closeLast => " + JSON.stringify(r);
  }
  async function notifCloseAll() {
    const r = await bindings.notificationCloseAll();
    document.getElementById("notif-log").textContent =
      "closeAll => " + JSON.stringify(r);
  }

  // ── Secondary window ─────────────────────────────────────────────────────

  async function secondWindowOpen() {
    const r = await bindings.secondWindowOpen();
    document.getElementById("secondwin-log").textContent =
      "open => " + JSON.stringify(r);
  }
  async function secondWindowClose() {
    const r = await bindings.secondWindowClose();
    document.getElementById("secondwin-log").textContent =
      "close => " + JSON.stringify(r);
  }
  async function secondWindowStatus() {
    const r = await bindings.secondWindowStatus();
    document.getElementById("secondwin-log").textContent =
      "status => " + JSON.stringify(r);
  }

  // ── Window navigate / reload ─────────────────────────────────────────────

  // Stash the URL the page loaded from so navigate-home brings us back.
  bindings.windowSetHome(location.href);

  async function windowReload() {
    document.getElementById("navigate-log").textContent =
      "reload() — page about to reload…";
    await bindings.windowReload();
  }
  async function windowNavigateBlank() {
    document.getElementById("navigate-log").textContent =
      "navigate(data:…) — page about to leave…";
    await bindings.windowNavigate(
      "data:text/html,<body style='background:%23222;color:%23ccc;font-family:sans-serif;padding:20px'>" +
        "<h1>Blank page (navigate test)</h1>" +
        "<p>Use the WEF window's back gesture or reopen the dashboard.</p></body>",
    );
  }
  async function windowGoHome() {
    const r = await bindings.windowGoHome();
    document.getElementById("navigate-log").textContent =
      "navigate(home) => " + JSON.stringify(r);
  }

  // ── Re-run ───────────────────────────────────────────────────────────────

  async function rerunAutoTests() {
    document.getElementById("props-results").textContent = "Re-running...";
    document.getElementById("exec-results").textContent = "Re-running...";
    await bindings.triggerAutoTests();
    await renderAutoResults();
  }

  // ── Report ───────────────────────────────────────────────────────────────

  async function generateReport() {
    const results = await bindings.getTestResults();
    const el = document.getElementById("report");
    el.style.display = "block";

    const allTests = Object.entries(results);
    const passed = allTests.filter(([, r]) => r.pass);
    const failed = allTests.filter(([, r]) => !r.pass);

    const bindPassed = bindResults.filter(r => r.pass).length;
    const bindFailed = bindResults.filter(r => !r.pass).length;

    const observedEvents = Object.keys(eventCounts);

    let report = "";
    report += "=== DESKTOP FEATURE TEST REPORT ===\\n";
    report += "Generated: " + new Date().toISOString() + "\\n\\n";

    report += "--- Backend Auto Tests ---\\n";
    report += "Total: " + allTests.length + "  Passed: " + passed.length + "  Failed: " + failed.length + "\\n\\n";
    for (const [name, r] of allTests) {
      report += (r.pass ? "  PASS" : "  FAIL") + "  " + name + "  " + r.detail + "\\n";
    }

    report += "\\n--- Binding Roundtrip Tests ---\\n";
    report += "Total: " + bindResults.length + "  Passed: " + bindPassed + "  Failed: " + bindFailed + "\\n\\n";
    for (const r of bindResults) {
      report += (r.pass ? "  PASS" : "  FAIL") + "  " + r.label;
      if (!r.pass) {
        if (r.error) report += "  threw: " + r.error;
        else report += "  got: " + JSON.stringify(r.result);
      }
      report += "\\n";
    }

    report += "\\n--- Events Observed ---\\n";
    if (observedEvents.length === 0) {
      report += "  (none yet -- interact with the window)\\n";
    } else {
      for (const t of observedEvents.sort()) {
        report += "  " + t + ": " + eventCounts[t] + "\\n";
      }
    }

    const allEventTypes = [
      "keydown", "keyup", "mousedown", "mouseup", "click", "dblclick",
      "mousemove", "mouseenter", "mouseleave", "wheel",
      "focus", "blur", "resize", "move",
      "menuclick", "contextmenuclick",
      "dockmenuclick", "dockreopen",
      "trayclick", "traydblclick", "traymenuclick",
      "secondwin:close",
      "notification:show", "notification:click",
      "notification:close", "notification:error"
    ];
    const missing = allEventTypes.filter(t => !observedEvents.includes(t));
    if (missing.length > 0) {
      report += "\\n  Not yet observed: " + missing.join(", ") + "\\n";
    }

    report += "\\n--- Known Issues ---\\n";
    report += "  getSize/getPosition may return [0,0] (WEF cross-thread bug)\\n";

    const totalPassed = passed.length + bindPassed;
    const totalFailed = failed.length + bindFailed;
    const total = totalPassed + totalFailed;
    report += "\\n=== SUMMARY: " + totalPassed + "/" + total + " tests passed";
    if (totalFailed > 0) report += ", " + totalFailed + " failed";
    report += " ===\\n";

    el.textContent = report;
    el.scrollIntoView({ behavior: "smooth" });
  }

  // ── Init ─────────────────────────────────────────────────────────────────

  (async () => {
    // Wait for WEF binding injection to complete after navigation
    await new Promise(r => setTimeout(r, 500));
    await runBindTests();
    // Wait for async backend tests to finish
    await new Promise(r => setTimeout(r, 2500));
    await renderAutoResults();
    // Start polling
    setInterval(pollEvents, 1500);
  })();
</script>
</body>
</html>`;

// ── HTTP Server ────────────────────────────────────────────────────────────

Deno.serve((req: Request) => {
  const url = new URL(req.url);
  if (url.pathname === "/api/results") {
    return Response.json(Object.fromEntries(testResults));
  }
  if (url.pathname === "/api/events") {
    return Response.json(eventLog.slice(-100));
  }
  return new Response(html, {
    headers: { "content-type": "text/html" },
  });
});
