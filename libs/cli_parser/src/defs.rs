// Copyright 2018-2026 the Deno authors. MIT license.
//! Static command definitions for the Deno CLI.
//!
//! All definitions are `const` - they live in `.rodata` and cost zero
//! runtime initialization.

use crate::types::*;

/// Allowed values for the executable `--ext` flag (run/serve/eval/test/bench/
/// compile/desktop/lint), mirroring clap's `executable_ext_arg`.
const EXECUTABLE_EXTS: &[&str] =
  &["ts", "tsx", "js", "jsx", "mts", "mjs", "cts", "cjs"];

/// Supported `--target` triples for `compile`/`desktop` (clap's `SUPPORTED_OS`).
const SUPPORTED_OS: &[&str] = &[
  "x86_64-unknown-linux-gnu",
  "aarch64-unknown-linux-gnu",
  "x86_64-pc-windows-msvc",
  "aarch64-pc-windows-msvc",
  "x86_64-apple-darwin",
  "aarch64-apple-darwin",
];

// ============================================================
// Shared arg groups
// ============================================================

pub static PERMISSION_ARGS: &[ArgDef] = &[
  ArgDef::new("allow-all")
    .short('A')
    .long("allow-all")
    .set_true()
.help("Allow all permissions")
.hidden(),
  ArgDef::new("allow-read")
    .short('R')
    .long("allow-read")
    .action(ArgAction::Append)
    .num_args(NumArgs::ZeroOrMore)
    .require_equals()
    .value_delimiter(',')
.hidden(),
  ArgDef::new("deny-read")
    .long("deny-read")
    .action(ArgAction::Append)
    .num_args(NumArgs::ZeroOrMore)
    .require_equals()
    .value_delimiter(',')
.hidden(),
  ArgDef::new("ignore-read")
    .long("ignore-read")
    .action(ArgAction::Append)
    .num_args(NumArgs::ZeroOrMore)
    .require_equals()
    .value_delimiter(',')
.hidden(),
  ArgDef::new("allow-write")
    .short('W')
    .long("allow-write")
    .action(ArgAction::Append)
    .num_args(NumArgs::ZeroOrMore)
    .require_equals()
    .value_delimiter(',')
.hidden(),
  ArgDef::new("deny-write")
    .long("deny-write")
    .action(ArgAction::Append)
    .num_args(NumArgs::ZeroOrMore)
    .require_equals()
    .value_delimiter(',')
.hidden(),
  ArgDef::new("allow-net")
    .short('N')
    .long("allow-net")
    .action(ArgAction::Append)
    .num_args(NumArgs::ZeroOrMore)
    .require_equals()
    .value_delimiter(',')
.hidden(),
  ArgDef::new("deny-net")
    .long("deny-net")
    .action(ArgAction::Append)
    .num_args(NumArgs::ZeroOrMore)
    .require_equals()
    .value_delimiter(',')
.hidden(),
  ArgDef::new("allow-env")
    .short('E')
    .long("allow-env")
    .action(ArgAction::Append)
    .num_args(NumArgs::ZeroOrMore)
    .require_equals()
    .value_delimiter(',')
.hidden(),
  ArgDef::new("deny-env")
    .long("deny-env")
    .action(ArgAction::Append)
    .num_args(NumArgs::ZeroOrMore)
    .require_equals()
    .value_delimiter(',')
.hidden(),
  ArgDef::new("ignore-env")
    .long("ignore-env")
    .action(ArgAction::Append)
    .num_args(NumArgs::ZeroOrMore)
    .require_equals()
    .value_delimiter(',')
.hidden(),
  ArgDef::new("allow-run")
    .long("allow-run")
    .action(ArgAction::Append)
    .num_args(NumArgs::ZeroOrMore)
    .require_equals()
    .value_delimiter(',')
.hidden(),
  ArgDef::new("deny-run")
    .long("deny-run")
    .action(ArgAction::Append)
    .num_args(NumArgs::ZeroOrMore)
    .require_equals()
    .value_delimiter(',')
.hidden(),
  ArgDef::new("allow-sys")
    .short('S')
    .long("allow-sys")
    .action(ArgAction::Append)
    .num_args(NumArgs::ZeroOrMore)
    .require_equals()
    .value_delimiter(',')
.hidden(),
  ArgDef::new("deny-sys")
    .long("deny-sys")
    .action(ArgAction::Append)
    .num_args(NumArgs::ZeroOrMore)
    .require_equals()
    .value_delimiter(',')
.hidden(),
  ArgDef::new("allow-ffi")
    .long("allow-ffi")
    .action(ArgAction::Append)
    .num_args(NumArgs::ZeroOrMore)
    .require_equals()
    .value_delimiter(',')
.hidden(),
  ArgDef::new("deny-ffi")
    .long("deny-ffi")
    .action(ArgAction::Append)
    .num_args(NumArgs::ZeroOrMore)
    .require_equals()
    .value_delimiter(',')
.hidden(),
  ArgDef::new("allow-import")
    .short('I')
    .long("allow-import")
    .action(ArgAction::Append)
    .num_args(NumArgs::ZeroOrMore)
    .require_equals()
    .value_delimiter(',')
.help("Allow importing from remote hosts. Optionally specify allowed IP addresses and host names, with ports as necessary. Default value: deno.land:443,jsr.io:443,esm.sh:443,raw.esm.sh:443,cdn.jsdelivr.net:443,raw.githubusercontent.com:443,gist.githubusercontent.com:443")
.hidden(),
  ArgDef::new("deny-import")
    .long("deny-import")
    .action(ArgAction::Append)
    .num_args(NumArgs::ZeroOrMore)
    .require_equals()
    .value_delimiter(',')
.help("Deny importing from remote hosts. Optionally specify denied IP addresses and host names, with ports as necessary.")
.hidden(),
  ArgDef::new("no-prompt").long("no-prompt").set_true()
.hidden(),
  // Removed in Deno 2 — still accepted so we can print the deprecation
  // warning in `convert` instead of failing with "unexpected argument".
  ArgDef::new("allow-hrtime")
    .long("allow-hrtime")
    .set_true()
    .hidden(),
  ArgDef::new("deny-hrtime")
    .long("deny-hrtime")
    .set_true()
    .hidden(),
  ArgDef::new("permission-set")
    .short('P')
    .long("permission-set")
    .action(ArgAction::Set)
    .num_args(NumArgs::Optional)
    .require_equals()
.hidden(),
];

pub static COMPILE_ARGS: &[ArgDef] = &[
  ArgDef::new("no-check")
    .long("no-check")
    .action(ArgAction::Set)
    .num_args(NumArgs::Optional)
    .require_equals()
    .conflicts_with(&["check"])
.help("Skip type-checking. If the value of \"remote\" is supplied, diagnostic errors from remote modules will be ignored"),
  ArgDef::new("import-map")
    .long("import-map")
    .long_aliases(&["importmap"])
    .action(ArgAction::Set)
    .num_args(NumArgs::Exact(1))
.help("Load import map file from local file or remote URL\n  Docs: https://docs.deno.com/runtime/manual/basics/import_maps"),
  ArgDef::new("no-remote").long("no-remote").set_true()
.help("Do not resolve remote modules"),
  ArgDef::new("no-npm").long("no-npm").set_true()
.help("Do not resolve npm modules"),
  ArgDef::new("node-modules-dir")
    .long("node-modules-dir")
    .action(ArgAction::Set)
    .num_args(NumArgs::Optional)
    .require_equals()
    .value_parser(ValueParser::Choices(&[
      "auto", "true", "manual", "none", "false",
    ]))
.help("Selects the node_modules directory mode for npm packages (not a path). One of: auto (create a local node_modules directory and install npm packages into it), manual (use the existing local node_modules directory, do not modify it), none (do not use a local node_modules directory; resolve npm packages from the global cache). Defaults to auto when the flag is passed without a value."),
  ArgDef::new("vendor")
    .long("vendor")
    .action(ArgAction::Set)
    .num_args(NumArgs::Optional)
    .require_equals()
    .value_parser(ValueParser::Bool)
.help("Toggles local vendor folder usage for remote modules and a node_modules folder for npm packages"),
  ArgDef::new("node-modules-linker")
    .long("node-modules-linker")
    .long_aliases(&["linker"])
    .action(ArgAction::Set)
    .num_args(NumArgs::Exact(1))
    .require_equals()
    .value_parser(ValueParser::Choices(&["isolated", "hoisted"]))
.help("Sets the linker mode for npm packages (isolated or hoisted)"),
  ArgDef::new("config")
    .short('c')
    .long("config")
    .action(ArgAction::Set)
    .num_args(NumArgs::Exact(1))
.help("Configure different aspects of deno including TypeScript, linting, and code formatting.\n  Typically the configuration file will be called `deno.json` or `deno.jsonc` and\n  automatically detected; in that case this flag is not necessary.\n  Docs: https://docs.deno.com/go/config"),
  ArgDef::new("no-config")
    .long("no-config")
    .set_true()
    .conflicts_with(&["config"])
.help("Disable automatic loading of the configuration file"),
  ArgDef::new("reload")
    .short('r')
    .long("reload")
    .action(ArgAction::Append)
    .num_args(NumArgs::ZeroOrMore)
    .require_equals()
    .value_delimiter(',')
.help("Reload source code cache (recompile TypeScript). With no value, reloads everything. Pass a comma-separated list of specifiers to reload only those modules; npm: reloads all npm modules; npm:chalk reloads a single npm module; jsr:@std/http/file-server,jsr:@std/assert/assert-equals reloads specific modules."),
  ArgDef::new("lock")
    .long("lock")
    .action(ArgAction::Set)
    .num_args(NumArgs::Optional)
.help("Check the specified lock file. (If value is not provided, defaults to \"./deno.lock\")"),
  ArgDef::new("no-lock").long("no-lock").set_true()
.help("Disable auto discovery of the lock file"),
  ArgDef::new("frozen-lockfile")
    .long("frozen-lockfile")
    .long_aliases(&["frozen"])
    .action(ArgAction::Set)
    .num_args(NumArgs::Optional)
    .require_equals()
    .value_parser(ValueParser::Bool)
.help("Error out if lockfile is out of date"),
  ArgDef::new("cert")
    .long("cert")
    .action(ArgAction::Set)
    .num_args(NumArgs::Exact(1))
.help("Load certificate authority from PEM encoded file"),
  ArgDef::new("unsafely-ignore-certificate-errors")
    .long("unsafely-ignore-certificate-errors")
    .action(ArgAction::Append)
    .num_args(NumArgs::ZeroOrMore)
    .require_equals()
    .value_delimiter(',')
.help("DANGER: Disables verification of TLS certificates"),
  ArgDef::new("min-dep-age")
    .long("min-dep-age")
    .long_aliases(&["minimum-dependency-age"])
    .action(ArgAction::Set)
    .num_args(NumArgs::Exact(1))
.help("(Unstable) The age in minutes, ISO-8601 duration or RFC3339 absolute timestamp (e.g. '120' for two hours, 'P2D' for two days, '2025-09-16' for cutoff date, '2025-09-16T12:00:00+00:00' for cutoff time, '0' to disable)"),
];

pub static INSPECT_ARGS: &[ArgDef] = &[
  ArgDef::new("inspect")
    .long("inspect")
    .action(ArgAction::Set)
    .num_args(NumArgs::Optional)
    .require_equals()
.help("Activate inspector on host:port [default: 127.0.0.1:9229]. Host and port are optional. Using port 0 will assign a random free port."),
  ArgDef::new("inspect-brk")
    .long("inspect-brk")
    .action(ArgAction::Set)
    .num_args(NumArgs::Optional)
    .require_equals()
.help("Activate inspector on host:port, wait for debugger to connect and break at the start of user script"),
  ArgDef::new("inspect-wait")
    .long("inspect-wait")
    .action(ArgAction::Set)
    .num_args(NumArgs::Optional)
    .require_equals()
.help("Activate inspector on host:port and wait for debugger to connect before running user code"),
  ArgDef::new("inspect-publish-uid")
    .long("inspect-publish-uid")
    .action(ArgAction::Set)
    .num_args(NumArgs::Exact(1)),
];

pub static RUNTIME_MISC_ARGS: &[ArgDef] = &[
  ArgDef::new("cached-only").long("cached-only").set_true()
.help("Require that remote dependencies are already cached"),
  ArgDef::new("location")
    .long("location")
    .action(ArgAction::Set)
    .num_args(NumArgs::Exact(1))
.help("Value of globalThis.location used by some web APIs"),
  ArgDef::new("v8-flags")
    .long("v8-flags")
    .action(ArgAction::Append)
    .num_args(NumArgs::ZeroOrMore)
    .require_equals()
    .value_delimiter(',')
.help("To see a list of all available flags use --v8-flags=--help\n  Flags can also be set via the DENO_V8_FLAGS environment variable.\n  Any flags set with this flag are appended after the DENO_V8_FLAGS environment variable"),
  ArgDef::new("seed")
    .long("seed")
    .action(ArgAction::Set)
    .num_args(NumArgs::Exact(1))
    .value_parser(ValueParser::U64)
.help("Set the random number generator seed"),
  ArgDef::new("enable-testing-features")
    .long("enable-testing-features-do-not-use")
    .long_aliases(&["enable-testing-features"])
    .set_true()
    .hidden()
.help("INTERNAL: Enable internal features used during integration testing"),
  ArgDef::new("trace-ops")
    .long("trace-ops")
    .action(ArgAction::Append)
    .num_args(NumArgs::ZeroOrMore)
    .require_equals()
    .value_delimiter(',')
    .hidden()
.help("Trace low-level op calls"),
  ArgDef::new("eszip-internal-do-not-use")
    .long("eszip-internal-do-not-use")
    .set_true()
    .hidden(),
  ArgDef::new("preload")
    .long("preload")
    .long_aliases(&["import"])
    .action(ArgAction::Append)
    .num_args(NumArgs::Exact(1))
.help("A list of files that will be executed before the main module"),
  ArgDef::new("require")
    .long("require")
    .action(ArgAction::Append)
    .num_args(NumArgs::Exact(1))
.help("A list of CommonJS modules that will be executed before the main module"),
  ArgDef::new("node-conditions")
    .long("conditions")
    .action(ArgAction::Append)
    .num_args(NumArgs::OneOrMore)
    .value_delimiter(',')
.help("Use this argument to specify custom conditions for npm package exports. You can also use DENO_CONDITIONS env var.\n\nDocs: https://docs.deno.com/go/conditional-exports"),
];

pub static CPU_PROF_ARGS: &[ArgDef] = &[
  ArgDef::new("cpu-prof").long("cpu-prof").set_true().hidden()
.help("Start the V8 CPU profiler on startup and write the profile to disk on exit. Profiles are written to the current directory by default"),
  ArgDef::new("cpu-prof-dir")
    .long("cpu-prof-dir")
    .action(ArgAction::Set)
    .num_args(NumArgs::Exact(1))
    .hidden()
.help("Directory where the V8 CPU profiles will be written. Implicitly enables --cpu-prof"),
  ArgDef::new("cpu-prof-name")
    .long("cpu-prof-name")
    .action(ArgAction::Set)
    .num_args(NumArgs::Exact(1))
    .hidden()
.help("Filename for the CPU profile (defaults to CPU.<timestamp>.<pid>.cpuprofile)"),
  ArgDef::new("cpu-prof-interval")
    .long("cpu-prof-interval")
    .action(ArgAction::Set)
    .num_args(NumArgs::Exact(1))
    .value_parser(ValueParser::U32)
    .hidden()
.help("Sampling interval in microseconds for CPU profiling (default: 1000)"),
  ArgDef::new("cpu-prof-md")
    .long("cpu-prof-md")
    .set_true()
    .hidden()
.help("Generate a human-readable markdown report alongside the CPU profile"),
  ArgDef::new("cpu-prof-flamegraph")
    .long("cpu-prof-flamegraph")
    .set_true()
    .hidden()
.help("Generate an SVG flamegraph alongside the CPU profile"),
];

// All unstable feature flags from runtime/features/gen.rs.
// Keep in sync with UNSTABLE_FEATURES.
/// The deprecated bare `--unstable` flag. Hidden everywhere except `vendor`,
/// which defines its own visible copy (clap's `UnstableArgsConfig::None` vs
/// `ResolutionOnly`).
pub static UNSTABLE_DEPRECATED_ARG: &[ArgDef] = &[
  ArgDef::new("unstable").long("unstable").set_true().hidden()
.help("The `--unstable` flag has been deprecated. Use granular `--unstable-*` flags instead\n  To view the list of individual unstable feature flags, run this command again with --help=unstable"),
];

