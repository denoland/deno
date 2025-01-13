// Copyright 2018-2025 the Deno authors. MIT license.

use deno_core::cppgc::Ptr;
use deno_core::op2;
use deno_core::webidl::WebIdlInterfaceConverter;
use deno_core::GarbageCollected;
use deno_core::WebIDL;
use indexmap::IndexMap;

use crate::wrap::bind_group_layout::GPUBindGroupLayout;
use crate::wrap::shader::GPUShaderModule;
use crate::wrap::webidl::GPUPipelineLayoutOrGPUAutoLayoutMode;
use crate::Instance;

pub struct GPUComputePipeline {
  pub instance: Instance,
  pub error_handler: super::error::ErrorHandler,

  pub id: wgpu_core::id::ComputePipelineId,
  pub label: String,
}

impl WebIdlInterfaceConverter for GPUComputePipeline {
  const NAME: &'static str = "GPUComputePipeline";
}

impl GarbageCollected for GPUComputePipeline {}

#[op2]
impl GPUComputePipeline {
  crate::with_label!();

  #[cppgc]
  fn get_bind_group_layout(&self, #[webidl] index: u32) -> GPUBindGroupLayout {
    let (id, err) = self
      .instance
      .compute_pipeline_get_bind_group_layout(self.id, index, None);

    self.error_handler.push_error(err);

    // TODO: wgpu needs to support retrieving the label
    GPUBindGroupLayout {
      id,
      label: "".to_string(),
    }
  }
}

#[derive(WebIDL)]
#[webidl(dictionary)]
pub(crate) struct GPUComputePipelineDescriptor {
  #[webidl(default = String::new())]
  pub label: String,

  pub compute: GPUProgrammableStage,
  pub layout: GPUPipelineLayoutOrGPUAutoLayoutMode,
}

#[derive(WebIDL)]
#[webidl(dictionary)]
pub(crate) struct GPUProgrammableStage {
  pub module: Ptr<GPUShaderModule>,
  pub entry_point: Option<String>,
  #[webidl(default = Default::default())]
  pub constants: IndexMap<String, f64>,
}
