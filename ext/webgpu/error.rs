// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use deno_core::ResourceId;
use serde::Serialize;
use std::convert::From;
use std::error::Error;
use wgpu_core::binding_model::CreateBindGroupError;
use wgpu_core::binding_model::CreateBindGroupLayoutError;
use wgpu_core::binding_model::CreatePipelineLayoutError;
use wgpu_core::binding_model::GetBindGroupLayoutError;
use wgpu_core::command::ClearError;
use wgpu_core::command::CommandEncoderError;
use wgpu_core::command::ComputePassError;
use wgpu_core::command::CopyError;
use wgpu_core::command::CreateRenderBundleError;
use wgpu_core::command::QueryError;
use wgpu_core::command::RenderBundleError;
use wgpu_core::command::RenderPassError;
use wgpu_core::device::queue::QueueSubmitError;
use wgpu_core::device::queue::QueueWriteError;
use wgpu_core::device::DeviceError;
use wgpu_core::pipeline::CreateComputePipelineError;
use wgpu_core::pipeline::CreateRenderPipelineError;
use wgpu_core::pipeline::CreateShaderModuleError;
use wgpu_core::present::ConfigureSurfaceError;
use wgpu_core::resource::BufferAccessError;
use wgpu_core::resource::CreateBufferError;
use wgpu_core::resource::CreateQuerySetError;
use wgpu_core::resource::CreateSamplerError;
use wgpu_core::resource::CreateTextureError;
use wgpu_core::resource::CreateTextureViewError;

