// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

mod binding;
mod buffer;
mod bundle;
mod command_encoder;
mod compute_pass;
mod pipeline;
mod queue;
mod render_pass;
mod sampler;
mod shader;
mod texture;

use deno_core::error::bad_resource_id;
use deno_core::error::type_error;
use deno_core::error::AnyError;
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use deno_core::BufVec;
use deno_core::OpState;
use deno_core::{serde_json, ZeroCopyBuf};
use serde::Deserialize;
use std::cell::RefCell;
use std::rc::Rc;

pub fn init(rt: &mut deno_core::JsRuntime) {
  super::reg_json_sync(
    rt,
    "op_webgpu_create_instance",
    op_webgpu_create_instance,
  );
  super::reg_json_async(
    rt,
    "op_webgpu_request_adapter",
    op_webgpu_request_adapter,
  );
  super::reg_json_async(
    rt,
    "op_webgpu_request_device",
    op_webgpu_request_device,
  );

  super::reg_json_sync(
    rt,
    "op_webgpu_create_buffer",
    buffer::op_webgpu_create_buffer,
  );
  super::reg_json_async(
    rt,
    "op_webgpu_buffer_get_map_async",
    buffer::op_webgpu_buffer_get_map_async,
  );
  super::reg_json_async(
    rt,
    "op_webgpu_buffer_get_mapped_range",
    buffer::op_webgpu_buffer_get_mapped_range,
  );
  super::reg_json_sync(
    rt,
    "op_webgpu_buffer_unmap",
    buffer::op_webgpu_buffer_unmap,
  );

  super::reg_json_sync(
    rt,
    "op_webgpu_create_texture",
    texture::op_webgpu_create_texture,
  );
  super::reg_json_sync(
    rt,
    "op_webgpu_create_texture_view",
    texture::op_webgpu_create_texture_view,
  );

  super::reg_json_sync(
    rt,
    "op_webgpu_create_sampler",
    sampler::op_webgpu_create_sampler,
  );

  super::reg_json_sync(
    rt,
    "op_webgpu_create_bind_group_layout",
    binding::op_webgpu_create_bind_group_layout,
  );
  super::reg_json_sync(
    rt,
    "op_webgpu_create_pipeline_layout",
    binding::op_webgpu_create_pipeline_layout,
  );
  super::reg_json_sync(
    rt,
    "op_webgpu_create_bind_group",
    binding::op_webgpu_create_bind_group,
  );

  super::reg_json_sync(
    rt,
    "op_webgpu_create_compute_pipeline",
    pipeline::op_webgpu_create_compute_pipeline,
  );
  super::reg_json_sync(
    rt,
    "op_webgpu_compute_pipeline_get_bind_group_layout",
    pipeline::op_webgpu_compute_pipeline_get_bind_group_layout,
  );
  super::reg_json_sync(
    rt,
    "op_webgpu_create_render_pipeline",
    pipeline::op_webgpu_create_render_pipeline,
  );
  super::reg_json_sync(
    rt,
    "op_webgpu_render_pipeline_get_bind_group_layout",
    pipeline::op_webgpu_render_pipeline_get_bind_group_layout,
  );

  super::reg_json_sync(
    rt,
    "op_webgpu_create_command_encoder",
    command_encoder::op_webgpu_create_command_encoder,
  );
  super::reg_json_sync(
    rt,
    "op_webgpu_command_encoder_begin_render_pass",
    command_encoder::op_webgpu_command_encoder_begin_render_pass,
  );
  super::reg_json_sync(
    rt,
    "op_webgpu_command_encoder_begin_compute_pass",
    command_encoder::op_webgpu_command_encoder_begin_compute_pass,
  );
  super::reg_json_sync(
    rt,
    "op_webgpu_command_encoder_copy_buffer_to_buffer",
    command_encoder::op_webgpu_command_encoder_copy_buffer_to_buffer,
  );
  super::reg_json_sync(
    rt,
    "op_webgpu_command_encoder_copy_buffer_to_texture",
    command_encoder::op_webgpu_command_encoder_copy_buffer_to_texture,
  );
  super::reg_json_sync(
    rt,
    "op_webgpu_command_encoder_copy_texture_to_buffer",
    command_encoder::op_webgpu_command_encoder_copy_texture_to_buffer,
  );
  super::reg_json_sync(
    rt,
    "op_webgpu_command_encoder_copy_texture_to_texture",
    command_encoder::op_webgpu_command_encoder_copy_texture_to_texture,
  );
  super::reg_json_sync(
    rt,
    "op_webgpu_command_encoder_push_debug_group",
    command_encoder::op_webgpu_command_encoder_push_debug_group,
  );
  super::reg_json_sync(
    rt,
    "op_webgpu_command_encoder_pop_debug_group",
    command_encoder::op_webgpu_command_encoder_pop_debug_group,
  );
  super::reg_json_sync(
    rt,
    "op_webgpu_command_encoder_insert_debug_marker",
    command_encoder::op_webgpu_command_encoder_insert_debug_marker,
  );
  super::reg_json_sync(
    rt,
    "op_webgpu_command_encoder_finish",
    command_encoder::op_webgpu_command_encoder_finish,
  );

  super::reg_json_sync(
    rt,
    "op_webgpu_render_pass_set_viewport",
    render_pass::op_webgpu_render_pass_set_viewport,
  );
  super::reg_json_sync(
    rt,
    "op_webgpu_render_pass_set_scissor_rect",
    render_pass::op_webgpu_render_pass_set_scissor_rect,
  );
  super::reg_json_sync(
    rt,
    "op_webgpu_render_pass_set_blend_color",
    render_pass::op_webgpu_render_pass_set_blend_color,
  );
  super::reg_json_sync(
    rt,
    "op_webgpu_render_pass_set_stencil_reference",
    render_pass::op_webgpu_render_pass_set_stencil_reference,
  );
  super::reg_json_sync(
    rt,
    "op_webgpu_render_pass_execute_bundles",
    render_pass::op_webgpu_render_pass_execute_bundles,
  );
  super::reg_json_sync(
    rt,
    "op_webgpu_render_pass_end_pass",
    render_pass::op_webgpu_render_pass_end_pass,
  );
  super::reg_json_sync(
    rt,
    "op_webgpu_render_pass_set_bind_group",
    render_pass::op_webgpu_render_pass_set_bind_group,
  );
  super::reg_json_sync(
    rt,
    "op_webgpu_render_pass_push_debug_group",
    render_pass::op_webgpu_render_pass_push_debug_group,
  );
  super::reg_json_sync(
    rt,
    "op_webgpu_render_pass_pop_debug_group",
    render_pass::op_webgpu_render_pass_pop_debug_group,
  );
  super::reg_json_sync(
    rt,
    "op_webgpu_render_pass_insert_debug_marker",
    render_pass::op_webgpu_render_pass_insert_debug_marker,
  );
  super::reg_json_sync(
    rt,
    "op_webgpu_render_pass_set_pipeline",
    render_pass::op_webgpu_render_pass_set_pipeline,
  );
  super::reg_json_sync(
    rt,
    "op_webgpu_render_pass_set_index_buffer",
    render_pass::op_webgpu_render_pass_set_index_buffer,
  );
  super::reg_json_sync(
    rt,
    "op_webgpu_render_pass_set_vertex_buffer",
    render_pass::op_webgpu_render_pass_set_vertex_buffer,
  );
  super::reg_json_sync(
    rt,
    "op_webgpu_render_pass_draw",
    render_pass::op_webgpu_render_pass_draw,
  );
  super::reg_json_sync(
    rt,
    "op_webgpu_render_pass_draw_indexed",
    render_pass::op_webgpu_render_pass_draw_indexed,
  );
  super::reg_json_sync(
    rt,
    "op_webgpu_render_pass_draw_indirect",
    render_pass::op_webgpu_render_pass_draw_indirect,
  );
  super::reg_json_sync(
    rt,
    "op_webgpu_render_pass_draw_indexed_indirect",
    render_pass::op_webgpu_render_pass_draw_indexed_indirect,
  );

  super::reg_json_sync(
    rt,
    "op_webgpu_compute_pass_set_pipeline",
    compute_pass::op_webgpu_compute_pass_set_pipeline,
  );
  super::reg_json_sync(
    rt,
    "op_webgpu_compute_pass_dispatch",
    compute_pass::op_webgpu_compute_pass_dispatch,
  );
  super::reg_json_sync(
    rt,
    "op_webgpu_compute_pass_dispatch_indirect",
    compute_pass::op_webgpu_compute_pass_dispatch_indirect,
  );
  super::reg_json_sync(
    rt,
    "op_webgpu_compute_pass_end_pass",
    compute_pass::op_webgpu_compute_pass_end_pass,
  );
  super::reg_json_sync(
    rt,
    "op_webgpu_compute_pass_set_bind_group",
    compute_pass::op_webgpu_compute_pass_set_bind_group,
  );
  super::reg_json_sync(
    rt,
    "op_webgpu_compute_pass_push_debug_group",
    compute_pass::op_webgpu_compute_pass_push_debug_group,
  );
  super::reg_json_sync(
    rt,
    "op_webgpu_compute_pass_pop_debug_group",
    compute_pass::op_webgpu_compute_pass_pop_debug_group,
  );
  super::reg_json_sync(
    rt,
    "op_webgpu_compute_pass_insert_debug_marker",
    compute_pass::op_webgpu_compute_pass_insert_debug_marker,
  );

  super::reg_json_sync(
    rt,
    "op_webgpu_create_render_bundle_encoder",
    bundle::op_webgpu_create_render_bundle_encoder,
  );
  super::reg_json_sync(
    rt,
    "op_webgpu_render_bundle_encoder_finish",
    bundle::op_webgpu_render_bundle_encoder_finish,
  );
  super::reg_json_sync(
    rt,
    "op_webgpu_render_bundle_encoder_set_bind_group",
    bundle::op_webgpu_render_bundle_encoder_set_bind_group,
  );
  super::reg_json_sync(
    rt,
    "op_webgpu_render_bundle_encoder_push_debug_group",
    bundle::op_webgpu_render_bundle_encoder_push_debug_group,
  );
  super::reg_json_sync(
    rt,
    "op_webgpu_render_bundle_encoder_pop_debug_group",
    bundle::op_webgpu_render_bundle_encoder_pop_debug_group,
  );
  super::reg_json_sync(
    rt,
    "op_webgpu_render_bundle_encoder_insert_debug_marker",
    bundle::op_webgpu_render_bundle_encoder_insert_debug_marker,
  );
  super::reg_json_sync(
    rt,
    "op_webgpu_render_bundle_encoder_set_pipeline",
    bundle::op_webgpu_render_bundle_encoder_set_pipeline,
  );
  super::reg_json_sync(
    rt,
    "op_webgpu_render_bundle_encoder_set_index_buffer",
    bundle::op_webgpu_render_bundle_encoder_set_index_buffer,
  );
  super::reg_json_sync(
    rt,
    "op_webgpu_render_bundle_encoder_set_vertex_buffer",
    bundle::op_webgpu_render_bundle_encoder_set_vertex_buffer,
  );
  super::reg_json_sync(
    rt,
    "op_webgpu_render_bundle_encoder_draw",
    bundle::op_webgpu_render_bundle_encoder_draw,
  );
  super::reg_json_sync(
    rt,
    "op_webgpu_render_bundle_encoder_draw_indexed",
    bundle::op_webgpu_render_bundle_encoder_draw_indexed,
  );
  super::reg_json_sync(
    rt,
    "op_webgpu_render_bundle_encoder_draw_indirect",
    bundle::op_webgpu_render_bundle_encoder_draw_indirect,
  );

  super::reg_json_sync(
    rt,
    "op_webgpu_queue_submit",
    queue::op_webgpu_queue_submit,
  );
  super::reg_json_sync(
    rt,
    "op_webgpu_write_buffer",
    queue::op_webgpu_write_buffer,
  );
  super::reg_json_sync(
    rt,
    "op_webgpu_write_texture",
    queue::op_webgpu_write_texture,
  );

  super::reg_json_sync(
    rt,
    "op_webgpu_create_shader_module",
    shader::op_webgpu_create_shader_module,
  );
}

