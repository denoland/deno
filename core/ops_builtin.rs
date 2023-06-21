// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
use crate::error::format_file_name;
use crate::error::type_error;
use crate::io::BufMutView;
use crate::io::BufView;
use crate::ops_builtin_v8;
use crate::ops_metrics::OpMetrics;
use crate::resources::ResourceId;
use crate::JsBuffer;
use crate::OpState;
use crate::Resource;
use anyhow::Error;
use deno_ops::op;
use deno_ops::op2;
use serde_v8::ToJsBuffer;
use std::cell::RefCell;
use std::io::stderr;
use std::io::stdout;
use std::io::Write;
use std::rc::Rc;

crate::extension!(
  core,
  ops = [
    op_close,
    op_try_close,
    op_print,
    op_resources,
    op_wasm_streaming_feed,
    op_wasm_streaming_set_url,
    op_void_sync,
    op_error_async,
    op_error_async_deferred,
    op_void_async,
    op_void_async_deferred,
    op_add,
    op_add_async,
    // TODO(@AaronO): track IO metrics for builtin streams
    op_read,
    op_read_all,
    op_write,
    op_read_sync,
    op_write_sync,
    op_write_all,
    op_shutdown,
    op_metrics,
    op_format_file_name,
    op_is_proxy,
    op_str_byte_length,
    ops_builtin_v8::op_ref_op,
    ops_builtin_v8::op_unref_op,
    ops_builtin_v8::op_set_promise_reject_callback,
    ops_builtin_v8::op_run_microtasks,
    ops_builtin_v8::op_has_tick_scheduled,
    ops_builtin_v8::op_set_has_tick_scheduled,
    ops_builtin_v8::op_eval_context,
    ops_builtin_v8::op_queue_microtask,
    ops_builtin_v8::op_create_host_object,
    ops_builtin_v8::op_encode,
    ops_builtin_v8::op_decode,
    ops_builtin_v8::op_serialize,
    ops_builtin_v8::op_deserialize,
    ops_builtin_v8::op_set_promise_hooks,
    ops_builtin_v8::op_get_promise_details,
    ops_builtin_v8::op_get_proxy_details,
    ops_builtin_v8::op_get_non_index_property_names,
    ops_builtin_v8::op_get_constructor_name,
    ops_builtin_v8::op_memory_usage,
    ops_builtin_v8::op_set_wasm_streaming_callback,
    ops_builtin_v8::op_abort_wasm_streaming,
    ops_builtin_v8::op_destructure_error,
    ops_builtin_v8::op_dispatch_exception,
    ops_builtin_v8::op_op_names,
    ops_builtin_v8::op_apply_source_map,
    ops_builtin_v8::op_set_format_exception_callback,
    ops_builtin_v8::op_event_loop_has_more_work,
    ops_builtin_v8::op_store_pending_promise_rejection,
    ops_builtin_v8::op_remove_pending_promise_rejection,
    ops_builtin_v8::op_has_pending_promise_rejection,
    ops_builtin_v8::op_arraybuffer_was_detached,
  ],
  js = ["00_primordials.js", "01_core.js", "02_error.js"],
  customizer = |ext: &mut crate::ExtensionBuilder| {
    ext.deno_core();
  }
);

/// Return map of resources with id as key
/// and string representation as value.
#[op]
pub fn op_resources(state: &mut OpState) -> Vec<(ResourceId, String)> {
  state
    .resource_table
    .names()
    .map(|(rid, name)| (rid, name.to_string()))
    .collect()
}

#[op2(core, fast)]
fn op_add(a: i32, b: i32) -> i32 {
  a + b
}

#[op]
pub async fn op_add_async(a: i32, b: i32) -> i32 {
  a + b
}

#[op(fast)]
pub fn op_void_sync() {}

#[op]
pub async fn op_void_async() {}

#[op]
pub async fn op_error_async() -> Result<(), Error> {
  Err(Error::msg("error"))
}

