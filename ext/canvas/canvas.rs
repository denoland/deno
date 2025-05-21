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
use std::cell::OnceCell;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::op_create_image_bitmap::ImageBitmap;

struct BlobHandle(v8::Global<v8::Function>);

pub type GetContext = for<'s, 't> fn(
  id: &'t str,
  scope: &mut v8::HandleScope<'s>,
  local: v8::Local<'t, v8::Value>,
) -> Box<dyn CanvasContextHooks>;
pub struct GetContextContainer(pub GetContext);

pub type CreateCanvasContext = for<'s> fn(
  canvas: v8::Global<v8::Object>,
  data: Rc<RefCell<DynamicImage>>,
  scope: &mut v8::HandleScope<'s>,
  options: v8::Local<'s, v8::Value>,
  prefix: &'static str,
  context: &'static str,
) -> v8::Global<v8::Value>;

pub struct RegisteredContexts(pub HashMap<String, CreateCanvasContext>);

pub struct OffscreenCanvas {
  data: Rc<RefCell<DynamicImage>>,

  active_context: OnceCell<(String, v8::Global<v8::Value>)>,
}

impl GarbageCollected for OffscreenCanvas {
  fn get_name(&self) -> &'static std::ffi::CStr {
    c"OffscreenCanvas"
  }
}

#[op2]
impl OffscreenCanvas {
  #[getter]
  fn width(&self) -> u32 {
    self.data.borrow().width()
  }
  #[setter]
  fn width<'s>(
    &self,
    state: &mut OpState,
    scope: &mut v8::HandleScope<'s>,
    #[webidl(options(enforce_range = true))] value: u64,
  ) {
    {
      self
        .data
        .replace_with(|data| data.crop_imm(0, 0, value as _, data.height()));
    }
    if let Some((id, active_context)) = self.active_context.get() {
      let active_context = v8::Local::new(scope, active_context);
      let get_context = state.borrow::<GetContextContainer>();
      let active_context = get_context.0(id, scope, active_context);
      active_context.resize();
    }
  }

  #[getter]
  fn height(&self) -> u32 {
    self.data.borrow().height()
  }
  #[setter]
  fn height(
    &self,
    state: &mut OpState,
    scope: &mut v8::HandleScope,
    #[webidl(options(enforce_range = true))] value: u64,
  ) {
    {
      self
        .data
        .replace_with(|data| data.crop_imm(0, 0, data.width(), value as _));
    }
    if let Some((id, active_context)) = self.active_context.get() {
      let active_context = v8::Local::new(scope, active_context);
      let get_context = state.borrow::<GetContextContainer>();
      let active_context = get_context.0(id, scope, active_context);
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

      let context = create_context(
        this,
        self.data.clone(),
        scope,
        options,
        "Failed to execute 'getContext' on 'OffscreenCanvas'",
        "Argument 2",
      );
      let _ = self.active_context.set((name.clone(), context));
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
    state: &mut OpState,
    scope: &mut v8::HandleScope,
  ) -> Result<ImageBitmap, JsErrorBox> {
    if self.active_context.get().is_none() {
      return Err(JsErrorBox::new(
        "DOMExceptionInvalidStateError",
        "Canvas hasn't been initialized yet",
      ));
    }

    let active_context = self.active_context.get().unwrap();
    let active_context_local = v8::Local::new(scope, &active_context.1);
    let get_context = state.borrow::<GetContextContainer>();
    let active_context =
      get_context.0(&active_context.0, scope, active_context_local);

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
    let active_context = self.active_context.get().unwrap();
    let active_context_local = v8::Local::new(scope, &active_context.1);
    let get_context = state.borrow::<GetContextContainer>();
    let active_context =
      get_context.0(&active_context.0, scope, active_context_local);

    active_context.bitmap_read_hook();

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

pub trait CanvasContextHooks {
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
