// Copyright 2018-2025 the Deno authors. MIT license.

use deno_core::op2;
use deno_core::GarbageCollected;
use deno_core::WebIDL;

pub struct GPUCommandBuffer {
  pub id: wgpu_core::id::CommandBufferId,
  pub label: String,
}

impl deno_core::webidl::WebIdlInterfaceConverter for GPUCommandBuffer {
  const NAME: &'static str = "GPUCommandBuffer";
}

impl GarbageCollected for GPUCommandBuffer {}

#[op2]
impl GPUCommandBuffer {
  crate::with_label!();
}

#[derive(WebIDL)]
#[webidl(dictionary)]
pub(crate) struct GPUCommandBufferDescriptor {
  #[webidl(default = String::new())]
  pub label: String,
}
