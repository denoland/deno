// Copyright 2018-2026 the Deno authors. MIT license.

use std::borrow::Cow;
use std::cell::Cell;

use deno_core::GarbageCollected;
use deno_core::WebIDL;
use deno_core::op2;
use deno_core::v8;
use deno_core::webidl::ContextFn;
use deno_core::webidl::IntOptions;
use deno_core::webidl::WebIdlConverter;
use deno_core::webidl::WebIdlError;
use deno_core::webidl::WebIdlErrorKind;

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum ImageDataError {
  #[class(inherit)]
  #[error(transparent)]
  WebIDL(#[from] WebIdlError),
  #[class("DOMExceptionInvalidStateError")]
  #[error("Failed to construct 'ImageData': the input data has zero elements")]
  ZeroElements,
  #[class("DOMExceptionInvalidStateError")]
  #[error(
    "Failed to construct 'ImageData': the input data length is not a multiple of 4, received {0}"
  )]
  NotMultipleOfFour(usize),
  #[class("DOMExceptionIndexSizeError")]
  #[error(
    "Failed to construct 'ImageData': the source width is zero or not a number"
  )]
  ZeroWidth,
  #[class("DOMExceptionIndexSizeError")]
  #[error(
    "Failed to construct 'ImageData': the source height is zero or not a number"
  )]
  ZeroHeight,
  #[class("DOMExceptionIndexSizeError")]
  #[error(
    "Failed to construct 'ImageData': the input data length is not a multiple of (4 * width)"
  )]
  NotMultipleOfRow,
  #[class("DOMExceptionIndexSizeError")]
  #[error(
    "Failed to construct 'ImageData': the input data length is not equal to (4 * width * height)"
  )]
  WrongLength,
  #[class("DOMExceptionInvalidStateError")]
  #[error(
    "Failed to construct 'ImageData': Uint8ClampedArray must use rgba-unorm8 pixelFormat."
  )]
  Uint8ClampedNeedsUnorm,
  #[class("DOMExceptionInvalidStateError")]
  #[error(
    "Failed to construct 'ImageData': Float16Array must use rgba-float16 pixelFormat."
  )]
  Float16NeedsFloat16,
  #[class(generic)]
  #[error("Failed to allocate ImageData backing store")]
  AllocationFailed,
}

#[derive(WebIDL, Debug, Clone, Copy, PartialEq, Eq)]
#[webidl(enum)]
pub enum PredefinedColorSpace {
  #[webidl(rename = "srgb")]
  Srgb,
  #[webidl(rename = "display-p3")]
  DisplayP3,
}

impl PredefinedColorSpace {
  fn name(self) -> &'static str {
    match self {
      Self::Srgb => "srgb",
      Self::DisplayP3 => "display-p3",
    }
  }
}

#[derive(WebIDL, Debug, Clone, Copy, PartialEq, Eq)]
#[webidl(enum)]
pub enum ImageDataPixelFormat {
  #[webidl(rename = "rgba-unorm8")]
  RgbaUnorm8,
  #[webidl(rename = "rgba-float16")]
  RgbaFloat16,
}

impl ImageDataPixelFormat {
  fn name(self) -> &'static str {
    match self {
      Self::RgbaUnorm8 => "rgba-unorm8",
      Self::RgbaFloat16 => "rgba-float16",
    }
  }
}

#[derive(WebIDL, Debug)]
#[webidl(dictionary)]
pub struct ImageDataSettings {
  #[webidl(default = None)]
  color_space: Option<PredefinedColorSpace>,
  #[webidl(default = ImageDataPixelFormat::RgbaUnorm8)]
  pixel_format: ImageDataPixelFormat,
}

pub struct ImageData {
  width: Cell<u32>,
  height: Cell<u32>,
  pixel_format: Cell<ImageDataPixelFormat>,
  color_space: Cell<PredefinedColorSpace>,
  data: v8::TracedReference<v8::Object>,
}

// SAFETY: we're sure `ImageData` can be GCed.
unsafe impl GarbageCollected for ImageData {
  fn trace(&self, visitor: &mut v8::cppgc::Visitor) {
    visitor.trace(&self.data);
  }

  fn get_name(&self) -> &'static std::ffi::CStr {
    c"ImageData"
  }
}

