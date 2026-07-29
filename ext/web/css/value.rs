// Copyright 2018-2026 the Deno authors. MIT license.

use std::f64;
use std::ops;
use std::rc::Rc;

pub use cssparser::Parser;
pub use cssparser::ParserInput;
use cssparser::SourcePosition;
use cssparser::Token;
use cssparser::match_ignore_ascii_case;

use crate::css::error::CSSCustomError;
use crate::css::error::CSSParseError;
use crate::f64::maximum;
use crate::f64::minimum;

const INCH_TO_PX: f64 = 96.0;
const INCH_TO_CM: f64 = 2.54;
const TURN_TO_DEG: f64 = 360.0;
const TURN_TO_GRAD: f64 = 400.0;
const S_TO_MS: f64 = 1000.0;
const KHZ_TO_HZ: f64 = 1000.0;

/// Metrics of a single font, in pixels, used to resolve font-relative
/// `<length>` units.
/// https://www.w3.org/TR/css-values-4/#font-relative-lengths
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FontMetrics {
  /// `em`: the font size.
  pub em: f64,
  /// `cap`: the cap height.
  pub cap: f64,
  /// `ch`: the advance measure of the `0` (U+0030) glyph.
  pub ch: f64,
  /// `ex`: the x-height.
  pub ex: f64,
  /// `ic`: the advance measure of the `水` (U+6C34) glyph.
  pub ic: f64,
  /// `lh`: the used line height, which canvas always computes from `normal`.
  pub lh: f64,
}

impl FontMetrics {
  /// The ratio assumed for a `normal` line height when no font metrics are
  /// available.
  const NORMAL_LINE_HEIGHT_RATIO: f64 = 1.2;

  /// The values CSS mandates when a metric cannot be determined. `cap` has no
  /// numeric fallback in the spec (it says to use the font's ascent), so this
  /// reuses the 0.8em ascent assumed elsewhere when no face is available.
  /// https://www.w3.org/TR/css-values-4/#font-relative-lengths
  #[inline]
  pub fn fallback(em: f64) -> Self {
    Self {
      em,
      cap: em * 0.8,
      ch: em * 0.5,
      ex: em * 0.5,
      ic: em,
      lh: em * Self::NORMAL_LINE_HEIGHT_RATIO,
    }
  }
}

/// The context a font-relative `<length>` resolves against.
///
/// Canvas has no root element, so `new` points both the element and the root
/// metrics at the same font, matching Blink's canvas length resolution.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LengthResolution {
  pub font: FontMetrics,
  pub root: FontMetrics,
}

impl LengthResolution {
  #[inline]
  pub fn new(font: FontMetrics) -> Self {
    Self { font, root: font }
  }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Length {
  value: f64,
  unit: LengthUnit,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LengthUnit {
  // https://www.w3.org/TR/css-values-4/#absolute-lengths
  Cm,
  Mm,
  Q,
  In,
  Pc,
  Pt,
  Px,
  // https://www.w3.org/TR/css-values-4/#font-relative-lengths
  Em,
  Cap,
  Ch,
  Ex,
  Ic,
  Lh,
  Rem,
  Rcap,
  Rch,
  Rex,
  Ric,
  Rlh,
  /// Viewport- and container-relative units. Canvas has neither a viewport nor
  /// a query container, so the initial containing block is zero-sized and these
  /// always resolve to zero; only the unit name matters, for serialization.
  /// https://www.w3.org/TR/css-values-4/#viewport-relative-lengths
  /// https://drafts.csswg.org/css-conditional-5/#container-lengths
  Zero(&'static str),
}

impl LengthUnit {
  #[inline]
  fn parse(unit: &str) -> Option<Self> {
    Some(match_ignore_ascii_case! { unit,
      // https://www.w3.org/TR/css-values-4/#absolute-lengths
      "cm" => Self::Cm,
      "mm" => Self::Mm,
      "q" => Self::Q,
      "in" => Self::In,
      "pc" => Self::Pc,
      "pt" => Self::Pt,
      "px" => Self::Px,
      // https://www.w3.org/TR/css-values-4/#font-relative-lengths
      "em" => Self::Em,
      "cap" => Self::Cap,
      "ch" => Self::Ch,
      "ex" => Self::Ex,
      "ic" => Self::Ic,
      "lh" => Self::Lh,
      "rem" => Self::Rem,
      "rcap" => Self::Rcap,
      "rch" => Self::Rch,
      "rex" => Self::Rex,
      "ric" => Self::Ric,
      "rlh" => Self::Rlh,
      // https://www.w3.org/TR/css-values-4/#viewport-relative-lengths
      "vw" => Self::Zero("vw"),
      "svw" => Self::Zero("svw"),
      "lvw" => Self::Zero("lvw"),
      "dvw" => Self::Zero("dvw"),
      "vh" => Self::Zero("vh"),
      "svh" => Self::Zero("svh"),
      "lvh" => Self::Zero("lvh"),
      "dvh" => Self::Zero("dvh"),
      "vi" => Self::Zero("vi"),
      "svi" => Self::Zero("svi"),
      "lvi" => Self::Zero("lvi"),
      "dvi" => Self::Zero("dvi"),
      "vb" => Self::Zero("vb"),
      "svb" => Self::Zero("svb"),
      "lvb" => Self::Zero("lvb"),
      "dvb" => Self::Zero("dvb"),
      "vmin" => Self::Zero("vmin"),
      "svmin" => Self::Zero("svmin"),
      "lvmin" => Self::Zero("lvmin"),
      "dvmin" => Self::Zero("dvmin"),
      "vmax" => Self::Zero("vmax"),
      "svmax" => Self::Zero("svmax"),
      "lvmax" => Self::Zero("lvmax"),
      "dvmax" => Self::Zero("dvmax"),
      // https://drafts.csswg.org/css-conditional-5/#container-lengths
      "cqw" => Self::Zero("cqw"),
      "cqh" => Self::Zero("cqh"),
      "cqi" => Self::Zero("cqi"),
      "cqb" => Self::Zero("cqb"),
      "cqmin" => Self::Zero("cqmin"),
      "cqmax" => Self::Zero("cqmax"),
      _ => return None,
    })
  }

  #[inline]
  fn is_absolute(self) -> bool {
    matches!(
      self,
      Self::Cm | Self::Mm | Self::Q | Self::In | Self::Pc | Self::Pt | Self::Px
    )
  }

  /// Whether resolving this unit depends on the font in effect, and therefore
  /// has to be deferred until the value is used.
  #[inline]
  fn is_font_relative(self) -> bool {
    !self.is_absolute() && !matches!(self, Self::Zero(_))
  }

  /// The factor to the canonical unit, `px`. `None` for a unit that needs the
  /// font metrics or a viewport, which has no constant factor.
  /// https://www.w3.org/TR/css-values-4/#absolute-lengths
  #[inline]
  fn px_factor(self) -> Option<f64> {
    Some(match self {
      Self::Cm => INCH_TO_PX / INCH_TO_CM,
      Self::Mm => INCH_TO_PX / INCH_TO_CM / 10.0,
      Self::Q => INCH_TO_PX / INCH_TO_CM / 40.0,
      Self::In => INCH_TO_PX,
      Self::Pc => INCH_TO_PX / 6.0,
      Self::Pt => INCH_TO_PX / 72.0,
      Self::Px => 1.0,
      _ => return None,
    })
  }

  fn to_css_str(self) -> &'static str {
    match self {
      Self::Cm => "cm",
      Self::Mm => "mm",
      Self::Q => "q",
      Self::In => "in",
      Self::Pc => "pc",
      Self::Pt => "pt",
      Self::Px => "px",
      Self::Em => "em",
      Self::Cap => "cap",
      Self::Ch => "ch",
      Self::Ex => "ex",
      Self::Ic => "ic",
      Self::Lh => "lh",
      Self::Rem => "rem",
      Self::Rcap => "rcap",
      Self::Rch => "rch",
      Self::Rex => "rex",
      Self::Ric => "ric",
      Self::Rlh => "rlh",
      Self::Zero(unit) => unit,
    }
  }
}

impl Length {
  #[inline]
  pub(crate) fn zero() -> Self {
    Self::from_pixels(0.0)
  }

  #[inline]
  pub(crate) fn from_pixels(value: f64) -> Self {
    Self {
      value,
      unit: LengthUnit::Px,
    }
  }

  #[inline]
  pub fn is_absolute(&self) -> bool {
    self.unit.is_absolute()
  }

  /// Whether resolving this length needs the font metrics.
  #[inline]
  fn is_font_relative(&self) -> bool {
    self.unit.is_font_relative()
  }

  /// The pixel value, when the unit needs no font metrics.
  #[inline]
  pub fn to_pixels(&self) -> Option<f64> {
    Some(self.value * self.unit.px_factor()?)
  }

  /// The pixel value of a length the calculation engine produced.
  ///
  /// Every dimension the engine sees has already been folded to pixels: a unit
  /// only survives at the top level, and the additive and multiplicative layers
  /// are unreachable there because both `Token::ParenthesisBlock` and every math
  /// function require `function_depth > 0`. An unfolded unit here would be a
  /// parser bug rather than bad input.
  #[inline]
  fn folded_pixels(&self) -> f64 {
    self.to_pixels().unwrap_or_else(|| {
      debug_assert!(false, "unfolded {self:?} reached the calculation engine");
      0.0
    })
  }

  /// Resolves a `<length>` to pixels against the given metrics.
  pub fn resolve_to_pixels(&self, resolution: &LengthResolution) -> f64 {
    if let Some(factor) = self.unit.px_factor() {
      return self.value * factor;
    }
    let value = self.value;
    let font = &resolution.font;
    let root = &resolution.root;
    match self.unit {
      LengthUnit::Em => value * font.em,
      LengthUnit::Cap => value * font.cap,
      LengthUnit::Ch => value * font.ch,
      LengthUnit::Ex => value * font.ex,
      LengthUnit::Ic => value * font.ic,
      LengthUnit::Lh => value * font.lh,
      LengthUnit::Rem => value * root.em,
      LengthUnit::Rcap => value * root.cap,
      LengthUnit::Rch => value * root.ch,
      LengthUnit::Rex => value * root.ex,
      LengthUnit::Ric => value * root.ic,
      LengthUnit::Rlh => value * root.lh,
      // Canvas has neither a viewport nor a query container.
      _ => 0.0,
    }
  }

  #[inline]
  fn scaled(&self, factor: f64) -> Self {
    Self {
      value: self.value * factor,
      unit: self.unit,
    }
  }

  #[inline]
  fn abs(&self) -> Self {
    Self {
      value: self.value.abs(),
      unit: self.unit,
    }
  }

  pub fn to_css_string(&self) -> String {
    // CSS numeric literals are parsed at f32 precision (cssparser), so format
    // the value as f32 to avoid f32->f64 widening noise (e.g. `-0.1` becoming
    // `-0.10000000149011612`).
    format!("{}{}", self.value as f32, self.unit.to_css_str())
  }
}

/// A `<length>` expression kept in symbolic form, modelled on CSS Typed OM's
/// `CSSNumericValue` tree, so that font-relative units resolve against the font
/// in effect when the value is used rather than when it was parsed.
/// https://drafts.css-houdini.org/css-typed-om-1/#numeric-objects
#[derive(Clone, Debug, PartialEq)]
pub enum LengthCalc {
  /// `CSSUnitValue`
  Unit(Length),
  /// `CSSMathSum`
  Sum(Box<[LengthCalc]>),
  /// `CSSMathProduct` with a scalar factor. Also covers `CSSMathNegate`
  /// (factor -1) and division by a `<number>` (factor 1/n).
  Scale { factor: f64, value: Box<LengthCalc> },
  /// `CSSMathMin`
  Min(Box<[LengthCalc]>),
  /// `CSSMathMax`
  Max(Box<[LengthCalc]>),
  /// https://www.w3.org/TR/css-values-4/#funcdef-clamp
  Clamp {
    min: Option<Box<LengthCalc>>,
    value: Box<LengthCalc>,
    max: Option<Box<LengthCalc>>,
  },
  /// https://www.w3.org/TR/css-values-4/#round-func
  Round {
    strategy: RoundStrategy,
    value: Box<LengthCalc>,
    interval: Box<LengthCalc>,
  },
  /// https://www.w3.org/TR/css-values-4/#funcdef-mod
  Mod {
    dividend: Box<LengthCalc>,
    divisor: Box<LengthCalc>,
  },
  /// https://www.w3.org/TR/css-values-4/#funcdef-rem
  Rem {
    dividend: Box<LengthCalc>,
    divisor: Box<LengthCalc>,
  },
  /// https://www.w3.org/TR/css-values-4/#funcdef-abs
  Abs(Box<LengthCalc>),
  /// https://www.w3.org/TR/css-values-4/#funcdef-hypot
  Hypot(Box<[LengthCalc]>),
  /// A subexpression whose font dependency flows through a `<number>`, as in
  /// `sqrt(1em / 1px) * 1px`: dividing two lengths leaves the dimension system,
  /// so the tree cannot express it. Retained as specified text (a `<calc-sum>`
  /// body) and re-parsed on use.
  Deferred(Box<str>),
}

impl LengthCalc {
  /// Multiplies by a scalar, folding into the leaf when possible so simple
  /// products do not grow the tree.
  fn scale(self, factor: f64) -> Self {
    match self {
      Self::Unit(length) => Self::Unit(length.scaled(factor)),
      Self::Scale {
        factor: inner,
        value,
      } => Self::Scale {
        factor: inner * factor,
        value,
      },
      value => Self::Scale {
        factor,
        value: Box::new(value),
      },
    }
  }

  /// https://www.w3.org/TR/css-values-4/#funcdef-abs
  fn abs(self) -> Self {
    match self {
      // "Simplify a calculation tree" folds `abs()` of a numeric value.
      Self::Unit(length) => Self::Unit(length.abs()),
      value => Self::Abs(Box::new(value)),
    }
  }

  fn sum(terms: Vec<Self>) -> Self {
    match <[Self; 1]>::try_from(terms) {
      Ok([single]) => single,
      Err(terms) => Self::Sum(terms.into_boxed_slice()),
    }
  }

  pub fn resolve_to_pixels(&self, resolution: &LengthResolution) -> f64 {
    match self {
      Self::Unit(length) => length.resolve_to_pixels(resolution),
      Self::Sum(terms) => terms
        .iter()
        .map(|term| term.resolve_to_pixels(resolution))
        .sum(),
      Self::Scale { factor, value } => {
        value.resolve_to_pixels(resolution) * factor
      }
      Self::Min(terms) => terms
        .iter()
        .map(|term| term.resolve_to_pixels(resolution))
        .fold(f64::INFINITY, minimum),
      Self::Max(terms) => terms
        .iter()
        .map(|term| term.resolve_to_pixels(resolution))
        .fold(f64::NEG_INFINITY, maximum),
      Self::Clamp { min, value, max } => {
        let low = min
          .as_ref()
          .map_or(f64::NEG_INFINITY, |min| min.resolve_to_pixels(resolution));
        let high = max
          .as_ref()
          .map_or(f64::INFINITY, |max| max.resolve_to_pixels(resolution));
        maximum(low, minimum(value.resolve_to_pixels(resolution), high))
      }
      Self::Round {
        strategy,
        value,
        interval,
      } => round_to_interval(
        *strategy,
        value.resolve_to_pixels(resolution),
        interval.resolve_to_pixels(resolution),
      ),
      Self::Mod { dividend, divisor } => dividend
        .resolve_to_pixels(resolution)
        .rem_euclid(divisor.resolve_to_pixels(resolution)),
      Self::Rem { dividend, divisor } => {
        dividend.resolve_to_pixels(resolution)
          % divisor.resolve_to_pixels(resolution)
      }
      Self::Abs(value) => value.resolve_to_pixels(resolution).abs(),
      Self::Hypot(terms) => hypot(
        &terms
          .iter()
          .map(|term| term.resolve_to_pixels(resolution))
          .collect::<Vec<_>>(),
      ),
      // The text parsed once already, so a re-parse only fails if the metrics
      // changed what the expression can produce.
      Self::Deferred(css) => parse_length_text(css, resolution).unwrap_or(0.0),
    }
  }

  /// The unit a term is sorted by. Anything that is not a dimension -- a nested
  /// function, or a retained `Deferred` fragment -- returns `None` and lands in
  /// the trailing group, in its authored order, which is step 5 of "sort a
  /// calculation's children".
  /// https://www.w3.org/TR/css-values-4/#sort-a-calculations-children
  #[inline]
  fn sort_unit(&self) -> Option<&'static str> {
    match self {
      Self::Unit(length) => Some(length.unit.to_css_str()),
      _ => None,
    }
  }

  /// Serializes the node without the outer `calc()` wrapper.
  fn serialize(&self) -> String {
    match self {
      Self::Unit(length) => length.to_css_string(),
      Self::Sum(terms) => {
        // https://www.w3.org/TR/css-values-4/#sort-a-calculations-children
        let mut sorted = terms.iter().collect::<Vec<_>>();
        sorted.sort_by(|a, b| match (a.sort_unit(), b.sort_unit()) {
          (Some(a), Some(b)) => a.cmp(b),
          (Some(_), None) => std::cmp::Ordering::Less,
          (None, Some(_)) => std::cmp::Ordering::Greater,
          (None, None) => std::cmp::Ordering::Equal,
        });
        let mut out = String::new();
        for (index, term) in sorted.iter().enumerate() {
          // A negative leaf reads better as a subtraction, which is also what
          // CSSOM serializes.
          match term {
            Self::Unit(length) if length.value < 0.0 && index > 0 => {
              out.push_str(" - ");
              out.push_str(&length.scaled(-1.0).to_css_string());
              continue;
            }
            _ => {}
          }
          if index > 0 {
            out.push_str(" + ");
          }
          out.push_str(&term.serialize());
        }
        out
      }
      Self::Scale { factor, value } => {
        format!("{} * {}", *factor as f32, value.serialize())
      }
      Self::Min(terms) => format!("min({})", serialize_list(terms)),
      Self::Max(terms) => format!("max({})", serialize_list(terms)),
      Self::Clamp { min, value, max } => {
        let min = min.as_ref().map_or("none".to_string(), |m| m.serialize());
        let max = max.as_ref().map_or("none".to_string(), |m| m.serialize());
        format!("clamp({min}, {}, {max})", value.serialize())
      }
      Self::Round {
        strategy,
        value,
        interval,
      } => {
        let value = value.serialize();
        let interval = interval.serialize();
        match strategy {
          RoundStrategy::Nearest => format!("round({value}, {interval})"),
          RoundStrategy::Up => format!("round(up, {value}, {interval})"),
          RoundStrategy::Down => format!("round(down, {value}, {interval})"),
          RoundStrategy::ToZero => {
            format!("round(to-zero, {value}, {interval})")
          }
        }
      }
      Self::Mod { dividend, divisor } => {
        format!("mod({}, {})", dividend.serialize(), divisor.serialize())
      }
      Self::Rem { dividend, divisor } => {
        format!("rem({}, {})", dividend.serialize(), divisor.serialize())
      }
      Self::Abs(value) => format!("abs({})", value.serialize()),
      Self::Hypot(terms) => format!("hypot({})", serialize_list(terms)),
      Self::Deferred(css) => css.to_string(),
    }
  }

  /// The CSSOM serialization: `calc()` wraps a sum or product, a named function
  /// serializes as itself, and a tree that simplified down to a single dimension
  /// serializes as that dimension.
  /// https://www.w3.org/TR/css-values-4/#calc-serialize
  pub fn to_css_string(&self) -> String {
    match self {
      // A bare sum, product or retained fragment is only valid inside `calc()`.
      Self::Sum(_) | Self::Scale { .. } | Self::Deferred(_) => {
        format!("calc({})", self.serialize())
      }
      _ => self.serialize(),
    }
  }
}

/// A `<length>` as specified, which is what the canvas text styles have to
/// store: the getters serialize it back, and font-relative units only resolve
/// when the value is used.
/// https://html.spec.whatwg.org/multipage/canvas.html#dom-context-2d-letterspacing
#[derive(Clone, Debug, PartialEq)]
pub enum SpecifiedLength {
  /// `CSSUnitValue`: a single dimension, e.g. `3px`, `1ex`, `1vw`.
  Unit(Length),
  /// A math function over font-relative units. `resolved_px` is the value
  /// folded against the metrics in effect at parse time, which is what lets the
  /// calculation engine keep working in pixels; `tree` re-resolves against the
  /// font actually in use.
  Calc {
    resolved_px: f64,
    tree: Rc<LengthCalc>,
  },
}

impl SpecifiedLength {
  #[inline]
  pub(crate) fn zero() -> Self {
    Self::Unit(Length::zero())
  }

