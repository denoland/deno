// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use deno_core::error::AnyError;
use deno_core::op;
use deno_core::ResourceId;
use deno_core::{OpState, Resource};
use serde::Deserialize;
use std::borrow::Cow;

use super::error::WebGpuResult;

pub(crate) struct WebGpuShaderModule(pub(crate) wgpu_core::id::ShaderModuleId);
impl Resource for WebGpuShaderModule {
  fn name(&self) -> Cow<str> {
    "webGPUShaderModule".into()
  }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateShaderModuleArgs {
  device_rid: ResourceId,
  label: Option<String>,
  code: String,
  _source_map: Option<()>, // not yet implemented
}

#[op]
pub fn op_webgpu_create_shader_module(
  state: &mut OpState,
  args: CreateShaderModuleArgs,
) -> Result<WebGpuResult, AnyError> {
  let instance = state.borrow::<super::Instance>();
  let device_resource = state
    .resource_table
    .get::<super::WebGpuDevice>(args.device_rid)?;
  let device = device_resource.0;

  let source =
    wgpu_core::pipeline::ShaderModuleSource::Wgsl(Cow::from(args.code));

  let descriptor = wgpu_core::pipeline::ShaderModuleDescriptor {
    label: args.label.map(Cow::from),
    shader_bound_checks: wgpu_types::ShaderBoundChecks::default(),
  };

  gfx_put!(device => instance.device_create_shader_module(
    device,
    &descriptor,
    source,
    std::marker::PhantomData
  ) => state, WebGpuShaderModule)
}