#[inline]
fn convert_unsigned_long<'a>(
  scope: &mut v8::PinScope<'a, '_>,
  value: v8::Local<'a, v8::Value>,
  context: &'static str,
) -> Result<u32, WebIdlError> {
  u32::convert(
    scope,
    value,
    Cow::Borrowed("Failed to construct 'ImageData'"),
    ContextFn::new_borrowed(&|| Cow::Borrowed(context)),
    &IntOptions::default(),
  )
}

#[inline]
fn convert_settings<'a>(
  scope: &mut v8::PinScope<'a, '_>,
  value: v8::Local<'a, v8::Value>,
  context: &'static str,
) -> Result<ImageDataSettings, WebIdlError> {
  ImageDataSettings::convert(
    scope,
    value,
    Cow::Borrowed("Failed to construct 'ImageData'"),
    ContextFn::new_borrowed(&|| Cow::Borrowed(context)),
    &Default::default(),
  )
}

#[inline]
fn alloc_typed_array<'a>(
  scope: &mut v8::PinScope<'a, '_>,
  pixel_format: ImageDataPixelFormat,
  width: u32,
  height: u32,
) -> Result<v8::Local<'a, v8::Object>, ImageDataError> {
  let pixel_count = (width as usize)
    .checked_mul(height as usize)
    .and_then(|v| v.checked_mul(4))
    .ok_or(ImageDataError::AllocationFailed)?;
  match pixel_format {
    ImageDataPixelFormat::RgbaUnorm8 => {
      let buffer = v8::ArrayBuffer::new(scope, pixel_count);
      v8::Uint8ClampedArray::new(scope, buffer, 0, pixel_count)
        .map(Into::into)
        .ok_or(ImageDataError::AllocationFailed)
    }
    ImageDataPixelFormat::RgbaFloat16 => {
      let byte_length = pixel_count
        .checked_mul(2)
        .ok_or(ImageDataError::AllocationFailed)?;
      let buffer = v8::ArrayBuffer::new(scope, byte_length);
      let arr = v8::Float16Array::new(scope, buffer, 0, pixel_count)
        .ok_or(ImageDataError::AllocationFailed)?;
      // SAFETY: a Float16Array is a v8::Object (transmute is the existing
      // workaround until rusty_v8 implements `Into`).
      Ok(unsafe {
        std::mem::transmute::<v8::Local<v8::Float16Array>, v8::Local<v8::Object>>(
          arr,
        )
      })
    }
  }
}

