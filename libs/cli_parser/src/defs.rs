// Copyright 2018-2026 the Deno authors. MIT license.
//! Static command definitions for the Deno CLI.
//!
//! All definitions are `const` — they live in `.rodata` and cost zero
//! runtime initialization.

use crate::types::*;

// ============================================================
// Shared arg groups
// ============================================================

pub static PERMISSION_ARGS: &[ArgDef] = &[
  ArgDef::new("allow-all")
    .short('A')
    .long("allow-all")
    .set_true(),
  ArgDef::new("allow-read")
    .short('R')
    .long("allow-read")
    .action(ArgAction::Append)
    .num_args(NumArgs::ZeroOrMore)
    .require_equals()
    .value_delimiter(','),
  ArgDef::new("deny-read")
    .long("deny-read")
    .action(ArgAction::Append)
    .num_args(NumArgs::ZeroOrMore)
    .require_equals()
    .value_delimiter(','),
  ArgDef::new("ignore-read")
    .long("ignore-read")
    .action(ArgAction::Append)
    .num_args(NumArgs::ZeroOrMore)
    .require_equals()
    .value_delimiter(','),
  ArgDef::new("allow-write")
    .short('W')
    .long("allow-write")
    .action(ArgAction::Append)
    .num_args(NumArgs::ZeroOrMore)
    .require_equals()
    .value_delimiter(','),
  ArgDef::new("deny-write")
    .long("deny-write")
    .action(ArgAction::Append)
    .num_args(NumArgs::ZeroOrMore)
    .require_equals()
    .value_delimiter(','),
  ArgDef::new("allow-net")
    .short('N')
    .long("allow-net")
    .action(ArgAction::Append)
    .num_args(NumArgs::ZeroOrMore)
    .require_equals()
    .value_delimiter(','),
  ArgDef::new("deny-net")
    .long("deny-net")
    .action(ArgAction::Append)
    .num_args(NumArgs::ZeroOrMore)
    .require_equals()
    .value_delimiter(','),
  ArgDef::new("allow-env")
    .short('E')
    .long("allow-env")
    .action(ArgAction::Append)
    .num_args(NumArgs::ZeroOrMore)
    .require_equals()
    .value_delimiter(','),
  ArgDef::new("deny-env")
    .long("deny-env")
    .action(ArgAction::Append)
    .num_args(NumArgs::ZeroOrMore)
    .require_equals()
    .value_delimiter(','),
  ArgDef::new("ignore-env")
    .long("ignore-env")
    .action(ArgAction::Append)
    .num_args(NumArgs::ZeroOrMore)
    .require_equals()
    .value_delimiter(','),
  ArgDef::new("allow-run")
    .long("allow-run")
    .action(ArgAction::Append)
    .num_args(NumArgs::ZeroOrMore)
    .require_equals()
    .value_delimiter(','),
  ArgDef::new("deny-run")
    .long("deny-run")
    .action(ArgAction::Append)
    .num_args(NumArgs::ZeroOrMore)
    .require_equals()
    .value_delimiter(','),
  ArgDef::new("allow-sys")
    .short('S')
    .long("allow-sys")
    .action(ArgAction::Append)
    .num_args(NumArgs::ZeroOrMore)
    .require_equals()
    .value_delimiter(','),
  ArgDef::new("deny-sys")
    .long("deny-sys")
    .action(ArgAction::Append)
    .num_args(NumArgs::ZeroOrMore)
    .require_equals()
    .value_delimiter(','),
  ArgDef::new("allow-ffi")
    .long("allow-ffi")
    .action(ArgAction::Append)
    .num_args(NumArgs::ZeroOrMore)
    .require_equals()
    .value_delimiter(','),
  ArgDef::new("deny-ffi")
    .long("deny-ffi")
    .action(ArgAction::Append)
    .num_args(NumArgs::ZeroOrMore)
    .require_equals()
    .value_delimiter(','),
  ArgDef::new("allow-import")
    .short('I')
    .long("allow-import")
    .action(ArgAction::Append)
    .num_args(NumArgs::ZeroOrMore)
    .require_equals()
    .value_delimiter(','),
  ArgDef::new("deny-import")
    .long("deny-import")
    .action(ArgAction::Append)
    .num_args(NumArgs::ZeroOrMore)
    .require_equals()
    .value_delimiter(','),
  ArgDef::new("no-prompt").long("no-prompt").set_true(),
  ArgDef::new("permission-set")
    .short('P')
    .long("permission-set")
    .action(ArgAction::Set)
    .num_args(NumArgs::Optional)
    .require_equals(),
];

pub static COMPILE_ARGS: &[ArgDef] = &[
  ArgDef::new("no-check")
    .long("no-check")
    .action(ArgAction::Set)
    .num_args(NumArgs::Optional)
    .require_equals(),
  ArgDef::new("import-map")
    .long("import-map")
    .action(ArgAction::Set)
    .num_args(NumArgs::Exact(1)),
  ArgDef::new("no-remote").long("no-remote").set_true(),
  ArgDef::new("no-npm").long("no-npm").set_true(),
  ArgDef::new("node-modules-dir")
    .long("node-modules-dir")
    .action(ArgAction::Set)
    .num_args(NumArgs::Optional)
    .require_equals(),
  ArgDef::new("vendor")
    .long("vendor")
    .action(ArgAction::Set)
    .num_args(NumArgs::Optional)
    .require_equals(),
  ArgDef::new("config")
    .short('c')
    .long("config")
    .action(ArgAction::Set)
    .num_args(NumArgs::Exact(1)),
  ArgDef::new("no-config").long("no-config").set_true(),
  ArgDef::new("reload")
    .short('r')
    .long("reload")
    .action(ArgAction::Append)
    .num_args(NumArgs::ZeroOrMore)
    .require_equals()
    .value_delimiter(','),
  ArgDef::new("lock")
    .long("lock")
    .action(ArgAction::Set)
    .num_args(NumArgs::Optional),
  ArgDef::new("no-lock").long("no-lock").set_true(),
  ArgDef::new("frozen-lockfile")
    .long("frozen-lockfile")
    .long_aliases(&["frozen"])
    .action(ArgAction::Set)
    .num_args(NumArgs::Optional)
    .require_equals(),
  ArgDef::new("cert")
    .long("cert")
    .action(ArgAction::Set)
    .num_args(NumArgs::Exact(1)),
  ArgDef::new("unsafely-ignore-certificate-errors")
    .long("unsafely-ignore-certificate-errors")
    .action(ArgAction::Append)
    .num_args(NumArgs::ZeroOrMore)
    .require_equals()
    .value_delimiter(','),
  ArgDef::new("min-dep-age")
    .long("min-dep-age")
    .long_aliases(&["minimum-dependency-age"])
    .action(ArgAction::Set)
    .num_args(NumArgs::Exact(1)),
];

