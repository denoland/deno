use std::cell::RefCell;
use std::rc::Rc;

use deno_canvas::canvas::CanvasContext;
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
use crate::texture::GPUTexture;
use crate::texture::GPUTextureFormat;

struct GPUCanvasContext {
  canvas: v8::Global<v8::Object>,
  bitmap: Rc<RefCell<DynamicImage>>,

  texture_descriptor: RefCell<Option<TextureDescriptor<'static>>>,
  configuration: RefCell<Option<GPUCanvasConfiguration>>,

  current_texture: RefCell<Option<v8::Global<v8::Object>>>,
}

impl GarbageCollected for GPUCanvasContext {}

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

    if let Some(current_texture) = current_texture.as_ref() {
      Ok(current_texture.clone())
    } else {
      // TODO: except with the GPUTexture’s underlying storage pointing to this.[[drawingBuffer]].
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

      let texture = deno_core::cppgc::make_cppgc_object(scope, texture);
      let texture = v8::Global::new(scope, texture);

      *current_texture = Some(texture.clone());

      Ok(texture)
    }
  }
}

impl GPUCanvasContext {
  pub fn get_descriptor_for_configuration(
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
      usage: wgpu_types::TextureUsages::from_bits(configuration.usage)
        .ok_or_else(|| JsErrorBox::type_error("usage is not valid"))?,
      view_formats: configuration
        .view_formats
        .clone()
        .into_iter()
        .map(Into::into)
        .collect(),
    })
  }
}

impl CanvasContext for GPUCanvasContext {
  fn value(&self) -> v8::Global<v8::Value> {
    todo!()
  }

  fn resize(&self) {
    if let Some(configuration) = self.configuration.borrow().as_ref() {
      self.texture_descriptor.replace(Some(self.get_descriptor_for_configuration(configuration).unwrap()));
    }
  }

  fn bitmap_read_hook(&self) {
    todo!()
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
