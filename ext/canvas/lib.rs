// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use deno_core::error::type_error;
use deno_core::error::AnyError;
use deno_core::op2;
use deno_core::JsBuffer;
use deno_core::ToJsBuffer;
use deno_terminal::colors::cyan;
use image::codecs::bmp::BmpDecoder;
use image::codecs::gif::GifDecoder;
use image::codecs::ico::IcoDecoder;
use image::codecs::jpeg::JpegDecoder;
use image::codecs::png::PngDecoder;
use image::codecs::webp::WebPDecoder;
use image::imageops::overlay;
use image::imageops::FilterType;
use image::ColorType;
use image::DynamicImage;
use image::GenericImageView;
use image::ImageBuffer;
use image::ImageError;
use image::LumaA;
use image::Pixel;
use image::Primitive;
use image::Rgb;
use image::Rgba;
use image::RgbaImage;
use num_traits::NumCast;
use num_traits::SaturatingMul;
use serde::Deserialize;
use serde::Serialize;
use std::borrow::Cow;
use std::io::BufRead;
use std::io::BufReader;
use std::io::Cursor;
use std::io::Seek;
use std::path::PathBuf;

pub mod error;
use error::DOMExceptionInvalidStateError;

fn to_js_buffer(image: &DynamicImage) -> ToJsBuffer {
  image.as_bytes().to_vec().into()
}

fn image_error_message<'a, T: Into<Cow<'a, str>>>(
  opreation: T,
  reason: T,
) -> String {
  format!(
    "An error has occurred while {}.
reason: {}",
    opreation.into(),
    reason.into(),
  )
}

// reference
// https://github.com/image-rs/image/blob/6d19ffa72756c1b00e7979a90f8794a0ef847b88/src/color.rs#L739
trait ProcessPremultiplyAlpha {
  fn premultiply_alpha(&self) -> Self;
}

impl<T: Primitive> ProcessPremultiplyAlpha for LumaA<T> {
  fn premultiply_alpha(&self) -> Self {
    let max_t = T::DEFAULT_MAX_VALUE;

    let mut pixel = [self.0[0], self.0[1]];
    let alpha_index = pixel.len() - 1;
    let alpha = pixel[alpha_index];
    let normalized_alpha = alpha.to_f32().unwrap() / max_t.to_f32().unwrap();

    if normalized_alpha == 0.0 {
      return LumaA::<T>([pixel[0], pixel[alpha_index]]);
    }

    for rgb in pixel.iter_mut().take(alpha_index) {
      *rgb = NumCast::from((rgb.to_f32().unwrap() * normalized_alpha).round())
        .unwrap()
    }

    LumaA::<T>([pixel[0], pixel[alpha_index]])
  }
}

impl<T: Primitive> ProcessPremultiplyAlpha for Rgba<T> {
  fn premultiply_alpha(&self) -> Self {
    let max_t = T::DEFAULT_MAX_VALUE;

    let mut pixel = [self.0[0], self.0[1], self.0[2], self.0[3]];
    let alpha_index = pixel.len() - 1;
    let alpha = pixel[alpha_index];
    let normalized_alpha = alpha.to_f32().unwrap() / max_t.to_f32().unwrap();

    if normalized_alpha == 0.0 {
      return Rgba::<T>([pixel[0], pixel[1], pixel[2], pixel[alpha_index]]);
    }

    for rgb in pixel.iter_mut().take(alpha_index) {
      *rgb = NumCast::from((rgb.to_f32().unwrap() * normalized_alpha).round())
        .unwrap()
    }

    Rgba::<T>([pixel[0], pixel[1], pixel[2], pixel[alpha_index]])
  }
}

fn process_premultiply_alpha<I, P, S>(image: &I) -> ImageBuffer<P, Vec<S>>
where
  I: GenericImageView<Pixel = P>,
  P: Pixel<Subpixel = S> + ProcessPremultiplyAlpha + 'static,
  S: Primitive + 'static,
{
  let (width, height) = image.dimensions();
  let mut out = ImageBuffer::new(width, height);

  for (x, y, pixel) in image.pixels() {
    let pixel = pixel.premultiply_alpha();

    out.put_pixel(x, y, pixel);
  }

  out
}

