// Copyright 2018-2026 the Deno authors. MIT license.

//! CSS `<color>` parsing and serialization for canvas.
//!
//! The syntax layer (tokens to color components, including `calc()`,
//! `color-mix()` and the relative color syntax) is implemented here on top of
//! `cssparser` and [`super::value`], while color space conversion and
//! interpolation math are delegated to the `color` crate.
//!
//! https://www.w3.org/TR/css-color-4/
//! https://www.w3.org/TR/css-color-5/

mod mix;
mod parse;
mod relative;
mod serialize;
mod system;

use color::ColorSpaceTag;
use color::DynamicColor;
use color::Flags;
use color::Missing;
use cssparser::Parser;
use cssparser::ParserInput;
pub use peniko::Color;
use vello::peniko;

pub use self::serialize::serialize_color_for_canvas;
pub use self::system::is_css_system_color;
use super::error::CSSCustomError;
use super::error::CSSParseError;

/// How a color was written, which drives canvas getter serialization.
/// https://html.spec.whatwg.org/multipage/canvas.html#serialisation-of-a-color
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorSyntax {
  /// Named colors, hex, `rgb()`/`rgba()`, `hsl()`/`hsla()`, `hwb()`,
  /// `transparent`, `currentcolor` and system colors. Serialized as
  /// `#rrggbb` / `rgba()`.
  Legacy,
  /// `lab()`, `lch()`, `oklab()`, `oklch()`, `color()`, `color-mix()` and the
  /// relative color syntax. Serialized in their modern form.
  Modern,
}

/// A parsed CSS `<color>` retaining its color space, missing components and
/// enough syntax information for canvas serialization.
#[derive(Clone, Copy, Debug)]
pub struct ParsedColor {
  pub color: DynamicColor,
  pub syntax: ColorSyntax,
}

impl ParsedColor {
  pub const BLACK: Self = Self::from_srgb([0.0, 0.0, 0.0, 1.0]);
  pub const TRANSPARENT: Self = Self::from_srgb([0.0, 0.0, 0.0, 0.0]);

  #[inline]
  const fn from_srgb(components: [f32; 4]) -> Self {
    Self {
      color: DynamicColor {
        cs: ColorSpaceTag::Srgb,
        flags: Flags::from_missing(Missing::EMPTY),
        components,
      },
      syntax: ColorSyntax::Legacy,
    }
  }

  #[inline]
  fn from_srgb8(r: u8, g: u8, b: u8, a: f32) -> Self {
    Self::from_srgb([r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, a])
  }

  /// Quantizes to an 8-bit sRGB color for rendering. Missing components are
  /// interpreted as 0.
  #[inline]
  pub fn to_srgb8(&self) -> Color {
    let srgb = self.color.to_alpha_color::<color::Srgb>();
    let [r, g, b, a] = srgb.components;
    Color::from_rgba8(
      (r.clamp(0.0, 1.0) * 255.0).round() as u8,
      (g.clamp(0.0, 1.0) * 255.0).round() as u8,
      (b.clamp(0.0, 1.0) * 255.0).round() as u8,
      (a.clamp(0.0, 1.0) * 255.0).round() as u8,
    )
  }

  #[inline]
  pub fn is_transparent(&self) -> bool {
    self.to_srgb8().to_rgba8().a == 0
  }
}

/// Parses a whole string as a CSS `<color>`.
pub fn parse_css_color(s: &str) -> Result<ParsedColor, CSSCustomError> {
  let mut input = ParserInput::new(s);
  let mut parser = Parser::new(&mut input);
  let color =
    parse_color_value(&mut parser).map_err(|_| CSSCustomError::InvalidColor)?;
  parser
    .expect_exhausted()
    .map_err(|_| CSSCustomError::InvalidColor)?;
  Ok(color)
}

/// Parses a single CSS `<color>` value from an ongoing parser, for use inside
/// larger productions such as `drop-shadow()`, `color-mix()` and the relative
/// color syntax.
pub fn parse_color_value<'i, 't>(
  input: &mut Parser<'i, 't>,
) -> Result<ParsedColor, CSSParseError<'i>> {
  parse::parse_color_value(input)
}

#[cfg(test)]
mod tests {
  use super::*;

  fn parse(s: &str) -> ParsedColor {
    parse_css_color(s).unwrap_or_else(|_| panic!("failed to parse: {s}"))
  }