/// The granular `--unstable-*` feature flags. clap attaches these to every
/// subcommand, so any command that omits them will reject flags the old
/// parser accepted.
pub static UNSTABLE_FEATURE_ARGS: &[ArgDef] = &[
ArgDef::new("unstable-bare-node-builtins")
    .long("unstable-bare-node-builtins")
    .set_true()
    .hidden()
.help("Enable unstable bare node builtins feature"),
  ArgDef::new("unstable-broadcast-channel")
    .long("unstable-broadcast-channel")
    .set_true()
    .hidden()
.help("Enable unstable `BroadcastChannel` API"),
  ArgDef::new("unstable-bundle")
    .long("unstable-bundle")
    .set_true()
    .hidden()
.help("Enable unstable bundle runtime API"),
  ArgDef::new("unstable-byonm")
    .long("unstable-byonm")
    .set_true()
    .hidden(),
  ArgDef::new("unstable-cron")
    .long("unstable-cron")
    .set_true()
    .hidden()
.help("Enable unstable `Deno.cron` API"),
  ArgDef::new("unstable-detect-cjs")
    .long("unstable-detect-cjs")
    .set_true()
    .hidden()
.help("Treats ambiguous .js, .jsx, .ts, .tsx files as CommonJS modules in more cases"),
  ArgDef::new("unstable-ffi")
    .long("unstable-ffi")
    .set_true()
    .hidden()
.help("Enable unstable FFI APIs"),
  ArgDef::new("unstable-fs")
    .long("unstable-fs")
    .set_true()
    .hidden()
.help("Enable unstable file system APIs"),
  ArgDef::new("unstable-http")
    .long("unstable-http")
    .set_true()
    .hidden()
.help("Enable unstable HTTP APIs"),
  ArgDef::new("unstable-kv")
    .long("unstable-kv")
    .set_true()
    .hidden()
.help("Enable unstable KV APIs"),
  ArgDef::new("unstable-lazy-dynamic-imports")
    .long("unstable-lazy-dynamic-imports")
    .set_true()
    .hidden()
.help("Lazily loads statically analyzable dynamic imports when not running with type checking. Warning: This may change the order of semver specifier resolution."),
  ArgDef::new("unstable-lockfile-v5")
    .long("unstable-lockfile-v5")
    .set_true()
    .hidden()
.help("Enable unstable lockfile v5"),
  ArgDef::new("unstable-net")
    .long("unstable-net")
    .set_true()
    .hidden()
.help("enable unstable net APIs"),
  ArgDef::new("unstable-no-legacy-abort")
    .long("unstable-no-legacy-abort")
    .set_true()
    .hidden()
.help("Enable abort signal in Deno.serve without legacy behavior. This will not abort the server when the request is handled successfully."),
  ArgDef::new("unstable-node-globals")
    .long("unstable-node-globals")
    .set_true()
    .hidden()
.help("Deprecated. Node.js `setTimeout` and `setInterval` globals are now always enabled, so this flag has no effect."),
  ArgDef::new("unstable-npm-lazy-caching")
    .long("unstable-npm-lazy-caching")
    .set_true()
    .hidden()
.help("Enable unstable lazy caching of npm dependencies, downloading them only as needed (disabled: all npm packages in package.json are installed on startup; enabled: only npm packages that are actually referenced in an import are installed"),
  ArgDef::new("unstable-otel")
    .long("unstable-otel")
    .set_true()
    .hidden()
.help("Enable unstable OpenTelemetry features"),
  ArgDef::new("unstable-process")
    .long("unstable-process")
    .set_true()
    .hidden()
.help("Enable unstable process APIs"),
  ArgDef::new("unstable-raw-imports")
    .long("unstable-raw-imports")
    .set_true()
    .hidden()
.help("Enable unstable 'bytes' imports."),
  ArgDef::new("unstable-sloppy-imports")
    .long("unstable-sloppy-imports")
    .long_aliases(&["sloppy-imports"])
    .set_true()
    .hidden()
.help("Enable unstable resolving of specifiers by extension probing, .js to .ts, and directory probing"),
  ArgDef::new("unstable-subdomain-wildcards")
    .long("unstable-subdomain-wildcards")
    .set_true()
    .hidden()
.help("Enable subdomain wildcards support for the `--allow-net` flag"),
  ArgDef::new("unstable-temporal")
    .long("unstable-temporal")
    .set_true()
    .hidden()
.help("Enable unstable Temporal API"),
  ArgDef::new("unstable-unsafe-proto")
    .long("unstable-unsafe-proto")
    .long_aliases(&["unsafe-proto"])
    .set_true()
    .hidden()
.help("Enable unsafe __proto__ support. This is a security risk."),
  ArgDef::new("unstable-vsock")
    .long("unstable-vsock")
    .set_true()
    .hidden()
.help("Enable unstable VSOCK APIs"),
  ArgDef::new("unstable-webgpu")
    .long("unstable-webgpu")
    .set_true()
    .hidden()
.help("Enable unstable WebGPU APIs"),
  ArgDef::new("unstable-worker-options")
    .long("unstable-worker-options")
    .set_true()
    .hidden()
.help("Enable unstable Web Worker APIs"),
];

/// The only permission args accepted by commands that resolve a module graph
/// but never run it (`bundle`, `cache`, `check`, `doc`, `info`). Unlike the
/// flags in `PERMISSION_ARGS` these are shown in the normal options table,
/// since those commands have no "Permission options" section.
pub static IMPORT_PERMISSION_ARGS: &[ArgDef] = &[
  ArgDef::new("allow-import")
    .short('I')
    .long("allow-import")
    .action(ArgAction::Append)
    .num_args(NumArgs::ZeroOrMore)
    .require_equals()
    .value_delimiter(',')
    .help("Allow importing from remote hosts. Optionally specify allowed IP addresses and host names, with ports as necessary. Default value: deno.land:443,jsr.io:443,esm.sh:443,raw.esm.sh:443,cdn.jsdelivr.net:443,raw.githubusercontent.com:443,gist.githubusercontent.com:443"),
  ArgDef::new("deny-import")
    .long("deny-import")
    .action(ArgAction::Append)
    .num_args(NumArgs::ZeroOrMore)
    .require_equals()
    .value_delimiter(',')
    .help("Deny importing from remote hosts. Optionally specify denied IP addresses and host names, with ports as necessary."),
];

pub static ALLOW_SCRIPTS_ARG: &[ArgDef] = &[ArgDef::new("allow-scripts")
  .long("allow-scripts")
  .action(ArgAction::Append)
  .num_args(NumArgs::ZeroOrMore)
  .require_equals()
  .value_delimiter(',')
.help("Allow running npm lifecycle scripts for the given packages\n  Note: Scripts will only be executed when using a node_modules directory (`--node-modules-dir`)")];

// Standalone lock flags for subcommands that don't pull in COMPILE_ARGS
// (which also defines these). Do not combine LOCK_ARGS with COMPILE_ARGS on the
// same command or the arg names would collide.
pub static LOCK_ARGS: &[ArgDef] = &[
  ArgDef::new("lock")
    .long("lock")
    .action(ArgAction::Set)
    .num_args(NumArgs::Optional)
.help("Check the specified lock file. (If value is not provided, defaults to \"./deno.lock\")"),
  ArgDef::new("no-lock").long("no-lock").set_true()
.help("Disable auto discovery of the lock file"),
  ArgDef::new("frozen-lockfile")
    .long("frozen-lockfile")
    .long_aliases(&["frozen"])
    .action(ArgAction::Set)
    .num_args(NumArgs::Optional)
    .require_equals()
    .value_parser(ValueParser::Bool)
.help("Error out if lockfile is out of date"),
];

// ============================================================
// Subcommand definitions
// ============================================================

// Shared between `run` and its alias-like `watch` subcommand
// (`deno watch` == `deno run --watch-hmr`).
static RUN_ARGS: &[ArgDef] = &[
  ArgDef::new("script_arg")
    .positional()
    .action(ArgAction::Set)
    .num_args(NumArgs::Exact(1))
.help("Script arg"),
  ArgDef::new("check")
    .long("check")
    .action(ArgAction::Set)
    .num_args(NumArgs::Optional)
    .require_equals()
.help("Enable type-checking. This subcommand does not type-check by default; pass --check=all to also type-check remote modules. Alternatively, use the 'deno check' subcommand."),
  ArgDef::new("watch")
    .long("watch")
    .action(ArgAction::Append)
    .num_args(NumArgs::ZeroOrMore)
    .require_equals()
    .value_delimiter(',')
.help("Watch for file changes and restart process automatically.\n  Local files from entry point module graph are watched by default.\n  Additional paths might be watched by passing them as arguments to this flag."),
  ArgDef::new("hmr")
    .long("hmr")
    .long_aliases(&["watch-hmr", "unstable-hmr"])
    .action(ArgAction::Append)
    .num_args(NumArgs::ZeroOrMore)
    .require_equals()
    .value_delimiter(',')
    .conflicts_with(&["watch"])
.help("Watch for file changes and hot-replace modules. The process restarts if hot replacement fails.\n  Local files from entry point module graph are watched by default.\n  Additional paths might be watched by passing them as arguments to this flag."),
  ArgDef::new("watch-exclude")
    .long("watch-exclude")
    .action(ArgAction::Append)
    .num_args(NumArgs::ZeroOrMore)
    .require_equals()
    .value_delimiter(',')
.help("Exclude provided files/patterns from watch mode"),
  ArgDef::new("no-clear-screen")
    .long("no-clear-screen")
    .set_true()
    .requires(&["watch"])
.help("Do not clear terminal screen when under watch mode"),
  ArgDef::new("ext")
    .long("ext")
    .action(ArgAction::Set)
    .num_args(NumArgs::Exact(1))
    .value_parser(ValueParser::Choices(EXECUTABLE_EXTS))
.help("Set content type of the supplied file"),
  ArgDef::new("env-file")
    .long("env-file")
    .long_aliases(&["env"])
    .action(ArgAction::Append)
    .num_args(NumArgs::Optional)
    .require_equals()
.help("Load environment variables from local file\n  Only the first environment variable with a given key is used.\n  Existing process environment variables are not overwritten, so if variables with the same names already exist in the environment, their values will be preserved.\n  Where multiple declarations for the same environment variable exist in your .env file, the first one encountered is applied. This is determined by the order of the files you pass as arguments."),
  ArgDef::new("no-code-cache")
    .long("no-code-cache")
    .set_true()
.help("Disable V8 code cache feature"),
  ArgDef::new("coverage")
    .long("coverage")
    .action(ArgAction::Set)
    .num_args(NumArgs::Optional)
    .require_equals()
    .conflicts_with(&["inspect", "inspect-brk", "inspect-wait"])
.help("Collect coverage profile data into DIR. If DIR is not specified, it uses 'coverage/'.\n  This option can also be set via the DENO_COVERAGE_DIR environment variable."),
  ArgDef::new("tunnel")
    .long("tunnel")
    .long_aliases(&["connected"])
    .set_true()
    .hidden()
.help("Execute tasks with a tunnel to Deno Deploy.\n\n    Create a secure connection between your local machine and Deno Deploy,\n    providing access to centralised environment variables, logging,\n    and serving from your local environment to the public internet"),
  ArgDef::new("use-env-proxy")
    .long("use-env-proxy")
    .set_true()
    .conflicts_with(&["no-use-env-proxy"])
.help("Use HTTP_PROXY, HTTPS_PROXY, and NO_PROXY for node:http/node:https"),
  ArgDef::new("no-use-env-proxy")
    .long("no-use-env-proxy")
    .set_true()
    .conflicts_with(&["use-env-proxy"])
    .hidden(),
  // Allow --allow-scripts on run (through arg_groups, but also directly)
];

static RUN_ARG_GROUPS: &[&[ArgDef]] = &[
  UNSTABLE_DEPRECATED_ARG,
  UNSTABLE_FEATURE_ARGS,
  PERMISSION_ARGS,
  COMPILE_ARGS,
  INSPECT_ARGS,
  RUNTIME_MISC_ARGS,
  CPU_PROF_ARGS,
  ALLOW_SCRIPTS_ARG,
];

pub static RUN_SUBCOMMAND: CommandDef = CommandDef {
  name: "run",
  about: "Run a JavaScript or TypeScript program, or a task",
  aliases: &[],
  args: RUN_ARGS,
  arg_groups: RUN_ARG_GROUPS,
  subcommands: &[],
  default_subcommand: None,
  trailing_var_arg: true,
  passthrough: false,
  keep_double_dash: true,
};

pub static WATCH_SUBCOMMAND: CommandDef = CommandDef {
  name: "watch",
  about: "Run a JavaScript or TypeScript program, watching for file changes and hot-replacing modules",
  aliases: &[],
  args: RUN_ARGS,
  arg_groups: RUN_ARG_GROUPS,
  subcommands: &[],
  default_subcommand: None,
  trailing_var_arg: true,
  passthrough: false,
  keep_double_dash: true,
};

pub static SERVE_SUBCOMMAND: CommandDef = CommandDef {
  name: "serve",
  about: "Run a server",
  aliases: &[],
  args: &[
    ArgDef::new("script_arg")
      .positional()
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1))
.help("Script arg"),
    ArgDef::new("port")
      .long("port")
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1))
      .value_parser(ValueParser::U16)
.help("The TCP port to serve on. Pass 0 to pick a random free port [default: 8000]"),
    ArgDef::new("host")
      .long("host")
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1))
.help("The TCP address to serve on, defaulting to 0.0.0.0 (all interfaces)"),
    ArgDef::new("open").long("open").set_true()
.help("Open the browser on the address that the server is running on."),
    ArgDef::new("tunnel")
      .short('t')
      .long("tunnel")
      .long_aliases(&["connected"])
      .set_true()
      .hidden()
.help("Execute tasks with a tunnel to Deno Deploy.\n\n    Create a secure connection between your local machine and Deno Deploy,\n    providing access to centralised environment variables, logging,\n    and serving from your local environment to the public internet"),
    ArgDef::new("parallel").long("parallel").set_true()
.help("Run multiple server workers in parallel. Parallelism defaults to the number of available CPUs or the value of the DENO_JOBS environment variable"),
    ArgDef::new("check")
      .long("check")
      .action(ArgAction::Set)
      .num_args(NumArgs::Optional)
      .require_equals()
.help("Enable type-checking. This subcommand does not type-check by default; pass --check=all to also type-check remote modules. Alternatively, use the 'deno check' subcommand."),
    ArgDef::new("watch")
      .long("watch")
      .action(ArgAction::Append)
      .num_args(NumArgs::ZeroOrMore)
      .require_equals()
      .value_delimiter(',')
.help("Watch for file changes and restart process automatically.\n  Local files from entry point module graph are watched by default.\n  Additional paths might be watched by passing them as arguments to this flag."),
    ArgDef::new("hmr")
      .long("watch-hmr")
      .long_aliases(&["unstable-hmr"])
      .action(ArgAction::Append)
      .num_args(NumArgs::ZeroOrMore)
      .require_equals()
      .value_delimiter(',')
      .conflicts_with(&["watch"])
.help("Watch for file changes and hot-replace modules. The process restarts if hot replacement fails.\n  Local files from entry point module graph are watched by default.\n  Additional paths might be watched by passing them as arguments to this flag."),
    ArgDef::new("watch-exclude")
      .long("watch-exclude")
      .action(ArgAction::Append)
      .num_args(NumArgs::ZeroOrMore)
      .require_equals()
      .value_delimiter(',')
.help("Exclude provided files/patterns from watch mode"),
    ArgDef::new("no-clear-screen")
      .long("no-clear-screen")
      .set_true()
      .requires(&["watch"])
.help("Do not clear terminal screen when under watch mode"),
    ArgDef::new("ext")
      .long("ext")
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1))
      .value_parser(ValueParser::Choices(EXECUTABLE_EXTS))
.help("Set content type of the supplied file"),
    ArgDef::new("env-file")
      .long("env-file")
      .action(ArgAction::Append)
      .num_args(NumArgs::Optional)
      .require_equals()
.help("Load environment variables from local file\n  Only the first environment variable with a given key is used.\n  Existing process environment variables are not overwritten, so if variables with the same names already exist in the environment, their values will be preserved.\n  Where multiple declarations for the same environment variable exist in your .env file, the first one encountered is applied. This is determined by the order of the files you pass as arguments."),
    ArgDef::new("no-code-cache")
      .long("no-code-cache")
      .set_true()
.help("Disable V8 code cache feature"),
  ],
  arg_groups: &[
    UNSTABLE_DEPRECATED_ARG, UNSTABLE_FEATURE_ARGS,
    PERMISSION_ARGS,
    COMPILE_ARGS,
    INSPECT_ARGS,
    RUNTIME_MISC_ARGS,
    CPU_PROF_ARGS,
    ALLOW_SCRIPTS_ARG,
  ],
  subcommands: &[],
  default_subcommand: None,
  trailing_var_arg: true,
  passthrough: false,
  keep_double_dash: true,
};

pub static EVAL_SUBCOMMAND: CommandDef = CommandDef {
  name: "eval",
  about: "Evaluate a script from the command line",
  aliases: &[],
  args: &[
    ArgDef::new("code_arg")
      .positional()
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1))
.help("Code to evaluate"),
    ArgDef::new("print").short('p').long("print").set_true()
.help("print result to stdout"),
    ArgDef::new("ext")
      .long("ext")
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1))
      .value_parser(ValueParser::Choices(EXECUTABLE_EXTS))
.help("Set content type of the supplied file"),
    ArgDef::new("env-file")
      .long("env-file")
      .long_aliases(&["env"])
      .action(ArgAction::Append)
      .num_args(NumArgs::Optional)
      .require_equals()