pub static INSPECT_ARGS: &[ArgDef] = &[
  ArgDef::new("inspect")
    .long("inspect")
    .action(ArgAction::Set)
    .num_args(NumArgs::Optional)
    .require_equals(),
  ArgDef::new("inspect-brk")
    .long("inspect-brk")
    .action(ArgAction::Set)
    .num_args(NumArgs::Optional)
    .require_equals(),
  ArgDef::new("inspect-wait")
    .long("inspect-wait")
    .action(ArgAction::Set)
    .num_args(NumArgs::Optional)
    .require_equals(),
  ArgDef::new("inspect-publish-uid")
    .long("inspect-publish-uid")
    .action(ArgAction::Set)
    .num_args(NumArgs::Exact(1)),
];

pub static RUNTIME_MISC_ARGS: &[ArgDef] = &[
  ArgDef::new("cached-only").long("cached-only").set_true(),
  ArgDef::new("location")
    .long("location")
    .action(ArgAction::Set)
    .num_args(NumArgs::Exact(1)),
  ArgDef::new("v8-flags")
    .long("v8-flags")
    .action(ArgAction::Append)
    .num_args(NumArgs::ZeroOrMore)
    .require_equals()
    .value_delimiter(','),
  ArgDef::new("seed")
    .long("seed")
    .action(ArgAction::Set)
    .num_args(NumArgs::Exact(1)),
  ArgDef::new("enable-testing-features")
    .long("enable-testing-features-do-not-use")
    .long_aliases(&["enable-testing-features"])
    .set_true()
    .hidden(),
  ArgDef::new("trace-ops")
    .long("trace-ops")
    .action(ArgAction::Append)
    .num_args(NumArgs::ZeroOrMore)
    .require_equals()
    .value_delimiter(',')
    .hidden(),
  ArgDef::new("eszip-internal-do-not-use")
    .long("eszip-internal-do-not-use")
    .set_true()
    .hidden(),
  ArgDef::new("preload")
    .long("preload")
    .long_aliases(&["import"])
    .action(ArgAction::Append)
    .num_args(NumArgs::Exact(1)),
  ArgDef::new("require")
    .long("require")
    .action(ArgAction::Append)
    .num_args(NumArgs::Exact(1)),
  ArgDef::new("node-conditions")
    .long("conditions")
    .action(ArgAction::Append)
    .num_args(NumArgs::OneOrMore)
    .value_delimiter(','),
];

pub static CPU_PROF_ARGS: &[ArgDef] = &[
  ArgDef::new("cpu-prof").long("cpu-prof").set_true().hidden(),
  ArgDef::new("cpu-prof-dir")
    .long("cpu-prof-dir")
    .action(ArgAction::Set)
    .num_args(NumArgs::Exact(1))
    .hidden(),
  ArgDef::new("cpu-prof-name")
    .long("cpu-prof-name")
    .action(ArgAction::Set)
    .num_args(NumArgs::Exact(1))
    .hidden(),
  ArgDef::new("cpu-prof-interval")
    .long("cpu-prof-interval")
    .action(ArgAction::Set)
    .num_args(NumArgs::Exact(1))
    .hidden(),
  ArgDef::new("cpu-prof-md")
    .long("cpu-prof-md")
    .set_true()
    .hidden(),
  ArgDef::new("cpu-prof-flamegraph")
    .long("cpu-prof-flamegraph")
    .set_true()
    .hidden(),
];

// All unstable feature flags from runtime/features/gen.rs.
// Keep in sync with UNSTABLE_FEATURES.
pub static UNSTABLE_ARGS: &[ArgDef] = &[
  ArgDef::new("unstable").long("unstable").set_true().hidden(),
  ArgDef::new("unstable-bare-node-builtins")
    .long("unstable-bare-node-builtins")
    .set_true()
    .hidden(),
  ArgDef::new("unstable-broadcast-channel")
    .long("unstable-broadcast-channel")
    .set_true()
    .hidden(),
  ArgDef::new("unstable-bundle")
    .long("unstable-bundle")
    .set_true()
    .hidden(),
  ArgDef::new("unstable-byonm")
    .long("unstable-byonm")
    .set_true()
    .hidden(),
  ArgDef::new("unstable-cron")
    .long("unstable-cron")
    .set_true()
    .hidden(),
  ArgDef::new("unstable-detect-cjs")
    .long("unstable-detect-cjs")
    .set_true()
    .hidden(),
  ArgDef::new("unstable-ffi")
    .long("unstable-ffi")
    .set_true()
    .hidden(),
  ArgDef::new("unstable-fs")
    .long("unstable-fs")
    .set_true()
    .hidden(),
  ArgDef::new("unstable-http")
    .long("unstable-http")
    .set_true()
    .hidden(),
  ArgDef::new("unstable-kv")
    .long("unstable-kv")
    .set_true()
    .hidden(),
  ArgDef::new("unstable-lazy-dynamic-imports")
    .long("unstable-lazy-dynamic-imports")
    .set_true()
    .hidden(),
  ArgDef::new("unstable-lockfile-v5")
    .long("unstable-lockfile-v5")
    .set_true()
    .hidden(),
  ArgDef::new("unstable-net")
    .long("unstable-net")
    .set_true()
    .hidden(),
  ArgDef::new("unstable-no-legacy-abort")
    .long("unstable-no-legacy-abort")
    .set_true()
    .hidden(),
  ArgDef::new("unstable-node-globals")
    .long("unstable-node-globals")
    .set_true()
    .hidden(),
  ArgDef::new("unstable-npm-lazy-caching")
    .long("unstable-npm-lazy-caching")
    .set_true()
    .hidden(),
  ArgDef::new("unstable-otel")
    .long("unstable-otel")
    .set_true()
    .hidden(),
  ArgDef::new("unstable-process")
    .long("unstable-process")
    .set_true()
    .hidden(),
  ArgDef::new("unstable-raw-imports")
    .long("unstable-raw-imports")
    .set_true()
    .hidden(),
  ArgDef::new("unstable-sloppy-imports")
    .long("unstable-sloppy-imports")
    .long_aliases(&["sloppy-imports"])
    .set_true()
    .hidden(),
  ArgDef::new("unstable-subdomain-wildcards")
    .long("unstable-subdomain-wildcards")
    .set_true()
    .hidden(),
  ArgDef::new("unstable-temporal")
    .long("unstable-temporal")
    .set_true()
    .hidden(),
  ArgDef::new("unstable-tsgo")
    .long("unstable-tsgo")
    .set_true()
    .hidden(),
  ArgDef::new("unstable-unsafe-proto")
    .long("unstable-unsafe-proto")
    .set_true()
    .hidden(),
  ArgDef::new("unstable-vsock")
    .long("unstable-vsock")
    .set_true()
    .hidden(),
  ArgDef::new("unstable-webgpu")
    .long("unstable-webgpu")
    .set_true()
    .hidden(),
  ArgDef::new("unstable-worker-options")
    .long("unstable-worker-options")
    .set_true()
    .hidden(),
];