  #[inline]
  pub(crate) fn from_pixels(value: f64) -> Self {
    Self::Unit(Length::from_pixels(value))
  }

  #[inline]
  fn from_calc(resolved_px: f64, tree: LengthCalc) -> Self {
    Self::Calc {
      resolved_px,
      tree: Rc::new(tree),
    }
  }

  /// Whether resolving this value needs no font metrics.
  #[inline]
  pub fn is_absolute(&self) -> bool {
    match self {
      Self::Unit(length) => length.is_absolute(),
      Self::Calc { .. } => false,
    }
  }

  /// Whether the font metrics affect this value.
  #[inline]
  fn is_font_dependent(&self) -> bool {
    match self {
      Self::Unit(length) => length.is_font_relative(),
      Self::Calc { .. } => true,
    }
  }

  /// The symbolic form of this value, for building a larger tree.
  #[inline]
  fn to_calc(&self) -> LengthCalc {
    match self {
      Self::Unit(length) => LengthCalc::Unit(*length),
      Self::Calc { tree, .. } => (**tree).clone(),
    }
  }

  /// Drops the symbolic form: a math function becomes the pixel value it folded
  /// to against the metrics it was parsed with.
  #[inline]
  pub fn to_length(&self) -> Length {
    match self {
      Self::Unit(length) => *length,
      Self::Calc { resolved_px, .. } => Length::from_pixels(*resolved_px),
    }
  }

  /// The value in pixels as folded when it was parsed. Only the calculation
  /// engine reads this, to keep its arithmetic in pixels while parsing.
  #[inline]
  fn folded_pixels(&self) -> f64 {
    match self {
      Self::Unit(length) => length.folded_pixels(),
      Self::Calc { resolved_px, .. } => *resolved_px,
    }
  }

  /// Resolves a `<length>` to pixels against the given metrics.
  pub fn resolve_to_pixels(&self, resolution: &LengthResolution) -> f64 {
    match self {
      Self::Unit(length) => length.resolve_to_pixels(resolution),
      Self::Calc { tree, .. } => tree.resolve_to_pixels(resolution),
    }
  }

  pub fn to_css_string(&self) -> String {
    match self {
      Self::Unit(length) => length.to_css_string(),
      Self::Calc { tree, .. } => tree.to_css_string(),
    }
  }

  /// `abs()` keeps the specified unit rather than converting to pixels.
  /// https://www.w3.org/TR/css-values-4/#funcdef-abs
  fn abs(self) -> Self {
    match self {
      Self::Unit(length) => Self::Unit(length.abs()),
      Self::Calc { resolved_px, tree } => {
        Self::from_calc(resolved_px.abs(), (*tree).clone().abs())
      }
    }
  }

