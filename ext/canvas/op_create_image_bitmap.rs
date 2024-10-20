// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::io::BufReader;
use std::io::Cursor;

use deno_core::error::type_error;
use deno_core::error::AnyError;
use deno_core::op2;
use deno_core::JsBuffer;
use deno_core::ToJsBuffer;
use image::codecs::bmp::BmpDecoder;
use image::codecs::gif::GifDecoder;
use image::codecs::ico::IcoDecoder;
use image::codecs::jpeg::JpegDecoder;
use image::codecs::png::PngDecoder;
use image::codecs::webp::WebPDecoder;
use image::imageops::overlay;
use image::imageops::FilterType;
use image::metadata::Orientation;
use image::DynamicImage;
use image::ImageDecoder;
use image::ImageError;
use image::RgbaImage;

use crate::error::image_error_message;
use crate::error::DOMExceptionInvalidStateError;
use crate::image_decoder::ImageDecoderFromReader;
use crate::image_decoder::ImageDecoderFromReaderType;
use crate::image_ops::premultiply_alpha as process_premultiply_alpha;
use crate::image_ops::to_srgb_from_icc_profile;
use crate::image_ops::unpremultiply_alpha;

#[derive(Debug, PartialEq)]
enum ImageBitmapSource {
  Blob,
  ImageData,
}

#[derive(Debug, PartialEq)]
enum ImageOrientation {
  FlipY,
  FromImage,
}

#[derive(Debug, PartialEq)]
enum PremultiplyAlpha {
  Default,
  Premultiply,
  None,
}

#[derive(Debug, PartialEq)]
enum ColorSpaceConversion {
  Default,
  None,
}

#[derive(Debug, PartialEq)]
enum ResizeQuality {
  Pixelated,
  Low,
  Medium,
  High,
}

#[derive(Debug, PartialEq)]
enum MimeType {
  NoMatch,
  Png,
  Jpeg,
  Gif,
  Bmp,
  Ico,
  Webp,
}

type DecodeBitmapDataReturn =
  (DynamicImage, u32, u32, Option<Orientation>, Option<Vec<u8>>);

