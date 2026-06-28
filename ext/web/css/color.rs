// Copyright 2018-2026 the Deno authors. MIT license.

// TODO(petamoriken): peniko::Color is AlphaColor<Srgb>. When peniko adds
// support for non-sRGB color spaces (e.g. display-p3), update this module
// to leverage those types for wide-gamut canvas color spaces.

use color::DynamicColor;
use color::Srgb;
pub use peniko::Color;
use vello::peniko;

use super::error::CSSCustomError;

pub fn is_color_transparent(c: Color) -> bool {
  c.to_rgba8().a == 0
}

pub fn color_to_css_string(color: Color) -> String {
  let rgba = color.to_rgba8();
  if rgba.a == 255 {
    format!("#{:02x}{:02x}{:02x}", rgba.r, rgba.g, rgba.b)
  } else {
    let alpha = rgba.a as f64 / 255.0;
    let alpha_str = format!("{alpha:.6}");
    let alpha_str = alpha_str.trim_end_matches('0');
    let alpha_str = alpha_str.strip_suffix('.').unwrap_or(alpha_str);
    format!("rgba({}, {}, {}, {alpha_str})", rgba.r, rgba.g, rgba.b)
  }
}

/// Parses a CSS Color Level 4 string into a [`Color`].
///
/// Supported formats:
/// - Hex: `#RGB`, `#RRGGBB`, `#RGBA`, `#RRGGBBAA`
/// - Functions: `rgb()`, `rgba()`, `hsl()`, `hsla()`, `hwb()`
/// - Modern: `lab()`, `lch()`, `oklab()`, `oklch()`
/// - Color function: `color(srgb ...)`, `color(display-p3 ...)`, etc.
/// - Named: `red`, `blue`, `transparent`, etc.
pub fn parse_css_color(s: &str) -> Result<Color, CSSCustomError> {
  let s = s.trim();
  let dyn_color: DynamicColor =
    color::parse_color(s).map_err(|_| CSSCustomError::InvalidColor)?;
  let srgb = dyn_color.to_alpha_color::<Srgb>();
  let [r, g, b, a] = srgb.components;
  Ok(Color::from_rgba8(
    (r.clamp(0.0, 1.0) * 255.0).round() as u8,
    (g.clamp(0.0, 1.0) * 255.0).round() as u8,
    (b.clamp(0.0, 1.0) * 255.0).round() as u8,
    (a.clamp(0.0, 1.0) * 255.0).round() as u8,
  ))
}