  /// The number as written, used by `sign()`, which does no unit conversion.
  #[inline]
  fn raw_value(&self) -> f64 {
    match self {
      Self::Unit(length) => length.value,
      Self::Calc { resolved_px, .. } => *resolved_px,
    }
  }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Angle {
  value: f64,
  unit: AngleUnit,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AngleUnit {
  Deg,
  Grad,
  Rad,
  Turn,
}

impl AngleUnit {
  /// The factor to the canonical unit, `deg`.
  /// https://www.w3.org/TR/css-values-4/#angles
  #[inline]
  fn deg_factor(self) -> f64 {
    match self {
      Self::Deg => 1.0,
      Self::Grad => TURN_TO_DEG / TURN_TO_GRAD,
      Self::Rad => TURN_TO_DEG / f64::consts::TAU,
      Self::Turn => TURN_TO_DEG,
    }
  }

  /// https://www.w3.org/TR/css-values-4/#angles
  #[inline]
  fn parse(unit: &str) -> Option<Self> {
    Some(match_ignore_ascii_case! { unit,
      "deg" => Self::Deg,
      "grad" => Self::Grad,
      "rad" => Self::Rad,
      "turn" => Self::Turn,
      _ => return None,
    })
  }
}

impl Angle {
  #[inline]
  pub(crate) fn zero() -> Self {
    Self::from_degrees(0.0)
  }

  #[inline]
  fn from_degrees(value: f64) -> Self {
    Self {
      value,
      unit: AngleUnit::Deg,
    }
  }

  #[inline]
  fn from_radians(value: f64) -> Self {
    Self {
      value,
      unit: AngleUnit::Rad,
    }
  }

  #[inline]
  pub fn to_degrees(&self) -> f64 {
    self.value * self.unit.deg_factor()
  }

  #[inline]
  pub fn to_radians(&self) -> f64 {
    self.to_degrees().to_radians()
  }
}

#[derive(Clone, Debug)]
#[cfg_attr(test, derive(PartialEq))]
pub struct Time {
  value: f64,
  unit: TimeUnit,
}

#[derive(Clone, Copy, Debug)]
#[cfg_attr(test, derive(Eq, PartialEq))]
enum TimeUnit {
  S,
  Ms,
}

impl TimeUnit {
  /// The factor to the canonical unit, `s`.
  /// https://www.w3.org/TR/css-values-4/#time
  #[inline]
  fn s_factor(self) -> f64 {
    match self {
      Self::S => 1.0,
      Self::Ms => 1.0 / S_TO_MS,
    }
  }

  /// https://www.w3.org/TR/css-values-4/#time
  #[inline]
  fn parse(unit: &str) -> Option<Self> {
    Some(match_ignore_ascii_case! { unit,
      "s" => Self::S,
      "ms" => Self::Ms,
      _ => return None,
    })
  }
}

impl Time {
  #[inline]
  fn from_seconds(value: f64) -> Self {
    Self {
      value,
      unit: TimeUnit::S,
    }
  }

  #[inline]
  pub fn to_seconds(&self) -> f64 {
    self.value * self.unit.s_factor()
  }
}

#[derive(Clone, Debug)]
#[cfg_attr(test, derive(PartialEq))]
pub struct Frequency {
  value: f64,
  unit: FrequencyUnit,
}

#[derive(Clone, Copy, Debug)]
#[cfg_attr(test, derive(Eq, PartialEq))]
enum FrequencyUnit {
  Hz,
  Khz,
}

impl FrequencyUnit {
  /// The factor to the canonical unit, `hz`.
  /// https://www.w3.org/TR/css-values-4/#frequency
  #[inline]
  fn hz_factor(self) -> f64 {
    match self {
      Self::Hz => 1.0,
      Self::Khz => KHZ_TO_HZ,
    }
  }

  /// https://www.w3.org/TR/css-values-4/#frequency
  #[inline]
  fn parse(unit: &str) -> Option<Self> {
    Some(match_ignore_ascii_case! { unit,
      "hz" => Self::Hz,
      "khz" => Self::Khz,
      _ => return None,
    })
  }
}

impl Frequency {
  #[inline]
  fn from_hertz(value: f64) -> Self {
    Self {
      value,
      unit: FrequencyUnit::Hz,
    }
  }

  #[inline]
  pub fn to_hertz(&self) -> f64 {
    self.value * self.unit.hz_factor()
  }
}

#[derive(Clone, Debug)]
#[cfg_attr(test, derive(PartialEq))]
pub struct Resolution {
  value: f64,
  unit: ResolutionUnit,
}

#[derive(Clone, Copy, Debug)]
#[cfg_attr(test, derive(Eq, PartialEq))]
enum ResolutionUnit {
  Dpi,
  Dpcm,
  Dppx,
}

impl ResolutionUnit {
  /// The factor to the canonical unit, `dppx`.
  /// https://www.w3.org/TR/css-values-4/#resolution
  #[inline]
  fn dppx_factor(self) -> f64 {
    match self {
      Self::Dpi => 1.0 / INCH_TO_PX,
      Self::Dpcm => INCH_TO_CM / INCH_TO_PX,
      Self::Dppx => 1.0,
    }
  }

  /// https://www.w3.org/TR/css-values-4/#resolution
  #[inline]
  fn parse(unit: &str) -> Option<Self> {
    Some(match_ignore_ascii_case! { unit,
      "dpi" => Self::Dpi,
      "dpcm" => Self::Dpcm,
      "dppx" | "x" => Self::Dppx,
      _ => return None,
    })
  }
}

impl Resolution {
  #[inline]
  fn from_dot_per_pixels(value: f64) -> Self {
    Self {
      value,
      unit: ResolutionUnit::Dppx,
    }
  }

  #[inline]
  fn to_dot_per_pixels(&self) -> f64 {
    self.value * self.unit.dppx_factor()
  }
}

#[derive(Debug)]
#[cfg_attr(test, derive(PartialEq))]
pub enum NumericValue {
  Zero,
  Number(f64),
  Percent(f64),
  Length(SpecifiedLength),
  Angle(Angle),
  Time(Time),
  Frequency(Frequency),
  Resolution(Resolution),
  Flex(f64),
}

impl From<SpecifiedLength> for NumericValue {
  #[inline]
  fn from(value: SpecifiedLength) -> Self {
    NumericValue::Length(value)
  }
}

impl From<Angle> for NumericValue {
  #[inline]
  fn from(value: Angle) -> Self {
    NumericValue::Angle(value)
  }
}

impl From<Time> for NumericValue {
  #[inline]
  fn from(value: Time) -> Self {
    NumericValue::Time(value)
  }
}

impl From<Frequency> for NumericValue {
  #[inline]
  fn from(value: Frequency) -> Self {
    NumericValue::Frequency(value)
  }
}

impl From<Resolution> for NumericValue {
  #[inline]
  fn from(value: Resolution) -> Self {
    NumericValue::Resolution(value)
  }
}

impl NumericValue {
  #[inline]
  pub fn expect_number(self) -> Result<f64, CSSCustomError> {
    match self {
      NumericValue::Zero => Ok(0.0),
      NumericValue::Number(number) => Ok(number),
      _ => Err(CSSCustomError::UnexpectedNumericType),
    }
  }

  #[inline]
  pub fn expect_percent(self) -> Result<f64, CSSCustomError> {
    match self {
      NumericValue::Percent(percent) => Ok(percent),
      _ => Err(CSSCustomError::UnexpectedNumericType),
    }
  }

  #[inline]
  pub fn expect_number_or_percent(self) -> Result<f64, CSSCustomError> {
    match self {
      NumericValue::Zero => Ok(0.0),
      NumericValue::Number(number) => Ok(number),
      NumericValue::Percent(percent) => Ok(percent),
      _ => Err(CSSCustomError::UnexpectedNumericType),
    }
  }

  /// Extracts a `<length>`, dropping the symbolic form of a math function: what
  /// comes back is the dimension as specified, or the pixel value the function
  /// folded to against the metrics it was parsed with. Callers that have to
  /// survive a later font change want [`Self::expect_specified_length`] instead.
  #[inline]
  pub fn expect_length(
    self,
    allow_zero: bool,
  ) -> Result<Length, CSSCustomError> {
    Ok(self.expect_specified_length(allow_zero)?.to_length())
  }

  /// Extracts a `<length>` as specified, keeping the `LengthCalc` tree of a math
  /// function so it can be re-resolved against whichever font is in effect.
  #[inline]
  pub fn expect_specified_length(
    self,
    allow_zero: bool,
  ) -> Result<SpecifiedLength, CSSCustomError> {
    match self {
      NumericValue::Zero => {
        if allow_zero {
          Ok(SpecifiedLength::from_pixels(0.0))
        } else {
          Err(CSSCustomError::UnexpectedNumericType)
        }
      }
      NumericValue::Length(length) => Ok(length),
      _ => Err(CSSCustomError::UnexpectedNumericType),
    }
  }

  #[inline]
  pub fn expect_angle(self, allow_zero: bool) -> Result<Angle, CSSCustomError> {
    match self {
      NumericValue::Zero => {
        if allow_zero {
          Ok(Angle::from_degrees(0.0))
        } else {
          Err(CSSCustomError::UnexpectedNumericType)
        }
      }
      NumericValue::Angle(angle) => Ok(angle),
      _ => Err(CSSCustomError::UnexpectedNumericType),
    }
  }

  #[inline]
  pub fn expect_time(self) -> Result<Time, CSSCustomError> {
    match self {
      NumericValue::Time(time) => Ok(time),
      _ => Err(CSSCustomError::UnexpectedNumericType),
    }
  }

  #[inline]
  pub fn expect_frequency(self) -> Result<Frequency, CSSCustomError> {
    match self {
      NumericValue::Frequency(frequency) => Ok(frequency),
      _ => Err(CSSCustomError::UnexpectedNumericType),
    }
  }

  #[inline]
  pub fn expect_resolution(self) -> Result<Resolution, CSSCustomError> {
    match self {
      NumericValue::Resolution(resolution) => Ok(resolution),
      _ => Err(CSSCustomError::UnexpectedNumericType),
    }
  }

  #[inline]
  pub fn expect_flex(self) -> Result<f64, CSSCustomError> {
    match self {
      NumericValue::Flex(flex) => Ok(flex),
      _ => Err(CSSCustomError::UnexpectedNumericType),
    }
  }
}

// https://drafts.css-houdini.org/css-typed-om-1/#numeric-typing
#[derive(Debug, PartialEq)]
struct Dimension {
  percent: i8,
  length: i8,
  angle: i8,
  time: i8,
  frequency: i8,
  resolution: i8,
  flex: i8,
}

impl Dimension {
  const NUMBER: Self = Self {
    percent: 0,
    length: 0,
    angle: 0,
    time: 0,
    frequency: 0,
    resolution: 0,
    flex: 0,
  };
  const PERCENT: Self = Self {
    percent: 1,
    ..Self::NUMBER
  };
  const LENGTH: Self = Self {
    length: 1,
    ..Self::NUMBER
  };
  const ANGLE: Self = Self {
    angle: 1,
    ..Self::NUMBER
  };
  const TIME: Self = Self {
    time: 1,
    ..Self::NUMBER
  };
  const FREQUENCY: Self = Self {
    frequency: 1,
    ..Self::NUMBER
  };
  const RESOLUTION: Self = Self {
    resolution: 1,
    ..Self::NUMBER
  };
  const FLEX: Self = Self {
    flex: 1,
    ..Self::NUMBER
  };
}

impl ops::AddAssign<&Dimension> for Dimension {
  #[inline]
  fn add_assign(&mut self, rhs: &Self) {
    self.percent += rhs.percent;
    self.length += rhs.length;
    self.angle += rhs.angle;
    self.time += rhs.time;
    self.frequency += rhs.frequency;
    self.resolution += rhs.resolution;
    self.flex += rhs.flex;
  }
}

impl ops::SubAssign<&Dimension> for Dimension {
  #[inline]
  fn sub_assign(&mut self, rhs: &Self) {
    self.percent -= rhs.percent;
    self.length -= rhs.length;
    self.angle -= rhs.angle;
    self.time -= rhs.time;
    self.frequency -= rhs.frequency;
    self.resolution -= rhs.resolution;
    self.flex -= rhs.flex;
  }
}

// Struct for intermediate representations of calculations like `calc(1px / 1px * 1px)`
// Currently, combined units such as <length-percentage> are not supported
// https://drafts.css-houdini.org/css-typed-om-1/#cssnumericvalue-percent-hint
#[derive(Debug)]
#[cfg_attr(test, derive(PartialEq))]
struct MathValue {
  value: f64,
  dimension: Dimension,
  /// Symbolic `<length>` form, kept while the dimension is a length and no
  /// font-dependent `<number>` has entered the computation.
  calc: Option<LengthCalc>,
  /// Whether a font-relative unit contributed anywhere in this value.
  font_dependent: bool,
}

impl From<NumericValue> for MathValue {
  fn from(value: NumericValue) -> Self {
    match value {
      NumericValue::Zero => MathValue {
        value: 0.0,
        dimension: Dimension::NUMBER,
        calc: None,
        font_dependent: false,
      },
      NumericValue::Number(value) => MathValue {
        value,
        dimension: Dimension::NUMBER,
        calc: None,
        font_dependent: false,
      },
      NumericValue::Percent(value) => MathValue {
        value,
        dimension: Dimension::PERCENT,
        calc: None,
        font_dependent: false,
      },
      NumericValue::Length(length) => MathValue {
        value: length.folded_pixels(),
        dimension: Dimension::LENGTH,
        font_dependent: length.is_font_dependent(),
        calc: Some(length.to_calc()),
      },
      NumericValue::Angle(angle) => {
        let value = angle.to_degrees();
        MathValue {
          value,
          dimension: Dimension::ANGLE,
          calc: None,
          font_dependent: false,
        }
      }
      NumericValue::Time(time) => {
        let value = time.to_seconds();
        MathValue {
          value,
          dimension: Dimension::TIME,
          calc: None,
          font_dependent: false,
        }
      }
      NumericValue::Frequency(frequency) => {
        let value = frequency.to_hertz();
        MathValue {
          value,
          dimension: Dimension::FREQUENCY,
          calc: None,
          font_dependent: false,
        }
      }
      NumericValue::Resolution(resolution) => {
        let value = resolution.to_dot_per_pixels();
        MathValue {
          value,
          dimension: Dimension::RESOLUTION,
          calc: None,
          font_dependent: false,
        }
      }
      NumericValue::Flex(value) => MathValue {
        value,
        dimension: Dimension::FLEX,
        calc: None,
        font_dependent: false,
      },
    }
  }
}

impl TryFrom<MathValue> for NumericValue {
  type Error = CSSCustomError;

  fn try_from(math: MathValue) -> Result<Self, Self::Error> {
    let value = math.value;
    if math.is_number() {
      Ok(NumericValue::Number(value))
    } else if math.is_percent() {
      Ok(NumericValue::Percent(value))
    } else if math.is_length() {
      Ok(math.into_specified_length().into())
    } else if math.is_angle() {
      Ok(Angle::from_degrees(value).into())
    } else if math.is_time() {
      Ok(Time::from_seconds(value).into())
    } else if math.is_frequency() {
      Ok(Frequency::from_hertz(value).into())
    } else if math.is_resolution() {
      Ok(Resolution::from_dot_per_pixels(value).into())
    } else if math.is_flex() {
      Ok(NumericValue::Flex(value))
    } else {
      Err(CSSCustomError::InvalidDimension)
    }
  }
}

macro_rules! impl_math_value_is {
  ($($fn_name:ident: $dim_const:ident),* $(,)?) => {
    $(
      #[inline]
      fn $fn_name(&self) -> bool {
        self.dimension == Dimension::$dim_const
      }
    )*
  };
}

impl MathValue {
  impl_math_value_is! {
    is_number: NUMBER,
    is_percent: PERCENT,
    is_length: LENGTH,
    is_angle: ANGLE,
    is_time: TIME,
    is_frequency: FREQUENCY,
    is_resolution: RESOLUTION,
    is_flex: FLEX,
  }

  fn dimension_mismatch_error(&self, other: &MathValue) -> CSSCustomError {
    if self.is_percent() || other.is_percent() {
      if self.is_length() || other.is_length() {
        return CSSCustomError::ContainPercentAndDimensionCalculations(
          "<length-percentage>",
        );
      } else if self.is_angle() || other.is_angle() {
        return CSSCustomError::ContainPercentAndDimensionCalculations(
          "<angle-percentage>",
        );
      } else if self.is_time() || other.is_time() {
        return CSSCustomError::ContainPercentAndDimensionCalculations(
          "<time-percentage>",
        );
      } else if self.is_frequency() || other.is_frequency() {
        return CSSCustomError::ContainPercentAndDimensionCalculations(
          "<frequency-percentage>",
        );
      }
    }
    CSSCustomError::NumericTypeMismatch
  }

  /// Convert a percent value to an absolute length in pixels using the given base.
  /// Returns self unchanged if not a percent.
  fn resolve_percent_as_length(self, base: f64) -> Self {
    if self.is_percent() {
      // The base is the parent font size, which is fixed for the value being
      // parsed, so the result is a plain pixel length.
      let value = self.value * base;
      MathValue {
        value,
        dimension: Dimension::LENGTH,
        calc: Some(LengthCalc::Unit(Length::from_pixels(value))),
        font_dependent: self.font_dependent,
      }
    } else {
      self
    }
  }

  /// The `<length>` this value represents, keeping its symbolic form when the
  /// font metrics still matter and the tree survived the computation.
  fn into_specified_length(self) -> SpecifiedLength {
    match self.calc {
      Some(tree) if self.font_dependent => {
        SpecifiedLength::from_calc(self.value, tree)
      }
      _ => SpecifiedLength::from_pixels(self.value),
    }
  }

  #[inline]
  fn try_add_assign(
    &mut self,
    other: &MathValue,
  ) -> Result<(), CSSCustomError> {
    if self.dimension != other.dimension {
      return Err(self.dimension_mismatch_error(other));
    }
    self.add_terms(other, 1.0);
    self.value += other.value;
    Ok(())
  }

  #[inline]
  fn try_sub_assign(
    &mut self,
    other: &MathValue,
  ) -> Result<(), CSSCustomError> {
    if self.dimension != other.dimension {
      return Err(self.dimension_mismatch_error(other));
    }
    self.add_terms(other, -1.0);
    self.value -= other.value;
    Ok(())
  }

  /// Combines the symbolic length forms of a sum or difference into a
  /// `CSSMathSum`, dropping it if either side has already lost its form.
  fn add_terms(&mut self, other: &MathValue, sign: f64) {
    self.font_dependent |= other.font_dependent;
    if !self.is_length() {
      self.calc = None;
      return;
    }
    let (Some(left), Some(right)) = (self.calc.take(), other.calc.clone())
    else {
      self.calc = None;
      return;
    };
    let right = if sign < 0.0 { right.scale(sign) } else { right };
    let mut terms = match left {
      LengthCalc::Sum(terms) => terms.into_vec(),
      left => vec![left],
    };
    terms.push(right);
    self.calc = Some(LengthCalc::sum(terms));
  }

  /// Scales the symbolic length form by a `<number>` factor. A factor that is
  /// itself font-dependent, or a quotient of two lengths, cannot be expressed
  /// in the tree, so the form is dropped.
  fn scale_terms(&mut self, other: &MathValue, factor: f64) {
    self.font_dependent |= other.font_dependent;
    if !other.is_number() || other.font_dependent {
      self.calc = None;
      return;
    }
    if let Some(tree) = self.calc.take() {
      self.calc = Some(tree.scale(factor));
    }
  }

  #[inline]
  fn expect_number(self) -> Result<f64, CSSCustomError> {
    if !self.is_number() {
      return Err(CSSCustomError::UnexpectedNumericType);
    }
    Ok(self.value)
  }

  #[inline]
  fn expect_percent(self) -> Result<f64, CSSCustomError> {
    if !self.is_percent() {
      return Err(CSSCustomError::UnexpectedNumericType);
    }
    Ok(self.value)
  }

  #[inline]
  fn expect_length(self) -> Result<Length, CSSCustomError> {
    if !self.is_length() {
      return Err(CSSCustomError::UnexpectedNumericType);
    }
    Ok(Length::from_pixels(self.value))
  }

  #[inline]
  fn expect_specified_length(self) -> Result<SpecifiedLength, CSSCustomError> {
    if !self.is_length() {
      return Err(CSSCustomError::UnexpectedNumericType);
    }
    Ok(self.into_specified_length())
  }

  #[inline]
  fn expect_angle(self) -> Result<Angle, CSSCustomError> {
    if !self.is_angle() {
      return Err(CSSCustomError::UnexpectedNumericType);
    }
    Ok(Angle::from_degrees(self.value))
  }

  #[inline]
  fn expect_time(self) -> Result<Time, CSSCustomError> {
    if !self.is_time() {
      return Err(CSSCustomError::UnexpectedNumericType);
    }
    Ok(Time::from_seconds(self.value))
  }

  #[inline]
  fn expect_frequency(self) -> Result<Frequency, CSSCustomError> {
    if !self.is_frequency() {
      return Err(CSSCustomError::UnexpectedNumericType);
    }
    Ok(Frequency::from_hertz(self.value))
  }

  #[inline]
  fn expect_resolution(self) -> Result<Resolution, CSSCustomError> {
    if !self.is_resolution() {
      return Err(CSSCustomError::UnexpectedNumericType);
    }
    Ok(Resolution::from_dot_per_pixels(self.value))
  }

  #[inline]
  fn expect_flex(self) -> Result<f64, CSSCustomError> {
    if !self.is_flex() {
      return Err(CSSCustomError::UnexpectedNumericType);
    }
    Ok(self.value)
  }
}

impl ops::MulAssign<&MathValue> for MathValue {
  #[inline]
  fn mul_assign(&mut self, rhs: &Self) {
    self.scale_terms(rhs, rhs.value);
    self.value *= rhs.value;
    self.dimension += &rhs.dimension;
  }
}

impl ops::DivAssign<&MathValue> for MathValue {
  #[inline]
  fn div_assign(&mut self, rhs: &Self) {
    self.scale_terms(rhs, 1.0 / rhs.value);
    self.value /= rhs.value;
    self.dimension -= &rhs.dimension;
  }
}

#[derive(Debug)]
#[cfg_attr(test, derive(PartialEq))]
enum NumericAccumulator {
  Numeric(NumericValue),
  Math(MathValue),
}

impl From<NumericValue> for NumericAccumulator {
  #[inline]
  fn from(value: NumericValue) -> Self {
    NumericAccumulator::Numeric(value)
  }
}

impl From<MathValue> for NumericAccumulator {
  #[inline]
  fn from(value: MathValue) -> Self {
    NumericAccumulator::Math(value)
  }
}

impl NumericAccumulator {
  #[inline]
  fn into_math(self) -> MathValue {
    match self {
      NumericAccumulator::Numeric(numeric) => MathValue::from(numeric),
      NumericAccumulator::Math(math) => math,
    }
  }

  #[inline]
  fn expect_numeric(self) -> Result<NumericValue, CSSCustomError> {
    match self {
      NumericAccumulator::Numeric(numeric) => Ok(numeric),
      NumericAccumulator::Math(math) => math.try_into(),
    }
  }

  #[inline]
  fn expect_number(self) -> Result<f64, CSSCustomError> {
    match self {
      NumericAccumulator::Numeric(numeric) => numeric.expect_number(),
      NumericAccumulator::Math(math) => math.expect_number(),
    }
  }

  #[inline]
  fn expect_percent(self) -> Result<f64, CSSCustomError> {
    match self {
      NumericAccumulator::Numeric(numeric) => numeric.expect_percent(),
      NumericAccumulator::Math(math) => math.expect_percent(),
    }
  }

  #[inline]
  fn expect_length(self, allow_zero: bool) -> Result<Length, CSSCustomError> {
    match self {
      NumericAccumulator::Numeric(numeric) => numeric.expect_length(allow_zero),
      NumericAccumulator::Math(math) => math.expect_length(),
    }
  }

  #[inline]
  fn expect_specified_length(
    self,
    allow_zero: bool,
  ) -> Result<SpecifiedLength, CSSCustomError> {
    match self {
      NumericAccumulator::Numeric(numeric) => {
        numeric.expect_specified_length(allow_zero)
      }
      NumericAccumulator::Math(math) => math.expect_specified_length(),
    }
  }

  #[inline]
  fn expect_angle(self, allow_zero: bool) -> Result<Angle, CSSCustomError> {
    match self {
      NumericAccumulator::Numeric(numeric) => numeric.expect_angle(allow_zero),
      NumericAccumulator::Math(math) => math.expect_angle(),
    }
  }

  #[inline]
  fn expect_time(self) -> Result<Time, CSSCustomError> {
    match self {
      NumericAccumulator::Numeric(numeric) => numeric.expect_time(),
      NumericAccumulator::Math(math) => math.expect_time(),
    }
  }

  #[inline]
  fn expect_frequency(self) -> Result<Frequency, CSSCustomError> {
    match self {
      NumericAccumulator::Numeric(numeric) => numeric.expect_frequency(),
      NumericAccumulator::Math(math) => math.expect_frequency(),
    }
  }

  #[inline]
  fn expect_resolution(self) -> Result<Resolution, CSSCustomError> {
    match self {
      NumericAccumulator::Numeric(numeric) => numeric.expect_resolution(),
      NumericAccumulator::Math(math) => math.expect_resolution(),
    }
  }

  #[inline]
  fn expect_flex(self) -> Result<f64, CSSCustomError> {
    match self {
      NumericAccumulator::Numeric(numeric) => numeric.expect_flex(),
      NumericAccumulator::Math(math) => math.expect_flex(),
    }
  }
}

/// Channel keyword substitutions for CSS relative color syntax
/// (e.g. `r`, `g`, `b`, `alpha` in `rgb(from red calc(r / 2) g b)`).
/// Keywords are resolved as plain `<number>` values, per CSS Color 5.
/// https://www.w3.org/TR/css-color-5/#relative-colors
#[derive(Clone, Copy, Debug, Default)]
pub struct ChannelKeywords {
  entries: [Option<(&'static str, f64)>; 4],
}

impl ChannelKeywords {
  #[inline]
  pub fn new(entries: [Option<(&'static str, f64)>; 4]) -> Self {
    Self { entries }
  }

  #[inline]
  fn get(&self, ident: &str) -> Option<f64> {
    self.entries.iter().flatten().find_map(|(name, value)| {
      ident.eq_ignore_ascii_case(name).then_some(*value)
    })
  }
}

#[derive(Default)]
pub struct ParseOptions {
  /// Metrics for resolving font-relative `<length>` units and percentages.
  /// When `None`, any non-absolute `<length>` unit yields
  /// `ContainsRelativeLengthValues`.
  pub length_resolution: Option<LengthResolution>,
  /// Channel keywords resolvable as `<number>` values, used by the CSS
  /// relative color syntax. When `None`, bare identifiers other than calc
  /// constants are rejected.
  pub channel_keywords: Option<ChannelKeywords>,
}

#[derive(Debug)]
struct ParseState {
  function_depth: u8,
  length_resolution: Option<LengthResolution>,
  channel_keywords: Option<ChannelKeywords>,
  /// Set when a font-relative unit was consumed anywhere in the value.
  saw_font_relative: bool,
}

impl ParseState {
  fn new(opts: ParseOptions) -> Self {
    Self {
      function_depth: 0,
      length_resolution: opts.length_resolution,
      channel_keywords: opts.channel_keywords,
      saw_font_relative: false,
    }
  }

  #[inline]
  fn em_base(&self) -> Option<f64> {
    self.length_resolution.map(|resolution| resolution.font.em)
  }
}

macro_rules! extract_as_raw {
  ($expr:expr) => {
    match &$expr {
      NumericValue::Zero => unreachable!(),
      NumericValue::Number(number) => *number,
      NumericValue::Percent(percent) => *percent,
      NumericValue::Length(length) => length.folded_pixels(),
      NumericValue::Angle(angle) => angle.to_degrees(),
      NumericValue::Time(time) => time.to_seconds(),
      NumericValue::Frequency(frequency) => frequency.to_hertz(),
      NumericValue::Resolution(resolution) => resolution.to_dot_per_pixels(),
      NumericValue::Flex(flex) => *flex,
    }
  };
}

macro_rules! try_extract_as_raw {
  ($expr:expr, $type_ref:expr, $input:expr) => {
    match &$type_ref {
      NumericValue::Zero => unreachable!(),
      NumericValue::Number(_) => try_extract!($expr, expect_number(), $input),
      NumericValue::Percent(_) => try_extract!($expr, expect_percent(), $input),
      NumericValue::Length(_) => {
        try_extract!($expr, expect_length(false), folded_pixels(), $input)
      }
      NumericValue::Angle(_) => {
        try_extract!($expr, expect_angle(false), to_degrees(), $input)
      }
      NumericValue::Time(_) => {
        try_extract!($expr, expect_time(), to_seconds(), $input)
      }
      NumericValue::Frequency(_) => {
        try_extract!($expr, expect_frequency(), to_hertz(), $input)
      }
      NumericValue::Resolution(_) => {
        try_extract!($expr, expect_resolution(), to_dot_per_pixels(), $input)
      }
      NumericValue::Flex(_) => try_extract!($expr, expect_flex(), $input),
    }
  };
}

/// Extracts an operand that must have the same numeric kind as `$type_ref`,
/// keeping the `<length>` itself alongside the raw value the calculation engine
/// works with.
macro_rules! try_extract_operand {
  ($expr:expr, $type_ref:expr, $input:expr) => {
    match &$type_ref {
      NumericValue::Length(_) => {
        let length =
          try_extract!($expr, expect_specified_length(false), $input);
        (length.folded_pixels(), Some(length))
      }
      _ => (try_extract_as_raw!($expr, $type_ref, $input), None),
    }
  };
}

/// Like [`from_raw`], but attaches a symbolic `<length>` form when one survived.
macro_rules! from_raw_with_calc {
  ($value:expr, $type_ref:expr, $calc:expr) => {
    match (&$type_ref, $calc) {
      (NumericValue::Length(_), Some(tree)) => {
        NumericValue::Length(SpecifiedLength::from_calc($value, tree))
      }
      _ => from_raw!($value, $type_ref),
    }
  };
}

macro_rules! from_raw {
  ($value:expr, $type_ref:expr) => {
    match &$type_ref {
      NumericValue::Zero => unreachable!(),
      NumericValue::Number(_) => NumericValue::Number($value),
      NumericValue::Percent(_) => NumericValue::Percent($value),
      NumericValue::Length(_) => {
        NumericValue::Length(SpecifiedLength::from_pixels($value))
      }
      NumericValue::Angle(_) => {
        NumericValue::Angle(Angle::from_degrees($value))
      }
      NumericValue::Time(_) => NumericValue::Time(Time::from_seconds($value)),
      NumericValue::Frequency(_) => {
        NumericValue::Frequency(Frequency::from_hertz($value))
      }
      NumericValue::Resolution(_) => {
        NumericValue::Resolution(Resolution::from_dot_per_pixels($value))
      }
      NumericValue::Flex(_) => NumericValue::Flex($value),
    }
  };
}

impl NumericValue {
  pub fn parse<'i, 't>(
    input: &mut Parser<'i, 't>,
    opts: ParseOptions,
  ) -> Result<Self, CSSParseError<'i>> {
    let result = Self::parse_inner(input, &mut ParseState::new(opts))?;
    match result.expect_numeric() {
      Ok(numeric) => Ok(numeric),
      Err(error) => Err(input.new_custom_error(error)),
    }
  }

  fn parse_inner<'i, 't>(
    input: &mut Parser<'i, 't>,
    state: &mut ParseState,
  ) -> Result<NumericAccumulator, CSSParseError<'i>> {
    let token = input.next()?;
    match token {
      Token::Number { value, .. } => {
        // Due to historical reasons, <transform-function> must allow <zero> (the literal `0`) for <length> and <angle>
        // https://www.w3.org/TR/css-values-4/#zero-value
        if state.function_depth == 0 && *value == 0.0 {
          return Ok(NumericValue::Zero.into());
        }
        Ok(NumericValue::Number(*value as f64).into())
      }
      Token::Percentage { unit_value, .. } => {
        Ok(NumericValue::Percent(*unit_value as f64).into())
      }
      Token::Dimension { value, unit, .. } => {
        let value = *value as f64;
        // Non-absolute units are only accepted when metrics are provided (font
        // and spacing contexts). At the top level they keep their original unit
        // so they can be resolved lazily against the font actually in effect;
        // inside math functions they must be folded to pixels so the dimension
        // arithmetic stays in pixels.
        if let Some(unit) = LengthUnit::parse(unit) {
          let length = Length { value, unit };
          if unit.is_absolute() {
            return Ok(
              NumericValue::Length(SpecifiedLength::Unit(length)).into(),
            );
          }
          if unit.is_font_relative() {
            state.saw_font_relative = true;
          }
          let Some(resolution) = state.length_resolution else {
            return Err(
              input
                .new_custom_error(CSSCustomError::ContainsRelativeLengthValues),
            );
          };
          if state.function_depth == 0 {
            return Ok(
              NumericValue::Length(SpecifiedLength::Unit(length)).into(),
            );
          }
          // Inside a math function the engine works in pixels, so fold the
          // value. Font-relative units keep their symbolic form so they can be
          // re-resolved; viewport and container units are a constant zero, so
          // the folded pixel value stays correct.
          let px = length.resolve_to_pixels(&resolution);
          return Ok(
            NumericValue::Length(if unit.is_font_relative() {
              SpecifiedLength::from_calc(px, LengthCalc::Unit(length))
            } else {
              SpecifiedLength::from_pixels(px)
            })
            .into(),
          );
        }
        if let Some(unit) = AngleUnit::parse(unit) {
          return Ok(NumericValue::Angle(Angle { value, unit }).into());
        }
        if let Some(unit) = TimeUnit::parse(unit) {
          return Ok(NumericValue::Time(Time { value, unit }).into());
        }
        if let Some(unit) = FrequencyUnit::parse(unit) {
          return Ok(NumericValue::Frequency(Frequency { value, unit }).into());
        }
        if let Some(unit) = ResolutionUnit::parse(unit) {
          return Ok(
            NumericValue::Resolution(Resolution { value, unit }).into(),
          );
        }
        // https://www.w3.org/TR/css-grid-2/#fr-unit
        if unit.eq_ignore_ascii_case("fr") {
          return Ok(NumericValue::Flex(value).into());
        }
        let token = token.clone();
        Err(input.new_unexpected_token_error(token))
      }
      Token::Function(name) => {
        state.function_depth += 1;
        let result = match_ignore_ascii_case! { &name,
          // https://www.w3.org/TR/css-values-4/#calc-func
          "calc" => {
            input.parse_nested_block(|arguments| {
              let acc = Self::parse_additive_expression(arguments, state)?;
              arguments.expect_exhausted()?;
              Ok(acc)
            })
          },
          // https://www.w3.org/TR/css-values-4/#comp-func
          "min" => {
            input.parse_nested_block(|arguments| {
              let acc = Self::parse_additive_expression(arguments, state)?;
              let numeric = try_extract!(acc, expect_numeric(), arguments);
              let mut current = extract_as_raw!(numeric);
              let mut operands = LengthOperands::start(&numeric);
              while !arguments.is_exhausted() {
                arguments.expect_comma()?;
                let acc = Self::parse_additive_expression(arguments, state)?;
                let (value, operand) = try_extract_operand!(acc, numeric, arguments);
                push_operand(&mut operands, &operand);
                current = minimum(current, value);
              }
              let calc = operands
                .and_then(LengthOperands::into_trees)
                .map(|trees| LengthCalc::Min(trees.into_boxed_slice()));
              Ok(from_raw_with_calc!(current, numeric, calc).into())
            })
          },
          "max" => {
            input.parse_nested_block(|arguments| {
              let acc = Self::parse_additive_expression(arguments, state)?;
              let numeric = try_extract!(acc, expect_numeric(), arguments);
              let mut current = extract_as_raw!(numeric);
              let mut operands = LengthOperands::start(&numeric);
              while !arguments.is_exhausted() {
                arguments.expect_comma()?;
                let acc = Self::parse_additive_expression(arguments, state)?;
                let (value, operand) = try_extract_operand!(acc, numeric, arguments);
                push_operand(&mut operands, &operand);
                current = maximum(current, value);
              }
              let calc = operands
                .and_then(LengthOperands::into_trees)
                .map(|trees| LengthCalc::Max(trees.into_boxed_slice()));
              Ok(from_raw_with_calc!(current, numeric, calc).into())
            })
          },
          "clamp" => {
            input.parse_nested_block(|arguments| {
              let min: Option<NumericValue> = {
                let start = arguments.state();
                if arguments.expect_ident_matching("none").is_ok() {
                  None
                } else {
                  arguments.reset(&start);
                  let acc = Self::parse_additive_expression(arguments, state)?;
                  let numeric = try_extract!(acc, expect_numeric(), arguments);
                  Some(numeric)
                }
              };
              arguments.expect_comma()?;
              let acc = Self::parse_additive_expression(arguments, state)?;
              let numeric = try_extract!(acc, expect_numeric(), arguments);
              arguments.expect_comma()?;
              let max: Option<NumericValue> = {
                let start = arguments.state();
                if arguments.expect_ident_matching("none").is_ok() {
                  None
                } else {
                  arguments.reset(&start);
                  let acc = Self::parse_additive_expression(arguments, state)?;
                  let numeric = try_extract!(acc, expect_numeric(), arguments);
                  Some(numeric)
                }
              };
              arguments.expect_exhausted()?;

              let (min, min_operand) = match min {
                Some(value) => try_extract_operand!(value, numeric, arguments),
                None => (f64::NEG_INFINITY, None),
              };
              let (max, max_operand) = match max {
                Some(value) => try_extract_operand!(value, numeric, arguments),
                None => (f64::INFINITY, None),
              };
              let calc = LengthOperands::start(&numeric).and_then(|mut operands| {
                let value = operands.trees[0].clone();
                let min = min_operand.map(|operand| {
                  operands.push(&operand);
                  Box::new(operands.trees.pop().unwrap())
                });
                let max = max_operand.map(|operand| {
                  operands.push(&operand);
                  Box::new(operands.trees.pop().unwrap())
                });
                operands.font_dependent.then_some(LengthCalc::Clamp {
                  min,
                  value: Box::new(value),
                  max,
                })
              });
              let value = extract_as_raw!(numeric);
              let result = maximum(min, minimum(value, max));
              Ok(from_raw_with_calc!(result, numeric, calc).into())
            })
          },
          // https://www.w3.org/TR/css-values-4/#round-func
          "round" => {
            input.parse_nested_block(|arguments| {
              let strategy = {
                let start = arguments.state();
                let token = arguments.next()?;
                match token {
                  Token::Ident(ident) => {
                    let strategy = match_ignore_ascii_case! { &ident,
                      "nearest" => RoundStrategy::Nearest,
                      "up" => RoundStrategy::Up,
                      "down" => RoundStrategy::Down,
                      "to-zero" => RoundStrategy::ToZero,
                      _ => {
                        let token = token.clone();
                        return Err(arguments.new_unexpected_token_error(token))
                      }
                    };
                    arguments.expect_comma()?;
                    strategy
                  },
                  _ => {
                    arguments.reset(&start);
                    RoundStrategy::Nearest
                  }
                }
              };
              let acc = Self::parse_additive_expression(arguments, state)?;
              let numeric = try_extract!(acc, expect_numeric(), arguments);
              let mut operands = LengthOperands::start(&numeric);
              let interval = if !arguments.is_exhausted() {
                arguments.expect_comma()?;
                let acc = Self::parse_additive_expression(arguments, state)?;
                let (interval, operand) = try_extract_operand!(acc, numeric, arguments);
                push_operand(&mut operands, &operand);
                arguments.expect_exhausted()?;
                interval
              } else {
                push_operand(
                  &mut operands,
                  &Some(SpecifiedLength::from_pixels(1.0)),
                );
                1.0
              };
              let value = extract_as_raw!(numeric);
              let result = round_to_interval(strategy, value, interval);
              let calc = operands.and_then(LengthOperands::into_trees).map(
                |mut trees| LengthCalc::Round {
                  strategy,
                  interval: Box::new(trees.pop().unwrap()),
                  value: Box::new(trees.remove(0)),
                },
              );
              Ok(from_raw_with_calc!(result, numeric, calc).into())
            })
          },
          "mod" => {
            input.parse_nested_block(|arguments| {
              let acc = Self::parse_additive_expression(arguments, state)?;
              let numeric = try_extract!(acc, expect_numeric(), arguments);
              let dividend = extract_as_raw!(numeric);
              let mut operands = LengthOperands::start(&numeric);
              arguments.expect_comma()?;
              let acc = Self::parse_additive_expression(arguments, state)?;
              let (divisor, operand) = try_extract_operand!(acc, numeric, arguments);
              push_operand(&mut operands, &operand);
              arguments.expect_exhausted()?;
              let result = dividend.rem_euclid(divisor);
              let calc = operands.and_then(LengthOperands::into_trees).map(
                |mut trees| LengthCalc::Mod {
                  divisor: Box::new(trees.pop().unwrap()),
                  dividend: Box::new(trees.remove(0)),
                },
              );
              Ok(from_raw_with_calc!(result, numeric, calc).into())
            })
          },
          "rem" => {
            input.parse_nested_block(|arguments| {
              let acc = Self::parse_additive_expression(arguments, state)?;
              let numeric = try_extract!(acc, expect_numeric(), arguments);
              let dividend = extract_as_raw!(numeric);
              let mut operands = LengthOperands::start(&numeric);
              arguments.expect_comma()?;
              let acc = Self::parse_additive_expression(arguments, state)?;
              let (divisor, operand) = try_extract_operand!(acc, numeric, arguments);
              push_operand(&mut operands, &operand);
              arguments.expect_exhausted()?;
              let result = dividend % divisor;
              let calc = operands.and_then(LengthOperands::into_trees).map(
                |mut trees| LengthCalc::Rem {
                  divisor: Box::new(trees.pop().unwrap()),
                  dividend: Box::new(trees.remove(0)),
                },
              );
              Ok(from_raw_with_calc!(result, numeric, calc).into())
            })
          },
          // https://www.w3.org/TR/css-values-4/#trig-funcs
          "sin" => {
            input.parse_nested_block(|arguments| {
              let acc = Self::parse_additive_expression(arguments, state)?;
              let numeric = try_extract!(acc, expect_numeric(), arguments);
              arguments.expect_exhausted()?;
              let result: NumericAccumulator = match numeric {
                NumericValue::Zero => unreachable!(),
                NumericValue::Number(number) => {
                  NumericValue::Number(number.sin()).into()
                }
                NumericValue::Angle(angle) => {
                  NumericValue::Number(angle.to_radians().sin()).into()
                }
                _ => return Err(arguments.new_custom_error(CSSCustomError::UnexpectedNumericType)),
              };
              Ok(result)
            })
          },
          "cos" => {
            input.parse_nested_block(|arguments| {
              let acc = Self::parse_additive_expression(arguments, state)?;
              let numeric = try_extract!(acc, expect_numeric(), arguments);
              arguments.expect_exhausted()?;
              let result: NumericAccumulator = match numeric {
                NumericValue::Zero => unreachable!(),
                NumericValue::Number(number) => {
                  NumericValue::Number(number.cos()).into()
                }
                NumericValue::Angle(angle) => {
                  NumericValue::Number(angle.to_radians().cos()).into()
                }
                _ => return Err(arguments.new_custom_error(CSSCustomError::UnexpectedNumericType)),
              };
              Ok(result)
            })
          },
          "tan" => {
            input.parse_nested_block(|arguments| {
              let acc = Self::parse_additive_expression(arguments, state)?;
              let numeric = try_extract!(acc, expect_numeric(), arguments);
              arguments.expect_exhausted()?;
              let result: NumericAccumulator = match numeric {
                NumericValue::Zero => unreachable!(),
                NumericValue::Number(number) => {
                  NumericValue::Number(number.tan()).into()
                }
                NumericValue::Angle(angle) => {
                  NumericValue::Number(angle.to_radians().tan()).into()
                }
                _ => return Err(arguments.new_custom_error(CSSCustomError::UnexpectedNumericType)),
              };
              Ok(result)
            })
          },
          "asin" => {
            input.parse_nested_block(|arguments| {
              let acc = Self::parse_additive_expression(arguments, state)?;
              let number = try_extract!(acc, expect_number(), arguments);
              arguments.expect_exhausted()?;
              let result: NumericAccumulator = NumericValue::Angle(Angle::from_radians(number.asin())).into();
              Ok(result)
            })
          },
          "acos" => {
            input.parse_nested_block(|arguments| {
              let acc = Self::parse_additive_expression(arguments, state)?;
              let number = try_extract!(acc, expect_number(), arguments);
              arguments.expect_exhausted()?;
              let result: NumericAccumulator = NumericValue::Angle(Angle::from_radians(number.acos())).into();
              Ok(result)
            })
          },
          "atan" => {
            input.parse_nested_block(|arguments| {
              let acc = Self::parse_additive_expression(arguments, state)?;
              let number = try_extract!(acc, expect_number(), arguments);
              arguments.expect_exhausted()?;
              let result: NumericAccumulator = NumericValue::Angle(Angle::from_radians(number.atan())).into();
              Ok(result)
            })
          },
          "atan2" => {
            input.parse_nested_block(|arguments| {
              let acc = Self::parse_additive_expression(arguments, state)?;
              let numeric = try_extract!(acc, expect_numeric(), arguments);
              let y = extract_as_raw!(numeric);
              arguments.expect_comma()?;
              let acc = Self::parse_additive_expression(arguments, state)?;
              let x = try_extract_as_raw!(acc, numeric, arguments);
              arguments.expect_exhausted()?;
              let result = NumericValue::Angle(Angle::from_radians(y.atan2(x))).into();
              Ok(result)
            })
          },
          // https://www.w3.org/TR/css-values-4/#exponent-funcs
          "pow" => {
            input.parse_nested_block(|arguments| {
              let acc = Self::parse_additive_expression(arguments, state)?;
              let base = try_extract!(acc, expect_number(), arguments);
              arguments.expect_comma()?;
              let acc = Self::parse_additive_expression(arguments, state)?;
              let exponent = try_extract!(acc, expect_number(), arguments);
              arguments.expect_exhausted()?;
              let result = NumericValue::Number(base.powf(exponent)).into();
              Ok(result)
            })
          },
          "sqrt" => {
            input.parse_nested_block(|arguments| {
              let acc = Self::parse_additive_expression(arguments, state)?;
              let value = try_extract!(acc, expect_number(), arguments);
              arguments.expect_exhausted()?;
              let result = NumericValue::Number(value.sqrt()).into();
              Ok(result)
            })
          },
          "hypot" => {
            input.parse_nested_block(|arguments| {
              let acc = Self::parse_additive_expression(arguments, state)?;
              let numeric = try_extract!(acc, expect_numeric(), arguments);
              let value = extract_as_raw!(numeric);
              let mut operands = LengthOperands::start(&numeric);
              let mut args = vec![value];
              while !arguments.is_exhausted() {
                arguments.expect_comma()?;
                let acc = Self::parse_additive_expression(arguments, state)?;
                let (value, operand) = try_extract_operand!(acc, numeric, arguments);
                push_operand(&mut operands, &operand);
                args.push(value);
              }
              let result = hypot(&args);
              let calc = operands
                .and_then(LengthOperands::into_trees)
                .map(|trees| LengthCalc::Hypot(trees.into_boxed_slice()));
              Ok(from_raw_with_calc!(result, numeric, calc).into())
            })
          },
          "log" => {
            input.parse_nested_block(|arguments| {
              let acc = Self::parse_additive_expression(arguments, state)?;
              let value = try_extract!(acc, expect_number(), arguments);
              let result: NumericAccumulator = if !arguments.is_exhausted() {
                arguments.expect_comma()?;
                let acc = Self::parse_additive_expression(arguments, state)?;
                let base = try_extract!(acc, expect_number(), arguments);
                arguments.expect_exhausted()?;
                NumericValue::Number(value.log(base)).into()
              } else {
                NumericValue::Number(value.ln()).into()
              };
              Ok(result)
            })
          },
          "exp" => {
            input.parse_nested_block(|arguments| {
              let acc = Self::parse_additive_expression(arguments, state)?;
              let number = try_extract!(acc, expect_number(), arguments);
              arguments.expect_exhausted()?;
              let result: NumericAccumulator = NumericValue::Number(number.exp()).into();
              Ok(result)
            })
          },
          // https://www.w3.org/TR/css-values-4/#sign-funcs
          "abs" => {
            input.parse_nested_block(|arguments| {
              let acc = Self::parse_additive_expression(arguments, state)?;
              let numeric = try_extract!(acc, expect_numeric(), arguments);
              arguments.expect_exhausted()?;
              // NOTE: extract_as_raw! is not used because unit conversion is exceptionally not performed.
              let result: NumericAccumulator = match numeric {
                NumericValue::Zero => unreachable!(),
                NumericValue::Number(number) => {
                  NumericValue::Number(number.abs()).into()
                }
                NumericValue::Percent(percent) => {
                  NumericValue::Percent(percent.abs()).into()
                }
                NumericValue::Length(length) => {
                  NumericValue::Length(length.abs()).into()
                }
                NumericValue::Angle(angle) => {
                  NumericValue::Angle(Angle {
                    value: angle.value.abs(),
                    unit: angle.unit,
                  }).into()
                }
                NumericValue::Time(time) => {
                  NumericValue::Time(Time {
                    value: time.value.abs(),
                    unit: time.unit,
                  }).into()
                }
                NumericValue::Frequency(frequency) => {
                  NumericValue::Frequency(Frequency {
                    value: frequency.value.abs(),
                    unit: frequency.unit,
                  }).into()
                }
                NumericValue::Resolution(resolution) => {
                  NumericValue::Resolution(Resolution {
                    value: resolution.value.abs(),
                    unit: resolution.unit,
                  }).into()
                }
                NumericValue::Flex(flex) => {
                  NumericValue::Flex(flex.abs()).into()
                }
              };
              Ok(result)
            })
          },
          "sign" => {
            #[inline]
            fn sign(value: f64) -> f64 {
              if value == 0.0 { value } else { value.signum() }
            }

            input.parse_nested_block(|arguments| {
              let acc = Self::parse_additive_expression(arguments, state)?;
              let numeric = try_extract!(acc, expect_numeric(), arguments);
              arguments.expect_exhausted()?;
              // NOTE: extract_as_raw! is not used because unit conversion is exceptionally not performed.
              let value = match numeric {
                NumericValue::Zero => unreachable!(),
                NumericValue::Number(number) => number,
                NumericValue::Percent(percent) => percent,
                NumericValue::Length(length) => length.raw_value(),
                NumericValue::Angle(angle) => angle.value,
                NumericValue::Time(time) => time.value,
                NumericValue::Frequency(frequency) => frequency.value,
                NumericValue::Resolution(resolution) => resolution.value,
                NumericValue::Flex(flex) => flex,
              };
              Ok(NumericValue::Number(sign(value)).into())
            })
          },
          _ => {
            let name = name.to_string();
            return Err(input.new_custom_error(CSSCustomError::InvalidFunction(name)))
          },
        };
        state.function_depth -= 1;
        result
      }
      Token::ParenthesisBlock => {
        if state.function_depth == 0 {
          let token = token.clone();
          return Err(input.new_unexpected_token_error(token));
        }
        input.parse_nested_block(|arguments| {
          let acc = Self::parse_additive_expression(arguments, state)?;
          arguments.expect_exhausted()?;
          Ok(acc)
        })
      }
      Token::Ident(ident) => {
        // Channel keywords of the relative color syntax resolve as
        // `<number>` values both inside and outside of math functions.
        if let Some(channel_keywords) = &state.channel_keywords
          && let Some(value) = channel_keywords.get(ident)
        {
          return Ok(NumericValue::Number(value).into());
        }
        if state.function_depth == 0 {
          let token = token.clone();
          return Err(input.new_unexpected_token_error(token));
        }
        match_ignore_ascii_case! { &ident,
          // https://www.w3.org/TR/css-values-4/#calc-constants
          "e" => Ok(NumericValue::Number(f64::consts::E).into()),
          "pi" => Ok(NumericValue::Number(f64::consts::PI).into()),
          // https://www.w3.org/TR/css-values-4/#calc-error-constants
          "infinity" => Ok(NumericValue::Number(f64::INFINITY).into()),
          "-infinity" => Ok(NumericValue::Number(f64::NEG_INFINITY).into()),
          "nan" => Ok(NumericValue::Number(f64::NAN).into()),
          _ => {
            let token = token.clone();
            Err(input.new_unexpected_token_error(token))
          }
        }
      }
      _ => {
        let token = token.clone();
        Err(input.new_unexpected_token_error(token))
      }
    }
  }

  fn parse_additive_expression<'i, 't>(
    input: &mut Parser<'i, 't>,
    state: &mut ParseState,
  ) -> Result<NumericAccumulator, CSSParseError<'i>> {
    let span_start = input.position();
    let mut lhs = Self::parse_multiplicative_expression(input, state)?;

    while !input.is_exhausted() {
      let start = input.state();
      let token = input.next_including_whitespace()?;
      if let Token::WhiteSpace(_) = token {
        let token = input.next()?;
        match token {
          Token::Delim('+') => {
            input.expect_whitespace()?;
            let rhs = Self::parse_multiplicative_expression(input, state)?;
            let mut left = lhs.into_math();
            let mut right = rhs.into_math();
            if let Some(base) = state.em_base() {
              if left.is_length() && right.is_percent() {
                right = right.resolve_percent_as_length(base);
              } else if left.is_percent() && right.is_length() {
                left = left.resolve_percent_as_length(base);
              }
            }
            if let Err(error) = left.try_add_assign(&right) {
              return Err(input.new_custom_error(error));
            }
            lhs = left.into();
          }
          Token::Delim('-') => {
            input.expect_whitespace()?;
            let rhs = Self::parse_multiplicative_expression(input, state)?;
            let mut left = lhs.into_math();
            let mut right = rhs.into_math();
            if let Some(base) = state.em_base() {
              if left.is_length() && right.is_percent() {
                right = right.resolve_percent_as_length(base);
              } else if left.is_percent() && right.is_length() {
                left = left.resolve_percent_as_length(base);
              }
            }
            if let Err(error) = left.try_sub_assign(&right) {
              return Err(input.new_custom_error(error));
            }
            lhs = left.into();
          }
          _ => {
            input.reset(&start);
            break;
          }
        }
      } else {
        input.reset(&start);
        break;
      }
    }

    Ok(retain_lost_length(lhs, input, span_start))
  }

  /// Parses one factor, marking it font-dependent when it consumed a
  /// font-relative unit. A `<number>` derived from one (`sqrt(1em / 1px)`)
  /// carries no unit, so only this bookkeeping keeps the dependency visible.
  fn parse_operand<'i, 't>(
    input: &mut Parser<'i, 't>,
    state: &mut ParseState,
  ) -> Result<MathValue, CSSParseError<'i>> {
    let before = state.saw_font_relative;
    let rhs = Self::parse_inner(input, state)?;
    let mut rhs = rhs.into_math();
    rhs.font_dependent |= state.saw_font_relative != before;
    Ok(rhs)
  }

  fn parse_multiplicative_expression<'i, 't>(
    input: &mut Parser<'i, 't>,
    state: &mut ParseState,
  ) -> Result<NumericAccumulator, CSSParseError<'i>> {
    let span_start = input.position();
    let before = state.saw_font_relative;
    let mut lhs = Self::parse_inner(input, state)?;
    // The first factor can hide a font dependency too, as in
    // `calc(sqrt(1em / 1px) * 1px)`, so it needs the same bookkeeping. Once it
    // has been folded into a `MathValue` the flag lives there instead.
    let mut lhs_font_dependent = state.saw_font_relative != before;

    while !input.is_exhausted() {
      let start = input.state();
      let token = input.next()?;
      match token {
        Token::Delim('*') => {
          let rhs = Self::parse_operand(input, state)?;
          let mut left = lhs.into_math();
          left.font_dependent |= lhs_font_dependent;
          lhs_font_dependent = false;
          left *= &rhs;
          lhs = left.into();
        }
        Token::Delim('/') => {
          let rhs = Self::parse_operand(input, state)?;
          let mut left = lhs.into_math();
          left.font_dependent |= lhs_font_dependent;
          lhs_font_dependent = false;
          left /= &rhs;
          lhs = left.into();
        }
        _ => {
          input.reset(&start);
          break;
        }
      }
    }

    Ok(retain_lost_length(lhs, input, span_start))
  }
}

/// https://www.w3.org/TR/css-values-4/#round-func
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RoundStrategy {
  Nearest,
  Up,
  Down,
  ToZero,
}

fn round_to_interval(
  strategy: RoundStrategy,
  value: f64,
  interval: f64,
) -> f64 {
  if interval == 0.0
    || value.is_nan()
    || interval.is_nan()
    || value.is_infinite() && interval.is_infinite()
  {
    return f64::NAN;
  }
  if value.is_infinite() {
    return value;
  }
  if interval.is_infinite() {
    return match strategy {
      RoundStrategy::Up => {
        if value > 0.0 {
          f64::INFINITY
        } else if value == 0.0 && value.is_sign_positive() {
          0.0
        } else {
          -0.0
        }
      }
      RoundStrategy::Down => {
        if value < 0.0 {
          f64::NEG_INFINITY
        } else if value == 0.0 && value.is_sign_negative() {
          -0.0
        } else {
          0.0
        }
      }
      RoundStrategy::Nearest | RoundStrategy::ToZero => {
        if value.is_sign_positive() { 0.0 } else { -0.0 }
      }
    };
  }
  let interval = interval.abs();
  let quotient = value / interval;
  let rounded = match strategy {
    RoundStrategy::Nearest => (quotient + 0.5).floor(),
    RoundStrategy::Up => quotient.ceil(),
    RoundStrategy::Down => quotient.floor(),
    RoundStrategy::ToZero => quotient.trunc(),
  };
  rounded * interval
}

/// https://www.w3.org/TR/css-values-4/#funcdef-hypot
fn hypot(args: &[f64]) -> f64 {
  match *args {
    [] => 0.0,
    [arg1] => arg1.abs(),
    [arg1, arg2] => arg1.hypot(arg2),
    _ => {
      let mut sum = 0.0;
      let mut scale = 0.0;
      for &arg in args {
        let value = arg.abs();
        if !value.is_finite() {
          return value;
        }
        if scale < value {
          let div = scale / value;
          sum = sum * div * div + 1.0;
          scale = value;
        } else if value > 0.0 {
          let div = value / scale;
          sum += div * div;
        }
      }
      scale * sum.sqrt()
    }
  }
}

fn serialize_list(terms: &[LengthCalc]) -> String {
  terms
    .iter()
    .map(LengthCalc::serialize)
    .collect::<Vec<_>>()
    .join(", ")
}

/// Re-parses a retained `<calc-sum>` body and resolves it against `resolution`.
fn parse_length_text(css: &str, resolution: &LengthResolution) -> Option<f64> {
  let css = format!("calc({css})");
  let mut input = ParserInput::new(&css);
  let mut parser = Parser::new(&mut input);
  let value = NumericValue::parse(
    &mut parser,
    ParseOptions {
      length_resolution: Some(*resolution),
      ..Default::default()
    },
  )
  .ok()?;
  parser.is_exhausted().then_some(())?;
  // The re-parse already folded every unit against `resolution`, and
  // `expect_length` drops the tree, so this cannot recurse back into here.
  Some(
    value
      .expect_length(true)
      .ok()?
      .resolve_to_pixels(resolution),
  )
}

/// Retains the source text of a `<length>` subexpression that lost its symbolic
/// form, so it can be re-parsed against the font in use.
///
/// The only way to lose it is a `<number>` factor that itself depends on the
/// font (`sqrt(1em / 1px) * 1px`), because dividing two lengths leaves the
/// dimension system. Capturing here keeps the retained fragment as tight as
/// possible: the rest of the expression stays a tree.
fn retain_lost_length(
  accumulator: NumericAccumulator,
  input: &Parser<'_, '_>,
  span_start: SourcePosition,
) -> NumericAccumulator {
  let NumericAccumulator::Math(mut math) = accumulator else {
    return accumulator;
  };
  if math.is_length() && math.font_dependent && math.calc.is_none() {
    let css = input.slice_from(span_start).trim();
    math.calc = Some(LengthCalc::Deferred(Box::from(css)));
  }
  NumericAccumulator::Math(math)
}

/// The `<length>` operands of a math function, collected so the result can keep
/// a symbolic form when it still depends on the font metrics.
struct LengthOperands {
  trees: Vec<LengthCalc>,
  font_dependent: bool,
}

impl LengthOperands {
  /// `None` when the function is not operating on `<length>` values.
  #[inline]
  fn start(value: &NumericValue) -> Option<Self> {
    match value {
      NumericValue::Length(length) => Some(Self {
        trees: vec![length.to_calc()],
        font_dependent: length.is_font_dependent(),
      }),
      _ => None,
    }
  }