#[op(deferred)]
pub async fn op_error_async_deferred() -> Result<(), Error> {
  Err(Error::msg("error"))
}

#[op(deferred)]
pub async fn op_void_async_deferred() {}

/// Remove a resource from the resource table.
#[op]
pub fn op_close(
  state: &mut OpState,
  rid: Option<ResourceId>,
) -> Result<(), Error> {
  // TODO(@AaronO): drop Option after improving type-strictness balance in
  // serde_v8
  let rid = rid.ok_or_else(|| type_error("missing or invalid `rid`"))?;
  state.resource_table.close(rid)?;
  Ok(())
}

/// Try to remove a resource from the resource table. If there is no resource
/// with the specified `rid`, this is a no-op.
#[op]
pub fn op_try_close(
  state: &mut OpState,
  rid: Option<ResourceId>,
) -> Result<(), Error> {
  // TODO(@AaronO): drop Option after improving type-strictness balance in
  // serde_v8.
  let rid = rid.ok_or_else(|| type_error("missing or invalid `rid`"))?;
  let _ = state.resource_table.close(rid);
  Ok(())
}

#[op]
pub fn op_metrics(state: &mut OpState) -> (OpMetrics, Vec<OpMetrics>) {
  let aggregate = state.tracker.aggregate();
  let per_op = state.tracker.per_op();
  (aggregate, per_op)
}

/// Builtin utility to print to stdout/stderr
#[op]
pub fn op_print(msg: &str, is_err: bool) -> Result<(), Error> {
  if is_err {
    stderr().write_all(msg.as_bytes())?;
    stderr().flush().unwrap();
  } else {
    stdout().write_all(msg.as_bytes())?;
    stdout().flush().unwrap();
  }
  Ok(())
}

pub struct WasmStreamingResource(pub(crate) RefCell<v8::WasmStreaming>);

impl Resource for WasmStreamingResource {
  fn close(self: Rc<Self>) {
    // At this point there are no clones of Rc<WasmStreamingResource> on the
    // resource table, and no one should own a reference outside of the stack.
    // Therefore, we can be sure `self` is the only reference.
    if let Ok(wsr) = Rc::try_unwrap(self) {
      wsr.0.into_inner().finish();
    } else {
      panic!("Couldn't consume WasmStreamingResource.");
    }
  }
}

/// Feed bytes to WasmStreamingResource.
#[op]
pub fn op_wasm_streaming_feed(
  state: &mut OpState,
  rid: ResourceId,
  bytes: &[u8],
) -> Result<(), Error> {
  let wasm_streaming =
    state.resource_table.get::<WasmStreamingResource>(rid)?;

  wasm_streaming.0.borrow_mut().on_bytes_received(bytes);

  Ok(())
}

#[op]
pub fn op_wasm_streaming_set_url(
  state: &mut OpState,
  rid: ResourceId,
  url: &str,
) -> Result<(), Error> {
  let wasm_streaming =
    state.resource_table.get::<WasmStreamingResource>(rid)?;

  wasm_streaming.0.borrow_mut().set_url(url);

  Ok(())
}

#[op]
async fn op_read(
  state: Rc<RefCell<OpState>>,
  rid: ResourceId,
  buf: JsBuffer,
) -> Result<u32, Error> {
  let resource = state.borrow().resource_table.get_any(rid)?;
  let view = BufMutView::from(buf);
  resource.read_byob(view).await.map(|(n, _)| n as u32)
}

