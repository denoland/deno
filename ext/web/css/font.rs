// Copyright 2018-2026 the Deno authors. MIT license.

use cssparser::Token;
use cssparser::match_ignore_ascii_case;
use deno_core::WebIDL;

use super::error::CSSCustomError;
use super::error::CSSParseError;
use super::value::Length;
use super::value::NumericValue;
use super::value::ParseOptions;
use super::value::Parser;
use super::value::ParserInput;

/// Parsed CSS font shorthand.
/// https://html.spec.whatwg.org/multipage/canvas.html#dom-context-2d-font
/// https://drafts.csswg.org/css-fonts-4/#font-prop
/// Values for `CanvasTextDrawingStyles.direction`.
#[derive(WebIDL, Clone, Copy, Debug, Default, PartialEq)]
#[webidl(enum)]
pub enum TextDirection {
  #[default]
  Inherit,
  Ltr,
  Rtl,
}

/// Values for `CanvasTextDrawingStyles.fontKerning`.
#[derive(WebIDL, Clone, Copy, Debug, Default, PartialEq)]
#[webidl(enum)]
pub enum FontKerning {
  #[default]
  Auto,
  Normal,
  None,
}

/// Values for `CanvasTextDrawingStyles.fontVariantCaps`.
#[derive(WebIDL, Clone, Copy, Debug, Default)]
#[cfg_attr(test, derive(PartialEq))]
#[webidl(enum)]
pub enum FontVariantCaps {
  #[default]
  Normal,
  SmallCaps,
  AllSmallCaps,
  PetiteCaps,
  AllPetiteCaps,
  Unicase,
  TitlingCaps,
}

/// Values for `CanvasTextDrawingStyles.textRendering`.
#[derive(WebIDL, Clone, Copy, Debug, Default)]
#[webidl(enum)]
pub enum TextRendering {
  #[default]
  Auto,
  #[webidl(rename = "optimizeSpeed")]
  OptimizeSpeed,
  #[webidl(rename = "optimizeLegibility")]
  OptimizeLegibility,
  #[webidl(rename = "geometricPrecision")]
  GeometricPrecision,
}

/// CSS font-style values.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum CssFontStyle {
  #[default]
  Normal,
  Italic,
  Oblique,
}

/// CSS font-stretch values.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum CssFontStretch {
  UltraCondensed,
  ExtraCondensed,
  Condensed,
  SemiCondensed,
  #[default]
  Normal,
  SemiExpanded,
  Expanded,
  ExtraExpanded,
  UltraExpanded,
}

impl CssFontStyle {
  pub fn to_parley(self) -> parley::FontStyle {
    match self {
      CssFontStyle::Normal => parley::FontStyle::Normal,
      CssFontStyle::Italic => parley::FontStyle::Italic,
      CssFontStyle::Oblique => parley::FontStyle::Oblique(None),
    }
  }
}

impl CssFontStretch {
  pub fn to_parley(self) -> parley::FontWidth {
    match self {
      CssFontStretch::UltraCondensed => parley::FontWidth::ULTRA_CONDENSED,
      CssFontStretch::ExtraCondensed => parley::FontWidth::EXTRA_CONDENSED,
      CssFontStretch::Condensed => parley::FontWidth::CONDENSED,
      CssFontStretch::SemiCondensed => parley::FontWidth::SEMI_CONDENSED,
      CssFontStretch::Normal => parley::FontWidth::NORMAL,
      CssFontStretch::SemiExpanded => parley::FontWidth::SEMI_EXPANDED,
      CssFontStretch::Expanded => parley::FontWidth::EXPANDED,
      CssFontStretch::ExtraExpanded => parley::FontWidth::EXTRA_EXPANDED,
      CssFontStretch::UltraExpanded => parley::FontWidth::ULTRA_EXPANDED,
    }
  }
}

/// Parses `letterSpacing` / `wordSpacing`.
pub fn parse_css_spacing(s: &str) -> Option<Length> {
  let mut input = ParserInput::new(s.trim());
  let mut parser = Parser::new(&mut input);
  let value = NumericValue::parse(
    &mut parser,
    ParseOptions {
      em_base: Some(EM_BASE_PX),
    },
  )
  .ok()?;
  // The literal `0` is a valid <length>; everything else must be a length.
  let length = value.expect_length(true).ok()?;
  if !parser.is_exhausted() {
    return None;
  }
  Some(length)
}

