// Copyright 2018-2025 the Deno authors. MIT license.

use deno_core::_ops::make_cppgc_object;
use deno_core::GarbageCollected;
use deno_core::WebIDL;
use deno_core::cppgc::Member;
use deno_core::cppgc::Ref;
use deno_core::op2;
use deno_core::v8;
use deno_core::v8::cppgc::GcCell;
use deno_error::JsErrorBox;
use wgpu_types::SurfaceStatus;

use crate::device::GPUDevice;
use crate::error::GPUGenericError;
use crate::texture::GPUTexture;
use crate::texture::GPUTextureFormat;

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum SurfaceError {
  #[class("DOMExceptionInvalidStateError")]
  #[error("Context is not configured")]
  UnconfiguredContext,
  #[class(generic)]
  #[error("Invalid Surface Status")]
  InvalidStatus,
  #[class(generic)]
  #[error(transparent)]
  Surface(#[from] wgpu_core::present::SurfaceError),
}

pub struct Configuration {
  pub device: Member<GPUDevice>,
  pub usage: u32,
  pub format: GPUTextureFormat,
  pub surface_config:
    wgpu_types::SurfaceConfiguration<Vec<wgpu_types::TextureFormat>>,
}

impl GarbageCollected for Configuration {
  fn trace(&self, visitor: &Visitor) {
    self.device.trace(visitor);
  }

  fn get_name(&self) -> &'static std::ffi::CStr {
    c"GPUCanvasContextConfiguration"
  }
}

pub struct GPUCanvasContext {
  pub surface_id: wgpu_core::id::SurfaceId,
  pub width: GcCell<u32>,
  pub height: GcCell<u32>,

  pub config: GcCell<Option<Configuration>>,
  pub texture: GcCell<Option<v8::Global<v8::Object>>>,

  pub canvas: GcCell<v8::Global<v8::Object>>,
}

unsafe impl GarbageCollected for GPUCanvasContext {
  fn trace(&self, _visitor: &mut deno_core::v8::cppgc::Visitor) {}

  fn get_name(&self) -> &'static std::ffi::CStr {
    c"GPUCanvasContext"
  }
}

#[op2]
impl GPUCanvasContext {
  #[constructor]
  #[cppgc]
  fn constructor(_: bool) -> Result<GPUCanvasContext, GPUGenericError> {
    Err(GPUGenericError::InvalidConstructor)
  }

  #[getter]
  #[global]
  fn canvas(&self, isolate: &v8::Isolate) -> v8::Global<v8::Object> {
    self.canvas.get(isolate).clone()
  }

  fn configure(
    &self,
    isolate: &mut v8::Isolate,
    #[webidl] configuration: GPUCanvasConfiguration,
  ) -> Result<(), JsErrorBox> {
    let usage = wgpu_types::TextureUsages::from_bits(configuration.usage)
      .ok_or_else(|| JsErrorBox::type_error("usage is not valid"))?;
    let format = configuration.format.clone().into();
    let conf = wgpu_types::SurfaceConfiguration {
      usage,
      format,
      width: *self.width.get(isolate),
      height: *self.height.get(isolate),
      present_mode: configuration
        .present_mode
        .map(Into::into)
        .unwrap_or_default(),
      alpha_mode: configuration.alpha_mode.into(),
      view_formats: configuration
        .view_formats
        .into_iter()
        .map(Into::into)
        .collect(),
      desired_maximum_frame_latency: 2,
    };

    let device = configuration.device;

    let err =
      device
        .instance
        .surface_configure(self.surface_id, device.id, &conf);

    device.error_handler.push_error(err);

    self.config.set(
      isolate,
      Some(Configuration {
        device,
        usage: configuration.usage,
        format: configuration.format,
        surface_config: conf,
      }),
    );

    Ok(())
  }

  #[fast]
  fn unconfigure(&self, isolate: &mut v8::Isolate) {
    self.config.set(isolate, None);
  }

