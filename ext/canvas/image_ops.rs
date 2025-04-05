// Copyright 2018-2025 the Deno authors. MIT license.

use bytemuck::cast_slice;
use bytemuck::cast_slice_mut;
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
) -> Result<DynamicImage, CanvasError> {
  match image {
    DynamicImage::ImageLumaA8(image) => {
      Ok(process_premultiply_alpha(&image).into())
    }
    DynamicImage::ImageLumaA16(image) => {
      Ok(process_premultiply_alpha(&image).into())
    }
    DynamicImage::ImageRgba8(image) => {
      Ok(process_premultiply_alpha(&image).into())
    }
    DynamicImage::ImageRgba16(image) => {
      Ok(process_premultiply_alpha(&image).into())
    }
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

    // avoid to divide by zero
    if alpha.to_f32().unwrap() == 0.0 {
      return Rgba([pixel[0], pixel[1], pixel[2], pixel[alpha_index]]);
    }

    for rgb in pixel.iter_mut().take(alpha_index) {
      let unchecked_value = (rgb.to_f32().unwrap()
        / (alpha.to_f32().unwrap() / max_t.to_f32().unwrap()))
      .round();
      let checked_value = if unchecked_value > max_t.to_f32().unwrap() {
        max_t.to_f32().unwrap()
      } else {
        unchecked_value
      };

      *rgb = NumCast::from(checked_value).unwrap();
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

    // avoid to divide by zero
    if alpha.to_f32().unwrap() == 0.0 {
      return LumaA([pixel[0], pixel[alpha_index]]);
    }

    for rgb in pixel.iter_mut().take(alpha_index) {
      let unchecked_value = (rgb.to_f32().unwrap()
        / (alpha.to_f32().unwrap() / max_t.to_f32().unwrap()))
      .round();
      let checked_value = if unchecked_value > max_t.to_f32().unwrap() {
        max_t.to_f32().unwrap()
      } else {
        unchecked_value
      };

      *rgb = NumCast::from(checked_value).unwrap();
    }

    LumaA([pixel[0], pixel[alpha_index]])
  }
}

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

/// Invert the premultiplied alpha channel of the image.
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

fn process_icc_profile_conversion<I, P, S>(
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

/// Convert the color space of the image from the ICC profile to sRGB.
pub(crate) fn to_srgb_from_icc_profile(
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
          DynamicImage::ImageLuma8(image) => Ok(
            process_icc_profile_conversion(
              &image,
              color,
              icc_profile,
              srgb_icc_profile,
            )?
            .into(),
          ),
          DynamicImage::ImageLuma16(image) => Ok(
            process_icc_profile_conversion(
              &image,
              color,
              icc_profile,
              srgb_icc_profile,
            )?
            .into(),
          ),
          DynamicImage::ImageLumaA8(image) => Ok(
            process_icc_profile_conversion(
              &image,
              color,
              icc_profile,
              srgb_icc_profile,
            )?
            .into(),
          ),
          DynamicImage::ImageLumaA16(image) => Ok(
            process_icc_profile_conversion(
              &image,
              color,
              icc_profile,
              srgb_icc_profile,
            )?
            .into(),
          ),
          DynamicImage::ImageRgb8(image) => Ok(
            process_icc_profile_conversion(
              &image,
              color,
              icc_profile,
              srgb_icc_profile,
            )?
            .into(),
          ),
          DynamicImage::ImageRgb16(image) => Ok(
            process_icc_profile_conversion(
              &image,
              color,
              icc_profile,
              srgb_icc_profile,
            )?
            .into(),
          ),
          DynamicImage::ImageRgba8(image) => Ok(
            process_icc_profile_conversion(
              &image,
              color,
              icc_profile,
              srgb_icc_profile,
            )?
            .into(),
          ),
          DynamicImage::ImageRgba16(image) => Ok(
            process_icc_profile_conversion(
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

    // https://github.com/denoland/deno/issues/28732
    let rgba = Rgba::<u8>([247, 0, 0, 233]);
    let rgba = rgba.unpremultiply_alpha();
    assert_eq!(rgba, Rgba::<u8>([255, 0, 0, 233]));

    let rgba = Rgba::<u8>([255, 0, 0, 0]);
    let rgba = rgba.unpremultiply_alpha();
    assert_eq!(rgba, Rgba::<u8>([255, 0, 0, 0]));
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
}
