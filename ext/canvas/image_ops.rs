// Copyright 2018-2025 the Deno authors. MIT license.

use bytemuck::cast_slice;
use bytemuck::cast_slice_mut;
use image::imageops::overlay;
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

use crate::webidl::PredefinedColorSpace;
use crate::CanvasError;

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
      return LumaA([pixel[0], pixel[alpha_index]]);
    }

    for rgb in pixel.iter_mut().take(alpha_index) {
      *rgb = NumCast::from((rgb.to_f32().unwrap() * normalized_alpha).round())
        .unwrap()
    }

    LumaA([pixel[0], pixel[alpha_index]])
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
      return Rgba([pixel[0], pixel[1], pixel[2], pixel[alpha_index]]);
    }

    for rgb in pixel.iter_mut().take(alpha_index) {
      *rgb = NumCast::from((rgb.to_f32().unwrap() * normalized_alpha).round())
        .unwrap()
    }

    Rgba([pixel[0], pixel[1], pixel[2], pixel[alpha_index]])
  }
}

fn process_premultiply_alpha<I, P, S>(
  image: &I,
) -> Result<ImageBuffer<P, Vec<S>>, CanvasError>
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

  Ok(out)
}

/// Premultiply the alpha channel of the image.\
/// The operation will be skipped if the image is already premultiplied or has no alpha value.
pub fn premultiply_alpha(
  image: DynamicImage,
) -> Result<DynamicImage, CanvasError> {
  match image {
    DynamicImage::ImageLumaA8(image) => Ok(if is_premultiplied_alpha(&image) {
      image.into()
    } else {
      process_premultiply_alpha(&image)?.into()
    }),
    DynamicImage::ImageLumaA16(image) => {
      Ok(if is_premultiplied_alpha(&image) {
        image.into()
      } else {
        process_premultiply_alpha(&image)?.into()
      })
    }
    DynamicImage::ImageRgba8(image) => Ok(if is_premultiplied_alpha(&image) {
      image.into()
    } else {
      process_premultiply_alpha(&image)?.into()
    }),
    DynamicImage::ImageRgba16(image) => Ok(if is_premultiplied_alpha(&image) {
      image.into()
    } else {
      process_premultiply_alpha(&image)?.into()
    }),
    DynamicImage::ImageRgb32F(_) => {
      Err(CanvasError::UnsupportedColorType(image.color()))
    }
    DynamicImage::ImageRgba32F(_) => {
      Err(CanvasError::UnsupportedColorType(image.color()))
    }
    // If the image does not have an alpha channel, return the image as is.
    _ => Ok(image),
  }
}

pub(crate) trait UnpremultiplyAlpha {
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

    Rgba([pixel[0], pixel[1], pixel[2], pixel[alpha_index]])
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

    LumaA([pixel[0], pixel[alpha_index]])
  }
}

/// To determine if the image is premultiplied alpha,
/// checking premultiplied RGBA value is one where any of the R/G/B channel values exceeds the alpha channel value.\
/// https://www.w3.org/TR/webgpu/#color-spaces
fn is_premultiplied_alpha<I, P, S>(image: &I) -> bool
where
  I: GenericImageView<Pixel = P>,
  P: Pixel<Subpixel = S> + UnpremultiplyAlpha + 'static,
  S: Primitive + 'static,
{
  image
    .pixels()
    .any(|(_, _, pixel)| pixel.is_premultiplied_alpha())
}

fn process_unpremultiply_alpha<I, P, S>(image: &I) -> ImageBuffer<P, Vec<S>>
where
  I: GenericImageView<Pixel = P>,
  P: Pixel<Subpixel = S> + UnpremultiplyAlpha + 'static,
  S: Primitive + 'static,
{
  let (width, height) = image.dimensions();
  let mut out = ImageBuffer::new(width, height);

  for (x, y, pixel) in image.pixels() {
    let pixel = pixel.unpremultiply_alpha();

    out.put_pixel(x, y, pixel);
  }

  out
}