fn serialize_features(features: &wgt::Features) -> Vec<&str> {
  let mut extensions: Vec<&str> = vec![];

  if features.contains(wgt::Features::DEPTH_CLAMPING) {
    extensions.push("depth-clamping");
  }
  if features.contains(wgt::Features) {
    // TODO
    extensions.push("depth24unorm-stencil8");
  }
  if features.contains(wgt::Features) {
    // TODO
    extensions.push("depth32float-stencil8");
  }
  if features.contains(wgt::Features) {
    // TODO
    extensions.push("pipeline-statistics-query");
  }
  if features.contains(wgt::Features::TEXTURE_COMPRESSION_BC) {
    extensions.push("texture-compression-bc");
  }
  if features.contains(wgt::Features) {
    // TODO
    extensions.push("timestamp-query");
  }

  extensions
}

pub type WgcInstance = wgc::hub::Global<wgc::hub::IdentityManagerFactory>;

pub fn op_webgpu_create_instance(
  state: &mut OpState,
  _args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let instance = wgc::hub::Global::new(
    "webgpu",
    wgc::hub::IdentityManagerFactory,
    wgt::BackendBit::PRIMARY,
  );

  let rid = state
    .resource_table
    .add("webGPUInstance", Box::new(adapter));

  Ok(json!({
    "rid": rid,
  }))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RequestAdapterArgs {
  instance_rid: u32,
  power_preference: Option<String>,
}

pub async fn op_webgpu_request_adapter(
  state: Rc<RefCell<OpState>>,
  args: Value,
  _bufs: BufVec,
) -> Result<Value, AnyError> {
  let args: RequestAdapterArgs = serde_json::from_value(args)?;

  let mut state = state.borrow_mut();
  let instance = state
    .resource_table
    .get_mut::<WgcInstance>(args.instance_rid)
    .ok_or_else(bad_resource_id)?;

  let adapter = instance.request_adapter(
    &wgc::instance::RequestAdapterOptions {
      power_preference: match args.power_preference {
        Some(&"low-power") => wgt::PowerPreference::LowPower,
        Some(&"high-performance") => wgt::PowerPreference::HighPerformance,
        Some(_) => unreachable!(),
        None => wgt::PowerPreference::Default,
      },
      compatible_surface: None, // windowless
    },
    wgc::instance::AdapterInputs::Mask(wgt::BackendBit::PRIMARY, ()), // TODO
  )?;

  let name = instance.adapter_get_info(adapter)?.name;
  let features = serialize_features(&instance.adapter_features(adapter)?);

  let rid = state
    .resource_table
    .add("webGPUInstance", Box::new(instance));
  let rid = state.resource_table.add("webGPUAdapter", Box::new(adapter));

  Ok(json!({
    "rid": rid,
    "name": name,
    "features": features,
  }))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GPULimits {
  // TODO: each should be Option
  max_bind_groups: u32,
  max_dynamic_uniform_buffers_per_pipeline_layout: u32,
  max_dynamic_storage_buffers_per_pipeline_layout: u32,
  max_sampled_textures_per_shader_stage: u32,
  max_samplers_per_shader_stage: u32,
  max_storage_buffers_per_shader_stage: u32,
  max_storage_textures_per_shader_stage: u32,
  max_uniform_buffers_per_shader_stage: u32,
  max_uniform_buffer_binding_size: u32,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RequestDeviceArgs {
  instance_rid: u32,
  adapter_rid: u32,
  label: Option<String>, // wgpu#976
  features: Option<[String]>,
  limits: Option<GPULimits>,
}

pub async fn op_webgpu_request_device(
  state: Rc<RefCell<OpState>>,
  args: Value,
  _bufs: BufVec,
) -> Result<Value, AnyError> {
  let args: RequestDeviceArgs = serde_json::from_value(args)?;

  let mut state = state.borrow_mut();
  let instance = state
    .resource_table
    .get_mut::<WgcInstance>(args.instance_rid)
    .ok_or_else(bad_resource_id)?;
  let adapter = state
    .resource_table
    .get_mut::<wgc::id::AdapterId>(args.adapter_rid)
    .ok_or_else(bad_resource_id)?;

  let device = instance.adapter_request_device(
    *adapter,
    &wgt::DeviceDescriptor {
      features: Default::default(), // TODO
      limits: args.limits.map_or(Default::default(), |limits| {
        wgt::Limits {
          max_bind_groups: limits.max_bind_groups,
          max_dynamic_uniform_buffers_per_pipeline_layout: limits
            .max_dynamic_uniform_buffers_per_pipeline_layout,
          max_dynamic_storage_buffers_per_pipeline_layout: limits
            .max_dynamic_storage_buffers_per_pipeline_layout,
          max_sampled_textures_per_shader_stage: limits
            .max_sampled_textures_per_shader_stage,
          max_samplers_per_shader_stage: limits.max_samplers_per_shader_stage,
          max_storage_buffers_per_shader_stage: limits
            .max_storage_buffers_per_shader_stage,
          max_storage_textures_per_shader_stage: limits
            .max_storage_textures_per_shader_stage,
          max_uniform_buffers_per_shader_stage: limits
            .max_uniform_buffers_per_shader_stage,
          max_uniform_buffer_binding_size: limits
            .max_uniform_buffer_binding_size,
          max_push_constant_size: 0, // TODO
        }
      }),
      shader_validation: false, // TODO
    },
    None,
    std::marker::PhantomData,
  )?;

  let features = serialize_features(&instance.device_features(device)?);
  let limits = instance.device_limits(device)?;
  let json_limits = json!({
     "max_bind_groups": limits.max_bind_groups,
     "max_dynamic_uniform_buffers_per_pipeline_layout": limits.max_dynamic_uniform_buffers_per_pipeline_layout,
     "max_dynamic_storage_buffers_per_pipeline_layout": limits.max_dynamic_storage_buffers_per_pipeline_layout,
     "max_sampled_textures_per_shader_stage": limits.max_sampled_textures_per_shader_stage,
     "max_samplers_per_shader_stage": limits.max_samplers_per_shader_stage,
     "max_storage_buffers_per_shader_stage": limits.max_storage_buffers_per_shader_stage,
     "max_storage_textures_per_shader_stage": limits.max_storage_textures_per_shader_stage,
     "max_uniform_buffers_per_shader_stage": limits.max_uniform_buffers_per_shader_stage,
     "max_uniform_buffer_binding_size": limits.max_uniform_buffer_binding_size,
  });

  let rid = state.resource_table.add("webGPUDevice", Box::new(device));

  Ok(json!({
    "rid": rid,
    "features": features,
    "limits": json_limits,
  }))
}
