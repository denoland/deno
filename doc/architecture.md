# Architecture overview

Deno is built as a stack of layers. Each layer depends only on the ones below
it, which keeps the system testable and lets the lower layers be reused outside
of the `deno` binary. From the top down:

```
+-----------------------------------------------------------+
|  cli/            the `deno` binary: subcommands, tooling   |
+-----------------------------------------------------------+
|  runtime/        deno_runtime: assembles the JS runtime    |
+-----------------------------------------------------------+
|  ext/*           extensions: native capabilities for JS    |
+-----------------------------------------------------------+
|  libs/*          deno_core + supporting crates (V8 bridge) |
+-----------------------------------------------------------+
|  V8 + Tokio      JavaScript engine and async runtime       |
+-----------------------------------------------------------+
```

## The CLI layer (`cli/`)

The `deno` crate in `cli/` is everything a user touches directly. It owns flag
parsing, the subcommands (`run`, `test`, `fmt`, `lint`, `compile`, `bundle`,
`install`, `publish`, and so on), package-management tooling, the LSP, and the
module loader that ties module resolution to the runtime.

Key entry points:

- `cli/main.rs` — process entry point and command routing.
- `cli/args/flags.rs` — the full `clap` flag and subcommand definition. Adding a
  flag or a subcommand starts here.
- `cli/tools/<tool>/` — one module per subcommand (for example
  `cli/tools/fmt.rs` for a simple command, `cli/tools/test/` for a complex one).
- `cli/module_loader.rs` — resolves and loads modules, bridging the resolver and
  the graph to the runtime.

The CLI is intentionally heavy: it pulls in TypeScript checking, npm and JSR
resolution, the lockfile, and the bundler. Lower layers must not depend back up
into it.

## The runtime layer (`runtime/`)

The `deno_runtime` crate assembles a working JavaScript runtime out of
`deno_core` plus a curated set of extensions. It is the piece embedders use when
they want "Deno the runtime" without "Deno the CLI".

Key files:

- `runtime/worker.rs` — constructs the main worker: an isolate, the op set, and
  the bootstrap sequence.
- `runtime/web_worker.rs` — the Web Worker variant.
- `runtime/permissions/` — the permission model that gates every sensitive op
  (read, write, net, env, run, ffi, sys). Permissions are checked in Rust at the
  op boundary, never in JavaScript.

## The extension layer (`ext/*`)

Each directory under `ext/` is a self-contained extension: a Rust crate that
defines **ops** (native functions callable from JS) plus the JavaScript that
exposes a higher-level API on top of them. This is where the platform actually
lives. Examples:

- Web platform: `ext/web`, `ext/fetch`, `ext/url`, `ext/crypto`, `ext/console`,
  `ext/webidl`, `ext/websocket`, `ext/webgpu`, `ext/canvas`.
- System access: `ext/fs`, `ext/net`, `ext/io`, `ext/os`, `ext/process`,
  `ext/signals`, `ext/tls`.
- Deno-specific: `ext/kv`, `ext/cron`, `ext/cache`, `ext/ffi`, `ext/napi`,
  `ext/bundle`.
- Node compatibility: `ext/node` (the bulk of the `node:*` builtins, both Rust
  ops and JavaScript polyfills), plus `ext/node_crypto` and `ext/node_sqlite`.

The typical shape of an extension is:

1. Rust `#[op2]` functions that do the privileged work and take a permission
   check where needed.
2. A `00_*.js` / `01_*.js` set of JavaScript modules that build the public API
   and call the ops through `Deno.core.ops`.
3. Registration of the extension in `runtime/worker.rs` (and the CLI's snapshot)
   so it is part of the assembled runtime.

When adding native functionality, add the op in the relevant `ext/<name>/`
crate; do not reach into the runtime or CLI for it.

## The core layer (`libs/*`)

`libs/` holds `deno_core` and the crates merged in from the former standalone
`deno_core` repository. This is the bridge between Rust and V8: it owns the op
infrastructure, the module loader trait, the snapshotting machinery, the
JsRuntime event loop, and the `serde_v8` serialization layer.

Notable members:

- `libs/core` — `deno_core` itself: `JsRuntime`, op registration, the module
  map, the inspector integration.
- `libs/ops` — the `#[op2]` proc-macro that generates the Rust/V8 glue.
- `libs/serde_v8` — zero-copy-ish serialization between Rust types and V8
  values.
- `libs/resolver`, `libs/node_resolver`, `libs/npm`, `libs/npm_installer`,
  `libs/package_json`, `libs/lockfile`, `libs/config`, `libs/npmrc` — the
  resolution and package-management building blocks the CLI composes.
- `libs/typescript_go_client` — client for the out-of-process TypeScript
  type-checker.

These crates are deliberately free of CLI concerns so they can be unit-tested in
isolation and reused by other tools.

## Cross-cutting concepts

- **Ops** are the only way JavaScript reaches native code. An op is a Rust
  function exposed to JS; sync ops return immediately, async ops return a future
  that resolves on the event loop.
- **Extensions** bundle ops with their JavaScript and are the unit of
  composition for a runtime.
- **Workers** are isolated JavaScript execution contexts (the main worker and
  Web Workers); each has its own V8 isolate.
- **Resources** are managed handles (open files, sockets, readers) tracked by
  `deno_core` and passed across the Rust/JS boundary by integer id.
- **Permissions** are enforced in Rust at the op boundary. A capability that is
  not granted causes the op to error before doing any work.

## Where to make a change

| You want to…                         | Start in…                         |
| ------------------------------------ | --------------------------------- |
| Add or change a CLI flag/subcommand  | `cli/args/flags.rs`, `cli/tools/` |
| Add a native capability to JS        | `ext/<name>/` (op + JS)           |
| Change how the runtime is assembled  | `runtime/worker.rs`               |
| Touch the Rust/V8 bridge or op macro | `libs/core`, `libs/ops`           |
| Change module/npm/JSR resolution     | `libs/resolver`, `libs/npm`, CLI  |

See [`codebase-map.md`](./codebase-map.md) for a finer-grained directory map and
[`testing.md`](./testing.md) for how to test each layer.