/// Invert the premultiplied alpha channel of the image.\
/// The operation will be skipped if the image is not premultiplied or has no alpha value.
pub(crate) fn unpremultiply_alpha(
  image: DynamicImage,
) -> Result<DynamicImage, CanvasError> {
  match image {
    DynamicImage::ImageLumaA8(image) => Ok(if is_premultiplied_alpha(&image) {
      process_unpremultiply_alpha(&image).into()
    } else {
      image.into()
    }),
    DynamicImage::ImageLumaA16(image) => {
      Ok(if is_premultiplied_alpha(&image) {
        process_unpremultiply_alpha(&image).into()
      } else {
        image.into()
      })
    }
    DynamicImage::ImageRgba8(image) => Ok(if is_premultiplied_alpha(&image) {
      process_unpremultiply_alpha(&image).into()
    } else {
      image.into()
    }),
    DynamicImage::ImageRgba16(image) => Ok(if is_premultiplied_alpha(&image) {
      process_unpremultiply_alpha(&image).into()
    } else {
      image.into()
    }),
    DynamicImage::ImageRgb32F(_) => {
      Err(CanvasError::UnsupportedColorType(image.color()))
    }
    DynamicImage::ImageRgba32F(_) => {
      Err(CanvasError::UnsupportedColorType(image.color()))
    }
    // If the image does not have an alpha channel, return the image as is.
    _ => Ok(image),
  }
}

pub(crate) trait SliceToPixel {
  fn slice_to_pixel(pixel: &[u8]) -> Self;
}

impl<T: Primitive + Pod> SliceToPixel for Luma<T> {
  fn slice_to_pixel(pixel: &[u8]) -> Self {
    let pixel: &[T] = cast_slice(pixel);
    let pixel = [pixel[0]];

    Luma(pixel)
  }
}

impl<T: Primitive + Pod> SliceToPixel for LumaA<T> {
  fn slice_to_pixel(pixel: &[u8]) -> Self {
    let pixel: &[T] = cast_slice(pixel);
    let pixel = [pixel[0], pixel[1]];

    LumaA(pixel)
  }
}

impl<T: Primitive + Pod> SliceToPixel for Rgb<T> {
  fn slice_to_pixel(pixel: &[u8]) -> Self {
    let pixel: &[T] = cast_slice(pixel);
    let pixel = [pixel[0], pixel[1], pixel[2]];

    Rgb(pixel)
  }
}

impl<T: Primitive + Pod> SliceToPixel for Rgba<T> {
  fn slice_to_pixel(pixel: &[u8]) -> Self {
    let pixel: &[T] = cast_slice(pixel);
    let pixel = [pixel[0], pixel[1], pixel[2], pixel[3]];

    Rgba(pixel)
  }
}

// reference
// https://www.w3.org/TR/css-color-4/#color-conversion-code
fn srgb_to_linear(value: f32) -> f32 {
  if value <= 0.04045 {
    value / 12.92
  } else {
    ((value + 0.055) / 1.055).powf(2.4)
  }
}

// same as sRGB
// https://www.w3.org/TR/css-color-4/#color-conversion-code
fn p3_to_linear(value: f32) -> f32 {
  srgb_to_linear(value)
}

// reference
// https://www.w3.org/TR/css-color-4/#color-conversion-code
fn linear_to_gamma_srgb(value: f32) -> f32 {
  if value <= 0.0031308 {
    value * 12.92
  } else {
    1.055 * value.powf(1.0 / 2.4) - 0.055
  }
}

