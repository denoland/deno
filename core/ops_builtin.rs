use crate::error::bad_resource_id;
use crate::error::type_error;
use crate::error::AnyError;
use crate::include_js_files;
use crate::op_sync;
use crate::resources::ResourceId;
use crate::Extension;
use crate::OpState;
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
