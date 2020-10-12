// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use deno_core::error::type_error;
use deno_core::error::AnyError;
use deno_core::error::{bad_resource_id, not_supported};
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use deno_core::BufVec;
use deno_core::OpState;
use deno_core::{serde_json, ZeroCopyBuf};
use serde::Deserialize;
use std::cell::RefCell;
use std::rc::Rc;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateShaderModuleArgs {
  instance_rid: u32,
  device_rid: u32,
  label: Option<String>, // wgpu#977
  code: String,
  source_map: (), // TODO: https://gpuweb.github.io/gpuweb/#shader-module-creation
}

pub fn op_webgpu_create_shader_module(
  state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let args: CreateShaderModuleArgs = serde_json::from_value(args)?;

  let instance = state
    .resource_table
    .get_mut::<super::WgcInstance>(args.instance_rid)
    .ok_or_else(bad_resource_id)?;
  let device = state
    .resource_table
    .get_mut::<wgc::id::DeviceId>(args.device_rid)
    .ok_or_else(bad_resource_id)?;

  let shader_module = instance.device_create_shader_module(
    *device,
    wgc::pipeline::ShaderModuleSource, // TODO
    (),                                // TODO: id_in
  )?;

  let rid = state
    .resource_table
    .add("webGPUShaderModule", Box::new(shader_module));

  Ok(json!({
    "rid": rid,
  }))
}
