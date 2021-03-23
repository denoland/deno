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

struct WebGPUSwapChain(wgpu_core::id::SwapChainId);
impl Resource for WebGPUSwapChain {
  fn name(&self) -> Cow<str> {
    "webGPUSwapChain".into()
  }
}

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
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let instance = state.borrow::<super::Instance>();
  let device_resource = state
    .resource_table
    .get::<super::WebGPUDevice>(args.device_rid)
    .ok_or_else(bad_resource_id)?;
  let device = device_resource.0;
  let surface_resource = state
    .resource_table
    .get::<super::WebGPUSurface>(args.surface_rid)
    .ok_or_else(bad_resource_id)?;
  let surface = surface_resource.0;

  let descriptor = wgpu_types::SwapChainDescriptor {
    usage: wgpu_types::TextureUsage::from_bits(args.usage).unwrap(),
    format: super::texture::serialize_texture_format(&args.format)?,
    width: args.width,
    height: args.height,
    present_mode: wgpu_types::PresentMode::Fifo,
  };

  let swap_chain = gfx_select!(device => instance.device_create_swap_chain(
    device,
    surface,
    &descriptor
  ))?;

  let rid = state.resource_table.add(WebGPUSwapChain(swap_chain));

  Ok(json!({ "rid": rid }))
}

pub struct GetSwapchainPreferredFormat {
  adapter_rid: u32,
  swapchain_rid: u32,
}

pub fn op_webgpu_get_swapchain_preferred_format(
  state: &mut OpState,
  args: GetSwapchainPreferredFormat,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let instance = state.borrow::<super::Instance>();
  let adapter_resource = state
    .resource_table
    .get::<super::WebGPUAdapter>(args.adapter_rid)
    .ok_or_else(bad_resource_id)?;
  let adapter = adapter_resource.0;
  let swapchain_resource = state
    .resource_table
    .get::<WebGPUSwapChain>(args.swapchain_rid)
    .ok_or_else(bad_resource_id)?;
  let swapchain = swapchain_resource.0;

  let texture_format = gfx_select!(adapter => instance.adapter_get_swap_chain_preferred_format(
    adapter,
    surface
  ))?;

  let format = super::texture::deserialize_texture_format(&texture_format)?;

  Ok(json!({ "format": format }))
}

