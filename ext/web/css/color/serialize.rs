// Copyright 2018-2026 the Deno authors. MIT license.

//! Color serialization for canvas `fillStyle` / `strokeStyle` /
//! `shadowColor` getters.
//!
//! https://html.spec.whatwg.org/multipage/canvas.html#serialisation-of-a-color
//! https://www.w3.org/TR/css-color-4/#serializing-color-values

use std::fmt::Write;

use color::ColorSpaceTag;
use color::DynamicColor;

use super::ColorSyntax;
use super::ParsedColor;

#[inline]
pub fn serialize_color_for_canvas(color: &ParsedColor) -> String {
  // Colors in the hsl/hwb color spaces (e.g. from `color-mix(in hsl, ...)`)
  // also serialize in the legacy sRGB form, per CSS Color 4 §15.
  let legacy = color.syntax == ColorSyntax::Legacy
    || matches!(color.color.cs, ColorSpaceTag::Hsl | ColorSpaceTag::Hwb);
  if legacy {
    serialize_legacy(color)
  } else {
    serialize_modern(&color.color)
  }
}

/// `#rrggbb` when fully opaque, `rgba(r, g, b, a)` otherwise, after
/// quantization to 8-bit sRGB.
fn serialize_legacy(color: &ParsedColor) -> String {
  let rgba = color.to_srgb8().to_rgba8();
  if rgba.a == 255 {
    return format!("#{:02x}{:02x}{:02x}", rgba.r, rgba.g, rgba.b);
  }
  let mut result = format!("rgba({}, {}, {}", rgba.r, rgba.g, rgba.b);
  // Round-trips through the quantized alpha with the fewest decimals, per
  // CSS Color 4 §15.2 (e.g. `128` serializes as `0.5`).
  cssparser::color::serialize_color_alpha(
    &mut result,
    Some(rgba.a as f32 / 255.0),
    true,
  )
  .unwrap();
  result.push(')');
  result
}

fn serialize_modern(color: &DynamicColor) -> String {
  let missing = color.flags.missing();
  let component = |i: usize| -> String {
    if missing.contains(i) {
      "none".to_string()
    } else {
      format_css_number(color.components[i])
    }
  };
  let function = match color.cs {
    ColorSpaceTag::Lab => "lab",
    ColorSpaceTag::Lch => "lch",
    ColorSpaceTag::Oklab => "oklab",
    ColorSpaceTag::Oklch => "oklch",
    tag => {
      let name = match tag {
        ColorSpaceTag::Srgb => "srgb",
        ColorSpaceTag::LinearSrgb => "srgb-linear",
        ColorSpaceTag::DisplayP3 => "display-p3",
        ColorSpaceTag::A98Rgb => "a98-rgb",
        ColorSpaceTag::ProphotoRgb => "prophoto-rgb",
        ColorSpaceTag::Rec2020 => "rec2020",
        ColorSpaceTag::XyzD50 => "xyz-d50",
        ColorSpaceTag::XyzD65 => "xyz-d65",
        // The parser never produces other tags; convert defensively.
        _ => {
          return serialize_modern(&color.convert(ColorSpaceTag::Srgb));
        }
      };
      let mut result = format!(
        "color({name} {} {} {}",
        component(0),
        component(1),
        component(2)
      );
      write_modern_alpha(&mut result, color);
      result.push(')');
      return result;
    }
  };
  let mut result = format!(
    "{function}({} {} {}",
    component(0),
    component(1),
    component(2)
  );
  write_modern_alpha(&mut result, color);
  result.push(')');
  result
}

#[inline]
fn write_modern_alpha(dest: &mut String, color: &DynamicColor) {
  if color.flags.missing().contains(3) {
    dest.push_str(" / none");
    return;
  }
  let alpha = color.components[3];
  if alpha < 1.0 {
    write!(dest, " / {}", format_css_number(alpha)).unwrap();
  }
}

/// Formats a color component the way browsers do: rounded to at most 6
/// decimal places, with trailing zeros removed and `-0` normalized to `0`.
fn format_css_number(value: f32) -> String {
  // The `f32` Display impl yields the shortest string that round-trips,
  // avoiding f64-widening noise like `52.200001` for `52.2f32`.
  let shortest: f64 = format!("{value}").parse().unwrap_or(value as f64);
  let rounded = (shortest * 1e6).round() / 1e6;
  let rounded = if rounded == 0.0 { 0.0 } else { rounded };
  let mut result = format!("{rounded:.6}");
  let trimmed = result.trim_end_matches('0').trim_end_matches('.').len();
  result.truncate(trimmed);
  result
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn format_number() {
    assert_eq!(format_css_number(0.5), "0.5");
    assert_eq!(format_css_number(1.0), "1");
    assert_eq!(format_css_number(0.0), "0");
    assert_eq!(format_css_number(-0.0), "0");
    assert_eq!(format_css_number(125.0), "125");
    assert_eq!(format_css_number(0.125), "0.125");
    assert_eq!(format_css_number(1.0 / 3.0), "0.333333");
  }
}
