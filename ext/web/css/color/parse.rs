// Copyright 2018-2026 the Deno authors. MIT license.

use color::ColorSpaceTag;
use color::DynamicColor;
use color::Flags;
use color::Missing;
use cssparser::Parser;
use cssparser::Token;
use cssparser::match_ignore_ascii_case;

use super::ColorSyntax;
use super::ParsedColor;
use super::mix;
use super::relative::OriginChannels;
use super::system;
use crate::css::error::CSSCustomError;
use crate::css::error::CSSParseError;
use crate::css::value::NumericValue;
use crate::css::value::ParseOptions;

/// A single parsed color component, before per-channel scaling.
#[derive(Clone, Copy, Debug)]
pub(super) enum Component {
  /// The `none` keyword, or a bare channel keyword whose origin component is
  /// missing.
  None,
  Number(f64),
  /// Already divided by 100 (`50%` is `0.5`).
  Percent(f64),
  /// In degrees.
  Angle(f64),
}

impl Component {
  /// Resolves a `<number> | <percentage> | none` channel, scaling `100%` to
  /// `percent_basis`. Returns `None` for a missing component.
  #[inline]
  fn number_or_percent(
    self,
    percent_basis: f64,
  ) -> Result<Option<f64>, CSSCustomError> {
    match self {
      Self::None => Ok(None),
      Self::Number(n) => Ok(Some(n)),
      Self::Percent(p) => Ok(Some(p * percent_basis)),
      Self::Angle(_) => Err(CSSCustomError::UnexpectedNumericType),
    }
  }

  /// Resolves a `<hue> | none` channel to degrees normalized to `[0, 360)`.
  #[inline]
  fn hue(self) -> Result<Option<f64>, CSSCustomError> {
    match self {
      Self::None => Ok(None),
      Self::Number(n) => Ok(Some(n.rem_euclid(360.0))),
      Self::Angle(deg) => Ok(Some(deg.rem_euclid(360.0))),
      Self::Percent(_) => Err(CSSCustomError::UnexpectedNumericType),
    }
  }
}

/// Parses one color component: the `none` keyword, a bare channel keyword of
/// the relative color syntax, or any numeric value (including math functions
/// resolving channel keywords).
pub(super) fn parse_component<'i, 't>(
  input: &mut Parser<'i, 't>,
  allow_none: bool,
  channels: Option<&OriginChannels>,
) -> Result<Component, CSSParseError<'i>> {
  if allow_none && input.try_parse(|i| i.expect_ident_matching("none")).is_ok()
  {
    return Ok(Component::None);
  }
  if let Some(origin) = channels {
    // A bare channel keyword whose origin component is missing yields a
    // missing component; inside math functions it resolves to 0 instead.
    if let Ok(component) =
      input.try_parse(|i| -> Result<Component, CSSParseError<'i>> {
        let ident = i.expect_ident_cloned()?;
        if origin.is_missing_channel(&ident) {
          Ok(Component::None)
        } else {
          Err(i.new_custom_error(CSSCustomError::InvalidColor))
        }
      })
    {
      return Ok(component);
    }
  }
  let value = NumericValue::parse(
    input,
    ParseOptions {
      channel_keywords: channels.map(|c| c.keywords()),
      ..Default::default()
    },
  )?;
  match value {
    NumericValue::Zero => Ok(Component::Number(0.0)),
    NumericValue::Number(n) => Ok(Component::Number(n)),
    NumericValue::Percent(p) => Ok(Component::Percent(p)),
    NumericValue::Angle(angle) => Ok(Component::Angle(angle.to_degrees())),
    _ => Err(input.new_custom_error(CSSCustomError::UnexpectedNumericType)),
  }
}

/// Parses the optional modern alpha production `/ <alpha-value> | none`.
/// Returns `default` when the whole production is absent.
#[inline]
fn parse_modern_alpha<'i, 't>(
  input: &mut Parser<'i, 't>,
  channels: Option<&OriginChannels>,
  default: Component,
) -> Result<Component, CSSParseError<'i>> {
  if input.is_exhausted() {
    return Ok(default);
  }
  input.expect_delim('/')?;
  parse_component(input, true, channels)
}