.help("Load environment variables from local file\n  Only the first environment variable with a given key is used.\n  Existing process environment variables are not overwritten, so if variables with the same names already exist in the environment, their values will be preserved.\n  Where multiple declarations for the same environment variable exist in your .env file, the first one encountered is applied. This is determined by the order of the files you pass as arguments."),
    ArgDef::new("check")
      .long("check")
      .action(ArgAction::Set)
      .num_args(NumArgs::Optional)
      .require_equals()
.help("Enable type-checking. This subcommand does not type-check by default; pass --check=all to also type-check remote modules. Alternatively, use the 'deno check' subcommand."),
  ],
  arg_groups: &[
    UNSTABLE_DEPRECATED_ARG, UNSTABLE_FEATURE_ARGS,
    PERMISSION_ARGS,
    COMPILE_ARGS,
    INSPECT_ARGS,
    RUNTIME_MISC_ARGS,
    CPU_PROF_ARGS,
  ],
  subcommands: &[],
  default_subcommand: None,
  trailing_var_arg: true,
  passthrough: false,
  keep_double_dash: false,
};

pub static FMT_SUBCOMMAND: CommandDef = CommandDef {
  name: "fmt",
  about: "Format source files",
  aliases: &[],
  args: &[
    ArgDef::new("files")
      .positional()
      .action(ArgAction::Append)
      .num_args(NumArgs::ZeroOrMore),
    ArgDef::new("check").long("check").set_true()
.help("Check if the source files are formatted"),
    ArgDef::new("fail-fast")
      .long("fail-fast")
      .long_aliases(&["failfast"])
      .set_true()
      .requires(&["check"])
.help("Stop checking files on first format error"),
    ArgDef::new("permit-no-files")
      .long("permit-no-files")
      .set_true()
.help("Don't return an error code if no files were found"),
    ArgDef::new("watch")
      .long("watch")
      .action(ArgAction::Append)
      .num_args(NumArgs::ZeroOrMore)
      .require_equals()
      .value_delimiter(',')
.help("Watch for file changes and restart process automatically.\n  Local files from entry point module graph are watched by default.\n  Additional paths might be watched by passing them as arguments to this flag."),
    ArgDef::new("watch-exclude")
      .long("watch-exclude")
      .action(ArgAction::Append)
      .num_args(NumArgs::ZeroOrMore)
      .require_equals()
      .value_delimiter(',')
.help("Exclude provided files/patterns from watch mode"),
    ArgDef::new("no-clear-screen")
      .long("no-clear-screen")
      .set_true()
      .requires(&["watch"])
.help("Do not clear terminal screen when under watch mode"),
    ArgDef::new("ext")
      .long("ext")
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1))
      .requires(&["files"])
      .value_parser(ValueParser::Choices(&[
        "ts", "tsx", "js", "jsx", "mts", "mjs", "cts", "cjs", "md", "json",
        "jsonc", "css", "scss", "less", "html", "xml", "svg", "svelte", "vue",
        "astro", "yml", "yaml", "ipynb", "sql", "vto", "njk",
      ]))
.help("Set content type of the supplied file"),
    ArgDef::new("ignore")
      .long("ignore")
      .action(ArgAction::Append)
      .num_args(NumArgs::ZeroOrMore)
      .require_equals()
      .value_delimiter(',')
.help("Ignore formatting particular source files"),
    ArgDef::new("use-tabs")
      .long("use-tabs")
      .long_aliases(&["options-use-tabs"])
      .action(ArgAction::Set)
      .num_args(NumArgs::Optional)
      .require_equals()
      .value_parser(ValueParser::Bool)
.help("Use tabs instead of spaces for indentation [default: false]"),
    ArgDef::new("line-width")
      .long("line-width")
      .long_aliases(&["options-line-width"])
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1))
      .value_parser(ValueParser::NonZeroU32)
.help("Define maximum line width [default: 80]"),
    ArgDef::new("indent-width")
      .long("indent-width")
      .long_aliases(&["options-indent-width"])
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1))
      .value_parser(ValueParser::NonZeroU8)
.help("Define indentation width [default: 2]"),
    ArgDef::new("single-quote")
      .long("single-quote")
      .long_aliases(&["options-single-quote"])
      .action(ArgAction::Set)
      .num_args(NumArgs::Optional)
      .require_equals()
      .value_parser(ValueParser::Bool)
.help("Use single quotes [default: false]"),
    ArgDef::new("prose-wrap")
      .long("prose-wrap")
      .long_aliases(&["options-prose-wrap"])
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1))
      .value_parser(ValueParser::Choices(&["always", "never", "preserve"]))
.help("Define how prose should be wrapped [default: always]"),
    ArgDef::new("no-semicolons")
      .long("no-semicolons")
      .long_aliases(&["options-no-semicolons"])
      .action(ArgAction::Set)
      .num_args(NumArgs::Optional)
      .require_equals()
      .value_parser(ValueParser::Bool)
.help("Don't use semicolons except where necessary [default: false]"),
    ArgDef::new("config")
      .short('c')
      .long("config")
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1))
.help("Configure different aspects of deno including TypeScript, linting, and code formatting.\n  Typically the configuration file will be called `deno.json` or `deno.jsonc` and\n  automatically detected; in that case this flag is not necessary.\n  Docs: https://docs.deno.com/go/config"),
    ArgDef::new("no-config").long("no-config").set_true()
.help("Disable automatic loading of the configuration file"),
    ArgDef::new("no-editorconfig")
      .long("no-editorconfig")
      .set_true()
.help("Don't read .editorconfig files to infer formatting options [default: false]"),
    ArgDef::new("unstable-component")
      .long("unstable-component")
      .set_true()
.help("Enable formatting Svelte, Vue, Astro and Angular files"),
    ArgDef::new("unstable-sql").long("unstable-sql").set_true()
.help("Enable formatting SQL files."),
    ArgDef::new("unstable-css")
      .long("unstable-css")
      .set_true()
      .hidden()
.help("Enable formatting CSS, SCSS and Less files"),
    ArgDef::new("unstable-html")
      .long("unstable-html")
      .set_true()
      .hidden()
.help("Enable formatting HTML files"),
    ArgDef::new("unstable-yaml")
      .long("unstable-yaml")
      .set_true()
      .hidden()
.help("Enable formatting YAML files"),
  ],
  arg_groups: &[UNSTABLE_DEPRECATED_ARG, UNSTABLE_FEATURE_ARGS],
  subcommands: &[],
  default_subcommand: None,
  trailing_var_arg: false,
  passthrough: false,
  keep_double_dash: false,
};

pub static LINT_SUBCOMMAND: CommandDef = CommandDef {
  name: "lint",
  about: "Lint source files",
  aliases: &[],
  args: &[
    ArgDef::new("files")
      .positional()
      .action(ArgAction::Append)
      .num_args(NumArgs::ZeroOrMore),
    ArgDef::new("rules").long("rules").set_true()
.help("List available rules"),
    ArgDef::new("fix").long("fix").set_true()
.help("Fix any linting errors for rules that support it"),
    ArgDef::new("rules-tags")
      .long("rules-tags")
      .action(ArgAction::Append)
      .num_args(NumArgs::ZeroOrMore)
      .require_equals()
      .value_delimiter(',')
.help("Use set of rules with a tag"),
    ArgDef::new("rules-include")
      .long("rules-include")
      .action(ArgAction::Append)
      .num_args(NumArgs::ZeroOrMore)
      .require_equals()
      .value_delimiter(',')
      .conflicts_with(&["rules"])
.help("Include lint rules"),
    ArgDef::new("rules-exclude")
      .long("rules-exclude")
      .action(ArgAction::Append)
      .num_args(NumArgs::ZeroOrMore)
      .require_equals()
      .value_delimiter(',')
      .conflicts_with(&["rules"])
.help("Exclude lint rules"),
    ArgDef::new("json").long("json").set_true()
.help("Output lint result in JSON format"),
    ArgDef::new("compact")
      .long("compact")
      .set_true()
      .conflicts_with(&["json"])
.help("Output lint result in compact format"),
    ArgDef::new("ignore")
      .long("ignore")
      .action(ArgAction::Append)
      .num_args(NumArgs::ZeroOrMore)
      .require_equals()
      .value_delimiter(',')
.help("Ignore linting particular source files"),
    ArgDef::new("watch")
      .long("watch")
      .action(ArgAction::Append)
      .num_args(NumArgs::ZeroOrMore)
      .require_equals()
      .value_delimiter(',')
.help("Watch for file changes and restart process automatically.\n  Local files from entry point module graph are watched by default.\n  Additional paths might be watched by passing them as arguments to this flag."),
    ArgDef::new("watch-exclude")
      .long("watch-exclude")
      .action(ArgAction::Append)
      .num_args(NumArgs::ZeroOrMore)
      .require_equals()
      .value_delimiter(',')
.help("Exclude provided files/patterns from watch mode"),
    ArgDef::new("no-clear-screen")
      .long("no-clear-screen")
      .set_true()
      .requires(&["watch"])
.help("Do not clear terminal screen when under watch mode"),
    ArgDef::new("permit-no-files")
      .long("permit-no-files")
      .set_true()
.help("Don't return an error code if no files were found"),
    ArgDef::new("config")
      .short('c')
      .long("config")
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1))
.help("Configure different aspects of deno including TypeScript, linting, and code formatting.\n  Typically the configuration file will be called `deno.json` or `deno.jsonc` and\n  automatically detected; in that case this flag is not necessary.\n  Docs: https://docs.deno.com/go/config"),
    ArgDef::new("no-config").long("no-config").set_true()
.help("Disable automatic loading of the configuration file"),
    ArgDef::new("ext")
      .long("ext")
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1))
      .value_parser(ValueParser::Choices(EXECUTABLE_EXTS))
.help("Specify the file extension to lint when reading from stdin.For example, use `jsx` to lint JSX files or `tsx` for TSX files.This argument is necessary because stdin input does not automatically infer the file type.Example usage: `cat file.jsx | deno lint - --ext=jsx`."),
    ArgDef::new("allow-import")
      .short('I')
      .long("allow-import")
      .action(ArgAction::Append)
      .num_args(NumArgs::ZeroOrMore)
      .require_equals()
      .value_delimiter(',')
.help("Allow importing from remote hosts. Optionally specify allowed IP addresses and host names, with ports as necessary. Default value: deno.land:443,jsr.io:443,esm.sh:443,raw.esm.sh:443,cdn.jsdelivr.net:443,raw.githubusercontent.com:443,gist.githubusercontent.com:443"),
    ArgDef::new("deny-import")
      .long("deny-import")
      .action(ArgAction::Append)
      .num_args(NumArgs::ZeroOrMore)
      .require_equals()
      .value_delimiter(',')
.help("Deny importing from remote hosts. Optionally specify denied IP addresses and host names, with ports as necessary."),
  ],
  // NOTE: lint takes only import permission args, no other permission
  // args (see issue #27336).
  arg_groups: &[UNSTABLE_DEPRECATED_ARG, UNSTABLE_FEATURE_ARGS],
  subcommands: &[],
  default_subcommand: None,
  trailing_var_arg: false,
  passthrough: false,
  keep_double_dash: false,
};

pub static TEST_SUBCOMMAND: CommandDef = CommandDef {
  name: "test",
  about: "Run tests",
  aliases: &[],
  args: &[
    ArgDef::new("files")
      .positional()
      .action(ArgAction::Append)
      .num_args(NumArgs::ZeroOrMore)
.help("List of file names to run"),
    ArgDef::new("doc").long("doc").set_true()
.help("Evaluate code blocks in JSDoc and Markdown"),
    ArgDef::new("no-run").long("no-run").set_true()
.help("Cache test modules, but don't run tests"),
    ArgDef::new("coverage")
      .long("coverage")
      .action(ArgAction::Set)
      .num_args(NumArgs::Optional)
      .require_equals()
      .conflicts_with(&["inspect", "inspect-brk", "inspect-wait"])
.help("Collect coverage profile data into DIR. If DIR is not specified, it uses 'coverage/'.\n  This option can also be set via the DENO_COVERAGE_DIR environment variable."),
    ArgDef::new("clean").long("clean").set_true()
.help("Empty the temporary coverage profile data directory before running tests.\n  Note: running multiple `deno test --clean` calls in series or parallel for the same coverage directory may cause race conditions."),
    ArgDef::new("fail-fast")
      .long("fail-fast")
      .action(ArgAction::Set)
      .num_args(NumArgs::Optional)
      .require_equals()
      .value_parser(ValueParser::NonZeroUsize)
.help("Stop after N errors. Defaults to stopping after first failure"),
    ArgDef::new("filter")
      .long("filter")
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1))
.help("Run tests with this string or regexp pattern in the test name"),
    ArgDef::new("shuffle")
      .long("shuffle")
      .action(ArgAction::Set)
      .num_args(NumArgs::Optional)
      .require_equals()
      .value_parser(ValueParser::U64)
.help("Shuffle the order in which the tests are run"),
    ArgDef::new("parallel").long("parallel").set_true()
.help("Run test modules in parallel. Parallelism defaults to the number of available CPUs or the value of the DENO_JOBS environment variable"),
    ArgDef::new("sanitize-ops").long("sanitize-ops").set_true()
.help("Enable the ops sanitizer, which ensures that all async ops started in a test are completed before the test ends"),
    ArgDef::new("sanitize-resources")
      .long("sanitize-resources")
      .set_true()
.help("Enable the resources sanitizer, which ensures that all resources opened in a test are closed before the test ends"),
    ArgDef::new("coverage-threshold")
      .long("coverage-threshold")
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1))
      .require_equals()
      .requires(&["coverage"])
      .value_parser(ValueParser::U32Range(0, 100))
.help("Fail if coverage is below this percentage (0-100). Requires --coverage"),
    ArgDef::new("update-snapshots")
      .short('u')
      .long("update-snapshots")
      .set_true()
.help("Update snapshots created with `t.assertSnapshot()` instead of failing when they do not match"),
    ArgDef::new("trace-leaks")
      .long("trace-leaks")
      .set_true()
      .hidden()
.help("Enable tracing of leaks. Useful when debugging leaking ops in test, but impacts test execution time"),
    ArgDef::new("watch")
      .long("watch")
      .action(ArgAction::Append)
      .num_args(NumArgs::ZeroOrMore)
      .require_equals()
      .value_delimiter(',')
      .conflicts_with(&["no-run", "coverage"])
.help("Watch for file changes and restart process automatically.\n  Local files from entry point module graph are watched by default.\n  Additional paths might be watched by passing them as arguments to this flag."),
    ArgDef::new("watch-exclude")
      .long("watch-exclude")
      .action(ArgAction::Append)
      .num_args(NumArgs::ZeroOrMore)
      .require_equals()
      .value_delimiter(',')
.help("Exclude provided files/patterns from watch mode"),
    ArgDef::new("no-clear-screen")
      .long("no-clear-screen")
      .set_true()
      .requires(&["watch"])
.help("Do not clear terminal screen when under watch mode"),
    ArgDef::new("reporter")
      .long("reporter")
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1))
      .value_parser(ValueParser::Choices(&["pretty", "dot", "junit", "tap"]))
.help("Select reporter to use. Default to 'pretty'"),
    ArgDef::new("junit-path")
      .long("junit-path")
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1))
.help("Write a JUnit XML test report to PATH. Use '-' to write to stdout which is the default when PATH is not provided"),
    ArgDef::new("hide-stacktraces")
      .long("hide-stacktraces")
      .set_true()
.help("Hide stack traces for errors in failure test results."),
    ArgDef::new("retry")
      .long("retry")
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1))
      .value_parser(ValueParser::U32)
.help("Re-run failing tests up to NUMBER times. A test passes if any attempt passes. Tests that set their own `retry` option take precedence"),
    ArgDef::new("repeats")
      .long("repeats")
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1))
      .value_parser(ValueParser::U32)
.help("Run each test NUMBER additional times. Every repetition must pass. Tests that set their own `repeats` option take precedence"),
    ArgDef::new("shard")
      .long("shard")
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1))
      .require_equals()
.help("Run only the test files for shard INDEX of COUNT, e.g. --shard=2/3.\n  The discovered test files are sorted and split into COUNT consecutive groups; INDEX is 1-based. Useful for splitting a run across machines."),
    ArgDef::new("changed")
      .long("changed")
      .action(ArgAction::Set)
      .num_args(NumArgs::Optional)
      .require_equals()
      .conflicts_with(&["watch"])
.help("Run only test modules affected by files changed in git.\n  With no value, uses uncommitted changes (staged, unstaged and untracked).\n  Pass a git ref to compare against, e.g. --changed=main or --changed=HEAD~1."),
    ArgDef::new("related")
      .long("related")
      .action(ArgAction::Append)
      .num_args(NumArgs::Exact(1))
      .require_equals()
      .conflicts_with(&["watch"])
.help("Run only test modules that depend on the given source files"),
    ArgDef::new("coverage-raw-data-only")
      .long("coverage-raw-data-only")
      .set_true()
.help("Only collect raw coverage data, without generating a report"),
    ArgDef::new("ignore")
      .long("ignore")
      .action(ArgAction::Append)
      .num_args(NumArgs::ZeroOrMore)
      .require_equals()
      .value_delimiter(',')
.help("Ignore files"),
    ArgDef::new("env-file")
      .long("env-file")
      .action(ArgAction::Append)
      .num_args(NumArgs::Optional)
      .require_equals()
