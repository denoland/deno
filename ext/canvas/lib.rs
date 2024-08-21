// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use deno_core::error::type_error;
use deno_core::error::AnyError;
use deno_core::op2;
use deno_core::ToJsBuffer;
use image::imageops::FilterType;
use image::AnimationDecoder;
use image::GenericImageView;
use image::Pixel;
use serde::Deserialize;
use serde::Serialize;
use std::io::BufReader;
use std::io::Cursor;
use std::path::PathBuf;

pub mod error;
use error::DOMExceptionInvalidStateError;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ImageResizeQuality {
  Pixelated,
  Low,
  Medium,
  High,
}

#[derive(Debug, Deserialize)]
// Follow the cases defined in the spec
enum ImageBitmapSource {
  Blob,
  ImageData,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ImageProcessArgs {
  width: u32,
  height: u32,
  surface_width: u32,
  surface_height: u32,
  input_x: i64,
  input_y: i64,
  output_width: u32,
  output_height: u32,
  resize_quality: ImageResizeQuality,
  flip_y: bool,
  premultiply: Option<bool>,
  image_bitmap_source: ImageBitmapSource,
}

#[op2]
#[serde]
fn op_image_process(
  #[buffer] buf: &[u8],
  #[serde] args: ImageProcessArgs,
) -> Result<ToJsBuffer, AnyError> {
  let view = match args.image_bitmap_source {
    ImageBitmapSource::Blob => image::ImageReader::new(Cursor::new(buf))
      .with_guessed_format()?
      .decode()?,
    ImageBitmapSource::ImageData => {
      // > 4.12.5.1.15 Pixel manipulation
      // > imagedata.data
      // >   Returns the one-dimensional array containing the data in RGBA order, as integers in the range 0 to 255.
      // https://html.spec.whatwg.org/multipage/canvas.html#pixel-manipulation
      let image: image::DynamicImage =
        image::RgbaImage::from_raw(args.width, args.height, buf.into())
          .expect("Invalid ImageData.")
          .into();
      image
    }
  };
  let color = view.color();

  let surface = if !(args.width == args.surface_width
    && args.height == args.surface_height
    && args.input_x == 0
    && args.input_y == 0)
  {
    let mut surface =
      image::DynamicImage::new(args.surface_width, args.surface_height, color);
    image::imageops::overlay(&mut surface, &view, args.input_x, args.input_y);

    surface
  } else {
    view
  };

  let filter_type = match args.resize_quality {
    ImageResizeQuality::Pixelated => FilterType::Nearest,
    ImageResizeQuality::Low => FilterType::Triangle,
    ImageResizeQuality::Medium => FilterType::CatmullRom,
    ImageResizeQuality::High => FilterType::Lanczos3,
  };

  // should use resize_exact
  // https://github.com/image-rs/image/issues/1220#issuecomment-632060015
  let mut image_out =
    surface.resize_exact(args.output_width, args.output_height, filter_type);

  if args.flip_y {
    image::imageops::flip_vertical_in_place(&mut image_out);
  }

  // ignore 9.

  if color.has_alpha() {
    if let Some(premultiply) = args.premultiply {
      let is_not_premultiplied = image_out.pixels().any(|(_, _, pixel)| {
        (pixel[0].max(pixel[1]).max(pixel[2])) > (255 * pixel[3])
      });

      if premultiply {
        if is_not_premultiplied {
          for (_, _, mut pixel) in &mut image_out.pixels() {
            let alpha = pixel[3];
            pixel.apply_without_alpha(|channel| {
              (channel as f32 * (alpha as f32 / 255.0)) as u8
            })
          }
        }
      } else if !is_not_premultiplied {
        for (_, _, mut pixel) in &mut image_out.pixels() {
          let alpha = pixel[3];
          pixel.apply_without_alpha(|channel| {
            (channel as f32 / (alpha as f32 / 255.0)) as u8
          })
        }
      }
    }
  }

  Ok(image_out.into_bytes().into())
}

#[derive(Debug, Serialize)]
struct DecodedImage {
  width: u32,
  height: u32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ImageDecodeOptions {
  mime_type: String,
}

#[op2]
#[serde]
fn op_image_decode(
  #[buffer] buf: &[u8],
  #[serde] options: ImageDecodeOptions,
) -> Result<DecodedImage, AnyError> {
  let reader = BufReader::new(Cursor::new(buf));
  //
  // TODO: support animated images
  // It's a little hard to implement animated images along spec because of the complexity.
  //
  // > If this is an animated image, imageBitmap's bitmap data must only be taken from
  // > the default image of the animation (the one that the format defines is to be used when animation is
  // > not supported or is disabled), or, if there is no such image, the first frame of the animation.
  // https://html.spec.whatwg.org/multipage/imagebitmap-and-animations.html
  //
  // see also browser implementations: (The implementation of Gecko and WebKit is hard to read.)
  // https://source.chromium.org/chromium/chromium/src/+/bdbc054a6cabbef991904b5df9066259505cc686:third_party/blink/renderer/platform/image-decoders/image_decoder.h;l=175-189
  //
  let image = match &*options.mime_type {
    "image/png" => {
      let decoder = image::codecs::png::PngDecoder::new(reader)?;
      if decoder.is_apng()? {
        return Err(type_error("Animation image is not supported."));
      }
      image::DynamicImage::from_decoder(decoder)?
    }
    "image/jpeg" => {
      let decoder = image::codecs::jpeg::JpegDecoder::new(reader)?;
      image::DynamicImage::from_decoder(decoder)?
    }
    "image/gif" => {
      let decoder = image::codecs::gif::GifDecoder::new(reader)?;
      if decoder.into_frames().count() > 1 {
        return Err(type_error("Animation image is not supported."));
      }
      let reader = BufReader::new(Cursor::new(buf));
      let decoder = image::codecs::gif::GifDecoder::new(reader)?;
      image::DynamicImage::from_decoder(decoder)?
    }
    "image/bmp" => {
      let decoder = image::codecs::bmp::BmpDecoder::new(reader)?;
      image::DynamicImage::from_decoder(decoder)?
    }
    "image/x-icon" => {
      let decoder = image::codecs::ico::IcoDecoder::new(reader)?;
      image::DynamicImage::from_decoder(decoder)?
    }
    "image/webp" => {
      let decoder = image::codecs::webp::WebPDecoder::new(reader)?;
      if decoder.has_animation() {
        return Err(type_error("Animation image is not supported."));
      }
      image::DynamicImage::from_decoder(decoder)?
    }
    // return an error if the mime type is not supported in the variable list of ImageTypePatternTable below
    // ext/web/01_mimesniff.js
    _ => {
      return Err(
        DOMExceptionInvalidStateError::new(
          "The source image is not a supported format.",
        )
        .into(),
      )
    }
  };
  let (width, height) = image.dimensions();

  Ok(DecodedImage { width, height })
}

deno_core::extension!(
  deno_canvas,
  deps = [deno_webidl, deno_web, deno_webgpu],
  ops = [op_image_process, op_image_decode],
  lazy_loaded_esm = ["01_image.js"],
);

pub fn get_declaration() -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("lib.deno_canvas.d.ts")
}