fn decode_bitmap_data(
  buf: &[u8],
  width: u32,
  height: u32,
  image_bitmap_source: &ImageBitmapSource,
  mime_type: MimeType,
) -> Result<DecodeBitmapDataReturn, AnyError> {
  let (image, width, height, orientation, icc_profile) =
    match image_bitmap_source {
      ImageBitmapSource::Blob => {
        fn image_decoding_error(error: ImageError) -> AnyError {
          DOMExceptionInvalidStateError::new(&image_error_message(
            "decoding",
            &error.to_string(),
          ))
          .into()
        }
        let (image, orientation, icc_profile) = match mime_type {
          // Should we support the "image/apng" MIME type here?
          MimeType::Png => {
            let mut decoder: PngDecoder<ImageDecoderFromReaderType> =
              ImageDecoderFromReader::to_decoder(
                BufReader::new(Cursor::new(buf)),
                image_decoding_error,
              )?;
            let orientation = decoder.orientation()?;
            let icc_profile = decoder.icc_profile()?;
            (
              decoder.to_intermediate_image(image_decoding_error)?,
              orientation,
              icc_profile,
            )
          }
          MimeType::Jpeg => {
            let mut decoder: JpegDecoder<ImageDecoderFromReaderType> =
              ImageDecoderFromReader::to_decoder(
                BufReader::new(Cursor::new(buf)),
                image_decoding_error,
              )?;
            let orientation = decoder.orientation()?;
            let icc_profile = decoder.icc_profile()?;
            (
              decoder.to_intermediate_image(image_decoding_error)?,
              orientation,
              icc_profile,
            )
          }
          MimeType::Gif => {
            let mut decoder: GifDecoder<ImageDecoderFromReaderType> =
              ImageDecoderFromReader::to_decoder(
                BufReader::new(Cursor::new(buf)),
                image_decoding_error,
              )?;
            let orientation = decoder.orientation()?;
            let icc_profile = decoder.icc_profile()?;
            (
              decoder.to_intermediate_image(image_decoding_error)?,
              orientation,
              icc_profile,
            )
          }
          MimeType::Bmp => {
            let mut decoder: BmpDecoder<ImageDecoderFromReaderType> =
              ImageDecoderFromReader::to_decoder(
                BufReader::new(Cursor::new(buf)),
                image_decoding_error,
              )?;
            let orientation = decoder.orientation()?;
            let icc_profile = decoder.icc_profile()?;
            (
              decoder.to_intermediate_image(image_decoding_error)?,
              orientation,
              icc_profile,
            )
          }
          MimeType::Ico => {
            let mut decoder: IcoDecoder<ImageDecoderFromReaderType> =
              ImageDecoderFromReader::to_decoder(
                BufReader::new(Cursor::new(buf)),
                image_decoding_error,
              )?;
            let orientation = decoder.orientation()?;
            let icc_profile = decoder.icc_profile()?;
            (
              decoder.to_intermediate_image(image_decoding_error)?,
              orientation,
              icc_profile,
            )
          }
          MimeType::Webp => {
            let mut decoder: WebPDecoder<ImageDecoderFromReaderType> =
              ImageDecoderFromReader::to_decoder(
                BufReader::new(Cursor::new(buf)),
                image_decoding_error,
              )?;
            let orientation = decoder.orientation()?;
            let icc_profile = decoder.icc_profile()?;
            (
              decoder.to_intermediate_image(image_decoding_error)?,
              orientation,
              icc_profile,
            )
          }
          // This pattern is unreachable due to current block is already checked by the ImageBitmapSource above.
          MimeType::NoMatch => unreachable!(),
        };

        let width = image.width();
        let height = image.height();

        (image, width, height, Some(orientation), icc_profile)
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

        (image, width, height, None, None)
      }
    };

  Ok((image, width, height, orientation, icc_profile))
}

/// According to the spec, it's not clear how to handle the color space conversion.
///
/// Therefore, if you interpret the specification description from the implementation and wpt results, it will be as follows.
///
/// Let val be the value of the colorSpaceConversion member of options, and then run these substeps:
///  1. If val is "default", to convert to the sRGB color space.
///  2. If val is "none", to use the decoded image data as is.
///
/// related issue in whatwg
/// https://github.com/whatwg/html/issues/10578
///
/// reference in wpt  
/// https://github.com/web-platform-tests/wpt/blob/d575dc75ede770df322fbc5da3112dcf81f192ec/html/canvas/element/manual/imagebitmap/createImageBitmap-colorSpaceConversion.html#L18  
/// https://wpt.live/html/canvas/element/manual/imagebitmap/createImageBitmap-colorSpaceConversion.html
fn apply_color_space_conversion(
  image: DynamicImage,
  icc_profile: Option<Vec<u8>>,
  color_space_conversion: &ColorSpaceConversion,
) -> Result<DynamicImage, AnyError> {
  match color_space_conversion {
    // return the decoded image as is.
    ColorSpaceConversion::None => Ok(image),
    ColorSpaceConversion::Default => {
      to_srgb_from_icc_profile(image, icc_profile)
    }
  }
}

fn apply_premultiply_alpha(
  image: DynamicImage,
  image_bitmap_source: &ImageBitmapSource,
  premultiply_alpha: &PremultiplyAlpha,
) -> Result<DynamicImage, AnyError> {
  match premultiply_alpha {
    // 1.
    PremultiplyAlpha::Default => Ok(image),

    // https://html.spec.whatwg.org/multipage/canvas.html#convert-from-premultiplied

    // 2.
    PremultiplyAlpha::Premultiply => process_premultiply_alpha(image),
    // 3.
    PremultiplyAlpha::None => {
      // NOTE: It's not clear how to handle the case of ImageData.
      // https://issues.chromium.org/issues/339759426
      // https://github.com/whatwg/html/issues/5365
      if *image_bitmap_source == ImageBitmapSource::ImageData {
        return Ok(image);
      }

      unpremultiply_alpha(image)
    }
  }
}

