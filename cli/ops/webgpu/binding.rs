// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use super::texture::{serialize_dimension, serialize_texture_format};
use deno_core::error::type_error;
use deno_core::error::AnyError;
use deno_core::error::{bad_resource_id, not_supported};
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use deno_core::BufVec;
use deno_core::OpState;
use deno_core::{serde_json, ZeroCopyBuf};
use serde::Deserialize;
use std::borrow::Cow;
use std::cell::RefCell;
use std::rc::Rc;

pub fn init(rt: &mut deno_core::JsRuntime) {
  super::super::reg_json_sync(rt, "op_webgpu_create_bind_group_layout", op_webgpu_create_bind_group_layout);
  super::super::reg_json_sync(rt, "op_webgpu_create_pipeline_layout", op_webgpu_create_pipeline_layout);
  super::super::reg_json_sync(rt, "op_webgpu_create_bind_group", op_webgpu_create_bind_group);
}

fn serialize_texture_component_type(
  component_type: String,
) -> Result<wgt::TextureComponentType, AnyError> {
  Ok(match component_type {
    &"float" => wgt::TextureComponentType::Float,
    &"sint" => wgt::TextureComponentType::Sint,
    &"uint" => wgt::TextureComponentType::Uint,
    &"depth-comparison" => return Err(not_supported()),
    _ => unreachable!(),
  })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GPUBindGroupLayoutEntry {
  binding: u32,
  visibility: u32,
  #[serde(rename = "type")]
  kind: String,
  has_dynamic_offset: Option<bool>,
  min_buffer_binding_size: Option<std::num::NonZeroU64>,
  view_dimension: Option<String>,
  texture_component_type: Option<String>,
  storage_texture_format: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateBindGroupLayoutArgs {
  instance_rid: u32,
  device_rid: u32,
  label: Option<String>,
  entries: [GPUBindGroupLayoutEntry],
}

pub fn op_webgpu_create_bind_group_layout(
  state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let args: CreateBindGroupLayoutArgs = serde_json::from_value(args)?;

  let instance = state
    .resource_table
    .get_mut::<super::WgcInstance>(args.instance_rid)
    .ok_or_else(bad_resource_id)?;
  let device = state
    .resource_table
    .get_mut::<wgc::id::DeviceId>(args.device_rid)
    .ok_or_else(bad_resource_id)?;

  let bind_group_layout = instance.device_create_bind_group_layout(
    *device,
    &wgc::binding_model::BindGroupLayoutDescriptor {
      label: args.label.map(|label| Cow::Borrowed(&label)),
      entries: Cow::Owned(
        args
          .entries
          .iter()
          .map(|entry| {
            wgt::BindGroupLayoutEntry {
              binding: entry.binding,
              visibility: wgt::ShaderStage::from_bits(entry.visibility)
                .unwrap(),
              ty: match entry.kind {
                &"uniform-buffer" => wgt::BindingType::UniformBuffer {
                  dynamic: entry.has_dynamic_offset.unwrap_or(false),
                  min_binding_size: entry.min_buffer_binding_size,
                },
                &"storage-buffer" => wgt::BindingType::StorageBuffer {
                  dynamic: entry.has_dynamic_offset.unwrap_or(false),
                  min_binding_size: entry.min_buffer_binding_size,
                  readonly: false,
                },
                &"readonly-storage-buffer" => wgt::BindingType::StorageBuffer {
                  dynamic: entry.has_dynamic_offset.unwrap_or(false),
                  min_binding_size: entry.min_buffer_binding_size,
                  readonly: true,
                },
                &"sampler" => wgt::BindingType::Sampler { comparison: false },
                &"comparison-sampler" => {
                  wgt::BindingType::Sampler { comparison: true }
                }
                &"sampled-texture" => wgt::BindingType::SampledTexture {
                  dimension: serialize_dimension(entry.view_dimension.unwrap()),
                  component_type: serialize_texture_component_type(
                    entry.texture_component_type.unwrap(),
                  )?,
                  multisampled: false,
                },
                &"multisampled-texture" => wgt::BindingType::SampledTexture {
                  dimension: serialize_dimension(entry.view_dimension.unwrap()),
                  component_type: serialize_texture_component_type(
                    entry.texture_component_type.unwrap(),
                  )?,
                  multisampled: true,
                },
                &"readonly-storage-texture" => {
                  wgt::BindingType::StorageTexture {
                    dimension: serialize_dimension(
                      entry.view_dimension.unwrap(),
                    ),
                    format: serialize_texture_format(
                      entry.storage_texture_format.unwrap(),
                    )?,
                    readonly: true,
                  }
                }
                &"writeonly-storage-texture" => {
                  wgt::BindingType::StorageTexture {
                    dimension: serialize_dimension(
                      entry.view_dimension.unwrap(),
                    ),
                    format: serialize_texture_format(
                      entry.storage_texture_format.unwrap(),
                    )?,
                    readonly: false,
                  }
                }
                _ => unreachable!(),
              },
              count: None,
            }
          })
          .collect::<Vec<wgt::BindGroupLayoutEntry>>(),
      ),
    },
    std::marker::PhantomData,
  )?;

  let rid = state
    .resource_table
    .add("webGPUBindGroupLayout", Box::new(bind_group_layout));

  Ok(json!({
    "rid": rid,
  }))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreatePipelineLayoutArgs {
  instance_rid: u32,
  device_rid: u32,
  label: Option<String>,
  bind_group_layouts: [u32],
}

pub fn op_webgpu_create_pipeline_layout(
  state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let args: CreatePipelineLayoutArgs = serde_json::from_value(args)?;

  let instance = state
    .resource_table
    .get_mut::<super::WgcInstance>(args.instance_rid)
    .ok_or_else(bad_resource_id)?;
  let device = state
    .resource_table
    .get_mut::<wgc::id::DeviceId>(args.device_rid)
    .ok_or_else(bad_resource_id)?;

  let pipeline_layout = instance.device_create_pipeline_layout(
    *device,
    &wgc::binding_model::PipelineLayoutDescriptor {
      label: args.label.map(|label| Cow::Borrowed(&label)),
      bind_group_layouts: Cow::Owned(
        args
          .bind_group_layouts
          .iter()
          .map(|rid| {
            state
              .resource_table
              .get_mut::<wgc::id::BindGroupLayoutId>(*rid)
              .ok_or_else(bad_resource_id)?
          })
          .collect::<Vec<wgc::id::BindGroupLayoutId>>(),
      ),
      push_constant_ranges: Default::default(),
    },
    std::marker::PhantomData,
  )?;

  let rid = state
    .resource_table
    .add("webGPUPipelineLayout", Box::new(pipeline_layout));

  Ok(json!({
    "rid": rid,
  }))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GPUBindGroupEntry {
  binding: u32,
  kind: String,
  resource: u32,
  offset: Option<u64>,
  size: Option<std::num::NonZeroU64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateBindGroupArgs {
  instance_rid: u32,
  device_rid: u32,
  label: Option<String>,
  layout: u32,
  entries: [GPUBindGroupEntry],
}

pub fn op_webgpu_create_bind_group(
  state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let args: CreateBindGroupArgs = serde_json::from_value(args)?;

  let instance = state
    .resource_table
    .get_mut::<super::WgcInstance>(args.instance_rid)
    .ok_or_else(bad_resource_id)?;
  let device = state
    .resource_table
    .get_mut::<wgc::id::DeviceId>(args.device_rid)
    .ok_or_else(bad_resource_id)?;

  let bind_group = instance.device_create_bind_group(
    *device,
    &wgc::binding_model::BindGroupDescriptor {
      label: args.label.map(|label| Cow::Borrowed(&label)),
      layout: *state
        .resource_table
        .get_mut::<wgc::id::BindGroupLayoutId>(args.layout)
        .ok_or_else(bad_resource_id)?,
      entries: Cow::Owned(
        args
          .entries
          .iter()
          .map(|entry| wgc::binding_model::BindGroupEntry {
            binding: entry.binding,
            resource: match entry.resource_kind {
              &"GPUSampler" => wgc::binding_model::BindingResource::Sampler(
                *state
                  .resource_table
                  .get_mut::<wgc::id::SamplerId>(entry.resource)
                  .ok_or_else(bad_resource_id)?,
              ),
              &"GPUTextureView" => {
                wgc::binding_model::BindingResource::TextureView(
                  *state
                    .resource_table
                    .get_mut::<wgc::id::TextureViewId>(entry.resource)
                    .ok_or_else(bad_resource_id)?,
                )
              }
              &"GPUBufferBinding" => {
                wgc::binding_model::BindingResource::Buffer(
                  wgc::binding_model::BufferBinding {
                    buffer_id: *state
                      .resource_table
                      .get_mut::<wgc::id::BufferId>(entry.resource)
                      .ok_or_else(bad_resource_id)?,
                    offset: entry.offset.unwrap_or(0),
                    size: entry.size,
                  },
                )
              }
              _ => unreachable!(),
            },
          })
          .collect::<Vec<wgc::binding_model::BindGroupEntry>>(),
      ),
    },
    std::marker::PhantomData,
  )?;

  let rid = state
    .resource_table
    .add("webGPUBindGroup", Box::new(bind_group));

  Ok(json!({
    "rid": rid,
  }))
}
