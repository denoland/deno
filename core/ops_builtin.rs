use crate::error::type_error;
use crate::include_js_files;
use crate::resources::ResourceId;
use crate::Extension;
use crate::OpMetrics;
use crate::OpState;
use crate::Resource;
use crate::ZeroCopyBuf;
use anyhow::Error;
use deno_ops::op;
use std::cell::RefCell;
use std::io::{stderr, stdout, Write};
use std::rc::Rc;

pub(crate) fn init_builtins() -> Extension {
  Extension::builder()
    .js(include_js_files!(
      prefix "deno:core",
      "00_primordials.js",
      "01_core.js",
      "02_error.js",
    ))
    .ops(|ctx| {
      ctx.register("op_close", op_close);
      ctx.register("op_try_close", op_try_close);
      ctx.register("op_print", op_print);
      ctx.register("op_resources", op_resources);
      ctx.register("op_wasm_streaming_feed", op_wasm_streaming_feed);
      ctx.register("op_wasm_streaming_abort", op_wasm_streaming_abort);
      ctx.register("op_wasm_streaming_set_url", op_wasm_streaming_set_url);
      ctx.register("op_void_sync", op_void_sync);
      ctx.register("op_void_async", op_void_async);
      // // TODO(@AaronO): track IO metrics for builtin streams
      ctx.register("op_read", op_read);
      ctx.register("op_write", op_write);
      ctx.register("op_shutdown", op_shutdown);
      ctx.register("op_metrics", op_metrics);
    })
    .build()
}

mod deno_core {
  pub use crate::*;
}

/// Return map of resources with id as key
/// and string representation as value.
#[op]
pub fn op_resources(
  state: &mut OpState,
  _: (),
  _: (),
) -> Result<Vec<(ResourceId, String)>, Error> {
  let serialized_resources = state
    .resource_table
    .names()
    .map(|(rid, name)| (rid, name.to_string()))
    .collect();
  Ok(serialized_resources)
}

#[op]
pub fn op_void_sync(_state: &mut OpState, _: (), _: ()) -> Result<(), Error> {
  Ok(())
}

#[op]
pub async fn op_void_async(
  _state: Rc<RefCell<OpState>>,
  _: (),
  _: (),
) -> Result<(), Error> {
  Ok(())
}

/// Remove a resource from the resource table.
#[op]
pub fn op_close(
  state: &mut OpState,
  rid: Option<ResourceId>,
  _: (),
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
  _: (),
) -> Result<(), Error> {
  // TODO(@AaronO): drop Option after improving type-strictness balance in
  // serde_v8.
  let rid = rid.ok_or_else(|| type_error("missing or invalid `rid`"))?;
  let _ = state.resource_table.close(rid);
  Ok(())
}

#[op]
pub fn op_metrics(
  state: &mut OpState,
  _: (),
  _: (),
) -> Result<(OpMetrics, Vec<OpMetrics>), Error> {
  let aggregate = state.tracker.aggregate();
  let per_op = state.tracker.per_op();
  Ok((aggregate, per_op))
}

/// Builtin utility to print to stdout/stderr
#[op]
pub fn op_print(
  _state: &mut OpState,
  msg: String,
  is_err: bool,
) -> Result<(), Error> {
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
  bytes: ZeroCopyBuf,
) -> Result<(), Error> {
  let wasm_streaming =
    state.resource_table.get::<WasmStreamingResource>(rid)?;

  wasm_streaming.0.borrow_mut().on_bytes_received(&bytes);

  Ok(())
}

/// Abort a WasmStreamingResource.
#[op]
pub fn op_wasm_streaming_abort(
  state: &mut OpState,
  rid: ResourceId,
  exception: serde_v8::Value,
) -> Result<(), Error> {
  let wasm_streaming =
    state.resource_table.take::<WasmStreamingResource>(rid)?;

  // At this point there are no clones of Rc<WasmStreamingResource> on the
  // resource table, and no one should own a reference because we're never
  // cloning them. So we can be sure `wasm_streaming` is the only reference.
  if let Ok(wsr) = Rc::try_unwrap(wasm_streaming) {
    wsr.0.into_inner().abort(Some(exception.v8_value));
  } else {
    panic!("Couldn't consume WasmStreamingResource.");
  }

  Ok(())
}

#[op]
pub fn op_wasm_streaming_set_url(
  state: &mut OpState,
  rid: ResourceId,
  url: String,
) -> Result<(), Error> {
  let wasm_streaming =
    state.resource_table.get::<WasmStreamingResource>(rid)?;

  wasm_streaming.0.borrow_mut().set_url(&url);

  Ok(())
}

#[op]
async fn op_read(
  state: Rc<RefCell<OpState>>,
  rid: ResourceId,
  buf: ZeroCopyBuf,
) -> Result<u32, Error> {
  let resource = state.borrow().resource_table.get_any(rid)?;
  resource.read(buf).await.map(|n| n as u32)
}

#[op]
async fn op_write(
  state: Rc<RefCell<OpState>>,
  rid: ResourceId,
  buf: ZeroCopyBuf,
) -> Result<u32, Error> {
  let resource = state.borrow().resource_table.get_any(rid)?;
  resource.write(buf).await.map(|n| n as u32)
}

#[op]
async fn op_shutdown(
  state: Rc<RefCell<OpState>>,
  rid: ResourceId,
  _: (),
) -> Result<(), Error> {
  let resource = state.borrow().resource_table.get_any(rid)?;
  resource.shutdown().await
}
