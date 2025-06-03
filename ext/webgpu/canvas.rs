// Copyright 2018-2025 the Deno authors. MIT license.
use std::cell::RefCell;
use std::rc::Rc;

use deno_canvas::canvas::CanvasContextHooks;
use deno_canvas::image::DynamicImage;
use deno_canvas::image::GenericImageView;
use deno_core::cppgc::Ptr;
use deno_core::op2;
use deno_core::v8;
use deno_core::GarbageCollected;
use deno_core::WebIDL;
use deno_error::JsErrorBox;
use wgpu_core::resource::TextureDescriptor;
use wgpu_types::Extent3d;

use crate::device::GPUDevice;
use crate::error::GPUError;
use crate::texture::GPUTexture;
use crate::texture::GPUTextureFormat;
use crate::Instance;

pub struct GPUCanvasContext {
  canvas: v8::Global<v8::Object>,
  bitmap: Rc<RefCell<DynamicImage>>,

  texture_descriptor: RefCell<Option<TextureDescriptor<'static>>>,
  configuration: RefCell<Option<GPUCanvasConfiguration>>,

  current_texture: RefCell<Option<v8::Global<v8::Object>>>,
}

impl GarbageCollected for GPUCanvasContext {
  fn get_name(&self) -> &'static std::ffi::CStr {
    c"GPUCanvasContext"
  }
}

#[op2]
impl GPUCanvasContext {
  #[getter]
  #[global]
  fn canvas(&self) -> v8::Global<v8::Object> {
    self.canvas.clone()
  }

  fn configure(
    &self,
    #[webidl] configuration: GPUCanvasConfiguration,
  ) -> Result<(), JsErrorBox> {
    if !matches!(
      configuration.format,
      GPUTextureFormat::Bgra8unorm
        | GPUTextureFormat::Rgba8unorm
        | GPUTextureFormat::Rgba16float
    ) {
      return Err(JsErrorBox::type_error(format!(
        "The format '{}' is not supported",
        configuration.format.as_str()
      )));
    }

    let descriptor = self.get_descriptor_for_configuration(&configuration)?;
    self.configuration.replace(Some(configuration));
    self.texture_descriptor.replace(Some(descriptor));

    Ok(())
  }

  #[fast]
  fn unconfigure(&self) {
    self.configuration.take();
    self.texture_descriptor.take();
  }

  #[fast]
  fn get_configuration(&self) {
    let configuration = self.configuration.borrow();
    todo!()
  }

  #[global]
  fn get_current_texture(
    &self,
    scope: &mut v8::HandleScope,
  ) -> Result<v8::Global<v8::Object>, JsErrorBox> {
    let configuration = self.configuration.borrow();
    let configuration = configuration.as_ref().ok_or_else(|| {
      JsErrorBox::type_error("GPUCanvasContext has not been configured")
    })?;
    let texture_descriptor = self.texture_descriptor.borrow();
    let texture_descriptor = texture_descriptor.as_ref().unwrap();
    let device = &configuration.device;

    let mut current_texture = self.current_texture.borrow_mut();

    if let Some(texture) = current_texture.as_ref() {
      Ok(texture.clone())
    } else {
      let (id, err) = device.instance.device_create_texture(
        device.id,
        texture_descriptor,
        None,
      );
      device.error_handler.push_error(err);

      let texture = GPUTexture {
        instance: device.instance.clone(),
        error_handler: device.error_handler.clone(),
        id,
        device_id: device.id,
        queue_id: device.queue,
        label: texture_descriptor.label.as_ref().unwrap().to_string(),
        size: texture_descriptor.size,
        mip_level_count: texture_descriptor.mip_level_count,
        sample_count: texture_descriptor.sample_count,
        dimension: crate::texture::GPUTextureDimension::D2,
        format: configuration.format.clone(),
        usage: configuration.usage,
      };

      let texture_obj = deno_core::cppgc::make_cppgc_object(scope, texture);
      let texture_obj = v8::Global::new(scope, texture_obj);

      *current_texture = Some(texture_obj.clone());

      Ok(texture_obj)
    }
  }
}

impl GPUCanvasContext {
  fn get_descriptor_for_configuration(
    &self,
    configuration: &GPUCanvasConfiguration,
  ) -> Result<TextureDescriptor<'static>, JsErrorBox> {
    let (width, height) = {
      let data = self.bitmap.borrow();
      data.dimensions()
    };

