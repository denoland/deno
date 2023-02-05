// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use super::WebGpuResult;
use deno_core::error::AnyError;
use deno_core::include_js_files;
use deno_core::op;
use deno_core::Extension;
use deno_core::OpState;
use deno_core::Resource;
use deno_core::ResourceId;
use serde::Deserialize;
use std::borrow::Cow;
use wgpu_types::SurfaceStatus;

pub fn init_surface(unstable: bool) -> Extension {
  Extension::builder("deno_webgpu_surface")
    .dependencies(vec!["deno_webidl", "deno_web", "deno_webgpu"])
    .js(include_js_files!(
      prefix "internal:deno_webgpu",
      "03_surface.js",
      "04_surface_idl_types.js",
    ))
    .ops(vec![
      op_webgpu_surface_configure::decl(),
      op_webgpu_surface_get_current_texture::decl(),
      op_webgpu_surface_present::decl(),
    ])
    .state(move |state| {
      // TODO: check & possibly streamline this
      // Unstable might be able to be OpMiddleware
      // let unstable_checker = state.borrow::<super::UnstableChecker>();
      // let unstable = unstable_checker.unstable;
      state.put(super::Unstable(unstable));
      Ok(())
    })
    .build()
}

pub struct WebGpuSurface(pub wgpu_core::id::SurfaceId);
impl Resource for WebGpuSurface {
  fn name(&self) -> Cow<str> {
    "webGPUSurface".into()
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

#[op]
pub fn op_webgpu_surface_configure(
  state: &mut OpState,
  args: SurfaceConfigureArgs,
) -> Result<WebGpuResult, AnyError> {
  let instance = state.borrow::<super::Instance>();
  let device_resource = state
    .resource_table
    .get::<super::WebGpuDevice>(args.device_rid)?;
  let device = device_resource.0;
  let surface_resource = state
    .resource_table
    .get::<WebGpuSurface>(args.surface_rid)?;
  let surface = surface_resource.0;

  let conf = wgpu_types::SurfaceConfiguration::<Vec<wgpu_types::TextureFormat>> {
    usage: wgpu_types::TextureUsages::from_bits_truncate(args.usage),
    format: args.format,
    width: args.width,
    height: args.height,
    present_mode: args.present_mode.unwrap_or_default(),
    alpha_mode: args.alpha_mode,
    view_formats: args.view_formats,
  };

  let err =
    gfx_select!(device => instance.surface_configure(surface, device, &conf));

  Ok(WebGpuResult::maybe_err(err))
}

#[op]
pub fn op_webgpu_surface_get_current_texture(
  state: &mut OpState,
  device_rid: ResourceId,
  surface_rid: ResourceId,
) -> Result<WebGpuResult, AnyError> {
  let instance = state.borrow::<super::Instance>();
  let device_resource = state
    .resource_table
    .get::<super::WebGpuDevice>(device_rid)?;
  let device = device_resource.0;
  let surface_resource =
    state.resource_table.get::<WebGpuSurface>(surface_rid)?;
  let surface = surface_resource.0;

  let output =
    gfx_select!(device => instance.surface_get_current_texture(surface, ()))?;

  match output.status {
    SurfaceStatus::Good | SurfaceStatus::Suboptimal => {
      let id = output.texture_id.unwrap();
      let rid = state.resource_table.add(crate::texture::WebGpuTexture(id));
      Ok(WebGpuResult::rid(rid))
    }
    _ => Err(AnyError::msg("Invalid Surface Status")),
  }
}

#[op]
pub fn op_webgpu_surface_present(
  state: &mut OpState,
  device_rid: ResourceId,
  surface_rid: ResourceId,
) -> Result<(), AnyError> {
  let instance = state.borrow::<super::Instance>();
  let device_resource = state
    .resource_table
    .get::<super::WebGpuDevice>(device_rid)?;
  let device = device_resource.0;
  let surface_resource =
    state.resource_table.get::<WebGpuSurface>(surface_rid)?;
  let surface = surface_resource.0;

  let _ = gfx_select!(device => instance.surface_present(surface))?;

  Ok(())
}
