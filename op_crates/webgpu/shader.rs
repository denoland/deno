// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use deno_core::error::bad_resource_id;
use deno_core::error::AnyError;
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use deno_core::OpState;
use deno_core::{serde_json, ZeroCopyBuf};
use serde::Deserialize;
use std::borrow::Cow;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateShaderModuleArgs {
  instance_rid: u32,
  device_rid: u32,
  _label: Option<String>, // wgpu#977
  code: Option<String>,
  _source_map: Option<()>, // not in wgpu
}

pub fn op_webgpu_create_shader_module(
  state: &mut OpState,
  args: Value,
  zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let args: CreateShaderModuleArgs = serde_json::from_value(args)?;

  let device = *state
    .resource_table
    .get::<wgc::id::DeviceId>(args.device_rid)
    .ok_or_else(bad_resource_id)?;
  let instance = state
    .resource_table
    .get_mut::<super::WgcInstance>(args.instance_rid)
    .ok_or_else(bad_resource_id)?;

  let source = match args.code {
    Some(code) => wgc::pipeline::ShaderModuleSource::Wgsl(Cow::Owned(code)),
    None => wgc::pipeline::ShaderModuleSource::SpirV(Cow::Borrowed(unsafe {
      let (prefix, data, suffix) = zero_copy[0].align_to::<u32>();
      assert!(prefix.is_empty());
      assert!(suffix.is_empty());
      data
    })),
  };
  let shader_module = wgc::gfx_select!(device => instance.device_create_shader_module(
    device,
    source,
    std::marker::PhantomData
  ))?;

  let rid = state
    .resource_table
    .add("webGPUShaderModule", Box::new(shader_module));

  Ok(json!({
    "rid": rid,
  }))
}
