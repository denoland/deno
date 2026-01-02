// Copyright 2018-2025 the Deno authors. MIT license.
use std::cell::OnceCell;
use std::cell::RefCell;
use std::rc::Rc;

use deno_core::GarbageCollected;
use deno_core::OpState;
use deno_core::WebIDL;
use deno_core::op2;
use deno_core::v8;
use deno_core::v8::cppgc::Visitor;
use deno_core::webidl::UnrestrictedDouble;
use deno_error::JsErrorBox;
use deno_image::image;
use deno_image::image::ColorType;
use deno_image::image::DynamicImage;
use deno_image::image::GenericImageView;
use deno_image::op_create_image_bitmap::ImageBitmap;
use deno_webgpu::canvas::Data;

pub struct BlobHandle(pub v8::Global<v8::Function>);

pub type CreateCanvasContext = for<'s> fn(
  canvas: v8::Global<v8::Object>,
  data: Rc<RefCell<Data>>,
  scope: &mut v8::PinScope<'s, '_>,
  options: v8::Local<'s, v8::Value>,
  prefix: &'static str,
  context: &'static str,
) -> v8::Global<v8::Value>;

pub struct OffscreenCanvas {
  data: Rc<RefCell<Data>>,

  active_context: OnceCell<(String, v8::Global<v8::Value>)>,
}

// SAFETY: we're sure this can be GCed
unsafe impl GarbageCollected for OffscreenCanvas {
  fn trace(&self, _visitor: &mut Visitor) {}

  fn get_name(&self) -> &'static std::ffi::CStr {
    c"OffscreenCanvas"
  }
}

#[op2]
impl OffscreenCanvas {
  #[getter]
  fn width(&self) -> u32 {
    let data = self.data.borrow();
    let Data::Image(data) = &*data else {
      unreachable!();
    };

    data.width()
  }
  #[setter]
  fn width(
    &self,
    scope: &mut v8::PinScope<'_, '_>,
    #[webidl(options(enforce_range = true))] value: u64,
  ) {
    {
      self.data.replace_with(|data| {
        let Data::Image(data) = &data else {
          unreachable!();
        };
        Data::Image(data.crop_imm(0, 0, value as _, data.height()))
      });
    }
    if let Some((id, active_context)) = self.active_context.get() {
      let active_context = v8::Local::new(scope, active_context);
      match get_context(id, scope, active_context) {
        Context::Bitmap(_) => {}
        Context::WebGPU(context) => context.resize(scope),
      }
    }
  }

  #[getter]
  fn height(&self) -> u32 {
    let data = self.data.borrow();
    let Data::Image(data) = &*data else {
      unreachable!();
    };

    data.height()
  }
  #[setter]
  fn height(
    &self,
    scope: &mut v8::PinScope<'_, '_>,
    #[webidl(options(enforce_range = true))] value: u64,
  ) {
    {
      self.data.replace_with(|data| {
        let Data::Image(data) = &data else {
          unreachable!();
        };

        Data::Image(data.crop_imm(0, 0, data.width(), value as _))
      });
    }
    if let Some((id, active_context)) = self.active_context.get() {
      let active_context = v8::Local::new(scope, active_context);
      match get_context(id, scope, active_context) {
        Context::Bitmap(_) => {}
        Context::WebGPU(context) => context.resize(scope),
      }
    }
  }

  #[constructor]
  #[cppgc]
  #[required(2)]
  fn new(
    #[webidl(options(enforce_range = true))] width: u64,
    #[webidl(options(enforce_range = true))] height: u64,
  ) -> OffscreenCanvas {
    OffscreenCanvas {
      data: Rc::new(RefCell::new(Data::Image(DynamicImage::new(
        width as _,
        height as _,
        ColorType::Rgba8,
      )))),
      active_context: Default::default(),
    }
  }

