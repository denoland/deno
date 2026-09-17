// Copyright 2018-2026 the Deno authors. MIT license.

use cssparser::Token;
use cssparser::match_ignore_ascii_case;
use deno_core::WebIDL;

use super::error::CSSCustomError;
use super::error::CSSParseError;
use super::value::FontMetrics;
use super::value::LengthResolution;
use super::value::NumericValue;
use super::value::ParseOptions;
use super::value::Parser;
use super::value::ParserInput;
use super::value::SpecifiedNumericValue;

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

/// Discrete font-width keywords (canvas `fontStretch`, font-stretch-css3).
/// https://drafts.csswg.org/css-fonts-4/#font-width-prop
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum CssFontWidth {
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

impl CssFontWidth {
  pub fn to_parley(self) -> parley::FontWidth {
    match self {
      CssFontWidth::UltraCondensed => parley::FontWidth::ULTRA_CONDENSED,
      CssFontWidth::ExtraCondensed => parley::FontWidth::EXTRA_CONDENSED,
      CssFontWidth::Condensed => parley::FontWidth::CONDENSED,
      CssFontWidth::SemiCondensed => parley::FontWidth::SEMI_CONDENSED,
      CssFontWidth::Normal => parley::FontWidth::NORMAL,
      CssFontWidth::SemiExpanded => parley::FontWidth::SEMI_EXPANDED,
      CssFontWidth::Expanded => parley::FontWidth::EXPANDED,
      CssFontWidth::ExtraExpanded => parley::FontWidth::EXTRA_EXPANDED,
      CssFontWidth::UltraExpanded => parley::FontWidth::ULTRA_EXPANDED,
    }
  }
}

/// Parses `letterSpacing` / `wordSpacing`. Font-relative units resolve at
/// layout time, not set time.
/// https://html.spec.whatwg.org/multipage/canvas.html#dom-context-2d-letterspacing
pub fn parse_css_spacing(
  s: &str,
  resolution: &LengthResolution,
) -> Option<SpecifiedNumericValue> {
  let s = s.trim();
  let mut input = ParserInput::new(s);
  let mut parser = Parser::new(&mut input);
  let length = SpecifiedNumericValue::parse(
    &mut parser,
    ParseOptions {
      length_resolution: Some(*resolution),
      ..Default::default()
    },
  )
  .ok()?;
  if !length.is_length() || !parser.is_exhausted() {
    return None;
  }
  Some(length)
}

#[derive(Clone, Debug)]
pub struct FontState {
  pub style: CssFontStyle,
  pub weight: u16,
  pub width: CssFontWidth,
  pub size: f32,
  pub line_height: Option<f32>,
  pub families: Vec<String>,
  pub direction: TextDirection,
  pub font_kerning: FontKerning,
  pub font_variant_caps: FontVariantCaps,
  /// CSS letter-spacing value (default `0px`).
  pub letter_spacing: SpecifiedNumericValue,
  /// CSS word-spacing value (default `0px`).
  pub word_spacing: SpecifiedNumericValue,
  pub text_rendering: TextRendering,
}

impl Default for FontState {
  fn default() -> Self {
    Self {
      style: CssFontStyle::Normal,
      weight: 400,
      width: CssFontWidth::Normal,
      size: 10.0,
      line_height: None,
      families: vec!["sans-serif".to_string()],
      direction: TextDirection::default(),
      font_kerning: FontKerning::default(),
      font_variant_caps: FontVariantCaps::default(),
      letter_spacing: SpecifiedNumericValue::zero(),
      word_spacing: SpecifiedNumericValue::zero(),
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
    let width = match self.width {
      CssFontWidth::Normal => String::new(),
      CssFontWidth::UltraCondensed => "ultra-condensed ".to_string(),
      CssFontWidth::ExtraCondensed => "extra-condensed ".to_string(),
      CssFontWidth::Condensed => "condensed ".to_string(),
      CssFontWidth::SemiCondensed => "semi-condensed ".to_string(),
      CssFontWidth::SemiExpanded => "semi-expanded ".to_string(),
      CssFontWidth::Expanded => "expanded ".to_string(),
      CssFontWidth::ExtraExpanded => "extra-expanded ".to_string(),
      CssFontWidth::UltraExpanded => "ultra-expanded ".to_string(),
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
    format!("{style}{variant}{weight}{width}{size} {families}")
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
    quote_font_family(family)
  }
}

/// Quotes a font family name as a CSS `<string>`.
fn quote_font_family(family: &str) -> String {
  format!("\"{}\"", family.replace('\\', "\\\\").replace('"', "\\\""))
}

/// Normalize a FontFace family: quote invalid/generic names.
/// https://github.com/w3c/csswg-drafts/issues/6236
pub fn normalize_font_face_family(s: &str) -> String {
  match parse_font_face_family(s) {
    // Quoted: serialize content so `'"Arial"'` becomes `Arial`.
    Some(ParsedFontFaceFamily::Quoted(family)) => {
      serialize_font_family(&family)
    }
    // Unquoted: keep only if serialization round-trips (preserve spaces).
    Some(ParsedFontFaceFamily::Unquoted(family))
      if serialize_font_family(&family) == s =>
    {
      family
    }
    _ => quote_font_family(s),
  }
}

enum ParsedFontFaceFamily {
  Quoted(String),
  Unquoted(String),
}

/// Parse a single non-generic family-name, fully consuming `s`.
fn parse_font_face_family(s: &str) -> Option<ParsedFontFaceFamily> {
  let mut input = ParserInput::new(s);
  let mut parser = Parser::new(&mut input);
  // Quoted names are always custom (non-generic).
  if let Ok(quoted) =
    parser.try_parse(|p| -> Result<String, CSSParseError<'_>> {
      let value = p.expect_string()?.as_ref().to_string();
      p.expect_exhausted()?;
      Ok(value)
    })
  {
    return Some(ParsedFontFaceFamily::Quoted(quoted));
  }
  let family = parse_one_font_family(&mut parser)?;
  if !parser.is_exhausted() {
    return None;
  }
  // Generics are not valid FontFace family names.
  if is_generic_family(&family) {
    return None;
  }
  Some(ParsedFontFaceFamily::Unquoted(family))
}

/// Returns true if `family` is a CSS generic font family keyword.
pub fn is_generic_family(family: &str) -> bool {
  GENERIC_FAMILIES
    .iter()
    .any(|g| family.eq_ignore_ascii_case(g))
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

/// Keyword or `<percentage [0,∞]>` (`font-stretch` is a legacy alias).
/// https://drafts.csswg.org/css-fonts-4/#font-width-prop
pub fn parse_css_width(s: &str) -> Option<parley::FontWidth> {
  let width = parley::FontWidth::parse_css(s.trim())?;
  let percentage = width.percentage();
  (percentage.is_finite() && percentage >= 0.0).then_some(width)
}

/// Keyword only (font-stretch-css3 / CanvasFontStretch).
/// https://drafts.csswg.org/css-fonts-4/#font-width-prop
pub fn parse_css_width_keyword(s: &str) -> Option<CssFontWidth> {
  match s.trim() {
    "ultra-condensed" => Some(CssFontWidth::UltraCondensed),
    "extra-condensed" => Some(CssFontWidth::ExtraCondensed),
    "condensed" => Some(CssFontWidth::Condensed),
    "semi-condensed" => Some(CssFontWidth::SemiCondensed),
    "normal" => Some(CssFontWidth::Normal),
    "semi-expanded" => Some(CssFontWidth::SemiExpanded),
    "expanded" => Some(CssFontWidth::Expanded),
    "extra-expanded" => Some(CssFontWidth::ExtraExpanded),
    "ultra-expanded" => Some(CssFontWidth::UltraExpanded),
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

pub fn width_to_css_str(width: CssFontWidth) -> &'static str {
  match width {
    CssFontWidth::UltraCondensed => "ultra-condensed",
    CssFontWidth::ExtraCondensed => "extra-condensed",
    CssFontWidth::Condensed => "condensed",
    CssFontWidth::SemiCondensed => "semi-condensed",
    CssFontWidth::Normal => "normal",
    CssFontWidth::SemiExpanded => "semi-expanded",
    CssFontWidth::Expanded => "expanded",
    CssFontWidth::ExtraExpanded => "extra-expanded",
    CssFontWidth::UltraExpanded => "ultra-expanded",
  }
}

/// One entry of a CSS `src` descriptor (`@font-face` / FontFace).
/// https://drafts.csswg.org/css-fonts-4/#src-desc
#[derive(Clone, Debug, PartialEq)]
pub enum FontSrc {
  /// `url(<url>) [format(...)]? [tech(...)]?`
  Url {
    url: String,
    format: Option<String>,
    tech: Vec<String>,
  },
  /// `local(<family-name>)`
  Local(String),
}

/// Supported `<font-format>` values (others, e.g. woff/woff2, are skipped).
/// https://drafts.csswg.org/css-fonts-4/#font-format-values
const SUPPORTED_FONT_FORMATS: &[&str] = &["collection", "opentype", "truetype"];

/// Supported `<font-tech>` values (unsupported hints skip the entry).
/// https://drafts.csswg.org/css-fonts-4/#font-tech-values
const SUPPORTED_FONT_TECHS: &[&str] = &[
  "features-opentype",
  "variations",
  "palettes",
  "color-colrv0",
  "color-colrv1",
  "color-sbix",
  "color-cbdt",
];

impl FontSrc {
  /// False when `format()` / `tech()` promise something we cannot use.
  pub fn is_supported(&self) -> bool {
    match self {
      FontSrc::Local(_) => true,
      FontSrc::Url { format, tech, .. } => {
        format.as_ref().is_none_or(|format| {
          SUPPORTED_FONT_FORMATS.contains(&format.as_str())
        }) && tech
          .iter()
          .all(|tech| SUPPORTED_FONT_TECHS.contains(&tech.as_str()))
      }
    }
  }
}

/// Parse a CSS `src` descriptor. `None` => SyntaxError in the constructor.
/// https://drafts.csswg.org/css-font-loading-3/#dom-fontface-fontface
pub fn parse_css_font_src(s: &str) -> Option<Vec<FontSrc>> {
  let mut input = ParserInput::new(s.trim());
  let mut parser = Parser::new(&mut input);
  let mut srcs = Vec::new();
  loop {
    srcs.push(parse_one_font_src(&mut parser)?);
    if parser.try_parse(|p| p.expect_comma()).is_err() {
      break;
    }
  }
  if !parser.is_exhausted() {
    return None;
  }
  Some(srcs)
}

fn parse_one_font_src<'i, 't>(input: &mut Parser<'i, 't>) -> Option<FontSrc> {
  let tok = input.next().ok()?.clone();
  let url = match &tok {
    Token::UnquotedUrl(url) => url.as_ref().to_string(),
    Token::Function(name) if name.eq_ignore_ascii_case("url") => {
      input.parse_nested_block(parse_url_string).ok()?
    }
    Token::Function(name) if name.eq_ignore_ascii_case("local") => {
      return Some(FontSrc::Local(
        input.parse_nested_block(parse_local_family).ok()?,
      ));
    }
    _ => return None,
  };

  let format = input.try_parse(parse_format_hint).ok();
  let tech = input.try_parse(parse_tech_hint).unwrap_or_default();
  Some(FontSrc::Url { url, format, tech })
}

fn parse_url_string<'i, 't>(
  input: &mut Parser<'i, 't>,
) -> Result<String, CSSParseError<'i>> {
  Ok(input.expect_string()?.as_ref().to_string())
}

fn parse_local_family<'i, 't>(
  input: &mut Parser<'i, 't>,
) -> Result<String, CSSParseError<'i>> {
  parse_one_font_family(input).ok_or_else(|| {
    input.new_custom_error(CSSCustomError::InvalidFunction("local".to_string()))
  })
}

/// Parses `format(<font-format>)`, which also accepts a legacy string.
fn parse_format_hint<'i, 't>(
  input: &mut Parser<'i, 't>,
) -> Result<String, CSSParseError<'i>> {
  let tok = input.next()?.clone();
  match &tok {
    Token::Function(name) if name.eq_ignore_ascii_case("format") => input
      .parse_nested_block(|p| {
        let tok = p.next()?.clone();
        match &tok {
          Token::QuotedString(s) | Token::Ident(s) => {
            Ok(s.as_ref().to_ascii_lowercase())
          }
          _ => Err(p.new_custom_error(CSSCustomError::InvalidFunction(
            "format".to_string(),
          ))),
        }
      }),
    _ => Err(
      input
        .new_custom_error(CSSCustomError::InvalidFunction("src".to_string())),
    ),
  }
}

/// Parses `tech(<font-tech>#)`.
fn parse_tech_hint<'i, 't>(
  input: &mut Parser<'i, 't>,
) -> Result<Vec<String>, CSSParseError<'i>> {
  let tok = input.next()?.clone();
  match &tok {
    Token::Function(name) if name.eq_ignore_ascii_case("tech") => input
      .parse_nested_block(|p| {
        let mut techs = Vec::new();
        loop {
          techs.push(p.expect_ident()?.as_ref().to_ascii_lowercase());
          if p.try_parse(|p| p.expect_comma()).is_err() {
            break;
          }
        }
        Ok(techs)
      }),
    _ => Err(
      input
        .new_custom_error(CSSCustomError::InvalidFunction("src".to_string())),
    ),
  }
}

