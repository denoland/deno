// Copyright 2018-2026 the Deno authors. MIT license.

use color::DynamicColor;
use color::Srgb;

use super::error::CSSCustomError;

/// Parses a CSS Color Level 4 string into an RGBA8 array [r, g, b, a].
///
/// Supported formats:
/// - Hex: `#RGB`, `#RRGGBB`, `#RGBA`, `#RRGGBBAA`
/// - Functions: `rgb()`, `rgba()`, `hsl()`, `hsla()`, `hwb()`
/// - Modern: `lab()`, `lch()`, `oklab()`, `oklch()`
/// - Color function: `color(srgb ...)`, `color(display-p3 ...)`, etc.
/// - Named: `red`, `blue`, `transparent`, etc.
pub fn parse_css_color(s: &str) -> Result<[u8; 4], CSSCustomError> {
  let s = s.trim();
  let dyn_color: DynamicColor =
    color::parse_color(s).map_err(|_| CSSCustomError::InvalidColor)?;
  let srgb = dyn_color.to_alpha_color::<Srgb>();
  let [r, g, b, a] = srgb.components;
  Ok([
    (r.clamp(0.0, 1.0) * 255.0).round() as u8,
    (g.clamp(0.0, 1.0) * 255.0).round() as u8,
    (b.clamp(0.0, 1.0) * 255.0).round() as u8,
    (a.clamp(0.0, 1.0) * 255.0).round() as u8,
  ])
}

/// Converts RGBA8 [r, g, b, a] to a CSS color string.
pub fn rgba8_to_css(rgba: [u8; 4]) -> String {
  let [r, g, b, a] = rgba;
  if a == 255 {
    format!("rgb({r}, {g}, {b})")
  } else {
    format!("rgba({r}, {g}, {b}, {:.6})", a as f64 / 255.0)
  }
}
