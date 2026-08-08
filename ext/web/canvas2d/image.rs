// Copyright 2018-2026 the Deno authors. MIT license.

use std::sync::Arc;

use deno_core::OpState;
use deno_core::v8;
use deno_error::JsErrorBox;
use deno_image::bitmap::ImageBitmap;
use deno_image::image::GenericImageView;
use vello::peniko;

use crate::canvas2d::error::Canvas2DError;

pub struct ResolvedCanvasImage {
  pub width: u32,
  pub height: u32,
  pub pixels: Vec<u8>,
}

pub type SyncOffscreenCanvasPixelsFn =
  for<'a> fn(
    scope: &mut v8::PinScope<'a, 'a>,
    image: v8::Local<'a, v8::Value>,
  ) -> Result<(u32, u32, Vec<u8>), JsErrorBox>;

pub struct OffscreenCanvasPixelSync(pub SyncOffscreenCanvasPixelsFn);

pub fn set_offscreen_canvas_pixel_sync(
  state: &mut OpState,
  sync: SyncOffscreenCanvasPixelsFn,
) {
  state.put(OffscreenCanvasPixelSync(sync));
}

fn resolve_offscreen_canvas_image<'a>(
  state: &OpState,
  scope: &mut v8::PinScope<'a, 'a>,
  image: v8::Local<'a, v8::Value>,
) -> Result<ResolvedCanvasImage, Canvas2DError> {
  let sync = state
    .try_borrow::<OffscreenCanvasPixelSync>()
    .ok_or(Canvas2DError::NotCanvasImageSource)?;
  // Preserve the callback's own error class (e.g. a plain TypeError for
  // "not a CanvasImageSource", or DOMExceptionInvalidStateError for open
  // layers) instead of forcing everything to InvalidState.
  let (width, height, pixels) = sync.0(scope, image)?;
  Ok(ResolvedCanvasImage {
    width,
    height,
    pixels,
  })
}

/// Resolves an ImageBitmap or OffscreenCanvas into raw RGBA8 pixels.
pub fn resolve_canvas_image_source<'a>(
  state: &OpState,
  scope: &mut v8::PinScope<'a, 'a>,
  image: v8::Local<'a, v8::Value>,
) -> Result<ResolvedCanvasImage, Canvas2DError> {
  if image.is_null_or_undefined() {
    return Err(Canvas2DError::NotCanvasImageSource);
  }

  if let Some(bitmap) =
    deno_core::cppgc::try_unwrap_cppgc_object::<ImageBitmap>(scope, image)
  {
    if bitmap.detached.get().is_some() {
      return Err(Canvas2DError::ImageSourceDetached);
    }
    let data = bitmap.data.borrow();
    let (width, height) = data.dimensions();
    if width == 0 || height == 0 {
      return Err(Canvas2DError::ImageSourceZeroDimensions);
    }
    return Ok(ResolvedCanvasImage {
      width,
      height,
      // `DynamicImage::as_bytes()` returns whatever the decoder's native
      // color type is (e.g. 3 bytes/pixel for opaque PNGs/JPEGs), so the
      // buffer must be normalized to RGBA8 before callers assume a 4-byte
      // stride.
      pixels: data.to_rgba8().into_raw(),
    });
  }

  resolve_offscreen_canvas_image(state, scope, image)
}

/// Builds a peniko ImageData from resolved RGBA8 pixels.
pub fn image_data_from_pixels(
  pixels: Vec<u8>,
  width: u32,
  height: u32,
) -> peniko::ImageData {
  let bytes: Arc<dyn AsRef<[u8]> + Send + Sync> = Arc::new(pixels);
  peniko::ImageData {
    data: peniko::Blob::new(bytes),
    format: peniko::ImageFormat::Rgba8,
    alpha_type: peniko::ImageAlphaType::Alpha,
    width,
    height,
  }
}

pub(super) fn image_data_from_premultiplied_pixels(
  pixels: Vec<u8>,
  width: u32,
  height: u32,
) -> peniko::ImageData {
  let bytes: Arc<dyn AsRef<[u8]> + Send + Sync> = Arc::new(pixels);
  peniko::ImageData {
    data: peniko::Blob::new(bytes),
    format: peniko::ImageFormat::Rgba8,
    alpha_type: peniko::ImageAlphaType::AlphaPremultiplied,
    width,
    height,
  }
}

pub(super) fn unpremultiply_rgba(data: &mut [u8]) {
  for pixel in data.chunks_exact_mut(4) {
    let a = pixel[3] as u32;
    if a > 0 && a < 255 {
      pixel[0] = ((pixel[0] as u32 * 255 + a / 2) / a).min(255) as u8;
      pixel[1] = ((pixel[1] as u32 * 255 + a / 2) / a).min(255) as u8;
      pixel[2] = ((pixel[2] as u32 * 255 + a / 2) / a).min(255) as u8;
    }
  }
}
