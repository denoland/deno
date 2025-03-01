use std::cell::OnceCell;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use deno_core::cppgc::SameObject;
use deno_core::op2;
use deno_core::v8;
use deno_core::webidl::UnrestrictedDouble;
use deno_core::GarbageCollected;
use deno_core::OpState;
use deno_core::WebIDL;
use deno_error::JsErrorBox;
use image::ColorType;
use image::DynamicImage;
use image::GenericImageView;

use crate::op_create_image_bitmap::ImageBitmap;

struct BlobHandle(v8::Global<v8::Function>);

pub type CreateCanvasContext = for<'s> fn(
  canvas: v8::Global<v8::Object>,
  data: Rc<RefCell<DynamicImage>>,
  scope: &mut v8::HandleScope<'s>,
  options: v8::Local<'s, v8::Value>,
  prefix: &'static str,
  context: &'static str,
) -> Box<dyn CanvasContext>;

pub struct RegisteredContexts(pub HashMap<String, CreateCanvasContext>);

pub struct OffscreenCanvas {
  data: Rc<RefCell<DynamicImage>>,

  active_context: OnceCell<(String, Box<dyn CanvasContext>)>,
}

impl GarbageCollected for OffscreenCanvas {}

#[op2]
impl OffscreenCanvas {
  #[getter]
  fn width(&self) -> u32 {
    self.data.borrow().width()
  }
  #[setter]
  fn width(&self, #[webidl(options(enforce_range = true))] value: u64) {
    {
      self
        .data
        .replace_with(|data| data.crop_imm(0, 0, value as _, data.height()));
    }
    if let Some((_, active_context)) = self.active_context.get() {
      active_context.resize();
    }
  }

  #[getter]
  fn height(&self) -> u32 {
    self.data.borrow().height()
  }
  #[setter]
  fn height(&self, #[webidl(options(enforce_range = true))] value: u64) {
    {
      self
        .data
        .replace_with(|data| data.crop_imm(0, 0, data.width(), value as _));
    }
    if let Some((_, active_context)) = self.active_context.get() {
      active_context.resize();
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
      data: Rc::new(RefCell::new(DynamicImage::new(
        width as _,
        height as _,
        ColorType::Rgba8,
      ))),
      active_context: Default::default(),
    }
  }

  #[global]
  fn get_context<'s>(
    &self,
    state: &mut OpState,
    #[this] this: v8::Global<v8::Object>,
    scope: &mut v8::HandleScope<'s>,
    #[webidl] context_id: String,
    #[webidl] options: v8::Local<'s, v8::Value>,
  ) -> Result<Option<v8::Global<v8::Value>>, JsErrorBox> {
    if self.active_context.get().is_none() {
      let registered_contexts = state.borrow::<RegisteredContexts>();

      let (name, create_context) = registered_contexts
        .0
        .get_key_value(&context_id)
        .ok_or_else(|| {
          JsErrorBox::new(
            "DOMExceptionNotSupportedError",
            format!("Context '{context_id}' not implemented"),
          )
        })?;

      let _ = self.active_context.set((
        name.clone(),
        
        create_context(
          this,
          self.data.clone(),
          scope,
          options,
          "Failed to execute 'getContext' on 'OffscreenCanvas'",
          "Argument 2",
        ),
      ));
    }

    let (name, context) = self.active_context.get().unwrap();

    if &context_id == name {
      Ok(Some(context.value()))
    } else {
      Ok(None)
    }
  }

  #[cppgc]
  fn transfer_to_image_bitmap(&self) -> Result<ImageBitmap, JsErrorBox> {
    if self.active_context.get().is_none() {
      return Err(JsErrorBox::new(
        "DOMExceptionInvalidStateError",
        "Canvas hasn't been initialized yet",
      ));
    }

    let active_context = &self.active_context.get().as_ref().unwrap().1;

    active_context.bitmap_read_hook();

    let data = self.data.replace_with(|image| {
      let (width, height) = image.dimensions();
      DynamicImage::new(width, height, ColorType::Rgba8)
    });

    active_context.post_transfer_to_image_bitmap_hook();

    Ok(ImageBitmap {
      detached: Default::default(),
      data: RefCell::new(data),
    })
  }

  fn convert_to_blob<'s>(
    &self,
    state: &mut OpState,
    scope: &mut v8::HandleScope<'s>,
    #[webidl] options: ImageEncodeOptions,
  ) -> Result<v8::Local<'s, v8::Object>, JsErrorBox> {
    self
      .active_context
      .get()
      .as_ref()
      .unwrap()
      .1
      .bitmap_read_hook();

    let data = self.data.borrow();

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
    let blob_constructor = v8::Local::new(scope, blob_constructor.0.clone());

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

    Ok(
      blob_constructor
        .new_instance(scope, &[data.into(), options.into()])
        .unwrap(),
    )
  }
}

pub trait CanvasContext: GarbageCollected {
  fn value(&self) -> v8::Global<v8::Value>;

  fn resize(&self);

  fn bitmap_read_hook(&self);

  fn post_transfer_to_image_bitmap_hook(&self);
}

#[derive(WebIDL)]
#[webidl(dictionary)]
struct ImageEncodeOptions {
  #[webidl(default = String::from("image/png"))]
  r#type: String,
  quality: Option<UnrestrictedDouble>,
}