#[derive(Debug, PartialEq)]
struct ParsedArgs {
  resize_width: Option<u32>,
  resize_height: Option<u32>,
  sx: Option<i32>,
  sy: Option<i32>,
  sw: Option<i32>,
  sh: Option<i32>,
  image_orientation: ImageOrientation,
  premultiply_alpha: PremultiplyAlpha,
  color_space_conversion: ColorSpaceConversion,
  resize_quality: ResizeQuality,
  image_bitmap_source: ImageBitmapSource,
  mime_type: MimeType,
}

#[allow(clippy::too_many_arguments)]
fn parse_args(
  sx: i32,
  sy: i32,
  sw: i32,
  sh: i32,
  image_orientation: u8,
  premultiply_alpha: u8,
  color_space_conversion: u8,
  resize_width: u32,
  resize_height: u32,
  resize_quality: u8,
  image_bitmap_source: u8,
  mime_type: u8,
) -> ParsedArgs {
  let resize_width = if resize_width == 0 {
    None
  } else {
    Some(resize_width)
  };
  let resize_height = if resize_height == 0 {
    None
  } else {
    Some(resize_height)
  };
  let sx = if sx == 0 { None } else { Some(sx) };
  let sy = if sy == 0 { None } else { Some(sy) };
  let sw = if sw == 0 { None } else { Some(sw) };
  let sh = if sh == 0 { None } else { Some(sh) };

  // Their unreachable wildcard patterns are validated in JavaScript-side.
  let image_orientation = match image_orientation {
    0 => ImageOrientation::FromImage,
    1 => ImageOrientation::FlipY,
    _ => unreachable!(),
  };
  let premultiply_alpha = match premultiply_alpha {
    0 => PremultiplyAlpha::Default,
    1 => PremultiplyAlpha::Premultiply,
    2 => PremultiplyAlpha::None,
    _ => unreachable!(),
  };
  let color_space_conversion = match color_space_conversion {
    0 => ColorSpaceConversion::Default,
    1 => ColorSpaceConversion::None,
    _ => unreachable!(),
  };
  let resize_quality = match resize_quality {
    0 => ResizeQuality::Low,
    1 => ResizeQuality::Pixelated,
    2 => ResizeQuality::Medium,
    3 => ResizeQuality::High,
    _ => unreachable!(),
  };
  let image_bitmap_source = match image_bitmap_source {
    0 => ImageBitmapSource::Blob,
    1 => ImageBitmapSource::ImageData,
    _ => unreachable!(),
  };
  let mime_type = match mime_type {
    0 => MimeType::NoMatch,
    1 => MimeType::Png,
    2 => MimeType::Jpeg,
    3 => MimeType::Gif,
    4 => MimeType::Bmp,
    5 => MimeType::Ico,
    6 => MimeType::Webp,
    _ => unreachable!(),
  };
  ParsedArgs {
    resize_width,
    resize_height,
    sx,
    sy,
    sw,
    sh,
    image_orientation,
    premultiply_alpha,
    color_space_conversion,
    resize_quality,
    image_bitmap_source,
    mime_type,
  }
}