  #[global]
  fn get_context<'s>(
    &self,
    #[this] this: v8::Global<v8::Object>,
    scope: &mut v8::PinScope<'s, '_>,
    #[webidl] context_id: String,
    #[webidl] options: v8::Local<'s, v8::Value>,
  ) -> Result<Option<v8::Global<v8::Value>>, JsErrorBox> {
    if self.active_context.get().is_none() {
      let create_context: CreateCanvasContext = match context_id.as_str() {
        super::bitmaprenderer::CONTEXT_ID => super::bitmaprenderer::create as _,
        deno_webgpu::canvas::CONTEXT_ID => deno_webgpu::canvas::create as _,
        _ => {
          return Err(JsErrorBox::new(
            "DOMExceptionNotSupportedError",
            format!("Context '{context_id}' not implemented"),
          ));
        }
      };

      let context = create_context(
        this,
        self.data.clone(),
        scope,
        options,
        "Failed to execute 'getContext' on 'OffscreenCanvas'",
        "Argument 2",
      );
      let _ = self.active_context.set((context_id.clone(), context));
    }

    let (name, context) = self.active_context.get().unwrap();

    if &context_id == name {
      Ok(Some(context.clone()))
    } else {
      Ok(None)
    }
  }

  #[cppgc]
  fn transfer_to_image_bitmap(
    &self,
    scope: &mut v8::PinScope<'_, '_>,
  ) -> Result<ImageBitmap, JsErrorBox> {
    if self.active_context.get().is_none() {
      return Err(JsErrorBox::new(
        "DOMExceptionInvalidStateError",
        "Canvas hasn't been initialized yet",
      ));
    }

    let active_context = self.active_context.get().unwrap();
    let active_context_local = v8::Local::new(scope, &active_context.1);
    let context = get_context(&active_context.0, scope, active_context_local);
    match &context {
      Context::Bitmap(_) => {}
      Context::WebGPU(context) => context.bitmap_read_hook(scope)?,
    }

    let data = self.data.replace_with(|image| {
      let data = self.data.borrow();
      let Data::Image(image) = &*data else {
        unreachable!();
      };

      let (width, height) = image.dimensions();
      Data::Image(DynamicImage::new(width, height, ColorType::Rgba8))
    });

    match &context {
      Context::Bitmap(_) => {}
      Context::WebGPU(context) => {
        context.post_transfer_to_image_bitmap_hook(scope)
      }
    }

    let Data::Image(data) = data else {
      unreachable!();
    };

    Ok(ImageBitmap {
      detached: Default::default(),
      data: RefCell::new(data),
    })
  }

  #[reentrant]
  fn convert_to_blob<'s>(
    &self,
    state: Rc<RefCell<OpState>>,
    scope: &mut v8::PinScope<'s, '_>,
    #[webidl] options: ImageEncodeOptions,
  ) -> Result<v8::Local<'s, v8::Object>, JsErrorBox> {
    let state = state.borrow();
    let active_context = self.active_context.get().unwrap();
    let active_context_local = v8::Local::new(scope, &active_context.1);
    match get_context(&active_context.0, scope, active_context_local) {
      Context::Bitmap(_) => {}
      Context::WebGPU(context) => context.bitmap_read_hook(scope)?,
    }

    let data = self.data.borrow();
    let Data::Image(data) = &*data else {
      unreachable!();
    };

    let mut out = vec![];

    match options.r#type.as_str() {
      "image/png" => {
        let encoder = image::codecs::png::PngEncoder::new(&mut out);
        data.write_with_encoder(encoder).unwrap();
      }
      "image/jpeg" => {
        let encoder = image::codecs::jpeg::JpegEncoder::new(&mut out);
        data.write_with_encoder(encoder).unwrap();
      }
      "image/bmp" => {
        let encoder = image::codecs::bmp::BmpEncoder::new(&mut out);
        data.write_with_encoder(encoder).unwrap();
      }
      "image/x-icon" => {
        let encoder = image::codecs::ico::IcoEncoder::new(&mut out);
        data.write_with_encoder(encoder).unwrap();
      }
      _ => todo!(),
    }

    let blob_constructor = state.borrow::<BlobHandle>();
    let blob_constructor = v8::Local::new(scope, &blob_constructor.0);

    let len = out.len();
    let bs = v8::ArrayBuffer::new_backing_store_from_vec(out);
    let shared_bs = bs.make_shared();
    let ab = v8::ArrayBuffer::with_backing_store(scope, &shared_bs);
    let data = v8::Uint8Array::new(scope, ab, 0, len).unwrap();
    let data = v8::Array::new_with_elements(scope, &[data.into()]);

    let key = v8::String::new(scope, "type").unwrap();
    let value = v8::String::new(scope, options.r#type.as_str()).unwrap();

    let null = v8::null(scope);

    let options = v8::Object::with_prototype_and_properties(
      scope,
      null.into(),
      &[key.into()],
      &[value.into()],
    );

    drop(state);

    Ok(
      blob_constructor
        .new_instance(scope, &[data.into(), options.into()])
        .unwrap(),
    )
  }
}

pub enum Context {
  #[allow(dead_code)]
  Bitmap(
    deno_core::cppgc::Ref<crate::bitmaprenderer::ImageBitmapRenderingContext>,
  ),
  WebGPU(deno_core::cppgc::Ref<deno_webgpu::canvas::GPUCanvasContext>),
}

pub fn get_context<'t>(
  id: &'t str,
  scope: &mut v8::PinScope<'_, '_>,
  local: v8::Local<'t, v8::Value>,
) -> Context {
  match id {
    crate::bitmaprenderer::CONTEXT_ID => {
      let ptr = deno_core::cppgc::try_unwrap_cppgc_persistent_object::<
        crate::bitmaprenderer::ImageBitmapRenderingContext,
      >(scope, local)
      .unwrap();
      Context::Bitmap(ptr)
    }
    deno_webgpu::canvas::CONTEXT_ID => {
      let ptr = deno_core::cppgc::try_unwrap_cppgc_persistent_object::<
        deno_webgpu::canvas::GPUCanvasContext,
      >(scope, local)
      .unwrap();
      Context::WebGPU(ptr)
    }
    _ => panic!(),
  }
}

#[derive(WebIDL)]
#[webidl(dictionary)]
struct ImageEncodeOptions {
  #[webidl(default = String::from("image/png"))]
  r#type: String,
  quality: Option<UnrestrictedDouble>,
}