// same as sRGB
// https://www.w3.org/TR/css-color-4/#color-conversion-code
fn linear_to_gamma_p3(value: f32) -> f32 {
  linear_to_gamma_srgb(value)
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

// Display P3 Color Encoding (v 1.0)
// https://www.color.org/chardata/rgb/DisplayP3.xalter

// See the following references for the conversion matrix.
// http://www.brucelindbloom.com/index.html?Eqn_RGB_XYZ_Matrix.html
// https://fujiwaratko.sakura.ne.jp/infosci/colorspace/colorspace2_e.html

// [ sRGB (D65) to XYZ ]
#[rustfmt::skip]
const SRGB_R65_TO_XYZ_MATRIX: ([f32; 3], [f32; 3], [f32; 3]) = (
  [0.4124564, 0.3575761, 0.1804375],
  [0.2126729, 0.7151522, 0.0721750],
  [0.0193339, 0.119_192, 0.9503041],
);

// inv[ P3-D65 (D65) to XYZ ]
#[rustfmt::skip]
const INV_P3_R65_TO_XYZ_MATRIX: ([f32; 3], [f32; 3], [f32; 3]) = (
  [   2.493_497,  -0.931_383_6,  -0.402_710_8],
  [  -0.829_489,   1.762_664_1, 0.023_624_687],
  [0.035_845_83, -0.076_172_39,   0.956_884_5],
);

// [ P3 (D65) to XYZ ]
#[rustfmt::skip]
const P3_R65_TO_XYZ_MATRIX: ([f32; 3], [f32; 3], [f32; 3]) = (
  [0.486571, 0.265668, 0.198217],
  [0.228975, 0.691739, 0.079287],
  [0.000000, 0.045113, 1.043944],
);

// inv[ sRGB (D65) to XYZ ]
#[rustfmt::skip]
const INV_SRGB_R65_TO_XYZ_MATRIX: ([f32; 3], [f32; 3], [f32; 3]) = (
  [ 3.240_97, -1.537383, -0.498611],
  [-0.969244,  1.875968,  0.041555],
  [ 0.055630, -0.203977,  1.056972],
);

fn transform_rgb_color_space_from_parameters<T: Primitive>(
  r: T,
  g: T,
  b: T,
  to_linear_fn: fn(f32) -> f32,
  input_color_transform_matrix: ([f32; 3], [f32; 3], [f32; 3]),
  output_color_transform_inv_matrix: ([f32; 3], [f32; 3], [f32; 3]),
  to_gamma_fn: fn(f32) -> f32,
) -> (T, T, T) {
  // normalize the value to 0.0 - 1.0
  let (r, g, b) = (
    normalize_value_to_0_1(r),
    normalize_value_to_0_1(g),
    normalize_value_to_0_1(b),
  );
  let (r, g, b) = (to_linear_fn(r), to_linear_fn(g), to_linear_fn(b));
  // input color space (RGB) -> input color space (XYZ)
  //
  // inv[ output color space to XYZ ] * [ input color space to XYZ ]
  let (m1x, m1y, m1z) = input_color_transform_matrix;
  let (r, g, b) = (
    r * m1x[0] + g * m1x[1] + b * m1x[2],
    r * m1y[0] + g * m1y[1] + b * m1y[2],
    r * m1z[0] + g * m1z[1] + b * m1z[2],
  );
  let (m2x, m2y, m2z) = output_color_transform_inv_matrix;
  let (r, g, b) = (
    r * m2x[0] + g * m2x[1] + b * m2x[2],
    r * m2y[0] + g * m2y[1] + b * m2y[2],
    r * m2z[0] + g * m2z[1] + b * m2z[2],
  );
  // output color space (Linear) -> output color space (Gamma)
  let (r, g, b) = (to_gamma_fn(r), to_gamma_fn(g), to_gamma_fn(b));
  // unnormalize the value from 0.0 - 1.0
  (
    unnormalize_value_from_0_1(r),
    unnormalize_value_from_0_1(g),
    unnormalize_value_from_0_1(b),
  )
}

trait TransformRgbColorSpace {
  fn transform_rgb_color_space(
    &self,
    to_linear_fn: fn(f32) -> f32,
    input_color_transform_matrix: ([f32; 3], [f32; 3], [f32; 3]),
    output_color_transform_inv_matrix: ([f32; 3], [f32; 3], [f32; 3]),
    to_gamma_fn: fn(f32) -> f32,
  ) -> Self;
}

impl<T: Primitive> TransformRgbColorSpace for Rgb<T> {
  fn transform_rgb_color_space(
    &self,
    to_linear_fn: fn(f32) -> f32,
    input_color_transform_matrix: ([f32; 3], [f32; 3], [f32; 3]),
    output_color_transform_inv_matrix: ([f32; 3], [f32; 3], [f32; 3]),
    to_gamma_fn: fn(f32) -> f32,
  ) -> Self {
    let (r, g, b) = (self.0[0], self.0[1], self.0[2]);

    let (r, g, b) = transform_rgb_color_space_from_parameters(
      r,
      g,
      b,
      to_linear_fn,
      input_color_transform_matrix,
      output_color_transform_inv_matrix,
      to_gamma_fn,
    );

    Rgb([r, g, b])
  }
}

impl<T: Primitive> TransformRgbColorSpace for Rgba<T> {
  fn transform_rgb_color_space(
    &self,
    to_linear_fn: fn(f32) -> f32,
    input_color_transform_matrix: ([f32; 3], [f32; 3], [f32; 3]),
    output_color_transform_inv_matrix: ([f32; 3], [f32; 3], [f32; 3]),
    to_gamma_fn: fn(f32) -> f32,
  ) -> Self {
    let (r, g, b, a) = (self.0[0], self.0[1], self.0[2], self.0[3]);

    let (r, g, b) = transform_rgb_color_space_from_parameters(
      r,
      g,
      b,
      to_linear_fn,
      input_color_transform_matrix,
      output_color_transform_inv_matrix,
      to_gamma_fn,
    );

    Rgba([r, g, b, a])
  }
}

fn process_transform_rgb_color_space<I, P, S>(
  image: &I,
  to_linear_fn: fn(f32) -> f32,
  input_color_transform_matrix: ([f32; 3], [f32; 3], [f32; 3]),
  output_color_transform_inv_matrix: ([f32; 3], [f32; 3], [f32; 3]),
  to_gamma_fn: fn(f32) -> f32,
) -> Result<ImageBuffer<P, Vec<S>>, CanvasError>
where
  I: GenericImageView<Pixel = P>,
  P: Pixel<Subpixel = S> + TransformRgbColorSpace + 'static,
  S: Primitive + 'static,
{
  let (width, height) = image.dimensions();
  let mut out = ImageBuffer::new(width, height);

  for (x, y, pixel) in image.pixels() {
    let pixel = pixel.transform_rgb_color_space(
      to_linear_fn,
      input_color_transform_matrix,
      output_color_transform_inv_matrix,
      to_gamma_fn,
    );

    out.put_pixel(x, y, pixel);
  }

  Ok(out)
}

/// Transform the color space of the image from input to output.
/// # Arguments
///
/// * `image`
/// * `input_color_space` - the color space of the input
/// * `output_color_space` - the color space of the output
pub fn transform_rgb_color_space(
  image: DynamicImage,
  input_color_space: PredefinedColorSpace,
  output_color_space: PredefinedColorSpace,
) -> Result<DynamicImage, CanvasError> {
  type Parameters = (
    fn(f32) -> f32,
    ([f32; 3], [f32; 3], [f32; 3]),
    ([f32; 3], [f32; 3], [f32; 3]),
    fn(f32) -> f32,
  );
  let (
    to_linear_fn,
    input_color_transform_matrix,
    output_color_transform_inv_matrix,
    to_gamma_fn,
  ): Parameters = match (input_color_space, output_color_space) {
    // if the color space is the same, return the image as is
    (PredefinedColorSpace::Srgb, PredefinedColorSpace::Srgb)
    | (PredefinedColorSpace::DisplayP3, PredefinedColorSpace::DisplayP3) => {
      return Ok(image);
    }
    (PredefinedColorSpace::Srgb, PredefinedColorSpace::DisplayP3) => (
      srgb_to_linear,
      SRGB_R65_TO_XYZ_MATRIX,
      INV_P3_R65_TO_XYZ_MATRIX,
      linear_to_gamma_p3,
    ),
    (PredefinedColorSpace::DisplayP3, PredefinedColorSpace::Srgb) => (
      p3_to_linear,
      P3_R65_TO_XYZ_MATRIX,
      INV_SRGB_R65_TO_XYZ_MATRIX,
      linear_to_gamma_srgb,
    ),
  };
  match image {
    // The color space conversion of the gray scale color types is meaningless
    // due to the lack of color information.
    DynamicImage::ImageLuma8(_)
    | DynamicImage::ImageLuma16(_)
    | DynamicImage::ImageLumaA8(_)
    | DynamicImage::ImageLumaA16(_) => Ok(image),
    DynamicImage::ImageRgb8(image) => Ok(
      process_transform_rgb_color_space(
        &image,
        to_linear_fn,
        input_color_transform_matrix,
        output_color_transform_inv_matrix,
        to_gamma_fn,
      )?
      .into(),
    ),
    DynamicImage::ImageRgb16(image) => Ok(
      process_transform_rgb_color_space(
        &image,
        to_linear_fn,
        input_color_transform_matrix,
        output_color_transform_inv_matrix,
        to_gamma_fn,
      )?
      .into(),
    ),
    DynamicImage::ImageRgba8(image) => Ok(
      process_transform_rgb_color_space(
        &image,
        to_linear_fn,
        input_color_transform_matrix,
        output_color_transform_inv_matrix,
        to_gamma_fn,
      )?
      .into(),
    ),
    DynamicImage::ImageRgba16(image) => Ok(
      process_transform_rgb_color_space(
        &image,
        to_linear_fn,
        input_color_transform_matrix,
        output_color_transform_inv_matrix,
        to_gamma_fn,
      )?
      .into(),
    ),
    DynamicImage::ImageRgb32F(_) => {
      Err(CanvasError::UnsupportedColorType(image.color()))
    }
    DynamicImage::ImageRgba32F(_) => {
      Err(CanvasError::UnsupportedColorType(image.color()))
    }
    _ => Err(CanvasError::UnsupportedColorType(image.color())),
  }
}

pub(crate) trait TransformColorProfile {
  fn transform_color_profile<P, S>(
    &mut self,
    transformer: &Transform<u8, u8>,
  ) -> P
  where
    P: Pixel<Subpixel = S> + SliceToPixel + 'static,
    S: Primitive + 'static;
}

macro_rules! impl_transform_color_profile {
  ($type:ty) => {
    impl TransformColorProfile for $type {
      fn transform_color_profile<P, S>(
        &mut self,
        transformer: &Transform<u8, u8>,
      ) -> P
      where
        P: Pixel<Subpixel = S> + SliceToPixel + 'static,
        S: Primitive + 'static,
      {
        let mut pixel = cast_slice_mut(self.0.as_mut_slice());
        transformer.transform_in_place(&mut pixel);

        P::slice_to_pixel(&pixel)
      }
    }
  };
}

impl_transform_color_profile!(Luma<u8>);
impl_transform_color_profile!(Luma<u16>);
impl_transform_color_profile!(LumaA<u8>);
impl_transform_color_profile!(LumaA<u16>);
impl_transform_color_profile!(Rgb<u8>);
impl_transform_color_profile!(Rgb<u16>);
impl_transform_color_profile!(Rgba<u8>);
impl_transform_color_profile!(Rgba<u16>);

fn process_transform_color_space_from_icc_profile<I, P, S>(
  image: &I,
  color: ColorType,
  input_icc_profile: Profile,
  output_icc_profile: Profile,
) -> Result<ImageBuffer<P, Vec<S>>, CanvasError>
where
  I: GenericImageView<Pixel = P>,
  P: Pixel<Subpixel = S> + SliceToPixel + TransformColorProfile + 'static,
  S: Primitive + 'static,
{
  let (width, height) = image.dimensions();
  let mut out = ImageBuffer::new(width, height);
  let pixel_format = match color {
    ColorType::L8 => Ok(PixelFormat::GRAY_8),
    ColorType::L16 => Ok(PixelFormat::GRAY_16),
    ColorType::La8 => Ok(PixelFormat::GRAYA_8),
    ColorType::La16 => Ok(PixelFormat::GRAYA_16),
    ColorType::Rgb8 => Ok(PixelFormat::RGB_8),
    ColorType::Rgb16 => Ok(PixelFormat::RGB_16),
    ColorType::Rgba8 => Ok(PixelFormat::RGBA_8),
    ColorType::Rgba16 => Ok(PixelFormat::RGBA_16),
    _ => Err(CanvasError::UnsupportedColorType(color)),
  }?;
  let transformer = Transform::new(
    &input_icc_profile,
    pixel_format,
    &output_icc_profile,
    pixel_format,
    output_icc_profile.header_rendering_intent(),
  )
  .map_err(CanvasError::Lcms)?;

  for (x, y, mut pixel) in image.pixels() {
    let pixel = pixel.transform_color_profile(&transformer);

    out.put_pixel(x, y, pixel);
  }

  Ok(out)
}

/// Transofrm the color space of the image from the ICC profile to sRGB.
pub(crate) fn transform_color_space_from_icc_profile_to_srgb(
  image: DynamicImage,
  icc_profile: Option<Vec<u8>>,
) -> Result<DynamicImage, CanvasError> {
  match icc_profile {
    // If there is no color profile information, return the image as is.
    None => Ok(image),
    Some(icc_profile) => match Profile::new_icc(&icc_profile) {
      // If the color profile information is invalid, return the image as is.
      Err(_) => Ok(image),
      Ok(icc_profile) => {
        let srgb_icc_profile = Profile::new_srgb();
        let color = image.color();
        match image {
          // The color space conversion of the gray scale color types to the sRGB is meaningless due to the lack of color information.
          DynamicImage::ImageLuma8(_)
          | DynamicImage::ImageLuma16(_)
          | DynamicImage::ImageLumaA8(_)
          | DynamicImage::ImageLumaA16(_) => Ok(image),
          DynamicImage::ImageRgb8(image) => Ok(
            process_transform_color_space_from_icc_profile(
              &image,
              color,
              icc_profile,
              srgb_icc_profile,
            )?
            .into(),
          ),
          DynamicImage::ImageRgb16(image) => Ok(
            process_transform_color_space_from_icc_profile(
              &image,
              color,
              icc_profile,
              srgb_icc_profile,
            )?
            .into(),
          ),
          DynamicImage::ImageRgba8(image) => Ok(
            process_transform_color_space_from_icc_profile(
              &image,
              color,
              icc_profile,
              srgb_icc_profile,
            )?
            .into(),
          ),
          DynamicImage::ImageRgba16(image) => Ok(
            process_transform_color_space_from_icc_profile(
              &image,
              color,
              icc_profile,
              srgb_icc_profile,
            )?
            .into(),
          ),
          DynamicImage::ImageRgb32F(_) => {
            Err(CanvasError::UnsupportedColorType(image.color()))
          }
          DynamicImage::ImageRgba32F(_) => {
            Err(CanvasError::UnsupportedColorType(image.color()))
          }
          _ => Err(CanvasError::UnsupportedColorType(image.color())),
        }
      }
    },
  }
}

/// Create an image buffer from raw bytes.
fn process_image_buffer_from_raw_bytes<P, S>(
  width: u32,
  height: u32,
  buffer: &[u8],
  bytes_per_pixel: usize,
) -> ImageBuffer<P, Vec<S>>
where
  P: Pixel<Subpixel = S> + SliceToPixel + 'static,
  S: Primitive + 'static,
{
  let mut out = ImageBuffer::new(width, height);
  for (index, buffer) in buffer.chunks_exact(bytes_per_pixel).enumerate() {
    let pixel = P::slice_to_pixel(buffer);

    out.put_pixel(index as u32, index as u32, pixel);
  }

  out
}

pub(crate) fn create_image_from_raw_bytes(
  width: u32,
  height: u32,
  buffer: &[u8],
) -> Result<DynamicImage, CanvasError> {
  let total_pixels = (width * height) as usize;
  // avoid to divide by zero
  let bytes_per_pixel = buffer
    .len()
    .checked_div(total_pixels)
    .ok_or(CanvasError::InvalidSizeZero(width, height))?;
  // convert from a bytes per pixel to the color type of the image
  // https://github.com/image-rs/image/blob/2c986d353333d2604f0c3f1fcef262cc763c0001/src/color.rs#L38-L49
  match bytes_per_pixel {
    1 => Ok(DynamicImage::ImageLuma8(
      process_image_buffer_from_raw_bytes(
        width,
        height,
        buffer,
        bytes_per_pixel,
      ),
    )),
    2 => Ok(
      // NOTE: ImageLumaA8 is also the same bytes per pixel.
      DynamicImage::ImageLuma16(process_image_buffer_from_raw_bytes(
        width,
        height,
        buffer,
        bytes_per_pixel,
      )),
    ),
    3 => Ok(DynamicImage::ImageRgb8(
      process_image_buffer_from_raw_bytes(
        width,
        height,
        buffer,
        bytes_per_pixel,
      ),
    )),
    4 => Ok(
      // NOTE: ImageLumaA16 is also the same bytes per pixel.
      DynamicImage::ImageRgba8(process_image_buffer_from_raw_bytes(
        width,
        height,
        buffer,
        bytes_per_pixel,
      )),
    ),
    6 => Ok(DynamicImage::ImageRgb16(
      process_image_buffer_from_raw_bytes(
        width,
        height,
        buffer,
        bytes_per_pixel,
      ),
    )),
    8 => Ok(DynamicImage::ImageRgba16(
      process_image_buffer_from_raw_bytes(
        width,
        height,
        buffer,
        bytes_per_pixel,
      ),
    )),
    12 => Err(CanvasError::UnsupportedColorType(ColorType::Rgb32F)),
    16 => Err(CanvasError::UnsupportedColorType(ColorType::Rgba32F)),
    _ => Err(CanvasError::UnsupportedColorType(ColorType::L8)),
  }
}

/// Crop the image
pub fn crop(
  image: DynamicImage,
  x_start: u32,
  y_start: u32,
  copy_width: u32,
  copy_height: u32,
) -> DynamicImage {
  // it's slow to use crop_imm?
  // https://github.com/image-rs/image/issues/2295
  let mut new_image = DynamicImage::new(copy_width, copy_height, image.color());
  overlay(&mut new_image, &image, x_start.into(), y_start.into());

  new_image
}

#[cfg(test)]
mod tests {
  use image::Rgba;

  use super::*;

  #[test]
  fn test_premultiply_alpha() {
    let rgba = Rgba::<u8>([255, 128, 0, 128]);
    let rgba = rgba.premultiply_alpha();
    assert_eq!(rgba, Rgba::<u8>([128, 64, 0, 128]));

    let rgba = Rgba::<u8>([255, 255, 255, 255]);
    let rgba = rgba.premultiply_alpha();
    assert_eq!(rgba, Rgba::<u8>([255, 255, 255, 255]));
  }

  #[test]
  fn test_unpremultiply_alpha() {
    let rgba = Rgba::<u8>([127, 0, 0, 127]);
    let rgba = rgba.unpremultiply_alpha();
    assert_eq!(rgba, Rgba::<u8>([255, 0, 0, 127]));
  }

  #[test]
  fn test_process_image_buffer_from_raw_bytes() {
    let buffer = &[255, 255, 0, 0, 0, 0, 255, 255];
    let color = ColorType::Rgba16;
    let bytes_per_pixel = color.bytes_per_pixel() as usize;
    let image = DynamicImage::ImageRgba16(process_image_buffer_from_raw_bytes(
      1,
      1,
      buffer,
      bytes_per_pixel,
    ))
    .to_rgba16();
    assert_eq!(image.get_pixel(0, 0), &Rgba::<u16>([65535, 0, 0, 65535]));
  }

  #[test]
  fn test_transform_rgb_color_space_from_parameters() {
    // sRGB -> Display-P3
    let srgb_to_p3 = (
      srgb_to_linear,
      SRGB_R65_TO_XYZ_MATRIX,
      INV_P3_R65_TO_XYZ_MATRIX,
      linear_to_gamma_p3,
    );
    // Display-P3 -> sRGB
    let p3_to_srgb = (
      p3_to_linear,
      P3_R65_TO_XYZ_MATRIX,
      INV_SRGB_R65_TO_XYZ_MATRIX,
      linear_to_gamma_srgb,
    );

    // lossless conversion from (255,0,0) to p3 and back to srgb
    {
      // sRGB -> Display-P3
      let (r1, g1, b1) = (255_u8, 0, 0);
      let (r2, g2, b2) = transform_rgb_color_space_from_parameters(
        255_u8,
        0,
        0,
        srgb_to_p3.0,
        srgb_to_p3.1,
        srgb_to_p3.2,
        srgb_to_p3.3,
      );
      assert_eq!((r2, g2, b2), (234, 51, 35));

      // Display-P3 -> sRGB
      let (r3, g3, b3) = transform_rgb_color_space_from_parameters(
        r2,
        g2,
        b2,
        p3_to_srgb.0,
        p3_to_srgb.1,
        p3_to_srgb.2,
        p3_to_srgb.3,
      );
      assert_eq!((r3, g3, b3), (r1, g1, b1));
    }

    // lossless conversion from (0,255,0) to p3 and back to srgb
    {
      // sRGB -> Display-P3
      let (_r1, g1, b1) = (0_u8, 255, 0);
      let (r2, g2, b2) = transform_rgb_color_space_from_parameters(
        0_u8,
        255,
        0,
        srgb_to_p3.0,
        srgb_to_p3.1,
        srgb_to_p3.2,
        srgb_to_p3.3,
      );
      assert_eq!((r2, g2, b2), (117, 251, 76));

      // Display-P3 -> sRGB
      let (r3, g3, b3) = transform_rgb_color_space_from_parameters(
        r2,
        g2,
        b2,
        p3_to_srgb.0,
        p3_to_srgb.1,
        p3_to_srgb.2,
        p3_to_srgb.3,
      );
      // is it an error the _r1 not matches to r3?
      assert_eq!((r3, g3, b3), (3, g1, b1));
    }

    // lossless conversion from (0,0,255) to p3 and back to srgb
    {
      // sRGB -> Display-P3
      let (r1, g1, b1) = (0_u8, 0, 255);
      let (r2, g2, b2) = transform_rgb_color_space_from_parameters(
        0_u8,
        0,
        255,
        srgb_to_p3.0,
        srgb_to_p3.1,
        srgb_to_p3.2,
        srgb_to_p3.3,
      );
      assert_eq!((r2, g2, b2), (0, 0, 245));

      // Display-P3 -> sRGB
      let (r3, g3, b3) = transform_rgb_color_space_from_parameters(
        r2,
        g2,
        b2,
        p3_to_srgb.0,
        p3_to_srgb.1,
        p3_to_srgb.2,
        p3_to_srgb.3,
      );
      assert_eq!((r3, g3, b3), (r1, g1, b1));
    }
  }
}
