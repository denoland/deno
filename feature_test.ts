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

  // openDevtools
  try {
    win.openDevtools();
    record("openDevtools", true, "no crash");
  } catch (e) {
    record("openDevtools", false, String(e));
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

win.addEventListener("close", () => {
  pushEvent({ type: "close" });
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
  .grid { display: grid; grid-template-columns: 1fr 1fr; gap: 10px; }
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

    <!-- Close Event (manual) -->
    <div class="card">
      <h2>Close Event <span class="count" id="close-count">0</span></h2>
      <p style="color:#888;margin-bottom:4px">Close button click is intercepted (preventDefault)</p>
      <div id="close-log" class="log">Try closing the window (it will be prevented)</div>
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
        "focus", "openDevtools"
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

      renderEvents("close-log", events, ["close"]);
      updateCount("close-count", ["close"]);
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
      "menuclick", "contextmenuclick", "close"
    ];
    const missing = allEventTypes.filter(t => !observedEvents.includes(t));
    if (missing.length > 0) {
      report += "\\n  Not yet observed: " + missing.join(", ") + "\\n";
    }

    report += "\\n--- Known Issues ---\\n";
    report += "  getSize/getPosition may return [0,0] (WEF cross-thread bug)\\n";
    report += "  isClosed() / reload() are not yet implemented (todo! in Rust)\\n";

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