pub static ALLOW_SCRIPTS_ARG: &[ArgDef] = &[ArgDef::new("allow-scripts")
  .long("allow-scripts")
  .action(ArgAction::Append)
  .num_args(NumArgs::ZeroOrMore)
  .require_equals()
  .value_delimiter(',')];

pub static LOCK_ARGS: &[ArgDef] = &[
    // lock and no-lock are in COMPILE_ARGS, we reuse them via arg_groups
];

// ============================================================
// Subcommand definitions
// ============================================================

pub static RUN_SUBCOMMAND: CommandDef = CommandDef {
  name: "run",
  about: "Run a JavaScript or TypeScript program",
  aliases: &[],
  args: &[
    ArgDef::new("script_arg")
      .positional()
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1)),
    ArgDef::new("check")
      .long("check")
      .action(ArgAction::Set)
      .num_args(NumArgs::Optional)
      .require_equals(),
    ArgDef::new("watch")
      .long("watch")
      .action(ArgAction::Append)
      .num_args(NumArgs::ZeroOrMore)
      .require_equals()
      .value_delimiter(','),
    ArgDef::new("hmr")
      .long("hmr")
      .long_aliases(&["watch-hmr", "unstable-hmr"])
      .action(ArgAction::Append)
      .num_args(NumArgs::ZeroOrMore)
      .require_equals()
      .value_delimiter(','),
    ArgDef::new("watch-exclude")
      .long("watch-exclude")
      .action(ArgAction::Append)
      .num_args(NumArgs::ZeroOrMore)
      .require_equals()
      .value_delimiter(','),
    ArgDef::new("no-clear-screen")
      .long("no-clear-screen")
      .set_true(),
    ArgDef::new("ext")
      .long("ext")
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1)),
    ArgDef::new("env-file")
      .long("env-file")
      .long_aliases(&["env"])
      .action(ArgAction::Append)
      .num_args(NumArgs::Optional)
      .require_equals(),
    ArgDef::new("no-code-cache")
      .long("no-code-cache")
      .set_true(),
    ArgDef::new("coverage")
      .long("coverage")
      .action(ArgAction::Set)
      .num_args(NumArgs::Optional)
      .require_equals(),
    ArgDef::new("tunnel").long("tunnel").set_true().hidden(),
    // Allow --allow-scripts on run (through arg_groups, but also directly)
  ],
  arg_groups: &[
    UNSTABLE_ARGS,
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
};

pub static SERVE_SUBCOMMAND: CommandDef = CommandDef {
  name: "serve",
  about: "Run a server",
  aliases: &[],
  args: &[
    ArgDef::new("script_arg")
      .positional()
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1)),
    ArgDef::new("port")
      .long("port")
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1)),
    ArgDef::new("host")
      .long("host")
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1)),
    ArgDef::new("parallel").long("parallel").set_true(),
    ArgDef::new("check")
      .long("check")
      .action(ArgAction::Set)
      .num_args(NumArgs::Optional)
      .require_equals(),
    ArgDef::new("watch")
      .long("watch")
      .action(ArgAction::Append)
      .num_args(NumArgs::ZeroOrMore)
      .require_equals()
      .value_delimiter(','),
    ArgDef::new("hmr")
      .long("hmr")
      .action(ArgAction::Append)
      .num_args(NumArgs::ZeroOrMore)
      .require_equals()
      .value_delimiter(','),
    ArgDef::new("watch-exclude")
      .long("watch-exclude")
      .action(ArgAction::Append)
      .num_args(NumArgs::ZeroOrMore)
      .require_equals()
      .value_delimiter(','),
    ArgDef::new("no-clear-screen")
      .long("no-clear-screen")
      .set_true(),
    ArgDef::new("ext")
      .long("ext")
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1)),
    ArgDef::new("env-file")
      .long("env-file")
      .action(ArgAction::Append)
      .num_args(NumArgs::Optional)
      .require_equals(),
    ArgDef::new("no-code-cache")
      .long("no-code-cache")
      .set_true(),
  ],
  arg_groups: &[
    UNSTABLE_ARGS,
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
};

