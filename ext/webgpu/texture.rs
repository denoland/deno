// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use deno_core::error::AnyError;
use deno_core::op2;
use deno_core::OpState;
use deno_core::Resource;
use deno_core::ResourceId;
use serde::Deserialize;
use std::borrow::Cow;
use std::rc::Rc;

use super::error::WebGpuResult;
pub(crate) struct WebGpuTexture {
  pub(crate) instance: crate::Instance,
  pub(crate) id: wgpu_core::id::TextureId,
  pub(crate) owned: bool,
}

impl Resource for WebGpuTexture {
  fn name(&self) -> Cow<str> {
    "webGPUTexture".into()
  }

  fn close(self: Rc<Self>) {
    if self.owned {
      let instance = &self.instance;
      gfx_select!(self.id => instance.texture_drop(self.id, true));
    }
  }
}

pub(crate) struct WebGpuTextureView(
  pub(crate) crate::Instance,
  pub(crate) wgpu_core::id::TextureViewId,
);
impl Resource for WebGpuTextureView {
  fn name(&self) -> Cow<str> {
    "webGPUTextureView".into()
  }

  fn close(self: Rc<Self>) {
    gfx_select!(self.1 => self.0.texture_view_drop(self.1, true)).unwrap();
  }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateTextureArgs {
  device_rid: ResourceId,
  label: String,
  size: wgpu_types::Extent3d,
  mip_level_count: u32,
  sample_count: u32,
  dimension: wgpu_types::TextureDimension,
  format: wgpu_types::TextureFormat,
  usage: u32,
  view_formats: Vec<wgpu_types::TextureFormat>,
}

#[op2]
#[serde]
pub fn op_webgpu_create_texture(
  state: &mut OpState,
  #[serde] args: CreateTextureArgs,
) -> Result<WebGpuResult, AnyError> {
  let instance = state.borrow::<super::Instance>();
  let device_resource = state
    .resource_table
    .get::<super::WebGpuDevice>(args.device_rid)?;
  let device = device_resource.1;

  let descriptor = wgpu_core::resource::TextureDescriptor {
    label: Some(Cow::Owned(args.label)),
    size: args.size,
    mip_level_count: args.mip_level_count,
    sample_count: args.sample_count,
    dimension: args.dimension,
    format: args.format,
    usage: wgpu_types::TextureUsages::from_bits_truncate(args.usage),
    view_formats: args.view_formats,
  };

  let (val, maybe_err) = gfx_select!(device => instance.device_create_texture(
    device,
    &descriptor,
    None
  ));

  let rid = state.resource_table.add(WebGpuTexture {
    instance: instance.clone(),
    id: val,
    owned: true,
  });

  Ok(WebGpuResult::rid_err(rid, maybe_err))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateTextureViewArgs {
  texture_rid: ResourceId,
  label: String,
  format: Option<wgpu_types::TextureFormat>,
  dimension: Option<wgpu_types::TextureViewDimension>,
  #[serde(flatten)]
  range: wgpu_types::ImageSubresourceRange,
}

#[op2]
#[serde]
pub fn op_webgpu_create_texture_view(
  state: &mut OpState,
  #[serde] args: CreateTextureViewArgs,
) -> Result<WebGpuResult, AnyError> {
  let instance = state.borrow::<super::Instance>();
  let texture_resource = state
    .resource_table
    .get::<WebGpuTexture>(args.texture_rid)?;
  let texture = texture_resource.id;

  let descriptor = wgpu_core::resource::TextureViewDescriptor {
    label: Some(Cow::Owned(args.label)),
    format: args.format,
    dimension: args.dimension,
    range: args.range,
  };

  gfx_put!(texture => instance.texture_create_view(
    texture,
    &descriptor,
    None
  ) => state, WebGpuTextureView)
}
