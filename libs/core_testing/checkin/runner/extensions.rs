// Copyright 2018-2025 the Deno authors. MIT license.

use crate::checkin::runner::Output;
use crate::checkin::runner::TestData;
use crate::checkin::runner::ops;
use crate::checkin::runner::ops_async;
use crate::checkin::runner::ops_buffer;
use crate::checkin::runner::ops_error;
use crate::checkin::runner::ops_io;
use crate::checkin::runner::ops_worker;

pub trait SomeType {}

impl SomeType for () {}

deno_core::extension!(
  checkin_runtime,
  parameters = [P: SomeType],
  ops = [
    ops::op_log_debug,
    ops::op_log_info,
    ops::op_stats_capture,
    ops::op_stats_diff,
    ops::op_stats_dump,
    ops::op_stats_delete,
    ops::op_nop_generic<P>,
    ops_io::op_pipe_create,
    ops_io::op_file_open,
    ops_io::op_path_to_url,
    ops_async::op_task_submit,
    ops_async::op_async_yield,
    ops_async::op_async_barrier_create,
    ops_async::op_async_barrier_await,
    ops_async::op_async_spin_on_state,
    ops_async::op_async_make_cppgc_resource,
    ops_async::op_async_get_cppgc_resource,
    ops_async::op_async_never_resolves,
    ops_async::op_async_fake,
    ops_async::op_async_promise_id,
    ops_error::op_async_throw_error_eager,
    ops_error::op_async_throw_error_lazy,
    ops_error::op_async_throw_error_deferred,
    ops_error::op_error_custom_sync,
    ops_error::op_error_custom_with_code_sync,
    ops_buffer::op_v8slice_store,
    ops_buffer::op_v8slice_clone,
    ops_worker::op_worker_spawn,
    ops_worker::op_worker_send,
    ops_worker::op_worker_recv,
    ops_worker::op_worker_parent,
    ops_worker::op_worker_await_close,
    ops_worker::op_worker_terminate,
  ],
  objects = [
    ops::DOMPointReadOnly,
    ops::DOMPoint,
    ops::DOMPoint3D,
    ops::TestObjectWrap,
    ops::TestEnumWrap
  ],
  esm_entry_point = "ext:checkin_runtime/__init.js",
  esm = [
    dir "checkin/runtime",
    "__init.js",
    "checkin:async" = "async.ts",
    "checkin:console" = "console.ts",
    "checkin:object" = "object.ts",
    "checkin:error" = "error.ts",
    "checkin:timers" = "timers.ts",
    "checkin:worker" = "worker.ts",
    "checkin:throw" = "throw.ts",
    "checkin:callsite" = "callsite.ts",
  ],
  state = |state| {
    state.put(TestData::default());
    state.put(Output::default());
  }
);