/// Builds a [`ParsedColor`], turning `None` components into missing bits.
/// Non-finite values are sanitized so serialization stays well-formed.
#[inline]
pub(super) fn build_color(
  tag: ColorSpaceTag,
  channels: [Option<f64>; 3],
  alpha: Option<f64>,
  syntax: ColorSyntax,
) -> ParsedColor {
  let mut missing = Missing::EMPTY;
  let mut components = [0f32; 4];
  for (i, channel) in channels.iter().chain([&alpha]).enumerate() {
    match channel {
      Some(value) => components[i] = sanitize(*value),
      None => missing.insert(i),
    }
  }
  ParsedColor {
    color: DynamicColor {
      cs: tag,
      flags: Flags::from_missing(missing),
      components,
    },
    syntax,
  }
}

#[inline]
fn sanitize(value: f64) -> f32 {
  if value.is_nan() {
    0.0
  } else {
    value.clamp(f32::MIN as f64, f32::MAX as f64) as f32
  }
}

#[inline]
fn clamp_alpha(alpha: Option<f64>) -> Option<f64> {
  alpha.map(|a| a.clamp(0.0, 1.0))
}

/// Parses `from <color>` at the start of a color function's arguments, per
/// the relative color syntax.
#[inline]
fn try_parse_origin<'i, 't>(
  input: &mut Parser<'i, 't>,
) -> Result<Option<ParsedColor>, CSSParseError<'i>> {
  if input.try_parse(|i| i.expect_ident_matching("from")).is_ok() {
    Ok(Some(parse_color_value(input)?))
  } else {
    Ok(None)
  }
}

pub(super) fn parse_color_value<'i, 't>(
  input: &mut Parser<'i, 't>,
) -> Result<ParsedColor, CSSParseError<'i>> {
  let location = input.current_source_location();
  let token = input.next()?.clone();
  match token {
    Token::Hash(ref value) | Token::IDHash(ref value) => {
      let (r, g, b, a) = cssparser::color::parse_hash_color(value.as_bytes())
        .map_err(|()| {
        location.new_custom_error(CSSCustomError::InvalidColor)
      })?;
      Ok(ParsedColor::from_srgb8(r, g, b, a))
    }
    Token::Ident(ref ident) => {
      if ident.eq_ignore_ascii_case("transparent") {
        return Ok(ParsedColor::TRANSPARENT);
      }
      // OffscreenCanvas has no style context, so `currentcolor` computes to
      // the canvas text default of black.
      if ident.eq_ignore_ascii_case("currentcolor") {
        return Ok(ParsedColor::BLACK);
      }
      if let Some((r, g, b)) = system::lookup(ident) {
        return Ok(ParsedColor::from_srgb8(r, g, b, 1.0));
      }
      let (r, g, b) =
        cssparser::color::parse_named_color(ident).map_err(|()| {
          location.new_custom_error(CSSCustomError::InvalidColor)
        })?;
      Ok(ParsedColor::from_srgb8(r, g, b, 1.0))
    }
    Token::Function(ref name) => {
      let name = name.clone();
      match_ignore_ascii_case! { &name,
        "rgb" | "rgba" => input.parse_nested_block(parse_rgb),
        "hsl" | "hsla" => input.parse_nested_block(parse_hsl),
        "hwb" => input.parse_nested_block(parse_hwb),
        "lab" => input.parse_nested_block(|args| {
          parse_lab_like(args, ColorSpaceTag::Lab, 100.0, 125.0)
        }),
        "lch" => input.parse_nested_block(|args| {
          parse_lch_like(args, ColorSpaceTag::Lch, 100.0, 150.0)
        }),
        "oklab" => input.parse_nested_block(|args| {
          parse_lab_like(args, ColorSpaceTag::Oklab, 1.0, 0.4)
        }),
        "oklch" => input.parse_nested_block(|args| {
          parse_lch_like(args, ColorSpaceTag::Oklch, 1.0, 0.4)
        }),
        "color" => input.parse_nested_block(parse_color_function),
        "color-mix" => input.parse_nested_block(mix::parse_color_mix),
        _ => Err(location.new_custom_error(CSSCustomError::InvalidColor)),
      }
    }
    t => Err(location.new_unexpected_token_error(t)),
  }
}

