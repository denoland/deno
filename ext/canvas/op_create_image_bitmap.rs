// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::io::BufReader;
use std::io::Cursor;

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
use image::ImageError;
use image::RgbaImage;
use serde::Deserialize;
use serde::Serialize;

use crate::error::image_error_message;
use crate::error::DOMExceptionInvalidStateError;
use crate::image_decoder::ImageDecoderFromReader;
use crate::image_decoder::ImageDecoderFromReaderType;
use crate::image_ops::premultiply_alpha as process_premultiply_alpha;
use crate::image_ops::to_srgb_from_icc_profile;
use crate::image_ops::unpremultiply_alpha;

#[derive(Debug, Deserialize, PartialEq)]
// Follow the cases defined in the spec
enum ImageBitmapSource {
  Blob,
  ImageData,
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
enum ImageOrientation {
  FlipY,
  #[serde(rename = "from-image")]
  FromImage,
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
enum PremultiplyAlpha {
  Default,
  Premultiply,
  None,
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
enum ColorSpaceConversion {
  Default,
  None,
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
enum ResizeQuality {
  Pixelated,
  Low,
  Medium,
  High,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OpCreateImageBitmapArgs {
  width: u32,
  height: u32,
  sx: Option<i32>,
  sy: Option<i32>,
  sw: Option<i32>,
  sh: Option<i32>,
  image_orientation: ImageOrientation,
  premultiply_alpha: PremultiplyAlpha,
  color_space_conversion: ColorSpaceConversion,
  resize_width: Option<u32>,
  resize_height: Option<u32>,
  resize_quality: ResizeQuality,
  image_bitmap_source: ImageBitmapSource,
  mime_type: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct OpCreateImageBitmapReturn {
  data: ToJsBuffer,
  width: u32,
  height: u32,
}

type DecodeBitmapDataReturn = (DynamicImage, u32, u32, Option<Vec<u8>>);

fn decode_bitmap_data(
  buf: &[u8],
  width: u32,
  height: u32,
  image_bitmap_source: &ImageBitmapSource,
  mime_type: String,
) -> Result<DecodeBitmapDataReturn, AnyError> {
  let (image, width, height, icc_profile) = match image_bitmap_source {
    ImageBitmapSource::Blob => {
      fn image_decoding_error(error: ImageError) -> AnyError {
        DOMExceptionInvalidStateError::new(&image_error_message(
          "decoding",
          &error.to_string(),
        ))
        .into()
      }
      let (image, icc_profile) = match &*mime_type {
        // Should we support the "image/apng" MIME type here?
        "image/png" => {
          let mut decoder: PngDecoder<ImageDecoderFromReaderType> =
            ImageDecoderFromReader::to_decoder(BufReader::new(Cursor::new(
              buf,
            )), image_decoding_error)?;
          let icc_profile = decoder.get_icc_profile();
          (decoder.to_intermediate_image(image_decoding_error)?, icc_profile)
        }
        "image/jpeg" => {
          let mut decoder: JpegDecoder<ImageDecoderFromReaderType> =
            ImageDecoderFromReader::to_decoder(BufReader::new(Cursor::new(
              buf,
            )), image_decoding_error)?;
          let icc_profile = decoder.get_icc_profile();
          (decoder.to_intermediate_image(image_decoding_error)?, icc_profile)
        }
        "image/gif" => {
          let mut decoder: GifDecoder<ImageDecoderFromReaderType> =
            ImageDecoderFromReader::to_decoder(BufReader::new(Cursor::new(
              buf,
            )), image_decoding_error)?;
          let icc_profile = decoder.get_icc_profile();
          (decoder.to_intermediate_image(image_decoding_error)?, icc_profile)
        }
        "image/bmp" => {
          let mut decoder: BmpDecoder<ImageDecoderFromReaderType> =
            ImageDecoderFromReader::to_decoder(BufReader::new(Cursor::new(
              buf,
            )), image_decoding_error)?;
          let icc_profile = decoder.get_icc_profile();
          (decoder.to_intermediate_image(image_decoding_error)?, icc_profile)
        }
        "image/x-icon" => {
          let mut decoder: IcoDecoder<ImageDecoderFromReaderType> =
            ImageDecoderFromReader::to_decoder(BufReader::new(Cursor::new(
              buf,
            )), image_decoding_error)?;
          let icc_profile = decoder.get_icc_profile();
          (decoder.to_intermediate_image(image_decoding_error)?, icc_profile)
        }
        "image/webp" => {
          let mut decoder: WebPDecoder<ImageDecoderFromReaderType> =
            ImageDecoderFromReader::to_decoder(BufReader::new(Cursor::new(
              buf,
            )), image_decoding_error)?;
          let icc_profile = decoder.get_icc_profile();
          (decoder.to_intermediate_image(image_decoding_error)?, icc_profile)
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

      (image, width, height, icc_profile)
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

      (image, width, height, None)
    }
  };

  Ok((image, width, height, icc_profile))
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
      fn unmatch_color_handler(
        x: ColorType,
        _: DynamicImage,
      ) -> Result<DynamicImage, AnyError> {
        Err(type_error(image_error_message(
          "apply colorspaceConversion: default",
          &format!("The color type {:?} is not supported.", x),
        )))
      }
      to_srgb_from_icc_profile(image, icc_profile, unmatch_color_handler)
    }
  }
}

fn apply_premultiply_alpha(
  image: DynamicImage,
  image_bitmap_source: &ImageBitmapSource,
  premultiply_alpha: &PremultiplyAlpha,
) -> Result<DynamicImage, AnyError> {
  let color = image.color();
  if !color.has_alpha() {
    Ok(image)
  } else {
    fn unmatch_color_handler(
      _: ColorType,
      image: DynamicImage,
    ) -> Result<DynamicImage, AnyError> {
      Ok(image)
    }
    match premultiply_alpha {
      // 1.
      PremultiplyAlpha::Default => Ok(image),

      // https://html.spec.whatwg.org/multipage/canvas.html#convert-from-premultiplied

      // 2.
      PremultiplyAlpha::Premultiply => {
        process_premultiply_alpha(image, unmatch_color_handler)
      }
      // 3.
      PremultiplyAlpha::None => {
        // NOTE: It's not clear how to handle the case of ImageData.
        // https://issues.chromium.org/issues/339759426
        // https://github.com/whatwg/html/issues/5365
        if *image_bitmap_source == ImageBitmapSource::ImageData {
          return Ok(image);
        }

        unpremultiply_alpha(image, unmatch_color_handler)
      }
    }
  }
}

#[op2]
#[serde]
pub(super) fn op_create_image_bitmap(
  #[buffer] zero_copy: JsBuffer,
  #[serde] args: OpCreateImageBitmapArgs,
) -> Result<OpCreateImageBitmapReturn, AnyError> {
  let buf = &*zero_copy;
  let OpCreateImageBitmapArgs {
    width,
    height,
    sh,
    sw,
    sx,
    sy,
    image_orientation,
    premultiply_alpha,
    color_space_conversion,
    resize_width,
    resize_height,
    resize_quality,
    image_bitmap_source,
    mime_type,
  } = OpCreateImageBitmapArgs {
    width: args.width,
    height: args.height,
    sx: args.sx,
    sy: args.sy,
    sw: args.sw,
    sh: args.sh,
    image_orientation: args.image_orientation,
    premultiply_alpha: args.premultiply_alpha,
    color_space_conversion: args.color_space_conversion,
    resize_width: args.resize_width,
    resize_height: args.resize_height,
    resize_quality: args.resize_quality,
    image_bitmap_source: args.image_bitmap_source,
    mime_type: args.mime_type,
  };

  // 6. Switch on image:
  let (image, width, height, icc_profile) =
    decode_bitmap_data(buf, width, height, &image_bitmap_source, mime_type)?;

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
  let image = image.resize_exact(output_width, output_height, filter_type);

  // 8.
  let image = if image_orientation == ImageOrientation::FlipY {
    image.flipv()
  } else {
    image
  };

  // 9.
  let image =
    apply_color_space_conversion(image, icc_profile, &color_space_conversion)?;

  // 10.
  let image =
    apply_premultiply_alpha(image, &image_bitmap_source, &premultiply_alpha)?;

  Ok(OpCreateImageBitmapReturn {
    data: image.into_bytes().into(),
    width: output_width,
    height: output_height,
  })
}
