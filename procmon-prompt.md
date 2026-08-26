# Build a process-monitor demo app with `deno desktop`

Create a small, tidy desktop app that lives in the macOS menu bar / system tray
and shows live system process info. This is a **demo** — the priority is that
the code is clean and easy to read at a glance. Split it into a few focused
files rather than cramming everything together, but don't over-engineer it: a
handful of small modules with clear responsibilities is the target.

Run it with:

```
./target/debug/deno desktop -A main.ts
```

(use `--hmr` while iterating).

## Suggested layout

Keep it overviewable — something like:

```
procmon/
  main.ts          # entry: wires up tray, panel, menu, polling loop
  processes.ts     # spawn `ps`, parse + sort process list
  tray.ts          # tray icon + context menu construction
  ui.html          # the panel UI (or a small ui.ts that builds the HTML)
  icon.ts          # embedded PNG icon bytes
```

Adjust as makes sense — the point is separation of concerns, not this exact
tree.

## Requirements

1. **Tray icon** — `const tray = new Deno.Tray()`, set a PNG icon via
   `tray.setIcon(uint8Array)` (embed a small PNG as base64 + `atob`), and
   `tray.setTooltip("Process Monitor")`.

2. **Tray popover panel** — use `tray.attachPanel({ url, width, height })` to
   attach a frameless, non-activating popover that toggles when the tray icon is
   clicked. The panel renders a live, sorted table of the top processes (PID,
   name, %CPU, memory).
   - ⚠️ `attachPanel()` already toggles the panel on tray click — do **not**
     also call `panel.toggle()` in a `"click"` listener or it flashes open then
     shut.

3. **Right-click menu** — give the tray a context menu via `tray.setMenu([...])`
   using the `Deno.MenuItem` shape (`{ item: { label, id, enabled } }`,
   `"separator"`, `{ submenu: { label, items } }`, `{ role: { role } }`).
   Include at least: a "Refresh now" item, a "Sort by" submenu (CPU / Memory /
   Name), and a "Quit" item. Handle clicks via the tray's `"menuclick"` event
   (`e.detail.id`); on "Quit" call `tray.destroy()` and exit.

4. **Data source** — gather process info with
   `new Deno.Command("ps", { args: ["-axo", "pid,pcpu,pmem,comm"] })` (or
   equivalent per platform), parse the output, and poll on a `setInterval` (e.g.
   every 2s). Push fresh data into the panel either via
   `panel.window.executeJs(...)` or by exposing a binding the page calls — e.g.
   `panel.window.bind("getProcesses", async () => processList)` and have the
   page poll it.

5. **Keep the process alive** — the event loop must not exit; keep a long-lived
   timer running.

## Notes

- The desktop type definitions are in `cli/tsc/dts/lib.deno.desktop.d.ts` —
  consult `Deno.Tray`, `Deno.BrowserWindow`, `Deno.MenuItem`, `TrayPanel`, and
  `TrayPanelOptions` for exact signatures.
- `BrowserWindow.bind` handlers must be `async` and return JSON-serializable
  values (`Deno.BrowserWindowReturn`).
- No third-party dependencies. Get the tray icon, popover, and menu working
  end-to-end first, then keep the code tidy and readable.
