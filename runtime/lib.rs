// Copyright 2018-2025 the Deno authors. MIT license.

pub use deno_broadcast_channel;
pub use deno_cache;
pub use deno_canvas;
pub use deno_console;
pub use deno_core;
pub use deno_cron;
pub use deno_crypto;
pub use deno_fetch;
pub use deno_ffi;
pub use deno_fs;
pub use deno_http;
pub use deno_io;
pub use deno_kv;
pub use deno_napi;
pub use deno_net;
pub use deno_node;
pub use deno_os;
pub use deno_permissions;
pub use deno_process;
pub use deno_telemetry;
pub use deno_terminal::colors;
pub use deno_tls;
pub use deno_url;
pub use deno_web;
pub use deno_webgpu;
pub use deno_webidl;
pub use deno_websocket;
pub use deno_webstorage;

pub mod code_cache;
pub mod fmt_errors;
pub mod fs_util;
pub mod inspector_server;
pub mod js;
pub mod ops;
pub mod permissions;
#[cfg(feature = "snapshot")]
pub mod snapshot;
pub mod snapshot_info;
pub mod tokio_util;
#[cfg(feature = "transpile")]
pub mod transpile;
pub mod web_worker;
pub mod worker;

mod worker_bootstrap;
pub use worker_bootstrap::BootstrapOptions;
pub use worker_bootstrap::WorkerExecutionMode;
pub use worker_bootstrap::WorkerLogLevel;

pub mod shared;
pub use deno_os::exit;
pub use shared::runtime;

pub struct UnstableGranularFlag {
  pub name: &'static str,
  pub help_text: &'static str,
  pub show_in_help: bool,
  // id to enable it in runtime/99_main.js
  pub id: i32,
}

// NOTE(bartlomieju): keep IDs in sync with `runtime/js/90_deno_ns.js` (search for `unstableFeatures`)
pub static UNSTABLE_GRANULAR_FLAGS: &[UnstableGranularFlag] = &[
  UnstableGranularFlag {
    name: deno_broadcast_channel::UNSTABLE_FEATURE_NAME,
    help_text: "Enable unstable `BroadcastChannel` API",
    show_in_help: true,
    id: 1,
  },
  UnstableGranularFlag {
    name: deno_cron::UNSTABLE_FEATURE_NAME,
    help_text: "Enable unstable Deno.cron API",
    show_in_help: true,
    id: 2,
  },
  UnstableGranularFlag {
    name: deno_ffi::UNSTABLE_FEATURE_NAME,
    help_text: "Enable unstable FFI APIs",
    show_in_help: false,
    id: 3,
  },
  UnstableGranularFlag {
    name: deno_fs::UNSTABLE_FEATURE_NAME,
    help_text: "Enable unstable file system APIs",
    show_in_help: false,
    id: 4,
  },
  UnstableGranularFlag {
    name: ops::http::UNSTABLE_FEATURE_NAME,
    help_text: "Enable unstable HTTP APIs",
    show_in_help: false,
    id: 5,
  },
  UnstableGranularFlag {
    name: deno_kv::UNSTABLE_FEATURE_NAME,
    help_text: "Enable unstable Key-Value store APIs",
    show_in_help: true,
    id: 6,
  },
  UnstableGranularFlag {
    name: deno_net::UNSTABLE_FEATURE_NAME,
    help_text: "Enable unstable net APIs",
    show_in_help: true,
    id: 7,
  },
  UnstableGranularFlag {
    name: "no-legacy-abort",
    help_text: "Enable abort signal in Deno.serve without legacy behavior. This will not abort the server when the request is handled successfully.",
    show_in_help: true,
    id: 8,
  },
  UnstableGranularFlag {
    name: "node-globals",
    help_text: "Expose Node globals everywhere",
    show_in_help: true,
    id: 9,
  },
  UnstableGranularFlag {
    name: "otel",
    help_text: "Enable unstable OpenTelemetry features",
    show_in_help: false,
    id: 10,
  },
  // TODO(bartlomieju): consider removing it
  UnstableGranularFlag {
    name: deno_process::UNSTABLE_FEATURE_NAME,
    help_text: "Enable unstable process APIs",
    show_in_help: false,
    id: 11,
  },
  UnstableGranularFlag {
    name: "temporal",
    help_text: "Enable unstable Temporal API",
    show_in_help: true,
    id: 12,
  },
  UnstableGranularFlag {
    name: "unsafe-proto",
    help_text: "Enable unsafe __proto__ support. This is a security risk.",
    show_in_help: true,
    // This number is used directly in the JS code. Search
    // for "unstableIds" to see where it's used.
    id: 13,
  },
  UnstableGranularFlag {
    name: "vsock",
    help_text: "Enable unstable VSOCK APIs",
    show_in_help: false,
    id: 14,
  },
  UnstableGranularFlag {
    name: deno_webgpu::UNSTABLE_FEATURE_NAME,
    help_text: "Enable unstable `WebGPU` APIs",
    show_in_help: true,
    id: 15,
  },
  UnstableGranularFlag {
    name: ops::worker_host::UNSTABLE_FEATURE_NAME,
    help_text: "Enable unstable Web Worker APIs",
    show_in_help: true,
    id: 16,
  },
];

#[cfg(test)]
mod test {
  use super::*;

  #[test]
  fn unstable_granular_flag_names_sorted() {
    let flags = UNSTABLE_GRANULAR_FLAGS
      .iter()
      .map(|granular_flag| granular_flag.name.to_string())
      .collect::<Vec<_>>();
    let mut sorted_flags = flags.clone();
    sorted_flags.sort();
    // sort the flags by name so they appear nicely in the help text
    assert_eq!(flags, sorted_flags);
  }
}
