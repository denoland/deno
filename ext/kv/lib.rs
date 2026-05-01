// Copyright 2018-2026 the Deno authors. MIT license.

// NOTE: The KV implementation has been moved to pure JS in impl/kv.ts.
// This Rust module now only provides the extension shell (ESM registration)
// and the backend initialization types that the runtime still references.
// The ops have been removed - all KV logic runs in JavaScript.

pub mod config;
pub mod dynamic;
mod interface;
pub mod remote;
pub mod sqlite;

use std::rc::Rc;

pub use crate::config::*;
pub use crate::interface::*;

pub const UNSTABLE_FEATURE_NAME: &str = "kv";

deno_core::extension!(deno_kv,
  deps = [ deno_web ],
  parameters = [ DBH: DatabaseHandler ],
  esm = [
    "impl/key_codec.ts",
    "impl/protobuf.ts",
    "impl/sqlite_backend.ts",
    "impl/remote_backend.ts",
    "impl/kv.ts",
    "01_db.ts",
  ],
  options = {
    handler: DBH,
    config: KvConfig,
  },
  state = |state, options| {
    state.put(Rc::new(options.config));
    state.put(Rc::new(options.handler));
  }
);
