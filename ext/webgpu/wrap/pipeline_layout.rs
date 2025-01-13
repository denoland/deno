// Copyright 2018-2025 the Deno authors. MIT license.

use deno_core::cppgc::Ptr;
use deno_core::op2;
use deno_core::webidl::Nullable;
use deno_core::webidl::WebIdlInterfaceConverter;
use deno_core::GarbageCollected;
use deno_core::WebIDL;

pub struct GPUPipelineLayout {
  pub id: wgpu_core::id::PipelineLayoutId,
  pub label: String,
}

impl WebIdlInterfaceConverter for GPUPipelineLayout {
  const NAME: &'static str = "GPUPipelineLayout";
}

impl GarbageCollected for GPUPipelineLayout {}

#[op2]
impl GPUPipelineLayout {
  crate::with_label!();
}

#[derive(WebIDL)]
#[webidl(dictionary)]
pub(crate) struct GPUPipelineLayoutDescriptor {
  #[webidl(default = String::new())]
  pub label: String,

  pub bind_group_layouts:
    Vec<Nullable<Ptr<super::bind_group_layout::GPUBindGroupLayout>>>,
}