/// https://www.w3.org/TR/css-color-4/#rgb-functions
fn parse_rgb<'i, 't>(
  args: &mut Parser<'i, 't>,
) -> Result<ParsedColor, CSSParseError<'i>> {
  let origin = try_parse_origin(args)?;
  let origin_channels = origin.map(|o| {
    OriginChannels::new(o.color, ColorSpaceTag::Srgb, ["r", "g", "b"], 255.0)
  });
  let channels = origin_channels.as_ref();

  let first = parse_component(args, true, channels)?;
  if channels.is_none() && args.try_parse(|i| i.expect_comma()).is_ok() {
    // Legacy syntax: components are all numbers or all percentages, and
    // `none` is not allowed.
    let second = parse_component(args, false, None)?;
    args.expect_comma()?;
    let third = parse_component(args, false, None)?;
    let alpha = if args.try_parse(|i| i.expect_comma()).is_ok() {
      parse_component(args, false, None)?
    } else {
      Component::Number(1.0)
    };
    args.expect_exhausted()?;
    let homogeneous = matches!(
      (first, second, third),
      (
        Component::Number(_),
        Component::Number(_),
        Component::Number(_)
      ) | (
        Component::Percent(_),
        Component::Percent(_),
        Component::Percent(_)
      )
    );
    if !homogeneous {
      return Err(args.new_custom_error(CSSCustomError::InvalidColor));
    }
    return finish_rgb([first, second, third], alpha, args);
  }

  let second = parse_component(args, true, channels)?;
  let third = parse_component(args, true, channels)?;
  let default_alpha = channels
    .map(|c| c.default_alpha())
    .unwrap_or(Component::Number(1.0));
  let alpha = parse_modern_alpha(args, channels, default_alpha)?;
  args.expect_exhausted()?;
  let color = finish_rgb([first, second, third], alpha, args)?;
  Ok(finish_relative(color, channels.is_some()))
}

fn finish_rgb<'i>(
  components: [Component; 3],
  alpha: Component,
  args: &Parser<'i, '_>,
) -> Result<ParsedColor, CSSParseError<'i>> {
  let mut resolved = [None; 3];
  for (i, component) in components.into_iter().enumerate() {
    // `100%` is `255`; values are stored as `0..1` and clamped like the
    // `color` crate parser does.
    resolved[i] = component
      .number_or_percent(255.0)
      .map_err(|e| args.new_custom_error(e))?
      .map(|n| (n / 255.0).clamp(0.0, 1.0));
  }
  let alpha = alpha
    .number_or_percent(1.0)
    .map_err(|e| args.new_custom_error(e))?;
  Ok(build_color(
    ColorSpaceTag::Srgb,
    resolved,
    clamp_alpha(alpha),
    ColorSyntax::Legacy,
  ))
}

/// https://www.w3.org/TR/css-color-4/#the-hsl-notation
fn parse_hsl<'i, 't>(
  args: &mut Parser<'i, 't>,
) -> Result<ParsedColor, CSSParseError<'i>> {
  let origin = try_parse_origin(args)?;
  let origin_channels = origin.map(|o| {
    OriginChannels::new(o.color, ColorSpaceTag::Hsl, ["h", "s", "l"], 1.0)
  });
  let channels = origin_channels.as_ref();

  let hue = parse_component(args, true, channels)?;
  if channels.is_none() && args.try_parse(|i| i.expect_comma()).is_ok() {
    // Legacy syntax: saturation and lightness must be percentages, and
    // `none` is not allowed.
    if matches!(hue, Component::None) {
      return Err(args.new_custom_error(CSSCustomError::InvalidColor));
    }
    let saturation = parse_component(args, false, None)?;
    args.expect_comma()?;
    let lightness = parse_component(args, false, None)?;
    let alpha = if args.try_parse(|i| i.expect_comma()).is_ok() {
      parse_component(args, false, None)?
    } else {
      Component::Number(1.0)
    };
    args.expect_exhausted()?;
    if !matches!(
      (saturation, lightness),
      (Component::Percent(_), Component::Percent(_))
    ) {
      return Err(args.new_custom_error(CSSCustomError::InvalidColor));
    }
    return finish_hsl(hue, [saturation, lightness], alpha, args);
  }

  let saturation = parse_component(args, true, channels)?;
  let lightness = parse_component(args, true, channels)?;
  let default_alpha = channels
    .map(|c| c.default_alpha())
    .unwrap_or(Component::Number(1.0));
  let alpha = parse_modern_alpha(args, channels, default_alpha)?;
  args.expect_exhausted()?;
  let color = finish_hsl(hue, [saturation, lightness], alpha, args)?;
  Ok(finish_relative(color, channels.is_some()))
}

