// Copyright 2018-2026 the Deno authors. MIT license.

//! `color-mix()` parsing.
//!
//! https://www.w3.org/TR/css-color-5/#color-mix

use color::ColorSpaceTag;
use color::HueDirection;
use cssparser::Parser;
use cssparser::match_ignore_ascii_case;

use super::ColorSyntax;
use super::ParsedColor;
use super::parse::parse_color_value;
use crate::css::error::CSSCustomError;
use crate::css::error::CSSParseError;
use crate::css::value::NumericValue;
use crate::css::value::ParseOptions;

/// `color-mix( <color-interpolation-method>, <color> <percentage>?,
/// <color> <percentage>? )`, with interpolation delegated to the `color`
/// crate (premultiplied alpha, missing component carrying and hue fixup per
/// CSS Color 4 §12).
pub(super) fn parse_color_mix<'i, 't>(
  args: &mut Parser<'i, 't>,
) -> Result<ParsedColor, CSSParseError<'i>> {
  args.expect_ident_matching("in")?;
  let space = args.expect_ident_cloned()?;
  let tag = interpolation_space(&space)
    .ok_or_else(|| args.new_custom_error(CSSCustomError::InvalidColor))?;
  let mut direction = HueDirection::Shorter;
  if is_polar(tag)
    && let Ok(parsed) =
      args.try_parse(|i| -> Result<HueDirection, CSSParseError<'i>> {
        let ident = i.expect_ident_cloned()?;
        let direction = match_ignore_ascii_case! { &ident,
          "shorter" => HueDirection::Shorter,
          "longer" => HueDirection::Longer,
          "increasing" => HueDirection::Increasing,
          "decreasing" => HueDirection::Decreasing,
          _ => return Err(i.new_custom_error(CSSCustomError::InvalidColor)),
        };
        i.expect_ident_matching("hue")?;
        Ok(direction)
      })
  {
    direction = parsed;
  }
  args.expect_comma()?;
  let (color1, percent1) = parse_mix_arm(args)?;
  args.expect_comma()?;
  let (color2, percent2) = parse_mix_arm(args)?;
  args.expect_exhausted()?;

  // https://www.w3.org/TR/css-color-5/#color-mix-percent-norm
  let (percent1, percent2) = match (percent1, percent2) {
    (None, None) => (0.5, 0.5),
    (Some(p), None) => (p, 1.0 - p),
    (None, Some(p)) => (1.0 - p, p),
    (Some(p1), Some(p2)) => (p1, p2),
  };
  let sum = percent1 + percent2;
  if sum <= 0.0 {
    return Err(args.new_custom_error(CSSCustomError::InvalidColor));
  }
  let t = (percent2 / sum) as f32;
  let alpha_multiplier = if sum < 1.0 { sum as f32 } else { 1.0 };

  let mut result = color1
    .color
    .interpolate(color2.color, tag, direction)
    .eval(t);
  if alpha_multiplier < 1.0 && !result.flags.missing().contains(3) {
    result.components[3] =
      (result.components[3] * alpha_multiplier).clamp(0.0, 1.0);
  }
  Ok(ParsedColor {
    color: result,
    syntax: ColorSyntax::Modern,
  })
}

/// `<color> <percentage>?` where the percentage may come on either side of
/// the color. Out-of-range percentages are clamped to `[0%, 100%]`.
#[inline]
fn parse_mix_arm<'i, 't>(
  args: &mut Parser<'i, 't>,
) -> Result<(ParsedColor, Option<f64>), CSSParseError<'i>> {
  let mut percent = try_parse_percentage(args);
  let color = parse_color_value(args)?;
  if percent.is_none() {
    percent = try_parse_percentage(args);
  }
  Ok((color, percent))
}

#[inline]
fn try_parse_percentage(args: &mut Parser<'_, '_>) -> Option<f64> {
  args
    .try_parse(|i| -> Result<f64, CSSParseError<'_>> {
      match NumericValue::parse(i, ParseOptions::default())? {
        NumericValue::Percent(p) => Ok(p.clamp(0.0, 1.0)),
        _ => Err(i.new_custom_error(CSSCustomError::UnexpectedNumericType)),
      }
    })
    .ok()
}

/// https://www.w3.org/TR/css-color-5/#typedef-color-interpolation-method
#[inline]
fn interpolation_space(ident: &str) -> Option<ColorSpaceTag> {
  Some(match_ignore_ascii_case! { ident,
    "srgb" => ColorSpaceTag::Srgb,
    "srgb-linear" => ColorSpaceTag::LinearSrgb,
    "display-p3" => ColorSpaceTag::DisplayP3,
    "a98-rgb" => ColorSpaceTag::A98Rgb,
    "prophoto-rgb" => ColorSpaceTag::ProphotoRgb,
    "rec2020" => ColorSpaceTag::Rec2020,
    "lab" => ColorSpaceTag::Lab,
    "oklab" => ColorSpaceTag::Oklab,
    "xyz" | "xyz-d65" => ColorSpaceTag::XyzD65,
    "xyz-d50" => ColorSpaceTag::XyzD50,
    "hsl" => ColorSpaceTag::Hsl,
    "hwb" => ColorSpaceTag::Hwb,
    "lch" => ColorSpaceTag::Lch,
    "oklch" => ColorSpaceTag::Oklch,
    _ => return None,
  })
}

#[inline]
fn is_polar(tag: ColorSpaceTag) -> bool {
  matches!(
    tag,
    ColorSpaceTag::Hsl
      | ColorSpaceTag::Hwb
      | ColorSpaceTag::Lch
      | ColorSpaceTag::Oklch
  )
}