#[derive(Clone, Debug)]
pub struct FontState {
  pub style: CssFontStyle,
  pub weight: u16,
  pub stretch: CssFontStretch,
  pub size: f32,
  pub line_height: Option<f32>,
  pub families: Vec<String>,
  pub direction: TextDirection,
  pub font_kerning: FontKerning,
  pub font_variant_caps: FontVariantCaps,
  /// CSS letter-spacing value (default `0px`).
  pub letter_spacing: Length,
  /// CSS word-spacing value (default `0px`).
  pub word_spacing: Length,
  pub text_rendering: TextRendering,
}

impl Default for FontState {
  fn default() -> Self {
    Self {
      style: CssFontStyle::Normal,
      weight: 400,
      stretch: CssFontStretch::Normal,
      size: 10.0,
      line_height: None,
      families: vec!["sans-serif".to_string()],
      direction: TextDirection::default(),
      font_kerning: FontKerning::default(),
      font_variant_caps: FontVariantCaps::default(),
      letter_spacing: Length::zero(),
      word_spacing: Length::zero(),
      text_rendering: TextRendering::default(),
    }
  }
}

impl FontState {
  /// Returns the CSS font shorthand string for this state.
  pub fn to_css_string(&self) -> String {
    let style = match self.style {
      CssFontStyle::Normal => String::new(),
      CssFontStyle::Italic => "italic ".to_string(),
      CssFontStyle::Oblique => "oblique ".to_string(),
    };
    let variant = match self.font_variant_caps {
      FontVariantCaps::SmallCaps => "small-caps ",
      _ => "",
    };
    let weight = if self.weight != 400 {
      format!("{} ", self.weight)
    } else {
      String::new()
    };
    let stretch = match self.stretch {
      CssFontStretch::Normal => String::new(),
      CssFontStretch::UltraCondensed => "ultra-condensed ".to_string(),
      CssFontStretch::ExtraCondensed => "extra-condensed ".to_string(),
      CssFontStretch::Condensed => "condensed ".to_string(),
      CssFontStretch::SemiCondensed => "semi-condensed ".to_string(),
      CssFontStretch::SemiExpanded => "semi-expanded ".to_string(),
      CssFontStretch::Expanded => "expanded ".to_string(),
      CssFontStretch::ExtraExpanded => "extra-expanded ".to_string(),
      CssFontStretch::UltraExpanded => "ultra-expanded ".to_string(),
    };
    let size = if self.size == self.size.floor() {
      format!("{}px", self.size as u32)
    } else {
      format!("{:.2}px", self.size)
    };
    let families = self
      .families
      .iter()
      .map(|f| serialize_font_family(f))
      .collect::<Vec<_>>()
      .join(", ");
    format!("{style}{variant}{weight}{stretch}{size} {families}")
  }
}

/// Serializes a font family name.
fn serialize_font_family(family: &str) -> String {
  let valid_unquoted = !family.is_empty()
    && family.split(' ').all(|part| {
      let mut chars = part.chars();
      let head_valid = match chars.next() {
        Some('-') => matches!(chars.next(), Some(c) if c.is_ascii_alphabetic() || c == '_' || c == '-' || !c.is_ascii()),
        Some(c) => c.is_ascii_alphabetic() || c == '_' || !c.is_ascii(),
        None => false,
      };
      head_valid
        && chars
          .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || !c.is_ascii())
    });
  if valid_unquoted {
    family.to_string()
  } else {
    format!("\"{}\"", family.replace('\\', "\\\\").replace('"', "\\\""))
  }
}

/// Parses a CSS `font-style` value.
/// https://drafts.csswg.org/css-fonts-4/#font-style-prop
pub fn parse_css_style(s: &str) -> Option<CssFontStyle> {
  let s = s.trim();
  match s {
    "italic" => Some(CssFontStyle::Italic),
    "oblique" => Some(CssFontStyle::Oblique),
    "normal" => Some(CssFontStyle::Normal),
    _ => None,
  }
}

