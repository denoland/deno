use deno_core::op;
use deno_core::OpState;
use deno_core::ResourceId;

// mod digest;

#[op]
pub fn op_node_create_hash(state: &mut OpState, algorithm: String) {}

#[op]
pub fn op_node_hash_update(state: &mut OpState, rid: ResourceId, data: &[u8]) {}

#[op]
pub fn op_node_hash_digest(state: &mut OpState, rid: ResourceId) {
  // Consumes
}

#[op]
pub fn op_node_clone_hash(state: &mut OpState, rid: ResourceId) {}
