# Deno Desktop API

## API

The API is more browser focused, which makes sense since we are working with
browser environments, so the gap will blur even more than with base Deno.

`.d.ts` :
https://github.com/littledivy/deno/blob/desktop-framework-hmr/cli/tsc/dts/lib.deno.desktop.d.ts.

The main entrypoint into the API is the `Deno.BrowserWindow` class.

### HTTP Serving

Desktop apps need to serve their UI over HTTP to the webview, and the embedded
CEF browser navigates to `http://127.0.0.1:<port>`. The runtime automatically
sets `DENO_SERVE_ADDRESS` so `Deno.serve()` binds to that port without any user
configuration.

```tsx
export default {
  fetch() {},
};
```

also works.

### Framework detection

For plain apps, you write a `Deno.serve()` handler and it just works. But the
real win is framework support: `deno desktop` detects frameworks like Next.js,
Astro, Fresh, Remix, Nuxt, and SvelteKit by inspecting your project config.

When a framework is detected, the compiled app runs the framework's production
server (or dev server in `--hmr` mode) and the webview points at it
automatically. You get a desktop app from your existing web project without
changing any code.

### Events

events use the browser-standard events (resize, focus & blur, mousemove, click,
etc) wherever possible and reasonable.

### Bindings (RPC)

`win.bind(name, fn)` / `win.unbind(name)` — registers a Deno-side function
callable from the browser JS via `bindings.<name>(args)`.

### Integration into the CLI globals

APIs that are interactive in the Deno side can be turned into more native
desktop behaviour. A prime example is `prompt()`, `alert()` and `confirm()` ,
where in Deno they do a terminal prompt, but in Deno Desktop they do an actual
popup.

### HMR

`deno desktop --hmr` enables hot module replacement during development. The mode
is automatically selected based on the project:

- **Framework projects** (Next.js, Vite-based, etc.): the framework's own dev
  server runs and the webview connects to it directly — fast refresh, state
  preservation, error overlays all work as expected.
- **Non-framework apps**: Deno watches source files and hot-swaps changed
  modules into the running V8 instance via `Debugger.setScriptSource`. No
  framework or tooling required.

In both modes, the Deno runtime and CEF process stay alive. No restart, no
teardown, no reconnecting. Changes are visible effectively instantly

### Backends

The desktop runtime is built on WEF, an abstraction layer over multiple web
engines. The `--backend` flag selects which one:

- **CEF** (default): Bundled Chromium Embedded Framework. Consistent rendering
  across platforms, but large binary.
- **WebView**: Uses the OS system webview (WKWebView on macOS, WebView2 on
  Windows, WebKitGTK on Linux). Much smaller app size, but rendering and feature
  support varies per platform and OS version.
- **Servo**: Experimental. Mozilla's Servo engine. (probably not worth shipping)
- **Raw**: Winit-based backend with no web engine. Provides window management,
  events, and the native API surface, but no webview rendering. Useful for apps
  that do their own rendering (e.g. via WebGPU directly).

The same app code works across all backends: WEF provides a unified Rust API for
window management, bindings, JS execution, events, and navigation. Switching
backends is a one-flag change, no code modifications (except the raw backend).

### Command

```bash
deno desktop main.tsx
deno desktop --hmr main.tsx
deno desktop --output MyApp.app main.tsx

Usage: deno desktop [OPTIONS] [SCRIPT_ARG]...

Desktop options:
      --all-targets        Build for all supported target platforms
      --backend <backend>  WEF backend to use for the desktop app [default: cef] [possible values: webview, cef, servo]
      --exclude <exclude>  Excludes a file/directory in the compiled executable.
                             Use this flag to exclude a specific file or directory within the included files.
      --hmr                Run the desktop app with Hot Module Replacement enabled
      --icon <icon>        Set the application icon (.ico on Windows, .icns or .png on macOS)
      --include <include>  Includes an additional module or file/directory in the compiled executable.
                             Use this flag if a dynamically imported module or a web worker main module
                             fails to load in the executable or to embed a file or directory in the executable.                                                                        
                             This flag can be passed multiple times, to include multiple additional modules.                                                                           
  -o, --output <output>    Output path (e.g. MyApp.app, MyApp.dmg, MyApp.msi)
      --target <target>    Target OS architecture [possible values: x86_64-unknown-linux-gnu, aarch64-unknown-linux-gnu, x86_64-pc-windows-msvc, x86_64-apple-darwin, aarch64-apple-darwin]
```

### Config (deno.json)

```json
{
  "desktop": {
    "app": {
      "name": "My App",
      "icons": {
        // Single file per platform
        "macos": "./icons/app.icns",
        "windows": "./icons/app.ico",
        "linux": "./icons/app.png"
        // Or multiple PNGs at specific sizes
        // "macos": [
        //   { "path": "./icons/16.png", "size": 16 },
        //   { "path": "./icons/32.png", "size": 32 },
        //   { "path": "./icons/128.png", "size": 128 },
        //   { "path": "./icons/256.png", "size": 256 },
        //   { "path": "./icons/512.png", "size": 512 }
        // ]
      }
    },
    "backend": "cef",
    "output": {
      "macos": "./dist/MyApp.app",
      "windows": "./dist/MyApp",
      "linux": "./dist/my-app"
    },
    "release": {
      "baseUrl": "https://releases.example.com/my-app"
    },
    "errorReporting": {
      "url": "https://errors.example.com/report"
    }
  }
}
```

