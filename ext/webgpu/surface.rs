// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use super::WebGpuResult;
use deno_core::error::AnyError;
use deno_core::op2;
use deno_core::OpState;
use deno_core::Resource;
use deno_core::ResourceId;
use serde::Deserialize;
use std::borrow::Cow;
use std::rc::Rc;
use wgpu_types::SurfaceStatus;

pub struct WebGpuSurface(pub crate::Instance, pub wgpu_core::id::SurfaceId);
impl Resource for WebGpuSurface {
  fn name(&self) -> Cow<str> {
    "webGPUSurface".into()
  }

  fn close(self: Rc<Self>) {
    self.0.surface_drop(self.1);
  }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SurfaceConfigureArgs {
  surface_rid: ResourceId,
  device_rid: ResourceId,
  format: wgpu_types::TextureFormat,
  usage: u32,
  width: u32,
  height: u32,
  present_mode: Option<wgpu_types::PresentMode>,
  alpha_mode: wgpu_types::CompositeAlphaMode,
  view_formats: Vec<wgpu_types::TextureFormat>,
}

#[op2]
#[serde]
pub fn op_webgpu_surface_configure(
  state: &mut OpState,
  #[serde] args: SurfaceConfigureArgs,
) -> Result<WebGpuResult, AnyError> {
  let instance = state.borrow::<super::Instance>();
  let device_resource = state
    .resource_table
    .get::<super::WebGpuDevice>(args.device_rid)?;
  let device = device_resource.1;
  let surface_resource = state
    .resource_table
    .get::<WebGpuSurface>(args.surface_rid)?;
  let surface = surface_resource.1;

  let conf = wgpu_types::SurfaceConfiguration::<Vec<wgpu_types::TextureFormat>> {
    usage: wgpu_types::TextureUsages::from_bits_truncate(args.usage),
    format: args.format,
    width: args.width,
    height: args.height,
    present_mode: args.present_mode.unwrap_or_default(),
    alpha_mode: args.alpha_mode,
    view_formats: args.view_formats,
    desired_maximum_frame_latency: 2,
  };

  let err =
    gfx_select!(device => instance.surface_configure(surface, device, &conf));

  Ok(WebGpuResult::maybe_err(err))
}

#[op2]
#[serde]
pub fn op_webgpu_surface_get_current_texture(
  state: &mut OpState,
  #[smi] device_rid: ResourceId,
  #[smi] surface_rid: ResourceId,
) -> Result<WebGpuResult, AnyError> {
  let instance = state.borrow::<super::Instance>();
  let device_resource = state
    .resource_table
    .get::<super::WebGpuDevice>(device_rid)?;
  let device = device_resource.1;
  let surface_resource =
    state.resource_table.get::<WebGpuSurface>(surface_rid)?;
  let surface = surface_resource.1;

  let output =
    gfx_select!(device => instance.surface_get_current_texture(surface, None))?;

  match output.status {
    SurfaceStatus::Good | SurfaceStatus::Suboptimal => {
      let id = output.texture_id.unwrap();
      let rid = state.resource_table.add(crate::texture::WebGpuTexture {
        instance: instance.clone(),
        id,
        owned: false,
      });
      Ok(WebGpuResult::rid(rid))
    }
    _ => Err(AnyError::msg("Invalid Surface Status")),
  }
}

#[op2(fast)]
pub fn op_webgpu_surface_present(
  state: &mut OpState,
  #[smi] device_rid: ResourceId,
  #[smi] surface_rid: ResourceId,
) -> Result<(), AnyError> {
  let instance = state.borrow::<super::Instance>();
  let device_resource = state
    .resource_table
    .get::<super::WebGpuDevice>(device_rid)?;
  let device = device_resource.1;
  let surface_resource =
    state.resource_table.get::<WebGpuSurface>(surface_rid)?;
  let surface = surface_resource.1;

  let _ = gfx_select!(device => instance.surface_present(surface))?;

  Ok(())
}
