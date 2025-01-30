// Copyright 2018-2025 the Deno authors. MIT license.

use deno_core::cppgc::Ptr;
use deno_core::op2;
use deno_core::webidl::WebIdlInterfaceConverter;
use deno_core::GarbageCollected;
use deno_core::WebIDL;

use crate::Instance;

pub struct GPUPipelineLayout {
  pub instance: Instance,
  pub id: wgpu_core::id::PipelineLayoutId,
  pub label: String,
}

impl Drop for GPUPipelineLayout {
  fn drop(&mut self) {
    self.instance.pipeline_layout_drop(self.id);
  }
}

impl WebIdlInterfaceConverter for GPUPipelineLayout {
  const NAME: &'static str = "GPUPipelineLayout";
}

impl GarbageCollected for GPUPipelineLayout {}

#[op2]
impl GPUPipelineLayout {
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
pub(crate) struct GPUPipelineLayoutDescriptor {
  #[webidl(default = String::new())]
  pub label: String,

  pub bind_group_layouts:
    Vec<Ptr<super::bind_group_layout::GPUBindGroupLayout>>,
}