fn fmt_err(err: &(dyn Error + 'static)) -> String {
  let mut output = err.to_string();

  let mut e = err.source();
  while let Some(source) = e {
    output.push_str(&format!(": {source}"));
    e = source.source();
  }

  output
}

#[derive(Serialize)]
pub struct WebGpuResult {
  pub rid: Option<ResourceId>,
  pub err: Option<WebGpuError>,
}

impl WebGpuResult {
  pub fn rid(rid: ResourceId) -> Self {
    Self {
      rid: Some(rid),
      err: None,
    }
  }

  pub fn rid_err<T: Into<WebGpuError>>(
    rid: ResourceId,
    err: Option<T>,
  ) -> Self {
    Self {
      rid: Some(rid),
      err: err.map(Into::into),
    }
  }

  pub fn maybe_err<T: Into<WebGpuError>>(err: Option<T>) -> Self {
    Self {
      rid: None,
      err: err.map(Into::into),
    }
  }

  pub fn empty() -> Self {
    Self {
      rid: None,
      err: None,
    }
  }
}

#[derive(Serialize)]
#[serde(tag = "type", content = "value")]
#[serde(rename_all = "kebab-case")]
pub enum WebGpuError {
  Lost,
  OutOfMemory,
  Validation(String),
  Internal,
}

impl From<CreateBufferError> for WebGpuError {
  fn from(err: CreateBufferError) -> Self {
    match err {
      CreateBufferError::Device(err) => err.into(),
      CreateBufferError::AccessError(err) => err.into(),
      err => WebGpuError::Validation(fmt_err(&err)),
    }
  }
}

impl From<DeviceError> for WebGpuError {
  fn from(err: DeviceError) -> Self {
    match err {
      DeviceError::Lost => WebGpuError::Lost,
      DeviceError::OutOfMemory => WebGpuError::OutOfMemory,
      _ => WebGpuError::Validation(fmt_err(&err)),
    }
  }
}

impl From<BufferAccessError> for WebGpuError {
  fn from(err: BufferAccessError) -> Self {
    match err {
      BufferAccessError::Device(err) => err.into(),
      err => WebGpuError::Validation(fmt_err(&err)),
    }
  }
}

impl From<CreateBindGroupLayoutError> for WebGpuError {
  fn from(err: CreateBindGroupLayoutError) -> Self {
    match err {
      CreateBindGroupLayoutError::Device(err) => err.into(),
      err => WebGpuError::Validation(fmt_err(&err)),
    }
  }
}

impl From<CreatePipelineLayoutError> for WebGpuError {
  fn from(err: CreatePipelineLayoutError) -> Self {
    match err {
      CreatePipelineLayoutError::Device(err) => err.into(),
      err => WebGpuError::Validation(fmt_err(&err)),
    }
  }
}

impl From<CreateBindGroupError> for WebGpuError {
  fn from(err: CreateBindGroupError) -> Self {
    match err {
      CreateBindGroupError::Device(err) => err.into(),
      err => WebGpuError::Validation(fmt_err(&err)),
    }
  }
}

impl From<RenderBundleError> for WebGpuError {
  fn from(err: RenderBundleError) -> Self {
    WebGpuError::Validation(fmt_err(&err))
  }
}

impl From<CreateRenderBundleError> for WebGpuError {
  fn from(err: CreateRenderBundleError) -> Self {
    WebGpuError::Validation(fmt_err(&err))
  }
}

impl From<CopyError> for WebGpuError {
  fn from(err: CopyError) -> Self {
    WebGpuError::Validation(fmt_err(&err))
  }
}

impl From<CommandEncoderError> for WebGpuError {
  fn from(err: CommandEncoderError) -> Self {
    WebGpuError::Validation(fmt_err(&err))
  }
}

impl From<QueryError> for WebGpuError {
  fn from(err: QueryError) -> Self {
    WebGpuError::Validation(fmt_err(&err))
  }
}

impl From<ComputePassError> for WebGpuError {
  fn from(err: ComputePassError) -> Self {
    WebGpuError::Validation(fmt_err(&err))
  }
}

impl From<CreateComputePipelineError> for WebGpuError {
  fn from(err: CreateComputePipelineError) -> Self {
    match err {
      CreateComputePipelineError::Device(err) => err.into(),
      err => WebGpuError::Validation(fmt_err(&err)),
    }
  }
}

impl From<GetBindGroupLayoutError> for WebGpuError {
  fn from(err: GetBindGroupLayoutError) -> Self {
    WebGpuError::Validation(fmt_err(&err))
  }
}

impl From<CreateRenderPipelineError> for WebGpuError {
  fn from(err: CreateRenderPipelineError) -> Self {
    match err {
      CreateRenderPipelineError::Device(err) => err.into(),
      err => WebGpuError::Validation(fmt_err(&err)),
    }
  }
}

impl From<RenderPassError> for WebGpuError {
  fn from(err: RenderPassError) -> Self {
    WebGpuError::Validation(fmt_err(&err))
  }
}

impl From<CreateSamplerError> for WebGpuError {
  fn from(err: CreateSamplerError) -> Self {
    match err {
      CreateSamplerError::Device(err) => err.into(),
      err => WebGpuError::Validation(fmt_err(&err)),
    }
  }
}

impl From<CreateShaderModuleError> for WebGpuError {
  fn from(err: CreateShaderModuleError) -> Self {
    match err {
      CreateShaderModuleError::Device(err) => err.into(),
      err => WebGpuError::Validation(fmt_err(&err)),
    }
  }
}

impl From<CreateTextureError> for WebGpuError {
  fn from(err: CreateTextureError) -> Self {
    match err {
      CreateTextureError::Device(err) => err.into(),
      err => WebGpuError::Validation(fmt_err(&err)),
    }
  }
}

impl From<CreateTextureViewError> for WebGpuError {
  fn from(err: CreateTextureViewError) -> Self {
    WebGpuError::Validation(fmt_err(&err))
  }
}

impl From<CreateQuerySetError> for WebGpuError {
  fn from(err: CreateQuerySetError) -> Self {
    match err {
      CreateQuerySetError::Device(err) => err.into(),
      err => WebGpuError::Validation(fmt_err(&err)),
    }
  }
}

impl From<QueueSubmitError> for WebGpuError {
  fn from(err: QueueSubmitError) -> Self {
    match err {
      QueueSubmitError::Queue(err) => err.into(),
      err => WebGpuError::Validation(fmt_err(&err)),
    }
  }
}

impl From<QueueWriteError> for WebGpuError {
  fn from(err: QueueWriteError) -> Self {
    match err {
      QueueWriteError::Queue(err) => err.into(),
      err => WebGpuError::Validation(fmt_err(&err)),
    }
  }
}

impl From<ClearError> for WebGpuError {
  fn from(err: ClearError) -> Self {
    WebGpuError::Validation(fmt_err(&err))
  }
}

impl From<ConfigureSurfaceError> for WebGpuError {
  fn from(err: ConfigureSurfaceError) -> Self {
    WebGpuError::Validation(fmt_err(&err))
  }
}