#[op]
async fn op_read_all(
  state: Rc<RefCell<OpState>>,
  rid: ResourceId,
) -> Result<ToJsBuffer, Error> {
  let resource = state.borrow().resource_table.get_any(rid)?;

  // The number of bytes we attempt to grow the buffer by each time it fills
  // up and we have more data to read. We start at 64 KB. The grow_len is
  // doubled if the nread returned from a single read is equal or greater than
  // the grow_len. This allows us to reduce allocations for resources that can
  // read large chunks of data at a time.
  let mut grow_len: usize = 64 * 1024;

  let (min, maybe_max) = resource.size_hint();
  // Try to determine an optimal starting buffer size for this resource based
  // on the size hint.
  let initial_size = match (min, maybe_max) {
    (min, Some(max)) if min == max => min as usize,
    (_min, Some(max)) if (max as usize) < grow_len => max as usize,
    (min, _) if (min as usize) < grow_len => grow_len,
    (min, _) => min as usize,
  };

  let mut buf = BufMutView::new(initial_size);
  loop {
    // if the buffer does not have much remaining space, we may have to grow it.
    if buf.len() < grow_len {
      let vec = buf.get_mut_vec();
      match maybe_max {
        Some(max) if vec.len() >= max as usize => {
          // no need to resize the vec, because the vec is already large enough
          // to accommodate the maximum size of the read data.
        }
        Some(max) if (max as usize) < vec.len() + grow_len => {
          // grow the vec to the maximum size of the read data
          vec.resize(max as usize, 0);
        }
        _ => {
          // grow the vec by grow_len
          vec.resize(vec.len() + grow_len, 0);
        }
      }
    }
    let (n, new_buf) = resource.clone().read_byob(buf).await?;
    buf = new_buf;
    buf.advance_cursor(n);
    if n == 0 {
      break;
    }
    if n >= grow_len {
      // we managed to read more or equal data than fits in a single grow_len in
      // a single go, so let's attempt to read even more next time. this reduces
      // allocations for resources that can read large chunks of data at a time.
      grow_len *= 2;
    }
  }

  let nread = buf.reset_cursor();
  let mut vec = buf.unwrap_vec();
  // If the buffer is larger than the amount of data read, shrink it to the
  // amount of data read.
  if nread < vec.len() {
    vec.truncate(nread);
  }

  Ok(ToJsBuffer::from(vec))
}

#[op]
async fn op_write(
  state: Rc<RefCell<OpState>>,
  rid: ResourceId,
  buf: JsBuffer,
) -> Result<u32, Error> {
  let resource = state.borrow().resource_table.get_any(rid)?;
  let view = BufView::from(buf);
  let resp = resource.write(view).await?;
  Ok(resp.nwritten() as u32)
}

#[op(fast)]
fn op_read_sync(
  state: &mut OpState,
  rid: ResourceId,
  data: &mut [u8],
) -> Result<u32, Error> {
  let resource = state.resource_table.get_any(rid)?;
  resource.read_byob_sync(data).map(|n| n as u32)
}

#[op]
fn op_write_sync(
  state: &mut OpState,
  rid: ResourceId,
  data: &[u8],
) -> Result<u32, Error> {
  let resource = state.resource_table.get_any(rid)?;
  let nwritten = resource.write_sync(data)?;
  Ok(nwritten as u32)
}

#[op]
async fn op_write_all(
  state: Rc<RefCell<OpState>>,
  rid: ResourceId,
  buf: JsBuffer,
) -> Result<(), Error> {
  let resource = state.borrow().resource_table.get_any(rid)?;
  let view = BufView::from(buf);
  resource.write_all(view).await?;
  Ok(())
}

#[op]
async fn op_shutdown(
  state: Rc<RefCell<OpState>>,
  rid: ResourceId,
) -> Result<(), Error> {
  let resource = state.borrow().resource_table.get_any(rid)?;
  resource.shutdown().await
}

#[op]
fn op_format_file_name(file_name: String) -> String {
  format_file_name(&file_name)
}

#[op(fast)]
fn op_is_proxy(value: serde_v8::Value) -> bool {
  value.v8_value.is_proxy()
}

#[op(v8)]
fn op_str_byte_length(
  scope: &mut v8::HandleScope,
  value: serde_v8::Value,
) -> u32 {
  if let Ok(string) = v8::Local::<v8::String>::try_from(value.v8_value) {
    string.utf8_length(scope) as u32
  } else {
    0
  }
}