.help("Load environment variables from local file\n  Only the first environment variable with a given key is used.\n  Existing process environment variables are not overwritten, so if variables with the same names already exist in the environment, their values will be preserved.\n  Where multiple declarations for the same environment variable exist in your .env file, the first one encountered is applied. This is determined by the order of the files you pass as arguments."),
    ArgDef::new("permit-no-files")
      .long("permit-no-files")
      .set_true()
.help("Don't return an error code if no files were found"),
    ArgDef::new("ext")
      .long("ext")
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1))
      .value_parser(ValueParser::Choices(EXECUTABLE_EXTS))
.help("Set content type of the supplied file"),
    ArgDef::new("check")
      .long("check")
      .action(ArgAction::Set)
      .num_args(NumArgs::Optional)
      .require_equals()
.help("Set type-checking behavior. This subcommand type-checks local modules by default, so passing --check is redundant; pass --check=all to also type-check remote modules. Alternatively, use the 'deno check' subcommand."),
  ],
  arg_groups: &[
    UNSTABLE_DEPRECATED_ARG, UNSTABLE_FEATURE_ARGS,
    PERMISSION_ARGS,
    COMPILE_ARGS,
    INSPECT_ARGS,
    RUNTIME_MISC_ARGS,
    ALLOW_SCRIPTS_ARG,
  ],
  subcommands: &[],
  default_subcommand: None,
  trailing_var_arg: true,
  passthrough: false,
  keep_double_dash: false,
};

pub static UPGRADE_SUBCOMMAND: CommandDef = CommandDef {
  name: "upgrade",
  about: "Upgrade deno executable to given version",
  aliases: &[],
  args: &[
    ArgDef::new("dry-run").long("dry-run").set_true()
.help("Perform all checks without replacing old exe"),
    ArgDef::new("force").short('f').long("force").set_true()
.help("Replace current exe even if not out-of-date"),
    ArgDef::new("canary").long("canary").set_true()
.help("Upgrade to canary builds"),
    ArgDef::new("release-candidate")
      .long("release-candidate")
      .long_aliases(&["rc"])
      .set_true()
      .conflicts_with(&["canary", "version"])
.help("Upgrade to a release candidate"),
    ArgDef::new("version")
      .long("version")
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1))
.help("The version to upgrade to"),
    ArgDef::new("output")
      .short('o')
      .long("output")
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1))
.help("The path to output the updated version to"),
    ArgDef::new("cert")
      .long("cert")
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1))
.help("Load certificate authority from PEM encoded file"),
    ArgDef::new("unsafely-ignore-certificate-errors")
      .long("unsafely-ignore-certificate-errors")
      .action(ArgAction::Append)
      .num_args(NumArgs::ZeroOrMore)
      .value_delimiter(',')
.help("DANGER: Disables verification of TLS certificates"),
    ArgDef::new("version-or-hash-or-channel")
      .positional()
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1))
.help("Version (v1.46.0), channel (alpha, beta, rc, canary), commit hash (9bc2dd29ad6ba334fd57a20114e367d3c04763d4), or pr 12345 to install from a PR"),
    ArgDef::new("pr-number-positional")
      .positional()
      .action(ArgAction::Set)
      .num_args(NumArgs::Optional),
    ArgDef::new("pr")
      .long("pr")
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1)),
    ArgDef::new("checksum")
      .long("checksum")
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1))
.help("Verify the downloaded archive against the provided SHA256 checksum"),
    ArgDef::new("branch")
      .long("branch")
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1)),
    ArgDef::new("no-delta").long("no-delta").set_true()
.help("Disable delta updates and always download the full archive"),
  ],
  arg_groups: &[UNSTABLE_DEPRECATED_ARG, UNSTABLE_FEATURE_ARGS],
  subcommands: &[],
  default_subcommand: None,
  trailing_var_arg: false,
  passthrough: false,
  keep_double_dash: false,
};

pub static CACHE_SUBCOMMAND: CommandDef = CommandDef {
  name: "cache",
  about: "Cache the dependencies",
  aliases: &[],
  args: &[
    ArgDef::new("files")
      .positional()
      .action(ArgAction::Append)
      .num_args(NumArgs::OneOrMore),
    ArgDef::new("check")
      .long("check")
      .action(ArgAction::Set)
      .num_args(NumArgs::Optional)
      .require_equals()
.help("Enable type-checking. This subcommand does not type-check by default; pass --check=all to also type-check remote modules. Alternatively, use the 'deno check' subcommand."),
    ArgDef::new("ext")
      .long("ext")
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1))
      .value_parser(ValueParser::Choices(EXECUTABLE_EXTS)),
    ArgDef::new("env-file")
      .long("env-file")
      .action(ArgAction::Append)
      .num_args(NumArgs::Optional)
      .require_equals()
.help("Load environment variables from local file\n  Only the first environment variable with a given key is used.\n  Existing process environment variables are not overwritten, so if variables with the same names already exist in the environment, their values will be preserved.\n  Where multiple declarations for the same environment variable exist in your .env file, the first one encountered is applied. This is determined by the order of the files you pass as arguments."),
  ],
  arg_groups: &[
    UNSTABLE_DEPRECATED_ARG, UNSTABLE_FEATURE_ARGS,
    COMPILE_ARGS,
    INSPECT_ARGS,
    IMPORT_PERMISSION_ARGS,
    ALLOW_SCRIPTS_ARG,
  ],
  subcommands: &[],
  default_subcommand: None,
  trailing_var_arg: false,
  passthrough: false,
  keep_double_dash: false,
};

pub static CHECK_SUBCOMMAND: CommandDef = CommandDef {
  name: "check",
  about: "Type-check the dependencies",
  aliases: &[],
  args: &[
    ArgDef::new("files")
      .positional()
      .action(ArgAction::Append)
      .num_args(NumArgs::OneOrMore),
    ArgDef::new("all")
      .long("all")
      .set_true()
      .conflicts_with(&["no-remote"])
.help("Type-check all code, including remote modules and npm packages"),
    ArgDef::new("remote")
      .long("remote")
      .set_true()
      .conflicts_with(&["no-remote"])
      .hidden()
.help("Type-check all modules, including remote ones"),
    ArgDef::new("doc").long("doc").set_true()
.help("Type-check code blocks in JSDoc as well as actual code"),
    ArgDef::new("doc-only")
      .long("doc-only")
      .set_true()
      .conflicts_with(&["doc"])
.help("Type-check code blocks in JSDoc and Markdown only"),
    ArgDef::new("check-js").long("check-js").set_true().hidden()
.help("Enable type-checking of JavaScript files (equivalent to `compilerOptions.checkJs: true`)"),
    ArgDef::new("desktop").long("desktop").set_true()
.help("Type-check using the type definitions for `deno desktop`"),
    ArgDef::new("no-code-cache")
      .long("no-code-cache")
      .set_true()
.help("Disable V8 code cache feature"),
    ArgDef::new("watch")
      .long("watch")
      .action(ArgAction::Append)
      .num_args(NumArgs::ZeroOrMore)
      .require_equals()
      .value_delimiter(',')
.help("Watch for file changes and restart process automatically.\n  Only local files from entry point module graph are watched."),
    ArgDef::new("watch-exclude")
      .long("watch-exclude")
      .action(ArgAction::Append)
      .num_args(NumArgs::ZeroOrMore)
      .require_equals()
      .value_delimiter(',')
.help("Exclude provided files/patterns from watch mode"),
    ArgDef::new("no-clear-screen")
      .long("no-clear-screen")
      .set_true()
      .requires(&["watch"])
.help("Do not clear terminal screen when under watch mode"),
  ],
  arg_groups: &[
    UNSTABLE_DEPRECATED_ARG, UNSTABLE_FEATURE_ARGS,
    COMPILE_ARGS,
    RUNTIME_MISC_ARGS,
    IMPORT_PERMISSION_ARGS,
  ],
  subcommands: &[],
  default_subcommand: None,
  trailing_var_arg: false,
  passthrough: false,
  keep_double_dash: false,
};

pub static INFO_SUBCOMMAND: CommandDef = CommandDef {
  name: "info",
  about: "Show info about cache or info related to source file",
  aliases: &[],
  args: &[
    ArgDef::new("file")
      .positional()
      .action(ArgAction::Set)
      .num_args(NumArgs::Optional),
    ArgDef::new("json").long("json").set_true()
.help("UNSTABLE: Outputs the information in JSON format"),
    ArgDef::new("location")
      .long("location")
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1))
      .conflicts_with(&["file"])
.help("Show files used for origin bound APIs like the Web Storage API when running a script with --location=<HREF>"),
  ],
  arg_groups: &[UNSTABLE_DEPRECATED_ARG, UNSTABLE_FEATURE_ARGS, COMPILE_ARGS, IMPORT_PERMISSION_ARGS],
  subcommands: &[],
  default_subcommand: None,
  trailing_var_arg: false,
  passthrough: false,
  keep_double_dash: false,
};

pub static DOC_SUBCOMMAND: CommandDef = CommandDef {
  name: "doc",
  about: "Generate and show documentation for a module or built-ins",
  aliases: &[],
  args: &[
    ArgDef::new("source_file")
      .positional()
      .action(ArgAction::Append)
      .num_args(NumArgs::ZeroOrMore),
    ArgDef::new("json").long("json").set_true()
.help("Output documentation in JSON format"),
    ArgDef::new("private").long("private").set_true()
.help("Output private documentation"),
    ArgDef::new("lint").long("lint").set_true()
.help("Output documentation diagnostics."),
    ArgDef::new("html")
      .long("html")
      .set_true()
      .conflicts_with(&["json"])
.help("Output documentation in HTML format"),
    ArgDef::new("name")
      .long("name")
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1))
.help("The name that will be used in the docs (ie for breadcrumbs)"),
    ArgDef::new("output")
      .long("output")
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1))
.help("Directory for HTML documentation output"),
    ArgDef::new("category-docs")
      .long("category-docs")
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1))
      .requires(&["html"])
.help("Path to a JSON file keyed by category and an optional value of a markdown doc"),
    ArgDef::new("symbol-redirect-map")
      .long("symbol-redirect-map")
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1))
      .requires(&["html"])
.help("Path to a JSON file keyed by file, with an inner map of symbol to an external link"),
    ArgDef::new("default-symbol-map")
      .long("default-symbol-map")
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1))
      .requires(&["html"])
.help("Uses the provided mapping of default name to wanted name for usage blocks"),
    ArgDef::new("strip-trailing-html")
      .long("strip-trailing-html")
      .set_true()
      .requires(&["html"])
.help("Remove trailing .html from various links. Will still generate files with a .html extension"),
    ArgDef::new("filter")
      .long("filter")
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1))
      .conflicts_with(&["json", "lint", "html"])
.help("Dot separated path to symbol"),
    ArgDef::new("builtin").long("builtin").set_true(),
  ],
  arg_groups: &[UNSTABLE_DEPRECATED_ARG, UNSTABLE_FEATURE_ARGS, COMPILE_ARGS, IMPORT_PERMISSION_ARGS],
  subcommands: &[],
  default_subcommand: None,
  trailing_var_arg: false,
  passthrough: false,
  keep_double_dash: false,
};

pub static TASK_SUBCOMMAND: CommandDef = CommandDef {
  name: "task",
  about: "Run a task defined in the configuration file",
  aliases: &[],
  args: &[
    ArgDef::new("task_name")
      .positional()
      .action(ArgAction::Set)
      .num_args(NumArgs::Optional),
    ArgDef::new("cwd")
      .long("cwd")
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1))
.help("Specify the directory to run the task in"),
    ArgDef::new("config")
      .short('c')
      .long("config")
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1))
.help("Configure different aspects of deno including TypeScript, linting, and code formatting.\n  Typically the configuration file will be called `deno.json` or `deno.jsonc` and\n  automatically detected; in that case this flag is not necessary.\n  Docs: https://docs.deno.com/go/config"),
    ArgDef::new("recursive")
      .short('r')
      .long("recursive")
      .set_true()
.help("Run the task in all projects in the workspace"),
    ArgDef::new("members")
      .long("members")
      .set_true()
      .conflicts_with(&["recursive", "filter"])
.help("Run the task in all workspace members, but not in the workspace root"),
    ArgDef::new("filter")
      .short('f')
      .long("filter")
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1))
.help("Filter members of the workspace by name, implies --recursive flag"),
    ArgDef::new("eval").long("eval").set_true()
.help("Evaluate the passed value as if it was a task in a configuration file"),
    ArgDef::new("if-present").long("if-present").set_true()
.help("Exit with code 0 instead of an error when the task is not found"),
    ArgDef::new("no-prefix").long("no-prefix").set_true()
.help("Disable prefixing the output of concurrently-executing tasks with the task name"),
    ArgDef::new("jobs")
      .long("jobs")
      .short('j')
      .long_aliases(&["concurrency"])
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1))
.help("Maximum number of tasks to run concurrently.\nOverrides the DENO_JOBS environment variable; defaults to the number of\navailable CPUs. Use 1 to force sequential execution. Only affects runs\nwhere multiple tasks can run concurrently (workspace runs, or a task with\nparallelizable dependencies)"),
    ArgDef::new("env-file")
      .long("env-file")
      .long_aliases(&["env"])
      .action(ArgAction::Append)
      .num_args(NumArgs::Optional)
      .require_equals()
.help("Load environment variables from local file\n  Only the first environment variable with a given key is used.\n  Existing process environment variables are not overwritten, so if variables with the same names already exist in the environment, their values will be preserved.\n  Where multiple declarations for the same environment variable exist in your .env file, the first one encountered is applied. This is determined by the order of the files you pass as arguments."),
    ArgDef::new("node-modules-dir")
      .long("node-modules-dir")
      .action(ArgAction::Set)
      .num_args(NumArgs::Optional)
      .require_equals()
.help("Selects the node_modules directory mode for npm packages (not a path). One of: auto (create a local node_modules directory and install npm packages into it), manual (use the existing local node_modules directory, do not modify it), none (do not use a local node_modules directory; resolve npm packages from the global cache). Defaults to auto when the flag is passed without a value."),
    ArgDef::new("tunnel").long("tunnel").set_true().hidden()
.help("Execute tasks with a tunnel to Deno Deploy.\n\n    Create a secure connection between your local machine and Deno Deploy,\n    providing access to centralised environment variables, logging,\n    and serving from your local environment to the public internet"),
    ArgDef::new("lock")
      .long("lock")
      .action(ArgAction::Set)
      .num_args(NumArgs::Optional)
.help("Check the specified lock file. (If value is not provided, defaults to \"./deno.lock\")"),
    ArgDef::new("no-lock").long("no-lock").set_true()
.help("Disable auto discovery of the lock file"),
    ArgDef::new("frozen-lockfile")
      .long("frozen-lockfile")
      .long_aliases(&["frozen"])
      .action(ArgAction::Set)
      .num_args(NumArgs::Optional)
      .require_equals()
.help("Error out if lockfile is out of date"),
  ],
  arg_groups: &[UNSTABLE_DEPRECATED_ARG, UNSTABLE_FEATURE_ARGS],
  subcommands: &[],
  default_subcommand: None,
  trailing_var_arg: true,
  passthrough: false,
  keep_double_dash: true,
};

pub static BENCH_SUBCOMMAND: CommandDef = CommandDef {
  name: "bench",
  about: "Run benchmarks",
  aliases: &[],
  args: &[
    ArgDef::new("files")
      .positional()
      .action(ArgAction::Append)
      .num_args(NumArgs::ZeroOrMore)
.help("List of file names to run"),
    ArgDef::new("filter")
      .long("filter")
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1))
.help("Run benchmarks with this string or regexp pattern in the bench name"),
    ArgDef::new("json").long("json").set_true()
.help("UNSTABLE: Output benchmark result in JSON format"),
    ArgDef::new("no-run").long("no-run").set_true()
.help("Cache bench modules, but don't run benchmarks"),
    ArgDef::new("permit-no-files")
      .long("permit-no-files")
      .set_true()
.help("Don't return an error code if no files were found"),
    ArgDef::new("watch")
      .long("watch")
      .action(ArgAction::Append)
      .num_args(NumArgs::ZeroOrMore)
      .require_equals()
      .value_delimiter(',')
.help("Watch for file changes and restart process automatically.\n  Local files from entry point module graph are watched by default.\n  Additional paths might be watched by passing them as arguments to this flag."),
    ArgDef::new("watch-exclude")
      .long("watch-exclude")
      .action(ArgAction::Append)
      .num_args(NumArgs::ZeroOrMore)
      .require_equals()
      .value_delimiter(',')
.help("Exclude provided files/patterns from watch mode"),
    ArgDef::new("no-clear-screen")
      .long("no-clear-screen")
      .set_true()
      .requires(&["watch"])
.help("Do not clear terminal screen when under watch mode"),
    ArgDef::new("ignore")
      .long("ignore")
      .action(ArgAction::Append)
      .num_args(NumArgs::ZeroOrMore)
      .require_equals()
      .value_delimiter(',')
.help("Ignore files"),
    ArgDef::new("env-file")
      .long("env-file")
      .action(ArgAction::Append)
      .num_args(NumArgs::Optional)
      .require_equals()