fn apply_premultiply_alpha(
  image: &DynamicImage,
) -> Result<DynamicImage, AnyError> {
  match image.color() {
    ColorType::La8 => Ok(DynamicImage::ImageLumaA8(process_premultiply_alpha(
      &image.to_luma_alpha8(),
    ))),
    ColorType::Rgba8 => Ok(DynamicImage::ImageRgba8(
      process_premultiply_alpha(&image.to_rgba8()),
    )),
    ColorType::La16 => Ok(DynamicImage::ImageLumaA16(
      process_premultiply_alpha(&image.to_luma_alpha16()),
    )),
    ColorType::Rgba16 => Ok(DynamicImage::ImageRgba16(
      process_premultiply_alpha(&image.to_rgba16()),
    )),
    _ => Err(type_error(image_error_message(
      "apply premultiplyAlpha: premultiply",
      "The color type is not supported.",
    ))),
  }
}

trait ProcessUnpremultiplyAlpha {
  /// To determine if the image is premultiplied alpha,
  /// checking premultiplied RGBA value is one where any of the R/G/B channel values exceeds the alpha channel value.\
  /// https://www.w3.org/TR/webgpu/#color-spaces
  fn is_premultiplied_alpha(&self) -> bool;
  fn unpremultiply_alpha(&self) -> Self;
}

impl<T: Primitive + SaturatingMul + Ord> ProcessUnpremultiplyAlpha for Rgba<T> {
  fn is_premultiplied_alpha(&self) -> bool {
    let max_t = T::DEFAULT_MAX_VALUE;

    let pixel = [self.0[0], self.0[1], self.0[2]];
    let alpha_index = self.0.len() - 1;
    let alpha = self.0[alpha_index];

    match pixel.iter().max() {
      Some(rgb_max) => rgb_max < &max_t.saturating_mul(&alpha),
      // usually doesn't reach here
      None => false,
    }
  }

  fn unpremultiply_alpha(&self) -> Self {
    let max_t = T::DEFAULT_MAX_VALUE;

    let mut pixel = [self.0[0], self.0[1], self.0[2], self.0[3]];
    let alpha_index = pixel.len() - 1;
    let alpha = pixel[alpha_index];

    for rgb in pixel.iter_mut().take(alpha_index) {
      *rgb = NumCast::from(
        (rgb.to_f32().unwrap()
          / (alpha.to_f32().unwrap() / max_t.to_f32().unwrap()))
        .round(),
      )
      .unwrap();
    }

    Rgba::<T>([pixel[0], pixel[1], pixel[2], pixel[alpha_index]])
  }
}

impl<T: Primitive + SaturatingMul + Ord> ProcessUnpremultiplyAlpha
  for LumaA<T>
{
  fn is_premultiplied_alpha(&self) -> bool {
    let max_t = T::DEFAULT_MAX_VALUE;

    let pixel = [self.0[0]];
    let alpha_index = self.0.len() - 1;
    let alpha = self.0[alpha_index];

    pixel[0] < max_t.saturating_mul(&alpha)
  }

  fn unpremultiply_alpha(&self) -> Self {
    let max_t = T::DEFAULT_MAX_VALUE;

    let mut pixel = [self.0[0], self.0[1]];
    let alpha_index = pixel.len() - 1;
    let alpha = pixel[alpha_index];

    for rgb in pixel.iter_mut().take(alpha_index) {
      *rgb = NumCast::from(
        (rgb.to_f32().unwrap()
          / (alpha.to_f32().unwrap() / max_t.to_f32().unwrap()))
        .round(),
      )
      .unwrap();
    }

    LumaA::<T>([pixel[0], pixel[alpha_index]])
  }
}

