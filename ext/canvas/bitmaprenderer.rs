use std::cell::RefCell;
use std::rc::Rc;

use deno_core::cppgc::Ptr;
use deno_core::op2;
use deno_core::v8;
use deno_core::webidl::Nullable;
use deno_core::webidl::WebIdlConverter;
use deno_core::GarbageCollected;
use deno_core::WebIDL;
use deno_error::JsErrorBox;
use image::DynamicImage;
use image::GenericImageView;

use crate::canvas::CanvasContext;
use crate::op_create_image_bitmap::ImageBitmap;

pub struct ImageBitmapRenderingContext {
  canvas: v8::Global<v8::Object>,
  bitmap: Rc<RefCell<DynamicImage>>,

  alpha: bool,
}

impl GarbageCollected for ImageBitmapRenderingContext {}

#[op2]
impl ImageBitmapRenderingContext {
  #[getter]
  #[global]
  fn canvas(&self) -> v8::Global<v8::Object> {
    self.canvas.clone()
  }

  fn transfer_from_image_bitmap(
    &self,
    #[webidl] bitmap: Nullable<Ptr<ImageBitmap>>,
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
      let data =
        bitmap
          .data
          .replace(DynamicImage::new(0, 0, image::ColorType::Rgba8));
      self.bitmap.replace(data);
    } else {
      let (width, height) = {
        let bitmap = self.bitmap.borrow();
        bitmap.dimensions()
      };

      self.bitmap.replace(DynamicImage::new(
        width,
        height,
        image::ColorType::Rgba8,
      ));
    }

    Ok(())
  }
}

impl CanvasContext for ImageBitmapRenderingContext {
  fn value(&self) -> v8::Global<v8::Value> {
    todo!()
  }

  fn resize(&self) {}

  fn bitmap_read_hook(&self) {}

  fn post_transfer_to_image_bitmap_hook(&self) {}
}

pub const CONTEXT_ID: &str = "bitmaprenderer";

pub fn create<'s>(
  canvas: v8::Global<v8::Object>,
  data: Rc<RefCell<DynamicImage>>,
  scope: &mut v8::HandleScope<'s>,
  options: v8::Local<'s, v8::Value>,
  prefix: &'static str,
  context: &'static str,
) -> Box<dyn CanvasContext> {
  let settings = ImageBitmapRenderingContextSettings::convert(
    scope,
    options,
    prefix.into(),
    (|| context.into()).into(),
    &(),
  )
  .unwrap();

  Box::new(ImageBitmapRenderingContext {
    alpha: settings.alpha,
    canvas,
    bitmap: data,
  })
}

#[derive(WebIDL)]
#[webidl(dictionary)]
struct ImageBitmapRenderingContextSettings {
  #[webidl(default = true)]
  alpha: bool,
}