.help("Load environment variables from local file\n  Only the first environment variable with a given key is used.\n  Existing process environment variables are not overwritten, so if variables with the same names already exist in the environment, their values will be preserved.\n  Where multiple declarations for the same environment variable exist in your .env file, the first one encountered is applied. This is determined by the order of the files you pass as arguments."),
    ArgDef::new("check")
      .long("check")
      .action(ArgAction::Set)
      .num_args(NumArgs::Optional)
      .require_equals()
.help("Set type-checking behavior. This subcommand type-checks local modules by default, so passing --check is redundant; pass --check=all to also type-check remote modules. Alternatively, use the 'deno check' subcommand."),
    ArgDef::new("ext")
      .long("ext")
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1))
      .value_parser(ValueParser::Choices(EXECUTABLE_EXTS))
.help("Set content type of the supplied file"),
  ],
  arg_groups: &[
    UNSTABLE_DEPRECATED_ARG, UNSTABLE_FEATURE_ARGS,
    PERMISSION_ARGS,
    COMPILE_ARGS,
    INSPECT_ARGS,
    RUNTIME_MISC_ARGS,
  ],
  subcommands: &[],
  default_subcommand: None,
  trailing_var_arg: true,
  passthrough: false,
  keep_double_dash: false,
};

pub static COMPILE_SUBCOMMAND: CommandDef = CommandDef {
  name: "compile",
  about: "Compile the script into a self contained executable",
  aliases: &[],
  args: &[
    ArgDef::new("source_file")
      .positional()
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1)),
    ArgDef::new("output")
      .short('o')
      .long("output")
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1))
.help("Output file (defaults to $PWD/<inferred-name>)"),
    ArgDef::new("target")
      .long("target")
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1))
      .value_parser(ValueParser::Choices(SUPPORTED_OS))
.help("Target OS architecture"),
    ArgDef::new("engine")
      .long("engine")
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1))
      .value_parser(ValueParser::Choices(&["v8", "quickjs"]))
      .help(
        "JS engine the compiled binary runs on (quickjs is smaller and experimental, and does not receive the same security updates as v8)",
      ),
    ArgDef::new("no-terminal").long("no-terminal").set_true()
.help("Hide terminal on Windows"),
    ArgDef::new("icon")
      .long("icon")
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1))
.help("Set the icon of the executable on Windows (.ico)"),
    ArgDef::new("include")
      .long("include")
      .action(ArgAction::Append)
      .num_args(NumArgs::Exact(1))
.help("Includes an additional module or file/directory in the compiled executable.\n  Use this flag if a dynamically imported module or a web worker main module\n  fails to load in the executable or to embed a file or directory in the executable.\n  This flag can be passed multiple times, to include multiple additional modules."),
    ArgDef::new("exclude")
      .long("exclude")
      .action(ArgAction::Append)
      .num_args(NumArgs::Exact(1))
.help("Excludes a file/directory in the compiled executable.\n  Use this flag to exclude a specific file or directory within the included files.\n  For example, to exclude a certain folder in the bundled node_modules directory."),
    ArgDef::new("env-file")
      .long("env-file")
      .long_aliases(&["env"])
      .action(ArgAction::Append)
      .num_args(NumArgs::Optional)
      .require_equals()
.help("Load environment variables from local file\n  Only the first environment variable with a given key is used.\n  Existing process environment variables are not overwritten, so if variables with the same names already exist in the environment, their values will be preserved.\n  Where multiple declarations for the same environment variable exist in your .env file, the first one encountered is applied. This is determined by the order of the files you pass as arguments."),
    ArgDef::new("no-code-cache")
      .long("no-code-cache")
      .set_true()
      .conflicts_with(&["include-code-cache"])
.help("Disable V8 code cache feature"),
    ArgDef::new("include-code-cache")
      .long("include-code-cache")
      .set_true()
      .conflicts_with(&["no-code-cache"])
.help("Generate and embed V8 code cache in the executable.
  Improves first-start performance at the cost of a larger binary.
  Only supported for native, non-desktop compilation."),
    ArgDef::new("ext")
      .long("ext")
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1))
      .value_parser(ValueParser::Choices(EXECUTABLE_EXTS))
.help("Set content type of the supplied file"),
    ArgDef::new("self-extracting")
      .long("self-extracting")
      .set_true()
.help("Create a self-extracting binary that extracts the embedded file system to disk on first run and then runs from there"),
    ArgDef::new("bundle").long("bundle").set_true()
.help("Experimental. Bundle the entrypoint with esbuild before embedding, instead of shipping the whole node_modules tree.\n  Produces a smaller binary with faster startup, at the cost of dropping dynamic require/import patterns that can't be statically traced."),
    ArgDef::new("minify")
      .long("minify")
      .set_true()
      .requires(&["bundle"])
.help("Experimental. Minify the bundled output. Only meaningful with --bundle.\n  Reduces both the embedded bundle size and runtime memory use, at the cost of less readable stack traces."),
    ArgDef::new("app-name")
      .long("app-name")
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1))
.help("Stable identity for the compiled app.\n  Determines where origin-bound storage such as the default `Deno.openKv()`,\n  `localStorage` and `caches` is persisted (under the platform's app data directory).\n  Defaults to the output file name. Set this to keep storage stable across renames."),
    ArgDef::new("exclude-unused-npm")
      .long("exclude-unused-npm")
      .set_true()
.help("Embed only the npm packages reachable from the module graph (managed npm; no node_modules directory).\n  Without this flag the full managed npm snapshot from the lockfile / package.json is embedded.\n  Reduces binary size when the lockfile contains packages the entrypoint does not import.\n  Skips packages that are only reached through non-statically-analyzable dynamic imports;\n  pass those with --include npm:<pkg> if needed."),
    ArgDef::new("check")
      .long("check")
      .action(ArgAction::Set)
      .num_args(NumArgs::Optional)
      .require_equals()
.help("Set type-checking behavior. This subcommand type-checks local modules by default, so passing --check is redundant; pass --check=all to also type-check remote modules. Alternatively, use the 'deno check' subcommand."),
    ArgDef::new("watch")
      .long("watch")
      .action(ArgAction::Append)
      .num_args(NumArgs::ZeroOrMore)
      .require_equals()
      .value_delimiter(',')
.help("Watch for file changes and restart process automatically.\n  Only local files from entry point module graph are watched."),
    ArgDef::new("watch-exclude")
      .long("watch-exclude")
      .action(ArgAction::Append)
      .num_args(NumArgs::ZeroOrMore)
      .require_equals()
      .value_delimiter(',')
.help("Exclude provided files/patterns from watch mode"),
    ArgDef::new("no-clear-screen")
      .long("no-clear-screen")
      .set_true()
      .requires(&["watch"])
.help("Do not clear terminal screen when under watch mode"),
  ],
  arg_groups: &[
    UNSTABLE_DEPRECATED_ARG, UNSTABLE_FEATURE_ARGS,
    PERMISSION_ARGS,
    COMPILE_ARGS,
    INSPECT_ARGS,
    RUNTIME_MISC_ARGS,
  ],
  subcommands: &[],
  default_subcommand: None,
  trailing_var_arg: true,
  passthrough: false,
  keep_double_dash: true,
};

pub static COVERAGE_SUBCOMMAND: CommandDef = CommandDef {
  name: "coverage",
  about: "Print coverage reports",
  aliases: &[],
  args: &[
    ArgDef::new("files")
      .positional()
      .action(ArgAction::Append)
      .num_args(NumArgs::ZeroOrMore),
    ArgDef::new("ignore")
      .long("ignore")
      .action(ArgAction::Append)
      .num_args(NumArgs::ZeroOrMore)
      .require_equals()
      .value_delimiter(',')
.help("Ignore coverage files"),
    ArgDef::new("include")
      .long("include")
      .action(ArgAction::Append)
      .num_args(NumArgs::ZeroOrMore)
      .require_equals()
      .value_delimiter(',')
.help("Include source files in the report"),
    ArgDef::new("exclude")
      .long("exclude")
      .action(ArgAction::Append)
      .num_args(NumArgs::ZeroOrMore)
      .require_equals()
      .value_delimiter(',')
.help("Exclude source files from the report"),
    ArgDef::new("lcov").long("lcov").set_true()
.help("Output coverage report in lcov format"),
    ArgDef::new("html").long("html").set_true()
.help("Output coverage report in HTML format in the given directory"),
    ArgDef::new("detailed").long("detailed").set_true()
.help("Output coverage report in detailed format in the terminal"),
    ArgDef::new("threshold")
      .long("threshold")
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1))
      .require_equals()
.help("Fail if coverage is below this percentage (0-100), applied to line, branch, and function coverage.\n  Per-metric thresholds can be set in deno.json under \"coverage\": { \"thresholds\": { ... } }. The flag takes precedence."),
    ArgDef::new("output")
      .long("output")
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1))
      .require_equals()
      .requires(&["lcov"])
.help("Exports the coverage report in lcov format to the given file.\n  If no --output arg is specified then the report is written to stdout."),
  ],
  arg_groups: &[UNSTABLE_DEPRECATED_ARG, UNSTABLE_FEATURE_ARGS],
  subcommands: &[],
  default_subcommand: None,
  trailing_var_arg: false,
  passthrough: false,
  keep_double_dash: false,
};

pub static REPL_SUBCOMMAND: CommandDef = CommandDef {
  name: "repl",
  about: "Start an interactive Read-Eval-Print Loop (REPL) for Deno",
  aliases: &[],
  args: &[
    ArgDef::new("eval")
      .long("eval")
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1))
.help("Evaluates the provided code when the REPL starts"),
    ArgDef::new("eval-file")
      .long("eval-file")
      .action(ArgAction::Append)
      .num_args(NumArgs::OneOrMore)
      .value_delimiter(',')
.help("Evaluates the provided file(s) as scripts when the REPL starts. Accepts file paths and URLs"),
    ArgDef::new("json").long("json").set_true(),
    ArgDef::new("env-file")
      .long("env-file")
      .long_aliases(&["env"])
      .action(ArgAction::Append)
      .num_args(NumArgs::Optional)
      .require_equals()
.help("Load environment variables from local file\n  Only the first environment variable with a given key is used.\n  Existing process environment variables are not overwritten, so if variables with the same names already exist in the environment, their values will be preserved.\n  Where multiple declarations for the same environment variable exist in your .env file, the first one encountered is applied. This is determined by the order of the files you pass as arguments."),
  ],
  arg_groups: &[
    UNSTABLE_DEPRECATED_ARG, UNSTABLE_FEATURE_ARGS,
    PERMISSION_ARGS,
    COMPILE_ARGS,
    INSPECT_ARGS,
    RUNTIME_MISC_ARGS,
  ],
  subcommands: &[],
  default_subcommand: None,
  trailing_var_arg: false,
  passthrough: false,
  keep_double_dash: false,
};

pub static INSTALL_SUBCOMMAND: CommandDef = CommandDef {
  name: "install",
  about: "Installs dependencies either in the local project or globally to a bin directory",
  aliases: &["i"],
  args: &[
    ArgDef::new("cmd")
      .positional()
      .action(ArgAction::Append)
      .num_args(NumArgs::ZeroOrMore),
    ArgDef::new("global").short('g').long("global").set_true()
.help("Install a package or script as a globally available executable"),
    ArgDef::new("name")
      .short('n')
      .long("name")
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1))
      .requires(&["global"])
.help("Executable file name"),
    ArgDef::new("root")
      .long("root")
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1))
      .requires(&["global"])
.help("Installation root"),
    ArgDef::new("force")
      .short('f')
      .long("force")
      .set_true()
      .requires(&["global"])
.help("Forcefully overwrite existing installation"),
    ArgDef::new("dev")
      .short('D')
      .long("dev")
      .set_true()
      .conflicts_with(&["entrypoint", "global"])
.help("Add the package as a dev dependency (under `devDependencies`). Note: this only applies when adding to a `package.json` file."),
    ArgDef::new("save-optional")
      .short('O')
      .long("save-optional")
      .set_true()
      .conflicts_with(&["entrypoint", "global", "dev"])
.help("Add the package as an optional dependency (under `optionalDependencies`). Note: this only applies when adding to a `package.json` file."),
    ArgDef::new("no-save")
      .long("no-save")
      .set_true()
      .conflicts_with(&["entrypoint", "global", "dev", "save-optional"])
.help("Install the package(s) without adding them to the configuration file."),
    ArgDef::new("prod")
      .long("prod")
      .long_aliases(&["production"])
      .set_true()
      .conflicts_with(&["global", "dev"])
.help("Only install production dependencies (excludes devDependencies)"),
    ArgDef::new("skip-types")
      .long("skip-types")
      .set_true()
      .requires(&["prod"])
.help("Exclude @types/* packages from installation.\nBe careful, as it uses a name-based heuristic and may skip packages that ship runtime code."),
    ArgDef::new("entrypoint")
      .short('e')
      .long("entrypoint")
      .set_true()
      .conflicts_with(&["global"])
.help("Install dependents of the specified entrypoint(s)"),
    ArgDef::new("compile")
      .long("compile")
      .set_true()
      .requires(&["global"])
.help("Install the script as a compiled executable"),
    ArgDef::new("lockfile-only")
      .long("lockfile-only")
      .set_true()
      .conflicts_with(&["global"])
.help("Install only updating the lockfile"),
    ArgDef::new("npm").long("npm").set_true().conflicts_with(&[
      "jsr",
      "entrypoint",
      "global",
    ])
.help("assume unprefixed package names are npm packages (default)"),
    ArgDef::new("jsr")
      .long("jsr")
      .set_true()
      .conflicts_with(&["entrypoint", "global"])
.help("assume unprefixed package names are jsr packages"),
    ArgDef::new("save-exact")
      .long("save-exact")
      .long_aliases(&["exact"])
      .set_true()
      .conflicts_with(&["entrypoint", "global"])
.help("Save exact version without the caret (^)"),
    ArgDef::new("unscoped")
      .long("unscoped")
      .set_true()
      .conflicts_with(&["entrypoint", "global"])
.help("Use the package name without its scope as the alias (ex. `jsr:@david/jsonc-morph` is added as `jsonc-morph`). Packages given an explicit alias are unaffected."),
    ArgDef::new("package-json")
      .long("package-json")
      .set_true()
      .conflicts_with(&["entrypoint", "global"])
.help("Force using package.json for dependency management instead of deno.json"),
    ArgDef::new("os")
      .long("os")
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1))
      .conflicts_with(&["global"])
      .value_parser(ValueParser::Choices(&[
        "aix", "android", "darwin", "freebsd", "linux", "openbsd", "sunos",
        "win32",
      ]))
.help("Target OS for npm package installation (e.g., linux, darwin, win32)"),
    ArgDef::new("arch")
      .long("arch")
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1))
      .conflicts_with(&["global"])
      .value_parser(ValueParser::Choices(&[
        "arm", "arm64", "ia32", "mips", "mipsel", "ppc", "ppc64", "s390",
        "s390x", "x64",
      ]))
.help("Target architecture for npm package installation (e.g., x64, arm64)"),
    ArgDef::new("env-file")
      .long("env-file")
      .long_aliases(&["env"])
      .action(ArgAction::Append)
      .num_args(NumArgs::Optional)
      .require_equals()
.help("Load environment variables from local file\n  Only the first environment variable with a given key is used.\n  Existing process environment variables are not overwritten, so if variables with the same names already exist in the environment, their values will be preserved.\n  Where multiple declarations for the same environment variable exist in your .env file, the first one encountered is applied. This is determined by the order of the files you pass as arguments."),
    ArgDef::new("check")
      .long("check")
      .action(ArgAction::Set)
      .num_args(NumArgs::Optional)
      .require_equals()
.help("Set type-checking behavior. This subcommand type-checks local modules by default, so passing --check is redundant; pass --check=all to also type-check remote modules. Alternatively, use the 'deno check' subcommand."),
  ],
  arg_groups: &[
    UNSTABLE_DEPRECATED_ARG, UNSTABLE_FEATURE_ARGS,
    PERMISSION_ARGS,
    COMPILE_ARGS,
    INSPECT_ARGS,
    RUNTIME_MISC_ARGS,
    ALLOW_SCRIPTS_ARG,
  ],
  subcommands: &[],
  default_subcommand: None,
  trailing_var_arg: true,
  passthrough: false,
  keep_double_dash: false,
};

pub static UNINSTALL_SUBCOMMAND: CommandDef = CommandDef {
  name: "uninstall",
  about: "Uninstalls a dependency or an executable script in the installation root's bin directory",
  aliases: &[],
  args: &[
    ArgDef::new("packages")
      .positional()
      .action(ArgAction::Append)
      .num_args(NumArgs::OneOrMore)
      .required(),
    ArgDef::new("global").short('g').long("global").set_true()
.help("Remove globally installed packages or modules"),
    ArgDef::new("root")
      .long("root")
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1))
      .requires(&["global"])
.help("Installation root"),
    ArgDef::new("lockfile-only")
      .long("lockfile-only")
      .set_true()
.help("Install only updating the lockfile"),
    ArgDef::new("package-json")
      .long("package-json")
      .set_true()
      .conflicts_with(&["global"])
