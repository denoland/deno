// Copyright 2018-2025 the Deno authors. MIT license.

use deno_core::op2;
use deno_core::webidl::WebIdlInterfaceConverter;
use deno_core::GarbageCollected;
use deno_core::WebIDL;

use crate::Instance;

pub struct GPUShaderModule {
  pub instance: Instance,
  pub id: wgpu_core::id::ShaderModuleId,
  pub label: String,
}

impl Drop for GPUShaderModule {
  fn drop(&mut self) {
    self.instance.shader_module_drop(self.id);
  }
}

impl WebIdlInterfaceConverter for GPUShaderModule {
  const NAME: &'static str = "GPUShaderModule";
}

impl GarbageCollected for GPUShaderModule {}

#[op2]
impl GPUShaderModule {
  #[getter]
  #[string]
  fn label(&self) -> String {
    self.label.clone()
  }
  #[setter]
  #[string]
  fn label(&self, #[webidl] _label: String) {
    // TODO(@crowlKats): no-op, needs wpgu to implement changing the label
  }
}

#[derive(WebIDL)]
#[webidl(dictionary)]
pub(crate) struct GPUShaderModuleDescriptor {
  #[webidl(default = String::new())]
  pub label: String,

  pub code: String,
}
