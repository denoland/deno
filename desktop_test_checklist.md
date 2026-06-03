# Desktop manual test checklist

Manual verification list for everything landed on `desktop-framework-hmr`. Group
results into ✅ pass / ❌ fail / ⏭ skip; copy the section heading into the bug
report if anything fails.

Many items reference the audit issue numbers from `desktop_hmr_issues.md` and
`desktop_other_issues.md` for context.

## Setup

```bash
# Build the dev binary once.
cargo build --bin deno

# Local wef checkout is patched in via Cargo.toml's [patch.crates-io].
# Verify it still resolves:
cargo metadata --format-version=1 --no-deps \
  | jq '.packages[] | select(.name == "just-wef") | .source'
# expected: null  (means it's the local path patch)

# Pre-build a wef backend so individual tests don't pay the download cost.
WEF_DEV_DIR=$(realpath ../wef) ./target/debug/deno desktop --help
```

For most tests below, the runner is:

```bash
WEF_DEV_DIR=$(realpath ../wef) ./target/debug/deno desktop \
  --hmr feature_test.ts
```

The expanded `feature_test.ts` exercises most of the in-process op surface.
Treat each card on its dashboard as one item.

---

## 1. Build & packaging (`deno desktop` compile path)

| #    | Test                | Steps                                                                | Expected                                                                                       |
| ---- | ------------------- | -------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------- |
| 1.1  | Basic compile       | `deno desktop tests/specs/desktop/.../main.ts`                       | Builds an output, prints `Bundle path/to/foo.app` (or .exe / dir)                              |
| 1.2  | macOS `.app`        | as 1.1 on macOS                                                      | `.app` bundle valid; double-click launches; bundle has `Contents/{MacOS,Resources,Info.plist}` |
| 1.3  | macOS `.dmg`        | `deno desktop --output Foo.dmg main.ts`                              | Mountable DMG with the `.app` + `/Applications` symlink                                        |
| 1.4  | Linux dir           | as 1.1 on Linux                                                      | App dir with `.sh` launcher (now `#!/bin/sh` — was `#!/bin/bash`)                              |
| 1.5  | Linux AppImage      | `deno desktop --output Foo.AppImage main.ts`                         | Single executable AppImage, runs                                                               |
| 1.6  | Windows dir         | as 1.1 on Windows                                                    | App dir with `.bat` launcher; double-click runs                                                |
| 1.7  | Cross-target        | `deno desktop --target aarch64-pc-windows-msvc main.ts` (from macOS) | Builds without panicking; expected error if missing prebuilt                                   |
| 1.8  | `--all-targets`     | `deno desktop --all-targets main.ts`                                 | All five targets build (slow)                                                                  |
| 1.9  | Icon `.png` (macOS) | `deno desktop --icon=icon.png main.ts`                               | Icon converted to `.icns` and shown in Finder                                                  |
| 1.10 | Icon set            | `deno.json` icon set with multiple sizes                             | macOS: combined `.icns`; Linux: largest size                                                   |

### Negative / hardening (cli/tools/desktop.rs)

