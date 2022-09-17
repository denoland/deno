// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
use crate::error::format_file_name;
use crate::error::type_error;
use crate::include_js_files;
use crate::ops_metrics::OpMetrics;
use crate::resources::ResourceId;
use crate::Extension;
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
    .ops(vec![
      op_close::decl(),
      op_try_close::decl(),
      op_print::decl(),
      op_resources::decl(),
      op_wasm_streaming_feed::decl(),
      op_wasm_streaming_set_url::decl(),
      op_void_sync::decl(),
      op_void_async::decl(),
      op_add::decl(),
      // // TODO(@AaronO): track IO metrics for builtin streams
      op_read::decl(),
      op_write::decl(),
      op_shutdown::decl(),
      op_metrics::decl(),
      op_format_file_name::decl(),
      op_is_proxy::decl(),
      op_str_byte_length::decl(),
    ])
    .ops(crate::ops_builtin_v8::init_builtins_v8())
    .build()
}

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

#[op(fast)]
fn op_add(a: i32, b: i32) -> i32 {
  a + b
}

#[op(fast)]
pub fn op_void_sync() {}

#[op]
pub async fn op_void_async() {}

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
pub fn op_print(msg: String, is_err: bool) -> Result<(), Error> {
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
