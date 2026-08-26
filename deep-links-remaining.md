# Deno Desktop Deep Links — Remaining Work

The merged PR [#35466](https://github.com/denoland/deno/pull/35466) implemented
the **declarative registration** half of deep-link support: telling the OS that
the app handles a `scheme://…` link at bundle time (macOS `CFBundleURLTypes`,
Linux `.desktop` `MimeType` + `Exec %u`, Windows `register-deep-links.bat`).

Everything below is the **delivery** half plus polish, tracked under
[#35465](https://github.com/denoland/deno/issues/35465). After clicking a link
the OS now routes to / launches the app, but the opened URL is **never delivered
to JS** — `DesktopEvent::OpenUrl { url }` exists as an enum variant
(`runtime/ops/desktop.rs:214`) but nothing constructs it, and it has no JS event
or TypeScript type.

## 1. Backend (laufey) — surface the OS open-URL signal — **blocker**

The event pipe already exists: backend →
`DesktopEventSender::try_send(DesktopEvent)` → channel → `op_desktop_recv_event`
(`runtime/ops/desktop.rs:1040`) → JSON with `kind` → JS. Nothing calls
`try_send(DesktopEvent::OpenUrl { … })`; that has to come from laufey, per
platform:

- **macOS:** implement the `application:openURLs:` delegate (Apple Event) and
  forward the URL(s).
- **Windows & Linux:** the OS spawns a _fresh process_ on every link click, so
  **single-instance forwarding** is required — the new process detects a running
  instance, hands it the URL (named pipe / socket / D-Bus), and exits. Without
  this, every click launches a duplicate app.

Requires a **laufey release** before the Rust side can consume it.

## 2. Cold-start launch URL

When the app is launched _by_ a link (not already running), the URL arrives
(argv on Windows/Linux, Apple Event on macOS) before the JS event loop and
handler are ready. It must be buffered and delivered once the handler attaches —
otherwise the first link click is silently dropped.

## 3. JS-facing API (the `open-url` event)

`OpenUrl` has no public surface today — unlike `DockReopen` / `TrayClick` it has
no `.d.ts` type and no `kind` serialization:

- Add `kind: "openUrl"` → `open-url` `CustomEvent` dispatch in the desktop JS
  layer.
- Add `OpenUrlDetail` + the event entry to `cli/tsc/dts/lib.deno.desktop.d.ts`.
- Add it to the `every_variant_has_camelcase_kind` / `kind_of` test coverage
  (`runtime/ops/desktop.rs:1885`).

## 4. Windows: actually apply the registration

Packaging only **drops** `register-deep-links.bat`; nothing runs it. To be
complete, the **MSI/installer should execute it on install** (and ideally remove
the keys on uninstall), so users don't have to run a script by hand.

## 5. Tests

Explicitly left out of the PR: a **spec test** that runs `deno desktop` with
`deepLinks` configured and asserts the `CFBundleURLTypes` / `.desktop`
`MimeType` / `register-deep-links.bat` output. Plus delivery tests once #1–#3
land.

---

**Ordering:** #1 is the blocker (needs the laufey release). #2–#3 complete the
runtime + JS path. #4–#5 are productionization / coverage. #3 (JS `open-url`
event + `.d.ts` types) and #5 (spec test) can be started now since they don't
depend on the laufey release.