pub static EVAL_SUBCOMMAND: CommandDef = CommandDef {
  name: "eval",
  about: "Evaluate a script from the command line",
  aliases: &[],
  args: &[
    ArgDef::new("code_arg")
      .positional()
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1)),
    ArgDef::new("print").short('p').long("print").set_true(),
    ArgDef::new("ext")
      .long("ext")
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1)),
    ArgDef::new("env-file")
      .long("env-file")
      .long_aliases(&["env"])
      .action(ArgAction::Append)
      .num_args(NumArgs::Optional)
      .require_equals(),
  ],
  arg_groups: &[
    UNSTABLE_ARGS,
    PERMISSION_ARGS,
    COMPILE_ARGS,
    INSPECT_ARGS,
    RUNTIME_MISC_ARGS,
  ],
  subcommands: &[],
  default_subcommand: None,
  trailing_var_arg: true,
  passthrough: false,
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
    ArgDef::new("check").long("check").set_true(),
    ArgDef::new("fail-fast").long("fail-fast").set_true(),
    ArgDef::new("permit-no-files")
      .long("permit-no-files")
      .set_true(),
    ArgDef::new("watch")
      .long("watch")
      .action(ArgAction::Append)
      .num_args(NumArgs::ZeroOrMore)
      .require_equals()
      .value_delimiter(','),
    ArgDef::new("watch-exclude")
      .long("watch-exclude")
      .action(ArgAction::Append)
      .num_args(NumArgs::ZeroOrMore)
      .require_equals()
      .value_delimiter(','),
    ArgDef::new("no-clear-screen")
      .long("no-clear-screen")
      .set_true(),
    ArgDef::new("ext")
      .long("ext")
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1)),
    ArgDef::new("ignore")
      .long("ignore")
      .action(ArgAction::Append)
      .num_args(NumArgs::ZeroOrMore)
      .require_equals()
      .value_delimiter(','),
    ArgDef::new("use-tabs")
      .long("use-tabs")
      .action(ArgAction::Set)
      .num_args(NumArgs::Optional)
      .require_equals(),
    ArgDef::new("line-width")
      .long("line-width")
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1)),
    ArgDef::new("indent-width")
      .long("indent-width")
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1)),
    ArgDef::new("single-quote")
      .long("single-quote")
      .action(ArgAction::Set)
      .num_args(NumArgs::Optional)
      .require_equals(),
    ArgDef::new("prose-wrap")
      .long("prose-wrap")
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1)),
    ArgDef::new("no-semicolons")
      .long("no-semicolons")
      .action(ArgAction::Set)
      .num_args(NumArgs::Optional)
      .require_equals(),
    ArgDef::new("config")
      .short('c')
      .long("config")
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1)),
    ArgDef::new("no-config").long("no-config").set_true(),
    ArgDef::new("unstable-component")
      .long("unstable-component")
      .set_true(),
    ArgDef::new("unstable-sql").long("unstable-sql").set_true(),
    ArgDef::new("unstable-css")
      .long("unstable-css")
      .set_true()
      .hidden(),
    ArgDef::new("unstable-html")
      .long("unstable-html")
      .set_true()
      .hidden(),
    ArgDef::new("unstable-yaml")
      .long("unstable-yaml")
      .set_true()
      .hidden(),
  ],
  arg_groups: &[UNSTABLE_ARGS],
  subcommands: &[],
  default_subcommand: None,
  trailing_var_arg: false,
  passthrough: false,
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
    ArgDef::new("rules").long("rules").set_true(),
    ArgDef::new("fix").long("fix").set_true(),
    ArgDef::new("rules-tags")
      .long("rules-tags")
      .action(ArgAction::Append)
      .num_args(NumArgs::ZeroOrMore)
      .require_equals()
      .value_delimiter(','),
    ArgDef::new("rules-include")
      .long("rules-include")
      .action(ArgAction::Append)
      .num_args(NumArgs::ZeroOrMore)
      .require_equals()
      .value_delimiter(','),
    ArgDef::new("rules-exclude")
      .long("rules-exclude")
      .action(ArgAction::Append)
      .num_args(NumArgs::ZeroOrMore)
      .require_equals()
      .value_delimiter(','),
    ArgDef::new("json").long("json").set_true(),
    ArgDef::new("compact").long("compact").set_true(),
    ArgDef::new("ignore")
      .long("ignore")
      .action(ArgAction::Append)
      .num_args(NumArgs::ZeroOrMore)
      .require_equals()
      .value_delimiter(','),
    ArgDef::new("watch")
      .long("watch")
      .action(ArgAction::Append)
      .num_args(NumArgs::ZeroOrMore)
      .require_equals()
      .value_delimiter(','),
    ArgDef::new("watch-exclude")
      .long("watch-exclude")
      .action(ArgAction::Append)
      .num_args(NumArgs::ZeroOrMore)
      .require_equals()
      .value_delimiter(','),
    ArgDef::new("no-clear-screen")
      .long("no-clear-screen")
      .set_true(),
    ArgDef::new("permit-no-files")
      .long("permit-no-files")
      .set_true(),
    ArgDef::new("config")
      .short('c')
      .long("config")
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1)),
    ArgDef::new("no-config").long("no-config").set_true(),
  ],
  arg_groups: &[UNSTABLE_ARGS, PERMISSION_ARGS],
  subcommands: &[],
  default_subcommand: None,
  trailing_var_arg: false,
  passthrough: false,
};

pub static TEST_SUBCOMMAND: CommandDef = CommandDef {
  name: "test",
  about: "Run tests",
  aliases: &[],
  args: &[
    ArgDef::new("files")
      .positional()
      .action(ArgAction::Append)
      .num_args(NumArgs::ZeroOrMore),
    ArgDef::new("doc").long("doc").set_true(),
    ArgDef::new("no-run").long("no-run").set_true(),
    ArgDef::new("coverage")
      .long("coverage")
      .action(ArgAction::Set)
      .num_args(NumArgs::Optional)
      .require_equals(),
    ArgDef::new("clean").long("clean").set_true(),
    ArgDef::new("fail-fast")
      .long("fail-fast")
      .action(ArgAction::Set)
      .num_args(NumArgs::Optional)
      .require_equals(),
    ArgDef::new("filter")
      .long("filter")
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1)),
    ArgDef::new("shuffle")
      .long("shuffle")
      .action(ArgAction::Set)
      .num_args(NumArgs::Optional)
      .require_equals(),
    ArgDef::new("parallel").long("parallel").set_true(),
    ArgDef::new("trace-leaks")
      .long("trace-leaks")
      .set_true()
      .hidden(),
    ArgDef::new("watch")
      .long("watch")
      .action(ArgAction::Append)
      .num_args(NumArgs::ZeroOrMore)
      .require_equals()
      .value_delimiter(','),
    ArgDef::new("watch-exclude")
      .long("watch-exclude")
      .action(ArgAction::Append)
      .num_args(NumArgs::ZeroOrMore)
      .require_equals()
      .value_delimiter(','),
    ArgDef::new("no-clear-screen")
      .long("no-clear-screen")
      .set_true(),
    ArgDef::new("reporter")
      .long("reporter")
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1)),
    ArgDef::new("junit-path")
      .long("junit-path")
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1)),
    ArgDef::new("hide-stacktraces")
      .long("hide-stacktraces")
      .set_true(),
    ArgDef::new("coverage-raw-data-only")
      .long("coverage-raw-data-only")
      .set_true(),
    ArgDef::new("ignore")
      .long("ignore")
      .action(ArgAction::Append)
      .num_args(NumArgs::ZeroOrMore)
      .require_equals()
      .value_delimiter(','),
    ArgDef::new("env-file")
      .long("env-file")
      .action(ArgAction::Append)
      .num_args(NumArgs::Optional)
      .require_equals(),
    ArgDef::new("permit-no-files")
      .long("permit-no-files")
      .set_true(),
    ArgDef::new("ext")
      .long("ext")
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1)),
  ],
  arg_groups: &[
    UNSTABLE_ARGS,
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
};