#[op2]
impl ImageData {
  #[constructor]
  #[reentrant]
  #[required(2)]
  #[cppgc]
  fn constructor<'a>(
    scope: &mut v8::PinScope<'a, '_>,
    #[varargs] args: Option<&v8::FunctionCallbackArguments<'a>>,
  ) -> Result<ImageData, ImageDataError> {
    // `#[required(2)]` ensures `args` has at least 2 entries.
    let args = args.expect("constructor requires arguments");
    let arg_count = args.length();
    let arg0 = args.get(0);
    let arg1 = args.get(1);
    let arg2 = if arg_count >= 3 {
      Some(args.get(2))
    } else {
      None
    };
    let arg3 = if arg_count >= 4 {
      Some(args.get(3))
    } else {
      None
    };

    let arg0_typed_array = v8::Local::<v8::TypedArray>::try_from(arg0).ok();
    let arg0_is_uint8_clamped = if arg0.is_object() {
      arg0.cast::<v8::Object>().is_uint8_clamped_array()
    } else {
      false
    };
    let arg0_is_float16 = if arg0.is_object() {
      arg0.cast::<v8::Object>().is_float16_array()
    } else {
      false
    };

    if arg_count > 3 || arg0_is_uint8_clamped || arg0_is_float16 {
      // Overload: new ImageData(data, sw [, sh [, settings ] ])
      let data = arg0_typed_array.ok_or_else(|| {
        WebIdlError::new(
          Cow::Borrowed("Failed to construct 'ImageData'"),
          ContextFn::new_borrowed(&|| Cow::Borrowed("Argument 1")),
          WebIdlErrorKind::ConvertToConverterType("ArrayBufferView"),
        )
      })?;
      let source_width = convert_unsigned_long(scope, arg1, "Argument 2")?;
      let source_height = match arg2 {
        Some(v) if !v.is_undefined() => {
          Some(convert_unsigned_long(scope, v, "Argument 3")?)
        }
        _ => None,
      };
      let settings_value = arg3.unwrap_or_else(|| v8::undefined(scope).into());
      let settings = convert_settings(scope, settings_value, "Argument 4")?;

      // Match `TypedArrayPrototypeGetLength` semantics: element count, not
      // byte count.
      let data_length = data.length();

      if data_length == 0 {
        return Err(ImageDataError::ZeroElements);
      }
      if data_length % 4 != 0 {
        return Err(ImageDataError::NotMultipleOfFour(data_length));
      }
      if source_width == 0 {
        return Err(ImageDataError::ZeroWidth);
      }
      if let Some(h) = source_height
        && h == 0
      {
        return Err(ImageDataError::ZeroHeight);
      }
      let pixel_count = data_length / 4;
      if pixel_count % source_width as usize != 0 {
        return Err(ImageDataError::NotMultipleOfRow);
      }
      let derived_height = (pixel_count / source_width as usize) as u32;
      if let Some(h) = source_height
        && h != derived_height
      {
        return Err(ImageDataError::WrongLength);
      }

      if arg0_is_uint8_clamped
        && !matches!(settings.pixel_format, ImageDataPixelFormat::RgbaUnorm8)
      {
        return Err(ImageDataError::Uint8ClampedNeedsUnorm);
      }
      if arg0_is_float16
        && !matches!(settings.pixel_format, ImageDataPixelFormat::RgbaFloat16)
      {
        return Err(ImageDataError::Float16NeedsFloat16);
      }

      let color_space =
        settings.color_space.unwrap_or(PredefinedColorSpace::Srgb);
      let height = source_height.unwrap_or(derived_height);
      let data_obj: v8::Local<v8::Object> = data.into();

      Ok(ImageData {
        width: Cell::new(source_width),
        height: Cell::new(height),
        pixel_format: Cell::new(settings.pixel_format),
        color_space: Cell::new(color_space),
        data: v8::TracedReference::new(scope, data_obj),
      })
    } else {
      // Overload: new ImageData(sw, sh [, settings])
      let source_width = convert_unsigned_long(scope, arg0, "Argument 1")?;
      let source_height = convert_unsigned_long(scope, arg1, "Argument 2")?;
      let settings_value = arg2.unwrap_or_else(|| v8::undefined(scope).into());
      let settings = convert_settings(scope, settings_value, "Argument 3")?;

      if source_width == 0 {
        return Err(ImageDataError::ZeroWidth);
      }
      if source_height == 0 {
        return Err(ImageDataError::ZeroHeight);
      }

      let data_obj = alloc_typed_array(
        scope,
        settings.pixel_format,
        source_width,
        source_height,
      )?;
      let color_space =
        settings.color_space.unwrap_or(PredefinedColorSpace::Srgb);

      Ok(ImageData {
        width: Cell::new(source_width),
        height: Cell::new(source_height),
        pixel_format: Cell::new(settings.pixel_format),
        color_space: Cell::new(color_space),
        data: v8::TracedReference::new(scope, data_obj),
      })
    }
  }

  #[fast]
  #[getter]
  fn width(&self) -> u32 {
    self.width.get()
  }

  #[fast]
  #[getter]
  fn height(&self) -> u32 {
    self.height.get()
  }

  /// Pixel buffer accessor — exposed under `Symbol.for("Deno_imageData_data")`
  /// rather than the public `data` name. The `data` attribute getter on the
  /// prototype (defined in `image_data.js`) calls this method to resolve the
  /// stored typed array. Same shape as `ImageBitmap`'s `Deno_bitmapData`.
  #[symbol("Deno_imageData_data")]
  fn get_data<'a>(
    &self,
    scope: &mut v8::PinScope<'a, '_>,
  ) -> v8::Local<'a, v8::Object> {
    self.data.get(scope).unwrap()
  }

  #[getter]
  #[string]
  fn pixel_format(&self) -> &'static str {
    self.pixel_format.get().name()
  }

  #[getter]
  #[string]
  fn color_space(&self) -> &'static str {
    self.color_space.get().name()
  }
}