  fn rgba8(s: &str) -> (u8, u8, u8, u8) {
    let c = parse(s).to_srgb8().to_rgba8();
    (c.r, c.g, c.b, c.a)
  }

  fn serialize(s: &str) -> String {
    serialize_color_for_canvas(&parse(s))
  }

  #[track_caller]
  fn assert_invalid(s: &str) {
    assert!(parse_css_color(s).is_err(), "expected invalid: {s}");
  }

  #[test]
  fn named_and_hex() {
    assert_eq!(rgba8("red"), (255, 0, 0, 255));
    assert_eq!(rgba8("ReD"), (255, 0, 0, 255));
    assert_eq!(rgba8("grey"), (128, 128, 128, 255));
    assert_eq!(rgba8("rebeccapurple"), (102, 51, 153, 255));
    assert_eq!(rgba8("#f00"), (255, 0, 0, 255));
    assert_eq!(rgba8("#ff0000"), (255, 0, 0, 255));
    assert_eq!(rgba8("#ff000080"), (255, 0, 0, 128));
    assert_eq!(rgba8("transparent"), (0, 0, 0, 0));
    assert_eq!(rgba8("currentcolor"), (0, 0, 0, 255));
    assert_eq!(rgba8("  blue  "), (0, 0, 255, 255));
    // 4-digit hex is `#rgba`.
    assert_eq!(rgba8("#ff00"), (255, 255, 0, 0));
    assert_invalid("notacolor");
    assert_invalid("#ff000");
    assert_invalid("");
  }

  #[test]
  fn system_colors() {
    assert_eq!(rgba8("Canvas"), (255, 255, 255, 255));
    assert_eq!(rgba8("CanvasText"), (0, 0, 0, 255));
    // Deprecated system colors map to their modern equivalents.
    assert_eq!(rgba8("ThreeDDarkShadow"), (118, 118, 118, 255));
    assert_eq!(rgba8("WindowText"), (0, 0, 0, 255));
  }

  #[test]
  fn rgb_legacy() {
    assert_eq!(rgba8("rgb(255, 0, 0)"), (255, 0, 0, 255));
    assert_eq!(rgba8("rgba(255, 0, 0, 0.5)"), (255, 0, 0, 128));
    assert_eq!(rgba8("rgb(100%, 0%, 50%)"), (255, 0, 128, 255));
    assert_eq!(rgba8("rgb(300, -10, 0)"), (255, 0, 0, 255));
    assert_eq!(rgba8("rgba(0, 0, 255, 50%)"), (0, 0, 255, 128));
    // Legacy syntax rejects mixed numbers and percentages.
    assert_invalid("rgb(255, 0%, 0)");
    assert_invalid("rgb(100%, 0, 0)");
    // Legacy syntax rejects `none`.
    assert_invalid("rgb(none, 0, 0)");
    assert_invalid("rgb(0, 0, none)");
    assert_invalid("rgba(0, 0, 0, none)");
    assert_invalid("rgb(0, 0)");
    assert_invalid("rgb(0, 0, 0, 0, 0)");
  }

  #[test]
  fn rgb_modern() {
    assert_eq!(rgba8("rgb(255 0 0)"), (255, 0, 0, 255));
    assert_eq!(rgba8("rgb(255 0 0 / 0.5)"), (255, 0, 0, 128));
    assert_eq!(rgba8("rgb(100% 0% 0% / 50%)"), (255, 0, 0, 128));
    assert_eq!(rgba8("rgb(none 255 0)"), (0, 255, 0, 255));
    assert_eq!(rgba8("rgb(50% none none / none)"), (128, 0, 0, 0));
    // Mixed numbers and percentages are allowed in the modern syntax.
    assert_eq!(rgba8("rgb(100% 0 0)"), (255, 0, 0, 255));
    assert_invalid("rgb(255 0 0 0.5)");
    assert_invalid("rgb(255 0 / 0.5)");
  }

  #[test]
  fn rgb_calc() {
    assert_eq!(rgba8("rgb(calc(200 + 55) 0 0)"), (255, 0, 0, 255));
    assert_eq!(rgba8("rgb(calc(50% * 2) 0 0)"), (255, 0, 0, 255));
    assert_eq!(rgba8("rgba(0, 0, 255, calc(1 / 2))"), (0, 0, 255, 128));
    assert_eq!(rgba8("rgb(min(255, 300) 0 0)"), (255, 0, 0, 255));
    // calc() preserves number vs percentage for the legacy homogeneity rule.
    assert_invalid("rgb(calc(100%), 0, 0)");
  }