    Ok(TextureDescriptor {
      label: Some("GPUCanvasContext".into()),
      size: Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
      },
      mip_level_count: 1,
      sample_count: 1,
      dimension: wgpu_types::TextureDimension::D2,
      format: configuration.format.clone().into(),
      usage: (wgpu_types::TextureUsages::from_bits(configuration.usage)
        .ok_or_else(|| JsErrorBox::type_error("usage is not valid"))?)
        | wgpu_types::TextureUsages::COPY_SRC,
      view_formats: configuration
        .view_formats
        .clone()
        .into_iter()
        .map(Into::into)
        .collect(),
    })
  }

  pub fn copy_image_contents_to_canvas_data(
    &self,
    scope: &mut v8::HandleScope,
  ) -> Result<(), JsErrorBox> {
    let configuration = self.configuration.borrow();
    let Some(GPUCanvasConfiguration { device, .. }) = configuration.as_ref()
    else {
      self.bitmap.replace_with(|image| {
        let (width, height) = image.dimensions();
        let image = deno_canvas::image::RgbaImage::new(width, height);
        DynamicImage::from(image)
      });

      return Ok(());
    };

    let texture_descriptor = self.texture_descriptor.borrow();

    if let Some(texture) = self.current_texture.borrow().as_ref() {
      let TextureDescriptor { size, .. } = texture_descriptor.as_ref().unwrap();

      let local = v8::Local::new(scope, texture).cast::<v8::Value>();
      let underlying_texture =
        deno_core::cppgc::try_unwrap_cppgc_object::<GPUTexture>(scope, local)
          .unwrap();

      let (command_encoder, err) =
        device.instance.device_create_command_encoder(
          device.id,
          &wgpu_types::CommandEncoderDescriptor {
            label: Some("GPUCanvasContext".into()),
          },
          None,
        );

      let data = copy_texture_to_vec(
        &device.instance,
        device.id,
        device.queue,
        command_encoder,
        underlying_texture.id,
        size,
      )?;

      self.bitmap.replace_with(|image| {
        let (width, height) = image.dimensions();
        let image =
          deno_canvas::image::RgbaImage::from_raw(width, height, data).unwrap();
        DynamicImage::from(image)
      });
    }

    Ok(())
  }

  fn expire_current_texture(&self, scope: &mut v8::HandleScope) {
    if let Some(texture) = self.current_texture.borrow().as_ref() {
      let local = v8::Local::new(scope, texture).cast::<v8::Value>();
      let underlying_texture =
        deno_core::cppgc::try_unwrap_cppgc_object::<GPUTexture>(scope, local)
          .unwrap();

      let _ = underlying_texture
        .instance
        .texture_destroy(underlying_texture.id);
    }
  }

  fn replace_drawing_buffer(&self, scope: &mut v8::HandleScope) {
    self.expire_current_texture(scope);
  }
}

impl CanvasContextHooks for GPUCanvasContext {
  fn resize(&self, scope: &mut v8::HandleScope) {
    self.replace_drawing_buffer(scope);
    if let Some(configuration) = self.configuration.borrow().as_ref() {
      self.texture_descriptor.replace(Some(
        self
          .get_descriptor_for_configuration(configuration)
          .unwrap(),
      ));
    }
  }

  fn bitmap_read_hook(
    &self,
    scope: &mut v8::HandleScope,
  ) -> Result<(), JsErrorBox> {
    self.copy_image_contents_to_canvas_data(scope)
  }

  fn post_transfer_to_image_bitmap_hook(&self, scope: &mut v8::HandleScope) {
    self.replace_drawing_buffer(scope);
  }
}

#[derive(WebIDL)]
#[webidl(dictionary)]
struct GPUCanvasConfiguration {
  device: Ptr<GPUDevice>,
  format: GPUTextureFormat,
  #[webidl(default = wgpu_types::TextureUsages::RENDER_ATTACHMENT.bits())]
  #[options(enforce_range = true)]
  usage: u32,
  #[webidl(default = vec![])]
  view_formats: Vec<GPUTextureFormat>,
  // TODO: PredefinedColorSpace colorSpace = "srgb";
  // TODO: GPUCanvasToneMapping toneMapping = {};
  #[webidl(default = GPUCanvasAlphaMode::Opaque)]
  alpha_mode: GPUCanvasAlphaMode,
}

#[derive(WebIDL)]
#[webidl(enum)]
enum GPUCanvasAlphaMode {
  Opaque,
  Premultiplied,
}

pub struct PaddedSize {
  pub padded_bytes_per_row: u32,
  pub unpadded_bytes_per_row: u32,
}

