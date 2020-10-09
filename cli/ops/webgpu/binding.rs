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
use std::cell::RefCell;
use std::rc::Rc;

fn serialize_texture_component_type(
  component_type: String,
) -> Result<wgpu::TextureComponentType, AnyError> {
  Ok(match component_type {
    &"float" => wgpu::TextureComponentType::Float,
    &"sint" => wgpu::TextureComponentType::Sint,
    &"uint" => wgpu::TextureComponentType::Uint,
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
  rid: u32,
  label: Option<String>,
  entries: [GPUBindGroupLayoutEntry],
}

pub fn op_webgpu_create_bind_group_layout(
  state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let args: CreateBindGroupLayoutArgs = serde_json::from_value(args)?;

  let device = state
    .resource_table
    .get_mut::<wgpu::Device>(args.rid)
    .ok_or_else(bad_resource_id)?;

  let bind_group_layout =
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
      label: args.label.map(|label| &label),
      entries: &args
        .entries
        .iter()
        .map(|entry| {
          wgpu::BindGroupLayoutEntry {
            binding: entry.binding,
            visibility: wgpu::ShaderStage::from_bits(entry.visibility).unwrap(), // TODO
            ty: match entry.kind {
              &"uniform-buffer" => wgpu::BindingType::UniformBuffer {
                dynamic: entry.has_dynamic_offset.unwrap_or(false),
                min_binding_size: entry.min_buffer_binding_size,
              },
              &"storage-buffer" => wgpu::BindingType::StorageBuffer {
                dynamic: entry.has_dynamic_offset.unwrap_or(false),
                min_binding_size: entry.min_buffer_binding_size,
                readonly: false,
              },
              &"readonly-storage-buffer" => wgpu::BindingType::StorageBuffer {
                dynamic: entry.has_dynamic_offset.unwrap_or(false),
                min_binding_size: entry.min_buffer_binding_size,
                readonly: true,
              },
              &"sampler" => wgpu::BindingType::Sampler { comparison: false },
              &"comparison-sampler" => {
                wgpu::BindingType::Sampler { comparison: true }
              }
              &"sampled-texture" => wgpu::BindingType::SampledTexture {
                dimension: serialize_dimension(entry.view_dimension.unwrap()), // TODO
                component_type: serialize_texture_component_type(
                  entry.texture_component_type.unwrap(),
                )?, // TODO
                multisampled: false,
              },
              &"multisampled-texture" => wgpu::BindingType::SampledTexture {
                dimension: serialize_dimension(entry.view_dimension.unwrap()), // TODO
                component_type: serialize_texture_component_type(
                  entry.texture_component_type.unwrap(),
                )?, // TODO
                multisampled: true,
              },
              &"readonly-storage-texture" => {
                wgpu::BindingType::StorageTexture {
                  dimension: serialize_dimension(entry.view_dimension.unwrap()), // TODO
                  format: serialize_texture_format(
                    entry.storage_texture_format.unwrap(),
                  )?, // TODO
                  readonly: true,
                }
              }
              &"writeonly-storage-texture" => {
                wgpu::BindingType::StorageTexture {
                  dimension: serialize_dimension(entry.view_dimension.unwrap()), // TODO
                  format: serialize_texture_format(
                    entry.storage_texture_format.unwrap(),
                  )?, // TODO
                  readonly: false,
                }
              }
              _ => unreachable!(),
            },
            count: None, // TODO
          }
        })
        .collect::<[wgpu::BindGroupLayoutEntry]>(),
    });

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
  rid: u32,
  label: Option<String>,
  bind_group_layouts: [u32],
}

pub fn op_webgpu_create_pipeline_layout(
  state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let args: CreatePipelineLayoutArgs = serde_json::from_value(args)?;

  let device = state
    .resource_table
    .get_mut::<wgpu::Device>(args.rid)
    .ok_or_else(bad_resource_id)?;

  let pipeline_layout =
    device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
      label: args.label.map(|label| &label),
      bind_group_layouts: &args
        .bind_group_layouts
        .iter()
        .map(|rid| {
          state
            .resource_table
            .get_mut::<wgpu::BindGroupLayout>(*rid)
            .ok_or_else(bad_resource_id)?
        })
        .collect::<[&wgpu::BindGroupLayout]>(),
      push_constant_ranges: &[], // TODO
    });

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
  resource_kind: String,
  resource: u32, // TODO
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateBindGroupArgs {
  rid: u32,
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

  let device = state
    .resource_table
    .get_mut::<wgpu::Device>(args.rid)
    .ok_or_else(bad_resource_id)?;

  let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
    label: args.label.map(|label| &label),
    layout: state
      .resource_table
      .get_mut::<wgpu::BindGroupLayout>(args.layout)
      .ok_or_else(bad_resource_id)?,
    entries: &args
      .entries
      .iter()
      .map(|entry| {
        let resource = state
          .resource_table
          .get_mut(entry.resource)
          .ok_or_else(bad_resource_id)?;

        wgpu::BindGroupEntry {
          binding: entry.binding,
          resource: match entry.resource_kind {
            &"GPUSampler" => {
              wgpu::BindingResource::Sampler(resource as &mut wgpu::Sampler)
            }
            &"GPUTextureView" => wgpu::BindingResource::TextureView(
              resource as &mut wgpu::TextureView,
            ),
            &"GPUBufferBinding" => wgpu::BindingResource::Buffer(), // TODO
            _ => unreachable!(),
          },
        }
      })
      .collect::<[wgpu::BindGroupEntry]>(),
  });

  let rid = state
    .resource_table
    .add("webGPUBindGroup", Box::new(bind_group));

  Ok(json!({
    "rid": rid,
  }))
}