### Auto-updater

Binary-diff updates (Electrobun-style) using bsdiff/bspatch.

The JS API:

- `Deno.desktopVersion` : the app version from deno.json
- `Deno.autoUpdate()` : starts polling a release server for updates

```tsx
Deno.autoUpdate({
  onUpdateReady(version) {
    // update downloaded, will apply on next launch
  },
  onRollback(reason) {
    // last update failed, rolled back
  },
});
```

The update flow:

1. Fetches `<baseUrl>/latest.json` for a manifest with
   `{ version, patches: { "1.0.0": "patch-1.0.0-to-1.1.0.bin" } }`
2. If the manifest version differs from the current version and a patch exists
   for the current version, downloads and applies the binary diff to the running
   dylib
3. Writes the patched binary as `<dylib>.update` — applied on next launch
4. Fires a `"desktop-update-ready"` event with `{ detail: { version } }`
5. On next launch, if the updated binary fails to start, it rolls back and fires
   a `"desktop-update-rollback"` event.

### Error reporting

Catches uncaught errors, unhandled rejections and panics, shows a native alert,
and optionally `POST`s a report, with the following format:

```json
{
  "version": 1,
  "message": "TypeError: Cannot read properties of null",
  "stack": "...",
  "appVersion": "1.0.0",
  "timestamp": "2026-04-08T12:00:00.000Z",
  "platform": "darwin",
  "arch": "aarch64"
}
```

### Why this API / how does the RPC work?

Inspired by Electrobun, with Electron compatibility where possible. The key
difference from Electron's IPC: the binding mechanism is **not IPC**: it's
in-process thread communication. The WEF backend (CEF) runs on the main thread,
Deno's tokio runtime runs on another. Calls go through `tokio::sync::mpsc`
channels and `oneshot` channels for responses, with the WEF capi dispatching via
a notify/poll pattern (`wef::run()` loop). This avoids serialization overhead of
a socket-based IPC.

The webview side uses a JS namespace proxy, and any property access on it
creates a function that, when called, sends args via CEF's process message IPC
(renderer → browser process), which routes it to the Deno runtime.

## Distribution

`deno desktop --output MyApp.app` produces a self-contained `.app` bundle on
macOS, with the WEF/CEF framework embedded under `Contents/Frameworks/`.
Self-extracting mode is supported: the VFS is extracted to disk at runtime so
frameworks like Next.js can find their build output relative to CWD.

On Windows, the output should be a directory containing the `.exe` and CEF DLLs,
suitable for zipping or feeding into an installer toolchain (NSIS, WiX, etc.).
`--output MyApp.msi` or `--output MyApp.exe` could optionally produce a
self-extracting installer directly.

On Linux, the output should be an AppImage or a directory structure suitable for
packaging into `.deb`/`.rpm`/Flatpak. AppImage is the most portable: single
file, no install step, runs on any distro.

### Installers

On macOS, `--output MyApp.dmg` should produce a DMG disk image with
drag-to-Applications. On Windows, `--output MyApp.msi` should produce an MSI
installer. These could be

implemented by shelling out to platform tools (`hdiutil` on macOS,
WiX/`light.exe` on Windows) or bundling a minimal installer generator.

For Linux, generating `.deb` and `.rpm` packages is straightforward (they're
just archives with metadata), and could be supported via `--output myapp.deb` /
`--output myapp.rpm`.

### Cross-compilation

`--target` and `--all-targets` already exist in the CLI. The JS/TS bundle is
platform-independent: only the `denort` binary and CEF framework differ per
platform.

Cross-compilation means downloading the right prebuilt CEF binaries and `denort`
for the target triple, then packaging them with the same VFS. No actual Rust
cross-compilation needed, same as `deno compile --target` today.

## Comparison

