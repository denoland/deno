// Copyright 2018-2026 the Deno authors. MIT license.

use std::fmt::Display;
use std::fmt::Formatter;
use std::sync::Mutex;
use std::sync::OnceLock;

use deno_core::JsRuntime;
use deno_core::V8TaskSpawner;
use deno_core::cppgc::make_cppgc_object;
use deno_core::v8;
use wgpu_core::binding_model::CreateBindGroupError;
use wgpu_core::binding_model::CreateBindGroupLayoutError;
use wgpu_core::binding_model::CreatePipelineLayoutError;
use wgpu_core::binding_model::GetBindGroupLayoutError;
use wgpu_core::command::ClearError;
use wgpu_core::command::CommandEncoderError;
use wgpu_core::command::ComputePassError;
use wgpu_core::command::CreateRenderBundleError;
use wgpu_core::command::EncoderStateError;
use wgpu_core::command::PassStateError;
use wgpu_core::command::QueryError;
use wgpu_core::command::RenderBundleError;
use wgpu_core::command::RenderPassError;
use wgpu_core::device::DeviceError;
use wgpu_core::device::queue::QueueSubmitError;
use wgpu_core::device::queue::QueueWriteError;
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
use wgpu_types::error::ErrorType;
use wgpu_types::error::WebGpuError;

use crate::device::GPUDeviceLostInfo;
use crate::device::GPUDeviceLostReason;

pub type ErrorHandler = std::rc::Rc<DeviceErrorHandler>;

pub struct DeviceErrorHandler {
  pub is_lost: OnceLock<()>,
  pub scopes: Mutex<Vec<(GPUErrorFilter, Vec<GPUError>)>>,
  lost_resolver: Mutex<Option<v8::Global<v8::PromiseResolver>>>,
  spawner: V8TaskSpawner,

  // The error handler is constructed before the device. A weak
  // reference to the device is placed here with `set_device`
  // after the device is constructed.
  device: OnceLock<v8::Weak<v8::Object>>,
}

impl DeviceErrorHandler {
  pub fn new(
    lost_resolver: v8::Global<v8::PromiseResolver>,
    spawner: V8TaskSpawner,
  ) -> Self {
    Self {
      is_lost: Default::default(),
      scopes: Mutex::new(vec![]),
      lost_resolver: Mutex::new(Some(lost_resolver)),
      device: OnceLock::new(),
      spawner,
    }
  }

  pub fn set_device(&self, device: v8::Weak<v8::Object>) {
    self.device.set(device).unwrap()
  }

