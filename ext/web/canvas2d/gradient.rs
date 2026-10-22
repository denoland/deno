// Copyright 2018-2026 the Deno authors. MIT license.

use std::cell::RefCell;
use std::f64::consts::TAU;

use deno_core::GarbageCollected;
use deno_core::op2;
use deno_core::v8::cppgc::Visitor;
use vello::kurbo::Point;
use vello::peniko;
use vello::peniko::InterpolationAlphaSpace;

use crate::canvas2d::angle::normalize_angle;
use crate::canvas2d::error::Canvas2DError;
use crate::css::color::parse_css_color;

pub struct CanvasGradient {
  pub(super) gradient: RefCell<peniko::Gradient>,
}

// SAFETY: CanvasGradient is only accessed from the JS thread.
unsafe impl GarbageCollected for CanvasGradient {
  fn trace(&self, _visitor: &mut Visitor) {}

  fn get_name(&self) -> &'static std::ffi::CStr {
    c"CanvasGradient"
  }
}

#[op2]
impl CanvasGradient {
  #[constructor]
  #[cppgc]
  fn new() -> Result<CanvasGradient, Canvas2DError> {
    Err(Canvas2DError::IllegalConstructor)
  }

  #[required(2)]
  #[undefined]
  fn add_color_stop(
    &self,
    #[webidl] offset: f64,
    #[webidl] color: String,
  ) -> Result<(), Canvas2DError> {
    let offset = validate_color_stop_offset(offset)?;
    let stop = parse_color_stop(offset, &color)?;
    self.gradient.borrow_mut().stops.push(stop);
    Ok(())
  }
}

#[inline]
fn canvas_gradient_base(mut gradient: peniko::Gradient) -> peniko::Gradient {
  gradient.interpolation_alpha_space = InterpolationAlphaSpace::Unpremultiplied;
  gradient
}

/// Builds a linear gradient per Canvas 2D createLinearGradient().
pub fn build_linear_gradient(
  x0: f64,
  y0: f64,
  x1: f64,
  y1: f64,
) -> peniko::Gradient {
  canvas_gradient_base(peniko::Gradient::new_linear(
    Point::new(x0, y0),
    Point::new(x1, y1),
  ))
}

/// Builds a two-point radial gradient per Canvas 2D createRadialGradient().
pub fn build_radial_gradient(
  x0: f64,
  y0: f64,
  r0: f64,
  x1: f64,
  y1: f64,
  r1: f64,
) -> peniko::Gradient {
  canvas_gradient_base(peniko::Gradient::new_two_point_radial(
    Point::new(x0, y0),
    r0 as f32,
    Point::new(x1, y1),
    r1 as f32,
  ))
}

/// Builds a conic (sweep) gradient per Canvas 2D createConicGradient().
pub fn build_conic_gradient(
  start_angle: f64,
  x: f64,
  y: f64,
) -> peniko::Gradient {
  let start = normalize_angle(start_angle) as f32;
  canvas_gradient_base(peniko::Gradient::new_sweep(
    Point::new(x, y),
    start,
    start + TAU as f32,
  ))
}

/// Validates a color-stop offset per Canvas 2D addColorStop().
pub fn validate_color_stop_offset(offset: f64) -> Result<f32, Canvas2DError> {
  if !offset.is_finite() {
    return Err(Canvas2DError::ColorStopTypeError);
  }
  if !(0.0..=1.0).contains(&offset) {
    return Err(Canvas2DError::ColorStopIndexSize);
  }
  Ok(offset as f32)
}

/// Parses a CSS color string into a peniko color stop at the given offset.
pub fn parse_color_stop(
  offset: f32,
  color: &str,
) -> Result<peniko::ColorStop, Canvas2DError> {
  let parsed =
    parse_css_color(color).map_err(|_| Canvas2DError::ColorStopSyntax)?;
  // Canvas gradients interpolate in sRGB; convert while keeping f32
  // precision instead of quantizing to 8 bits.
  Ok(peniko::ColorStop {
    offset,
    color: parsed.color.convert(peniko::color::ColorSpaceTag::Srgb),
  })
}