/// Parses a CSS `font-weight` value.
/// https://drafts.csswg.org/css-fonts-4/#font-weight-prop
pub fn parse_css_weight(s: &str) -> Option<u16> {
  let s = s.trim();
  match s {
    "normal" => Some(400),
    "bold" => Some(700),
    s => s.parse::<u16>().ok().filter(|&w| (1..=1000).contains(&w)),
  }
}

/// Parses a CSS `font-stretch` keyword.
/// https://drafts.csswg.org/css-fonts-4/#font-stretch-prop
pub fn parse_css_stretch(s: &str) -> Option<CssFontStretch> {
  let s = s.trim();
  match s {
    "ultra-condensed" => Some(CssFontStretch::UltraCondensed),
    "extra-condensed" => Some(CssFontStretch::ExtraCondensed),
    "condensed" => Some(CssFontStretch::Condensed),
    "semi-condensed" => Some(CssFontStretch::SemiCondensed),
    "normal" => Some(CssFontStretch::Normal),
    "semi-expanded" => Some(CssFontStretch::SemiExpanded),
    "expanded" => Some(CssFontStretch::Expanded),
    "extra-expanded" => Some(CssFontStretch::ExtraExpanded),
    "ultra-expanded" => Some(CssFontStretch::UltraExpanded),
    _ => None,
  }
}

/// Returns the CSS string representation of a font style.
pub fn style_to_css_str(style: CssFontStyle) -> &'static str {
  match style {
    CssFontStyle::Normal => "normal",
    CssFontStyle::Italic => "italic",
    CssFontStyle::Oblique => "oblique",
  }
}

/// Returns the CSS string representation of a font stretch.
pub fn stretch_to_css_str(stretch: CssFontStretch) -> &'static str {
  match stretch {
    CssFontStretch::UltraCondensed => "ultra-condensed",
    CssFontStretch::ExtraCondensed => "extra-condensed",
    CssFontStretch::Condensed => "condensed",
    CssFontStretch::SemiCondensed => "semi-condensed",
    CssFontStretch::Normal => "normal",
    CssFontStretch::SemiExpanded => "semi-expanded",
    CssFontStretch::Expanded => "expanded",
    CssFontStretch::ExtraExpanded => "extra-expanded",
    CssFontStretch::UltraExpanded => "ultra-expanded",
  }
}

/// Parses a CSS font shorthand string into a [`FontState`].
///
/// Grammar (simplified):
/// ```text
/// [font-style || font-variant-css2 || font-weight || font-stretch-css3]?
///   font-size[/line-height]? font-family-list
/// ```
///
/// https://drafts.csswg.org/css-fonts-4/#font-prop
pub fn parse_css_font(s: &str) -> Option<FontState> {
  let s = s.trim();

  // Reject system font keywords and CSS-wide keywords (case-insensitive per spec).
  match s.to_ascii_lowercase().as_str() {
    "caption" | "icon" | "menu" | "message-box" | "small-caption"
    | "status-bar" | "inherit" | "initial" | "revert" | "revert-layer"
    | "unset" => return None,
    _ => {}
  }

  let mut input = ParserInput::new(s);
  let mut parser = Parser::new(&mut input);
  parse_css_font_inner(&mut parser)
}

/// Canvas default font-size base.
const EM_BASE_PX: f64 = 10.0;

/// Result of attempting to parse one optional prefix keyword in the font shorthand.
enum PrefixValue {
  Style(CssFontStyle),
  Weight(u16),
  Stretch(CssFontStretch),
  /// `small-caps`, the only font-variant value allowed in the shorthand.
  SmallCaps,
  Neutral,
}