  pub fn push_error<E: Into<GPUError>>(&self, err: Option<E>) {
    let Some(err) = err else {
      return;
    };

    if self.is_lost.get().is_some() {
      return;
    }

    let err = err.into();

    if let GPUError::Lost(reason) = err {
      let _ = self.is_lost.set(());
      if let Some(resolver) = self.lost_resolver.lock().unwrap().take() {
        self.spawner.spawn(move |scope| {
          let resolver = v8::Local::new(scope, resolver);
          let info = make_cppgc_object(scope, GPUDeviceLostInfo { reason });
          let info = v8::Local::new(scope, info);
          resolver.resolve(scope, info.into());
        });
      }

      return;
    }

    let error_filter = match err {
      GPUError::Lost(_) => unreachable!(),
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
      let device = self
        .device
        .get()
        .expect("set_device was not called")
        .clone();
      self.spawner.spawn(move |scope| {
        let state = JsRuntime::op_state_from(&*scope);
        let Some(device) = device.to_local(scope) else {
          // The device has already gone away, so we don't have
          // anywhere to report the error.
          return;
        };
        let key = v8::String::new(scope, "dispatchEvent").unwrap();
        let val = device.get(scope, key.into()).unwrap();
        let func =
          v8::Global::new(scope, val.try_cast::<v8::Function>().unwrap());
        let device = v8::Global::new(scope, device.cast::<v8::Value>());
        let error_event_class =
          state.borrow().borrow::<crate::ErrorEventClass>().0.clone();

        let error = deno_core::error::to_v8_error(scope, &err);

        let error_event_class =
          v8::Local::new(scope, error_event_class.clone());
        let constructor =
          v8::Local::<v8::Function>::try_from(error_event_class).unwrap();
        let kind = v8::String::new(scope, "uncapturederror").unwrap();

        let obj = v8::Object::new(scope);
        let key = v8::String::new(scope, "error").unwrap();
        obj.set(scope, key.into(), error);

        let event = constructor
          .new_instance(scope, &[kind.into(), obj.into()])
          .unwrap();

        let recv = v8::Local::new(scope, device);
        func.open(scope).call(scope, recv, &[event.into()]);
      });
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
  Lost(GPUDeviceLostReason),
  #[class("GPUValidationError")]
  Validation(String),
  #[class("GPUOutOfMemoryError")]
  OutOfMemory,
  #[class("GPUInternalError")]
  Internal,
}

impl Display for GPUError {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    match self {
      GPUError::Lost(_) => Ok(()),
      GPUError::Validation(s) => f.write_str(s),
      GPUError::OutOfMemory => f.write_str("not enough memory left"),
      GPUError::Internal => Ok(()),
    }
  }
}

impl std::error::Error for GPUError {}

impl GPUError {
  fn from_webgpu(e: impl WebGpuError) -> Self {
    match e.webgpu_error_type() {
      ErrorType::Internal => GPUError::Internal,
      ErrorType::DeviceLost => GPUError::Lost(GPUDeviceLostReason::Unknown), // TODO: this variant should be ignored, register the lost callback instead.
      ErrorType::OutOfMemory => GPUError::OutOfMemory,
      ErrorType::Validation => GPUError::Validation(fmt_err(&e)),
    }
  }
}

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

impl From<EncoderStateError> for GPUError {
  fn from(err: EncoderStateError) -> Self {
    GPUError::from_webgpu(err)
  }
}

impl From<PassStateError> for GPUError {
  fn from(err: PassStateError) -> Self {
    GPUError::Validation(fmt_err(&err))
  }
}

impl From<CreateBufferError> for GPUError {
  fn from(err: CreateBufferError) -> Self {
    GPUError::from_webgpu(err)
  }
}

impl From<DeviceError> for GPUError {
  fn from(err: DeviceError) -> Self {
    GPUError::from_webgpu(err)
  }
}

impl From<BufferAccessError> for GPUError {
  fn from(err: BufferAccessError) -> Self {
    GPUError::from_webgpu(err)
  }
}

impl From<CreateBindGroupLayoutError> for GPUError {
  fn from(err: CreateBindGroupLayoutError) -> Self {
    GPUError::from_webgpu(err)
  }
}

impl From<CreatePipelineLayoutError> for GPUError {
  fn from(err: CreatePipelineLayoutError) -> Self {
    GPUError::from_webgpu(err)
  }
}

impl From<CreateBindGroupError> for GPUError {
  fn from(err: CreateBindGroupError) -> Self {
    GPUError::from_webgpu(err)
  }
}

impl From<RenderBundleError> for GPUError {
  fn from(err: RenderBundleError) -> Self {
    GPUError::from_webgpu(err)
  }
}

impl From<CreateRenderBundleError> for GPUError {
  fn from(err: CreateRenderBundleError) -> Self {
    GPUError::from_webgpu(err)
  }
}

impl From<CommandEncoderError> for GPUError {
  fn from(err: CommandEncoderError) -> Self {
    GPUError::from_webgpu(err)
  }
}

impl From<QueryError> for GPUError {
  fn from(err: QueryError) -> Self {
    GPUError::from_webgpu(err)
  }
}

impl From<ComputePassError> for GPUError {
  fn from(err: ComputePassError) -> Self {
    GPUError::from_webgpu(err)
  }
}

impl From<CreateComputePipelineError> for GPUError {
  fn from(err: CreateComputePipelineError) -> Self {
    GPUError::from_webgpu(err)
  }
}

impl From<GetBindGroupLayoutError> for GPUError {
  fn from(err: GetBindGroupLayoutError) -> Self {
    GPUError::from_webgpu(err)
  }
}

impl From<CreateRenderPipelineError> for GPUError {
  fn from(err: CreateRenderPipelineError) -> Self {
    GPUError::from_webgpu(err)
  }
}

impl From<RenderPassError> for GPUError {
  fn from(err: RenderPassError) -> Self {
    GPUError::from_webgpu(err)
  }
}

impl From<CreateSamplerError> for GPUError {
  fn from(err: CreateSamplerError) -> Self {
    GPUError::from_webgpu(err)
  }
}

impl From<CreateShaderModuleError> for GPUError {
  fn from(err: CreateShaderModuleError) -> Self {
    GPUError::from_webgpu(err)
  }
}

impl From<CreateTextureError> for GPUError {
  fn from(err: CreateTextureError) -> Self {
    GPUError::from_webgpu(err)
  }
}

impl From<CreateTextureViewError> for GPUError {
  fn from(err: CreateTextureViewError) -> Self {
    GPUError::from_webgpu(err)
  }
}

impl From<CreateQuerySetError> for GPUError {
  fn from(err: CreateQuerySetError) -> Self {
    GPUError::from_webgpu(err)
  }
}

impl From<QueueSubmitError> for GPUError {
  fn from(err: QueueSubmitError) -> Self {
    GPUError::from_webgpu(err)
  }
}

impl From<QueueWriteError> for GPUError {
  fn from(err: QueueWriteError) -> Self {
    GPUError::from_webgpu(err)
  }
}

impl From<ClearError> for GPUError {
  fn from(err: ClearError) -> Self {
    GPUError::from_webgpu(err)
  }
}

impl From<ConfigureSurfaceError> for GPUError {
  fn from(err: ConfigureSurfaceError) -> Self {
    GPUError::from_webgpu(err)
  }
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum GPUGenericError {
  #[class(type)]
  #[error("Illegal constructor")]
  InvalidConstructor,
}