  fn get_current_texture<'s>(
    &self,
    scope: &mut v8::HandleScope,
  ) -> Result<v8::Global<v8::Object>, SurfaceError> {
    let config = self.config.get(scope);
    let Some(config) = config.as_ref() else {
      return Err(SurfaceError::UnconfiguredContext);
    };

    {
      if let Some(obj) = self.texture.get(scope).as_ref() {
        return Ok(obj.clone());
      }
    }

    let output = config
      .device
      .instance
      .surface_get_current_texture(self.surface_id, None)?;

    match output.status {
      SurfaceStatus::Good | SurfaceStatus::Suboptimal => {
        let id = output.texture_id.unwrap();

        let texture = GPUTexture {
          instance: config.device.instance.clone(),
          error_handler: config.device.error_handler.clone(),
          id,
          device_id: config.device.id,
          queue_id: config.device.queue,
          label: "".to_string(),
          size: wgpu_types::Extent3d {
            width: *self.width.get(scope),
            height: *self.height.get(scope),
            depth_or_array_layers: 1,
          },
          mip_level_count: 0,
          sample_count: 0,
          dimension: crate::texture::GPUTextureDimension::D2,
          format: config.format.clone(),
          usage: config.usage,
        };
        let obj = make_cppgc_object(scope, texture);
        let obj = v8::Global::new(scope, obj);
        self.texture.set(scope, Some(obj.clone()));

        Ok(obj)
      }
      _ => Err(SurfaceError::InvalidStatus),
    }
  }
}

impl GPUCanvasContext {
  pub fn present(&self, isolate: &mut v8::Isolate) -> Result<(), SurfaceError> {
    let config = self.config.get(isolate);
    let Some(config) = config.as_ref() else {
      return Err(SurfaceError::UnconfiguredContext);
    };

    config.device.instance.surface_present(self.surface_id)?;

    // next `get_current_texture` call would get a new texture
    self.texture.set(isolate, None);

    Ok(())
  }

  pub fn resize_configure(
    &self,
    isolate: &mut v8::Isolate,
    width: u32,
    height: u32,
  ) {
    self.width.set(isolate, width);
    self.height.set(isolate, height);

    let config = self.config.get_mut(isolate);
    let Some(config) = config else {
      return;
    };

    config.surface_config.width = width;
    config.surface_config.height = height;

    let err = config.device.instance.surface_configure(
      self.surface_id,
      config.device.id,
      &config.surface_config,
    );

    config.device.error_handler.push_error(err);
  }
}

#[derive(WebIDL)]
#[webidl(dictionary)]
struct GPUCanvasConfiguration {
  device: Ref<GPUDevice>,
  format: GPUTextureFormat,
  #[webidl(default = wgpu_types::TextureUsages::RENDER_ATTACHMENT.bits())]
  #[options(enforce_range = true)]
  usage: u32,
  #[webidl(default = GPUCanvasAlphaMode::Opaque)]
  alpha_mode: GPUCanvasAlphaMode,

  // Extended from spec
  present_mode: Option<GPUPresentMode>,
  #[webidl(default = vec![])]
  view_formats: Vec<GPUTextureFormat>,
}

#[derive(WebIDL)]
#[webidl(enum)]
enum GPUCanvasAlphaMode {
  Opaque,
  Premultiplied,
}

impl From<GPUCanvasAlphaMode> for wgpu_types::CompositeAlphaMode {
  fn from(value: GPUCanvasAlphaMode) -> Self {
    match value {
      GPUCanvasAlphaMode::Opaque => Self::Opaque,
      GPUCanvasAlphaMode::Premultiplied => Self::PreMultiplied,
    }
  }
}

// Extended from spec
#[derive(WebIDL)]
#[webidl(enum)]
enum GPUPresentMode {
  #[webidl(rename = "autoVsync")]
  AutoVsync,
  #[webidl(rename = "autoNoVsync")]
  AutoNoVsync,
  #[webidl(rename = "fifo")]
  Fifo,
  #[webidl(rename = "fifoRelaxed")]
  FifoRelaxed,
  #[webidl(rename = "immediate")]
  Immediate,
  #[webidl(rename = "mailbox")]
  Mailbox,
}

impl From<GPUPresentMode> for wgpu_types::PresentMode {
  fn from(value: GPUPresentMode) -> Self {
    match value {
      GPUPresentMode::AutoVsync => Self::AutoVsync,
      GPUPresentMode::AutoNoVsync => Self::AutoNoVsync,
      GPUPresentMode::Fifo => Self::Fifo,
      GPUPresentMode::FifoRelaxed => Self::FifoRelaxed,
      GPUPresentMode::Immediate => Self::Immediate,
      GPUPresentMode::Mailbox => Self::Mailbox,
    }
  }
}