  #[inline]
  fn push(&mut self, value: &SpecifiedLength) {
    self.font_dependent |= value.is_font_dependent();
    self.trees.push(value.to_calc());
  }

  /// The operands, only when the result still depends on the font metrics; an
  /// expression over absolute units is already exact in pixels.
  #[inline]
  fn into_trees(self) -> Option<Vec<LengthCalc>> {
    self.font_dependent.then_some(self.trees)
  }
}

/// Appends an operand when both the collector and the operand are lengths.
#[inline]
fn push_operand(
  operands: &mut Option<LengthOperands>,
  value: &Option<SpecifiedLength>,
) {
  if let (Some(operands), Some(value)) = (operands.as_mut(), value) {
    operands.push(value);
  }
}

#[cfg(test)]
mod tests {
  use approx::assert_relative_eq;
  use cssparser::BasicParseErrorKind;
  use cssparser::ParseErrorKind;

  use super::*;

  #[test]
  fn zero() {
    let mut input = ParserInput::new("0.0");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser, ParseOptions::default());
    assert_eq!(result, Ok(NumericValue::Zero));
  }

  #[test]
  fn number() {
    let mut input = ParserInput::new("42");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser, ParseOptions::default());
    assert_eq!(result, Ok(NumericValue::Number(42.0)));
  }

