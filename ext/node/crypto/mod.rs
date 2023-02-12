// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
use deno_core::error::type_error;
use deno_core::error::AnyError;
use deno_core::op;
use deno_core::OpState;
use deno_core::ResourceId;
use deno_core::ZeroCopyBuf;
use std::rc::Rc;

mod digest;

#[op]
pub fn op_node_create_hash(
  state: &mut OpState,
  algorithm: String,
) -> Result<ResourceId, AnyError> {
  Ok(state.resource_table.add(digest::Context::new(&algorithm)?))
}

#[op]
pub fn op_node_hash_update(
  state: &mut OpState,
  rid: ResourceId,
  data: &[u8],
) -> Result<(), AnyError> {
  let context = state.resource_table.get::<digest::Context>(rid)?;
  context.update(data);
  Ok(())
}

#[op]
pub fn op_node_hash_digest(
  state: &mut OpState,
  rid: ResourceId,
) -> Result<ZeroCopyBuf, AnyError> {
  let context = state.resource_table.take::<digest::Context>(rid)?;
  let context = Rc::try_unwrap(context)
    .map_err(|_| type_error("Hash context is already in use"))?;
  Ok(context.digest()?.into())
}

#[op]
pub fn op_node_hash_clone(
  state: &mut OpState,
  rid: ResourceId,
) -> Result<ResourceId, AnyError> {
  let context = state.resource_table.get::<digest::Context>(rid)?;
  Ok(state.resource_table.add(context.as_ref().clone()))
}