pub static UPGRADE_SUBCOMMAND: CommandDef = CommandDef {
  name: "upgrade",
  about: "Upgrade deno executable",
  aliases: &[],
  args: &[
    ArgDef::new("dry-run").long("dry-run").set_true(),
    ArgDef::new("force").short('f').long("force").set_true(),
    ArgDef::new("canary").long("canary").set_true(),
    ArgDef::new("release-candidate")
      .long("release-candidate")
      .long_aliases(&["rc"])
      .set_true(),
    ArgDef::new("version")
      .long("version")
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1)),
    ArgDef::new("output")
      .short('o')
      .long("output")
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1)),
    ArgDef::new("cert")
      .long("cert")
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1)),
    ArgDef::new("version-or-hash-or-channel")
      .positional()
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1)),
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
      .num_args(NumArgs::Exact(1)),
    ArgDef::new("branch")
      .long("branch")
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1)),
  ],
  arg_groups: &[UNSTABLE_ARGS],
  subcommands: &[],
  default_subcommand: None,
  trailing_var_arg: false,
  passthrough: false,
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
      .require_equals(),
    ArgDef::new("ext")
      .long("ext")
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1)),
    ArgDef::new("env-file")
      .long("env-file")
      .action(ArgAction::Append)
      .num_args(NumArgs::Optional)
      .require_equals(),
  ],
  arg_groups: &[
    UNSTABLE_ARGS,
    COMPILE_ARGS,
    INSPECT_ARGS,
    PERMISSION_ARGS,
    ALLOW_SCRIPTS_ARG,
  ],
  subcommands: &[],
  default_subcommand: None,
  trailing_var_arg: false,
  passthrough: false,
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
    ArgDef::new("all").long("all").set_true(),
    ArgDef::new("remote").long("remote").set_true(),
    ArgDef::new("doc").long("doc").set_true(),
    ArgDef::new("doc-only").long("doc-only").set_true(),
    ArgDef::new("check-js").long("check-js").set_true().hidden(),
  ],
  arg_groups: &[UNSTABLE_ARGS, COMPILE_ARGS, RUNTIME_MISC_ARGS],
  subcommands: &[],
  default_subcommand: None,
  trailing_var_arg: false,
  passthrough: false,
};

pub static INFO_SUBCOMMAND: CommandDef = CommandDef {
  name: "info",
  about: "Show info about cache or a file",
  aliases: &[],
  args: &[
    ArgDef::new("file")
      .positional()
      .action(ArgAction::Set)
      .num_args(NumArgs::Optional),
    ArgDef::new("json").long("json").set_true(),
    ArgDef::new("location")
      .long("location")
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1)),
  ],
  arg_groups: &[UNSTABLE_ARGS, COMPILE_ARGS, PERMISSION_ARGS],
  subcommands: &[],
  default_subcommand: None,
  trailing_var_arg: false,
  passthrough: false,
};

pub static DOC_SUBCOMMAND: CommandDef = CommandDef {
  name: "doc",
  about: "Show documentation",
  aliases: &[],
  args: &[
    ArgDef::new("source_file")
      .positional()
      .action(ArgAction::Append)
      .num_args(NumArgs::ZeroOrMore),
    ArgDef::new("json").long("json").set_true(),
    ArgDef::new("private").long("private").set_true(),
    ArgDef::new("lint").long("lint").set_true(),
    ArgDef::new("html").long("html").set_true(),
    ArgDef::new("name")
      .long("name")
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1)),
    ArgDef::new("output")
      .long("output")
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1)),
    ArgDef::new("category-docs")
      .long("category-docs")
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1)),
    ArgDef::new("symbol-redirect-map")
      .long("symbol-redirect-map")
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1)),
    ArgDef::new("default-symbol-map")
      .long("default-symbol-map")
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1)),
    ArgDef::new("strip-trailing-html")
      .long("strip-trailing-html")
      .set_true(),
    ArgDef::new("filter")
      .long("filter")
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1)),
    ArgDef::new("builtin").long("builtin").set_true(),
  ],
  arg_groups: &[UNSTABLE_ARGS, COMPILE_ARGS],
  subcommands: &[],
  default_subcommand: None,
  trailing_var_arg: false,
  passthrough: false,
};

pub static TASK_SUBCOMMAND: CommandDef = CommandDef {
  name: "task",
  about: "Run a task",
  aliases: &[],
  args: &[
    ArgDef::new("task_name")
      .positional()
      .action(ArgAction::Set)
      .num_args(NumArgs::Optional),
    ArgDef::new("cwd")
      .long("cwd")
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1)),
    ArgDef::new("config")
      .short('c')
      .long("config")
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1)),
    ArgDef::new("recursive")
      .short('r')
      .long("recursive")
      .set_true(),
    ArgDef::new("filter")
      .short('f')
      .long("filter")
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1)),
    ArgDef::new("eval").long("eval").set_true(),
    ArgDef::new("node-modules-dir")
      .long("node-modules-dir")
      .action(ArgAction::Set)
      .num_args(NumArgs::Optional)
      .require_equals(),
    ArgDef::new("tunnel").long("tunnel").set_true().hidden(),
    ArgDef::new("lock")
      .long("lock")
      .action(ArgAction::Set)
      .num_args(NumArgs::Optional),
    ArgDef::new("no-lock").long("no-lock").set_true(),
    ArgDef::new("frozen-lockfile")
      .long("frozen-lockfile")
      .long_aliases(&["frozen"])
      .action(ArgAction::Set)
      .num_args(NumArgs::Optional)
      .require_equals(),
  ],
  arg_groups: &[UNSTABLE_ARGS],
  subcommands: &[],
  default_subcommand: None,
  trailing_var_arg: true,
  passthrough: false,
};