.help("Force using package.json for dependency management instead of deno.json"),
  ],
  arg_groups: &[UNSTABLE_DEPRECATED_ARG, UNSTABLE_FEATURE_ARGS, COMPILE_ARGS],
  subcommands: &[],
  default_subcommand: None,
  trailing_var_arg: false,
  passthrough: false,
  keep_double_dash: false,
};

pub static TYPES_SUBCOMMAND: CommandDef = CommandDef {
  name: "types",
  about: "Print runtime TypeScript declarations",
  aliases: &[],
  args: &[],
  arg_groups: &[UNSTABLE_DEPRECATED_ARG, UNSTABLE_FEATURE_ARGS],
  subcommands: &[],
  default_subcommand: None,
  trailing_var_arg: false,
  passthrough: false,
  keep_double_dash: false,
};

pub static COMPLETIONS_SUBCOMMAND: CommandDef = CommandDef {
  name: "completions",
  about: "Generate shell completions",
  aliases: &[],
  args: &[
    ArgDef::new("shell")
      .positional()
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1))
      .value_parser(ValueParser::Choices(&[
        "bash",
        "fish",
        "powershell",
        "zsh",
        "fig",
      ])),
    ArgDef::new("dynamic").long("dynamic").set_true()
.help("Generate dynamic completions for the given shell (unstable), currently this only provides available tasks for `deno task`."),
  ],
  arg_groups: &[UNSTABLE_DEPRECATED_ARG, UNSTABLE_FEATURE_ARGS],
  subcommands: &[],
  default_subcommand: None,
  trailing_var_arg: false,
  passthrough: false,
  keep_double_dash: false,
};

pub static INIT_SUBCOMMAND: CommandDef = CommandDef {
  name: "init",
  about: "Initialize a new project",
  aliases: &[],
  args: &[
    ArgDef::new("args")
      .positional()
      .action(ArgAction::Append)
      .num_args(NumArgs::ZeroOrMore)
      .trailing(),
    ArgDef::new("lib")
      .long("lib")
      .set_true()
      .help("Generate an example library project"),
    ArgDef::new("serve")
      .long("serve")
      .set_true()
      .conflicts_with(&["lib"])
      .help("Generate an example project for `deno serve`"),
    ArgDef::new("npm")
      .long("npm")
      .set_true()
      .conflicts_with(&["lib", "serve", "empty", "jsr"])
      .help("Generate a npm create-* project"),
    ArgDef::new("jsr")
      .long("jsr")
      .set_true()
      .conflicts_with(&["lib", "serve", "empty", "npm"])
      .help("Generate a project from a JSR package"),
    ArgDef::new("empty")
      .long("empty")
      .set_true()
      .conflicts_with(&["lib", "serve", "npm", "jsr"])
      .help("Generate a minimal project with just main.ts and deno.json"),
    ArgDef::new("yes")
      .short('y')
      .long("yes")
      .set_true()
      .help("Bypass the prompt and run with full permissions"),
  ],
  arg_groups: &[UNSTABLE_DEPRECATED_ARG, UNSTABLE_FEATURE_ARGS],
  subcommands: &[],
  default_subcommand: None,
  trailing_var_arg: true,
  passthrough: false,
  keep_double_dash: false,
};

pub static CREATE_SUBCOMMAND: CommandDef = CommandDef {
  name: "create",
  about: "Create a project from a template",
  aliases: &[],
  args: &[
    ArgDef::new("package")
      .positional()
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1))
      .required(),
    // Extra package args are only accepted after `--` (clap's `last(true)`)
    // and land in `result.trailing`.
    ArgDef::new("npm")
      .long("npm")
      .set_true()
      .conflicts_with(&["jsr"])
      .help("Treat unprefixed package names as npm packages"),
    ArgDef::new("jsr")
      .long("jsr")
      .set_true()
      .help("Treat unprefixed package names as JSR packages"),
    ArgDef::new("yes")
      .short('y')
      .long("yes")
      .set_true()
      .help("Bypass the prompt and run with full permissions"),
  ],
  arg_groups: &[UNSTABLE_DEPRECATED_ARG, UNSTABLE_FEATURE_ARGS],
  subcommands: &[],
  default_subcommand: None,
  trailing_var_arg: false,
  passthrough: false,
  keep_double_dash: false,
};

pub static JUPYTER_SUBCOMMAND: CommandDef = CommandDef {
  name: "jupyter",
  about: "Deno kernel for Jupyter notebooks",
  aliases: &[],
  args: &[
    ArgDef::new("install")
      .long("install")
      .set_true()
      .conflicts_with(&["kernel"])
.help("Install a kernelspec"),
    ArgDef::new("name")
      .short('n')
      .long("name")
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1))
      .conflicts_with(&["kernel"])
.help("Set a name for the kernel (defaults to 'deno'). Useful when maintaing multiple Deno kernels."),
    ArgDef::new("display")
      .short('d')
      .long("display")
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1))
      .requires(&["install"])
.help("Set a display name for the kernel (defaults to 'Deno'). Useful when maintaing multiple Deno kernels."),
    ArgDef::new("kernel")
      .long("kernel")
      .set_true()
      .conflicts_with(&["install"])
      .requires(&["conn"])
.help("Start the kernel"),
    ArgDef::new("conn")
      .long("conn")
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1))
      .conflicts_with(&["install"])
.help("Path to JSON file describing connection parameters, provided by Jupyter"),
    ArgDef::new("force")
      .long("force")
      .set_true()
      .requires(&["install"])
.help("Force installation of a kernel, overwriting previously existing kernelspec"),
  ],
  // clap's `jupyter` exposes only its own flags (no runtime/permission/compile
  // groups); `jupyter_parse` reads none of them, so they are trimmed to match.
  arg_groups: &[UNSTABLE_DEPRECATED_ARG, UNSTABLE_FEATURE_ARGS],
  subcommands: &[],
  default_subcommand: None,
  trailing_var_arg: false,
  passthrough: false,
  keep_double_dash: false,
};

pub static PUBLISH_SUBCOMMAND: CommandDef = CommandDef {
  name: "publish",
  about: "Publish the current working directory's package or workspace",
  aliases: &[],
  args: &[
    ArgDef::new("token")
      .long("token")
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1))
.help("The API token to use when publishing. If unset, interactive authentication is be used"),
    ArgDef::new("dry-run").long("dry-run").set_true()
.help("Prepare the package for publishing performing all checks and validations without uploading"),
    ArgDef::new("allow-slow-types")
      .long("allow-slow-types")
      .set_true()
.help("Allow publishing with slow types"),
    ArgDef::new("allow-dirty").long("allow-dirty").set_true()
.help("Allow publishing if the repository has uncommitted changed"),
    ArgDef::new("no-provenance")
      .long("no-provenance")
      .set_true()
.help("Disable provenance attestation.\n  Enabled by default on Github actions, publicly links the package to where it was built and published from."),
    ArgDef::new("set-version")
      .long("set-version")
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1))
.help("Set version for a package to be published.\n  This flag can be used while publishing individual packages and cannot be used in a workspace."),
    ArgDef::new("check")
      .long("check")
      .action(ArgAction::Set)
      .num_args(NumArgs::Optional)
      .require_equals()
.help("Set type-checking behavior. This subcommand type-checks local modules by default, so passing --check is redundant; pass --check=all to also type-check remote modules. Alternatively, use the 'deno check' subcommand."),
    ArgDef::new("env-file")
      .long("env-file")
      .long_aliases(&["env"])
      .action(ArgAction::Append)
      .num_args(NumArgs::Optional)
      .require_equals()
.help("Load environment variables from local file\n  Only the first environment variable with a given key is used.\n  Existing process environment variables are not overwritten, so if variables with the same names already exist in the environment, their values will be preserved.\n  Where multiple declarations for the same environment variable exist in your .env file, the first one encountered is applied. This is determined by the order of the files you pass as arguments."),
  ],
  arg_groups: &[UNSTABLE_DEPRECATED_ARG, UNSTABLE_FEATURE_ARGS, COMPILE_ARGS],
  subcommands: &[],
  default_subcommand: None,
  trailing_var_arg: false,
  passthrough: false,
  keep_double_dash: false,
};

pub static ADD_SUBCOMMAND: CommandDef = CommandDef {
  name: "add",
  about: "Add dependencies",
  aliases: &[],
  args: &[
    ArgDef::new("packages")
      .positional()
      .action(ArgAction::Append)
      .num_args(NumArgs::OneOrMore)
      .required()
.help("List of packages to add"),
    ArgDef::new("dev").short('D').long("dev").set_true()
.help("Add the package as a dev dependency (under `devDependencies`). Note: this only applies when adding to a `package.json` file."),
    ArgDef::new("save-optional")
      .short('O')
      .long("save-optional")
      .set_true()
      .conflicts_with(&["dev"])
.help("Add the package as an optional dependency (under `optionalDependencies`). Note: this only applies when adding to a `package.json` file."),
    ArgDef::new("no-save")
      .long("no-save")
      .set_true()
      .conflicts_with(&["dev", "save-optional"])
.help("Install the package(s) without adding them to the configuration file."),
    ArgDef::new("save-exact")
      .long("save-exact")
      .long_aliases(&["exact"])
      .set_true()
.help("Save exact version without the caret (^)"),
    ArgDef::new("unscoped").long("unscoped").set_true()
.help("Use the package name without its scope as the alias (ex. `jsr:@david/jsonc-morph` is added as `jsonc-morph`). Packages given an explicit alias are unaffected."),
    ArgDef::new("npm")
      .long("npm")
      .set_true()
      .conflicts_with(&["jsr"])
.help("assume unprefixed package names are npm packages (default)"),
    ArgDef::new("jsr").long("jsr").set_true()
.help("assume unprefixed package names are jsr packages"),
    ArgDef::new("lockfile-only")
      .long("lockfile-only")
      .set_true()
.help("Install only updating the lockfile"),
    ArgDef::new("allow-import")
      .short('I')
      .long("allow-import")
      .action(ArgAction::Append)
      .num_args(NumArgs::ZeroOrMore)
      .require_equals()
      .value_delimiter(',')
.help("Allow importing from remote hosts. Optionally specify allowed IP addresses and host names, with ports as necessary. Default value: deno.land:443,jsr.io:443,esm.sh:443,raw.esm.sh:443,cdn.jsdelivr.net:443,raw.githubusercontent.com:443,gist.githubusercontent.com:443"),
    ArgDef::new("deny-import")
      .long("deny-import")
      .action(ArgAction::Append)
      .num_args(NumArgs::ZeroOrMore)
      .require_equals()
      .value_delimiter(',')
.help("Deny importing from remote hosts. Optionally specify denied IP addresses and host names, with ports as necessary."),
    ArgDef::new("package-json").long("package-json").set_true()
.help("Force using package.json for dependency management instead of deno.json"),
    ArgDef::new("env-file")
      .long("env-file")
      .long_aliases(&["env"])
      .action(ArgAction::Append)
      .num_args(NumArgs::Optional)
      .require_equals()
.help("Load environment variables from local file\n  Only the first environment variable with a given key is used.\n  Existing process environment variables are not overwritten, so if variables with the same names already exist in the environment, their values will be preserved.\n  Where multiple declarations for the same environment variable exist in your .env file, the first one encountered is applied. This is determined by the order of the files you pass as arguments."),
  ],
  arg_groups: &[UNSTABLE_DEPRECATED_ARG, UNSTABLE_FEATURE_ARGS, ALLOW_SCRIPTS_ARG, COMPILE_ARGS],
  subcommands: &[],
  default_subcommand: None,
  trailing_var_arg: false,
  passthrough: false,
  keep_double_dash: false,
};

pub static REMOVE_SUBCOMMAND: CommandDef = CommandDef {
  name: "remove",
  about: "Remove dependencies",
  aliases: &["rm"],
  args: &[
    ArgDef::new("packages")
      .positional()
      .action(ArgAction::Append)
      .num_args(NumArgs::OneOrMore)
      .required()
.help("List of packages to remove"),
    ArgDef::new("global").short('g').long("global").set_true()
.help("Remove globally installed package or module"),
    ArgDef::new("root")
      .long("root")
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1))
      .requires(&["global"])
.help("Installation root"),
    ArgDef::new("lockfile-only")
      .long("lockfile-only")
      .set_true()
      .conflicts_with(&["global"])
.help("Install only updating the lockfile"),
    ArgDef::new("package-json")
      .long("package-json")
      .set_true()
      .conflicts_with(&["global"])
.help("Force using package.json for dependency management instead of deno.json"),
  ],
  arg_groups: &[UNSTABLE_DEPRECATED_ARG, UNSTABLE_FEATURE_ARGS, COMPILE_ARGS],
  subcommands: &[],
  default_subcommand: None,
  trailing_var_arg: false,
  passthrough: false,
  keep_double_dash: false,
};

pub static OUTDATED_SUBCOMMAND: CommandDef = CommandDef {
  name: "outdated",
  about: "Find outdated dependencies",
  aliases: &[],
  args: &[
    ArgDef::new("filters")
      .positional()
      .action(ArgAction::Append)
      .num_args(NumArgs::ZeroOrMore)
.help("Filters selecting which packages to act on. Can include wildcards (*) to match multiple packages. If a version requirement is specified, the matching packages will be updated to the given requirement."),
    ArgDef::new("recursive")
      .short('r')
      .long("recursive")
      .set_true()
.help("Include all workspace members"),
    ArgDef::new("compatible").long("compatible").set_true()
.help("Only consider versions that satisfy semver requirements"),
    ArgDef::new("update").long("update").short('u').set_true()
.help("Update dependency versions"),
    ArgDef::new("latest")
      .long("latest")
      .set_true()
      .conflicts_with(&["compatible"])
.help("Consider the latest version, regardless of semver constraints"),
    ArgDef::new("interactive")
      .short('i')
      .long("interactive")
      .set_true()
      .requires(&["update"])
.help("Interactively select which dependencies to update"),
    ArgDef::new("lockfile-only")
      .long("lockfile-only")
      .set_true()
.help("Install only updating the lockfile"),
    ArgDef::new("env-file")
      .long("env-file")
      .long_aliases(&["env"])
      .action(ArgAction::Append)
      .num_args(NumArgs::Optional)
      .require_equals()
.help("Load environment variables from local file\n  Only the first environment variable with a given key is used.\n  Existing process environment variables are not overwritten, so if variables with the same names already exist in the environment, their values will be preserved.\n  Where multiple declarations for the same environment variable exist in your .env file, the first one encountered is applied. This is determined by the order of the files you pass as arguments."),
  ],
  arg_groups: &[UNSTABLE_DEPRECATED_ARG, UNSTABLE_FEATURE_ARGS, COMPILE_ARGS],
  subcommands: &[],
  default_subcommand: None,
  trailing_var_arg: false,
  passthrough: false,
  keep_double_dash: false,
};

pub static UPDATE_SUBCOMMAND: CommandDef = CommandDef {
  name: "update",
  about: "Update outdated dependencies",
  aliases: &[],
  args: &[
    ArgDef::new("filters")
      .positional()
      .action(ArgAction::Append)
      .num_args(NumArgs::ZeroOrMore)
.help("Filters selecting which packages to act on. Can include wildcards (*) to match multiple packages. If a version requirement is specified, the matching packages will be updated to the given requirement."),
    ArgDef::new("recursive")
      .short('r')
      .long("recursive")
      .set_true()
.help("Include all workspace members"),
    ArgDef::new("latest").long("latest").set_true()
.help("Consider the latest version, regardless of semver constraints"),
    ArgDef::new("compatible").long("compatible").set_true()
.help("Only consider versions that satisfy semver requirements"),
    ArgDef::new("lockfile-only")
      .long("lockfile-only")
      .set_true()
.help("Install only updating the lockfile"),
    ArgDef::new("interactive")
      .short('i')
      .long("interactive")
      .set_true()
.help("Interactively select which dependencies to update"),
    ArgDef::new("env-file")
      .long("env-file")
      .long_aliases(&["env"])
      .action(ArgAction::Append)
      .num_args(NumArgs::Optional)
      .require_equals()
.help("Load environment variables from local file\n  Only the first environment variable with a given key is used.\n  Existing process environment variables are not overwritten, so if variables with the same names already exist in the environment, their values will be preserved.\n  Where multiple declarations for the same environment variable exist in your .env file, the first one encountered is applied. This is determined by the order of the files you pass as arguments."),
  ],
  arg_groups: &[UNSTABLE_DEPRECATED_ARG, UNSTABLE_FEATURE_ARGS, COMPILE_ARGS],
  subcommands: &[],
  default_subcommand: None,
  trailing_var_arg: false,
  passthrough: false,
  keep_double_dash: false,
};

pub static DEPLOY_SUBCOMMAND: CommandDef = CommandDef {
  name: "deploy",
  about: "Deploy to Deno Deploy",
  aliases: &[],
  args: &[],
  arg_groups: &[UNSTABLE_DEPRECATED_ARG, UNSTABLE_FEATURE_ARGS],
  subcommands: &[],
  default_subcommand: None,
  trailing_var_arg: false,
  passthrough: true,
  keep_double_dash: false,
};

