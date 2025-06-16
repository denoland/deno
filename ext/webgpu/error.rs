// Copyright 2018-2025 the Deno authors. MIT license.

use std::fmt::Display;
use std::fmt::Formatter;
use std::sync::Mutex;
use std::sync::OnceLock;

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
use wgpu_core::device::WaitIdleError;
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

pub type ErrorHandler = std::sync::Arc<DeviceErrorHandler>;

pub struct DeviceErrorHandler {
  pub is_lost: OnceLock<()>,
  lost_sender: Mutex<Option<tokio::sync::oneshot::Sender<()>>>,
  uncaptured_sender_is_closed: Mutex<Option<tokio::sync::oneshot::Sender<()>>>,

  pub uncaptured_sender: tokio::sync::mpsc::UnboundedSender<GPUError>,

  pub scopes: Mutex<Vec<(GPUErrorFilter, Vec<GPUError>)>>,
}

impl Drop for DeviceErrorHandler {
  fn drop(&mut self) {
    if let Some(sender) =
      self.uncaptured_sender_is_closed.lock().unwrap().take()
    {
      let _ = sender.send(());
    }
  }
}

impl DeviceErrorHandler {
  pub fn new(
    lost_sender: tokio::sync::oneshot::Sender<()>,
    uncaptured_sender: tokio::sync::mpsc::UnboundedSender<GPUError>,
    uncaptured_sender_is_closed: tokio::sync::oneshot::Sender<()>,
  ) -> Self {
    Self {
      is_lost: Default::default(),
      lost_sender: Mutex::new(Some(lost_sender)),
      uncaptured_sender,
      uncaptured_sender_is_closed: Mutex::new(Some(
        uncaptured_sender_is_closed,
      )),
      scopes: Mutex::new(vec![]),
    }
  }

  pub fn push_error<E: Into<GPUError>>(&self, err: Option<E>) {
    let Some(err) = err else {
      return;
    };

    if self.is_lost.get().is_some() {
      return;
    }

    let err = err.into();

    if matches!(err, GPUError::Lost) {
      let _ = self.is_lost.set(());

      if let Some(sender) = self.lost_sender.lock().unwrap().take() {
        let _ = sender.send(());
      }
      return;
    }

    let error_filter = match err {
      GPUError::Lost => unreachable!(),
      GPUError::Validation(_) => GPUErrorFilter::Validation,
      GPUError::OutOfMemory => GPUErrorFilter::OutOfMemory,
      GPUError::Internal => GPUErrorFilter::Internal,
    };

    let mut scopes = self.scopes.lock().unwrap();
    let scope = scopes
      .iter_mut()
      .rfind(|(filter, _)| filter == &error_filter);

    if let Some(scope) = scope {
      scope.1.push(err);
    } else {
      self.uncaptured_sender.send(err).unwrap();
    }
  }
}

#[derive(deno_core::WebIDL, Eq, PartialEq)]
#[webidl(enum)]
pub enum GPUErrorFilter {
  Validation,
  OutOfMemory,
  Internal,
}

#[derive(Debug, deno_error::JsError)]
pub enum GPUError {
  // TODO(@crowlKats): consider adding an unreachable value that uses unreachable!()
  #[class("UNREACHABLE")]
  Lost,
  #[class("GPUValidationError")]
  Validation(String),
  #[class("GPUOutOfMemoryError")]
  OutOfMemory,
  #[allow(dead_code)]
  #[class("GPUInternalError")]
  Internal,
}

impl Display for GPUError {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    match self {
      GPUError::Lost => Ok(()),
      GPUError::Validation(s) => f.write_str(s),
      GPUError::OutOfMemory => f.write_str("not enough memory left"),
      GPUError::Internal => Ok(()),
    }
  }
}

impl std::error::Error for GPUError {}