/// Tries to parse one optional font shorthand prefix keyword.
fn parse_prefix<'i, 't>(
  p: &mut Parser<'i, 't>,
) -> Result<PrefixValue, CSSParseError<'i>> {
  let tok = p.next()?.clone();
  match &tok {
    Token::Ident(ident) => {
      match_ignore_ascii_case! { ident,
        "italic" => Ok(PrefixValue::Style(CssFontStyle::Italic)),
        "oblique" => Ok(PrefixValue::Style(CssFontStyle::Oblique)),
        "bold" => Ok(PrefixValue::Weight(700)),
        "normal" => Ok(PrefixValue::Neutral),
        "small-caps" => Ok(PrefixValue::SmallCaps),
        _ => parse_css_stretch(ident)
          .map(PrefixValue::Stretch)
          .ok_or_else(|| p.new_custom_error(CSSCustomError::InvalidDimension)),
      }
    }
    Token::Number {
      int_value: Some(w), ..
    } => u16::try_from(*w)
      .ok()
      .filter(|&w| (1..=1000).contains(&w))
      .map(PrefixValue::Weight)
      .ok_or_else(|| p.new_custom_error(CSSCustomError::InvalidDimension)),
    _ => Err(p.new_custom_error(CSSCustomError::InvalidDimension)),
  }
}

fn parse_css_font_inner<'i, 't>(
  input: &mut Parser<'i, 't>,
) -> Option<FontState> {
  let mut style = CssFontStyle::Normal;
  let mut weight: u16 = 400;
  let mut stretch = CssFontStretch::Normal;
  let mut variant_caps = FontVariantCaps::Normal;

  // Parse optional leading keywords (style, weight, stretch may appear in any order).
  for _ in 0..4 {
    match input.try_parse(parse_prefix) {
      Ok(PrefixValue::Style(s)) => style = s,
      Ok(PrefixValue::Weight(w)) => weight = w,
      Ok(PrefixValue::Stretch(s)) => stretch = s,
      Ok(PrefixValue::SmallCaps) => variant_caps = FontVariantCaps::SmallCaps,
      Ok(PrefixValue::Neutral) => {}
      Err(_) => break,
    }
  }

  // Parse font-size (<length> | <percentage>).
  let size_value = input
    .try_parse(|p| {
      NumericValue::parse(
        p,
        ParseOptions {
          em_base: Some(EM_BASE_PX),
        },
      )
    })
    .ok()?;
  let size = match size_value {
    NumericValue::Length(l) => l.resolve_to_pixels(EM_BASE_PX) as f32,
    NumericValue::Percent(p) => (p * EM_BASE_PX) as f32,
    NumericValue::Zero => 0.0f32,
    _ => return None,
  };

  // Parse optional /line-height.
  let line_height: Option<f32> = input
    .try_parse(|p| {
      let tok = p.next()?.clone();
      if !matches!(tok, Token::Delim('/')) {
        return Err(p.new_custom_error(CSSCustomError::InvalidDimension));
      }
      let lh_value = NumericValue::parse(
        p,
        ParseOptions {
          em_base: Some(EM_BASE_PX),
        },
      )?;
      match lh_value {
        NumericValue::Number(n) => Ok((n * EM_BASE_PX) as f32),
        NumericValue::Length(l) => Ok(l.resolve_to_pixels(EM_BASE_PX) as f32),
        NumericValue::Percent(pct) => Ok((pct * EM_BASE_PX) as f32),
        NumericValue::Zero => Ok(0.0f32),
        _ => Err(p.new_custom_error(CSSCustomError::UnexpectedNumericType)),
      }
    })
    .ok();

  // Parse font-family list (required).
  if input.is_exhausted() {
    return None;
  }
  let families = parse_font_family_list(input)?;
  if families.is_empty() {
    return None;
  }

  Some(FontState {
    style,
    weight,
    stretch,
    size,
    line_height,
    families,
    font_variant_caps: variant_caps,
    ..FontState::default()
  })
}

fn parse_font_family_list<'i, 't>(
  input: &mut Parser<'i, 't>,
) -> Option<Vec<String>> {
  let mut families = Vec::new();
  loop {
    let family = parse_one_font_family(input)?;
    families.push(family);
    if input.try_parse(|p| p.expect_comma()).is_err() {
      break;
    }
  }
  if families.is_empty() || !input.is_exhausted() {
    return None;
  }
  Some(families)
}

/// Case-insensitive generic font family keywords.
const GENERIC_FAMILIES: &[&str] = &[
  "serif",
  "sans-serif",
  "cursive",
  "fantasy",
  "monospace",
  "system-ui",
  "math",
  "ui-serif",
  "ui-sans-serif",
  "ui-monospace",
  "ui-rounded",
];