pub fn copy_texture_to_vec(
  instance: &Instance,
  device: wgpu_core::id::DeviceId,
  queue: wgpu_core::id::QueueId,
  command_encoder: wgpu_core::id::CommandEncoderId,
  texture: wgpu_core::id::TextureId,
  size: &Extent3d,
) -> Result<Vec<u8>, JsErrorBox> {
  // We only support the 8 bit per pixel formats with 4 channels
  // as such a pixel has 4 bytes
  const BYTES_PER_PIXEL: u32 = 4;

  let unpadded_bytes_per_row = size.width * BYTES_PER_PIXEL;
  let padded_bytes_per_row_padding = (wgpu_types::COPY_BYTES_PER_ROW_ALIGNMENT
    - (unpadded_bytes_per_row % wgpu_types::COPY_BYTES_PER_ROW_ALIGNMENT))
    % wgpu_types::COPY_BYTES_PER_ROW_ALIGNMENT;
  let padded_bytes_per_row =
    unpadded_bytes_per_row + padded_bytes_per_row_padding;

  let (buffer, maybe_err) = instance.device_create_buffer(
    device,
    &wgpu_types::BufferDescriptor {
      label: None,
      size: (padded_bytes_per_row * size.height) as _,
      usage: wgpu_types::BufferUsages::MAP_READ
        | wgpu_types::BufferUsages::COPY_DST,
      mapped_at_creation: false,
    },
    None,
  );

  if let Some(maybe_err) = maybe_err {
    return Err(JsErrorBox::from_err::<GPUError>(maybe_err.into()));
  }

  instance
    .command_encoder_copy_texture_to_buffer(
      command_encoder,
      &wgpu_types::TexelCopyTextureInfo {
        texture,
        mip_level: 0,
        origin: Default::default(),
        aspect: Default::default(),
      },
      &wgpu_types::TexelCopyBufferInfo {
        buffer,
        layout: wgpu_types::TexelCopyBufferLayout {
          offset: 0,
          bytes_per_row: Some(padded_bytes_per_row),
          rows_per_image: None,
        },
      },
      size,
    )
    .map_err(|e| JsErrorBox::from_err::<GPUError>(e.into()))?;

  let (command_buffer, maybe_err) = instance.command_encoder_finish(
    command_encoder,
    &wgpu_types::CommandBufferDescriptor { label: None },
  );
  if let Some(maybe_err) = maybe_err {
    return Err(JsErrorBox::from_err::<GPUError>(maybe_err.into()));
  }

  let maybe_err = instance.queue_submit(queue, &[command_buffer]).err();
  if let Some((_, maybe_err)) = maybe_err {
    return Err(JsErrorBox::from_err::<GPUError>(maybe_err.into()));
  }

  let index = instance
    .buffer_map_async(
      buffer,
      0,
      None,
      wgpu_core::resource::BufferMapOperation {
        host: wgpu_core::device::HostMap::Read,
        callback: None,
      },
    )
    .map_err(|e| JsErrorBox::from_err::<GPUError>(e.into()))?;

  instance
    .device_poll(device, wgpu_types::Maintain::WaitForSubmissionIndex(index))
    .map_err(|e| JsErrorBox::from_err::<GPUError>(e.into()))?;

  let (slice_pointer, range_size) = instance
    .buffer_get_mapped_range(buffer, 0, None)
    .map_err(|e| JsErrorBox::from_err::<GPUError>(e.into()))?;

  let data = {
    // SAFETY: creating a slice from pointer and length provided by wgpu and
    // then dropping it before unmapping
    let slice = unsafe {
      std::slice::from_raw_parts(slice_pointer.as_ptr(), range_size as usize)
    };

    let mut unpadded =
      Vec::with_capacity((unpadded_bytes_per_row * size.height) as _);

    for i in 0..size.height {
      unpadded.extend_from_slice(
        &slice[((i * padded_bytes_per_row) as usize)
          ..(((i + 1) * padded_bytes_per_row) as usize)]
          [..(unpadded_bytes_per_row as usize)],
      );
    }

    unpadded
  };

  instance
    .buffer_unmap(buffer)
    .map_err(|e| JsErrorBox::from_err::<GPUError>(e.into()))?;
  instance.buffer_drop(buffer);

  Ok(data)
}

pub const CONTEXT_ID: &str = "webgpu";

pub fn create<'s>(
  canvas: v8::Global<v8::Object>,
  data: Rc<RefCell<DynamicImage>>,
  scope: &mut v8::HandleScope<'s>,
  _options: v8::Local<'s, v8::Value>,
  _prefix: &'static str,
  _context: &'static str,
) -> v8::Global<v8::Value> {
  let obj = deno_core::cppgc::make_cppgc_object(
    scope,
    GPUCanvasContext {
      canvas,
      bitmap: data,
      texture_descriptor: RefCell::new(None),
      configuration: RefCell::new(None),
      current_texture: RefCell::new(None),
    },
  );

  v8::Global::new(scope, obj.cast())
}
