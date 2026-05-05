# Desktop (non-HMR): Bugs and Issues

Audit of the `deno desktop` feature on branch `desktop-framework-hmr`, covering
correctness/security/lifecycle issues _outside_ the HMR runner (those live in
`desktop_hmr_issues.md` and were addressed in commit `e768fc3a70`).

All originally-tracked items have been addressed across commits on this branch.
Item #26 (`strip_cef_bloat` macOS-only) was a "not a bug" annotation, kept here
for reference.

Files of interest:

- `cli/tools/desktop.rs` — `deno desktop` subcommand: compile, package macOS
  .app / Linux dir / Windows dir, DMG, AppImage, icon handling, WEF backend
  resolver/downloader, archive extraction.
- `cli/tools/desktop_devtools.rs` — Unified DevTools CDP multiplexer fronting
  both Deno and CEF inspector ports.
- `cli/rt_desktop/lib.rs` — `denort_desktop` cdylib: WEF backend entry point,
  panic hook, auto-update apply/rollback, headless worker path
  (child_process.fork), event loop wiring.
- `runtime/ops/desktop.rs` — Op surface (BrowserWindow, Dock, Tray,
  alert/confirm/prompt, bind callbacks).
- `cli/rt/desktop.rs` — JS surface: BrowserWindow prototype, EventTarget
  plumbing, binding wrappers.

---

## Open follow-ups (not bugs in this audit's scope)

- **Workspace clippy debt.** The originally-flagged 8 errors in
  `runtime/ops/desktop.rs` (#31) are gone, but fixing them unmasked a backlog of
  pre-existing lint failures elsewhere in `cli/` (eprintln in `denort`,
  collapsible-ifs, large-future warnings on `compile_desktop`,
  `Path::canonicalize` in build scripts, etc.). `tools/lint.js` still fails on
  those. Out of scope for this audit; needs its own dedicated cleanup pass.

- **Real hot-accept HMR (`desktop_hmr_issues.md` #6, #15) and watcher excludes
  (#12).** Tracked in the HMR-specific issues file.

- **Auto-update apply UI freeze.** The `op_desktop_apply_patch` path runs on the
  runtime thread for the duration of a bspatch + dylib write (a few seconds for
  a 100MB binary). Considered acceptable: it's a one-shot user-initiated update,
  mirrors Sparkle/Squirrel semantics. If we ever care, wrap the body in
  `spawn_blocking` and switch the op to `#[op2(async)]`.

- **Panic-hook error report can hang a few seconds before exit.** Best-effort
  network call inside the panic hook joins synchronously. Considered acceptable:
  it's the death-spiral path, and bounding the joiner adds more code than it
  saves.
