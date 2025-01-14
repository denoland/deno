// Copyright 2018-2025 the Deno authors. MIT license.
#![cfg(not(target_arch = "wasm32"))]
#![warn(unsafe_op_in_unsafe_fn)]

pub use wgpu_core;
pub use wgpu_types;

pub const UNSTABLE_FEATURE_NAME: &str = "webgpu";

//pub mod byow;
//pub mod surface;
mod wrap;

pub type Instance = std::sync::Arc<wgpu_core::global::Global>;

deno_core::extension!(
  deno_webgpu,
  deps = [deno_webidl, deno_web],
  ops = [wrap::create_gpu],
  objects = [
    wrap::GPU,
    wrap::adapter::GPUAdapter,
    wrap::adapter::GPUAdapterInfo,
    wrap::bind_group::GPUBindGroup,
    wrap::bind_group_layout::GPUBindGroupLayout,
    wrap::buffer::GPUBuffer,
    wrap::command_buffer::GPUCommandBuffer,
    wrap::command_encoder::GPUCommandEncoder,
    wrap::compute_pass::GPUComputePassEncoder,
    wrap::compute_pipeline::GPUComputePipeline,
    wrap::device::GPUDevice,
    wrap::device::GPUDeviceLostInfo,
    wrap::pipeline_layout::GPUPipelineLayout,
    wrap::query_set::GPUQuerySet,
    wrap::queue::GPUQueue,
    wrap::render_bundle::GPURenderBundle,
    wrap::render_bundle::GPURenderBundleEncoder,
    wrap::render_pass::GPURenderPassEncoder,
    wrap::render_pipeline::GPURenderPipeline,
    wrap::sampler::GPUSampler,
    wrap::shader::GPUShaderModule,
    wrap::adapter::GPUSupportedFeatures,
    wrap::adapter::GPUSupportedLimits,
    wrap::texture::GPUTexture,
    wrap::texture::GPUTextureView,
  ],
  esm = ["00_init.js"],
  lazy_loaded_esm = ["01_webgpu.js"],
);