| #    | Test                       | Steps                                                     | Expected                                                              |
| ---- | -------------------------- | --------------------------------------------------------- | --------------------------------------------------------------------- |
| 1.20 | `--output /`               | `deno desktop --output / main.ts`                         | Friendly error, no panic on `file_stem().unwrap()` (#14)              |
| 1.21 | Shell metachar in app name | `deno desktop --output 'foo$bar' main.ts`                 | Error: `invalid app name "foo$bar": must match [A-Za-z0-9 ._-]+` (#4) |
| 1.22 | `.bat` metachars (Windows) | `deno desktop --output 'a&b' main.ts`                     | Error: same `validate_launcher_name` family (#5)                      |
| 1.23 | DMG staging cleanup        | `deno desktop --output Foo.dmg main.ts` mid-Ctrl-C, rerun | No leftover `.dmg-staging-*` dir (now via `tempfile::tempdir_in`, #6) |
| 1.24 | Spaces in app name         | `deno desktop --output 'My App' main.ts`                  | Allowed; produces `My App.app`                                        |
| 1.25 | Temp entrypoint cleanup    | `deno desktop .` (framework dir), Ctrl-C mid-build        | `.deno_desktop_entry-*.ts` removed (was leaked on Ctrl-C, #9)         |

### WEF backend download (TOFU surface)

| #    | Test                        | Steps                                                                     | Expected                                                                                         |
| ---- | --------------------------- | ------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------ |
| 1.30 | Concurrent `desktop` builds | Two terminals run the same compile at once with empty WEF cache           | Both succeed; no half-extracted dir; second one cheaply reuses first's cache (atomic-rename, #7) |
| 1.31 | Checksum mismatch error     | Hand-corrupt a downloaded WEF archive in cache, rerun                     | Bail message includes the post-redirect download URL (#15)                                       |
| 1.32 | User-Agent header           | Tcpdump / proxy a WEF download                                            | `User-Agent: deno-desktop/{ver} (+https://deno.com)` is sent (#30)                               |
| 1.33 | Tar zip-slip                | Hand-craft a tar.gz with `../etc/passwd` entry, re-point WEF mirror to it | `refusing tar entry that would unpack outside dest` — `unpack_in` blocks it (#2)                 |
| 1.34 | Tar symlink escape          | Tar with `foo -> ../../etc` then `foo/passwd`                             | Same — `unpack_in`'s symlink containment kicks in (#2)                                           |
| 1.35 | Zip zip-slip                | Hand-craft a zip with absolute path / `..`                                | `refusing zip entry with unsafe path` (zip hardening, item #1)                                   |
| 1.36 | Zip symlink                 | Zip with a symlink entry                                                  | `refusing symlink entry in wef archive`                                                          |
| 1.37 | Zip perm laundering         | Zip with file mode 04755 (setuid)                                         | File extracted with mode 0755, not 04755                                                         |

---

## 2. Framework auto-detection (`deno desktop .`)

For each framework, run `deno desktop --hmr .` from a sample project directory
and verify the dashboard loads.

| #    | Framework                       | Sample project                                              | Expected                                                                                              |
| ---- | ------------------------------- | ----------------------------------------------------------- | ----------------------------------------------------------------------------------------------------- |
| 2.1  | Next.js                         | `next.config.js` + npm build                                | Detects "Next.js"; builds via `task build`; runs `nextStart`; dashboard reachable on the desktop port |
| 2.2  | Astro                           | `astro.config.mjs` after `astro build`                      | Detects "Astro"; serves `dist/server/entry.mjs`                                                       |
| 2.3  | Fresh 2                         | `_fresh/server.js` present                                  | Detects "Fresh"; uses `Deno.serve(mod.default.fetch)`                                                 |
| 2.4  | Fresh 1                         | `fresh.gen.ts` only                                         | Detects "Fresh" (Fresh 1); imports `./main.ts`                                                        |
| 2.5  | SvelteKit + deno-deploy adapter | `.deno-deploy/server.ts` exists                             | Detects "SvelteKit"                                                                                   |
| 2.6  | SvelteKit + nitro deno preset   | `.output/server/index.{ts,mjs}`                             | Same                                                                                                  |
| 2.7  | Nuxt                            | `nuxt.config.ts` + `.output/server/index.mjs`               | Detects "Nuxt"                                                                                        |
| 2.8  | SolidStart                      | `package.json` has `@solidjs/start`                         | Detects "SolidStart"                                                                                  |
| 2.9  | TanStack Start                  | `package.json` has `@tanstack/react-start` or `solid-start` | Detects "TanStack Start"                                                                              |
| 2.10 | Remix                           | `package.json` has `@remix-run/react`                       | Detects "Remix"                                                                                       |
| 2.11 | Vite SSR                        | `vite.config.js` + `server.{js,ts,mjs}`                     | Detects "Vite"                                                                                        |
| 2.12 | No detection                    | Empty dir                                                   | Bails with "Could not detect a supported framework…"                                                  |
| 2.13 | Detection consistency           | Run twice in same dir                                       | Same framework reported (audit #14 — single-detection refactor)                                       |

### Sanity checks

- `cargo test -p deno --lib framework::` — all 44 unit tests pass.

---

## 3. Window API (`Deno.BrowserWindow`)

Run `feature_test.ts` and exercise each card.

| #    | Card               | Steps                                       | Expected                                                                 |
| ---- | ------------------ | ------------------------------------------- | ------------------------------------------------------------------------ |
| 3.1  | Window Properties  | (auto on launch)                            | All rows green (PASS) including new `isClosed` row (live window → false) |
| 3.2  | executeJs          | (auto on launch)                            | All four sub-tests PASS                                                  |
| 3.3  | Bindings Roundtrip | (auto on launch)                            | All seven bindings roundtrip                                             |
| 3.4  | Re-run Auto Tests  | Click button                                | Same results, no leaked state                                            |
| 3.5  | App Menu           | Click File → Test Action, Test → Action 1/2 | Each click bumps the counter; menuclick events with the right `id`       |
| 3.6  | Keyboard           | Type any key                                | keydown + keyup events with key/code/modifiers                           |
| 3.7  | Mouse              | Click / dbl-click in the test area          | mousedown/up/click/dblclick log entries                                  |
| 3.8  | Wheel              | Scroll                                      | wheel events with deltaX/Y/Mode                                          |
| 3.9  | Focus / Blur       | Click outside, then back                    | focus + blur events                                                      |
| 3.10 | Resize / Move      | Drag corner / drag titlebar                 | resize + move events                                                     |
| 3.11 | Close Event        | Click red close button                      | Counter increments, window stays open (preventDefault)                   |

### New cards (this branch's expansion)

| #    | Card                           | Steps                                                                     | Expected                                                                           |
| ---- | ------------------------------ | ------------------------------------------------------------------------- | ---------------------------------------------------------------------------------- |
| 3.20 | Secondary Window — open        | Click "Open Window"                                                       | Second window appears; status shows `windowId`, `isClosed:false`                   |
| 3.21 | Secondary Window — close (op)  | Click "Close (op)"                                                        | Window closes; `secondwin:close` event fires; status now `isClosed:true`           |
| 3.22 | Secondary Window — close via X | Open, then click red X on second                                          | Same `close` event delivered to the second window                                  |
| 3.23 | DevTools singleton             | Click open-devtools (or trigger via Re-run Auto Tests, which calls twice) | Exactly **one** DevTools window opens (#21). Repeat → second call focuses existing |
| 3.24 | Window Reload                  | Click "reload()"                                                          | Page reloads, dashboard re-initializes                                             |
| 3.25 | Window Navigate                | Click "navigate(blank page)" then "navigate(home)"                        | First navigates to inline data: page; second restores dashboard                    |

---

## 4. Dialogs (sync API, the deadlock fix)

The headline fix on this branch: web-spec sync `alert/confirm/prompt` that don't
deadlock the runtime.

| #   | Test                                         | Steps                                                             | Expected                                                                                                                                                                                                                      |
| --- | -------------------------------------------- | ----------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 4.1 | `alert`                                      | Dashboard → Dialogs → Alert                                       | OS native alert appears; dismissing returns to JS                                                                                                                                                                             |
| 4.2 | `confirm` accepted                           | Confirm button → click OK                                         | Returns `true`                                                                                                                                                                                                                |
| 4.3 | `confirm` cancelled                          | Confirm button → click Cancel                                     | Returns `false`                                                                                                                                                                                                               |
| 4.4 | `prompt` accepted                            | Prompt → type, OK                                                 | Returns the typed string                                                                                                                                                                                                      |
| 4.5 | `prompt` cancelled                           | Prompt → Cancel                                                   | Returns `null`                                                                                                                                                                                                                |
| 4.6 | **Cross-window responsiveness** during modal | Open second window first (3.20), then trigger `confirm` from main | Second window stays painted/responsive while modal is up — its mouse events queue but it doesn't deadlock the process. Specifically: dragging the secondary window or hovering should not spinning-rainbow/etc the entire app |
| 4.7 | **No process deadlock**                      | Run `alert` from inside a forked Next.js worker scenario          | Process completes the dialog and proceeds (would hang pre-fix on macOS)                                                                                                                                                       |
| 4.8 | Stress: 10 dialogs in a row                  | Click Alert 10 times rapidly                                      | Each shows + dismisses in sequence; no leaked threads / state                                                                                                                                                                 |

---

## 5. Dock (`Deno.dock`, macOS-prominent; degrades elsewhere)

| #   | Test                 | Steps                                        | Expected                                                                       |
| --- | -------------------- | -------------------------------------------- | ------------------------------------------------------------------------------ |
| 5.1 | Set badge            | Type "3", click "Set Badge"                  | Badge "3" on dock icon (macOS) / focused taskbar (Linux/Win)                   |
| 5.2 | Clear badge          | Click "Clear Badge"                          | Badge gone                                                                     |
| 5.3 | Bounce informational | Click "Bounce"                               | Dock icon bounces once (macOS) / urgency hint (Linux X11) / flash 3x (Windows) |
| 5.4 | Bounce critical      | Click "Bounce Critical"                      | Continuous bounce until dock icon is focused                                   |
| 5.5 | Set dock menu        | Click "Set Dock Menu", right-click dock icon | Custom menu items appear; clicking one fires `dockmenuclick` with `id`         |
| 5.6 | Hide dock icon       | Click "Hide Dock"                            | macOS: app drops from dock                                                     |
| 5.7 | Show dock icon       | Click "Show Dock"                            | macOS: app rejoins dock                                                        |
| 5.8 | Dock reopen          | Hide all windows, click dock icon (macOS)    | `dockreopen` event in event log with `hasVisibleWindows: false`                |

---

## 6. Tray (`Deno.Tray`)

| #    | Test              | Steps                                 | Expected                                                                                     |
| ---- | ----------------- | ------------------------------------- | -------------------------------------------------------------------------------------------- |
| 6.1  | Create tray       | Click "Create Tray"                   | Visible tray icon (black dot) appears; binding returns `{ ok: true, trayId, reused: false }` |
| 6.2  | Tray click        | Left-click tray icon                  | `trayclick` event                                                                            |
| 6.3  | Tray double-click | Double-click tray icon                | `traydblclick` event                                                                         |
| 6.4  | Tray menu         | Right-click tray, pick item           | `traymenuclick` event with the item `id`                                                     |
| 6.5  | Set tooltip       | Click "Set Tooltip", hover icon       | Tooltip text shows current time                                                              |
| 6.6  | Destroy tray      | Click "Destroy Tray"                  | Icon disappears; recreate via 6.1 succeeds                                                   |
| 6.7  | `Symbol.dispose`  | In an `await using` block in a script | Tray destroyed when block exits                                                              |
| 6.8  | Get bounds        | Click "Get Bounds"                    | `{ ok: true, bounds: {x,y,width,height} }` (or `bounds: null` on Linux)                      |
| 6.9  | Attach panel      | Click "Attach Panel"                  | Binding returns `{ ok: true, windowId }`; no window steals focus yet                         |
| 6.10 | Tray-toggle panel | Click the tray icon                   | Frameless popover appears anchored under the icon; click tray again → hides                  |
| 6.11 | Blur-dismiss      | Open panel, click another app/window  | Panel hides on blur                                                                          |
| 6.12 | Button toggle     | Click "Toggle Panel"                  | Panel shows/hides; binding returns `{ visible }`                                             |
| 6.13 | Detach panel      | Click "Detach Panel"                  | Panel window closes; tray click no longer toggles it                                         |

---

## 7. HMR (`--hmr` only — verify `--inspect` alone does NOT enable it)

Run `deno desktop --hmr feature_test.ts` and edit the file in another editor.

| #    | Test                    | Steps                                                              | Expected                                                                                                                      |
| ---- | ----------------------- | ------------------------------------------------------------------ | ----------------------------------------------------------------------------------------------------------------------------- |
| 7.1  | Edit constant string    | Change "Desktop Feature Test" to "Modified" and save               | Live page shows new title without losing renderer state                                                                       |
| 7.2  | Top-level export change | Add `export const x = 1` and save                                  | V8 returns `BlockedByTopLevelEsModuleChange`; runtime falls back to `location.reload()` (#2). Page reloads, edit takes effect |
| 7.3  | Save-then-format burst  | Save, then format-on-save fires another save within 50ms           | One coalesced reload (debounce, #10) — not two flickers                                                                       |
| 7.4  | Touch metadata only     | `chmod +w feature_test.ts` (mtime changes, content same)           | No reload (`Modify::Metadata` ignored, #11)                                                                                   |
| 7.5  | Delete tracked file     | `rm feature_test.ts && touch feature_test.ts`                      | Reload triggered, log says "removed" → reload (#13)                                                                           |
| 7.6  | New file                | `touch new.ts` (not imported)                                      | No-op silently — not tracked                                                                                                  |
| 7.7  | All windows reload      | Open Secondary Window (3.20), edit feature_test.ts                 | **Both** windows reload (#4) — pre-fix only main reloaded                                                                     |
| 7.8  | `--inspect` alone       | `deno desktop --inspect feature_test.ts` (no `--hmr`)              | DevTools mux runs; **no** file watcher started; no DENO_DESKTOP_HMR env var (#3)                                              |
| 7.9  | Dropped inspector       | Kill DevTools mid-edit, save file                                  | No panic in HMR runner — `wait_for_response` returns None instead of unwrapping (#7)                                          |
| 7.10 | Malformed CDP payload   | (hard to provoke; covered by `serde_json::from_str` no-unwrap, #8) | Runtime survives, debug log entry                                                                                             |
| 7.11 | Bad transpile           | Save file with syntax error                                        | Warn logged; no reload; runtime survives                                                                                      |
| 7.12 | Modules: .mjs/.cjs      | Edit a `.mjs` file imported by main                                | Watcher picks it up (extension allowlist now includes mjs/cjs)                                                                |
| 7.13 | hmr CustomEvent         | `addEventListener("hmr", ...)` in user code                        | Event fires with `detail.path` after each successful replace; works in **any** isolate (no hardcoded contextId, #5)           |

---

## 8. Inspector / DevTools mux

| #    | Test                        | Steps                                                          | Expected                                                                              |
| ---- | --------------------------- | -------------------------------------------------------------- | ------------------------------------------------------------------------------------- |
| 8.1  | `--inspect`                 | `deno desktop --inspect=127.0.0.1:9222 feature_test.ts`        | Logs `Inspector: DevTools on ws://127.0.0.1:9222`; chrome://inspect shows the target  |
| 8.2  | `--inspect-brk`             | Add `--inspect-brk`                                            | Page navigation waits until DevTools attaches; first JS line breaks                   |
| 8.3  | `--inspect-wait`            | Add `--inspect-wait`                                           | Page navigation waits until DevTools attaches; JS proceeds normally after attach      |
| 8.4  | DevTools mux works          | Open `chrome://inspect`, click "inspect" on the desktop target | Unified DevTools loads; shows both renderer + Deno targets                            |
| 8.5  | Renderer console            | Use Console tab                                                | `console.log` calls from in-page JS appear                                            |
| 8.6  | Deno console                | Switch to "deno-desktop" target                                | `console.log` from feature_test.ts appears                                            |
| 8.7  | `Deno.openDevtools` from JS | Call `win.openDevtools()` while mux is up                      | Browser tab opens to the unified frontend (uses `DENO_DESKTOP_MUX_WS` env var)        |
| 8.8  | Bad internal port env       | Manually set `DENO_DESKTOP_INSPECT_INTERNAL_PORT=garbage`      | Bails with clear "is not a valid SocketAddr" (#18 — was silently disabling inspector) |
| 8.9  | Mux accept loop resilience  | Provoke EMFILE (many open files)                               | Loop logs at attempts 1, 2, 4, 8, 16, … (throttled, #20) — not 5Hz spam               |
| 8.10 | Bad ws upstream port        | Modify a `/json/list` to omit `port` field                     | Bails with "ws url missing port: …" (#19 — was falling back to port 80)               |

---

## 9. Auto-update (privileged self-update)

These exercise the `Deno.desktop.applyPatch` op and the `apply_pending_update`
startup logic.

| #   | Test                                   | Steps                                                              | Expected                                                                                                   |
| --- | -------------------------------------- | ------------------------------------------------------------------ | ---------------------------------------------------------------------------------------------------------- |
| 9.1 | Apply happy path                       | Call `applyPatch(patchBytes, sha)` from JS with a valid bspatch    | Returns OK; `<dylib>.update` written; relaunch swaps in the new dylib                                      |
| 9.2 | Wrong SHA                              | Same with wrong sha string                                         | Throws `patch SHA-256 mismatch`                                                                            |
| 9.3 | Patched bytes don't look like a binary | Craft patch that produces non-ELF/Mach-O/PE                        | Throws `patched dylib does not look like a native binary`                                                  |
| 9.4 | Sentinel write                         | After 9.1's relaunch, call `confirmUpdate()` from JS               | `<dylib>.update-ok` sentinel written; next launch cleans up `.backup`                                      |
| 9.5 | Boot crash → rollback                  | Apply a patch that produces a binary that crashes during boot      | On next launch: no sentinel ⇒ rollback (rename `.backup` → dylib); `update_rolled_back: true` flows to JS  |
| 9.6 | Rename failure (EXDEV)                 | Mount cache dir on a different filesystem from dylib, apply update | Falls back to `copy → tmp → rename` instead of losing the update (#16)                                     |
| 9.7 | Rename + copy both fail                | Make dylib path read-only, apply update                            | `.update` is preserved for next-launch retry; `.backup` cleaned up so we don't trigger a spurious rollback |
| 9.8 | applyPatch UI freeze                   | Apply a 100MB patch                                                | App is unresponsive for a few seconds while bspatch runs (known, accepted)                                 |

---

## 10. Process lifecycle / panic / signals

| #     | Test                            | Steps                                                               | Expected                                                                                                               |
| ----- | ------------------------------- | ------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------- |
| 10.1  | Ctrl-C cleanup                  | Run `deno desktop --hmr foo.ts`, Ctrl-C                             | WEF subprocess + CEF renderers all gone (`kill_on_drop`, #24) — verify `pgrep wef` and `pgrep -f denort_desktop` empty |
| 10.2  | Parent panic cleanup            | Kill the parent with SIGKILL                                        | Same: subprocess dies on Child drop                                                                                    |
| 10.3  | Ctrl-C while WEF window open    | Window closes; no orphaned processes                                |                                                                                                                        |
| 10.4  | Forked Next.js worker           | Start Next.js dev under `deno desktop`, watch process tree          | Workers run headless (no extra windows); `is_worker` detects them via argv shape + env var combo (#17)                 |
| 10.5  | NODE_CHANNEL_FD leak from shell | `NODE_CHANNEL_FD=99 deno desktop main.ts`                           | Window still appears; the env-only check no longer false-positives (#17)                                               |
| 10.6  | Worker stderr                   | Force the parent to `process.exit()` mid-worker                     | No `/tmp/deno_desktop_worker.log` written (dropped, #8) — the eprintln is best-effort                                  |
| 10.7  | navigate poll cleanup           | Open desktop, immediately close window                              | No "Server not ready" warnings post-shutdown (navigate JoinHandle aborted, #22)                                        |
| 10.8  | Panic hook reports              | Force a panic with `error_reporting_url` set to `file:///tmp/r.log` | Report appended to file; process exits                                                                                 |
| 10.9  | Panic hook with bad URL         | Set `error_reporting_url` to `garbage` or empty                     | Report dropped; warn logged; **does not** write to a local file (#10)                                                  |
| 10.10 | Panic hook with `http://`       | Set to `http://example.com/r`                                       | Refused (https-only); warn logged                                                                                      |
| 10.11 | NAPI symbol promotion           | Run a desktop app that loads a NAPI addon (next-swc)                | Addon loads; if `dlopen(self, RTLD_NOLOAD\|RTLD_GLOBAL)` returns null, debug log entry (#12)                           |

---

## 11. CWD & env vars (thread-safety)

| #    | Test                              | Steps                                                                          | Expected                                                                                |
| ---- | --------------------------------- | ------------------------------------------------------------------------------ | --------------------------------------------------------------------------------------- |
| 11.1 | DENO_SERVE_ADDRESS visible to JS  | In the user script, `Deno.env.get("DENO_SERVE_ADDRESS")`                       | Returns `tcp:127.0.0.1:NNNN` matching the desktop port (set before runtime starts, #28) |
| 11.2 | CWD inside self-extracting bundle | `console.log(Deno.cwd())` in framework dev                                     | Shows the VFS extraction dir (chdir'd before runtime, #29)                              |
| 11.3 | No glibc setenv UB                | Run on Linux x86_64 with `RUST_LOG=trace` and a tsan-built binary if available | No data race reports on setenv path                                                     |

---

## 12. Lint / format / unit tests (CI gate)

```bash
./tools/format.js                       # must pass clean
cargo test -p deno --lib framework::    # 44 unit tests
cargo test -p denort                    # denort unit tests
cargo check -p deno -p denort -p denort_desktop -p deno_runtime  # quick build
```

`./tools/lint.js` is **known to fail** today on a backlog of pre-existing clippy
errors elsewhere in `cli/` (see `desktop_other_issues.md`'s follow-ups). Those
errors aren't from this branch's work; they're a separate cleanup pass.

---

## 13. wef (`../wef`) build sanity

Quick check that the wef-side commit (`cd25b77` — sync dialog ABI) still builds
for both backends:

```bash
cd ../wef
make webview && make cef
```

Both should compile clean (1 pre-existing macOS deprecation warning per backend
on `popUpStatusItemMenu:`).

---

## What this branch did NOT change (smoke check, not regression-prone)

These should still work the way they did at the merge-from-main point;
spot-check that nothing regressed.

- `deno run` non-desktop scripts.
- `deno compile` (non-desktop) basic + self-extracting variants.
- `deno repl`, `deno fmt`, `deno lint`.
- WPT runner: `./tests/wpt/wpt.ts`.

If any of the items in §1–§13 don't behave as expected, capture:

1. Exact command + env vars.
2. Platform + arch (`uname -a` / `ver`).
3. WEF backend variant (`webview` vs `cef`).
4. Stderr from both the parent and the WEF subprocess.
5. The relevant `desktop_*_issues.md` row number, if applicable.
