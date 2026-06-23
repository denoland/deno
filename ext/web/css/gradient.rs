// Copyright 2018-2026 the Deno authors. MIT license.

use std::f64::consts::TAU;

use vello::kurbo::Point;
use vello::peniko;
use vello::peniko::InterpolationAlphaSpace;

use super::color::parse_css_color;

#[derive(Debug, thiserror::Error)]
pub enum ColorStopError {
  #[error("The index is not in the allowed range.")]
  IndexSize,
  #[error("Failed to parse color")]
  Syntax,
  #[error("The value provided is non-finite.")]
  TypeError,
}

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
  let start = start_angle as f32;
  canvas_gradient_base(peniko::Gradient::new_sweep(
    Point::new(x, y),
    start,
    start + TAU as f32,
  ))
}

/// Validates a color-stop offset per Canvas 2D addColorStop().
pub fn validate_color_stop_offset(offset: f64) -> Result<f32, ColorStopError> {
  if !offset.is_finite() {
    return Err(ColorStopError::TypeError);
  }
  if !(0.0..=1.0).contains(&offset) {
    return Err(ColorStopError::IndexSize);
  }
  Ok(offset as f32)
}

/// Parses a CSS color string into a peniko color stop at the given offset.
pub fn parse_color_stop(
  offset: f32,
  color: &str,
) -> Result<peniko::ColorStop, ColorStopError> {
  let [r, g, b, a] =
    parse_css_color(color).map_err(|_| ColorStopError::Syntax)?;
  let c = peniko::Color::from_rgba8(r, g, b, a);
  Ok(peniko::ColorStop {
    offset,
    color: peniko::color::DynamicColor::from_alpha_color(c),
  })
}
