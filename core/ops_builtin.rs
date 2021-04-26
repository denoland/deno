use crate::error::bad_resource_id;
use crate::error::type_error;
use crate::error::AnyError;
use crate::resources::ResourceId;
use crate::OpState;
use crate::ZeroCopyBuf;

// TODO(@AaronO): provide these ops grouped as a runtime extension
// e.g:
// pub fn init_builtins() -> Extension { ... }

/// Return map of resources with id as key
/// and string representation as value.
///
/// This op must be wrapped in `op_sync`.
pub fn op_resources(
  state: &mut OpState,
  _args: (),
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<Vec<(ResourceId, String)>, AnyError> {
  let serialized_resources = state
    .resource_table
    .names()
    .map(|(rid, name)| (rid, name.to_string()))
    .collect();
  Ok(serialized_resources)
}

/// Remove a resource from the resource table.
///
/// This op must be wrapped in `op_sync`.
pub fn op_close(
  state: &mut OpState,
  rid: Option<ResourceId>,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<(), AnyError> {
  // TODO(@AaronO): drop Option after improving type-strictness balance in serde_v8
  let rid = rid.ok_or_else(|| type_error("missing or invalid `rid`"))?;
  state
    .resource_table
    .close(rid)
    .ok_or_else(bad_resource_id)?;

  Ok(())
}