fn process_unpremultiply_alpha<I, P, S>(image: &I) -> ImageBuffer<P, Vec<S>>
where
  I: GenericImageView<Pixel = P>,
  P: Pixel<Subpixel = S> + ProcessUnpremultiplyAlpha + 'static,
  S: Primitive + 'static,
{
  let (width, height) = image.dimensions();
  let mut out = ImageBuffer::new(width, height);

  let is_premultiplied_alpha = image
    .pixels()
    .any(|(_, _, pixel)| pixel.is_premultiplied_alpha());

  for (x, y, pixel) in image.pixels() {
    let pixel = if is_premultiplied_alpha {
      pixel.unpremultiply_alpha()
    } else {
      // return the original
      pixel
    };

    out.put_pixel(x, y, pixel);
  }

  out
}

fn apply_unpremultiply_alpha(
  image: &DynamicImage,
) -> Result<DynamicImage, AnyError> {
  match image.color() {
    ColorType::La8 => Ok(DynamicImage::ImageLumaA8(
      process_unpremultiply_alpha(&image.to_luma_alpha8()),
    )),
    ColorType::Rgba8 => Ok(DynamicImage::ImageRgba8(
      process_unpremultiply_alpha(&image.to_rgba8()),
    )),
    ColorType::La16 => Ok(DynamicImage::ImageLumaA16(
      process_unpremultiply_alpha(&image.to_luma_alpha16()),
    )),
    ColorType::Rgba16 => Ok(DynamicImage::ImageRgba16(
      process_unpremultiply_alpha(&image.to_rgba16()),
    )),
    _ => Err(type_error(image_error_message(
      "apply premultiplyAlpha: none",
      "The color type is not supported.",
    ))),
  }
}

// reference
// https://www.w3.org/TR/css-color-4/#color-conversion-code
fn srgb_to_linear<T: Primitive>(value: T) -> f32 {
  if value.to_f32().unwrap() <= 0.04045 {
    value.to_f32().unwrap() / 12.92
  } else {
    ((value.to_f32().unwrap() + 0.055) / 1.055).powf(2.4)
  }
}

// reference
// https://www.w3.org/TR/css-color-4/#color-conversion-code
fn linear_to_display_p3<T: Primitive>(value: T) -> f32 {
  if value.to_f32().unwrap() <= 0.0031308 {
    value.to_f32().unwrap() * 12.92
  } else {
    1.055 * value.to_f32().unwrap().powf(1.0 / 2.4) - 0.055
  }
}

fn normalize_value_to_0_1<T: Primitive>(value: T) -> f32 {
  value.to_f32().unwrap() / T::DEFAULT_MAX_VALUE.to_f32().unwrap()
}

fn unnormalize_value_from_0_1<T: Primitive>(value: f32) -> T {
  NumCast::from(
    (value.clamp(0.0, 1.0) * T::DEFAULT_MAX_VALUE.to_f32().unwrap()).round(),
  )
  .unwrap()
}

