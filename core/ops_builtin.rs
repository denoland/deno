use crate::error::type_error;
use crate::error::AnyError;
use crate::include_js_files;
use crate::op_sync;
use crate::resources::ResourceId;
use crate::Extension;
use crate::OpState;
use crate::Resource;
use crate::ZeroCopyBuf;
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
      ("op_close", op_sync(op_close)),
      ("op_try_close", op_sync(op_try_close)),
      ("op_print", op_sync(op_print)),
      ("op_resources", op_sync(op_resources)),
      ("op_wasm_streaming_feed", op_sync(op_wasm_streaming_feed)),
      ("op_wasm_streaming_abort", op_sync(op_wasm_streaming_abort)),
    ])
    .build()
}

/// Return map of resources with id as key
/// and string representation as value.
pub fn op_resources(
  state: &mut OpState,
  _: (),
  _: (),
) -> Result<Vec<(ResourceId, String)>, AnyError> {
  let serialized_resources = state
    .resource_table
    .names()
    .map(|(rid, name)| (rid, name.to_string()))
    .collect();
  Ok(serialized_resources)
}

/// Remove a resource from the resource table.
pub fn op_close(
  state: &mut OpState,
  rid: Option<ResourceId>,
  _: (),
) -> Result<(), AnyError> {
  // TODO(@AaronO): drop Option after improving type-strictness balance in
  // serde_v8
  let rid = rid.ok_or_else(|| type_error("missing or invalid `rid`"))?;
  state.resource_table.close(rid)?;
  Ok(())
}

/// Try to remove a resource from the resource table. If there is no resource
/// with the specified `rid`, this is a no-op.
pub fn op_try_close(
  state: &mut OpState,
  rid: Option<ResourceId>,
  _: (),
) -> Result<(), AnyError> {
  // TODO(@AaronO): drop Option after improving type-strictness balance in
  // serde_v8.
  let rid = rid.ok_or_else(|| type_error("missing or invalid `rid`"))?;
  let _ = state.resource_table.close(rid);
  Ok(())
}

/// Builtin utility to print to stdout/stderr
pub fn op_print(
  _state: &mut OpState,
  msg: String,
  is_err: bool,
) -> Result<(), AnyError> {
  if is_err {
    stderr().write_all(msg.as_bytes())?;
    stderr().flush().unwrap();
  } else {
    stdout().write_all(msg.as_bytes())?;
    stdout().flush().unwrap();
  }
  Ok(())
}

pub struct WasmStreamingResource(pub(crate) RefCell<rusty_v8::WasmStreaming>);

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
pub fn op_wasm_streaming_feed(
  state: &mut OpState,
  rid: ResourceId,
  bytes: ZeroCopyBuf,
) -> Result<(), AnyError> {
  let wasm_streaming =
    state.resource_table.get::<WasmStreamingResource>(rid)?;

  wasm_streaming.0.borrow_mut().on_bytes_received(&bytes);

  Ok(())
}

/// Abort a WasmStreamingResource.
pub fn op_wasm_streaming_abort(
  state: &mut OpState,
  rid: ResourceId,
  exception: serde_v8::Value,
) -> Result<(), AnyError> {
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