pub static SANDBOX_SUBCOMMAND: CommandDef = CommandDef {
  name: "sandbox",
  about: "Run in sandbox mode",
  aliases: &[],
  args: &[],
  arg_groups: &[UNSTABLE_DEPRECATED_ARG, UNSTABLE_FEATURE_ARGS],
  subcommands: &[],
  default_subcommand: None,
  trailing_var_arg: false,
  passthrough: true,
  keep_double_dash: false,
};

pub static CLEAN_SUBCOMMAND: CommandDef = CommandDef {
  name: "clean",
  about: "Remove the cache directory",
  aliases: &[],
  args: &[
    ArgDef::new("except-paths")
      .positional()
      .action(ArgAction::Append)
      .num_args(NumArgs::ZeroOrMore),
    ArgDef::new("except").long("except").short('e').set_true()
.help("Retain cache data needed by the given files"),
    ArgDef::new("dry-run").long("dry-run").set_true()
.help("Show what would be removed without performing any actions"),
    ArgDef::new("node-modules-dir")
      .long("node-modules-dir")
      .action(ArgAction::Set)
      .num_args(NumArgs::Optional)
      .require_equals()
.help("Selects the node_modules directory mode for npm packages (not a path). One of: auto (create a local node_modules directory and install npm packages into it), manual (use the existing local node_modules directory, do not modify it), none (do not use a local node_modules directory; resolve npm packages from the global cache). Defaults to auto when the flag is passed without a value."),
    ArgDef::new("vendor")
      .long("vendor")
      .action(ArgAction::Set)
      .num_args(NumArgs::Optional)
      .require_equals()
.help("Toggles local vendor folder usage for remote modules and a node_modules folder for npm packages"),
    ArgDef::new("node-modules-linker")
      .long("node-modules-linker")
      .long_aliases(&["linker"])
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1))
      .require_equals()
      .requires(&["except"])
.help("Sets the linker mode for npm packages (isolated or hoisted)"),
  ],
  arg_groups: &[UNSTABLE_DEPRECATED_ARG, UNSTABLE_FEATURE_ARGS],
  subcommands: &[],
  default_subcommand: None,
  trailing_var_arg: false,
  passthrough: false,
  keep_double_dash: false,
};

pub static LIST_SUBCOMMAND: CommandDef = CommandDef {
  name: "list",
  about: "List the dependencies declared in deno.json / package.json",
  aliases: &[],
  args: &[
    ArgDef::new("filters")
      .positional()
      .action(ArgAction::Append)
      .num_args(NumArgs::ZeroOrMore)
.help("Filters selecting which packages to list. Can include wildcards (*) to match multiple packages, and a leading '!' to exclude."),
    ArgDef::new("depth")
      .long("depth")
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1))
      .value_parser(ValueParser::U16)
.help("Maximum depth of the dependency tree to display (0 = direct dependencies only)"),
    ArgDef::new("prod")
      .long("prod")
      .set_true()
      .conflicts_with(&["dev"])
.help("Only list production dependencies"),
    ArgDef::new("dev").long("dev").set_true()
.help("Only list development dependencies"),
    ArgDef::new("recursive")
      .long("recursive")
      .short('r')
      .set_true()
.help("Include all workspace members"),
  ],
  arg_groups: &[UNSTABLE_DEPRECATED_ARG, UNSTABLE_FEATURE_ARGS],
  subcommands: &[],
  default_subcommand: None,
  trailing_var_arg: false,
  passthrough: false,
  keep_double_dash: false,
};

pub static APPROVE_SCRIPTS_SUBCOMMAND: CommandDef = CommandDef {
  name: "approve-scripts",
  about: "Approve npm lifecycle scripts",
  aliases: &["approve-builds"],
  args: &[
    ArgDef::new("packages")
      .positional()
      .action(ArgAction::Append)
      .num_args(NumArgs::ZeroOrMore)
      .value_delimiter(',')
.help("Packages to approve (npm specifiers). When omitted, you will be prompted to select from installed packages with lifecycle scripts."),
    ArgDef::new("lockfile-only")
      .long("lockfile-only")
      .set_true()
.help("Install only updating the lockfile"),
  ],
  arg_groups: &[UNSTABLE_DEPRECATED_ARG, UNSTABLE_FEATURE_ARGS, COMPILE_ARGS],
  subcommands: &[],
  default_subcommand: None,
  trailing_var_arg: false,
  passthrough: false,
  keep_double_dash: false,
};

pub static LSP_SUBCOMMAND: CommandDef = CommandDef {
  name: "lsp",
  about: "Start the language server",
  aliases: &[],
  args: &[],
  arg_groups: &[UNSTABLE_DEPRECATED_ARG, UNSTABLE_FEATURE_ARGS],
  subcommands: &[],
  default_subcommand: None,
  trailing_var_arg: false,
  passthrough: false,
  keep_double_dash: false,
};

pub static VENDOR_SUBCOMMAND: CommandDef = CommandDef {
  name: "vendor",
  about: "`deno vendor` was removed in Deno 2.\n\nSee the Deno 1.x to 2.x Migration Guide for migration instructions: https://docs.deno.com/runtime/manual/advanced/migrate_deprecations",
  aliases: &[],
  args: &[
    ArgDef::new("help")
      .short('h')
      .long("help")
      .action(ArgAction::Append)
      .num_args(NumArgs::Optional)
      .require_equals()
      .value_name("CONTEXT")
      .help("[possible values: unstable, full]"),
    ArgDef::new("quiet")
      .short('q')
      .long("quiet")
      .set_true()
      .help("Suppress diagnostic output")
.help("Suppress diagnostic output"),
    ArgDef::new("unstable")
      .long("unstable")
      .set_true()
      .help("The `--unstable` flag has been deprecated. Use granular `--unstable-*` flags instead\nTo view the list of individual unstable feature flags, run this command again with --help=unstable")
.help("The `--unstable` flag has been deprecated. Use granular `--unstable-*` flags instead\n  To view the list of individual unstable feature flags, run this command again with --help=unstable"),
  ],
  arg_groups: &[UNSTABLE_FEATURE_ARGS],
  subcommands: &[],
  default_subcommand: None,
  trailing_var_arg: false,
  passthrough: false,
  keep_double_dash: false,
};

pub static BUNDLE_SUBCOMMAND: CommandDef = CommandDef {
  name: "bundle",
  about: "Output a single JavaScript file with all dependencies",
  aliases: &[],
  args: &[
    ArgDef::new("file")
      .positional()
      .action(ArgAction::Append)
      .num_args(NumArgs::OneOrMore),
    ArgDef::new("output")
      .short('o')
      .long("output")
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1))
.help("Output path`"),
    ArgDef::new("outdir")
      .long("outdir")
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1))
.help("Output directory for bundled files"),
    ArgDef::new("format")
      .long("format")
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1))
      .default_value("esm"),
    ArgDef::new("packages")
      .long("packages")
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1))
      .default_value("bundle")
.help("How to handle packages. Accepted values are 'bundle' or 'external'"),
    ArgDef::new("platform")
      .long("platform")
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1))
      .default_value("deno")
.help("Platform to bundle for. Accepted values are 'browser' or 'deno'"),
    ArgDef::new("sourcemap")
      .long("sourcemap")
      .action(ArgAction::Set)
      .num_args(NumArgs::Optional)
      .require_equals()
      .value_parser(ValueParser::Choices(&["linked", "inline", "external"]))
.help("Generate source map. Accepted values are 'linked', 'inline', or 'external'"),
    ArgDef::new("external")
      .long("external")
      .action(ArgAction::Append)
      .num_args(NumArgs::ZeroOrMore),
    ArgDef::new("watch").long("watch").set_true()
.help("Watch and rebuild on changes"),
    ArgDef::new("minify").long("minify").set_true()
.help("Minify the output"),
    ArgDef::new("keep-names").long("keep-names").set_true()
.help("Keep function and class names"),
    ArgDef::new("code-splitting")
      .long("code-splitting")
      .set_true()
.help("Enable code splitting"),
    ArgDef::new("inline-imports")
      .long("inline-imports")
      .action(ArgAction::Set)
      .num_args(NumArgs::Optional)
      .require_equals()
      .default_value("true")
.help("Whether to inline imported modules into the importing file [default: true]"),
    ArgDef::new("declaration").long("declaration").set_true()
.help("Generate .d.ts declaration files alongside the bundle"),
    ArgDef::new("check")
      .long("check")
      .action(ArgAction::Set)
      .num_args(NumArgs::Optional)
      .require_equals()
.help("Enable type-checking. This subcommand does not type-check by default; pass --check=all to also type-check remote modules. Alternatively, use the 'deno check' subcommand."),
  ],
  arg_groups: &[
    COMPILE_ARGS,
    UNSTABLE_DEPRECATED_ARG, UNSTABLE_FEATURE_ARGS,
    IMPORT_PERMISSION_ARGS,
    ALLOW_SCRIPTS_ARG,
  ],
  subcommands: &[],
  default_subcommand: None,
  trailing_var_arg: false,
  passthrough: false,
  keep_double_dash: false,
};

pub static AUDIT_SUBCOMMAND: CommandDef = CommandDef {
  name: "audit",
  about: "Audit currently installed dependencies",
  aliases: &[],
  args: &[
    ArgDef::new("level")
      .long("level")
      .long_aliases(&["audit-level", "severity"])
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1))
      .default_value("low")
      .value_parser(ValueParser::Choices(&[
        "low", "moderate", "high", "critical",
      ]))
.help("Only show advisories with severity greater or equal to the one specified"),
    ArgDef::new("ignore-unfixable")
      .long("ignore-unfixable")
      .set_true()
.help("Ignore advisories that don't have any actions to resolve them"),
    ArgDef::new("ignore-registry-errors")
      .long("ignore-registry-errors")
      .set_true()
.help("Return exit code 0 if remote service(s) responds with an error."),
    ArgDef::new("socket").long("socket").set_true()
.help("Check against socket.dev vulnerability database"),
    ArgDef::new("fix").long("fix").set_true()
.help("Automatically fix vulnerabilities by upgrading packages"),
    ArgDef::new("action")
      .positional()
      .action(ArgAction::Set)
      .num_args(NumArgs::Optional),
    ArgDef::new("ignore")
      .long("ignore")
      .action(ArgAction::Append)
      .num_args(NumArgs::ZeroOrMore)
      .require_equals()
      .value_delimiter(',')
.help("Ignore advisories matching the given CVE IDs"),
  ],
  arg_groups: &[UNSTABLE_DEPRECATED_ARG, UNSTABLE_FEATURE_ARGS, LOCK_ARGS],
  subcommands: &[],
  default_subcommand: None,
  trailing_var_arg: false,
  passthrough: false,
  keep_double_dash: false,
};

pub static X_SUBCOMMAND: CommandDef = CommandDef {
  name: "x",
  about: "Execute a binary from npm or jsr, like npx",
  aliases: &[],
  args: &[
    ArgDef::new("script_arg")
      .positional()
      .action(ArgAction::Append)
      .num_args(NumArgs::ZeroOrMore)
      .trailing()
.help("Script arg"),
    ArgDef::new("yes")
      .short('y')
      .long("yes")
      .set_true()
      .conflicts_with(&["install-alias"])
.help("Assume confirmation for all prompts"),
    ArgDef::new("package")
      .short('p')
      .long("package")
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1))
      .conflicts_with(&["install-alias"])
.help("Package to install (use when the binary name differs from the package name)"),
    ArgDef::new("ignore-scripts")
      .long("ignore-scripts")
      .action(ArgAction::Append)
      .num_args(NumArgs::ZeroOrMore)
      .require_equals()
      .conflicts_with(&["allow-scripts", "install-alias"])
.help("Do not run npm lifecycle scripts for the given packages"),
    ArgDef::new("install-alias")
      .long("install-alias")
      .action(ArgAction::Set)
      .num_args(NumArgs::Optional)
      .default_value("dx")
      .conflicts_with(&["script_arg"])
.help("Creates a dx alias so you can run dx <command> instead of deno x <command>"),
    ArgDef::new("check")
      .long("check")
      .action(ArgAction::Set)
      .num_args(NumArgs::Optional)
      .require_equals()
.help("Enable type-checking. This subcommand does not type-check by default; pass --check=all to also type-check remote modules. Alternatively, use the 'deno check' subcommand."),
    ArgDef::new("env-file")
      .long("env-file")
      .action(ArgAction::Append)
      .num_args(NumArgs::Optional)
      .require_equals()
.help("Load environment variables from local file\n  Only the first environment variable with a given key is used.\n  Existing process environment variables are not overwritten, so if variables with the same names already exist in the environment, their values will be preserved.\n  Where multiple declarations for the same environment variable exist in your .env file, the first one encountered is applied. This is determined by the order of the files you pass as arguments."),
  ],
  arg_groups: &[
    ALLOW_SCRIPTS_ARG,
    UNSTABLE_DEPRECATED_ARG, UNSTABLE_FEATURE_ARGS,
    COMPILE_ARGS,
    PERMISSION_ARGS,
    RUNTIME_MISC_ARGS,
    INSPECT_ARGS,
    UNSTABLE_DEPRECATED_ARG, UNSTABLE_FEATURE_ARGS,
  ],
  subcommands: &[],
  default_subcommand: None,
  trailing_var_arg: true,
  passthrough: false,
  keep_double_dash: false,
};

pub static JSON_REFERENCE_SUBCOMMAND: CommandDef = CommandDef {
  name: "json_reference",
  about: "",
  aliases: &[],
  args: &[],
  arg_groups: &[],
  subcommands: &[],
  default_subcommand: None,
  trailing_var_arg: false,
  passthrough: false,
  keep_double_dash: false,
};

pub static HELP_SUBCOMMAND: CommandDef = CommandDef {
  name: "help",
  about: "Show help for a command",
  aliases: &[],
  args: &[ArgDef::new("subcommand")
    .positional()
    .action(ArgAction::Set)
    .num_args(NumArgs::Optional)],
  arg_groups: &[],
  subcommands: &[],
  default_subcommand: None,
  trailing_var_arg: false,
  passthrough: false,
  keep_double_dash: false,
};

// ============================================================
// Root command
// ============================================================

pub static GLOBAL_ARGS: &[ArgDef] = &[
  ArgDef::new("env-file")
    .long("env-file")
    .long_aliases(&["env"])
    .action(ArgAction::Append)
    .num_args(NumArgs::Optional)
    .require_equals()
    .global()
.help("Load environment variables from local file\n  Only the first environment variable with a given key is used.\n  Existing process environment variables are not overwritten, so if variables with the same names already exist in the environment, their values will be preserved.\n  Where multiple declarations for the same environment variable exist in your .env file, the first one encountered is applied. This is determined by the order of the files you pass as arguments."),
  ArgDef::new("help")
    .short('h')
    .long("help")
    .action(ArgAction::Append)
    .num_args(NumArgs::Optional)
    .require_equals()
    .global(),
  ArgDef::new("version")
    .short('V')
    .long("version")
    .set_true()
    .short_aliases(&['v'])
    .global()
.help("Print version"),
  ArgDef::new("log-level")
    .short('L')
    .long("log-level")
    .action(ArgAction::Set)
    .num_args(NumArgs::Exact(1))
    .value_parser(ValueParser::Choices(&["trace", "debug", "info"]))
    .global()
.help("Set log level"),
  ArgDef::new("quiet")
    .short('q')
    .long("quiet")
    .set_true()
    .global()
.help("Suppress diagnostic output"),
];

pub static DESKTOP_SUBCOMMAND: CommandDef = CommandDef {
  name: "desktop",
  about: "Compile a script into a desktop application",
  aliases: &[],
  args: &[
    ArgDef::new("script_arg")
      .positional()
      .action(ArgAction::Append)
      .num_args(NumArgs::ZeroOrMore)
.help("Script arg"),
    ArgDef::new("check")
      .long("check")
      .action(ArgAction::Set)
      .num_args(NumArgs::Optional)
      .require_equals()
.help("Set type-checking behavior. This subcommand type-checks local modules by default, so passing --check is redundant; pass --check=all to also type-check remote modules. Alternatively, use the 'deno check' subcommand."),
    ArgDef::new("inspect-renderer")
      .long("inspect-renderer")
      .action(ArgAction::Set)
      .num_args(NumArgs::Optional)
      .require_equals()
      .default_value("127.0.0.1:0")
.help("Override the CEF renderer debugger listen address; defaults to an auto-allocated port"),
    ArgDef::new("include")
      .long("include")
      .action(ArgAction::Append)
      .num_args(NumArgs::Exact(1))
.help("Includes an additional module or file/directory in the compiled executable.\n  Use this flag if a dynamically imported module or a web worker main module\n  fails to load in the executable or to embed a file or directory in the executable.\n  This flag can be passed multiple times, to include multiple additional modules."),
    ArgDef::new("exclude")
      .long("exclude")
      .action(ArgAction::Append)
      .num_args(NumArgs::Exact(1))
.help("Excludes a file/directory in the compiled executable.\n  Use this flag to exclude a specific file or directory within the included files."),
    ArgDef::new("exclude-unused-npm")
      .long("exclude-unused-npm")
      .set_true()
.help("Embed only the npm packages reachable from the module graph (managed npm; no node_modules directory).\n  Without this flag the full managed npm snapshot from the lockfile / package.json is embedded.\n  Reduces binary size when the lockfile contains packages the entrypoint does not import.\n  Skips packages that are only reached through non-statically-analyzable dynamic imports;\n  pass those with --include npm:<pkg> if needed."),
    ArgDef::new("output")
      .short('o')
      .long("output")
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1))
.help("Output path (e.g. MyApp.app, MyApp.dmg, MyApp.AppImage, MyApp.deb, MyApp.rpm, MyApp.msi)"),
    ArgDef::new("target")
      .long("target")
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1))
      .value_parser(ValueParser::Choices(SUPPORTED_OS))