#[op2]
#[serde]
#[allow(clippy::too_many_arguments)]
pub(super) fn op_create_image_bitmap(
  #[buffer] buf: JsBuffer,
  width: u32,
  height: u32,
  sx: i32,
  sy: i32,
  sw: i32,
  sh: i32,
  image_orientation: u8,
  premultiply_alpha: u8,
  color_space_conversion: u8,
  resize_width: u32,
  resize_height: u32,
  resize_quality: u8,
  image_bitmap_source: u8,
  mime_type: u8,
) -> Result<(ToJsBuffer, u32, u32), AnyError> {
  let ParsedArgs {
    resize_width,
    resize_height,
    sx,
    sy,
    sw,
    sh,
    image_orientation,
    premultiply_alpha,
    color_space_conversion,
    resize_quality,
    image_bitmap_source,
    mime_type,
  } = parse_args(
    sx,
    sy,
    sw,
    sh,
    image_orientation,
    premultiply_alpha,
    color_space_conversion,
    resize_width,
    resize_height,
    resize_quality,
    image_bitmap_source,
    mime_type,
  );

  // 6. Switch on image:
  let (image, width, height, orientation, icc_profile) =
    decode_bitmap_data(&buf, width, height, &image_bitmap_source, mime_type)?;

  // crop bitmap data
  // 2.
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

  // 3.
  let output_width = if let Some(resize_width) = resize_width {
    resize_width
  } else if let Some(resize_height) = resize_height {
    (surface_width * resize_height).div_ceil(surface_height)
  } else {
    surface_width
  };

  // 4.
  let output_height = if let Some(resize_height) = resize_height {
    resize_height
  } else if let Some(resize_width) = resize_width {
    (surface_height * resize_width).div_ceil(surface_width)
  } else {
    surface_height
  };

  // 5.
  let image = if !(width == surface_width
    && height == surface_height
    && input_x == 0
    && input_y == 0)
  {
    let mut surface =
      DynamicImage::new(surface_width, surface_height, image.color());
    overlay(&mut surface, &image, input_x, input_y);

    surface
  } else {
    image
  };

  // 7.
  let filter_type = match resize_quality {
    ResizeQuality::Pixelated => FilterType::Nearest,
    ResizeQuality::Low => FilterType::Triangle,
    ResizeQuality::Medium => FilterType::CatmullRom,
    ResizeQuality::High => FilterType::Lanczos3,
  };
  // should use resize_exact
  // https://github.com/image-rs/image/issues/1220#issuecomment-632060015
  let mut image = image.resize_exact(output_width, output_height, filter_type);

  // 8.
  let image = match image_bitmap_source {
    ImageBitmapSource::Blob => {
      // Note: According to browser behavior and wpt results, if Exif contains image orientation,
      // it applies the rotation from it before following the value of imageOrientation.
      // This is not stated in the spec but in MDN currently.
      // https://github.com/mdn/content/pull/34366

      // SAFETY: The orientation is always Some if the image is from a Blob.
      let orientation = orientation.unwrap();
      DynamicImage::apply_orientation(&mut image, orientation);

      match image_orientation {
        ImageOrientation::FlipY => image.flipv(),
        ImageOrientation::FromImage => image,
      }
    }
    ImageBitmapSource::ImageData => match image_orientation {
      ImageOrientation::FlipY => image.flipv(),
      ImageOrientation::FromImage => image,
    },
  };

  // 9.
  let image =
    apply_color_space_conversion(image, icc_profile, &color_space_conversion)?;

  // 10.
  let image =
    apply_premultiply_alpha(image, &image_bitmap_source, &premultiply_alpha)?;

  Ok((image.into_bytes().into(), output_width, output_height))
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_parse_args() {
    let parsed_args = parse_args(0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0);
    assert_eq!(
      parsed_args,
      ParsedArgs {
        resize_width: None,
        resize_height: None,
        sx: None,
        sy: None,
        sw: None,
        sh: None,
        image_orientation: ImageOrientation::FromImage,
        premultiply_alpha: PremultiplyAlpha::Default,
        color_space_conversion: ColorSpaceConversion::Default,
        resize_quality: ResizeQuality::Low,
        image_bitmap_source: ImageBitmapSource::Blob,
        mime_type: MimeType::NoMatch,
      }
    );
  }
}
