// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use deno_core::error::bad_resource_id;
use deno_core::error::null_opbuf;
use deno_core::error::AnyError;
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use deno_core::ResourceId;
use deno_core::ZeroCopyBuf;
use deno_core::{OpState, Resource};
use serde::Deserialize;
use std::borrow::Cow;

use super::error::WebGpuError;

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
  code: Option<String>,
  _source_map: Option<()>, // not yet implemented
}

pub fn op_webgpu_create_shader_module(
  state: &mut OpState,
  args: CreateShaderModuleArgs,
  zero_copy: Option<ZeroCopyBuf>,
) -> Result<Value, AnyError> {
  let instance = state.borrow::<super::Instance>();
  let device_resource = state
    .resource_table
    .get::<super::WebGpuDevice>(args.device_rid)
    .ok_or_else(bad_resource_id)?;
  let device = device_resource.0;

  let source = match args.code {
    Some(code) => {
      wgpu_core::pipeline::ShaderModuleSource::Wgsl(Cow::from(code))
    }
    None => wgpu_core::pipeline::ShaderModuleSource::SpirV(Cow::from(unsafe {
      match &zero_copy {
        Some(zero_copy) => {
          let (prefix, data, suffix) = zero_copy.align_to::<u32>();
          assert!(prefix.is_empty());
          assert!(suffix.is_empty());
          data
        }
        None => return Err(null_opbuf()),
      }
    })),
  };

  let mut flags = wgpu_types::ShaderFlags::default();
  flags.set(wgpu_types::ShaderFlags::VALIDATION, true);
  #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
  flags.set(wgpu_types::ShaderFlags::EXPERIMENTAL_TRANSLATION, true);

  let descriptor = wgpu_core::pipeline::ShaderModuleDescriptor {
    label: args.label.map(Cow::from),
    flags,
  };

  gfx_put!(device => instance.device_create_shader_module(
    device,
    &descriptor,
    source,
    std::marker::PhantomData
  ) => state, WebGpuShaderModule)
}
