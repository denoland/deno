// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use std::sync::atomic::AtomicI32;

pub use deno_broadcast_channel;
pub use deno_console;
pub use deno_core;
pub use deno_crypto;
pub use deno_fetch;
pub use deno_ffi;
pub use deno_http;
pub use deno_net;
pub use deno_timers;
pub use deno_tls;
pub use deno_url;
pub use deno_web;
pub use deno_webgpu;
pub use deno_webidl;
pub use deno_websocket;
pub use deno_webstorage;

pub mod colors;
pub mod errors;
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

// The global may not be very elegant but:
//
// 1. op_exit() calls std::process::exit() so there is not much point storing
//    the exit code in runtime state
//
// 2. storing it in runtime state makes retrieving it again in cli/main.rs
//    unduly complicated
pub static EXIT_CODE: AtomicI32 = AtomicI32::new(0);
