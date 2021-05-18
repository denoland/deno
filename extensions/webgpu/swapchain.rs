use deno_core::error::bad_resource_id;
use deno_core::error::AnyError;
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use deno_core::OpState;
use deno_core::Resource;
use deno_core::ResourceId;
use deno_core::ZeroCopyBuf;
use serde::Deserialize;
use std::borrow::Cow;

struct WebGpuRawWindowHandle(Box<dyn raw_window_handle::HasRawWindowHandle>);
impl Resource for WebGpuRawWindowHandle {
  fn name(&self) -> Cow<str> {
    "webGPURawWindowHandle".into()
  }
}

struct WebGpuSurface(wgpu_core::id::SurfaceId);
impl Resource for WebGpuSwapChain {
  fn name(&self) -> Cow<str> {
    "webGPUSurface".into()
  }
}

struct WebGpuSwapChain(wgpu_core::id::SwapChainId);
impl Resource for WebGpuSwapChain {
  fn name(&self) -> Cow<str> {
    "webGPUSwapChain".into()
  }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSurfaceArgs {
  device_rid: u32,
  raw_window_handle_rid: u32,
}

pub fn op_webgpu_create_surface(
  state: &mut OpState,
  args: CreateSurfaceArgs,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<ResourceId, AnyError> {
  let instance = state.borrow::<super::Instance>();
  let device_resource = state
    .resource_table
    .get::<super::WebGpuDevice>(args.device_rid)
    .ok_or_else(bad_resource_id)?;
  let device = device_resource.0;
  let raw_window_handle_resource = state
    .resource_table
    .get::<WebGpuRawWindowHandle>(args.raw_window_handle_rid)
    .ok_or_else(bad_resource_id)?;
  let raw_window_handle = &*raw_window_handle_resource.0;

  let surface_id = gfx_select!(device => instance.instance_create_surface(
    raw_window_handle,
    std::marker::PhantomData
  ));

  let rid = state.resource_table.add(WebGpuSurface(surface_id));

  Ok(rid)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigureSwapchainArgs {
  device_rid: u32,
  surface_rid: u32,
  format: String,
  usage: u32,
  width: u32,
  height: u32,
}

pub fn op_webgpu_configure_swapchain(
  state: &mut OpState,
  args: ConfigureSwapchainArgs,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<ResourceId, AnyError> {
  let instance = state.borrow::<super::Instance>();
  let device_resource = state
    .resource_table
    .get::<super::WebGpuDevice>(args.device_rid)
    .ok_or_else(bad_resource_id)?;
  let device = device_resource.0;
  let swapchain_resource = state
    .resource_table
    .get::<WebGpuSurface>(args.surface_rid)
    .ok_or_else(bad_resource_id)?;
  let swapchain = swapchain_resource.0;

  let descriptor = wgpu_types::SwapChainDescriptor {
    usage: wgpu_types::TextureUsage::from_bits(args.usage).unwrap(),
    format: super::texture::serialize_texture_format(&args.format)?,
    width: args.width,
    height: args.height,
    present_mode: wgpu_types::PresentMode::Fifo,
  };

  gfx_put!(device => instance.device_create_swap_chain(
    device,
    swapchain,
    &descriptor
  ) => state, WebGpuSwapChain)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetSwapchainPreferredFormat {
  adapter_rid: u32,
  swapchain_rid: u32,
}

pub fn op_webgpu_get_swapchain_preferred_format(
  state: &mut OpState,
  args: GetSwapchainPreferredFormat,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<String, AnyError> {
  let instance = state.borrow::<super::Instance>();
  let adapter_resource = state
    .resource_table
    .get::<super::WebGPUAdapter>(args.adapter_rid)
    .ok_or_else(bad_resource_id)?;
  let adapter = adapter_resource.0;
  let swapchain_resource = state
    .resource_table
    .get::<WebGpuSwapChain>(args.swapchain_rid)
    .ok_or_else(bad_resource_id)?;
  let swapchain = swapchain_resource.0;


  let texture_format = gfx_select!(adapter => instance.adapter_get_swap_chain_preferred_format(
    adapter,
    swapchain.to_surface_id()
  ))?;

  super::texture::deserialize_texture_format(&texture_format).into()
}

