// Copyright 2018-2025 the Deno authors. MIT license.

// NOTE(bartlomieju): some fields are marked as never read, even though they are
// actually used in the CLI.
#![allow(dead_code)]

use crate::structs::UnstableFeatureKind;

#[derive(Clone, Debug)]
pub struct UnstableFeatureDescription {
  pub name: &'static str,
  pub help_text: &'static str,
  // TODO(bartlomieju): is it needed?
  pub show_in_help: bool,
  pub kind: UnstableFeatureKind,
  pub env_var: Option<&'static str>,
}

pub static FEATURE_DESCRIPTIONS: &[UnstableFeatureDescription] = &[
  UnstableFeatureDescription {
    name: "bare-node-builtins",
    help_text: "Enable unstable bare node builtins feature",
    show_in_help: true,
    kind: UnstableFeatureKind::Cli,
    env_var: Some("DENO_UNSTABLE_BARE_NODE_BUILTINS"),
  },
  UnstableFeatureDescription {
    name: "broadcast-channel",
    help_text: "Enable unstable `BroadcastChannel` API",
    show_in_help: false,
    kind: UnstableFeatureKind::Runtime,
    env_var: None,
  },
  UnstableFeatureDescription {
    name: "byonm",
    help_text: "",
    show_in_help: false,
    kind: UnstableFeatureKind::Cli,
    env_var: None,
  },
  UnstableFeatureDescription {
    name: "cron",
    help_text: "Enable unstable `Deno.cron` API",
    show_in_help: true,
    kind: UnstableFeatureKind::Runtime,
    env_var: None,
  },
  UnstableFeatureDescription {
    name: "detect-cjs",
    help_text: "Treats ambiguous .js, .jsx, .ts, .tsx files as CommonJS modules in more cases",
    show_in_help: true,
    kind: UnstableFeatureKind::Cli,
    env_var: None,
  },
  UnstableFeatureDescription {
    name: "ffi",
    help_text: "Enable unstable FFI APIs",
    show_in_help: false,
    kind: UnstableFeatureKind::Runtime,
    env_var: None,
  },
  UnstableFeatureDescription {
    name: "fs",
    help_text: "Enable unstable file system APIs",
    show_in_help: false,
    kind: UnstableFeatureKind::Runtime,
    env_var: None,
  },
  UnstableFeatureDescription {
    name: "http",
    help_text: "Enable unstable HTTP APIs",
    show_in_help: false,
    kind: UnstableFeatureKind::Runtime,
    env_var: None,
  },
  UnstableFeatureDescription {
    name: "kv",
    help_text: "Enable unstable KV APIs",
    show_in_help: true,
    kind: UnstableFeatureKind::Runtime,
    env_var: None,
  },
  UnstableFeatureDescription {
    name: "lazy-dynamic-imports",
    help_text: "Lazily loads statically analyzable dynamic imports when not running with type checking. Warning: This may change the order of semver specifier resolution.",
    show_in_help: true,
    kind: UnstableFeatureKind::Cli,
    env_var: Some("DENO_UNSTABLE_LAZY_DYNAMIC_IMPORTS"),
  },
  UnstableFeatureDescription {
    name: "lockfile-v5",
    help_text: "Enable unstable lockfile v5",
    show_in_help: true,
    kind: UnstableFeatureKind::Cli,
    env_var: Some("DENO_UNSTABLE_LOCKFILE_V5"),
  },
  UnstableFeatureDescription {
    name: "net",
    help_text: "enable unstable net APIs",
    show_in_help: true,
    kind: UnstableFeatureKind::Runtime,
    env_var: None,
  },
  UnstableFeatureDescription {
    name: "no-legacy-abort",
    help_text: "Enable abort signal in Deno.serve without legacy behavior. This will not abort the server when the request is handled successfully.",
    show_in_help: true,
    kind: UnstableFeatureKind::Runtime,
    env_var: None,
  },
  UnstableFeatureDescription {
    name: "node-globals",
    help_text: "Prefer Node.js globals over Deno globals - currently this refers to `setTimeout` and `setInterval` APIs.",
    show_in_help: true,
    kind: UnstableFeatureKind::Runtime,
    env_var: None,
  },
  UnstableFeatureDescription {
    name: "npm-lazy-caching",
    help_text: "Enable unstable lazy caching of npm dependencies, downloading them only as needed (disabled: all npm packages in package.json are installed on startup; enabled: only npm packages that are actually referenced in an import are installed",
    show_in_help: true,
    kind: UnstableFeatureKind::Cli,
    env_var: Some("DENO_UNSTABLE_NPM_LAZY_CACHING"),
  },
  UnstableFeatureDescription {
    name: "otel",
    help_text: "Enable unstable OpenTelemetry features",
    show_in_help: false,
    kind: UnstableFeatureKind::Runtime,
    env_var: None,
  },
  UnstableFeatureDescription {
    name: "process",
    help_text: "Enable unstable process APIs",
    show_in_help: false,
    kind: UnstableFeatureKind::Runtime,
    env_var: None,
  },
  UnstableFeatureDescription {
    name: "raw-imports",
    help_text: "Enable unstable 'bytes' and 'text' imports.",
    show_in_help: true,
    kind: UnstableFeatureKind::Runtime,
    env_var: Some("DENO_UNSTABLE_RAW_IMPORTS"),
  },
  UnstableFeatureDescription {
    name: "sloppy-imports",
    help_text: "Enable unstable resolving of specifiers by extension probing, .js to .ts, and directory probing",
    show_in_help: true,
    kind: UnstableFeatureKind::Cli,
    env_var: Some("DENO_UNSTABLE_SLOPPY_IMPORTS"),
  },
  UnstableFeatureDescription {
    name: "subdomain-wildcards",
    help_text: "Enable subdomain wildcards support for the `--allow-net` flag",
    show_in_help: false,
    kind: UnstableFeatureKind::Cli,
    env_var: Some("DENO_UNSTABLE_SUBDOMAIN_WILDCARDS"),
  },
  UnstableFeatureDescription {
    name: "temporal",
    help_text: "Enable unstable Temporal API",
    show_in_help: true,
    kind: UnstableFeatureKind::Runtime,
    env_var: None,
  },
  UnstableFeatureDescription {
    name: "unsafe-proto",
    help_text: "Enable unsafe __proto__ support. This is a security risk.",
    show_in_help: true,
    kind: UnstableFeatureKind::Runtime,
    env_var: None,
  },
  UnstableFeatureDescription {
    name: "vsock",
    help_text: "Enable unstable VSOCK APIs",
    show_in_help: false,
    kind: UnstableFeatureKind::Runtime,
    env_var: None,
  },
  UnstableFeatureDescription {
    name: "webgpu",
    help_text: "Enable unstable WebGPU APIs",
    show_in_help: true,
    kind: UnstableFeatureKind::Runtime,
    env_var: None,
  },
  UnstableFeatureDescription {
    name: "worker-options",
    help_text: "Enable unstable Web Worker APIs",
    show_in_help: true,
    kind: UnstableFeatureKind::Runtime,
    env_var: None,
  },
  UnstableFeatureDescription {
    name: "bundle",
    help_text: "Enable unstable bundle runtime API",
    show_in_help: true,
    kind: UnstableFeatureKind::Runtime,
    env_var: None,
  },
  UnstableFeatureDescription {
    name: "tsgo",
    help_text: "Enable unstable TypeScript Go integration",
    show_in_help: true,
    kind: UnstableFeatureKind::Cli,
    env_var: Some("DENO_UNSTABLE_TSGO"),
  },
];
