// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use deno_core::error::AnyError;
use deno_core::op2;
use deno_core::OpState;
use deno_core::Resource;
use deno_core::ResourceId;
use std::borrow::Cow;
use std::rc::Rc;

use super::error::WebGpuResult;

pub(crate) struct WebGpuShaderModule(
  pub(crate) super::Instance,
  pub(crate) wgpu_core::id::ShaderModuleId,
);
impl Resource for WebGpuShaderModule {
  fn name(&self) -> Cow<str> {
    "webGPUShaderModule".into()
  }

  fn close(self: Rc<Self>) {
    gfx_select!(self.1 => self.0.shader_module_drop(self.1));
  }
}

#[op2]
#[serde]
pub fn op_webgpu_create_shader_module(
  state: &mut OpState,
  #[smi] device_rid: ResourceId,
  #[string] label: Cow<str>,
  #[string] code: Cow<str>,
) -> Result<WebGpuResult, AnyError> {
  let instance = state.borrow::<super::Instance>();
  let device_resource = state
    .resource_table
    .get::<super::WebGpuDevice>(device_rid)?;
  let device = device_resource.1;

  let source = wgpu_core::pipeline::ShaderModuleSource::Wgsl(code);

  let descriptor = wgpu_core::pipeline::ShaderModuleDescriptor {
    label: Some(label),
    shader_bound_checks: wgpu_types::ShaderBoundChecks::default(),
  };

  gfx_put!(device => instance.device_create_shader_module(
    device,
    &descriptor,
    source,
    None
  ) => state, WebGpuShaderModule)
}