/// Reserved unquoted font family names.
fn is_reserved_family_ident(ident: &str) -> bool {
  ident.eq_ignore_ascii_case("inherit")
    || ident.eq_ignore_ascii_case("initial")
    || ident.eq_ignore_ascii_case("unset")
    || ident.eq_ignore_ascii_case("revert")
    || ident.eq_ignore_ascii_case("revert-layer")
    || ident.eq_ignore_ascii_case("default")
}

fn parse_one_font_family<'i, 't>(input: &mut Parser<'i, 't>) -> Option<String> {
  let tok = input.next().ok()?.clone();
  match tok {
    Token::QuotedString(s) => Some(s.as_ref().to_string()),
    Token::Ident(first) => {
      let mut parts = vec![first.as_ref().to_string()];
      // Collect additional idents for unquoted multi-word family names.
      loop {
        let state = input.state();
        match input.next().cloned() {
          Ok(Token::Ident(s)) => parts.push(s.as_ref().to_string()),
          _ => {
            input.reset(&state);
            break;
          }
        }
      }
      if parts.iter().any(|p| is_reserved_family_ident(p)) {
        return None;
      }
      if parts.len() == 1 {
        let lower = parts[0].to_ascii_lowercase();
        if GENERIC_FAMILIES.contains(&lower.as_str()) {
          return Some(lower);
        }
      }
      Some(parts.join(" "))
    }
    _ => None,
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn parse(s: &str) -> Option<FontState> {
    parse_css_font(s)
  }

  #[test]
  fn basic_size_and_family() {
    let f = parse("16px serif").unwrap();
    assert_eq!(f.size, 16.0);
    assert_eq!(f.families, vec!["serif"]);
    assert_eq!(f.weight, 400);
    assert_eq!(f.style, CssFontStyle::Normal);
  }

  #[test]
  fn bold_keyword() {
    let f = parse("bold 14px Arial").unwrap();
    assert_eq!(f.weight, 700);
    assert_eq!(f.size, 14.0);
    assert_eq!(f.families, vec!["Arial"]);
  }

  #[test]
  fn numeric_weight() {
    let f = parse("300 14px Arial").unwrap();
    assert_eq!(f.weight, 300);
  }

  #[test]
  fn italic_style() {
    let f = parse("italic 12px sans-serif").unwrap();
    assert_eq!(f.style, CssFontStyle::Italic);
  }

  #[test]
  fn slash_line_height_attached() {
    let f = parse("16px/1.5 serif").unwrap();
    assert_eq!(f.size, 16.0);
    assert!(f.line_height.is_some());
  }

  #[test]
  fn slash_line_height_spaced() {
    let f = parse("16px / 1.5 serif").unwrap();
    assert_eq!(f.size, 16.0);
    assert!(f.line_height.is_some());
  }

  #[test]
  fn quoted_family() {
    let f = parse("12px \"Times New Roman\"").unwrap();
    assert_eq!(f.families, vec!["Times New Roman"]);
  }

  #[test]
  fn unquoted_multi_word_family() {
    let f = parse("12px Times New Roman").unwrap();
    assert_eq!(f.families, vec!["Times New Roman"]);
  }

  #[test]
  fn multiple_families() {
    let f = parse("12px Arial, sans-serif").unwrap();
    assert_eq!(f.families, vec!["Arial", "sans-serif"]);
  }

  #[test]
  fn style_weight_size_family() {
    let f = parse("italic bold 16px serif").unwrap();
    assert_eq!(f.style, CssFontStyle::Italic);
    assert_eq!(f.weight, 700);
    assert_eq!(f.size, 16.0);
  }

  #[test]
  fn numeric_weight_boundaries() {
    assert_eq!(parse("1 12px serif").unwrap().weight, 1);
    assert_eq!(parse("999 12px serif").unwrap().weight, 999);
    assert_eq!(parse("1000 12px serif").unwrap().weight, 1000);
    assert!(parse("0 12px serif").is_none());
    assert!(parse("1001 12px serif").is_none());
  }

  #[test]
  fn system_font_rejected() {
    assert!(parse("caption").is_none());
    assert!(parse("icon").is_none());
  }

  #[test]
  fn css_wide_keywords_rejected() {
    assert!(parse("inherit").is_none());
    assert!(parse("initial").is_none());
    assert!(parse("revert").is_none());
    assert!(parse("revert-layer").is_none());
    assert!(parse("unset").is_none());
    // Case-insensitive rejection.
    assert!(parse("Inherit").is_none());
  }

  #[test]
  fn missing_family_rejected() {
    assert!(parse("16px").is_none());
  }

  #[test]
  fn missing_size_rejected() {
    assert!(parse("serif").is_none());
  }

  #[test]
  fn small_caps_parse_and_serialize() {
    let f =
      parse("small-caps italic 400 12px/2 Unknown Font, sans-serif").unwrap();
    assert_eq!(f.font_variant_caps, FontVariantCaps::SmallCaps);
    assert_eq!(
      f.to_css_string(),
      "italic small-caps 12px Unknown Font, sans-serif"
    );
  }

  #[test]
  fn family_quoting_in_serialization() {
    let f = parse("12px \"Unknown Font #2\", sans-serif").unwrap();
    assert_eq!(f.to_css_string(), "12px \"Unknown Font #2\", sans-serif");
    let f = parse("12px \"QuotedFont\\\\\\\",\"").unwrap();
    assert_eq!(f.to_css_string(), "12px \"QuotedFont\\\\\\\",\"");
  }

  #[test]
  fn generic_family_lowercased() {
    let f = parse("20PX SERIF").unwrap();
    assert_eq!(f.size, 20.0);
    assert_eq!(f.to_css_string(), "20px serif");
  }

  #[test]
  fn relative_size_resolves_against_default_10px() {
    let f = parse("1em sans-serif").unwrap();
    assert_eq!(f.size, 10.0);
    assert_eq!(f.to_css_string(), "10px sans-serif");
  }

  #[test]
  fn reserved_family_idents_rejected() {
    assert!(parse("10px inherit").is_none());
    assert!(parse("10px initial").is_none());
    assert!(parse("10px revert").is_none());
    assert!(parse("10px default").is_none());
  }

  #[test]
  fn garbage_rejected() {
    assert!(parse("").is_none());
    assert!(parse("bogus").is_none());
    assert!(parse("10px {bogus}").is_none());
    assert!(parse("var(--x)").is_none());
    assert!(parse("var(--x, 10px serif)").is_none());
    assert!(parse("1em serif; background: green; margin: 10px").is_none());
  }

  #[test]
  fn spacing_parse_and_serialize() {
    let s = parse_css_spacing("3px").unwrap();
    assert_eq!(s.to_css_string(), "3px");
    assert_eq!(s.resolve_to_pixels(10.0), 3.0);

    let s = parse_css_spacing("1EX").unwrap();
    assert_eq!(s.to_css_string(), "1ex");
    assert_eq!(s.resolve_to_pixels(20.0), 10.0);

    let s = parse_css_spacing("1em").unwrap();
    assert_eq!(s.to_css_string(), "1em");
    assert_eq!(s.resolve_to_pixels(20.0), 20.0);

    // `rem` resolves against the fixed 16px root font size, not `font_size`.
    let s = parse_css_spacing("1rem").unwrap();
    assert_eq!(s.to_css_string(), "1rem");
    assert_eq!(s.resolve_to_pixels(20.0), 16.0);

    let s = parse_css_spacing("-0.1cm").unwrap();
    assert_eq!(s.to_css_string(), "-0.1cm");

    let s = parse_css_spacing("0").unwrap();
    assert_eq!(s.to_css_string(), "0px");

    // CSS math functions are supported; font-relative units inside them are
    // folded against the default em base (10px) at parse time.
    let s = parse_css_spacing("calc(1em + 2px)").unwrap();
    assert_eq!(s.resolve_to_pixels(20.0), 12.0);

    assert!(parse_css_spacing("5").is_none());
    assert!(parse_css_spacing("0s").is_none());
    assert!(parse_css_spacing("1min").is_none());
    assert!(parse_css_spacing("1deg").is_none());
    assert!(parse_css_spacing("1pp").is_none());
    assert!(parse_css_spacing("normal").is_none());
    assert!(parse_css_spacing("none").is_none());
    assert!(parse_css_spacing("NaN").is_none());
    assert!(parse_css_spacing("Infinity").is_none());
  }
}
