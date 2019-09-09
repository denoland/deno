// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use crate::state::ThreadSafeState;
use deno::*;
use deno_ops_fs as fs;
use std::sync::Arc;

mod compiler;
//  TODO(afinch7) remove this.
mod dispatch_json;
mod dispatch_minimal;
mod errors;
mod fetch;
mod files;
mod io;
mod metrics;
mod net;
mod os;
mod performance;
mod permissions;
mod process;
mod random;
mod repl;
mod resources;
mod timers;
mod workers;

const OP_NAMESPACE: &str = "builtins";

pub fn setup_dispatcher_registry(state: ThreadSafeState) -> Arc<OpDisReg> {
  let registry = Arc::new(OpDisReg::new());

  // Compiler
  registry.register_op(OP_NAMESPACE, state.wrap_op(compiler::OpCache));
  registry
    .register_op(OP_NAMESPACE, state.wrap_op(compiler::OpFetchSourceFile));
  registry.register_op(OP_NAMESPACE, state.wrap_op(compiler::OpFetchAsset));

  // Errors
  registry.register_op(OP_NAMESPACE, state.wrap_op(errors::OpFormatError));
  registry.register_op(OP_NAMESPACE, state.wrap_op(errors::OpApplySourceMap));

  // Fetch
  registry.register_op(OP_NAMESPACE, state.wrap_op(fetch::OpFetch));

  // Files
  registry.register_op(OP_NAMESPACE, state.wrap_op(files::OpOpen));
  registry.register_op(OP_NAMESPACE, state.wrap_op(files::OpClose));
  registry.register_op(OP_NAMESPACE, state.wrap_op(files::OpSeek));

  let state_ = state.clone();
  let state__ = state.clone();
  let fs_state = fs::TSFsOpsState::new(
    move |filename| {
      state_
        .check_read(filename)
        .map_err(crate::deno_error::CliOpError::from)
        .map_err(deno_dispatch_json::JsonErrBox::from)
    },
    move |filename| {
      state__
        .check_write(filename)
        .map_err(crate::deno_error::CliOpError::from)
        .map_err(deno_dispatch_json::JsonErrBox::from)
    },
  );

  // Fs
  registry.register_op(OP_NAMESPACE, fs_state.wrap_op(fs::OpChdir));
  registry.register_op(OP_NAMESPACE, fs_state.wrap_op(fs::OpMkdir));
  registry.register_op(OP_NAMESPACE, fs_state.wrap_op(fs::OpChmod));
  registry.register_op(OP_NAMESPACE, fs_state.wrap_op(fs::OpChown));
  registry.register_op(OP_NAMESPACE, fs_state.wrap_op(fs::OpRemove));
  registry.register_op(OP_NAMESPACE, fs_state.wrap_op(fs::OpCopyFile));
  registry.register_op(OP_NAMESPACE, fs_state.wrap_op(fs::OpStat));
  registry.register_op(OP_NAMESPACE, fs_state.wrap_op(fs::OpReadDir));
  registry.register_op(OP_NAMESPACE, fs_state.wrap_op(fs::OpRename));
  registry.register_op(OP_NAMESPACE, fs_state.wrap_op(fs::OpLink));
  registry.register_op(OP_NAMESPACE, fs_state.wrap_op(fs::OpSymlink));
  registry.register_op(OP_NAMESPACE, fs_state.wrap_op(fs::OpReadLink));
  registry.register_op(OP_NAMESPACE, fs_state.wrap_op(fs::OpTruncate));
  registry.register_op(OP_NAMESPACE, fs_state.wrap_op(fs::OpMakeTempDir));
  registry.register_op(OP_NAMESPACE, fs_state.wrap_op(fs::OpUtime));
  registry.register_op(OP_NAMESPACE, fs_state.wrap_op(fs::OpCwd));

  // Io
  registry.register_op(OP_NAMESPACE, state.wrap_op(io::OpRead));
  registry.register_op(OP_NAMESPACE, state.wrap_op(io::OpWrite));

  // Metrics
  registry.register_op(OP_NAMESPACE, state.wrap_op(metrics::OpMetrics));

  // Net
  registry.register_op(OP_NAMESPACE, state.wrap_op(net::OpAccept));
  registry.register_op(OP_NAMESPACE, state.wrap_op(net::OpDial));
  registry.register_op(OP_NAMESPACE, state.wrap_op(net::OpShutdown));
  registry.register_op(OP_NAMESPACE, state.wrap_op(net::OpListen));

  // Os
  registry.register_op(OP_NAMESPACE, state.wrap_op(os::OpStart));
  registry.register_op(OP_NAMESPACE, state.wrap_op(os::OpHomeDir));
  registry.register_op(OP_NAMESPACE, state.wrap_op(os::OpExecPath));
  registry.register_op(OP_NAMESPACE, state.wrap_op(os::OpSetEnv));
  registry.register_op(OP_NAMESPACE, state.wrap_op(os::OpEnv));
  registry.register_op(OP_NAMESPACE, state.wrap_op(os::OpExit));
  registry.register_op(OP_NAMESPACE, state.wrap_op(os::OpIsTty));

  // Performance
  registry.register_op(OP_NAMESPACE, state.wrap_op(performance::OpNow));

  // Permissions
  registry.register_op(OP_NAMESPACE, state.wrap_op(permissions::OpPermissions));
  registry
    .register_op(OP_NAMESPACE, state.wrap_op(permissions::OpRevokePermission));

  // Process
  registry.register_op(OP_NAMESPACE, state.wrap_op(process::OpRun));
  registry.register_op(OP_NAMESPACE, state.wrap_op(process::OpRunStatus));
  registry.register_op(OP_NAMESPACE, state.wrap_op(process::OpKill));

  // Random
  registry.register_op(OP_NAMESPACE, state.wrap_op(random::OpGetRandomValues));

  // Repl
  registry.register_op(OP_NAMESPACE, state.wrap_op(repl::OpReplStart));
  registry.register_op(OP_NAMESPACE, state.wrap_op(repl::OpReplReadline));

  // Resources
  registry.register_op(OP_NAMESPACE, state.wrap_op(resources::OpResources));

  // Timers
  registry.register_op(OP_NAMESPACE, state.wrap_op(timers::OpGlobalTimerStop));
  registry.register_op(OP_NAMESPACE, state.wrap_op(timers::OpGlobalTimer));

  // Workers
  registry
    .register_op(OP_NAMESPACE, state.wrap_op(workers::OpWorkerGetMessage));
  registry
    .register_op(OP_NAMESPACE, state.wrap_op(workers::OpWorkerPostMessage));
  registry.register_op(OP_NAMESPACE, state.wrap_op(workers::OpCreateWorker));
  registry
    .register_op(OP_NAMESPACE, state.wrap_op(workers::OpHostGetWorkerClosed));
  registry.register_op(OP_NAMESPACE, state.wrap_op(workers::OpHostGetMessage));
  registry.register_op(OP_NAMESPACE, state.wrap_op(workers::OpHostPostMessage));

  registry
}