pub static BENCH_SUBCOMMAND: CommandDef = CommandDef {
  name: "bench",
  about: "Run benchmarks",
  aliases: &[],
  args: &[
    ArgDef::new("files")
      .positional()
      .action(ArgAction::Append)
      .num_args(NumArgs::ZeroOrMore),
    ArgDef::new("filter")
      .long("filter")
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1)),
    ArgDef::new("json").long("json").set_true(),
    ArgDef::new("no-run").long("no-run").set_true(),
    ArgDef::new("permit-no-files")
      .long("permit-no-files")
      .set_true(),
    ArgDef::new("watch")
      .long("watch")
      .action(ArgAction::Append)
      .num_args(NumArgs::ZeroOrMore)
      .require_equals()
      .value_delimiter(','),
    ArgDef::new("watch-exclude")
      .long("watch-exclude")
      .action(ArgAction::Append)
      .num_args(NumArgs::ZeroOrMore)
      .require_equals()
      .value_delimiter(','),
    ArgDef::new("no-clear-screen")
      .long("no-clear-screen")
      .set_true(),
    ArgDef::new("ignore")
      .long("ignore")
      .action(ArgAction::Append)
      .num_args(NumArgs::ZeroOrMore)
      .require_equals()
      .value_delimiter(','),
    ArgDef::new("env-file")
      .long("env-file")
      .action(ArgAction::Append)
      .num_args(NumArgs::Optional)
      .require_equals(),
  ],
  arg_groups: &[
    UNSTABLE_ARGS,
    PERMISSION_ARGS,
    COMPILE_ARGS,
    INSPECT_ARGS,
    RUNTIME_MISC_ARGS,
  ],
  subcommands: &[],
  default_subcommand: None,
  trailing_var_arg: true,
  passthrough: false,
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
      .num_args(NumArgs::Exact(1)),
    ArgDef::new("target")
      .long("target")
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1)),
    ArgDef::new("no-terminal").long("no-terminal").set_true(),
    ArgDef::new("icon")
      .long("icon")
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1)),
    ArgDef::new("include")
      .long("include")
      .action(ArgAction::Append)
      .num_args(NumArgs::Exact(1)),
    ArgDef::new("exclude")
      .long("exclude")
      .action(ArgAction::Append)
      .num_args(NumArgs::Exact(1)),
    ArgDef::new("env-file")
      .long("env-file")
      .long_aliases(&["env"])
      .action(ArgAction::Append)
      .num_args(NumArgs::Optional)
      .require_equals(),
    ArgDef::new("no-code-cache")
      .long("no-code-cache")
      .set_true(),
    ArgDef::new("ext")
      .long("ext")
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1)),
    ArgDef::new("self-extracting")
      .long("self-extracting")
      .set_true(),
  ],
  arg_groups: &[
    UNSTABLE_ARGS,
    PERMISSION_ARGS,
    COMPILE_ARGS,
    INSPECT_ARGS,
    RUNTIME_MISC_ARGS,
  ],
  subcommands: &[],
  default_subcommand: None,
  trailing_var_arg: true,
  passthrough: false,
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
      .value_delimiter(','),
    ArgDef::new("include")
      .long("include")
      .action(ArgAction::Append)
      .num_args(NumArgs::ZeroOrMore)
      .require_equals()
      .value_delimiter(','),
    ArgDef::new("exclude")
      .long("exclude")
      .action(ArgAction::Append)
      .num_args(NumArgs::ZeroOrMore)
      .require_equals()
      .value_delimiter(','),
    ArgDef::new("lcov").long("lcov").set_true(),
    ArgDef::new("html").long("html").set_true(),
    ArgDef::new("detailed").long("detailed").set_true(),
    ArgDef::new("output")
      .long("output")
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1)),
  ],
  arg_groups: &[UNSTABLE_ARGS],
  subcommands: &[],
  default_subcommand: None,
  trailing_var_arg: false,
  passthrough: false,
};

pub static REPL_SUBCOMMAND: CommandDef = CommandDef {
  name: "repl",
  about: "Read Eval Print Loop",
  aliases: &[],
  args: &[
    ArgDef::new("eval")
      .long("eval")
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1)),
    ArgDef::new("eval-file")
      .long("eval-file")
      .action(ArgAction::Append)
      .num_args(NumArgs::OneOrMore)
      .value_delimiter(','),
    ArgDef::new("json").long("json").set_true(),
    ArgDef::new("env-file")
      .long("env-file")
      .long_aliases(&["env"])
      .action(ArgAction::Append)
      .num_args(NumArgs::Optional)
      .require_equals(),
  ],
  arg_groups: &[
    UNSTABLE_ARGS,
    PERMISSION_ARGS,
    COMPILE_ARGS,
    INSPECT_ARGS,
    RUNTIME_MISC_ARGS,
  ],
  subcommands: &[],
  default_subcommand: None,
  trailing_var_arg: false,
  passthrough: false,
};

pub static INSTALL_SUBCOMMAND: CommandDef = CommandDef {
  name: "install",
  about: "Install dependencies",
  aliases: &["i"],
  args: &[
    ArgDef::new("cmd")
      .positional()
      .action(ArgAction::Append)
      .num_args(NumArgs::ZeroOrMore),
    ArgDef::new("global").short('g').long("global").set_true(),
    ArgDef::new("name")
      .short('n')
      .long("name")
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1)),
    ArgDef::new("root")
      .long("root")
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1)),
    ArgDef::new("force").short('f').long("force").set_true(),
    ArgDef::new("dev").short('D').long("dev").set_true(),
    ArgDef::new("entrypoint")
      .short('e')
      .long("entrypoint")
      .action(ArgAction::Append)
      .num_args(NumArgs::OneOrMore),
    ArgDef::new("compile").long("compile").set_true(),
    ArgDef::new("lockfile-only")
      .long("lockfile-only")
      .set_true(),
    ArgDef::new("npm").long("npm").set_true(),
    ArgDef::new("jsr").long("jsr").set_true(),
    ArgDef::new("save-exact")
      .long("save-exact")
      .long_aliases(&["exact"])
      .set_true(),
    ArgDef::new("env-file")
      .long("env-file")
      .long_aliases(&["env"])
      .action(ArgAction::Append)
      .num_args(NumArgs::Optional)
      .require_equals(),
    ArgDef::new("check")
      .long("check")
      .action(ArgAction::Set)
      .num_args(NumArgs::Optional)
      .require_equals(),
  ],
  arg_groups: &[
    UNSTABLE_ARGS,
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
};

pub static UNINSTALL_SUBCOMMAND: CommandDef = CommandDef {
  name: "uninstall",
  about: "Uninstall a script or dependency",
  aliases: &[],
  args: &[
    ArgDef::new("packages")
      .positional()
      .action(ArgAction::Append)
      .num_args(NumArgs::ZeroOrMore),
    ArgDef::new("global").short('g').long("global").set_true(),
    ArgDef::new("root")
      .long("root")
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1)),
    ArgDef::new("lockfile-only")
      .long("lockfile-only")
      .set_true(),
  ],
  arg_groups: &[UNSTABLE_ARGS, COMPILE_ARGS],
  subcommands: &[],
  default_subcommand: None,
  trailing_var_arg: false,
  passthrough: false,
};

pub static TYPES_SUBCOMMAND: CommandDef = CommandDef {
  name: "types",
  about: "Print runtime TypeScript declarations",
  aliases: &[],
  args: &[],
  arg_groups: &[UNSTABLE_ARGS],
  subcommands: &[],
  default_subcommand: None,
  trailing_var_arg: false,
  passthrough: false,
};