/// Parses a CSS font shorthand into a [`FontState`].
/// https://html.spec.whatwg.org/multipage/canvas.html#dom-context-2d-font
/// https://drafts.csswg.org/css-fonts-4/#font-prop
pub fn parse_css_font(
  s: &str,
  resolution: &LengthResolution,
) -> Option<FontState> {
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
  parse_css_font_inner(&mut parser, resolution)
}

/// Canvas default font-size base.
/// https://html.spec.whatwg.org/multipage/canvas.html#dom-context-2d-font
pub const EM_BASE_PX: f64 = 10.0;

/// Default canvas font (`10px sans-serif`) with CSS fallback metrics.
pub fn default_font_resolution() -> LengthResolution {
  LengthResolution::new(FontMetrics::fallback(EM_BASE_PX))
}

/// Result of attempting to parse one optional prefix keyword in the font shorthand.
enum PrefixValue {
  Style(CssFontStyle),
  Weight(u16),
  Width(CssFontWidth),
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
        _ => parse_css_width_keyword(ident)
          .map(PrefixValue::Width)
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
  resolution: &LengthResolution,
) -> Option<FontState> {
  let mut style = CssFontStyle::Normal;
  let mut weight: u16 = 400;
  let mut width = CssFontWidth::Normal;
  let mut variant_caps = FontVariantCaps::Normal;

  for _ in 0..4 {
    match input.try_parse(parse_prefix) {
      Ok(PrefixValue::Style(s)) => style = s,
      Ok(PrefixValue::Weight(w)) => weight = w,
      Ok(PrefixValue::Width(w)) => width = w,
      Ok(PrefixValue::SmallCaps) => variant_caps = FontVariantCaps::SmallCaps,
      Ok(PrefixValue::Neutral) => {}
      Err(_) => break,
    }
  }

  // Parse font-size (<length-percentage>), a percentage of the font size in
  // effect on the canvas.
  let size_resolution = LengthResolution {
    percentage_basis: Some(resolution.font.em),
    ..*resolution
  };
  let size_value = input
    .try_parse(|p| {
      NumericValue::parse(
        p,
        ParseOptions {
          length_resolution: Some(size_resolution),
          ..Default::default()
        },
      )
    })
    .ok()?;
  let size = match size_value {
    NumericValue::Length(l) => l.resolve_to_pixels(&size_resolution) as f32,
    NumericValue::Zero => 0.0f32,
    _ => return None,
  };

  // /line-height is relative to the font-size just parsed.
  // https://drafts.csswg.org/css2/#propdef-line-height
  let size_px = size as f64;
  let line_height_resolution = LengthResolution {
    font: FontMetrics::fallback(size_px),
    percentage_basis: Some(size_px),
    ..*resolution
  };
  let line_height: Option<f32> = input
    .try_parse(|p| {
      let tok = p.next()?.clone();
      if !matches!(tok, Token::Delim('/')) {
        return Err(p.new_custom_error(CSSCustomError::InvalidDimension));
      }
      let lh_value = NumericValue::parse(
        p,
        ParseOptions {
          length_resolution: Some(line_height_resolution),
          ..Default::default()
        },
      )?;
      match lh_value {
        NumericValue::Number(n) => Ok((n * size_px) as f32),
        NumericValue::Length(l) => {
          Ok(l.resolve_to_pixels(&line_height_resolution) as f32)
        }
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
    width,
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
/// https://drafts.csswg.org/css-fonts-4/#generic-font-families
const GENERIC_FAMILIES: &[&str] = &[
  "serif",
  "sans-serif",
  "cursive",
  "fantasy",
  "monospace",
  "system-ui",
  "math",
  "emoji",
  "fangsong",
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
    parse_css_font(s, &default_font_resolution())
  }

  /// A synthetic face whose metrics are all distinct multiples of the font
  /// size, so a resolved value identifies the unit it came from.
  fn metrics(em: f64) -> FontMetrics {
    FontMetrics {
      em,
      cap: em * 0.7,
      ch: em * 0.6,
      ex: em * 0.5,
      ic: em * 1.1,
      lh: em * 1.4,
    }
  }

  fn spacing(s: &str, em: f64) -> Option<SpecifiedNumericValue> {
    parse_css_spacing(s, &LengthResolution::new(metrics(em)))
  }

  fn resolve_to_pixels(s: &str, em: f64) -> f64 {
    spacing(s, em)
      .unwrap()
      .resolve(&LengthResolution::new(metrics(em)))
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
  fn parse_css_width_keywords_and_percentages() {
    assert_eq!(
      parse_css_width("semi-condensed"),
      Some(parley::FontWidth::SEMI_CONDENSED)
    );
    assert_eq!(
      parse_css_width("  expanded "),
      Some(parley::FontWidth::EXPANDED)
    );
    assert_eq!(
      parse_css_width("87.5%"),
      Some(parley::FontWidth::from_percentage(87.5))
    );
    assert_eq!(
      parse_css_width("200%"),
      Some(parley::FontWidth::from_percentage(200.0))
    );
    assert_eq!(
      parse_css_width("0%"),
      Some(parley::FontWidth::from_percentage(0.0))
    );
    assert_eq!(parse_css_width("-1%"), None);
    assert_eq!(parse_css_width("invalid"), None);
    assert_eq!(parse_css_width("80"), None);
  }

  #[test]
  fn parse_css_width_keyword_equals_percentage() {
    assert_eq!(
      parse_css_width("semi-condensed").unwrap().percentage(),
      parse_css_width("87.5%").unwrap().percentage()
    );
    assert_eq!(
      parse_css_width("ultra-expanded").unwrap().percentage(),
      parse_css_width("200%").unwrap().percentage()
    );
    assert_eq!(
      parse_css_width("normal").unwrap().percentage(),
      parse_css_width("100%").unwrap().percentage()
    );
  }

  #[test]
  fn parse_css_weight_keywords_and_numbers() {
    assert_eq!(parse_css_weight("normal"), Some(400));
    assert_eq!(parse_css_weight("bold"), Some(700));
    assert_eq!(parse_css_weight("350"), Some(350));
    assert_eq!(parse_css_weight("0"), None);
    assert_eq!(parse_css_weight("1001"), None);
    assert_eq!(parse_css_weight("invalid"), None);
  }

  #[test]
  fn parse_css_width_keyword_rejects_percentages() {
    assert_eq!(
      parse_css_width_keyword("condensed"),
      Some(CssFontWidth::Condensed)
    );
    assert_eq!(parse_css_width_keyword("50%"), None);
    assert_eq!(parse_css_width_keyword("100%"), None);
  }

  #[test]
  fn font_shorthand_width_keyword() {
    let f = parse("condensed 12px serif").unwrap();
    assert_eq!(f.width, CssFontWidth::Condensed);
    let f = parse("ultra-expanded bold 14px Arial").unwrap();
    assert_eq!(f.width, CssFontWidth::UltraExpanded);
    assert_eq!(f.weight, 700);
    assert_eq!(parse_css_width_keyword("75%"), None);
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

    // No root element, so `rem` resolves against the same default font.
    let f = parse("1rem sans-serif").unwrap();
    assert_eq!(f.size, 10.0);

    // Fallback metrics: ex/ch are 0.5em, ic is 1em, cap is the assumed ascent.
    assert_eq!(parse("1ex sans-serif").unwrap().size, 5.0);
    assert_eq!(parse("1rch sans-serif").unwrap().size, 5.0);
    assert_eq!(parse("1ic sans-serif").unwrap().size, 10.0);
    assert_eq!(parse("1cap sans-serif").unwrap().size, 8.0);
    assert_eq!(parse("1lh sans-serif").unwrap().size, 12.0);
  }

  #[test]
  fn viewport_size_resolves_to_zero() {
    // Canvas has no viewport, so the shorthand parses but yields a 0px font.
    // https://www.w3.org/TR/css-values-4/#viewport-relative-lengths
    let f = parse("10vw sans-serif").unwrap();
    assert_eq!(f.size, 0.0);
    assert_eq!(f.to_css_string(), "0px sans-serif");
    assert_eq!(parse("10cqmin sans-serif").unwrap().size, 0.0);
    assert_eq!(parse("calc(10vw + 3px) sans-serif").unwrap().size, 3.0);
  }

  #[test]
  fn size_percentage_is_of_the_canvas_font() {
    // font-size is `<length-percentage>`, so `%` mixes with lengths.
    // https://drafts.csswg.org/css-fonts-4/#font-size-prop
    assert_eq!(parse("50% sans-serif").unwrap().size, 5.0);
    assert_eq!(parse("calc(0.5em + 50%) sans-serif").unwrap().size, 10.0);
    assert_eq!(parse("calc(50% - 0.2em) sans-serif").unwrap().size, 3.0);
    assert_eq!(parse("min(0.5em, 50%) sans-serif").unwrap().size, 5.0);
    assert_eq!(
      parse("clamp(50%, 1em, 200%) sans-serif").unwrap().size,
      10.0
    );

    // A percentage counts as a length, so these are type errors.
    assert!(parse("calc(1px * 50%) sans-serif").is_none());
    assert!(parse("calc(50% + 1) sans-serif").is_none());
  }

  #[test]
  fn line_height_is_relative_to_the_size_just_parsed() {
    // Not to the font the size was itself relative to.
    // https://drafts.csswg.org/css2/#propdef-line-height
    assert_eq!(parse("20px/1.5 serif").unwrap().line_height, Some(30.0));
    assert_eq!(parse("20px/150% serif").unwrap().line_height, Some(30.0));
    assert_eq!(parse("20px/2em serif").unwrap().line_height, Some(40.0));
    assert_eq!(parse("20px/30px serif").unwrap().line_height, Some(30.0));
    assert_eq!(
      parse("20px/calc(50% + 5px) serif").unwrap().line_height,
      Some(15.0)
    );
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
    let s = spacing("3px", 10.0).unwrap();
    assert_eq!(s.to_css_string(), "3px");
    assert!(!s.is_relative_length());
    assert_eq!(resolve_to_pixels("3px", 10.0), 3.0);

    let s = spacing("1EX", 20.0).unwrap();
    assert_eq!(s.to_css_string(), "1ex");
    assert!(s.is_relative_length());

    let s = spacing("-0.1cm", 10.0).unwrap();
    assert_eq!(s.to_css_string(), "-0.1cm");

    let s = spacing("0", 10.0).unwrap();
    assert_eq!(s.to_css_string(), "0px");

    assert!(spacing("5", 10.0).is_none());
    assert!(spacing("0s", 10.0).is_none());
    assert!(spacing("1min", 10.0).is_none());
    assert!(spacing("1deg", 10.0).is_none());
    assert!(spacing("1pp", 10.0).is_none());
    assert!(spacing("normal", 10.0).is_none());
    assert!(spacing("none", 10.0).is_none());
    assert!(spacing("NaN", 10.0).is_none());
    assert!(spacing("Infinity", 10.0).is_none());
  }

  #[test]
  fn spacing_font_relative_units() {
    // Every font-relative unit round-trips and reads its own metric.
    // https://www.w3.org/TR/css-values-4/#font-relative-lengths
    for (css, expected) in [
      ("1em", 20.0),
      ("1cap", 14.0),
      ("1ch", 12.0),
      ("1ex", 10.0),
      ("1ic", 22.0),
      ("1lh", 28.0),
      // No root element, so the root metrics are the same font.
      ("1rem", 20.0),
      ("1rcap", 14.0),
      ("1rch", 12.0),
      ("1rex", 10.0),
      ("1ric", 22.0),
      ("1rlh", 28.0),
    ] {
      let s = spacing(css, 20.0).unwrap();
      assert_eq!(s.to_css_string(), css, "serializing {css}");
      assert!(s.is_relative_length(), "{css} should need metrics");
      assert_eq!(resolve_to_pixels(css, 20.0), expected, "resolving {css}");
    }
  }

  #[test]
  fn spacing_root_units_read_the_root_metrics() {
    let resolution = LengthResolution {
      root: metrics(100.0),
      ..LengthResolution::new(metrics(20.0))
    };
    let s = parse_css_spacing("1rex", &resolution).unwrap();
    assert_eq!(s.resolve(&resolution), 50.0);
    let s = parse_css_spacing("1ex", &resolution).unwrap();
    assert_eq!(s.resolve(&resolution), 10.0);
  }

  #[test]
  fn spacing_viewport_units_resolve_to_zero() {
    // No viewport/container: these parse but resolve to zero.
    // https://drafts.csswg.org/css-conditional-5/#container-lengths
    for css in [
      "1vw", "1svh", "1lvi", "1dvb", "1vmin", "1dvmax", "1cqw", "1cqmin",
    ] {
      let s = spacing(css, 20.0).unwrap();
      assert_eq!(s.to_css_string(), css, "serializing {css}");
      assert_eq!(resolve_to_pixels(css, 20.0), 0.0, "resolving {css}");
    }
  }

  #[test]
  fn spacing_rejects_percentages() {
    // Spacing takes a plain `<length>`, so `%` has no basis.
    // https://html.spec.whatwg.org/multipage/canvas.html#dom-context-2d-letterspacing
    for css in [
      "50%",
      "calc(50%)",
      "calc(1em + 50%)",
      "calc(50% - 2px)",
      "min(1em, 50%)",
    ] {
      assert!(spacing(css, 20.0).is_none(), "{css}");
    }
  }

  #[test]
  fn spacing_math_functions_resolve_lazily() {
    // A math function over font-relative units is retained as a tree, so it
    // re-resolves against whichever font is in effect.
    assert_eq!(
      spacing("calc(1em + 2px)", 10.0).unwrap().to_css_string(),
      "calc(1em + 2px)"
    );
    assert_eq!(resolve_to_pixels("calc(1em + 2px)", 10.0), 12.0);
    assert_eq!(resolve_to_pixels("calc(1em + 2px)", 20.0), 22.0);
    assert_eq!(resolve_to_pixels("calc(1em - 2px)", 20.0), 18.0);
    assert_eq!(resolve_to_pixels("calc(2 * 1em)", 20.0), 40.0);
    assert_eq!(resolve_to_pixels("calc(1em / 2)", 20.0), 10.0);
    // ex is 0.5em and cap is 0.7em in the synthetic metrics.
    assert_eq!(resolve_to_pixels("calc(1ex + 1cap)", 20.0), 24.0);

    // Comparison and stepped functions keep their own nodes.
    assert_eq!(resolve_to_pixels("min(1em, 15px)", 10.0), 10.0);
    assert_eq!(resolve_to_pixels("min(1em, 15px)", 20.0), 15.0);
    assert_eq!(resolve_to_pixels("max(1em, 15px)", 10.0), 15.0);
    assert_eq!(resolve_to_pixels("clamp(5px, 1em, 15px)", 2.0), 5.0);
    assert_eq!(resolve_to_pixels("clamp(5px, 1em, 15px)", 10.0), 10.0);
    assert_eq!(resolve_to_pixels("clamp(5px, 1em, 15px)", 100.0), 15.0);
    assert_eq!(resolve_to_pixels("clamp(none, 1em, 15px)", 100.0), 15.0);
    assert_eq!(resolve_to_pixels("round(1em, 3px)", 10.0), 9.0);
    assert_eq!(resolve_to_pixels("round(up, 1em, 3px)", 10.0), 12.0);
    assert_eq!(resolve_to_pixels("mod(1em, 3px)", 10.0), 1.0);
    assert_eq!(resolve_to_pixels("rem(1em, 3px)", 10.0), 1.0);
    assert_eq!(resolve_to_pixels("abs(-1em)", 10.0), 10.0);
    assert_eq!(resolve_to_pixels("hypot(3em, 4em)", 10.0), 50.0);
    // Nesting composes.
    assert_eq!(resolve_to_pixels("calc(min(1em, 15px) + 1px)", 10.0), 11.0);
    assert_eq!(resolve_to_pixels("calc(min(1em, 15px) + 1px)", 20.0), 16.0);

    // An expression over absolute units is already exact, so it collapses.
    let s = spacing("calc(1px + 2px)", 10.0).unwrap();
    assert_eq!(s.to_css_string(), "3px");

    // A viewport unit resolves to zero here, but the tree is still retained so
    // the value serializes as written.
    let s = spacing("calc(1vw + 2px)", 10.0).unwrap();
    assert_eq!(s.to_css_string(), "calc(2px + 1vw)");
    assert_eq!(resolve_to_pixels("calc(1vw + 2px)", 10.0), 2.0);
  }

  #[test]
  fn spacing_math_functions_serialize() {
    // https://www.w3.org/TR/css-values-4/#calc-serialize
    for (css, expected) in [
      // Sum terms are sorted by unit, ASCII case-insensitively.
      ("calc(2px + 1em)", "calc(1em + 2px)"),
      ("calc(1em + 2px)", "calc(1em + 2px)"),
      ("calc(1em - 2px)", "calc(1em - 2px)"),
      // A tree that simplified to a single dimension drops the wrapper.
      ("calc(1em * 2)", "2em"),
      ("calc(1em / 2)", "0.5em"),
      ("min(1em, 15px)", "min(1em, 15px)"),
      ("max(1em, 15px)", "max(1em, 15px)"),
      ("clamp(5px, 1em, 15px)", "clamp(5px, 1em, 15px)"),
      ("clamp(none, 1em, 15px)", "clamp(none, 1em, 15px)"),
      ("round(1em, 3px)", "round(1em, 3px)"),
      ("round(to-zero, 1em, 3px)", "round(to-zero, 1em, 3px)"),
      ("mod(1em, 3px)", "mod(1em, 3px)"),
      ("rem(1em, 3px)", "rem(1em, 3px)"),
      ("abs(-1em)", "1em"),
      ("hypot(3em, 4em)", "hypot(3em, 4em)"),
      ("calc(min(1em, 15px) + 1px)", "calc(1px + min(1em, 15px))"),
    ] {
      let s = spacing(css, 10.0).unwrap();
      assert_eq!(s.to_css_string(), expected, "serializing {css}");
    }
  }

  #[test]
  fn spacing_relative_number_keeps_its_tree() {
    // A `<number>` subexpression still keeps the font dependency in the tree.
    let s = spacing("calc(sqrt(1em / 1px) * 1px)", 16.0).unwrap();
    assert_eq!(s.to_css_string(), "calc(sqrt(1em / 1px) * 1px)");
    assert_eq!(resolve_to_pixels("calc(sqrt(1em / 1px) * 1px)", 16.0), 4.0);
    assert_eq!(
      resolve_to_pixels("calc(sqrt(1em / 1px) * 1px)", 100.0),
      10.0
    );

    // A product has no unit of its own, so it sorts after the dimensions.
    let s = spacing("calc(2px + sqrt(1em / 1px) * 1px)", 16.0).unwrap();
    assert_eq!(s.to_css_string(), "calc(2px + sqrt(1em / 1px) * 1px)");
    assert_eq!(
      resolve_to_pixels("calc(2px + sqrt(1em / 1px) * 1px)", 16.0),
      6.0
    );
    assert_eq!(
      resolve_to_pixels("calc(sqrt(1em / 1px) * 1px + 2px)", 16.0),
      6.0
    );
    assert_eq!(
      spacing("calc(sqrt(1em / 1px) * 1px + 2px)", 16.0)
        .unwrap()
        .to_css_string(),
      "calc(2px + sqrt(1em / 1px) * 1px)"
    );

    // Dividing by a length, with no function in between.
    assert_eq!(resolve_to_pixels("calc(1em / 1px * 1px)", 16.0), 16.0);
    assert_eq!(resolve_to_pixels("calc(1em / 1px * 1px)", 25.0), 25.0);
    assert_eq!(
      spacing("calc(1em / 1px * 1px)", 16.0)
        .unwrap()
        .to_css_string(),
      "calc(1em / 1px * 1px)"
    );

    // A viewport unit takes the same path, even though it resolves to zero.
    let s = spacing("calc(sqrt(1vw / 1px) * 1px)", 16.0).unwrap();
    assert_eq!(s.to_css_string(), "calc(sqrt(1vw / 1px) * 1px)");
    assert_eq!(resolve_to_pixels("calc(sqrt(1vw / 1px) * 1px)", 16.0), 0.0);
  }

  #[test]
  fn spacing_relative_dependency_through_an_angle() {
    // `atan2()` can take a font dependency out of the length dimension.
    let s = spacing("calc(atan2(1em, 1px) / 1deg * 1px)", 16.0).unwrap();
    assert_eq!(s.to_css_string(), "calc(atan2(1em, 1px) / 1deg * 1px)");
    assert_eq!(
      resolve_to_pixels("calc(atan2(1em, 1px) / 1deg * 1px)", 16.0),
      16.0_f64.atan2(1.0).to_degrees()
    );
    assert_eq!(
      resolve_to_pixels("calc(atan2(1em, 1px) / 1deg * 1px)", 100.0),
      100.0_f64.atan2(1.0).to_degrees()
    );
  }

  #[test]
  fn spacing_relative_dependency_through_other_functions() {
    // Every `<number>`-valued function keeps its node, so each of these
    // re-resolves rather than freezing the parse-time metrics.
    for (css, em, expected) in [
      ("calc(pow(1em / 1px, 2) * 1px)", 4.0, 16.0),
      ("calc(pow(1em / 1px, 2) * 1px)", 5.0, 25.0),
      ("calc(sign(1em) * 2px)", 10.0, 2.0),
      ("calc(log(1em / 1px, 2) * 1px)", 8.0, 3.0),
      ("calc(exp(1em / 1px) * 1px)", 0.0, 1.0),
      ("calc(abs(1em / 1px) * 1px)", 7.0, 7.0),
      ("calc(sin(90deg) * 1em)", 12.0, 12.0),
    ] {
      assert_eq!(resolve_to_pixels(css, em), expected, "resolving {css}");
    }
  }

  fn url(url: &str) -> FontSrc {
    FontSrc::Url {
      url: url.to_string(),
      format: None,
      tech: vec![],
    }
  }

  #[test]
  fn font_src_urls() {
    assert_eq!(
      parse_css_font_src("url(blob:null/abc)").unwrap(),
      vec![url("blob:null/abc")]
    );
    assert_eq!(
      parse_css_font_src(" url('blob:null/abc') ").unwrap(),
      vec![url("blob:null/abc")]
    );
    assert_eq!(
      parse_css_font_src("url(\"blob:null/abc\")").unwrap(),
      vec![url("blob:null/abc")]
    );
  }

  #[test]
  fn font_src_keeps_format_and_tech_hints() {
    assert_eq!(
      parse_css_font_src("url(a.woff2) format(\"woff2\")").unwrap(),
      vec![FontSrc::Url {
        url: "a.woff2".to_string(),
        format: Some("woff2".to_string()),
        tech: vec![],
      }]
    );
    // Keyword form and mixed case both accepted.
    assert_eq!(
      parse_css_font_src(
        "url(a.ttf) format(TrueType) tech(variations, color-COLRv1)"
      )
      .unwrap(),
      vec![FontSrc::Url {
        url: "a.ttf".to_string(),
        format: Some("truetype".to_string()),
        tech: vec!["variations".to_string(), "color-colrv1".to_string()],
      }]
    );
  }

  #[test]
  fn font_src_supported_filters_out_unusable_entries() {
    let unsupported = [
      "url(a.woff2) format(\"woff2\")",
      "url(a.woff) format(\"woff\")",
      "url(a.svg) format(\"svg\")",
      "url(a.eot) format(\"embedded-opentype\")",
      "url(a.ttf) tech(features-graphite)",
      "url(a.ttf) tech(color-SVG)",
      "url(a.ttf) format(\"truetype\") tech(variations, incremental)",
    ];
    for src in unsupported {
      let parsed = parse_css_font_src(src).unwrap();
      assert!(!parsed[0].is_supported(), "{src} should be skipped");
    }

    let supported = [
      "url(a.ttf)",
      "url(a.ttf) format(\"truetype\")",
      "url(a.otf) format(\"opentype\")",
      "url(a.ttc) format(\"collection\")",
      "url(a.ttf) tech(color-COLRv0, color-sbix, palettes)",
      "local(My Font)",
    ];
    for src in supported {
      let parsed = parse_css_font_src(src).unwrap();
      assert!(parsed[0].is_supported(), "{src} should be usable");
    }
  }

  #[test]
  fn font_src_list_and_local() {
    assert_eq!(
      parse_css_font_src(
        "local(My Font), local(\"Other Font\"), url(a.ttf) format(\"truetype\")"
      )
      .unwrap(),
      vec![
        FontSrc::Local("My Font".to_string()),
        FontSrc::Local("Other Font".to_string()),
        FontSrc::Url {
          url: "a.ttf".to_string(),
          format: Some("truetype".to_string()),
          tech: vec![],
        },
      ]
    );
  }

  #[test]
  fn font_src_rejects_invalid_values() {
    assert!(parse_css_font_src("").is_none());
    assert!(parse_css_font_src("blob:null/abc").is_none());
    assert!(parse_css_font_src("url(a.ttf),").is_none());
    assert!(parse_css_font_src("url(a.ttf) garbage").is_none());
    assert!(parse_css_font_src("local()").is_none());
    assert!(parse_css_font_src("unknown(a.ttf)").is_none());
    assert!(parse_css_font_src("url(a.ttf) format()").is_none());
    assert!(parse_css_font_src("url(a.ttf) tech()").is_none());
  }

  #[test]
  fn font_face_family_keeps_valid_names() {
    assert_eq!(normalize_font_face_family("Arial"), "Arial");
    assert_eq!(
      normalize_font_face_family("Times New Roman"),
      "Times New Roman"
    );
    // Quoted name unwraps to content.
    assert_eq!(normalize_font_face_family("\"Arial\""), "Arial");
    assert_eq!(
      normalize_font_face_family("\"Times New Roman\""),
      "Times New Roman"
    );
  }

  #[test]
  fn font_face_family_quotes_invalid_or_generic_names() {
    // fontface-invalid-family.tentative.html (css-font-loading#6236).
    for raw in [
      "content:Segoe UI",
      "sans-serif",
      "A, B",
      "inherit",
      "a 1",
      "",
      "a  b",
      " a b",
      "a b ",
    ] {
      assert_eq!(
        normalize_font_face_family(raw),
        format!("\"{raw}\""),
        "expected {raw:?} to be quoted"
      );
    }
  }
}
