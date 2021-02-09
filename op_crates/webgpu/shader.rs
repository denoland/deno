// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use deno_core::error::bad_resource_id;
use deno_core::error::AnyError;
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use deno_core::{serde_json, RcRef, ZeroCopyBuf};
use deno_core::{OpState, Resource};
use serde::Deserialize;
use std::borrow::Cow;

pub(crate) struct WebGPUShaderModule(pub(crate) wgpu_core::id::ShaderModuleId);
impl Resource for WebGPUShaderModule {
  fn name(&self) -> Cow<str> {
    "webGPUShaderModule".into()
  }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateShaderModuleArgs {
  instance_rid: u32,
  device_rid: u32,
  label: Option<String>,
  code: Option<String>,
  _source_map: Option<()>, // not in wgpu
}

pub fn op_webgpu_create_shader_module(
  state: &mut OpState,
  args: Value,
  zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let args: CreateShaderModuleArgs = serde_json::from_value(args)?;

  let device_resource = state
    .resource_table
    .get::<super::WebGPUDevice>(args.device_rid)
    .ok_or_else(bad_resource_id)?;
  let device = device_resource.0;
  let instance_resource = state
    .resource_table
    .get::<super::WebGPUInstance>(args.instance_rid)
    .ok_or_else(bad_resource_id)?;
  let instance = RcRef::map(&instance_resource, |r| &r.0)
    .try_borrow()
    .unwrap();

  let source = match args.code {
    Some(code) => {
      wgpu_core::pipeline::ShaderModuleSource::Wgsl(Cow::from(code))
    }
    None => wgpu_core::pipeline::ShaderModuleSource::SpirV(Cow::from(unsafe {
      let (prefix, data, suffix) = zero_copy[0].align_to::<u32>();
      assert!(prefix.is_empty());
      assert!(suffix.is_empty());
      data
    })),
  };

  let descriptor = wgpu_core::pipeline::ShaderModuleDescriptor {
    label: args.label.map(Cow::from),
    flags: Default::default(),
  };

  let shader_module = gfx_select_err!(device => instance.device_create_shader_module(
    device,
    &descriptor,
    source,
    std::marker::PhantomData
  ))?;

  let rid = state.resource_table.add(WebGPUShaderModule(shader_module));

  Ok(json!({
    "rid": rid,
  }))
}