|                           | Electron              | Electrobun          | Tauri                | Dioxus               | Deno Desktop           |
| ------------------------- | --------------------- | ------------------- | -------------------- | -------------------- | ---------------------- |
| **Language**              | JS/TS (Node.js)       | JS/TS (Bun)         | Rust + web frontend  | Rust                 | JS/TS (Deno)           |
| **Web engine**            | Bundled Chromium      | System WebView      | System WebView (WRY) | System WebView (WRY) | Bundled CEF or WebView |
| **Consistent rendering    |                       |                     |                      |                      |                        |
| across platforms**        | ✅                    | ❌                  | ❌                   | ❌                   | ✅                     |
| **Process model**         | Multi-process         | Multi-process       | Multi-process        | Single process       | Multi-thread           |
| **Backend ↔ UI**          | IPC (cross-process)   | IPC (cross-process) | IPC (cross-process)  | Native Rust          | Channels (in-process)  |
| **App size**              | ~250MB                | ~14MB (fake)        | ~2–10MB              | ~5MB                 | CEF ~350MB             |
| webview ~116MB            |                       |                     |                      |                      |                        |
| **npm/Node compat**       | ✅ (it is Node)       | ✅ (via Bun)        | ❌                   | ❌                   | ✅                     |
| **Framework auto-detect** | ❌                    | ❌                  | ❌                   | ❌                   | ✅                     |
| HMR                       | ❌                    | ✅                  | ✅ (Vite-based)      | ✅ (`dx serve`)      | ✅                     |
| **Built-in auto-update**  | ✅ (full binary)      | ✅ (bsdiff)         | ❌ (plugin)          | ❌                   | ✅ (bsdiff)            |
| **Built-in installers**   | ✅                    | ❌                  | ✅                   | ❌                   | ✅                     |
| **Cross-compile**         | ✅ (electron-builder) | ❌                  | ❌ (needs native)    | ❌ (needs native)    | ✅ (`--target`)        |
| **macOS**                 | ✅                    | ✅                  | ✅                   | ✅                   | ✅                     |
| **Windows**               | ✅                    | ❌                  | ✅                   | ✅                   | ✅                     |
| **Linux**                 | ✅                    | ❌                  | ✅                   | ✅                   | ✅                     |
| **iOS / Android**         | ❌                    | ❌                  | ✅                   | ✅                   | ❌ (doable)            |

### What we provide

- **Zero config framework support:** `deno desktop .` on a Next.js/Astro/etc.
  project just works. No one else does this.
- **Cross-compile from one machine:** same as `deno compile --target`. Tauri and
  Dioxus need the native platform to build. Electrobun only does macOS.
- **Bundled engine + npm compat:** Electron has both but is massive.
  Tauri/Dioxus are small but have no JS ecosystem. We bundle CEF for consistent
  rendering AND have full Node compat via Deno.
- **Built-in auto-update with bsdiff:** Electron ships full binaries. Tauri and
  Dioxus have nothing built-in. We and Electrobun do binary diffs, but ours is
  integrated into the runtime, not a separate tool.
- **Unified DevTools for Deno backend + webview**: right now you debug one or
  the other, never both. A single `--inspect` session showing the Deno runtime
  and the CEF webview together eliminates the constant context-switching. (only
  works on CEF backend, not webiview backend)

### What more we could potentially provide

- **Runtime permissions for desktop apps**: no desktop framework gates what the
  app can access. Deno already has the permission system; applying it here means
  users can trust that a desktop app can't touch the filesystem or network
  unless explicitly allowed.
- **Codesigning/notarization as a flag**: currently requires separate tools,
  platform-specific scripts, and CI configuration. A `--sign` flag makes
  shipping a signed app a one-liner instead of a day of yak-shaving.
- **Shared CEF runtime across all Deno Desktop apps**: instead of each app
  bundling its own Chromium (60MB+), a single managed CEF installation on the
  system is shared by all apps. App sizes drop to a few MB, system memory drops
  because engine pages are shared. Same rendering consistency as Electron,
  without the bloat.
- **Persistent background service for instant startup**: a single CEF process
  stays warm after the first app launches. Subsequent apps and windows connect
  to it and appear in milliseconds instead of bootstrapping Chromium from
  scratch every time.

## Why a subcommand and not a separate project?

It's deeply integrated into Deno's existing infrastructure:

- Reuses `deno compile` for bundling (`cli/tools/compile.rs` has
  desktop-specific paths).
- Deno is an all-in-one toolkit: the benefit of just doing `deno --help` and
  seeing there is desktop capabilities is enticing, especially with just being
  able to do a simple `deno desktop .` and having an app.
- Framework detection (`cli/tools/framework.rs`) ties into existing
  workspace/config infrastructure; altough possible to do outside of CLI or
  having a way in CLI to expose this capability to users.
- The runtime (`cli/rt_desktop/lib.rs`) builds on top of `denort` — the same
  runtime used by `deno compile` standalone binaries, including the module
  loader, permissions, npm support, VFS, and HMR.
- If its a separate project, it would have to pull in `deno_runtime`,
  `deno_lib`, `deno_core`, `deno_config`, `deno_resolver` and more into the
  dependency tree. At that point it's separate in name only and just cause more
  trouble keeping it up to date.
  - Also we use ops: if we have it separately that would add an additional
    external system using ops: not ideal, this would be the non-ideal setup like
    deploy classic all over.
  - Config file handling would get even more fragmented: deploy-cli is like
    this, where we depend on `deno_config` and then use it, but every time
    anything needs changing we need to go back to CLI repo and coordinate.
- Various flags parsing for the command would need to be duplicated.

## Future Features

- Clipboard: Same as the notifications API, using the Clipboard API.
- secureStorage:
  [https://www.electronjs.org/docs/latest/api/safe-storage](https://www.electronjs.org/docs/latest/api/safe-storage)
