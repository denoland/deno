// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use crate::state::ThreadSafeState;
use deno::*;

pub mod compiler;
pub mod dispatch_json;
pub mod dispatch_minimal;
pub mod errors;
pub mod fetch;
pub mod files;
pub mod fs;
pub mod io;
pub mod metrics;
pub mod net;
pub mod os;
pub mod performance;
pub mod permissions;
pub mod process;
pub mod random;
pub mod repl;
pub mod resources;
pub mod serializer_json;
pub mod serializer_minimal;
pub mod timers;
pub mod workers;

pub type CliOpHandler =
  dyn Fn(&ThreadSafeState, &[u8], Option<PinnedBuf>) -> CoreOp
    + Send
    + Sync
    + 'static;
