# Codebase map

A directory-by-directory tour of the repository, plus the files worth reading
first. For the conceptual layering behind this layout, see
[`architecture.md`](./architecture.md).

## Top-level directories

| Directory      | What lives here                                             |
| -------------- | ----------------------------------------------------------- |
| `cli/`         | The `deno` binary: subcommands, tooling, LSP, module loader |
| `runtime/`     | `deno_runtime`: assembles the JS runtime and workers        |
| `ext/`         | Extensions — native capabilities exposed to JavaScript      |
| `libs/`        | `deno_core` and the supporting resolution/packaging crates  |
| `tests/`       | All test suites (see [`testing.md`](./testing.md))          |
| `tools/`       | Dev scripts: `format.js`, `lint.js`, CI helpers, releases   |
| `third_party/` | Vendored dependencies and test fixtures                     |
| `coverage/`    | Coverage output                                             |

## Files to understand first

1. `cli/main.rs` — entry point and command routing.
2. `cli/args/flags.rs` — every CLI flag and subcommand (`clap`).
3. `runtime/worker.rs` — how a worker/runtime is initialized.
4. `runtime/permissions/` — the permission system that gates ops.
5. `cli/module_loader.rs` — module loading and resolution.

## Inside `cli/`

- `cli/args/` — flag parsing (`flags.rs`) and resolved configuration.
- `cli/tools/` — one module per subcommand. Simple example: `cli/tools/fmt.rs`.
  Complex example: the `cli/tools/test/` directory. Other notable tools include
  `compile.rs`, `bundle/`, `coverage/`, `lint/`, `pm/` (package management),
  `installer/`, `jupyter/`, `publish/`, `repl/`, and `serve.rs`.
- `cli/lsp/` — the language server.
- `cli/module_loader.rs`, `cli/graph_util.rs` — module graph construction and
  loading.

## Inside `ext/`

Each subdirectory is one extension (a Rust crate plus its JavaScript). Grouped
roughly by purpose:

- **Web platform:** `web`, `fetch`, `url`, `crypto`, `console`, `webidl`,
  `websocket`, `webgpu`, `canvas`, `image`, `broadcast_channel`, `webstorage`.
- **System access:** `fs`, `net`, `io`, `os`, `process`, `signals`, `tls`.
- **Deno APIs:** `kv`, `cron`, `cache`, `ffi`, `napi`, `bundle`, `telemetry`.
- **Node compatibility:** `node` (most `node:*` builtins), `node_crypto`,
  `node_sqlite`.

The conventional layout inside an extension is Rust ops in `lib.rs` (and
submodules) and JavaScript API files named with a numeric prefix that controls
load order (`00_*.js`, `01_*.js`, …).

## Inside `libs/`

The `deno_core` foundation and the crates the CLI composes for resolution and
packaging:

- **Core/V8 bridge:** `core`, `core_testing`, `ops`, `serde_v8`, `dcore`.
- **Resolution & packaging:** `resolver`, `node_resolver`, `npm`, `npm_cache`,
  `npm_installer`, `npmrc`, `package_json`, `lockfile`, `config`, `cli_parser`,
  `cache_dir`.
- **Other building blocks:** `crypto`, `dotenv`, `eszip`, `http_h1`,
  `inspector_server`, `maybe_sync`, `napi_sys`, `node_shim`,
  `typescript_go_client`.

## Inside `tests/`

- `tests/specs/` — the primary integration tests (CLI command + asserted
  output), driven by `__test__.jsonc` files.
- `tests/unit/` — JavaScript/TypeScript unit tests for runtime APIs
  (`*_test.ts`).
- `tests/unit_node/` — unit tests for the `node:*` compatibility layer.
- `tests/node_compat/` — Node.js's own test suite, run against Deno.
- `tests/wpt/` — Web Platform Tests.
- `tests/testdata/` — fixtures shared across suites.

See [`testing.md`](./testing.md) for the command that runs each suite.

## Inside `tools/`

Developer and CI scripts, all run with Deno:

- `tools/format.js` — format the whole tree (`deno fmt` plus extras).
- `tools/lint.js` — lint Rust and JS/TS; pass `--js` for JS/TS only.
- `tools/check_deno_core_changes.js`, `tools/check_docs_only_changes.js` — CI
  helpers that decide which jobs to run based on the changed files.
- `tools/release/` — release automation.

The `./x` helper at the repository root wraps the common build/test commands;
run `./x --help` to see what it offers.
