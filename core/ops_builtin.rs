// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
use crate::error::format_file_name;
use crate::error::type_error;
use crate::include_js_files;
use crate::io::BufMutView;
use crate::io::BufView;
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
      op_read_all::decl(),
      op_write::decl(),
      op_write_all::decl(),
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
  let view = BufMutView::from(buf);
  resource.read_byob(view).await.map(|(n, _)| n as u32)
}

#[op]
async fn op_read_all(
  state: Rc<RefCell<OpState>>,
  rid: ResourceId,
) -> Result<ZeroCopyBuf, Error> {
  let resource = state.borrow().resource_table.get_any(rid)?;

  // The number of bytes we attempt to grow the buffer by each time it fills
  // up and we have more data to read. We start at 64 KB. The grow_len is
  // doubled if the nread returned from a single read is equal or greater than
  // the grow_len. This allows us to reduce allocations for resources that can
  // read large chunks of data at a time.
  let mut grow_len: usize = 64 * 1024;

  let (min, maybe_max) = resource.size_hint();
  // Try to determine an optimial starting buffer size for this resource based
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

  Ok(ZeroCopyBuf::from(vec))
}

#[op]
async fn op_write(
  state: Rc<RefCell<OpState>>,
  rid: ResourceId,
  buf: ZeroCopyBuf,
) -> Result<u32, Error> {
  let resource = state.borrow().resource_table.get_any(rid)?;
  let view = BufView::from(buf);
  let resp = resource.write(view).await?;
  Ok(resp.nwritten() as u32)
}

#[op]
async fn op_write_all(
  state: Rc<RefCell<OpState>>,
  rid: ResourceId,
  buf: ZeroCopyBuf,
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