pub static COMPLETIONS_SUBCOMMAND: CommandDef = CommandDef {
  name: "completions",
  about: "Generate shell completions",
  aliases: &[],
  args: &[ArgDef::new("shell")
    .positional()
    .action(ArgAction::Set)
    .num_args(NumArgs::Exact(1))],
  arg_groups: &[UNSTABLE_ARGS],
  subcommands: &[],
  default_subcommand: None,
  trailing_var_arg: false,
  passthrough: false,
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
    ArgDef::new("lib").long("lib").set_true(),
    ArgDef::new("serve").long("serve").set_true(),
    ArgDef::new("npm").long("npm").set_true(),
    ArgDef::new("jsr").long("jsr").set_true(),
    ArgDef::new("empty").long("empty").set_true(),
    ArgDef::new("yes").short('y').long("yes").set_true(),
  ],
  arg_groups: &[],
  subcommands: &[],
  default_subcommand: None,
  trailing_var_arg: true,
  passthrough: false,
};

pub static CREATE_SUBCOMMAND: CommandDef = CommandDef {
  name: "create",
  about: "Create a project from a template",
  aliases: &[],
  args: &[
    ArgDef::new("package")
      .positional()
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1)),
    ArgDef::new("package_args")
      .positional()
      .action(ArgAction::Append)
      .num_args(NumArgs::ZeroOrMore)
      .trailing(),
    ArgDef::new("npm").long("npm").set_true(),
    ArgDef::new("jsr").long("jsr").set_true(),
    ArgDef::new("yes").short('y').long("yes").set_true(),
  ],
  arg_groups: &[],
  subcommands: &[],
  default_subcommand: None,
  trailing_var_arg: true,
  passthrough: false,
};

pub static JUPYTER_SUBCOMMAND: CommandDef = CommandDef {
  name: "jupyter",
  about: "Jupyter kernel",
  aliases: &[],
  args: &[
    ArgDef::new("install").long("install").set_true(),
    ArgDef::new("name")
      .short('n')
      .long("name")
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1)),
    ArgDef::new("display")
      .long("display")
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1)),
    ArgDef::new("kernel").long("kernel").set_true(),
    ArgDef::new("conn")
      .long("conn")
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1)),
    ArgDef::new("force").long("force").set_true(),
  ],
  arg_groups: &[
    COMPILE_ARGS,
    RUNTIME_MISC_ARGS,
    PERMISSION_ARGS,
    UNSTABLE_ARGS,
  ],
  subcommands: &[],
  default_subcommand: None,
  trailing_var_arg: false,
  passthrough: false,
};

pub static PUBLISH_SUBCOMMAND: CommandDef = CommandDef {
  name: "publish",
  about: "Publish a package",
  aliases: &[],
  args: &[
    ArgDef::new("token")
      .long("token")
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1)),
    ArgDef::new("dry-run").long("dry-run").set_true(),
    ArgDef::new("allow-slow-types")
      .long("allow-slow-types")
      .set_true(),
    ArgDef::new("allow-dirty").long("allow-dirty").set_true(),
    ArgDef::new("no-provenance")
      .long("no-provenance")
      .set_true(),
    ArgDef::new("set-version")
      .long("set-version")
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1)),
  ],
  arg_groups: &[UNSTABLE_ARGS, COMPILE_ARGS],
  subcommands: &[],
  default_subcommand: None,
  trailing_var_arg: false,
  passthrough: false,
};

pub static ADD_SUBCOMMAND: CommandDef = CommandDef {
  name: "add",
  about: "Add dependencies",
  aliases: &[],
  args: &[
    ArgDef::new("packages")
      .positional()
      .action(ArgAction::Append)
      .num_args(NumArgs::OneOrMore),
    ArgDef::new("dev").short('D').long("dev").set_true(),
    ArgDef::new("save-exact")
      .long("save-exact")
      .long_aliases(&["exact"])
      .set_true(),
    ArgDef::new("npm").long("npm").set_true(),
    ArgDef::new("jsr").long("jsr").set_true(),
    ArgDef::new("lockfile-only")
      .long("lockfile-only")
      .set_true(),
  ],
  arg_groups: &[UNSTABLE_ARGS, ALLOW_SCRIPTS_ARG, COMPILE_ARGS],
  subcommands: &[],
  default_subcommand: None,
  trailing_var_arg: false,
  passthrough: false,
};

pub static REMOVE_SUBCOMMAND: CommandDef = CommandDef {
  name: "remove",
  about: "Remove dependencies",
  aliases: &["rm"],
  args: &[
    ArgDef::new("packages")
      .positional()
      .action(ArgAction::Append)
      .num_args(NumArgs::OneOrMore),
    ArgDef::new("lockfile-only")
      .long("lockfile-only")
      .set_true(),
  ],
  arg_groups: &[UNSTABLE_ARGS, COMPILE_ARGS],
  subcommands: &[],
  default_subcommand: None,
  trailing_var_arg: false,
  passthrough: false,
};

pub static OUTDATED_SUBCOMMAND: CommandDef = CommandDef {
  name: "outdated",
  about: "Find outdated dependencies",
  aliases: &[],
  args: &[
    ArgDef::new("filters")
      .positional()
      .action(ArgAction::Append)
      .num_args(NumArgs::ZeroOrMore),
    ArgDef::new("recursive")
      .short('r')
      .long("recursive")
      .set_true(),
    ArgDef::new("compatible").long("compatible").set_true(),
    ArgDef::new("update").long("update").short('u').set_true(),
    ArgDef::new("latest").long("latest").set_true(),
    ArgDef::new("interactive")
      .short('i')
      .long("interactive")
      .set_true(),
    ArgDef::new("lockfile-only")
      .long("lockfile-only")
      .set_true(),
  ],
  arg_groups: &[UNSTABLE_ARGS, COMPILE_ARGS],
  subcommands: &[],
  default_subcommand: None,
  trailing_var_arg: false,
  passthrough: false,
};

pub static UPDATE_SUBCOMMAND: CommandDef = CommandDef {
  name: "update",
  about: "Update outdated dependencies",
  aliases: &[],
  args: &[
    ArgDef::new("filters")
      .positional()
      .action(ArgAction::Append)
      .num_args(NumArgs::ZeroOrMore),
    ArgDef::new("recursive")
      .short('r')
      .long("recursive")
      .set_true(),
    ArgDef::new("latest").long("latest").set_true(),
    ArgDef::new("lockfile-only")
      .long("lockfile-only")
      .set_true(),
    ArgDef::new("interactive")
      .short('i')
      .long("interactive")
      .set_true(),
  ],
  arg_groups: &[UNSTABLE_ARGS, COMPILE_ARGS],
  subcommands: &[],
  default_subcommand: None,
  trailing_var_arg: false,
  passthrough: false,
};