.help("Target OS architecture"),
    ArgDef::new("no-code-cache")
      .long("no-code-cache")
      .set_true()
.help("Disable V8 code cache feature"),
    ArgDef::new("icon")
      .long("icon")
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1))
.help("Set the application icon (.ico on Windows, .icns or .png on macOS)"),
    ArgDef::new("hmr").long("hmr").set_true()
.help("Run the desktop app with Hot Module Replacement enabled"),
    ArgDef::new("backend")
      .long("backend")
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1))
      .value_parser(ValueParser::Choices(&["webview", "cef", "raw"]))
.help("Backend to use for the desktop app"),
    ArgDef::new("engine")
      .long("engine")
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1))
      .value_parser(ValueParser::Choices(&["v8", "quickjs"]))
      .help(
        "JS engine the desktop binary runs on (quickjs is smaller and experimental, and does not receive the same security updates as v8)",
      ),
    ArgDef::new("all-targets").long("all-targets").set_true()
.help("Build for all supported target platforms"),
    ArgDef::new("compress")
      .long("compress")
      .action(ArgAction::Set)
      .num_args(NumArgs::Optional)
      .default_value("xz")
      .value_parser(ValueParser::Choices(&["xz", "lzma", "zstd"]))
.help("Make the packaged app self-extracting: the payload is compressed inside the app and unpacked on first launch. Off by default. Defaults to xz (decompressed by the system `tar` everywhere); zstd is smaller/faster but needs the `zstd` tool at runtime."),
    ArgDef::new("ext")
      .long("ext")
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1))
      .value_parser(ValueParser::Choices(EXECUTABLE_EXTS))
.help("Set content type of the supplied file"),
    ArgDef::new("env-file")
      .long("env-file")
      .long_aliases(&["env"])
      .action(ArgAction::Append)
      .num_args(NumArgs::Optional)
      .require_equals()
.help("Load environment variables from local file\n  Only the first environment variable with a given key is used.\n  Existing process environment variables are not overwritten, so if variables with the same names already exist in the environment, their values will be preserved.\n  Where multiple declarations for the same environment variable exist in your .env file, the first one encountered is applied. This is determined by the order of the files you pass as arguments."),
  ],
  arg_groups: &[
    UNSTABLE_DEPRECATED_ARG, UNSTABLE_FEATURE_ARGS,
    PERMISSION_ARGS,
    COMPILE_ARGS,
    INSPECT_ARGS,
    RUNTIME_MISC_ARGS,
    CPU_PROF_ARGS,
    ALLOW_SCRIPTS_ARG,
  ],
  subcommands: &[],
  default_subcommand: None,
  trailing_var_arg: true,
  passthrough: false,
  keep_double_dash: true,
};

pub static PACK_SUBCOMMAND: CommandDef = CommandDef {
  name: "pack",
  about: "Create a tarball of the package",
  aliases: &[],
  args: &[
    ArgDef::new("files")
      .positional()
      .action(ArgAction::Append)
      .num_args(NumArgs::ZeroOrMore)
.help("List of file patterns to include"),
    ArgDef::new("output")
      .short('o')
      .long("output")
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1))
.help("Output file path (defaults to <name>-<version>.tgz)"),
    ArgDef::new("dry-run").long("dry-run").set_true()
.help("Show what would be packed without creating the tarball"),
    ArgDef::new("allow-slow-types")
      .long("allow-slow-types")
      .set_true()
.help("Skip fast-check type extraction; .d.ts files are omitted from the output"),
    ArgDef::new("allow-dirty").long("allow-dirty").set_true()
.help("Allow packing if the repository has uncommitted changes"),
    ArgDef::new("set-version")
      .long("set-version")
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1))
.help("Override the version in the tarball"),
    ArgDef::new("no-source-maps")
      .long("no-source-maps")
      .set_true()
.help("Don't include source maps in the output"),
    ArgDef::new("ignore")
      .long("ignore")
      .action(ArgAction::Append)
      .num_args(NumArgs::ZeroOrMore)
      .require_equals()
      .value_delimiter(',')
.help("Ignore files matching these patterns"),
    ArgDef::new("config")
      .short('c')
      .long("config")
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1))
.help("Configure different aspects of deno including TypeScript, linting, and code formatting.\n  Typically the configuration file will be called `deno.json` or `deno.jsonc` and\n  automatically detected; in that case this flag is not necessary.\n  Docs: https://docs.deno.com/go/config"),
    ArgDef::new("no-config")
      .long("no-config")
      .set_true()
      .conflicts_with(&["config"])
.help("Disable automatic loading of the configuration file"),
    ArgDef::new("env-file")
      .long("env-file")
      .long_aliases(&["env"])
      .action(ArgAction::Append)
      .num_args(NumArgs::Optional)
      .require_equals()
.help("Load environment variables from local file\n  Only the first environment variable with a given key is used.\n  Existing process environment variables are not overwritten, so if variables with the same names already exist in the environment, their values will be preserved.\n  Where multiple declarations for the same environment variable exist in your .env file, the first one encountered is applied. This is determined by the order of the files you pass as arguments."),
  ],
  arg_groups: &[UNSTABLE_DEPRECATED_ARG, UNSTABLE_FEATURE_ARGS],
  subcommands: &[],
  default_subcommand: None,
  trailing_var_arg: false,
  passthrough: false,
  keep_double_dash: false,
};

pub static CI_SUBCOMMAND: CommandDef = CommandDef {
  name: "ci",
  about: "Install dependencies from a lockfile in a frozen state",
  aliases: &[],
  args: &[
    ArgDef::new("prod")
      .long("prod")
      .long_aliases(&["production"])
      .set_true()
.help("Only install production dependencies (excludes devDependencies)"),
    ArgDef::new("skip-types")
      .long("skip-types")
      .set_true()
      .requires(&["prod"])
.help("Exclude @types/* packages from installation.\nBe careful, as it uses a name-based heuristic and may skip packages that ship runtime code."),
    ArgDef::new("env-file")
      .long("env-file")
      .long_aliases(&["env"])
      .action(ArgAction::Append)
      .num_args(NumArgs::Optional)
      .require_equals()
.help("Load environment variables from local file\n  Only the first environment variable with a given key is used.\n  Existing process environment variables are not overwritten, so if variables with the same names already exist in the environment, their values will be preserved.\n  Where multiple declarations for the same environment variable exist in your .env file, the first one encountered is applied. This is determined by the order of the files you pass as arguments."),
  ],
  arg_groups: &[UNSTABLE_DEPRECATED_ARG, UNSTABLE_FEATURE_ARGS],
  subcommands: &[],
  default_subcommand: None,
  trailing_var_arg: false,
  passthrough: false,
  keep_double_dash: false,
};

pub static BUMP_VERSION_SUBCOMMAND: CommandDef = CommandDef {
  name: "bump-version",
  about: "Update version in the configuration file",
  aliases: &[],
  args: &[
    ArgDef::new("increment")
      .positional()
      .action(ArgAction::Set)
      .num_args(NumArgs::Optional)
.help("Version increment type"),
    ArgDef::new("workspace")
      .long("workspace")
      .short('w')
      .set_true()
.help("Bump every package in the workspace (auto-detected at the workspace root)"),
    ArgDef::new("no-workspace").long("no-workspace").set_true()
.help("Disable workspace mode and only bump the deno.json/package.json in the current directory"),
    ArgDef::new("dry-run").long("dry-run").set_true()
.help("Print the planned changes without writing any files"),
    ArgDef::new("start")
      .long("start")
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1))
.help("[conventional-commits mode] Git ref to start from. Default: latest tag (git describe --tags --abbrev=0)"),
    ArgDef::new("base")
      .long("base")
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1))
.help("[conventional-commits mode] Git ref to compare against. Default: current branch"),
    ArgDef::new("import-map")
      .long("import-map")
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1))
.help("Path to the import map to rewrite jsr: version constraints in. Defaults to the root deno.json (or its importMap target)"),
    ArgDef::new("release-notes")
      .long("release-notes")
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1))
.help("[conventional-commits mode] Path to the release notes file to prepend. Default: Releases.md"),
    ArgDef::new("config")
      .short('c')
      .long("config")
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1))
      .conflicts_with(&["workspace"])
.help("Explicit path to the manifest file to bump.\n  May point to a `deno.json`/`deno.jsonc` or a `package.json`. When\n  set, single-file mode is forced (workspace auto-detection is bypassed).\n  Useful when both `deno.json` and `package.json` exist in the same\n  directory."),
  ],
  arg_groups: &[UNSTABLE_DEPRECATED_ARG, UNSTABLE_FEATURE_ARGS],
  subcommands: &[],
  default_subcommand: None,
  trailing_var_arg: false,
  passthrough: false,
  keep_double_dash: false,
};

pub static TRANSPILE_SUBCOMMAND: CommandDef = CommandDef {
  name: "transpile",
  about: "Transpile TypeScript/JSX/TSX files to JavaScript",
  aliases: &[],
  args: &[
    ArgDef::new("file")
      .positional()
      .action(ArgAction::Append)
      .num_args(NumArgs::OneOrMore)
      .required(),
    ArgDef::new("output")
      .long("output")
      .short('o')
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1))
      .conflicts_with(&["outdir"])
      .help("Output file path (for single file transpilation)"),
    ArgDef::new("outdir")
      .long("outdir")
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1))
      .help("Output directory for transpiled files"),
    ArgDef::new("source-map")
      .long("source-map")
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1))
      .default_value("none")
      .value_parser(ValueParser::Choices(&["none", "inline", "separate"]))
      .help("Source map mode: none, inline, or separate"),
    ArgDef::new("declaration")
      .long("declaration")
      .set_true()
      .help(
        "Generate .d.ts declaration files (requires type-checking via tsc)",
      ),
  ],
  arg_groups: &[UNSTABLE_DEPRECATED_ARG, UNSTABLE_FEATURE_ARGS, COMPILE_ARGS],
  subcommands: &[],
  default_subcommand: None,
  trailing_var_arg: false,
  passthrough: false,
  keep_double_dash: false,
};

pub static WHY_SUBCOMMAND: CommandDef = CommandDef {
  name: "why",
  about: "Show why a package is installed",
  aliases: &[],
  args: &[
    ArgDef::new("package").positional().required()
.help("The package name (and optional version) to explain"),
    ArgDef::new("env-file")
      .long("env-file")
      .long_aliases(&["env"])
      .action(ArgAction::Append)
      .num_args(NumArgs::Optional)
      .require_equals()
.help("Load environment variables from local file\n  Only the first environment variable with a given key is used.\n  Existing process environment variables are not overwritten, so if variables with the same names already exist in the environment, their values will be preserved.\n  Where multiple declarations for the same environment variable exist in your .env file, the first one encountered is applied. This is determined by the order of the files you pass as arguments."),
  ],
  arg_groups: &[UNSTABLE_DEPRECATED_ARG, UNSTABLE_FEATURE_ARGS, LOCK_ARGS],
  subcommands: &[],
  default_subcommand: None,
  trailing_var_arg: false,
  passthrough: false,
  keep_double_dash: false,
};

pub static LINK_SUBCOMMAND: CommandDef = CommandDef {
  name: "link",
  about: "Link a local JSR package into the current project for development",
  aliases: &[],
  args: &[
    ArgDef::new("paths")
      .positional()
      .action(ArgAction::Append)
      .num_args(NumArgs::OneOrMore)
      .required()
      .help("Paths to local package directories to link"),
    ArgDef::new("lockfile-only")
      .long("lockfile-only")
      .set_true()
      .help("Install only updating the lockfile"),
  ],
  arg_groups: &[UNSTABLE_DEPRECATED_ARG, UNSTABLE_FEATURE_ARGS, LOCK_ARGS],
  subcommands: &[],
  default_subcommand: None,
  trailing_var_arg: false,
  passthrough: false,
  keep_double_dash: false,
};

pub static UNLINK_SUBCOMMAND: CommandDef = CommandDef {
  name: "unlink",
  about: "Remove a linked local package from the current project",
  aliases: &[],
  args: &[
    ArgDef::new("names_or_paths")
      .positional()
      .action(ArgAction::Append)
      .num_args(NumArgs::OneOrMore)
      .required()
      .help("Linked package names or paths to remove"),
    ArgDef::new("lockfile-only")
      .long("lockfile-only")
      .set_true()
      .help("Install only updating the lockfile"),
  ],
  arg_groups: &[UNSTABLE_DEPRECATED_ARG, UNSTABLE_FEATURE_ARGS, LOCK_ARGS],
  subcommands: &[],
  default_subcommand: None,
  trailing_var_arg: false,
  passthrough: false,
  keep_double_dash: false,
};

pub static SYNC_TYPES_SUBCOMMAND: CommandDef = CommandDef {
  name: "sync-types",
  about: "Generate a tsconfig.json and type mappings so stock TypeScript tooling can type-check the project",
  aliases: &[],
  args: &[
    ArgDef::new("roots")
      .positional()
      .action(ArgAction::Append)
      .num_args(NumArgs::ZeroOrMore)
.help("Module graph roots to use for dependency discovery"),
    ArgDef::new("allow-import")
      .short('I')
      .long("allow-import")
      .action(ArgAction::Append)
      .num_args(NumArgs::ZeroOrMore)
      .require_equals()
      .value_delimiter(',')
.help("Allow importing from remote hosts. Optionally specify allowed IP addresses and host names, with ports as necessary. Default value: deno.land:443,jsr.io:443,esm.sh:443,raw.esm.sh:443,cdn.jsdelivr.net:443,raw.githubusercontent.com:443,gist.githubusercontent.com:443"),
    ArgDef::new("deny-import")
      .long("deny-import")
      .action(ArgAction::Append)
      .num_args(NumArgs::ZeroOrMore)
      .require_equals()
      .value_delimiter(',')
.help("Deny importing from remote hosts. Optionally specify denied IP addresses and host names, with ports as necessary."),
  ],
  arg_groups: &[UNSTABLE_DEPRECATED_ARG, UNSTABLE_FEATURE_ARGS],
  subcommands: &[],
  default_subcommand: None,
  trailing_var_arg: false,
  passthrough: false,
  keep_double_dash: false,
};

pub static DENO_ROOT: CommandDef = CommandDef {
  name: "deno",
  about: "A modern JavaScript and TypeScript runtime",
  aliases: &[],
  args: GLOBAL_ARGS,
  arg_groups: &[UNSTABLE_DEPRECATED_ARG, UNSTABLE_FEATURE_ARGS],
  subcommands: &[
    RUN_SUBCOMMAND,
    WATCH_SUBCOMMAND,
    SERVE_SUBCOMMAND,
    EVAL_SUBCOMMAND,
    FMT_SUBCOMMAND,
    LINT_SUBCOMMAND,
    TEST_SUBCOMMAND,
    UPGRADE_SUBCOMMAND,
    CACHE_SUBCOMMAND,
    CHECK_SUBCOMMAND,
    INFO_SUBCOMMAND,
    DOC_SUBCOMMAND,
    TASK_SUBCOMMAND,
    BENCH_SUBCOMMAND,
    COMPILE_SUBCOMMAND,
    COVERAGE_SUBCOMMAND,
    REPL_SUBCOMMAND,
    INSTALL_SUBCOMMAND,
    UNINSTALL_SUBCOMMAND,
    TYPES_SUBCOMMAND,
    COMPLETIONS_SUBCOMMAND,
    INIT_SUBCOMMAND,
    CREATE_SUBCOMMAND,
    JUPYTER_SUBCOMMAND,
    PUBLISH_SUBCOMMAND,
    ADD_SUBCOMMAND,
    REMOVE_SUBCOMMAND,
    OUTDATED_SUBCOMMAND,
    UPDATE_SUBCOMMAND,
    DEPLOY_SUBCOMMAND,
    SANDBOX_SUBCOMMAND,
    CLEAN_SUBCOMMAND,
    LIST_SUBCOMMAND,
    LINK_SUBCOMMAND,
    UNLINK_SUBCOMMAND,
    SYNC_TYPES_SUBCOMMAND,
    APPROVE_SCRIPTS_SUBCOMMAND,
    LSP_SUBCOMMAND,
    VENDOR_SUBCOMMAND,
    BUNDLE_SUBCOMMAND,
    AUDIT_SUBCOMMAND,
    WHY_SUBCOMMAND,
    TRANSPILE_SUBCOMMAND,
    BUMP_VERSION_SUBCOMMAND,
    CI_SUBCOMMAND,
    DESKTOP_SUBCOMMAND,
    PACK_SUBCOMMAND,
    X_SUBCOMMAND,
    JSON_REFERENCE_SUBCOMMAND,
    HELP_SUBCOMMAND,
  ],
  default_subcommand: Some("run"),
  trailing_var_arg: false,
  passthrough: false,
  keep_double_dash: false,
};
