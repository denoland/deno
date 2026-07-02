// Copyright 2018-2026 the Deno authors. MIT license.

use deno_error::JsErrorBox;

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum Canvas2DError {
  #[class(type)]
  #[error("Illegal constructor")]
  IllegalConstructor,
  #[class("DOMExceptionIndexSizeError")]
  #[error("The radius provided ({0}) is negative")]
  NegativeRadius(f64),
  #[class("DOMExceptionIndexSizeError")]
  #[error("The source width or height is zero")]
  ZeroSourceSize,
  #[class(range)]
  #[error("Radii must be non-negative")]
  NegativeRoundRectRadius,
  #[class(range)]
  #[error("The radii sequence must have between 0 and 4 elements, got {0}")]
  InvalidRadiiLength(usize),
  #[class("DOMExceptionInvalidStateError")]
  #[error("The image source is detached")]
  ImageSourceDetached,
  #[class("DOMExceptionInvalidStateError")]
  #[error("The image source has zero width or height")]
  ImageSourceZeroDimensions,
  #[class("DOMExceptionInvalidStateError")]
  #[error("{0}")]
  InvalidState(String),
  #[class(type)]
  #[error("Invalid beginLayer filter option")]
  InvalidBeginLayerFilter,
  #[class(type)]
  #[error("The provided value is non-finite")]
  NonFinite,
  #[class(type)]
  #[error("The argument is not of type 'CanvasImageSource'")]
  NotCanvasImageSource,
  #[class(type)]
  #[error("The argument is not of type 'ImageData'")]
  NotImageData,
  #[class(type)]
  #[error("{required} argument required, but only {provided} present")]
  MissingArgument { required: u32, provided: u32 },
  #[class(generic)]
  #[error(transparent)]
  Render(#[from] crate::canvas2d::error::RenderError),
  #[class(generic)]
  #[error("canvas2d not initialized")]
  NotInitialized,
  #[class("DOMExceptionIndexSizeError")]
  #[error("The index is not in the allowed range.")]
  ColorStopIndexSize,
  #[class("DOMExceptionSyntaxError")]
  #[error("Failed to parse color")]
  ColorStopSyntax,
  #[class(type)]
  #[error("The provided value is non-finite.")]
  ColorStopTypeError,
  #[class("DOMExceptionSyntaxError")]
  #[error("The string did not match the expected pattern.")]
  PatternSyntax,
  #[class(inherit)]
  #[error(transparent)]
  Geometry(#[from] crate::geometry::GeometryError),
  #[class(inherit)]
  #[error(transparent)]
  WebIdl(#[from] deno_core::webidl::WebIdlError),
  #[class(inherit)]
  #[error(transparent)]
  ImageData(#[from] crate::image_data::ImageDataError),
}

impl From<Canvas2DError> for JsErrorBox {
  fn from(err: Canvas2DError) -> Self {
    JsErrorBox::from_err(err)
  }
}

#[derive(Debug, thiserror::Error)]
#[error(transparent)]
pub struct RenderError(#[from] vello::Error);
