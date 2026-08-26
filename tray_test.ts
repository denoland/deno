// Minimal repro: deno desktop tray click event.
//
// Run:  deno desktop -A tray-click-repro.ts
//
// Expected: clicking the menu-bar (tray) icon prints "[repro] tray CLICK" and
//           shows the window.
// Actual:   the icon appears, but clicking it does nothing (no "click" event,
//           window never shows).
//
// No external deps — the icon is an embedded 22x22 PNG.

const ICON_B64 =
  "iVBORw0KGgoAAAANSUhEUgAAABYAAAAWCAYAAADEtGw7AAAAIklEQVR42mPYEqz1nxaYYdTgUYNHDR41eNTgUYNHDaYMAwArNSL9hneSNgAAAABJRU5ErkJggg==";
const iconBytes = Uint8Array.from(atob(ICON_B64), (c) => c.charCodeAt(0));

const tray = new Deno.Tray();
console.error("[repro] trayId =", tray.trayId);
tray.setIcon(iconBytes);
tray.setTooltip("tray repro");

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

const panel = tray.attachPanel({
  url: PANEL_URL,
  width: 280,
  height: 160,
});

// Note: attachPanel() already toggles the panel on tray click, so we must
// NOT call panel.toggle() here or the panel toggles twice (flashes up + away).
tray.addEventListener("click", () => {
  console.error("[repro] tray CLICK fired");
});
tray.addEventListener("dblclick", () => console.error("[repro] tray DBLCLICK"));
tray.addEventListener(
  "menuclick",
  (e) => console.error("[repro] tray MENUCLICK", (e as CustomEvent).detail),
);

setInterval(() => {}, 1 << 30);
console.error("[repro] ready — click the tray icon (no menu set)");