pub static DEPLOY_SUBCOMMAND: CommandDef = CommandDef {
  name: "deploy",
  about: "Deploy to Deno Deploy",
  aliases: &[],
  args: &[],
  arg_groups: &[UNSTABLE_ARGS],
  subcommands: &[],
  default_subcommand: None,
  trailing_var_arg: false,
  passthrough: true,
};

pub static SANDBOX_SUBCOMMAND: CommandDef = CommandDef {
  name: "sandbox",
  about: "Run in sandbox mode",
  aliases: &[],
  args: &[],
  arg_groups: &[UNSTABLE_ARGS],
  subcommands: &[],
  default_subcommand: None,
  trailing_var_arg: false,
  passthrough: true,
};

pub static CLEAN_SUBCOMMAND: CommandDef = CommandDef {
  name: "clean",
  about: "Remove the cache directory",
  aliases: &[],
  args: &[
    ArgDef::new("except")
      .long("except")
      .action(ArgAction::Append)
      .num_args(NumArgs::ZeroOrMore)
      .value_delimiter(','),
    ArgDef::new("dry-run").long("dry-run").set_true(),
  ],
  arg_groups: &[UNSTABLE_ARGS],
  subcommands: &[],
  default_subcommand: None,
  trailing_var_arg: false,
  passthrough: false,
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
      .value_delimiter(','),
    ArgDef::new("lockfile-only")
      .long("lockfile-only")
      .set_true(),
  ],
  arg_groups: &[UNSTABLE_ARGS, COMPILE_ARGS],
  subcommands: &[],
  default_subcommand: None,
  trailing_var_arg: false,
  passthrough: false,
};

pub static LSP_SUBCOMMAND: CommandDef = CommandDef {
  name: "lsp",
  about: "Start the language server",
  aliases: &[],
  args: &[],
  arg_groups: &[UNSTABLE_ARGS],
  subcommands: &[],
  default_subcommand: None,
  trailing_var_arg: false,
  passthrough: false,
};

pub static VENDOR_SUBCOMMAND: CommandDef = CommandDef {
  name: "vendor",
  about: "Vendor remote modules",
  aliases: &[],
  args: &[],
  arg_groups: &[UNSTABLE_ARGS],
  subcommands: &[],
  default_subcommand: None,
  trailing_var_arg: false,
  passthrough: false,
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
      .num_args(NumArgs::Exact(1)),
    ArgDef::new("outdir")
      .long("outdir")
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1)),
    ArgDef::new("format")
      .long("format")
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1))
      .default_value("esm"),
    ArgDef::new("packages")
      .long("packages")
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1))
      .default_value("bundle"),
    ArgDef::new("platform")
      .long("platform")
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1))
      .default_value("deno"),
    ArgDef::new("sourcemap")
      .long("sourcemap")
      .action(ArgAction::Set)
      .num_args(NumArgs::Exact(1)),
    ArgDef::new("external")
      .long("external")
      .action(ArgAction::Append)
      .num_args(NumArgs::ZeroOrMore),
    ArgDef::new("watch").long("watch").set_true(),
    ArgDef::new("minify").long("minify").set_true(),
    ArgDef::new("keep-names").long("keep-names").set_true(),
    ArgDef::new("code-splitting")
      .long("code-splitting")
      .set_true(),
    ArgDef::new("inline-imports")
      .long("inline-imports")
      .set_true(),
  ],
  arg_groups: &[
    COMPILE_ARGS,
    UNSTABLE_ARGS,
    PERMISSION_ARGS,
    ALLOW_SCRIPTS_ARG,
  ],
  subcommands: &[],
  default_subcommand: None,
  trailing_var_arg: false,
  passthrough: false,
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
      .default_value("low"),
    ArgDef::new("ignore-unfixable")
      .long("ignore-unfixable")
      .set_true(),
    ArgDef::new("ignore-registry-errors")
      .long("ignore-registry-errors")
      .set_true(),
    ArgDef::new("socket").long("socket").set_true(),
    ArgDef::new("ignore")
      .long("ignore")
      .action(ArgAction::Append)
      .num_args(NumArgs::ZeroOrMore)
      .require_equals()
      .value_delimiter(','),
  ],
  arg_groups: &[UNSTABLE_ARGS, LOCK_ARGS],
  subcommands: &[],
  default_subcommand: None,
  trailing_var_arg: false,
  passthrough: false,
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
      .trailing(),
    ArgDef::new("yes").short('y').long("yes").set_true(),
    ArgDef::new("install-alias")
      .long("install-alias")
      .action(ArgAction::Set)
      .num_args(NumArgs::Optional)
      .default_value("dx"),
  ],
  arg_groups: &[
    UNSTABLE_ARGS,
    COMPILE_ARGS,
    PERMISSION_ARGS,
    RUNTIME_MISC_ARGS,
    INSPECT_ARGS,
    UNSTABLE_ARGS,
  ],
  subcommands: &[],
  default_subcommand: None,
  trailing_var_arg: true,
  passthrough: false,
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
};

// ============================================================
// Root command
// ============================================================

pub static GLOBAL_ARGS: &[ArgDef] = &[
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
    .global(),
  ArgDef::new("log-level")
    .short('L')
    .long("log-level")
    .action(ArgAction::Set)
    .num_args(NumArgs::Exact(1))
    .global(),
  ArgDef::new("quiet")
    .short('q')
    .long("quiet")
    .set_true()
    .global(),
];

pub static DENO_ROOT: CommandDef = CommandDef {
  name: "deno",
  about: "A modern JavaScript and TypeScript runtime",
  aliases: &[],
  args: GLOBAL_ARGS,
  arg_groups: &[UNSTABLE_ARGS],
  subcommands: &[
    RUN_SUBCOMMAND,
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
    APPROVE_SCRIPTS_SUBCOMMAND,
    LSP_SUBCOMMAND,
    VENDOR_SUBCOMMAND,
    BUNDLE_SUBCOMMAND,
    AUDIT_SUBCOMMAND,
    X_SUBCOMMAND,
    JSON_REFERENCE_SUBCOMMAND,
    HELP_SUBCOMMAND,
  ],
  default_subcommand: Some("run"),
  trailing_var_arg: false,
  passthrough: false,
};
