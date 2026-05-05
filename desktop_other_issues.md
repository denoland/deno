# Desktop (non-HMR): Bugs and Issues

Audit of the `deno desktop` feature on branch `desktop-framework-hmr`, covering
correctness/security/lifecycle issues _outside_ the HMR runner (those live in
`desktop_hmr_issues.md` and were addressed in commit `e768fc3a70`).

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

Severity tags: **CRITICAL** (exploitable / silent data loss), **HIGH** (process
stability / privilege boundary), **MEDIUM** (deadlock / wrong behaviour under
load), **LOW** (polish / belt-and-braces).

---

## Security

### 2. Tar traversal check misses symlink-based escape — HIGH

`cli/tools/desktop.rs:1015-1022`

The traversal guard inspects each entry's path components but
`tar::Entry::unpack` will still follow a previously-extracted symlink to write
outside `dest`. Classic two-entry attack: entry A is symlink `foo -> ../../etc`,
entry B writes `foo/passwd`.

**Fix**: refuse `Symlink`/`Hardlink` whose link target contains `..` or absolute
components, or call `unpack_in(dest)` so tar's own check applies.

### 4. Launcher shell scripts inject names unsanitized — MEDIUM

`cli/tools/desktop.rs:1257-1267, 700-711`

`wef_executable_name` is read from the WEF Info.plist (item #3) and
`dylib_filename` ultimately derives from `desktop_flags.output` / app config
name. Either could contain `"`, `$`, backticks, or newlines and break out of the
quoted argument in the bash heredoc.

**Fix**: validate names against `[A-Za-z0-9._-]+`, or shell-escape before
interpolation.

### 5. Same problem in the Windows `.bat` launcher — MEDIUM

`cli/tools/desktop.rs:582-592`

`.bat` quoting is even worse than POSIX shell — `&`, `^`, `%` in
`app_name`/`wef_binary_name`/`dylib_filename` will run as commands.

**Fix**: validate names before writing.

### 6. DMG staging dir is created in shared parent with predictable name — LOW

`cli/tools/desktop.rs:1376-1383`

`.{dmg}.dmg-staging` lives next to the user's chosen output, with a predictable
name and no umask narrowing. On a multi-user macOS host another user could
race-create or symlink the staging path before `create_dir_all`.

**Fix**: stage in `tempfile::tempdir_in` under the cache dir.

### 7. TOCTOU between `marker.exists()` and extraction — MEDIUM

`cli/tools/desktop.rs:792-859`

Two concurrent `deno desktop` builds (or one + a rerun after Ctrl-C mid-extract)
can both pass the `marker.exists()` check or both blow away the dir while one is
mid-read. The marker is also written _after_ extraction completes, so a SIGKILL
mid-extract leaves a half-populated dir without a marker — the next run will
`remove_dir_all` it (good), but a _parallel_ run will read partial files.

**Fix**: extract into a tempdir then atomic-rename, or take a file lock on
`dir/.lock`.

### 9. Temp entrypoint written to user's source dir without unique name — LOW

`cli/tools/desktop.rs:209-211`

`.deno_desktop_entry.ts` collides across two simultaneous `deno desktop`
invocations in the same project; the `CleanupGuard` of the first run can also
delete the second's file mid-compile. The path is fixed, so an attacker with
write access to the cwd can pre-create it as a symlink.

**Fix**:
`tempfile::Builder::new().prefix(".deno_desktop_entry").tempfile_in(cwd)`.

---

## Soundness / UB

### 12. `dlopen(self, RTLD_NOLOAD|RTLD_GLOBAL)` not checked, no SAFETY comment — LOW

`cli/rt_desktop/lib.rs:758-781`

The call is sound (the dylib stays loaded; we add a refcount only on success),
but the `unsafe` block lacks a `SAFETY:` comment and silently ignores a failure
return.

**Fix**: add a SAFETY comment; log/panic on null return so we don't silently
fail to expose NAPI symbols.

### 13. `instance_create_surface` unsafe block panics inside a closure — MEDIUM

`runtime/ops/desktop.rs:685-689`

`.expect("failed to create surface")` is held inside a closure invoked from a v8
callback path. The failure mode is wgpu-side (unsupported display, OOM, plugin
missing) — user-controlled paths (window minimized, display detached) can hit
it.

**Fix**: bubble the error as a `JsErrorBox` instead of `unwrap`.

---

## Correctness

### 14. `dylib_path.file_stem().unwrap()` on user-controlled path — LOW

`cli/tools/desktop.rs:540-543, 646-650, 1189-1192`

`dylib_path` comes from `compile_binary` which derives from
`desktop_flags.output`. `--output /` or `--output .` panics. Same with
`dylib_path.parent().unwrap()` on root paths.

**Fix**: error message instead of `unwrap`.

### 15. Checksum mismatch error doesn't include the URL — LOW

`cli/tools/desktop.rs:843-849`

On checksum mismatch the message says only the archive name; an attacker who
poisoned an HTTP redirect would benefit from us logging the _final_ URL too.

**Fix**: include `url` (post-redirect, if obtainable) in the bail message.

### 17. Single-threaded tokio runtime stalls WEF event pump on big I/O — MEDIUM

`cli/rt_desktop/lib.rs:992-1017`

`new_current_thread` + `block_on` means any blocking call (e.g. `std::fs::read`
of dylib for `op_desktop_apply_patch` on a 100MB binary) stalls the whole event
loop including the WEF event pump.

**Fix**: spawn `op_desktop_apply_patch` body via `spawn_blocking`.

### 18. `inspect_internal_port` parsed silently to `None` on bad input — LOW

`cli/rt_desktop/lib.rs:1290-1306`

If the parent passes a malformed `DENO_DESKTOP_INSPECT_INTERNAL_PORT`, the
inspector silently isn't created and the user wonders why DevTools shows
nothing.

**Fix**: log or bail when the env var is set but unparseable.

### 19. `port_u16().unwrap_or(80)` on a `ws://` URL — LOW

`cli/tools/desktop_devtools.rs:670-675`

Defaulting to 80 when an upstream URL omits a port is wrong for WS — any real
Deno/CEF inspector will have one, but a malformed `/json/list` response would
cause a connect to port 80 of the upstream host.

**Fix**: bail when missing.

### 20. Mux listener `accept()` errors retry forever with 200ms backoff — LOW

`cli/tools/desktop_devtools.rs:135-161`

A persistent EMFILE situation will spin forever logging once per 200ms.
Acceptable for a dev-only tool; flagging for completeness.

**Fix**: cap retries, or log only once per N-second window.

### 21. `openDevtools` leaks windows on repeat calls — LOW

`cli/rt_desktop/lib.rs:265-284`

Each call to `openDevtools()` creates a fresh `just_wef::Window` and the
`_ = setup_window_events(window)` discards it without tracking —
`closed_windows` / `open_windows` state never gets updated.

**Fix**: track the devtools window or reuse a singleton.

---

## Resource / lifecycle

### 22. `tokio::spawn(navigate_fut)` is never cancelled — LOW

`cli/rt_desktop/lib.rs:1530`

When `wef_fut` returns first (window closed), the spawned navigate poll keeps
trying for up to 15s post-shutdown. Harmless on exit, but writes warnings to
stderr after the user closes the window.

**Fix**: hold the JoinHandle and abort when select wins.

### 23. Panic hook joins a thread that builds its own runtime per call — MEDIUM

`cli/rt_desktop/lib.rs:1003-1025`

Every panic-path error report spawns a thread + tokio runtime + does `.join()`
synchronously inside the panic hook. A network hang ⇒ panic hook hangs ⇒ process
won't exit.

**Fix**: bound it with a deadline (`thread::spawn` then a short `recv_timeout`
on a channel; abandon thread on timeout).

---

## Cross-platform

### 26. `strip_cef_bloat` macOS-only — not a bug

`cli/tools/desktop.rs:558`

Confirmed only called from `package_macos_app_bundle`. No action.

### 27. Launchers use `#!/bin/bash` — LOW

`cli/tools/desktop.rs:706-712, 1262-1267`

Some minimal Linux containers / Alpine without bash will fail. The script body
is sh-compatible.

**Fix**: switch to `#!/bin/sh`.

### 28. `unsafe { std::env::set_var(...) }` after threads have started — HIGH

`cli/rt_desktop/lib.rs:1311-1317`

Reached after `tokio::runtime::Builder::new_current_thread().build()` and after
potentially `tokio::spawn`-ed work in panic-hook init. Linux glibc setenv is not
thread-safe; Rust 1.81+ flagged this for a reason.

**Fix**: set `DENO_SERVE_ADDRESS` _before_ the runtime is built, or pass it
through `RunOptions` (which already carries `serve_port`/`serve_host`).

### 29. `set_current_dir` while other tasks may be doing relative-path I/O — MEDIUM

`cli/rt_desktop/lib.rs:1064`

Worker init changes cwd in the middle of `tokio::runtime::block_on`. Any
concurrent task resolving a relative path (worker subtasks, NAPI addons started
during `run_with_options`) sees an inconsistent cwd.

**Fix**: do the chdir before runtime init (move it above `rt.block_on`).

### 30. `download_with_progress_and_retries` passes empty headers — LOW

`cli/tools/desktop.rs:830-842`

No `User-Agent`; some CDNs (incl. parts of GitHub releases) start rate-limiting
empty UA aggressively.

**Fix**: pass a `deno-desktop/{ver}` UA.

---

## Lint (pre-existing)

### 31. 8 clippy errors in `runtime/ops/desktop.rs` — LOW (cleanup)

Disallowed `std::fs::read`/`write`/`OpenOptions` (lines 799, 833, 906, plus
`cli/rt_desktop/lib.rs` lines 992-999, 1099-1102), `url::Url::to_file_path`
(line 1041), missing `Default` on `PendingBindResponses`, collapsible `if`s in
`op_desktop_send_error_report` and elsewhere.

These pre-date this branch and currently block `tools/lint.js`.

---

## Severity summary

| Severity | Count | Items                                                         |
| -------- | ----- | ------------------------------------------------------------- |
| HIGH     | 2     | #2, #28                                                       |
| MEDIUM   | 5     | #7, #13, #17, #23, #29                                        |
| LOW      | 14    | #6, #9, #12, #14, #15, #18, #19, #20, #21, #22, #27, #30, #31 |

## Suggested fix order

**First batch — exploitable / process-stability bugs**, all local edits without
architectural changes:

1. **#2** tar symlink escape (`unpack_in`)
2. **#28** move `set_var(DENO_SERVE_ADDRESS)` before runtime init

**Second batch — robustness wins**:

10. **#4 / #5** validate launcher names
11. **#7** atomic-rename WEF cache extraction
12. **#13** wgpu surface error → JsErrorBox
13. **#17** `spawn_blocking` for auto-update I/O
14. **#23** bound the panic-hook reporter

**Cleanup batch** — everything else (#6, #9, #12, #14, #15, #18, #19, #20, #21,
#22, #27, #30) plus **#31** to unblock `tools/lint.js`.
