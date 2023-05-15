// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

pub use deno_broadcast_channel;
pub use deno_cache;
pub use deno_console;
pub use deno_core;
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
pub use deno_tls;
pub use deno_url;
pub use deno_web;
pub use deno_webidl;
pub use deno_websocket;
pub use deno_webstorage;

pub mod colors;
pub mod errors;
pub mod fmt_errors;
pub mod fs_util;
pub mod inspector_server;
pub mod js;
pub mod ops;
pub mod permissions;
pub mod tokio_util;
pub mod web_worker;
pub mod worker;

mod worker_bootstrap;
pub use worker_bootstrap::BootstrapOptions;

// This is duplicated in `./build.rs`, keep in sync!
deno_core::extension!(runtime,
  deps = [
    deno_webidl,
    deno_console,
    deno_url,
    deno_tls,
    deno_web,
    deno_fetch,
    deno_cache,
    deno_websocket,
    deno_webstorage,
    deno_crypto,
    deno_broadcast_channel,
    // FIXME(bartlomieju): this should be reenabled
    // "deno_node",
    deno_ffi,
    deno_net,
    deno_napi,
    deno_http,
    deno_io,
    deno_fs
  ],
  esm = [
    dir "js",
    "01_errors.js",
    "01_version.ts",
    "06_util.js",
    "10_permissions.js",
    "11_workers.js",
    "13_buffer.js",
    "30_os.js",
    "40_fs_events.js",
    "40_http.js",
    "40_process.js",
    "40_signals.js",
    "40_tty.js",
    "41_prompt.js",
    "90_deno_ns.js",
    "98_global_scope.js"
  ],
);