  #[test]
  fn percent() {
    let mut input = ParserInput::new("50%");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser, ParseOptions::default());
    assert_eq!(result, Ok(NumericValue::Percent(0.5)));
  }

  #[test]
  fn length() {
    let mut input = ParserInput::new("-1cm");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser, ParseOptions::default());
    let Ok(NumericValue::Length(length)) = result else {
      panic!("expect length: {:?}", result);
    };
    assert_eq!(
      length,
      SpecifiedLength::Unit(Length {
        value: -1.0,
        unit: LengthUnit::Cm,
      })
    );
    assert_relative_eq!(length.to_length().to_pixels().unwrap(), -96.0 / 2.54);
  }

  #[test]
  fn angle() {
    let mut input = ParserInput::new("180deg");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser, ParseOptions::default());
    let Ok(NumericValue::Angle(angle)) = result else {
      panic!("expect angle: {:?}", result);
    };
    assert_eq!(
      angle,
      Angle {
        value: 180.0,
        unit: AngleUnit::Deg,
      }
    );
    assert_eq!(angle.to_degrees(), 180.0);
    assert_relative_eq!(angle.to_radians(), f64::consts::PI);
  }

  #[test]
  fn time() {
    let mut input = ParserInput::new("3s");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser, ParseOptions::default());
    let Ok(NumericValue::Time(time)) = result else {
      panic!("expect time: {:?}", result);
    };
    assert_eq!(
      time,
      Time {
        value: 3.0,
        unit: TimeUnit::S,
      }
    );
    assert_eq!(time.to_seconds(), 3.0);
  }

  #[test]
  fn frequency() {
    let mut input = ParserInput::new("3hz");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser, ParseOptions::default());
    let Ok(NumericValue::Frequency(frequency)) = result else {
      panic!("expect frequency: {:?}", result);
    };
    assert_eq!(
      frequency,
      Frequency {
        value: 3.0,
        unit: FrequencyUnit::Hz,
      }
    );
    assert_eq!(frequency.to_hertz(), 3.0);
  }

  #[test]
  fn resolution() {
    let mut input = ParserInput::new("3dppx");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser, ParseOptions::default());
    let Ok(NumericValue::Resolution(resolution)) = result else {
      panic!("expect resolution: {:?}", result);
    };
    assert_eq!(
      resolution,
      Resolution {
        value: 3.0,
        unit: ResolutionUnit::Dppx,
      }
    );
    assert_eq!(resolution.to_dot_per_pixels(), 3.0);
  }

  #[test]
  fn canonical_unit_conversions() {
    fn parse_one(css: &str) -> NumericValue {
      let mut input = ParserInput::new(css);
      let mut parser = Parser::new(&mut input);
      NumericValue::parse(&mut parser, ParseOptions::default()).unwrap()
    }
    fn px(css: &str) -> f64 {
      parse_one(css)
        .expect_length(false)
        .unwrap()
        .to_pixels()
        .unwrap()
    }
    fn deg(css: &str) -> f64 {
      parse_one(css).expect_angle(false).unwrap().to_degrees()
    }
    fn rad(css: &str) -> f64 {
      parse_one(css).expect_angle(false).unwrap().to_radians()
    }

    // Every unit converts to its dimension's canonical unit. The inputs are all
    // exact in f32, which is the precision cssparser reads literals at.
    // https://www.w3.org/TR/css-values-4/#canonical-unit
    assert_relative_eq!(px("1in"), 96.0);
    assert_relative_eq!(px("72pt"), 96.0);
    assert_relative_eq!(px("6pc"), 96.0);
    assert_relative_eq!(px("1cm"), 96.0 / 2.54, epsilon = 1e-12);
    assert_relative_eq!(px("1mm"), 96.0 / 25.4, epsilon = 1e-12);
    assert_relative_eq!(px("1q"), 96.0 / 101.6, epsilon = 1e-12);

    assert_relative_eq!(deg("100grad"), 90.0);
    assert_relative_eq!(deg("0.5turn"), 180.0);
    assert_relative_eq!(deg("1rad"), 1.0f64.to_degrees());
    assert_relative_eq!(rad("180deg"), f64::consts::PI);
    assert_relative_eq!(rad("0.5turn"), f64::consts::PI);
    assert_relative_eq!(rad("200grad"), f64::consts::PI);

    let NumericValue::Time(time) = parse_one("500ms") else {
      panic!("expect time");
    };
    assert_relative_eq!(time.to_seconds(), 0.5);

    let NumericValue::Frequency(frequency) = parse_one("2khz") else {
      panic!("expect frequency");
    };
    assert_relative_eq!(frequency.to_hertz(), 2000.0);

    let NumericValue::Resolution(resolution) = parse_one("96dpi") else {
      panic!("expect resolution");
    };
    assert_relative_eq!(resolution.to_dot_per_pixels(), 1.0);
    let NumericValue::Resolution(resolution) = parse_one("1dpcm") else {
      panic!("expect resolution");
    };
    assert_relative_eq!(
      resolution.to_dot_per_pixels(),
      2.54 / 96.0,
      epsilon = 1e-12
    );
  }

  #[test]
  fn flex() {
    let mut input = ParserInput::new("1fr");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser, ParseOptions::default());
    assert_eq!(result, Ok(NumericValue::Flex(1.0)));
  }

  #[test]
  fn calc_zero() {
    let mut input = ParserInput::new("calc(0)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser, ParseOptions::default());
    assert_eq!(result, Ok(NumericValue::Number(0.0)));
  }

  #[test]
  fn calc_const_e() {
    let mut input = ParserInput::new("calc(e)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser, ParseOptions::default());
    assert_eq!(result, Ok(NumericValue::Number(f64::consts::E)));
  }

  #[test]
  fn calc_const_pi() {
    let mut input = ParserInput::new("calc(pi)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser, ParseOptions::default());
    assert_eq!(result, Ok(NumericValue::Number(f64::consts::PI)));
  }

  #[test]
  fn calc_const_infinity() {
    let mut input = ParserInput::new("calc(infinity)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser, ParseOptions::default());
    assert_eq!(result, Ok(NumericValue::Number(f64::INFINITY)));
  }

  #[test]
  fn calc_const_neg_infinity() {
    let mut input = ParserInput::new("calc(-infinity)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser, ParseOptions::default());
    assert_eq!(result, Ok(NumericValue::Number(f64::NEG_INFINITY)));
  }

  #[test]
  fn calc_const_nan() {
    let mut input = ParserInput::new("calc(nan)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser, ParseOptions::default());
    let Ok(NumericValue::Number(value)) = result else {
      panic!("expect number: {:?}", result);
    };
    assert!(value.is_nan());
  }

  #[test]
  fn calc() {
    let mut input = ParserInput::new("calc(1px + 2 * 3px)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser, ParseOptions::default());
    assert_eq!(
      result,
      Ok(NumericValue::Length(SpecifiedLength::Unit(Length {
        value: 7.0,
        unit: LengthUnit::Px,
      })))
    );
  }

  #[test]
  fn calc_parenthesis() {
    let mut input = ParserInput::new("calc((1px + 2px) * 3)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser, ParseOptions::default());
    assert_eq!(
      result,
      Ok(NumericValue::Length(SpecifiedLength::Unit(Length {
        value: 9.0,
        unit: LengthUnit::Px,
      })))
    );
  }

  #[test]
  fn calc_failed_by_whitespace() {
    let mut input = ParserInput::new("calc(1+2)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser, ParseOptions::default());
    assert!(result.is_err_and(|error| matches!(
      error.kind,
      ParseErrorKind::Basic(BasicParseErrorKind::UnexpectedToken(_))
    )));
  }

  #[test]
  fn calc_failed_by_type_mismatch() {
    let mut input = ParserInput::new("calc(1px + 2deg)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser, ParseOptions::default());
    assert!(result.is_err_and(|error| error.kind
      == ParseErrorKind::Custom(CSSCustomError::NumericTypeMismatch)));
  }

  #[test]
  fn calc_dimension() {
    let mut input = ParserInput::new("calc(1px * 1deg * 1% / 1deg / 1%)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser, ParseOptions::default());
    assert_eq!(
      result,
      Ok(NumericValue::Length(SpecifiedLength::Unit(Length {
        value: 1.0,
        unit: LengthUnit::Px,
      })))
    );
  }

  #[test]
  fn calc_zero_dimension() {
    let mut input = ParserInput::new("calc(2px / 1px)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser, ParseOptions::default());
    assert_eq!(result, Ok(NumericValue::Number(2.0)));
  }

  #[test]
  fn calc_failed_by_dimension() {
    let mut input = ParserInput::new("calc(1px * 1deg * 1% / 1deg)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser, ParseOptions::default());
    assert!(result.is_err_and(|error| error.kind
      == ParseErrorKind::Custom(CSSCustomError::InvalidDimension)));
  }

  #[test]
  fn min() {
    let mut input = ParserInput::new("min(-1, 1 - 3, 3)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser, ParseOptions::default());
    assert_eq!(result, Ok(NumericValue::Number(-2.0)));
  }

  #[test]
  fn min_nan() {
    let mut input = ParserInput::new("min(-1, nan, 3)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser, ParseOptions::default());
    let Ok(NumericValue::Number(value)) = result else {
      panic!("expect number: {:?}", result);
    };
    assert!(value.is_nan());
  }

  #[test]
  fn min_length() {
    let mut input = ParserInput::new("min(-1px, 1px - 3px, 3px)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser, ParseOptions::default());
    assert_eq!(
      result,
      Ok(NumericValue::Length(SpecifiedLength::Unit(Length {
        value: -2.0,
        unit: LengthUnit::Px,
      })))
    );
  }

  #[test]
  fn max() {
    let mut input = ParserInput::new("max(-1, 1 - 3, 3)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser, ParseOptions::default());
    assert_eq!(result, Ok(NumericValue::Number(3.0)));
  }

  #[test]
  fn max_nan() {
    let mut input = ParserInput::new("max(-1, nan, 3)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser, ParseOptions::default());
    let Ok(NumericValue::Number(value)) = result else {
      panic!("expect number: {:?}", result);
    };
    assert!(value.is_nan());
  }

  #[test]
  fn max_length() {
    let mut input = ParserInput::new("max(-1px, 1px - 3px, 3px)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser, ParseOptions::default());
    assert_eq!(
      result,
      Ok(NumericValue::Length(SpecifiedLength::Unit(Length {
        value: 3.0,
        unit: LengthUnit::Px,
      })))
    );
  }

  #[test]
  fn clamp() {
    let mut input = ParserInput::new("clamp(-1, 1 - 3, 3)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser, ParseOptions::default());
    assert_eq!(result, Ok(NumericValue::Number(-1.0)));
  }

  #[test]
  fn clamp_none() {
    let mut input = ParserInput::new("clamp(none, 1 - 3, 3)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser, ParseOptions::default());
    assert_eq!(result, Ok(NumericValue::Number(-2.0)));
  }

  #[test]
  fn clamp_nan() {
    let mut input = ParserInput::new("clamp(-1, nan, 3)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser, ParseOptions::default());
    let Ok(NumericValue::Number(value)) = result else {
      panic!("expect number: {:?}", result);
    };
    assert!(value.is_nan());
  }

  #[test]
  fn clamp_length() {
    let mut input = ParserInput::new("clamp(-1px, 1px - 3px, 3px)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser, ParseOptions::default());
    assert_eq!(
      result,
      Ok(NumericValue::Length(SpecifiedLength::Unit(Length {
        value: -1.0,
        unit: LengthUnit::Px,
      })))
    );
  }

  #[test]
  fn round() {
    let mut input = ParserInput::new("round(1.5)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser, ParseOptions::default());
    assert_eq!(result, Ok(NumericValue::Number(2.0)));
  }

  #[test]
  fn round_with_interval() {
    let mut input = ParserInput::new("round(1, 2)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser, ParseOptions::default());
    assert_eq!(result, Ok(NumericValue::Number(2.0)));
  }

  #[test]
  fn round_with_strategy() {
    let mut input = ParserInput::new("round(to-zero, 2.5, 5)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser, ParseOptions::default());
    assert_eq!(result, Ok(NumericValue::Number(0.0)));
  }

  #[test]
  fn round_with_interval_infinity() {
    let mut input = ParserInput::new("round(down, 1, infinity)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser, ParseOptions::default());
    assert_eq!(result, Ok(NumericValue::Number(0.0)));
  }

  #[test]
  fn round_nan() {
    let mut input = ParserInput::new("round(up, nan, 3)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser, ParseOptions::default());
    let Ok(NumericValue::Number(value)) = result else {
      panic!("expect number: {:?}", result);
    };
    assert!(value.is_nan());
  }

  #[test]
  fn round_length() {
    let mut input = ParserInput::new("round(-1.5px)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser, ParseOptions::default());
    assert_eq!(
      result,
      Ok(NumericValue::Length(SpecifiedLength::Unit(Length {
        value: -1.0,
        unit: LengthUnit::Px,
      })))
    );
  }

  #[test]
  fn modulo() {
    let mut input = ParserInput::new("mod(-3, 2)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser, ParseOptions::default());
    assert_eq!(result, Ok(NumericValue::Number(1.0)));
  }

  #[test]
  fn modulo_zero() {
    let mut input = ParserInput::new("mod(2, 0)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser, ParseOptions::default());
    let Ok(NumericValue::Number(value)) = result else {
      panic!("expect number: {:?}", result);
    };
    assert!(value.is_nan());
  }

  #[test]
  fn modulo_length() {
    let mut input = ParserInput::new("mod(3px, 2px)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser, ParseOptions::default());
    assert_eq!(
      result,
      Ok(NumericValue::Length(SpecifiedLength::Unit(Length {
        value: 1.0,
        unit: LengthUnit::Px,
      })))
    );
  }

  #[test]
  fn rem() {
    let mut input = ParserInput::new("rem(-3, 2)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser, ParseOptions::default());
    assert_eq!(result, Ok(NumericValue::Number(-1.0)));
  }

  #[test]
  fn rem_zero() {
    let mut input = ParserInput::new("rem(2, 0)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser, ParseOptions::default());
    let Ok(NumericValue::Number(value)) = result else {
      panic!("expect number: {:?}", result);
    };
    assert!(value.is_nan());
  }

  #[test]
  fn rem_length() {
    let mut input = ParserInput::new("mod(3px, 2px)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser, ParseOptions::default());
    assert_eq!(
      result,
      Ok(NumericValue::Length(SpecifiedLength::Unit(Length {
        value: 1.0,
        unit: LengthUnit::Px,
      })))
    );
  }

  #[test]
  fn sin() {
    let mut input = ParserInput::new("sin(pi / 2)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser, ParseOptions::default());
    let Ok(NumericValue::Number(value)) = result else {
      panic!("expect number: {:?}", result);
    };
    assert_relative_eq!(value, 1.0);
  }

  #[test]
  fn sin_angle() {
    let mut input = ParserInput::new("sin(90deg)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser, ParseOptions::default());
    let Ok(NumericValue::Number(value)) = result else {
      panic!("expect number: {:?}", result);
    };
    assert_relative_eq!(value, 1.0);
  }

  #[test]
  fn cos() {
    let mut input = ParserInput::new("cos(pi)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser, ParseOptions::default());
    let Ok(NumericValue::Number(value)) = result else {
      panic!("expect number: {:?}", result);
    };
    assert_relative_eq!(value, -1.0);
  }

  #[test]
  fn cos_angle() {
    let mut input = ParserInput::new("cos(180deg)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser, ParseOptions::default());
    let Ok(NumericValue::Number(value)) = result else {
      panic!("expect number: {:?}", result);
    };
    assert_relative_eq!(value, -1.0);
  }

  #[test]
  fn tan() {
    let mut input = ParserInput::new("tan(pi / 4)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser, ParseOptions::default());
    let Ok(NumericValue::Number(value)) = result else {
      panic!("expect number: {:?}", result);
    };
    assert_relative_eq!(value, 1.0);
  }

  #[test]
  fn tan_angle() {
    let mut input = ParserInput::new("tan(45deg)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser, ParseOptions::default());
    let Ok(NumericValue::Number(value)) = result else {
      panic!("expect number: {:?}", result);
    };
    assert_relative_eq!(value, 1.0);
  }

  #[test]
  fn asin() {
    let mut input = ParserInput::new("asin(-1)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser, ParseOptions::default());
    let Ok(NumericValue::Angle(angle)) = result else {
      panic!("expect angle: {:?}", result);
    };
    assert_relative_eq!(angle.to_degrees(), -90.0);
  }

  #[test]
  fn acos() {
    let mut input = ParserInput::new("acos(-1)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser, ParseOptions::default());
    let Ok(NumericValue::Angle(angle)) = result else {
      panic!("expect angle: {:?}", result);
    };
    assert_relative_eq!(angle.to_degrees(), 180.0);
  }

  #[test]
  fn atan() {
    let mut input = ParserInput::new("atan(-1)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser, ParseOptions::default());
    let Ok(NumericValue::Angle(angle)) = result else {
      panic!("expect angle: {:?}", result);
    };
    assert_relative_eq!(angle.to_degrees(), -45.0);
  }

  #[test]
  fn atan2() {
    let mut input = ParserInput::new("atan2(1, -1)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser, ParseOptions::default());
    let Ok(NumericValue::Angle(angle)) = result else {
      panic!("expect angle: {:?}", result);
    };
    assert_relative_eq!(angle.to_degrees(), 135.0);
  }

  #[test]
  fn atan2_length() {
    let mut input = ParserInput::new("atan2(1px, -1px)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser, ParseOptions::default());
    let Ok(NumericValue::Angle(angle)) = result else {
      panic!("expect angle: {:?}", result);
    };
    assert_relative_eq!(angle.to_degrees(), 135.0);
  }

  #[test]
  fn pow() {
    let mut input = ParserInput::new("pow(2, 3)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser, ParseOptions::default());
    let Ok(NumericValue::Number(value)) = result else {
      panic!("expect number: {:?}", result);
    };
    assert_relative_eq!(value, 8.0);
  }

  #[test]
  fn sqrt() {
    let mut input = ParserInput::new("sqrt(4)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser, ParseOptions::default());
    let Ok(NumericValue::Number(value)) = result else {
      panic!("expect number: {:?}", result);
    };
    assert_relative_eq!(value, 2.0);
  }

  #[test]
  fn hypot() {
    let mut input = ParserInput::new("hypot(3, 4, 12)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser, ParseOptions::default());
    let Ok(NumericValue::Number(value)) = result else {
      panic!("expect number: {:?}", result);
    };
    assert_relative_eq!(value, 13.0);
  }

  #[test]
  fn hypot_length() {
    let mut input = ParserInput::new("hypot(3px, 4px, 12px)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser, ParseOptions::default());
    let Ok(NumericValue::Length(length)) = result else {
      panic!("expect length: {:?}", result);
    };
    assert_relative_eq!(length.to_length().to_pixels().unwrap(), 13.0);
  }

  #[test]
  fn log() {
    let mut input = ParserInput::new("log(10)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser, ParseOptions::default());
    let Ok(NumericValue::Number(value)) = result else {
      panic!("expect number: {:?}", result);
    };
    assert_relative_eq!(value, 10.0_f64.ln());
  }

  #[test]
  fn log_multi_args() {
    let mut input = ParserInput::new("log(8, 2)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser, ParseOptions::default());
    let Ok(NumericValue::Number(value)) = result else {
      panic!("expect number: {:?}", result);
    };
    assert_relative_eq!(value, 3.0);
  }

  #[test]
  fn exp() {
    let mut input = ParserInput::new("exp(2)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser, ParseOptions::default());
    let Ok(NumericValue::Number(value)) = result else {
      panic!("expect number: {:?}", result);
    };
    assert_relative_eq!(value, 2.0_f64.exp());
  }

  #[test]
  fn abs() {
    let mut input = ParserInput::new("abs(-3)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser, ParseOptions::default());
    assert_eq!(result, Ok(NumericValue::Number(3.0)));
  }

  #[test]
  fn abs_length() {
    let mut input = ParserInput::new("abs(-3px)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser, ParseOptions::default());
    assert_eq!(
      result,
      Ok(NumericValue::Length(SpecifiedLength::Unit(Length {
        value: 3.0,
        unit: LengthUnit::Px,
      })))
    );
  }

  #[test]
  fn sign() {
    let mut input = ParserInput::new("sign(-2)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser, ParseOptions::default());
    assert_eq!(result, Ok(NumericValue::Number(-1.0)));
  }

  #[test]
  fn sign_zero() {
    let mut input = ParserInput::new("sign(0)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser, ParseOptions::default());
    let Ok(NumericValue::Number(value)) = result else {
      panic!("expect number: {:?}", result);
    };
    assert_eq!(value, 0.0);
    assert!(value.is_sign_positive());
  }

  #[test]
  fn sign_neg_zero() {
    let mut input = ParserInput::new("sign(-0)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser, ParseOptions::default());
    let Ok(NumericValue::Number(value)) = result else {
      panic!("expect number: {:?}", result);
    };
    assert_eq!(value, -0.0);
    assert!(value.is_sign_negative());
  }

  #[test]
  fn sign_length() {
    let mut input = ParserInput::new("sign(-2px)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser, ParseOptions::default());
    assert_eq!(result, Ok(NumericValue::Number(-1.0)));
  }

  #[test]
  fn channel_keyword_bare() {
    let mut input = ParserInput::new("g");
    let mut parser = Parser::new(&mut input);
    let options = ParseOptions {
      channel_keywords: Some(ChannelKeywords::new([
        Some(("r", 255.0)),
        Some(("g", 128.0)),
        Some(("b", 0.0)),
        Some(("alpha", 1.0)),
      ])),
      ..Default::default()
    };
    let result = NumericValue::parse(&mut parser, options);
    assert_eq!(result, Ok(NumericValue::Number(128.0)));
  }

  #[test]
  fn channel_keyword_case_insensitive() {
    let mut input = ParserInput::new("ALPHA");
    let mut parser = Parser::new(&mut input);
    let options = ParseOptions {
      channel_keywords: Some(ChannelKeywords::new([
        Some(("r", 255.0)),
        Some(("g", 128.0)),
        Some(("b", 0.0)),
        Some(("alpha", 1.0)),
      ])),
      ..Default::default()
    };
    let result = NumericValue::parse(&mut parser, options);
    assert_eq!(result, Ok(NumericValue::Number(1.0)));
  }

  #[test]
  fn channel_keyword_in_calc() {
    let mut input = ParserInput::new("calc(r / 2 + g)");
    let mut parser = Parser::new(&mut input);
    let options = ParseOptions {
      channel_keywords: Some(ChannelKeywords::new([
        Some(("r", 255.0)),
        Some(("g", 128.0)),
        Some(("b", 0.0)),
        Some(("alpha", 1.0)),
      ])),
      ..Default::default()
    };
    let result = NumericValue::parse(&mut parser, options);
    assert_eq!(result, Ok(NumericValue::Number(255.5)));
  }

  #[test]
  fn channel_keyword_unknown_ident() {
    let mut input = ParserInput::new("h");
    let mut parser = Parser::new(&mut input);
    let options = ParseOptions {
      channel_keywords: Some(ChannelKeywords::new([
        Some(("r", 255.0)),
        Some(("g", 128.0)),
        Some(("b", 0.0)),
        Some(("alpha", 1.0)),
      ])),
      ..Default::default()
    };
    let result = NumericValue::parse(&mut parser, options);
    assert!(result.is_err());
  }

  #[test]
  fn channel_keyword_disabled() {
    let mut input = ParserInput::new("r");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser, ParseOptions::default());
    assert!(result.is_err());
  }

  #[test]
  fn channel_keyword_calc_constants_still_work() {
    let mut input = ParserInput::new("calc(pi)");
    let mut parser = Parser::new(&mut input);
    let options = ParseOptions {
      channel_keywords: Some(ChannelKeywords::new([
        Some(("r", 255.0)),
        Some(("g", 128.0)),
        Some(("b", 0.0)),
        Some(("alpha", 1.0)),
      ])),
      ..Default::default()
    };
    let result = NumericValue::parse(&mut parser, options);
    assert_eq!(result, Ok(NumericValue::Number(f64::consts::PI)));
  }
}
