// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use bytemuck::cast_slice;
use deno_core::error::AnyError;
use image::ColorType;
use image::DynamicImage;
use image::GenericImageView;
use image::ImageBuffer;
use image::Luma;
use image::LumaA;
use image::Pixel;
use image::Primitive;
use image::Rgb;
use image::Rgba;
use lcms2::PixelFormat;
use lcms2::Pod;
use lcms2::Profile;
use lcms2::Transform;
use num_traits::NumCast;
use num_traits::SaturatingMul;

pub(crate) trait PremultiplyAlpha {
  fn premultiply_alpha(&self) -> Self;
}

impl<T: Primitive> PremultiplyAlpha for LumaA<T> {
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

impl<T: Primitive> PremultiplyAlpha for Rgba<T> {
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

// make public if needed
fn process_premultiply_alpha<I, P, S>(image: &I) -> ImageBuffer<P, Vec<S>>
where
  I: GenericImageView<Pixel = P>,
  P: Pixel<Subpixel = S> + PremultiplyAlpha + 'static,
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

/// Premultiply the alpha channel of the image.
pub(crate) fn premultiply_alpha(
  image: DynamicImage,
  unmatch: Option<fn(ColorType) -> Result<DynamicImage, AnyError>>,
) -> Result<DynamicImage, AnyError> {
  let color = image.color();
  match color {
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
    x => match unmatch {
      Some(unmatch) => unmatch(x),
      None => Ok(image),
    },
  }
}

pub(crate) trait UnpremultiplyAlpha {
  /// To determine if the image is premultiplied alpha,
  /// checking premultiplied RGBA value is one where any of the R/G/B channel values exceeds the alpha channel value.\
  /// https://www.w3.org/TR/webgpu/#color-spaces
  fn is_premultiplied_alpha(&self) -> bool;
  fn unpremultiply_alpha(&self) -> Self;
}

impl<T: Primitive + SaturatingMul + Ord> UnpremultiplyAlpha for Rgba<T> {
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

impl<T: Primitive + SaturatingMul + Ord> UnpremultiplyAlpha for LumaA<T> {
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

// make public if needed
fn process_unpremultiply_alpha<I, P, S>(image: &I) -> ImageBuffer<P, Vec<S>>
where
  I: GenericImageView<Pixel = P>,
  P: Pixel<Subpixel = S> + UnpremultiplyAlpha + 'static,
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

/// Invert the premultiplied alpha channel of the image.
pub(crate) fn unpremultiply_alpha(
  image: DynamicImage,
  unmatch: Option<fn(ColorType) -> Result<DynamicImage, AnyError>>,
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
    x => match unmatch {
      Some(unmatch) => unmatch(x),
      None => Ok(image),
    },
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

fn apply_conversion_matrix_srgb_to_display_p3<T: Primitive>(
  r: T,
  g: T,
  b: T,
) -> (T, T, T) {
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
    [0.0193339, 0.119_192, 0.9503041],
  );

  let (r, g, b) = (
    r * m1x[0] + g * m1x[1] + b * m1x[2],
    r * m1y[0] + g * m1y[1] + b * m1y[2],
    r * m1z[0] + g * m1z[1] + b * m1z[2],
  );

  // inv[ P3-D65 (D65) to XYZ ]
  #[rustfmt::skip]
  let (m2x, m2y, m2z) = (
    [    2.493_497,  -0.931_383_6,  -0.402_710_8 ],
    [   -0.829_489,   1.762_664_1, 0.023_624_687 ],
    [ 0.035_845_83, -0.076_172_39,   0.956_884_5 ],
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

pub(crate) trait ColorSpaceConversion {
  /// Display P3 Color Encoding (v 1.0)  
  /// https://www.color.org/chardata/rgb/DisplayP3.xalter
  fn srgb_to_display_p3(&self) -> Self;
}

impl<T: Primitive> ColorSpaceConversion for Rgb<T> {
  fn srgb_to_display_p3(&self) -> Self {
    let (r, g, b) = (self.0[0], self.0[1], self.0[2]);

    let (r, g, b) = apply_conversion_matrix_srgb_to_display_p3(r, g, b);

    Rgb::<T>([r, g, b])
  }
}

impl<T: Primitive> ColorSpaceConversion for Rgba<T> {
  fn srgb_to_display_p3(&self) -> Self {
    let (r, g, b, a) = (self.0[0], self.0[1], self.0[2], self.0[3]);

    let (r, g, b) = apply_conversion_matrix_srgb_to_display_p3(r, g, b);

    Rgba::<T>([r, g, b, a])
  }
}

// make public if needed
fn process_srgb_to_display_p3<I, P, S>(image: &I) -> ImageBuffer<P, Vec<S>>
where
  I: GenericImageView<Pixel = P>,
  P: Pixel<Subpixel = S> + ColorSpaceConversion + 'static,
  S: Primitive + 'static,
{
  let (width, height) = image.dimensions();
  let mut out = ImageBuffer::new(width, height);

  for (x, y, pixel) in image.pixels() {
    let pixel = pixel.srgb_to_display_p3();

    out.put_pixel(x, y, pixel);
  }

  out
}

/// Convert the color space of the image from sRGB to Display-P3.
pub(crate) fn srgb_to_display_p3(
  image: DynamicImage,
  unmatch: Option<fn(ColorType) -> Result<DynamicImage, AnyError>>,
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
    x => match unmatch {
      Some(unmatch) => unmatch(x),
      None => Ok(image),
    },
  }
}

pub(crate) trait SliceToPixel {
  fn slice_to_pixel(pixel: &[u8]) -> Self;
}

impl<T: Primitive + Pod> SliceToPixel for Luma<T> {
  fn slice_to_pixel(pixel: &[u8]) -> Self {
    let pixel: &[T] = cast_slice(pixel);
    let pixel = [pixel[0]];

    Luma::<T>(pixel)
  }
}

impl<T: Primitive + Pod> SliceToPixel for LumaA<T> {
  fn slice_to_pixel(pixel: &[u8]) -> Self {
    let pixel: &[T] = cast_slice(pixel);
    let pixel = [pixel[0], pixel[1]];

    LumaA::<T>(pixel)
  }
}

impl<T: Primitive + Pod> SliceToPixel for Rgb<T> {
  fn slice_to_pixel(pixel: &[u8]) -> Self {
    let pixel: &[T] = cast_slice(pixel);
    let pixel = [pixel[0], pixel[1], pixel[2]];

    Rgb::<T>(pixel)
  }
}

impl<T: Primitive + Pod> SliceToPixel for Rgba<T> {
  fn slice_to_pixel(pixel: &[u8]) -> Self {
    let pixel: &[T] = cast_slice(pixel);
    let pixel = [pixel[0], pixel[1], pixel[2], pixel[3]];

    Rgba::<T>(pixel)
  }
}

/// Convert the pixel slice to an array to avoid the copy to Vec.  
/// I implemented this trait because of I couldn't find a way to effectively combine   
/// the `Transform` struct of `lcms2` and `Pixel` trait of `image`.  
/// If there is an implementation that is safer and can withstand changes, I would like to adopt it.
pub(crate) trait SliceToArray<const N: usize> {
  fn slice_to_array(pixel: &[u8]) -> [u8; N];
}

macro_rules! impl_slice_to_array {
  ($type:ty, $n:expr) => {
    impl<T: Primitive + Pod> SliceToArray<$n> for $type {
      fn slice_to_array(pixel: &[u8]) -> [u8; $n] {
        let mut dst = [0_u8; $n];
        dst.copy_from_slice(&pixel[..$n]);

        dst
      }
    }
  };
}

impl_slice_to_array!(Luma<T>, 1);
impl_slice_to_array!(Luma<T>, 2);
impl_slice_to_array!(LumaA<T>, 2);
impl_slice_to_array!(LumaA<T>, 4);
impl_slice_to_array!(Rgb<T>, 3);
impl_slice_to_array!(Rgb<T>, 6);
impl_slice_to_array!(Rgba<T>, 4);
impl_slice_to_array!(Rgba<T>, 8);

// make public if needed
fn process_icc_profile_conversion<P, S, const N: usize>(
  image: &DynamicImage,
  input_icc_profile: Profile,
  output_icc_profile: Profile,
) -> ImageBuffer<P, Vec<S>>
where
  P: Pixel<Subpixel = S> + SliceToPixel + SliceToArray<N> + 'static,
  S: Primitive + 'static,
{
  let (width, height) = image.dimensions();
  let mut out = ImageBuffer::new(width, height);
  let chunk_size = image.color().bytes_per_pixel() as usize;
  let pixel_iter = image
    .as_bytes()
    .chunks_exact(chunk_size)
    .zip(image.pixels());
  let pixel_format = match image.color() {
    ColorType::L8 => PixelFormat::GRAY_8,
    ColorType::L16 => PixelFormat::GRAY_16,
    ColorType::La8 => PixelFormat::GRAYA_8,
    ColorType::La16 => PixelFormat::GRAYA_16,
    ColorType::Rgb8 => PixelFormat::RGB_8,
    ColorType::Rgb16 => PixelFormat::RGB_16,
    ColorType::Rgba8 => PixelFormat::RGBA_8,
    ColorType::Rgba16 => PixelFormat::RGBA_16,
    // This arm usually doesn't reach, but it should be handled with returning the original image.
    _ => {
      return {
        for (pixel, (x, y, _)) in pixel_iter {
          out.put_pixel(x, y, P::slice_to_pixel(pixel));
        }
        out
      }
    }
  };
  let transformer = Transform::new(
    &input_icc_profile,
    pixel_format,
    &output_icc_profile,
    pixel_format,
    output_icc_profile.header_rendering_intent(),
  );

  for (pixel, (x, y, _)) in pixel_iter {
    let pixel = match transformer {
      Ok(ref transformer) => {
        let mut dst = P::slice_to_array(pixel);
        transformer.transform_in_place(&mut dst);

        dst
      }
      // This arm will reach when the ffi call fails.
      Err(_) => P::slice_to_array(pixel),
    };

    out.put_pixel(x, y, P::slice_to_pixel(&pixel));
  }

  out
}

#[rustfmt::skip]
/// Convert the color space of the image from the ICC profile to sRGB.
pub(crate) fn to_srgb_from_icc_profile(
  image: DynamicImage,
  icc_profile: Option<Vec<u8>>,
  unmatch: Option<fn(ColorType) -> Result<DynamicImage, AnyError>>,
) -> Result<DynamicImage, AnyError> {
  match icc_profile {
    // If there is no color profile information, return the image as is.
    None => Ok(image),
    Some(icc_profile) => match Profile::new_icc(&icc_profile) {
      // If the color profile information is invalid, return the image as is.
      Err(_) => Ok(image),
      Ok(icc_profile) => {
        let srgb_icc_profile = Profile::new_srgb();
        match image.color() {
          ColorType::L8 => {
            Ok(DynamicImage::ImageLuma8(process_icc_profile_conversion::<_,_,1>(&image,icc_profile,srgb_icc_profile)))
          }
          ColorType::L16 => {
            Ok(DynamicImage::ImageLuma16(process_icc_profile_conversion::<_,_,2>(&image,icc_profile,srgb_icc_profile)))
          }
          ColorType::La8 => {
            Ok(DynamicImage::ImageLumaA8(process_icc_profile_conversion::<_,_,2>(&image,icc_profile,srgb_icc_profile)))
          }
          ColorType::La16 => {
            Ok(DynamicImage::ImageLumaA16(process_icc_profile_conversion::<_, _, 4>(&image,icc_profile,srgb_icc_profile)))
          },
          ColorType::Rgb8 => {
            Ok(DynamicImage::ImageRgb8(process_icc_profile_conversion::<_,_,3>(&image,icc_profile,srgb_icc_profile)))
          }
          ColorType::Rgb16 => {
            Ok(DynamicImage::ImageRgb16(process_icc_profile_conversion::<_,_,6>(&image,icc_profile,srgb_icc_profile)))
          }
          ColorType::Rgba8 => {
            Ok(DynamicImage::ImageRgba8(process_icc_profile_conversion::<_,_,4>(&image,icc_profile,srgb_icc_profile)))
          }
          ColorType::Rgba16 => {
            Ok(DynamicImage::ImageRgba16(process_icc_profile_conversion::<_,_,8>(&image,icc_profile,srgb_icc_profile)))
          }
          x => match unmatch {
            Some(unmatch) => unmatch(x),
            None => Ok(image),
          },
        }
      }
    },
  }
}
