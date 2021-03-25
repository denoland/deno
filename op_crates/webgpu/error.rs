use deno_core::error::AnyError;
use serde::Serialize;
use std::fmt;
use wgpu_core::binding_model::CreateBindGroupError;
use wgpu_core::binding_model::CreateBindGroupLayoutError;
use wgpu_core::binding_model::CreatePipelineLayoutError;
use wgpu_core::binding_model::GetBindGroupLayoutError;
use wgpu_core::command::CommandAllocatorError;
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
use wgpu_core::resource::BufferAccessError;
use wgpu_core::resource::CreateBufferError;
use wgpu_core::resource::CreateQuerySetError;
use wgpu_core::resource::CreateSamplerError;
use wgpu_core::resource::CreateTextureError;
use wgpu_core::resource::CreateTextureViewError;

#[derive(Serialize)]
#[serde(tag = "type", content = "value")]
#[serde(rename_all = "kebab-case")]
pub enum WebGpuError {
  Lost,
  OutOfMemory,
  Validation(String),
}

impl From<CreateBufferError> for WebGpuError {
  fn from(err: CreateBufferError) -> Self {
    match err {
      CreateBufferError::Device(err) => err.into(),
      CreateBufferError::AccessError(err) => err.into(),
      err => WebGpuError::Validation(err.to_string()),
    }
  }
}

impl From<DeviceError> for WebGpuError {
  fn from(err: DeviceError) -> Self {
    match err {
      DeviceError::Lost => WebGpuError::Lost,
      DeviceError::OutOfMemory => WebGpuError::OutOfMemory,
      DeviceError::Invalid => WebGpuError::Validation(err.to_string()),
    }
  }
}

impl From<BufferAccessError> for WebGpuError {
  fn from(err: BufferAccessError) -> Self {
    match err {
      BufferAccessError::Device(err) => err.into(),
      err => WebGpuError::Validation(err.to_string()),
    }
  }
}

impl From<CreateBindGroupLayoutError> for WebGpuError {
  fn from(err: CreateBindGroupLayoutError) -> Self {
    match err {
      CreateBindGroupLayoutError::Device(err) => err.into(),
      err => WebGpuError::Validation(err.to_string()),
    }
  }
}

impl From<CreatePipelineLayoutError> for WebGpuError {
  fn from(err: CreatePipelineLayoutError) -> Self {
    match err {
      CreatePipelineLayoutError::Device(err) => err.into(),
      err => WebGpuError::Validation(err.to_string()),
    }
  }
}

impl From<CreateBindGroupError> for WebGpuError {
  fn from(err: CreateBindGroupError) -> Self {
    match err {
      CreateBindGroupError::Device(err) => err.into(),
      err => WebGpuError::Validation(err.to_string()),
    }
  }
}

impl From<RenderBundleError> for WebGpuError {
  fn from(err: RenderBundleError) -> Self {
    WebGpuError::Validation(err.to_string())
  }
}

impl From<CreateRenderBundleError> for WebGpuError {
  fn from(err: CreateRenderBundleError) -> Self {
    WebGpuError::Validation(err.to_string())
  }
}

impl From<CommandAllocatorError> for WebGpuError {
  fn from(err: CommandAllocatorError) -> Self {
    match err {
      CommandAllocatorError::Device(err) => err.into(),
    }
  }
}

impl From<CopyError> for WebGpuError {
  fn from(err: CopyError) -> Self {
    WebGpuError::Validation(err.to_string())
  }
}

impl From<CommandEncoderError> for WebGpuError {
  fn from(err: CommandEncoderError) -> Self {
    WebGpuError::Validation(err.to_string())
  }
}

impl From<QueryError> for WebGpuError {
  fn from(err: QueryError) -> Self {
    WebGpuError::Validation(err.to_string())
  }
}

impl From<ComputePassError> for WebGpuError {
  fn from(err: ComputePassError) -> Self {
    WebGpuError::Validation(err.to_string())
  }
}

impl From<CreateComputePipelineError> for WebGpuError {
  fn from(err: CreateComputePipelineError) -> Self {
    match err {
      CreateComputePipelineError::Device(err) => err.into(),
      err => WebGpuError::Validation(err.to_string()),
    }
  }
}

impl From<GetBindGroupLayoutError> for WebGpuError {
  fn from(err: GetBindGroupLayoutError) -> Self {
    WebGpuError::Validation(err.to_string())
  }
}

impl From<CreateRenderPipelineError> for WebGpuError {
  fn from(err: CreateRenderPipelineError) -> Self {
    match err {
      CreateRenderPipelineError::Device(err) => err.into(),
      err => WebGpuError::Validation(err.to_string()),
    }
  }
}

impl From<RenderPassError> for WebGpuError {
  fn from(err: RenderPassError) -> Self {
    WebGpuError::Validation(err.to_string())
  }
}

impl From<CreateSamplerError> for WebGpuError {
  fn from(err: CreateSamplerError) -> Self {
    match err {
      CreateSamplerError::Device(err) => err.into(),
      err => WebGpuError::Validation(err.to_string()),
    }
  }
}

impl From<CreateShaderModuleError> for WebGpuError {
  fn from(err: CreateShaderModuleError) -> Self {
    match err {
      CreateShaderModuleError::Device(err) => err.into(),
      err => WebGpuError::Validation(err.to_string()),
    }
  }
}

impl From<CreateTextureError> for WebGpuError {
  fn from(err: CreateTextureError) -> Self {
    match err {
      CreateTextureError::Device(err) => err.into(),
      err => WebGpuError::Validation(err.to_string()),
    }
  }
}

impl From<CreateTextureViewError> for WebGpuError {
  fn from(err: CreateTextureViewError) -> Self {
    WebGpuError::Validation(err.to_string())
  }
}

impl From<CreateQuerySetError> for WebGpuError {
  fn from(err: CreateQuerySetError) -> Self {
    match err {
      CreateQuerySetError::Device(err) => err.into(),
      err => WebGpuError::Validation(err.to_string()),
    }
  }
}

impl From<QueueSubmitError> for WebGpuError {
  fn from(err: QueueSubmitError) -> Self {
    match err {
      QueueSubmitError::Queue(err) => err.into(),
      err => WebGpuError::Validation(err.to_string()),
    }
  }
}

impl From<QueueWriteError> for WebGpuError {
  fn from(err: QueueWriteError) -> Self {
    match err {
      QueueWriteError::Queue(err) => err.into(),
      err => WebGpuError::Validation(err.to_string()),
    }
  }
}

#[derive(Debug)]
pub struct DomExceptionOperationError {
  pub msg: String,
}

impl DomExceptionOperationError {
  pub fn new(msg: &str) -> Self {
    DomExceptionOperationError {
      msg: msg.to_string(),
    }
  }
}

impl fmt::Display for DomExceptionOperationError {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    f.pad(&self.msg)
  }
}

impl std::error::Error for DomExceptionOperationError {}

pub fn get_error_class_name(e: &AnyError) -> Option<&'static str> {
  e.downcast_ref::<DomExceptionOperationError>()
    .map(|_| "DOMExceptionOperationError")
}