  #[test]
  fn hsl() {
    assert_eq!(rgba8("hsl(120, 100%, 50%)"), (0, 255, 0, 255));
    assert_eq!(rgba8("hsl( -240 , 100% , 50% )"), (0, 255, 0, 255));
    assert_eq!(rgba8("hsl(120.0, 100%, 50%)"), (0, 255, 0, 255));
    assert_eq!(rgba8("hsl(120deg, 100%, 50%)"), (0, 255, 0, 255));
    assert_eq!(rgba8("hsl(0.5turn, 100%, 50%)"), (0, 255, 255, 255));
    assert_eq!(rgba8("hsla(120, 100%, 50%, 0.5)"), (0, 255, 0, 128));
    assert_eq!(rgba8("hsl(120 100% 50%)"), (0, 255, 0, 255));
    assert_eq!(rgba8("hsl(120 100 50 / 50%)"), (0, 255, 0, 128));
    assert_eq!(rgba8("hsl(none none none)"), (0, 0, 0, 255));
    // Legacy hsl() requires percentages for saturation and lightness.
    assert_invalid("hsl(120, 100, 50)");
    assert_invalid("hsl(120, 100%, 50)");
    assert_invalid("hsl(120, 100, 50%)");
    assert_invalid("hsl(120%, 100%, 50%)");
  }

  #[test]
  fn hwb() {
    assert_eq!(rgba8("hwb(120 0% 0%)"), (0, 255, 0, 255));
    assert_eq!(rgba8("hwb(120 100% 100%)"), (128, 128, 128, 255));
    assert_eq!(rgba8("hwb(120 0 0 / 0.5)"), (0, 255, 0, 128));
    assert_invalid("hwb(120, 0%, 0%)");
  }

  #[test]
  fn lab_lch_oklab_oklch() {
    assert_eq!(serialize("lab(50 40 59.5)"), "lab(50 40 59.5)");
    assert_eq!(serialize("lab(50% 100% -100%)"), "lab(50 125 -125)");
    assert_eq!(serialize("lab(150 0 0)"), "lab(100 0 0)");
    assert_eq!(serialize("lch(52.2% 72.2 50)"), "lch(52.2 72.2 50)");
    assert_eq!(serialize("lch(52.2 -10 50)"), "lch(52.2 0 50)");
    assert_eq!(serialize("oklab(0.5 0.1 -0.1)"), "oklab(0.5 0.1 -0.1)");
    assert_eq!(serialize("oklab(50% 100% 0)"), "oklab(0.5 0.4 0)");
    assert_eq!(serialize("oklch(0.5 0.2 120)"), "oklch(0.5 0.2 120)");
    assert_eq!(serialize("oklch(none none none)"), "oklch(none none none)");
    assert_invalid("lab(50, 40, 59.5)");
  }

  #[test]
  fn color_function() {
    assert_eq!(serialize("color(srgb 1 0 0)"), "color(srgb 1 0 0)");
    assert_eq!(serialize("color(srgb 100% 0% 50%)"), "color(srgb 1 0 0.5)");
    assert_eq!(
      serialize("color(display-p3 1 0 0 / 0.5)"),
      "color(display-p3 1 0 0 / 0.5)"
    );
    assert_eq!(serialize("color(xyz 0 0 0)"), "color(xyz-d65 0 0 0)");
    assert_eq!(
      serialize("color(srgb-linear 0.5 0.5 0.5)"),
      "color(srgb-linear 0.5 0.5 0.5)"
    );
    assert_eq!(serialize("color(srgb none 0 1)"), "color(srgb none 0 1)");
    assert_invalid("color(notaspace 0 0 0)");
    assert_invalid("color(srgb 0 0)");
  }