fn finish_hsl<'i>(
  hue: Component,
  components: [Component; 2],
  alpha: Component,
  args: &Parser<'i, '_>,
) -> Result<ParsedColor, CSSParseError<'i>> {
  let hue = hue.hue().map_err(|e| args.new_custom_error(e))?;
  // Saturation and lightness are stored on a 0..100 scale; negative
  // saturation is clamped, matching the `color` crate parser.
  let saturation = components[0]
    .number_or_percent(100.0)
    .map_err(|e| args.new_custom_error(e))?
    .map(|n| n.max(0.0));
  let lightness = components[1]
    .number_or_percent(100.0)
    .map_err(|e| args.new_custom_error(e))?;
  let alpha = alpha
    .number_or_percent(1.0)
    .map_err(|e| args.new_custom_error(e))?;
  Ok(build_color(
    ColorSpaceTag::Hsl,
    [hue, saturation, lightness],
    clamp_alpha(alpha),
    ColorSyntax::Legacy,
  ))
}

/// https://www.w3.org/TR/css-color-4/#the-hwb-notation
fn parse_hwb<'i, 't>(
  args: &mut Parser<'i, 't>,
) -> Result<ParsedColor, CSSParseError<'i>> {
  let origin = try_parse_origin(args)?;
  let origin_channels = origin.map(|o| {
    OriginChannels::new(o.color, ColorSpaceTag::Hwb, ["h", "w", "b"], 1.0)
  });
  let channels = origin_channels.as_ref();

  let hue = parse_component(args, true, channels)?;
  let whiteness = parse_component(args, true, channels)?;
  let blackness = parse_component(args, true, channels)?;
  let default_alpha = channels
    .map(|c| c.default_alpha())
    .unwrap_or(Component::Number(1.0));
  let alpha = parse_modern_alpha(args, channels, default_alpha)?;
  args.expect_exhausted()?;

  let hue = hue.hue().map_err(|e| args.new_custom_error(e))?;
  let whiteness = whiteness
    .number_or_percent(100.0)
    .map_err(|e| args.new_custom_error(e))?;
  let blackness = blackness
    .number_or_percent(100.0)
    .map_err(|e| args.new_custom_error(e))?;
  let alpha = alpha
    .number_or_percent(1.0)
    .map_err(|e| args.new_custom_error(e))?;
  let color = build_color(
    ColorSpaceTag::Hwb,
    [hue, whiteness, blackness],
    clamp_alpha(alpha),
    ColorSyntax::Legacy,
  );
  Ok(finish_relative(color, channels.is_some()))
}

/// https://www.w3.org/TR/css-color-4/#specifying-lab-lch
/// `lab()` and `oklab()`, parameterized by the percentage reference ranges.
fn parse_lab_like<'i, 't>(
  args: &mut Parser<'i, 't>,
  tag: ColorSpaceTag,
  lightness_max: f64,
  chroma_basis: f64,
) -> Result<ParsedColor, CSSParseError<'i>> {
  let origin = try_parse_origin(args)?;
  let origin_channels =
    origin.map(|o| OriginChannels::new(o.color, tag, ["l", "a", "b"], 1.0));
  let channels = origin_channels.as_ref();

  let lightness = parse_component(args, true, channels)?;
  let a = parse_component(args, true, channels)?;
  let b = parse_component(args, true, channels)?;
  let default_alpha = channels
    .map(|c| c.default_alpha())
    .unwrap_or(Component::Number(1.0));
  let alpha = parse_modern_alpha(args, channels, default_alpha)?;
  args.expect_exhausted()?;

  let lightness = lightness
    .number_or_percent(lightness_max)
    .map_err(|e| args.new_custom_error(e))?
    .map(|n| n.clamp(0.0, lightness_max));
  let a = a
    .number_or_percent(chroma_basis)
    .map_err(|e| args.new_custom_error(e))?;
  let b = b
    .number_or_percent(chroma_basis)
    .map_err(|e| args.new_custom_error(e))?;
  let alpha = alpha
    .number_or_percent(1.0)
    .map_err(|e| args.new_custom_error(e))?;
  let color = build_color(
    tag,
    [lightness, a, b],
    clamp_alpha(alpha),
    ColorSyntax::Modern,
  );
  Ok(finish_relative(color, channels.is_some()))
}

