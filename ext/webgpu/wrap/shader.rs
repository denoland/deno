// Copyright 2018-2025 the Deno authors. MIT license.

use deno_core::op2;
use deno_core::webidl::WebIdlInterfaceConverter;
use deno_core::GarbageCollected;
use deno_core::WebIDL;

pub struct GPUShaderModule {
  pub id: wgpu_core::id::ShaderModuleId,
  pub label: String,
}

impl WebIdlInterfaceConverter for GPUShaderModule {
  const NAME: &'static str = "GPUShaderModule";
}

impl GarbageCollected for GPUShaderModule {}

#[op2]
impl GPUShaderModule {
  crate::with_label!();
}

#[derive(WebIDL)]
#[webidl(dictionary)]
pub(crate) struct GPUShaderModuleDescriptor {
  #[webidl(default = String::new())]
  pub label: String,

  pub code: String,
}
