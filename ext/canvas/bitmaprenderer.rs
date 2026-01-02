// Copyright 2018-2025 the Deno authors. MIT license.
use std::cell::RefCell;
use std::rc::Rc;

use deno_core::GarbageCollected;
use deno_core::OpState;
use deno_core::WebIDL;
use deno_core::cppgc::Ref;
use deno_core::op2;
use deno_core::v8;
use deno_core::v8::cppgc::Visitor;
use deno_core::webidl::Nullable;
use deno_core::webidl::WebIdlConverter;
use deno_error::JsErrorBox;
use deno_image::image;
use deno_image::image::DynamicImage;
use deno_image::image::GenericImageView;
use deno_image::op_create_image_bitmap::ImageBitmap;
use deno_webgpu::canvas::Data;

pub struct ImageBitmapRenderingContext {
  canvas: v8::Global<v8::Object>,
  data: Rc<RefCell<Data>>,

  alpha: bool,
}

// SAFETY: we're sure this can be GCed
unsafe impl GarbageCollected for ImageBitmapRenderingContext {
  fn trace(&self, _visitor: &mut Visitor) {}

  fn get_name(&self) -> &'static std::ffi::CStr {
    c"ImageBitmapRenderingContext"
  }
}

#[op2]
impl ImageBitmapRenderingContext {
  #[getter]
  #[global]
  fn canvas(&self) -> v8::Global<v8::Object> {
    self.canvas.clone()
  }

  fn transfer_from_image_bitmap(
    &self,
    state: &mut OpState,
    #[webidl] bitmap: Nullable<Ref<ImageBitmap>>,
  ) -> Result<(), JsErrorBox> {
    if let Some(bitmap) = bitmap.into_option() {
      if bitmap.detached.get().is_some() {
        return Err(JsErrorBox::new(
          "DOMExceptionInvalidStateError",
          "The provided bitmap is detached.",
        ));
      }

      // the spec states to set ImageBitmapRenderingContext's bitmap to the same at ImageBitmap's without copy,
      // and then it detaches and clears it. So we move it instead and then detach it.
      // Maybe storing it as an RefCell<Rc> might be necessary at some point
      // but that case hasnt been hit yet

      let _ = bitmap.detached.set(());
      let new_data =
        bitmap
          .data
          .replace(DynamicImage::new(0, 0, image::ColorType::Rgba8));

      let mut data = self.data.borrow_mut();
      match &mut *data {
        Data::Image(image) => {
          *image = new_data;
        }
        Data::Surface { .. } => {
          todo!()
        }
      }
    } else {
      let mut data = self.data.borrow_mut();
      match &mut *data {
        Data::Image(image) => {
          let (width, height) = image.dimensions();

          *image = DynamicImage::new(width, height, image::ColorType::Rgba8);
        }
        Data::Surface { .. } => {
          todo!()
        }
      }
    }

    Ok(())
  }
}

pub const CONTEXT_ID: &str = "bitmaprenderer";

pub fn create<'s>(
  canvas: v8::Global<v8::Object>,
  data: Rc<RefCell<Data>>,
  scope: &mut v8::PinScope<'s, '_>,
  options: v8::Local<'s, v8::Value>,
  prefix: &'static str,
  context: &'static str,
) -> v8::Global<v8::Value> {
  let settings = ImageBitmapRenderingContextSettings::convert(
    scope,
    options,
    prefix.into(),
    (|| context.into()).into(),
    &(),
  )
  .unwrap();

  let obj = deno_core::cppgc::make_cppgc_object(
    scope,
    ImageBitmapRenderingContext {
      alpha: settings.alpha,
      canvas,
      data,
    },
  );
  v8::Global::new(scope, obj.cast())
}

#[derive(WebIDL)]
#[webidl(dictionary)]
struct ImageBitmapRenderingContextSettings {
  #[webidl(default = true)]
  alpha: bool,
}