fn srgb_to_display_p3<T: Primitive>(r: T, g: T, b: T) -> (T, T, T) {
  // normalize the value to 0.0 - 1.0
  let (r, g, b) = (
    normalize_value_to_0_1(r),
    normalize_value_to_0_1(g),
    normalize_value_to_0_1(b),
  );

  // sRGB -> Linear RGB
  let (r, g, b) = (srgb_to_linear(r), srgb_to_linear(g), srgb_to_linear(b));

  // Display-P3 (RGB) -> Display-P3 (XYZ)
  //
  // inv[ P3-D65 (D65) to XYZ ] * [ sRGB (D65) to XYZ ]
  // http://www.brucelindbloom.com/index.html?Eqn_RGB_XYZ_Matrix.html
  // https://fujiwaratko.sakura.ne.jp/infosci/colorspace/colorspace2_e.html

  // [ sRGB (D65) to XYZ ]
  #[rustfmt::skip]
    let (m1x, m1y, m1z) = (
      [0.4124564, 0.3575761, 0.1804375],
      [0.2126729, 0.7151522, 0.0721750],
      [0.0193339, 0.1191920, 0.9503041],
    );

  let (r, g, b) = (
    r * m1x[0] + g * m1x[1] + b * m1x[2],
    r * m1y[0] + g * m1y[1] + b * m1y[2],
    r * m1z[0] + g * m1z[1] + b * m1z[2],
  );

  // inv[ P3-D65 (D65) to XYZ ]
  #[rustfmt::skip]
    let (m2x, m2y, m2z) = (
      [   2.493496911941425, -0.9313836179191239, -0.40271078445071684 ],
      [ -0.8294889695615747,  1.7626640603183463, 0.023624685841943577 ],
      [ 0.03584583024378447,-0.07617238926804182,   0.9568845240076872 ],
    );

  let (r, g, b) = (
    r * m2x[0] + g * m2x[1] + b * m2x[2],
    r * m2y[0] + g * m2y[1] + b * m2y[2],
    r * m2z[0] + g * m2z[1] + b * m2z[2],
  );

  // This calculation is similar as above that it is a little faster, but less accurate.
  // let r = 0.8225 * r + 0.1774 * g + 0.0000 * b;
  // let g = 0.0332 * r + 0.9669 * g + 0.0000 * b;
  // let b = 0.0171 * r + 0.0724 * g + 0.9108 * b;

  // Display-P3 (Linear) -> Display-P3
  let (r, g, b) = (
    linear_to_display_p3(r),
    linear_to_display_p3(g),
    linear_to_display_p3(b),
  );

  // unnormalize the value from 0.0 - 1.0
  (
    unnormalize_value_from_0_1(r),
    unnormalize_value_from_0_1(g),
    unnormalize_value_from_0_1(b),
  )
}

trait ProcessColorSpaceConversion {
  /// Display P3 Color Encoding (v 1.0)  
  /// https://www.color.org/chardata/rgb/DisplayP3.xalter
  fn process_srgb_to_display_p3(&self) -> Self;
}

impl<T: Primitive> ProcessColorSpaceConversion for Rgb<T> {
  fn process_srgb_to_display_p3(&self) -> Self {
    let (r, g, b) = (self.0[0], self.0[1], self.0[2]);

    let (r, g, b) = srgb_to_display_p3(r, g, b);

    Rgb::<T>([r, g, b])
  }
}

impl<T: Primitive> ProcessColorSpaceConversion for Rgba<T> {
  fn process_srgb_to_display_p3(&self) -> Self {
    let (r, g, b, a) = (self.0[0], self.0[1], self.0[2], self.0[3]);

    let (r, g, b) = srgb_to_display_p3(r, g, b);

    Rgba::<T>([r, g, b, a])
  }
}

fn process_srgb_to_display_p3<I, P, S>(image: &I) -> ImageBuffer<P, Vec<S>>
where
  I: GenericImageView<Pixel = P>,
  P: Pixel<Subpixel = S> + ProcessColorSpaceConversion + 'static,
  S: Primitive + 'static,
{
  let (width, height) = image.dimensions();
  let mut out = ImageBuffer::new(width, height);

  for (x, y, pixel) in image.pixels() {
    let pixel = pixel.process_srgb_to_display_p3();

    out.put_pixel(x, y, pixel);
  }

  out
}