  #[test]
  fn color_mix() {
    assert_eq!(
      serialize("color-mix(in srgb, red, blue)"),
      "color(srgb 0.5 0 0.5)"
    );
    assert_eq!(
      serialize("color-mix(in srgb, red, color(srgb 1 0 0))"),
      "color(srgb 1 0 0)"
    );
    assert_eq!(
      serialize("color-mix(in srgb, red 25%, blue)"),
      "color(srgb 0.25 0 0.75)"
    );
    assert_eq!(
      serialize("color-mix(in srgb, red 30%, blue 30%)"),
      "color(srgb 0.5 0 0.5 / 0.6)"
    );
    assert_eq!(
      serialize("color-mix(in oklab, red, blue)"),
      serialize("color-mix(in oklab, blue, red)")
    );
    // Polar interpolation with an explicit hue direction.
    assert_eq!(
      rgba8("color-mix(in hsl longer hue, hsl(0 100% 50%), hsl(90 100% 50%))"),
      rgba8("hsl(225 100% 50%)")
    );
    // color-mix() in a polar space serializes like the legacy form.
    assert_eq!(
      serialize("color-mix(in hsl, hsl(0 100% 50%), hsl(120 100% 50%))"),
      "#ffff00"
    );
    assert_invalid("color-mix(red, blue)");
    assert_invalid("color-mix(in srgb, red 0%, blue 0%)");
    assert_invalid("color-mix(in srgb longer hue, red, blue)");
    assert_invalid("color-mix(in srgb, red, blue, green)");
  }

  #[test]
  fn relative_color() {
    assert_eq!(serialize("rgb(from red g r b)"), "color(srgb 0 1 0)");
    assert_eq!(
      serialize(
        "color(from color(srgb 0.25 0.5 0.75 / 0.5) srgb r g b / alpha)"
      ),
      "color(srgb 0.25 0.5 0.75 / 0.5)"
    );
    // Omitted alpha defaults to the origin color's alpha.
    assert_eq!(
      serialize("rgb(from rgb(255 0 0 / 0.5) r g b)"),
      "color(srgb 1 0 0 / 0.5)"
    );
    assert_eq!(
      serialize("rgb(from red calc(r / 2) g b)"),
      "color(srgb 0.5 0 0)"
    );
    assert_eq!(serialize("hsl(from red h s l)"), "color(srgb 1 0 0)");
    assert_eq!(
      serialize("hsl(from red calc(h + 120) s l)"),
      "color(srgb 0 1 0)"
    );
    assert_eq!(serialize("hwb(from red h w b)"), "color(srgb 1 0 0)");
    assert_eq!(serialize("lab(from lab(50 40 60) l a b)"), "lab(50 40 60)");
    assert_eq!(
      serialize("oklch(from oklch(0.5 0.2 120) l c h)"),
      "oklch(0.5 0.2 120)"
    );
    assert_eq!(
      serialize("color(from red xyz-d65 x y z)"),
      serialize("color(from red xyz x y z)")
    );
    // Nested relative colors and color-mix() as origin.
    assert_eq!(
      serialize("rgb(from rgb(from red g r b) g r b)"),
      "color(srgb 1 0 0)"
    );
    assert_eq!(
      serialize("rgb(from color-mix(in srgb, red, red) r g b)"),
      "color(srgb 1 0 0)"
    );
    // Relative colors always use the modern grammar.
    assert_invalid("rgb(from red g, r, b)");
    assert_invalid("rgb(from red x y z)");
  }

  #[test]
  fn serialize_legacy() {
    assert_eq!(serialize("red"), "#ff0000");
    assert_eq!(serialize("#ff0000ff"), "#ff0000");
    assert_eq!(serialize("rgb(255 0 0)"), "#ff0000");
    assert_eq!(serialize("hsl(120 100% 50%)"), "#00ff00");
    assert_eq!(serialize("transparent"), "rgba(0, 0, 0, 0)");
    assert_eq!(serialize("rgba(255, 0, 0, 0.5)"), "rgba(255, 0, 0, 0.5)");
    assert_eq!(serialize("rgba(255, 0, 0, 0.25)"), "rgba(255, 0, 0, 0.25)");
    assert_eq!(serialize("rgba(0, 0, 255, 0)"), "rgba(0, 0, 255, 0)");
  }

  #[test]
  fn serialize_modern() {
    assert_eq!(serialize("lab(50 0 0 / 0.5)"), "lab(50 0 0 / 0.5)");
    assert_eq!(serialize("lab(50 0 0 / none)"), "lab(50 0 0 / none)");
    assert_eq!(
      serialize("color(srgb 0.125 0.25 0.5)"),
      "color(srgb 0.125 0.25 0.5)"
    );
  }
}