fn fmt_err(err: &(dyn std::error::Error + 'static)) -> String {
  let mut output = err.to_string();

  let mut e = err.source();
  while let Some(source) = e {
    output.push_str(&format!(": {source}"));
    e = source.source();
  }

  if output.is_empty() {
    output.push_str("validation error");
  }

  output
}

impl From<CreateBufferError> for GPUError {
  fn from(err: CreateBufferError) -> Self {
    match err {
      CreateBufferError::Device(err) => err.into(),
      CreateBufferError::AccessError(err) => err.into(),
      err => GPUError::Validation(fmt_err(&err)),
    }
  }
}

impl From<DeviceError> for GPUError {
  fn from(err: DeviceError) -> Self {
    match err {
      DeviceError::Lost => GPUError::Lost,
      DeviceError::OutOfMemory => GPUError::OutOfMemory,
      _ => GPUError::Validation(fmt_err(&err)),
    }
  }
}

impl From<BufferAccessError> for GPUError {
  fn from(err: BufferAccessError) -> Self {
    match err {
      BufferAccessError::Device(err) => err.into(),
      err => GPUError::Validation(fmt_err(&err)),
    }
  }
}

impl From<CreateBindGroupLayoutError> for GPUError {
  fn from(err: CreateBindGroupLayoutError) -> Self {
    match err {
      CreateBindGroupLayoutError::Device(err) => err.into(),
      err => GPUError::Validation(fmt_err(&err)),
    }
  }
}

impl From<CreatePipelineLayoutError> for GPUError {
  fn from(err: CreatePipelineLayoutError) -> Self {
    match err {
      CreatePipelineLayoutError::Device(err) => err.into(),
      err => GPUError::Validation(fmt_err(&err)),
    }
  }
}

impl From<CreateBindGroupError> for GPUError {
  fn from(err: CreateBindGroupError) -> Self {
    match err {
      CreateBindGroupError::Device(err) => err.into(),
      err => GPUError::Validation(fmt_err(&err)),
    }
  }
}

impl From<RenderBundleError> for GPUError {
  fn from(err: RenderBundleError) -> Self {
    GPUError::Validation(fmt_err(&err))
  }
}

impl From<CreateRenderBundleError> for GPUError {
  fn from(err: CreateRenderBundleError) -> Self {
    GPUError::Validation(fmt_err(&err))
  }
}

impl From<CopyError> for GPUError {
  fn from(err: CopyError) -> Self {
    GPUError::Validation(fmt_err(&err))
  }
}

impl From<CommandEncoderError> for GPUError {
  fn from(err: CommandEncoderError) -> Self {
    GPUError::Validation(fmt_err(&err))
  }
}

impl From<QueryError> for GPUError {
  fn from(err: QueryError) -> Self {
    GPUError::Validation(fmt_err(&err))
  }
}

impl From<ComputePassError> for GPUError {
  fn from(err: ComputePassError) -> Self {
    GPUError::Validation(fmt_err(&err))
  }
}

impl From<CreateComputePipelineError> for GPUError {
  fn from(err: CreateComputePipelineError) -> Self {
    match err {
      CreateComputePipelineError::Device(err) => err.into(),
      err => GPUError::Validation(fmt_err(&err)),
    }
  }
}

impl From<GetBindGroupLayoutError> for GPUError {
  fn from(err: GetBindGroupLayoutError) -> Self {
    GPUError::Validation(fmt_err(&err))
  }
}

impl From<CreateRenderPipelineError> for GPUError {
  fn from(err: CreateRenderPipelineError) -> Self {
    match err {
      CreateRenderPipelineError::Device(err) => err.into(),
      err => GPUError::Validation(fmt_err(&err)),
    }
  }
}

impl From<RenderPassError> for GPUError {
  fn from(err: RenderPassError) -> Self {
    GPUError::Validation(fmt_err(&err))
  }
}

impl From<CreateSamplerError> for GPUError {
  fn from(err: CreateSamplerError) -> Self {
    match err {
      CreateSamplerError::Device(err) => err.into(),
      err => GPUError::Validation(fmt_err(&err)),
    }
  }
}

impl From<CreateShaderModuleError> for GPUError {
  fn from(err: CreateShaderModuleError) -> Self {
    match err {
      CreateShaderModuleError::Device(err) => err.into(),
      err => GPUError::Validation(fmt_err(&err)),
    }
  }
}

impl From<CreateTextureError> for GPUError {
  fn from(err: CreateTextureError) -> Self {
    match err {
      CreateTextureError::Device(err) => err.into(),
      err => GPUError::Validation(fmt_err(&err)),
    }
  }
}

impl From<CreateTextureViewError> for GPUError {
  fn from(err: CreateTextureViewError) -> Self {
    GPUError::Validation(fmt_err(&err))
  }
}

impl From<CreateQuerySetError> for GPUError {
  fn from(err: CreateQuerySetError) -> Self {
    match err {
      CreateQuerySetError::Device(err) => err.into(),
      err => GPUError::Validation(fmt_err(&err)),
    }
  }
}

impl From<QueueSubmitError> for GPUError {
  fn from(err: QueueSubmitError) -> Self {
    match err {
      QueueSubmitError::Queue(err) => err.into(),
      err => GPUError::Validation(fmt_err(&err)),
    }
  }
}

impl From<QueueWriteError> for GPUError {
  fn from(err: QueueWriteError) -> Self {
    match err {
      QueueWriteError::Queue(err) => err.into(),
      err => GPUError::Validation(fmt_err(&err)),
    }
  }
}

impl From<ClearError> for GPUError {
  fn from(err: ClearError) -> Self {
    GPUError::Validation(fmt_err(&err))
  }
}

impl From<ConfigureSurfaceError> for GPUError {
  fn from(err: ConfigureSurfaceError) -> Self {
    GPUError::Validation(fmt_err(&err))
  }
}

impl From<WaitIdleError> for GPUError {
  fn from(err: WaitIdleError) -> Self {
    GPUError::Validation(fmt_err(&err))
  }
}