fn apply_srgb_to_display_p3(
  image: &DynamicImage,
) -> Result<DynamicImage, AnyError> {
  match image.color() {
    // The conversion of the lumincance color types to the display-p3 color space is meaningless.
    ColorType::L8 => Ok(DynamicImage::ImageLuma8(image.to_luma8())),
    ColorType::L16 => Ok(DynamicImage::ImageLuma16(image.to_luma16())),
    ColorType::La8 => Ok(DynamicImage::ImageLumaA8(image.to_luma_alpha8())),
    ColorType::La16 => Ok(DynamicImage::ImageLumaA16(image.to_luma_alpha16())),
    ColorType::Rgb8 => Ok(DynamicImage::ImageRgb8(process_srgb_to_display_p3(
      &image.to_rgb8(),
    ))),
    ColorType::Rgb16 => Ok(DynamicImage::ImageRgb16(
      process_srgb_to_display_p3(&image.to_rgb16()),
    )),
    ColorType::Rgba8 => Ok(DynamicImage::ImageRgba8(
      process_srgb_to_display_p3(&image.to_rgba8()),
    )),
    ColorType::Rgba16 => Ok(DynamicImage::ImageRgba16(
      process_srgb_to_display_p3(&image.to_rgba16()),
    )),
    _ => Err(type_error(image_error_message(
      "apply colorspace: display-p3",
      "The color type is not supported.",
    ))),
  }
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
enum ImageResizeQuality {
  Pixelated,
  Low,
  Medium,
  High,
}

#[derive(Debug, Deserialize, PartialEq)]
// Follow the cases defined in the spec
enum ImageBitmapSource {
  Blob,
  ImageData,
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
enum PremultiplyAlpha {
  Default,
  Premultiply,
  None,
}

// https://github.com/gfx-rs/wgpu/blob/04618b36a89721c23dc46f5844c71c0e10fc7844/wgpu-types/src/lib.rs#L6948C10-L6948C30
#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
enum PredefinedColorSpace {
  Srgb,
  #[serde(rename = "display-p3")]
  DisplayP3,
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
enum ColorSpaceConversion {
  Default,
  None,
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
enum ImageOrientation {
  FlipY,
  #[serde(rename = "from-image")]
  FromImage,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ImageProcessArgs {
  width: u32,
  height: u32,
  sx: Option<i32>,
  sy: Option<i32>,
  sw: Option<i32>,
  sh: Option<i32>,
  image_orientation: ImageOrientation,
  premultiply_alpha: PremultiplyAlpha,
  predefined_color_space: PredefinedColorSpace,
  color_space_conversion: ColorSpaceConversion,
  resize_width: Option<u32>,
  resize_height: Option<u32>,
  resize_quality: ImageResizeQuality,
  image_bitmap_source: ImageBitmapSource,
  mime_type: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ImageProcessResult {
  data: ToJsBuffer,
  width: u32,
  height: u32,
}

//
// About the animated image
// > Blob .4
// > ... If this is an animated image, imageBitmap's bitmap data must only be taken from
// > the default image of the animation (the one that the format defines is to be used when animation is
// > not supported or is disabled), or, if there is no such image, the first frame of the animation.
// https://html.spec.whatwg.org/multipage/imagebitmap-and-animations.html
//
// see also browser implementations: (The implementation of Gecko and WebKit is hard to read.)
// https://source.chromium.org/chromium/chromium/src/+/bdbc054a6cabbef991904b5df9066259505cc686:third_party/blink/renderer/platform/image-decoders/image_decoder.h;l=175-189
//

trait ImageDecoderFromReader<'a, R: BufRead + Seek> {
  fn to_decoder(reader: R) -> Result<Self, AnyError>
  where
    Self: Sized;
  fn to_intermediate_image(self) -> Result<DynamicImage, AnyError>;
}

type ImageDecoderFromReaderType<'a> = BufReader<Cursor<&'a [u8]>>;

fn image_decoding_error(error: ImageError) -> DOMExceptionInvalidStateError {
  DOMExceptionInvalidStateError::new(&image_error_message(
    "decoding",
    &error.to_string(),
  ))
}

macro_rules! impl_image_decoder_from_reader {
  ($decoder:ty, $reader:ty) => {
    impl<'a, R: BufRead + Seek> ImageDecoderFromReader<'a, R> for $decoder {
      fn to_decoder(reader: R) -> Result<Self, AnyError>
      where
        Self: Sized,
      {
        match <$decoder>::new(reader) {
          Ok(decoder) => Ok(decoder),
          Err(err) => return Err(image_decoding_error(err).into()),
        }
      }
      fn to_intermediate_image(self) -> Result<DynamicImage, AnyError> {
        match DynamicImage::from_decoder(self) {
          Ok(image) => Ok(image),
          Err(err) => Err(image_decoding_error(err).into()),
        }
      }
    }
  };
}

// If PngDecoder decodes an animated image, it returns the default image if one is set, or the first frame if not.
impl_image_decoder_from_reader!(PngDecoder<R>, ImageDecoderFromReaderType);
impl_image_decoder_from_reader!(JpegDecoder<R>, ImageDecoderFromReaderType);
// The GifDecoder decodes the first frame.
impl_image_decoder_from_reader!(GifDecoder<R>, ImageDecoderFromReaderType);
impl_image_decoder_from_reader!(BmpDecoder<R>, ImageDecoderFromReaderType);
impl_image_decoder_from_reader!(IcoDecoder<R>, ImageDecoderFromReaderType);
// The WebPDecoder decodes the first frame.
impl_image_decoder_from_reader!(WebPDecoder<R>, ImageDecoderFromReaderType);

fn decode_bitmap_data(
  buf: &[u8],
  width: u32,
  height: u32,
  image_bitmap_source: &ImageBitmapSource,
  mime_type: String,
) -> Result<(DynamicImage, u32, u32), AnyError> {
  let (view, width, height) = match image_bitmap_source {
    ImageBitmapSource::Blob => {
      let image = match &*mime_type {
        // Should we support the "image/apng" MIME type here?
        "image/png" => {
          let decoder: PngDecoder<ImageDecoderFromReaderType> =
            ImageDecoderFromReader::to_decoder(BufReader::new(Cursor::new(
              buf,
            )))?;
          decoder.to_intermediate_image()?
        }
        "image/jpeg" => {
          let decoder: JpegDecoder<ImageDecoderFromReaderType> =
            ImageDecoderFromReader::to_decoder(BufReader::new(Cursor::new(
              buf,
            )))?;
          decoder.to_intermediate_image()?
        }
        "image/gif" => {
          let decoder: GifDecoder<ImageDecoderFromReaderType> =
            ImageDecoderFromReader::to_decoder(BufReader::new(Cursor::new(
              buf,
            )))?;
          decoder.to_intermediate_image()?
        }
        "image/bmp" => {
          let decoder: BmpDecoder<ImageDecoderFromReaderType> =
            ImageDecoderFromReader::to_decoder(BufReader::new(Cursor::new(
              buf,
            )))?;
          decoder.to_intermediate_image()?
        }
        "image/x-icon" => {
          let decoder: IcoDecoder<ImageDecoderFromReaderType> =
            ImageDecoderFromReader::to_decoder(BufReader::new(Cursor::new(
              buf,
            )))?;
          decoder.to_intermediate_image()?
        }
        "image/webp" => {
          let decoder: WebPDecoder<ImageDecoderFromReaderType> =
            ImageDecoderFromReader::to_decoder(BufReader::new(Cursor::new(
              buf,
            )))?;
          decoder.to_intermediate_image()?
        }
        "" => {
          return Err(
            DOMExceptionInvalidStateError::new(
              &format!("The MIME type of source image is not specified.
INFO: The behavior of the Blob constructor in browsers is different from the spec.
It needs to specify the MIME type like {} that works well between Deno and browsers.
See: https://developer.mozilla.org/en-US/docs/Web/API/Blob/type\n",
              cyan("new Blob([blobParts], { type: 'image/png' })")
            )).into(),
          )
        }
        // return an error if the MIME type is not supported in the variable list of ImageTypePatternTable below
        // ext/web/01_mimesniff.js
        //
        // NOTE: Chromium supports AVIF
        // https://source.chromium.org/chromium/chromium/src/+/ef3f4e4ed97079dc57861d1195fb2389483bc195:third_party/blink/renderer/platform/image-decoders/image_decoder.cc;l=311
        x => {
          return Err(
            DOMExceptionInvalidStateError::new(
              &format!("The the MIME type {} of source image is not a supported format.
INFO: The following MIME types are supported:
See: https://mimesniff.spec.whatwg.org/#image-type-pattern-matching-algorithm\n",
              x
            )).into()
          )
        }
      };

      let width = image.width();
      let height = image.height();

      (image, width, height)
    }
    ImageBitmapSource::ImageData => {
      // > 4.12.5.1.15 Pixel manipulation
      // > imagedata.data
      // >   Returns the one-dimensional array containing the data in RGBA order, as integers in the range 0 to 255.
      // https://html.spec.whatwg.org/multipage/canvas.html#pixel-manipulation
      let image = match RgbaImage::from_raw(width, height, buf.into()) {
        Some(image) => image.into(),
        None => {
          return Err(type_error(image_error_message(
            "decoding",
            "The Chunk Data is not big enough with the specified width and height.",
          )))
        }
      };

      (image, width, height)
    }
  };

  Ok((view, width, height))
}

#[op2]
#[serde]
fn op_image_process(
  #[buffer] zero_copy: JsBuffer,
  #[serde] args: ImageProcessArgs,
) -> Result<ImageProcessResult, AnyError> {
  let buf = &*zero_copy;
  let ImageProcessArgs {
    width,
    height,
    sh,
    sw,
    sx,
    sy,
    image_orientation,
    premultiply_alpha,
    predefined_color_space,
    color_space_conversion,
    resize_width,
    resize_height,
    resize_quality,
    image_bitmap_source,
    mime_type,
  } = ImageProcessArgs {
    width: args.width,
    height: args.height,
    sx: args.sx,
    sy: args.sy,
    sw: args.sw,
    sh: args.sh,
    image_orientation: args.image_orientation,
    premultiply_alpha: args.premultiply_alpha,
    predefined_color_space: args.predefined_color_space,
    color_space_conversion: args.color_space_conversion,
    resize_width: args.resize_width,
    resize_height: args.resize_height,
    resize_quality: args.resize_quality,
    image_bitmap_source: args.image_bitmap_source,
    mime_type: args.mime_type,
  };

  let (view, width, height) =
    decode_bitmap_data(buf, width, height, &image_bitmap_source, mime_type)?;

  #[rustfmt::skip]
  let source_rectangle: [[i32; 2]; 4] =
    if let (Some(sx), Some(sy), Some(sw), Some(sh)) = (sx, sy, sw, sh) {
    [
      [sx, sy],
      [sx + sw, sy],
      [sx + sw, sy + sh],
      [sx, sy + sh]
    ]
  } else {
    [
      [0, 0],
      [width as i32, 0],
      [width as i32, height as i32],
      [0, height as i32],
    ]
  };

  /*
   * The cropping works differently than the spec specifies:
   * The spec states to create an infinite surface and place the top-left corner
   * of the image a 0,0 and crop based on sourceRectangle.
   *
   * We instead create a surface the size of sourceRectangle, and position
   * the image at the correct location, which is the inverse of the x & y of
   * sourceRectangle's top-left corner.
   */
  let input_x = -(source_rectangle[0][0] as i64);
  let input_y = -(source_rectangle[0][1] as i64);

  let surface_width = (source_rectangle[1][0] - source_rectangle[0][0]) as u32;
  let surface_height = (source_rectangle[3][1] - source_rectangle[0][1]) as u32;

  let output_width = if let Some(resize_width) = resize_width {
    resize_width
  } else if let Some(resize_height) = resize_height {
    (surface_width * resize_height).div_ceil(surface_height)
  } else {
    surface_width
  };

  let output_height = if let Some(resize_height) = resize_height {
    resize_height
  } else if let Some(resize_width) = resize_width {
    (surface_height * resize_width).div_ceil(surface_width)
  } else {
    surface_height
  };

  let color = view.color();

  let surface = if !(width == surface_width
    && height == surface_height
    && input_x == 0
    && input_y == 0)
  {
    let mut surface = DynamicImage::new(surface_width, surface_height, color);
    overlay(&mut surface, &view, input_x, input_y);

    surface
  } else {
    view
  };

  let filter_type = match resize_quality {
    ImageResizeQuality::Pixelated => FilterType::Nearest,
    ImageResizeQuality::Low => FilterType::Triangle,
    ImageResizeQuality::Medium => FilterType::CatmullRom,
    ImageResizeQuality::High => FilterType::Lanczos3,
  };

  // should use resize_exact
  // https://github.com/image-rs/image/issues/1220#issuecomment-632060015
  let image_out =
    surface.resize_exact(output_width, output_height, filter_type);

  //
  // FIXME: It also need to fix about orientation when the spec is updated.
  //
  // > Multiple browser vendors discussed this a while back and (99% sure, from recollection)
  // > agreed to change createImageBitmap's behavior.
  // > The HTML spec should be updated to say:
  // > first EXIF orientation is applied, and then if imageOrientation is flipY, the image is flipped vertically
  // https://github.com/whatwg/html/issues/8085#issuecomment-2204696312
  let image_out = if image_orientation == ImageOrientation::FlipY {
    image_out.flipv()
  } else {
    image_out
  };

  // 9. TODO: Implement color space conversion.
  // Currently, the behavior of the color space conversion is always 'none' due to
  // the decoder always returning the sRGB bitmap data.
  // We need to apply ICC color profiles within the image from Blob,
  // or the parameter of 'settings.colorSpace' from ImageData.
  // https://github.com/whatwg/html/issues/10578
  // https://github.com/whatwg/html/issues/10577
  let image_out = match color_space_conversion {
    ColorSpaceConversion::Default => {
      match image_bitmap_source {
        ImageBitmapSource::Blob => {
          // If there is no color profile information, it will use sRGB.
          image_out
        }
        ImageBitmapSource::ImageData => match predefined_color_space {
          // If the color space is sRGB, return the image as is.
          PredefinedColorSpace::Srgb => image_out,
          PredefinedColorSpace::DisplayP3 => {
            apply_srgb_to_display_p3(&image_out)?
          }
        },
      }
    }
    ColorSpaceConversion::None => image_out,
  };

  // 10.
  if color.has_alpha() {
    match premultiply_alpha {
      // 1.
      PremultiplyAlpha::Default => { /* noop */ }

      // https://html.spec.whatwg.org/multipage/canvas.html#convert-from-premultiplied

      // 2.
      PremultiplyAlpha::Premultiply => {
        let result = apply_premultiply_alpha(&image_out)?;
        let data = to_js_buffer(&result);
        return Ok(ImageProcessResult {
          data,
          width: output_width,
          height: output_height,
        });
      }
      // 3.
      PremultiplyAlpha::None => {
        // NOTE: It's not clear how to handle the case of ImageData.
        // https://issues.chromium.org/issues/339759426
        // https://github.com/whatwg/html/issues/5365
        if image_bitmap_source == ImageBitmapSource::ImageData {
          return Ok(ImageProcessResult {
            data: image_out.clone().into_bytes().into(),
            width: output_width,
            height: output_height,
          });
        }

        let result = apply_unpremultiply_alpha(&image_out)?;
        let data = to_js_buffer(&result);
        return Ok(ImageProcessResult {
          data,
          width: output_width,
          height: output_height,
        });
      }
    }
  }

  Ok(ImageProcessResult {
    data: image_out.clone().into_bytes().into(),
    width: output_width,
    height: output_height,
  })
}

deno_core::extension!(
  deno_canvas,
  deps = [deno_webidl, deno_web, deno_webgpu],
  ops = [op_image_process],
  lazy_loaded_esm = ["01_image.js"],
);

pub fn get_declaration() -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("lib.deno_canvas.d.ts")
}