/// `lch()` and `oklch()`, parameterized by the percentage reference ranges.
fn parse_lch_like<'i, 't>(
  args: &mut Parser<'i, 't>,
  tag: ColorSpaceTag,
  lightness_max: f64,
  chroma_basis: f64,
) -> Result<ParsedColor, CSSParseError<'i>> {
  let origin = try_parse_origin(args)?;
  let origin_channels =
    origin.map(|o| OriginChannels::new(o.color, tag, ["l", "c", "h"], 1.0));
  let channels = origin_channels.as_ref();

  let lightness = parse_component(args, true, channels)?;
  let chroma = parse_component(args, true, channels)?;
  let hue = parse_component(args, true, channels)?;
  let default_alpha = channels
    .map(|c| c.default_alpha())
    .unwrap_or(Component::Number(1.0));
  let alpha = parse_modern_alpha(args, channels, default_alpha)?;
  args.expect_exhausted()?;

  let lightness = lightness
    .number_or_percent(lightness_max)
    .map_err(|e| args.new_custom_error(e))?
    .map(|n| n.clamp(0.0, lightness_max));
  let chroma = chroma
    .number_or_percent(chroma_basis)
    .map_err(|e| args.new_custom_error(e))?
    .map(|n| n.max(0.0));
  let hue = hue.hue().map_err(|e| args.new_custom_error(e))?;
  let alpha = alpha
    .number_or_percent(1.0)
    .map_err(|e| args.new_custom_error(e))?;
  let color = build_color(
    tag,
    [lightness, chroma, hue],
    clamp_alpha(alpha),
    ColorSyntax::Modern,
  );
  Ok(finish_relative(color, channels.is_some()))
}

/// https://www.w3.org/TR/css-color-4/#color-function
fn parse_color_function<'i, 't>(
  args: &mut Parser<'i, 't>,
) -> Result<ParsedColor, CSSParseError<'i>> {
  let origin = try_parse_origin(args)?;
  let space = args.expect_ident_cloned()?;
  let tag = predefined_color_space(&space)
    .ok_or_else(|| args.new_custom_error(CSSCustomError::InvalidColor))?;
  let names = if matches!(tag, ColorSpaceTag::XyzD50 | ColorSpaceTag::XyzD65) {
    ["x", "y", "z"]
  } else {
    ["r", "g", "b"]
  };
  let origin_channels =
    origin.map(|o| OriginChannels::new(o.color, tag, names, 1.0));
  let channels = origin_channels.as_ref();

  let c0 = parse_component(args, true, channels)?;
  let c1 = parse_component(args, true, channels)?;
  let c2 = parse_component(args, true, channels)?;
  let default_alpha = channels
    .map(|c| c.default_alpha())
    .unwrap_or(Component::Number(1.0));
  let alpha = parse_modern_alpha(args, channels, default_alpha)?;
  args.expect_exhausted()?;

  let mut resolved = [None; 3];
  for (i, component) in [c0, c1, c2].into_iter().enumerate() {
    resolved[i] = component
      .number_or_percent(1.0)
      .map_err(|e| args.new_custom_error(e))?;
  }
  let alpha = alpha
    .number_or_percent(1.0)
    .map_err(|e| args.new_custom_error(e))?;
  Ok(build_color(
    tag,
    resolved,
    clamp_alpha(alpha),
    ColorSyntax::Modern,
  ))
}

/// For the relative color syntax, `rgb()`, `hsl()` and `hwb()` results are
/// resolved into `color(srgb ...)` and every result serializes in the modern
/// form, matching browsers.
#[inline]
fn finish_relative(mut color: ParsedColor, relative: bool) -> ParsedColor {
  if relative {
    if matches!(color.color.cs, ColorSpaceTag::Hsl | ColorSpaceTag::Hwb) {
      color.color = color.color.convert(ColorSpaceTag::Srgb);
    }
    color.syntax = ColorSyntax::Modern;
  }
  color
}

/// https://www.w3.org/TR/css-color-4/#predefined
#[inline]
fn predefined_color_space(ident: &str) -> Option<ColorSpaceTag> {
  Some(match_ignore_ascii_case! { ident,
    "srgb" => ColorSpaceTag::Srgb,
    "srgb-linear" => ColorSpaceTag::LinearSrgb,
    "display-p3" => ColorSpaceTag::DisplayP3,
    "a98-rgb" => ColorSpaceTag::A98Rgb,
    "prophoto-rgb" => ColorSpaceTag::ProphotoRgb,
    "rec2020" => ColorSpaceTag::Rec2020,
    "xyz" | "xyz-d65" => ColorSpaceTag::XyzD65,
    "xyz-d50" => ColorSpaceTag::XyzD50,
    _ => return None,
  })
}
