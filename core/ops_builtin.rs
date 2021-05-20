use crate::error::bad_resource_id;
use crate::error::type_error;
use crate::error::AnyError;
use crate::include_js_files;
use crate::op_sync;
use crate::resources::ResourceId;
use crate::Extension;
use crate::OpState;
use crate::ZeroCopyBuf;
use std::io::{stderr, stdout, Write};

pub(crate) fn init_builtins() -> Extension {
  Extension::builder()
    .js(include_js_files!(
      prefix "deno:core",
      "core.js",
      "error.js",
    ))
    .ops(vec![
      ("op_close", op_sync(op_close)),
      ("op_decode", op_sync(op_decode)),
      ("op_encode", op_sync(op_encode)),
      ("op_print", op_sync(op_print)),
      ("op_resources", op_sync(op_resources)),
    ])
    .build()
}

/// Return map of resources with id as key
/// and string representation as value.
pub fn op_resources(
  state: &mut OpState,
  _args: (),
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
  // TODO(@AaronO): drop Option after improving type-strictness balance in serde_v8
  let rid = rid.ok_or_else(|| type_error("missing or invalid `rid`"))?;
  state
    .resource_table
    .close(rid)
    .ok_or_else(bad_resource_id)?;

  Ok(())
}

/// Builtin utility to print to stdout/stderr
pub fn op_print(
  _state: &mut OpState,
  msg: String,
  is_err: bool,
) -> Result<(), AnyError> {
  if is_err {
    eprint!("{}", msg);
    stderr().flush().unwrap();
  } else {
    print!("{}", msg);
    stdout().flush().unwrap();
  }
  Ok(())
}

/// Converts String to Uint8Array
fn op_encode(
  _state: &mut OpState,
  text: String,
  _: (),
) -> Result<ZeroCopyBuf, AnyError> {
  Ok(text.into_bytes().into())
}

/// Converts Uint8Array to String
fn op_decode(
  _state: &mut OpState,
  zero_copy: ZeroCopyBuf,
  _: (),
) -> Result<String, AnyError> {
  let buf = &zero_copy;

  // Strip BOM
  let buf = if buf.len() >= 3 && buf[..3] == [0xEF, 0xBB, 0xBF] {
    &buf[3..]
  } else {
    buf
  };

  std::str::from_utf8(buf)
    .map(str::to_string)
    .map_err(AnyError::from)
}
