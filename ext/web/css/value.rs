// Copyright 2018-2026 the Deno authors. MIT license.

use std::f64;
use std::ops;
use std::rc::Rc;

pub use cssparser::Parser;
pub use cssparser::ParserInput;
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

/// Font metrics in px for font-relative `<length>` units.
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
  /// `lh`: the used line height.
  pub lh: f64,
}

impl FontMetrics {
  /// The ratio assumed for a `normal` line height when no font metrics are
  /// available.
  const NORMAL_LINE_HEIGHT_RATIO: f64 = 1.2;

  /// CSS fallback metrics. `cap` reuses the 0.8em assumed ascent.
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

/// The size a viewport- or container-percentage `<length>` resolves against, in
/// pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct BoxSize {
  pub width: f64,
  pub height: f64,
}

impl BoxSize {
  #[inline]
  fn min(&self) -> f64 {
    minimum(self.width, self.height)
  }

  #[inline]
  fn max(&self) -> f64 {
    maximum(self.width, self.height)
  }
}

/// Relative `<length>` context. `new` has no root/viewport (`rem` == `em`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LengthResolution {
  pub font: FontMetrics,
  pub root: FontMetrics,
  /// What a `<percentage>` is 1% of. `None` rejects `%` on a plain `<length>`.
  /// https://www.w3.org/TR/css-values-4/#mixed-percentages
  pub percentage_basis: Option<f64>,
  /// The initial containing block, which every viewport- and
  /// container-percentage unit resolves against.
  pub viewport: BoxSize,
}

impl LengthResolution {
  #[inline]
  pub fn new(font: FontMetrics) -> Self {
    Self {
      font,
      root: font,
      percentage_basis: None,
      viewport: BoxSize::default(),
    }
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
  // https://www.w3.org/TR/css-values-4/#viewport-relative-lengths
  Vw,
  Vh,
  Vi,
  Vb,
  Vmin,
  Vmax,
  Svw,
  Svh,
  Svi,
  Svb,
  Svmin,
  Svmax,
  Lvw,
  Lvh,
  Lvi,
  Lvb,
  Lvmin,
  Lvmax,
  Dvw,
  Dvh,
  Dvi,
  Dvb,
  Dvmin,
  Dvmax,
  // https://drafts.csswg.org/css-conditional-5/#container-lengths
  Cqw,
  Cqh,
  Cqi,
  Cqb,
  Cqmin,
  Cqmax,
  /// A `<percentage>`, which is a `<length>` only given a basis.
  /// https://www.w3.org/TR/css-values-4/#mixed-percentages
  Percent,
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
      "vw" => Self::Vw,
      "vh" => Self::Vh,
      "vi" => Self::Vi,
      "vb" => Self::Vb,
      "vmin" => Self::Vmin,
      "vmax" => Self::Vmax,
      "svw" => Self::Svw,
      "svh" => Self::Svh,
      "svi" => Self::Svi,
      "svb" => Self::Svb,
      "svmin" => Self::Svmin,
      "svmax" => Self::Svmax,
      "lvw" => Self::Lvw,
      "lvh" => Self::Lvh,
      "lvi" => Self::Lvi,
      "lvb" => Self::Lvb,
      "lvmin" => Self::Lvmin,
      "lvmax" => Self::Lvmax,
      "dvw" => Self::Dvw,
      "dvh" => Self::Dvh,
      "dvi" => Self::Dvi,
      "dvb" => Self::Dvb,
      "dvmin" => Self::Dvmin,
      "dvmax" => Self::Dvmax,
      // https://drafts.csswg.org/css-conditional-5/#container-lengths
      "cqw" => Self::Cqw,
      "cqh" => Self::Cqh,
      "cqi" => Self::Cqi,
      "cqb" => Self::Cqb,
      "cqmin" => Self::Cqmin,
      "cqmax" => Self::Cqmax,
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

  /// The size this unit is 1% of. All box-relative units share one box.
  /// https://www.w3.org/TR/css-values-4/#mixed-percentages
  /// https://www.w3.org/TR/css-values-4/#viewport-relative-lengths
  /// https://drafts.csswg.org/css-conditional-5/#container-lengths
  #[inline]
  fn percentage_basis(self, resolution: &LengthResolution) -> Option<f64> {
    let viewport = &resolution.viewport;
    Some(match self {
      Self::Vw
      | Self::Vi
      | Self::Svw
      | Self::Svi
      | Self::Lvw
      | Self::Lvi
      | Self::Dvw
      | Self::Dvi
      | Self::Cqw
      | Self::Cqi => viewport.width,
      Self::Vh
      | Self::Vb
      | Self::Svh
      | Self::Svb
      | Self::Lvh
      | Self::Lvb
      | Self::Dvh
      | Self::Dvb
      | Self::Cqh
      | Self::Cqb => viewport.height,
      Self::Vmin | Self::Svmin | Self::Lvmin | Self::Dvmin | Self::Cqmin => {
        viewport.min()
      }
      Self::Vmax | Self::Svmax | Self::Lvmax | Self::Dvmax | Self::Cqmax => {
        viewport.max()
      }
      Self::Percent => return resolution.percentage_basis,
      _ => return None,
    })
  }

  /// Factor to `px`. `None` if the unit needs font metrics or a viewport.
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
      Self::Vw => "vw",
      Self::Vh => "vh",
      Self::Vi => "vi",
      Self::Vb => "vb",
      Self::Vmin => "vmin",
      Self::Vmax => "vmax",
      Self::Svw => "svw",
      Self::Svh => "svh",
      Self::Svi => "svi",
      Self::Svb => "svb",
      Self::Svmin => "svmin",
      Self::Svmax => "svmax",
      Self::Lvw => "lvw",
      Self::Lvh => "lvh",
      Self::Lvi => "lvi",
      Self::Lvb => "lvb",
      Self::Lvmin => "lvmin",
      Self::Lvmax => "lvmax",
      Self::Dvw => "dvw",
      Self::Dvh => "dvh",
      Self::Dvi => "dvi",
      Self::Dvb => "dvb",
      Self::Dvmin => "dvmin",
      Self::Dvmax => "dvmax",
      Self::Cqw => "cqw",
      Self::Cqh => "cqh",
      Self::Cqi => "cqi",
      Self::Cqb => "cqb",
      Self::Cqmin => "cqmin",
      Self::Cqmax => "cqmax",
      Self::Percent => "%",
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

  /// The pixel value, when the unit needs no font metrics.
  #[inline]
  pub fn to_pixels(&self) -> Option<f64> {
    Some(self.value * self.unit.px_factor()?)
  }

  /// Pixel value from the calculation engine. An unfolded unit here is a bug.
  #[inline]
  fn folded_pixels(&self) -> f64 {
    self.to_pixels().unwrap_or_else(|| {
      debug_assert!(false, "unfolded {self:?} reached the calculation engine");
      0.0
    })
  }

  pub fn resolve_to_pixels(&self, resolution: &LengthResolution) -> f64 {
    if let Some(factor) = self.unit.px_factor() {
      return self.value * factor;
    }
    if let Some(basis) = self.unit.percentage_basis(resolution) {
      return self.value * basis / 100.0;
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
      // Handled by `px_factor` and `percentage_basis` above.
      _ => {
        debug_assert!(false, "no resolution rule for {self:?}");
        0.0
      }
    }
  }

  pub fn to_css_string(&self) -> String {
    // Format as f32 to avoid cssparser f32→f64 widening noise.
    format!("{}{}", self.value as f32, self.unit.to_css_str())
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

  fn to_css_str(self) -> &'static str {
    match self {
      Self::Deg => "deg",
      Self::Grad => "grad",
      Self::Rad => "rad",
      Self::Turn => "turn",
    }
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
  pub fn to_degrees(&self) -> f64 {
    self.value * self.unit.deg_factor()
  }

  #[inline]
  pub fn to_radians(&self) -> f64 {
    self.to_degrees().to_radians()
  }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Time {
  value: f64,
  unit: TimeUnit,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
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

  fn to_css_str(self) -> &'static str {
    match self {
      Self::S => "s",
      Self::Ms => "ms",
    }
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

#[derive(Clone, Debug, PartialEq)]
pub struct Frequency {
  value: f64,
  unit: FrequencyUnit,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
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

  fn to_css_str(self) -> &'static str {
    match self {
      Self::Hz => "hz",
      Self::Khz => "khz",
    }
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

#[derive(Clone, Debug, PartialEq)]
pub struct Resolution {
  value: f64,
  unit: ResolutionUnit,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
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

  fn to_css_str(self) -> &'static str {
    match self {
      Self::Dpi => "dpi",
      Self::Dpcm => "dpcm",
      Self::Dppx => "dppx",
    }
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

/// Numeric types with a canonical unit.
/// https://drafts.css-houdini.org/css-typed-om-1/#numeric-typing
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum NumericType {
  Number,
  Percent,
  Length,
  Angle,
  Time,
  Frequency,
  Resolution,
  Flex,
}

// https://drafts.css-houdini.org/css-typed-om-1/#numeric-typing
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
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

  /// `None` for a compound dimension, such as the `1px / 1s` of
  /// `calc(1px / 1s * 1s)`, which no single numeric type expresses.
  fn numeric_type(self) -> Option<NumericType> {
    Some(match self {
      Self::NUMBER => NumericType::Number,
      Self::PERCENT => NumericType::Percent,
      Self::LENGTH => NumericType::Length,
      Self::ANGLE => NumericType::Angle,
      Self::TIME => NumericType::Time,
      Self::FREQUENCY => NumericType::Frequency,
      Self::RESOLUTION => NumericType::Resolution,
      Self::FLEX => NumericType::Flex,
      _ => return None,
    })
  }
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

// `<length-percentage>`: `%` gets a parse-time basis. Other mixed units stay unsupported.
// https://drafts.css-houdini.org/css-typed-om-1/#cssnumericvalue-percent-hint
#[derive(Debug)]
#[cfg_attr(test, derive(PartialEq))]
struct MathValue {
  /// The value in the canonical unit of `dimension`, which is what the engine
  /// computes with.
  value: f64,
  dimension: Dimension,
  /// The symbolic form, built only while the value reads the resolution context.
  /// A compound dimension keeps a tree too, having no literal to fall back on.
  calc: Option<CalcNode>,
}

impl From<NumericValue> for MathValue {
  fn from(value: NumericValue) -> Self {
    /// A context-independent literal needs no symbolic form: its canonical value
    /// is exact, and [`MathValue::into_node`] recovers a literal from it.
    #[inline]
    fn absolute(value: f64, dimension: Dimension) -> MathValue {
      MathValue {
        value,
        dimension,
        calc: None,
      }
    }
    match value {
      NumericValue::Zero => absolute(0.0, Dimension::NUMBER),
      NumericValue::Number(value) => absolute(value, Dimension::NUMBER),
      NumericValue::Percent(value) => absolute(value, Dimension::PERCENT),
      NumericValue::Length(length) => MathValue {
        value: length.folded_pixels(),
        dimension: Dimension::LENGTH,
        // Only reachable for an absolute unit: a relative one goes through
        // `relative_length`, which folds against the resolution.
        calc: None,
      },
      NumericValue::Angle(angle) => {
        absolute(angle.to_degrees(), Dimension::ANGLE)
      }
      NumericValue::Time(time) => absolute(time.to_seconds(), Dimension::TIME),
      NumericValue::Frequency(frequency) => {
        absolute(frequency.to_hertz(), Dimension::FREQUENCY)
      }
      NumericValue::Resolution(resolution) => {
        absolute(resolution.to_dot_per_pixels(), Dimension::RESOLUTION)
      }
      NumericValue::Flex(value) => absolute(value, Dimension::FLEX),
    }
  }
}

impl TryFrom<MathValue> for NumericValue {
  type Error = CSSCustomError;

  /// Dropping the symbolic form of a `<length>` is folding it to the pixel
  /// literal, which is exactly what [`MathValue::to_leaf`] builds.
  fn try_from(math: MathValue) -> Result<Self, Self::Error> {
    math.to_leaf().ok_or(CSSCustomError::InvalidDimension)
  }
}

impl MathValue {
  impl_math_value_is! {
    is_number: NUMBER,
    is_percent: PERCENT,
    is_length: LENGTH,
    is_angle: ANGLE,
    is_time: TIME,
    is_frequency: FREQUENCY,
  }

  /// The `<length-percentage>` arm is only reached where the context withheld a
  /// percentage basis, since a percentage that has one is already a length.
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

  /// Whether the value reads the resolution context, so an operation over it is
  /// worth a symbolic form.
  #[inline]
  fn is_relative_length(&self) -> bool {
    self.calc.as_ref().is_some_and(CalcNode::is_relative_length)
  }

  /// Operation result. Allocates a tree only if an operand is context-relative
  /// or the dimension is compound.
  /// https://www.w3.org/TR/css-values-4/#calc-simplification
  fn from_operation(
    value: f64,
    dimension: Dimension,
    keep: bool,
    build: impl FnOnce() -> CalcNode,
  ) -> Self {
    let mut math = Self {
      value,
      dimension,
      calc: None,
    };
    if keep || math.to_leaf().is_none() {
      math.calc = Some(build());
    }
    math
  }

  /// The operand as a node, standing a literal in for a value with no tree.
  fn into_node(self) -> CalcNode {
    match self.calc {
      Some(calc) => calc,
      // `from_operation` keeps the tree whenever there is no literal for it.
      None => match self.to_leaf() {
        Some(leaf) => CalcNode::Leaf(leaf),
        None => CalcNode::number(self.value),
      },
    }
  }

  /// The literal for this value in its canonical unit. `None` for a compound
  /// dimension, which no single numeric type expresses.
  fn to_leaf(&self) -> Option<NumericValue> {
    let value = self.value;
    Some(match self.dimension.numeric_type()? {
      NumericType::Number => NumericValue::Number(value),
      NumericType::Percent => NumericValue::Percent(value),
      NumericType::Length => NumericValue::Length(Length::from_pixels(value)),
      NumericType::Angle => NumericValue::Angle(Angle::from_degrees(value)),
      NumericType::Time => NumericValue::Time(Time::from_seconds(value)),
      NumericType::Frequency => {
        NumericValue::Frequency(Frequency::from_hertz(value))
      }
      NumericType::Resolution => {
        NumericValue::Resolution(Resolution::from_dot_per_pixels(value))
      }
      NumericType::Flex => NumericValue::Flex(value),
    })
  }

  /// The value `1` in the given canonical unit, which is the interval `round()`
  /// defaults to.
  #[inline]
  fn one(dimension: Dimension) -> Self {
    Self {
      value: 1.0,
      dimension,
      calc: None,
    }
  }

  #[inline]
  fn try_add_assign(self, other: MathValue) -> Result<Self, CSSCustomError> {
    self.try_sum(other, false)
  }

  #[inline]
  fn try_sub_assign(self, other: MathValue) -> Result<Self, CSSCustomError> {
    self.try_sum(other, true)
  }

  /// Combines the symbolic forms of a sum or difference into a `CSSMathSum`.
  fn try_sum(
    self,
    other: MathValue,
    subtract: bool,
  ) -> Result<Self, CSSCustomError> {
    if self.dimension != other.dimension {
      return Err(self.dimension_mismatch_error(&other));
    }
    let value = if subtract {
      self.value - other.value
    } else {
      self.value + other.value
    };
    let keep = self.is_relative_length() || other.is_relative_length();
    Ok(Self::from_operation(value, self.dimension, keep, || {
      let right = if subtract {
        other.into_node().negate()
      } else {
        other.into_node()
      };
      let mut terms = match self.into_node() {
        CalcNode::Sum(terms) => terms.into_vec(),
        left => vec![left],
      };
      terms.push(right);
      CalcNode::sum(terms)
    }))
  }

  /// `CSSMathProduct`
  fn multiplied_by(self, other: MathValue) -> Self {
    let value = self.value * other.value;
    let mut dimension = self.dimension;
    dimension += &other.dimension;
    let keep = self.is_relative_length() || other.is_relative_length();
    Self::from_operation(value, dimension, keep, || {
      CalcNode::product(vec![self.into_node(), other.into_node()])
    })
  }

  /// `CSSMathProduct` over a `CSSMathInvert`
  fn divided_by(self, other: MathValue) -> Self {
    let value = self.value / other.value;
    let mut dimension = self.dimension;
    dimension -= &other.dimension;
    let keep = self.is_relative_length() || other.is_relative_length();
    Self::from_operation(value, dimension, keep, || {
      CalcNode::product(vec![self.into_node(), other.into_node().invert()])
    })
  }
}

/// Relative-color channel keywords, resolved as `<number>` values.
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
  /// Font/percentage metrics. `None` rejects non-absolute `<length>` units.
  pub length_resolution: Option<LengthResolution>,
  /// Relative-color channel keywords. `None` rejects bare identifiers.
  pub channel_keywords: Option<ChannelKeywords>,
}

#[derive(Debug)]
struct ParseState {
  function_depth: u8,
  length_resolution: Option<LengthResolution>,
  channel_keywords: Option<ChannelKeywords>,
  /// Whether the caller keeps the specified form. Derived from the entry point
  /// rather than [`ParseOptions`], so it cannot contradict the return type.
  retain_specified: bool,
}

impl ParseState {
  fn new(opts: ParseOptions, retain_specified: bool) -> Self {
    Self {
      function_depth: 0,
      length_resolution: opts.length_resolution,
      channel_keywords: opts.channel_keywords,
      retain_specified,
    }
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
  /// Reads one operand, recursing into math functions.
  fn parse<'i, 't>(
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
        // Without a basis a percentage keeps its own numeric type, which is
        // what rejects it in a plain `<length>` position.
        let Some(resolution) = state
          .length_resolution
          .filter(|resolution| resolution.percentage_basis.is_some())
        else {
          return Ok(NumericValue::Percent(*unit_value as f64).into());
        };
        let length = Length {
          value: *unit_value as f64 * 100.0,
          unit: LengthUnit::Percent,
        };
        Ok(relative_length(length, &resolution, state))
      }
      Token::Dimension { value, unit, .. } => {
        let value = *value as f64;
        // Relative units are only accepted when a resolution context is
        // provided (font and spacing contexts).
        if let Some(unit) = LengthUnit::parse(unit) {
          let length = Length { value, unit };
          if unit.is_absolute() {
            return Ok(NumericValue::Length(length).into());
          }
          let Some(resolution) = state.length_resolution else {
            return Err(
              input
                .new_custom_error(CSSCustomError::ContainsRelativeLengthValues),
            );
          };
          return Ok(relative_length(length, &resolution, state));
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
              let (operands, dimension) =
                Self::parse_operand_list(arguments, state)?;
              let result = operands
                .iter()
                .map(|operand| operand.value)
                .fold(f64::INFINITY, minimum);
              let keep = operands.iter().any(MathValue::is_relative_length);
              Ok(MathValue::from_operation(result, dimension, keep, || {
                CalcNode::Min(collect_nodes(operands))
              })
              .into())
            })
          },
          "max" => {
            input.parse_nested_block(|arguments| {
              let (operands, dimension) =
                Self::parse_operand_list(arguments, state)?;
              let result = operands
                .iter()
                .map(|operand| operand.value)
                .fold(f64::NEG_INFINITY, maximum);
              let keep = operands.iter().any(MathValue::is_relative_length);
              Ok(MathValue::from_operation(result, dimension, keep, || {
                CalcNode::Max(collect_nodes(operands))
              })
              .into())
            })
          },
          "clamp" => {
            input.parse_nested_block(|arguments| {
              // A bound cannot be type-checked before the value is parsed.
              let min: Option<NumericAccumulator> = {
                let start = arguments.state();
                if arguments.expect_ident_matching("none").is_ok() {
                  None
                } else {
                  arguments.reset(&start);
                  Some(Self::parse_additive_expression(arguments, state)?)
                }
              };
              arguments.expect_comma()?;
              let acc = Self::parse_additive_expression(arguments, state)?;
              arguments.expect_comma()?;
              let max: Option<NumericAccumulator> = {
                let start = arguments.state();
                if arguments.expect_ident_matching("none").is_ok() {
                  None
                } else {
                  arguments.reset(&start);
                  Some(Self::parse_additive_expression(arguments, state)?)
                }
              };
              arguments.expect_exhausted()?;

              let value = try_extract!(acc, expect_operand(), arguments);
              let dimension = value.dimension;
              let min = match min {
                Some(acc) => {
                  Some(try_extract!(acc, expect_operand_of(dimension), arguments))
                }
                None => None,
              };
              let max = match max {
                Some(acc) => {
                  Some(try_extract!(acc, expect_operand_of(dimension), arguments))
                }
                None => None,
              };
              let low = min.as_ref().map_or(f64::NEG_INFINITY, |min| min.value);
              let high = max.as_ref().map_or(f64::INFINITY, |max| max.value);
              let result = maximum(low, minimum(value.value, high));
              let keep = value.is_relative_length()
                || min.iter().chain(&max).any(MathValue::is_relative_length);
              Ok(MathValue::from_operation(result, dimension, keep, || {
                CalcNode::Clamp {
                  min: min.map(|min| Box::new(min.into_node())),
                  value: Box::new(value.into_node()),
                  max: max.map(|max| Box::new(max.into_node())),
                }
              })
              .into())
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
              let value = try_extract!(acc, expect_operand(), arguments);
              let dimension = value.dimension;
              let interval = if !arguments.is_exhausted() {
                arguments.expect_comma()?;
                let acc = Self::parse_additive_expression(arguments, state)?;
                let interval =
                  try_extract!(acc, expect_operand_of(dimension), arguments);
                arguments.expect_exhausted()?;
                interval
              } else {
                // The default interval is `1` in the value's canonical unit.
                MathValue::one(dimension)
              };
              let result =
                round_to_interval(strategy, value.value, interval.value);
              let keep = value.is_relative_length() || interval.is_relative_length();
              Ok(MathValue::from_operation(result, dimension, keep, || {
                CalcNode::Round {
                  strategy,
                  value: Box::new(value.into_node()),
                  interval: Box::new(interval.into_node()),
                }
              })
              .into())
            })
          },
          "mod" => {
            input.parse_nested_block(|arguments| {
              let (dividend, divisor) =
                Self::parse_operand_pair(arguments, state)?;
              let result = dividend.value.rem_euclid(divisor.value);
              let dimension = dividend.dimension;
              let keep = dividend.is_relative_length() || divisor.is_relative_length();
              Ok(MathValue::from_operation(result, dimension, keep, || {
                CalcNode::Mod {
                  dividend: Box::new(dividend.into_node()),
                  divisor: Box::new(divisor.into_node()),
                }
              })
              .into())
            })
          },
          "rem" => {
            input.parse_nested_block(|arguments| {
              let (dividend, divisor) =
                Self::parse_operand_pair(arguments, state)?;
              let result = dividend.value % divisor.value;
              let dimension = dividend.dimension;
              let keep = dividend.is_relative_length() || divisor.is_relative_length();
              Ok(MathValue::from_operation(result, dimension, keep, || {
                CalcNode::Rem {
                  dividend: Box::new(dividend.into_node()),
                  divisor: Box::new(divisor.into_node()),
                }
              })
              .into())
            })
          },
          // https://www.w3.org/TR/css-values-4/#trig-funcs
          "sin" => Self::parse_trig(input, state, TrigFunction::Sin),
          "cos" => Self::parse_trig(input, state, TrigFunction::Cos),
          "tan" => Self::parse_trig(input, state, TrigFunction::Tan),
          "asin" => Self::parse_inverse_trig(input, state, InverseTrigFunction::Asin),
          "acos" => Self::parse_inverse_trig(input, state, InverseTrigFunction::Acos),
          "atan" => Self::parse_inverse_trig(input, state, InverseTrigFunction::Atan),
          "atan2" => {
            input.parse_nested_block(|arguments| {
              let (y, x) = Self::parse_operand_pair(arguments, state)?;
              let result = y.value.atan2(x.value).to_degrees();
              let keep = y.is_relative_length() || x.is_relative_length();
              Ok(
                MathValue::from_operation(result, Dimension::ANGLE, keep, || {
                  CalcNode::Atan2 {
                    y: Box::new(y.into_node()),
                    x: Box::new(x.into_node()),
                  }
                })
                .into(),
              )
            })
          },
          // https://www.w3.org/TR/css-values-4/#exponent-funcs
          "pow" => {
            input.parse_nested_block(|arguments| {
              let acc = Self::parse_additive_expression(arguments, state)?;
              let base = try_extract!(acc, expect_number_operand(), arguments);
              arguments.expect_comma()?;
              let acc = Self::parse_additive_expression(arguments, state)?;
              let exponent =
                try_extract!(acc, expect_number_operand(), arguments);
              arguments.expect_exhausted()?;
              let result = base.value.powf(exponent.value);
              let keep = base.is_relative_length() || exponent.is_relative_length();
              Ok(
                MathValue::from_operation(result, Dimension::NUMBER, keep, || {
                  CalcNode::Pow {
                    base: Box::new(base.into_node()),
                    exponent: Box::new(exponent.into_node()),
                  }
                })
                .into(),
              )
            })
          },
          "sqrt" => {
            input.parse_nested_block(|arguments| {
              let acc = Self::parse_additive_expression(arguments, state)?;
              let value = try_extract!(acc, expect_number_operand(), arguments);
              arguments.expect_exhausted()?;
              let result = value.value.sqrt();
              let keep = value.is_relative_length();
              Ok(
                MathValue::from_operation(result, Dimension::NUMBER, keep, || {
                  CalcNode::Sqrt(Box::new(value.into_node()))
                })
                .into(),
              )
            })
          },
          "hypot" => {
            input.parse_nested_block(|arguments| {
              let (operands, dimension) =
                Self::parse_operand_list(arguments, state)?;
              let args = operands
                .iter()
                .map(|operand| operand.value)
                .collect::<Vec<_>>();
              let result = hypot(&args);
              let keep = operands.iter().any(MathValue::is_relative_length);
              Ok(MathValue::from_operation(result, dimension, keep, || {
                CalcNode::Hypot(collect_nodes(operands))
              })
              .into())
            })
          },
          "log" => {
            input.parse_nested_block(|arguments| {
              let acc = Self::parse_additive_expression(arguments, state)?;
              let value = try_extract!(acc, expect_number_operand(), arguments);
              let base = if !arguments.is_exhausted() {
                arguments.expect_comma()?;
                let acc = Self::parse_additive_expression(arguments, state)?;
                let base =
                  try_extract!(acc, expect_number_operand(), arguments);
                arguments.expect_exhausted()?;
                Some(base)
              } else {
                None
              };
              let result = match &base {
                Some(base) => value.value.log(base.value),
                None => value.value.ln(),
              };
              let keep = value.is_relative_length()
                || base.as_ref().is_some_and(MathValue::is_relative_length);
              Ok(
                MathValue::from_operation(result, Dimension::NUMBER, keep, || {
                  CalcNode::Log {
                    value: Box::new(value.into_node()),
                    base: base.map(|base| Box::new(base.into_node())),
                  }
                })
                .into(),
              )
            })
          },
          "exp" => {
            input.parse_nested_block(|arguments| {
              let acc = Self::parse_additive_expression(arguments, state)?;
              let value = try_extract!(acc, expect_number_operand(), arguments);
              arguments.expect_exhausted()?;
              let result = value.value.exp();
              let keep = value.is_relative_length();
              Ok(
                MathValue::from_operation(result, Dimension::NUMBER, keep, || {
                  CalcNode::Exp(Box::new(value.into_node()))
                })
                .into(),
              )
            })
          },
          // https://www.w3.org/TR/css-values-4/#sign-funcs
          "abs" => {
            input.parse_nested_block(|arguments| {
              let acc = Self::parse_additive_expression(arguments, state)?;
              let value = try_extract!(acc, expect_operand(), arguments);
              arguments.expect_exhausted()?;
              // `CalcNode::abs` folds into the literal, keeping its unit.
              let dimension = value.dimension;
              let result = value.value.abs();
              let keep = value.is_relative_length();
              Ok(
                MathValue::from_operation(result, dimension, keep, || {
                  value.into_node().abs()
                })
                .into(),
              )
            })
          },
          "sign" => {
            input.parse_nested_block(|arguments| {
              let acc = Self::parse_additive_expression(arguments, state)?;
              let value = try_extract!(acc, expect_operand(), arguments);
              arguments.expect_exhausted()?;
              // Sign of the computed value. Differs only when a metric is 0.
              let result = sign(value.value);
              let keep = value.is_relative_length();
              Ok(
                MathValue::from_operation(result, Dimension::NUMBER, keep, || {
                  CalcNode::Sign(Box::new(value.into_node()))
                })
                .into(),
              )
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

  /// The operand of a math function, keeping its symbolic form so a dependency
  /// survives whatever the function does to the dimension.
  #[inline]
  fn expect_operand(self) -> Result<MathValue, CSSCustomError> {
    let math = self.into_math();
    if math.dimension.numeric_type().is_some() {
      Ok(math)
    } else {
      Err(CSSCustomError::InvalidDimension)
    }
  }

  /// Like [`Self::expect_operand`], but also requires the numeric type of an
  /// operand already parsed, which makes `min(1px, 1deg)` a type error.
  #[inline]
  fn expect_operand_of(
    self,
    dimension: Dimension,
  ) -> Result<MathValue, CSSCustomError> {
    let math = self.into_math();
    if math.dimension == dimension {
      Ok(math)
    } else {
      Err(CSSCustomError::UnexpectedNumericType)
    }
  }

  #[inline]
  fn expect_number_operand(self) -> Result<MathValue, CSSCustomError> {
    self.expect_operand_of(Dimension::NUMBER)
  }

  /// The value as specified, keeping the symbolic form so relative units resolve
  /// when it is used. This is [`Self::expect_numeric`]'s counterpart.
  fn into_specified(self) -> Result<SpecifiedNumericValue, CSSCustomError> {
    match self {
      // `<zero>` is a unit-less `<length>` or `<angle>`. Only the former is
      // ever retained, so it takes that reading rather than staying `0`.
      Self::Numeric(NumericValue::Zero) => Ok(SpecifiedNumericValue::zero()),
      // Any other top-level literal needs no folding, and never reaches `Math`,
      // which only exists inside a math function.
      Self::Numeric(value) => Ok(SpecifiedNumericValue::literal(value)),
      Self::Math(math) => {
        let numeric_type = math
          .dimension
          .numeric_type()
          .ok_or(CSSCustomError::InvalidDimension)?;
        Ok(SpecifiedNumericValue::new(numeric_type, math.into_node()))
      }
    }
  }
  fn parse_additive_expression<'i, 't>(
    input: &mut Parser<'i, 't>,
    state: &mut ParseState,
  ) -> Result<NumericAccumulator, CSSParseError<'i>> {
    let mut lhs = Self::parse_multiplicative_expression(input, state)?;

    while !input.is_exhausted() {
      let start = input.state();
      let token = input.next_including_whitespace()?;
      if let Token::WhiteSpace(_) = token {
        let token = input.next()?;
        let subtract = match token {
          Token::Delim('+') => false,
          Token::Delim('-') => true,
          _ => {
            input.reset(&start);
            break;
          }
        };
        input.expect_whitespace()?;
        let rhs = Self::parse_multiplicative_expression(input, state)?;
        let left = lhs.into_math();
        let right = rhs.into_math();
        let sum = if subtract {
          left.try_sub_assign(right)
        } else {
          left.try_add_assign(right)
        };
        match sum {
          Ok(sum) => lhs = sum.into(),
          Err(error) => return Err(input.new_custom_error(error)),
        }
      } else {
        input.reset(&start);
        break;
      }
    }

    Ok(lhs)
  }

  fn parse_multiplicative_expression<'i, 't>(
    input: &mut Parser<'i, 't>,
    state: &mut ParseState,
  ) -> Result<NumericAccumulator, CSSParseError<'i>> {
    let mut lhs = Self::parse(input, state)?;

    while !input.is_exhausted() {
      let start = input.state();
      let token = input.next()?;
      let divide = match token {
        Token::Delim('*') => false,
        Token::Delim('/') => true,
        _ => {
          input.reset(&start);
          break;
        }
      };
      let rhs = Self::parse(input, state)?.into_math();
      let left = lhs.into_math();
      lhs = if divide {
        left.divided_by(rhs).into()
      } else {
        left.multiplied_by(rhs).into()
      };
    }

    Ok(lhs)
  }

  /// A comma-separated list of operands of one numeric type, the shape `min()`,
  /// `max()` and `hypot()` share. Never empty.
  fn parse_operand_list<'i, 't>(
    input: &mut Parser<'i, 't>,
    state: &mut ParseState,
  ) -> Result<(Vec<MathValue>, Dimension), CSSParseError<'i>> {
    let acc = Self::parse_additive_expression(input, state)?;
    let first = try_extract!(acc, expect_operand(), input);
    let dimension = first.dimension;
    let mut operands = vec![first];
    while !input.is_exhausted() {
      input.expect_comma()?;
      let acc = Self::parse_additive_expression(input, state)?;
      operands.push(try_extract!(acc, expect_operand_of(dimension), input));
    }
    Ok((operands, dimension))
  }

  /// Two operands of the same numeric type, the shape `mod()`, `rem()` and
  /// `atan2()` share.
  fn parse_operand_pair<'i, 't>(
    input: &mut Parser<'i, 't>,
    state: &mut ParseState,
  ) -> Result<(MathValue, MathValue), CSSParseError<'i>> {
    let acc = Self::parse_additive_expression(input, state)?;
    let left = try_extract!(acc, expect_operand(), input);
    input.expect_comma()?;
    let acc = Self::parse_additive_expression(input, state)?;
    let right = try_extract!(acc, expect_operand_of(left.dimension), input);
    input.expect_exhausted()?;
    Ok((left, right))
  }

  /// https://www.w3.org/TR/css-values-4/#trig-funcs
  fn parse_trig<'i, 't>(
    input: &mut Parser<'i, 't>,
    state: &mut ParseState,
    function: TrigFunction,
  ) -> Result<NumericAccumulator, CSSParseError<'i>> {
    input.parse_nested_block(|arguments| {
      let acc = Self::parse_additive_expression(arguments, state)?;
      let value = try_extract!(acc, expect_operand(), arguments);
      arguments.expect_exhausted()?;
      let degrees = value.is_angle();
      if !degrees && !value.is_number() {
        return Err(
          arguments.new_custom_error(CSSCustomError::UnexpectedNumericType),
        );
      }
      let radians = if degrees {
        value.value.to_radians()
      } else {
        value.value
      };
      let keep = value.is_relative_length();
      Ok(
        MathValue::from_operation(
          function.apply(radians),
          Dimension::NUMBER,
          keep,
          || CalcNode::Trig {
            function,
            degrees,
            value: Box::new(value.into_node()),
          },
        )
        .into(),
      )
    })
  }

  /// https://www.w3.org/TR/css-values-4/#trig-funcs
  fn parse_inverse_trig<'i, 't>(
    input: &mut Parser<'i, 't>,
    state: &mut ParseState,
    function: InverseTrigFunction,
  ) -> Result<NumericAccumulator, CSSParseError<'i>> {
    input.parse_nested_block(|arguments| {
      let acc = Self::parse_additive_expression(arguments, state)?;
      let value = try_extract!(acc, expect_number_operand(), arguments);
      arguments.expect_exhausted()?;
      let result = function.apply(value.value);
      let keep = value.is_relative_length();
      Ok(
        MathValue::from_operation(result, Dimension::ANGLE, keep, || {
          CalcNode::InverseTrig {
            function,
            value: Box::new(value.into_node()),
          }
        })
        .into(),
      )
    })
  }
}

/// A context-dependent `<length>`. Top-level keeps its unit; in math it folds.
fn relative_length(
  length: Length,
  resolution: &LengthResolution,
  state: &ParseState,
) -> NumericAccumulator {
  if state.function_depth == 0 {
    return NumericValue::Length(length).into();
  }
  MathValue {
    value: length.resolve_to_pixels(resolution),
    dimension: Dimension::LENGTH,
    // A tree only ever starts here, so withholding the literal keeps every math
    // function from building one.
    calc: state
      .retain_specified
      .then_some(CalcNode::Leaf(NumericValue::Length(length))),
  }
  .into()
}

/// A numeric literal, which is also what a calculation tree's leaves are.
/// https://drafts.css-houdini.org/css-typed-om-1/#cssunitvalue
#[derive(Clone, Debug, PartialEq)]
pub enum NumericValue {
  /// Literal `0`, accepted as `<length>` or `<angle>` outside math.
  /// https://www.w3.org/TR/css-values-4/#zero-value
  Zero,
  Number(f64),
  /// The unit value, i.e. `0.5` for `50%`.
  Percent(f64),
  Length(Length),
  Angle(Angle),
  Time(Time),
  Frequency(Frequency),
  Resolution(Resolution),
  Flex(f64),
}

impl From<Length> for NumericValue {
  #[inline]
  fn from(value: Length) -> Self {
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
  /// Parses a numeric value, folding a math function against the metrics in
  /// `opts`. Nothing keeps the symbolic form, so no tree is built at all.
  pub fn parse<'i, 't>(
    input: &mut Parser<'i, 't>,
    opts: ParseOptions,
  ) -> Result<Self, CSSParseError<'i>> {
    let result =
      NumericAccumulator::parse(input, &mut ParseState::new(opts, false))?;
    match result.expect_numeric() {
      Ok(numeric) => Ok(numeric),
      Err(error) => Err(input.new_custom_error(error)),
    }
  }

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

  /// Extracts a `<length>` (math folds at parse time).
  #[inline]
  pub fn expect_length(
    self,
    allow_zero: bool,
  ) -> Result<Length, CSSCustomError> {
    match self {
      NumericValue::Zero => {
        if allow_zero {
          Ok(Length::zero())
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

  // A calculation tree's leaves are numeric literals, so these are the
  // operations `CalcNode` needs of one.

  /// The type this literal has. `<zero>` counts as a `<number>`; only
  /// `NumericAccumulator::into_specified` reads it as a `<length>`.
  fn numeric_type(&self) -> NumericType {
    match self {
      Self::Zero | Self::Number(_) => NumericType::Number,
      Self::Percent(_) => NumericType::Percent,
      Self::Length(_) => NumericType::Length,
      Self::Angle(_) => NumericType::Angle,
      Self::Time(_) => NumericType::Time,
      Self::Frequency(_) => NumericType::Frequency,
      Self::Resolution(_) => NumericType::Resolution,
      Self::Flex(_) => NumericType::Flex,
    }
  }

  /// Applies `f` to the number as written, keeping the unit, so `abs(-1pt)`
  /// stays `1pt`.
  fn map_value(&self, f: impl Fn(f64) -> f64) -> Self {
    match self {
      // `<zero>` is unit-less, and `f` need not keep it zero.
      Self::Zero => Self::Number(f(0.0)),
      Self::Number(value) => Self::Number(f(*value)),
      Self::Percent(value) => Self::Percent(f(*value)),
      Self::Length(length) => Self::Length(Length {
        value: f(length.value),
        unit: length.unit,
      }),
      Self::Angle(angle) => Self::Angle(Angle {
        value: f(angle.value),
        unit: angle.unit,
      }),
      Self::Time(time) => Self::Time(Time {
        value: f(time.value),
        unit: time.unit,
      }),
      Self::Frequency(frequency) => Self::Frequency(Frequency {
        value: f(frequency.value),
        unit: frequency.unit,
      }),
      Self::Resolution(resolution) => Self::Resolution(Resolution {
        value: f(resolution.value),
        unit: resolution.unit,
      }),
      Self::Flex(value) => Self::Flex(f(*value)),
    }
  }

  /// The number as written, which is what decides how a sum term serializes.
  #[inline]
  fn raw_value(&self) -> f64 {
    match self {
      Self::Zero => 0.0,
      Self::Number(value) | Self::Percent(value) | Self::Flex(value) => *value,
      Self::Length(length) => length.value,
      Self::Angle(angle) => angle.value,
      Self::Time(time) => time.value,
      Self::Frequency(frequency) => frequency.value,
      Self::Resolution(resolution) => resolution.value,
    }
  }

  #[inline]
  fn scaled(&self, factor: f64) -> Self {
    self.map_value(|value| value * factor)
  }

  #[inline]
  fn abs(&self) -> Self {
    self.map_value(f64::abs)
  }

  /// The value in the canonical unit of its dimension.
  fn resolve(&self, resolution: &LengthResolution) -> f64 {
    match self {
      Self::Zero => 0.0,
      Self::Number(value) | Self::Percent(value) | Self::Flex(value) => *value,
      Self::Length(length) => length.resolve_to_pixels(resolution),
      Self::Angle(angle) => angle.to_degrees(),
      Self::Time(time) => time.to_seconds(),
      Self::Frequency(frequency) => frequency.to_hertz(),
      Self::Resolution(resolution) => resolution.to_dot_per_pixels(),
    }
  }

  /// Sort key for a sum term. `""` puts `<number>` first.
  /// https://www.w3.org/TR/css-values-4/#sort-a-calculations-children
  fn sort_unit(&self) -> &'static str {
    match self {
      Self::Zero | Self::Number(_) => "",
      Self::Percent(_) => "%",
      Self::Length(length) => length.unit.to_css_str(),
      Self::Angle(angle) => angle.unit.to_css_str(),
      Self::Time(time) => time.unit.to_css_str(),
      Self::Frequency(frequency) => frequency.unit.to_css_str(),
      Self::Resolution(resolution) => resolution.unit.to_css_str(),
      Self::Flex(_) => "fr",
    }
  }

  fn to_css_string(&self) -> String {
    // Format as f32 to avoid cssparser f32→f64 widening noise.
    match self {
      Self::Zero => "0".to_string(),
      Self::Number(value) => format!("{}", *value as f32),
      // A percentage stores its unit value, so the literal is 100x that.
      Self::Percent(value) => format!("{}%", (*value * 100.0) as f32),
      Self::Length(length) => length.to_css_string(),
      Self::Angle(angle) => {
        format!("{}{}", angle.value as f32, angle.unit.to_css_str())
      }
      Self::Time(time) => {
        format!("{}{}", time.value as f32, time.unit.to_css_str())
      }
      Self::Frequency(frequency) => {
        format!("{}{}", frequency.value as f32, frequency.unit.to_css_str())
      }
      Self::Resolution(resolution) => {
        format!(
          "{}{}",
          resolution.value as f32,
          resolution.unit.to_css_str()
        )
      }
      Self::Flex(value) => format!("{}fr", *value as f32),
    }
  }
}

/// https://www.w3.org/TR/css-values-4/#trig-funcs
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TrigFunction {
  Sin,
  Cos,
  Tan,
}

impl TrigFunction {
  #[inline]
  fn apply(self, radians: f64) -> f64 {
    match self {
      Self::Sin => radians.sin(),
      Self::Cos => radians.cos(),
      Self::Tan => radians.tan(),
    }
  }

  fn to_css_str(self) -> &'static str {
    match self {
      Self::Sin => "sin",
      Self::Cos => "cos",
      Self::Tan => "tan",
    }
  }
}

/// https://www.w3.org/TR/css-values-4/#trig-funcs
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum InverseTrigFunction {
  Asin,
  Acos,
  Atan,
}

impl InverseTrigFunction {
  /// The result in `deg`, which is the canonical `<angle>` unit.
  #[inline]
  fn apply(self, value: f64) -> f64 {
    match self {
      Self::Asin => value.asin(),
      Self::Acos => value.acos(),
      Self::Atan => value.atan(),
    }
    .to_degrees()
  }

  fn to_css_str(self) -> &'static str {
    match self {
      Self::Asin => "asin",
      Self::Acos => "acos",
      Self::Atan => "atan",
    }
  }
}

/// Symbolic calculation. Resolves to the dimension's canonical unit.
/// https://drafts.css-houdini.org/css-typed-om-1/#numeric-objects
#[derive(Clone, Debug, PartialEq)]
enum CalcNode {
  /// `CSSUnitValue`
  Leaf(NumericValue),
  /// `CSSMathSum`
  Sum(Box<[CalcNode]>),
  /// `CSSMathProduct`
  Product(Box<[CalcNode]>),
  /// `CSSMathInvert`, the divisor of a division.
  Invert(Box<CalcNode>),
  /// `CSSMathNegate`, the right operand of a subtraction.
  Negate(Box<CalcNode>),
  /// `CSSMathMin`
  Min(Box<[CalcNode]>),
  /// `CSSMathMax`
  Max(Box<[CalcNode]>),
  /// `CSSMathClamp`
  Clamp {
    min: Option<Box<CalcNode>>,
    value: Box<CalcNode>,
    max: Option<Box<CalcNode>>,
  },
  /// https://www.w3.org/TR/css-values-4/#round-func
  Round {
    strategy: RoundStrategy,
    value: Box<CalcNode>,
    interval: Box<CalcNode>,
  },
  /// https://www.w3.org/TR/css-values-4/#funcdef-mod
  Mod {
    dividend: Box<CalcNode>,
    divisor: Box<CalcNode>,
  },
  /// https://www.w3.org/TR/css-values-4/#funcdef-rem
  Rem {
    dividend: Box<CalcNode>,
    divisor: Box<CalcNode>,
  },
  /// https://www.w3.org/TR/css-values-4/#funcdef-abs
  Abs(Box<CalcNode>),
  /// https://www.w3.org/TR/css-values-4/#funcdef-sign
  Sign(Box<CalcNode>),
  /// https://www.w3.org/TR/css-values-4/#funcdef-hypot
  Hypot(Box<[CalcNode]>),
  /// https://www.w3.org/TR/css-values-4/#funcdef-sqrt
  Sqrt(Box<CalcNode>),
  /// https://www.w3.org/TR/css-values-4/#funcdef-pow
  Pow {
    base: Box<CalcNode>,
    exponent: Box<CalcNode>,
  },
  /// https://www.w3.org/TR/css-values-4/#funcdef-log
  Log {
    value: Box<CalcNode>,
    base: Option<Box<CalcNode>>,
  },
  /// https://www.w3.org/TR/css-values-4/#funcdef-exp
  Exp(Box<CalcNode>),
  /// `degrees` marks an `<angle>` argument (canonical `deg` → radians).
  /// https://www.w3.org/TR/css-values-4/#trig-funcs
  Trig {
    function: TrigFunction,
    degrees: bool,
    value: Box<CalcNode>,
  },
  /// https://www.w3.org/TR/css-values-4/#trig-funcs
  InverseTrig {
    function: InverseTrigFunction,
    value: Box<CalcNode>,
  },
  /// https://www.w3.org/TR/css-values-4/#funcdef-atan2
  Atan2 { y: Box<CalcNode>, x: Box<CalcNode> },
}

impl CalcNode {
  #[inline]
  fn number(value: f64) -> Self {
    Self::Leaf(NumericValue::Number(value))
  }

  fn sum(terms: Vec<Self>) -> Self {
    match <[Self; 1]>::try_from(terms) {
      Ok([single]) => single,
      Err(terms) => Self::Sum(terms.into_boxed_slice()),
    }
  }

  /// Fold literal `<number>` factors (`1em * 2` → `2em`). Flatten products.
  fn product(terms: Vec<Self>) -> Self {
    let mut flat = Vec::with_capacity(terms.len());
    for term in terms {
      match term {
        Self::Product(children) => flat.extend(children),
        term => flat.push(term),
      }
    }
    let numbers = flat
      .iter()
      .filter(|term| matches!(term, Self::Leaf(NumericValue::Number(_))))
      .count();
    if numbers > 0 && flat.len() - numbers <= 1 {
      let mut factor = 1.0;
      let mut rest = None;
      for term in flat {
        match term {
          Self::Leaf(NumericValue::Number(value)) => factor *= value,
          term => rest = Some(term),
        }
      }
      return match rest {
        None => Self::number(factor),
        Some(term) if factor == 1.0 => term,
        Some(Self::Leaf(leaf)) => Self::Leaf(leaf.scaled(factor)),
        Some(term) => Self::Product(Box::from([Self::number(factor), term])),
      };
    }
    match <[Self; 1]>::try_from(flat) {
      Ok([single]) => single,
      Err(terms) => Self::Product(terms.into_boxed_slice()),
    }
  }

  /// The right operand of a division.
  fn invert(self) -> Self {
    match self {
      Self::Leaf(NumericValue::Number(value)) => Self::number(1.0 / value),
      Self::Invert(value) => *value,
      value => Self::Invert(Box::new(value)),
    }
  }

  /// The right operand of a subtraction.
  fn negate(self) -> Self {
    match self {
      Self::Leaf(leaf) => Self::Leaf(leaf.scaled(-1.0)),
      Self::Negate(value) => *value,
      value => Self::Negate(Box::new(value)),
    }
  }

  /// https://www.w3.org/TR/css-values-4/#funcdef-abs
  fn abs(self) -> Self {
    match self {
      // "Simplify a calculation tree" folds `abs()` of a numeric value.
      Self::Leaf(leaf) => Self::Leaf(leaf.abs()),
      value @ Self::Abs(_) => value,
      value => Self::Abs(Box::new(value)),
    }
  }

  /// True if a relative `<length>` contributed (e.g. `atan2(1em, 1px)`).
  fn is_relative_length(&self) -> bool {
    match self {
      // A relative `<length>` is the only literal a resolution context reaches.
      Self::Leaf(NumericValue::Length(length)) => !length.is_absolute(),
      Self::Leaf(_) => false,
      Self::Sum(terms)
      | Self::Product(terms)
      | Self::Min(terms)
      | Self::Max(terms)
      | Self::Hypot(terms) => terms.iter().any(Self::is_relative_length),
      Self::Invert(value)
      | Self::Negate(value)
      | Self::Abs(value)
      | Self::Sign(value)
      | Self::Sqrt(value)
      | Self::Exp(value)
      | Self::Trig { value, .. }
      | Self::InverseTrig { value, .. } => value.is_relative_length(),
      Self::Clamp { min, value, max } => {
        min.as_deref().is_some_and(Self::is_relative_length)
          || value.is_relative_length()
          || max.as_deref().is_some_and(Self::is_relative_length)
      }
      Self::Round {
        value, interval, ..
      } => value.is_relative_length() || interval.is_relative_length(),
      Self::Mod { dividend, divisor } | Self::Rem { dividend, divisor } => {
        dividend.is_relative_length() || divisor.is_relative_length()
      }
      Self::Pow { base, exponent } => {
        base.is_relative_length() || exponent.is_relative_length()
      }
      Self::Log { value, base } => {
        value.is_relative_length()
          || base.as_deref().is_some_and(Self::is_relative_length)
      }
      Self::Atan2 { y, x } => y.is_relative_length() || x.is_relative_length(),
    }
  }

  fn resolve(&self, resolution: &LengthResolution) -> f64 {
    let resolve = |node: &Self| node.resolve(resolution);
    match self {
      Self::Leaf(leaf) => leaf.resolve(resolution),
      Self::Sum(terms) => terms.iter().map(resolve).sum(),
      Self::Product(terms) => terms.iter().map(resolve).product(),
      Self::Invert(value) => 1.0 / resolve(value),
      Self::Negate(value) => -resolve(value),
      Self::Min(terms) => {
        terms.iter().map(resolve).fold(f64::INFINITY, minimum)
      }
      Self::Max(terms) => {
        terms.iter().map(resolve).fold(f64::NEG_INFINITY, maximum)
      }
      Self::Clamp { min, value, max } => {
        let low = min.as_deref().map_or(f64::NEG_INFINITY, resolve);
        let high = max.as_deref().map_or(f64::INFINITY, resolve);
        maximum(low, minimum(resolve(value), high))
      }
      Self::Round {
        strategy,
        value,
        interval,
      } => round_to_interval(*strategy, resolve(value), resolve(interval)),
      Self::Mod { dividend, divisor } => {
        resolve(dividend).rem_euclid(resolve(divisor))
      }
      Self::Rem { dividend, divisor } => resolve(dividend) % resolve(divisor),
      Self::Abs(value) => resolve(value).abs(),
      Self::Sign(value) => sign(resolve(value)),
      Self::Hypot(terms) => {
        hypot(&terms.iter().map(resolve).collect::<Vec<_>>())
      }
      Self::Sqrt(value) => resolve(value).sqrt(),
      Self::Pow { base, exponent } => resolve(base).powf(resolve(exponent)),
      Self::Log { value, base } => match base {
        Some(base) => resolve(value).log(resolve(base)),
        None => resolve(value).ln(),
      },
      Self::Exp(value) => resolve(value).exp(),
      Self::Trig {
        function,
        degrees,
        value,
      } => {
        let value = resolve(value);
        function.apply(if *degrees { value.to_radians() } else { value })
      }
      Self::InverseTrig { function, value } => function.apply(resolve(value)),
      Self::Atan2 { y, x } => resolve(y).atan2(resolve(x)).to_degrees(),
    }
  }

  /// Sort unit for a term. Non-literals keep authored order.
  /// https://www.w3.org/TR/css-values-4/#sort-a-calculations-children
  #[inline]
  fn sort_unit(&self) -> Option<&'static str> {
    match self {
      Self::Leaf(leaf) => Some(leaf.sort_unit()),
      _ => None,
    }
  }

  /// Serializes the node without the outer `calc()` wrapper.
  fn serialize(&self) -> String {
    match self {
      Self::Leaf(leaf) => leaf.to_css_string(),
      Self::Sum(terms) => {
        // Only a sum's children are sorted; a product keeps its authored order.
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
          // A negated term reads as a subtraction, as CSSOM serializes it.
          match term {
            Self::Leaf(leaf) if leaf.raw_value() < 0.0 && index > 0 => {
              out.push_str(" - ");
              out.push_str(&leaf.scaled(-1.0).to_css_string());
              continue;
            }
            Self::Negate(value) if index > 0 => {
              out.push_str(" - ");
              out.push_str(&value.serialize());
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
      Self::Product(terms) => {
        let mut out = String::new();
        for (index, term) in terms.iter().enumerate() {
          // A reciprocal factor reads as a division, as CSSOM serializes it.
          match term {
            Self::Invert(value) if index > 0 => {
              out.push_str(" / ");
              out.push_str(&value.serialize());
              continue;
            }
            _ => {}
          }
          if index > 0 {
            out.push_str(" * ");
          }
          out.push_str(&term.serialize());
        }
        out
      }
      Self::Invert(value) => format!("1 / {}", value.serialize()),
      Self::Negate(value) => format!("-1 * {}", value.serialize()),
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
      Self::Sign(value) => format!("sign({})", value.serialize()),
      Self::Hypot(terms) => format!("hypot({})", serialize_list(terms)),
      Self::Sqrt(value) => format!("sqrt({})", value.serialize()),
      Self::Pow { base, exponent } => {
        format!("pow({}, {})", base.serialize(), exponent.serialize())
      }
      Self::Log { value, base } => match base {
        Some(base) => {
          format!("log({}, {})", value.serialize(), base.serialize())
        }
        None => format!("log({})", value.serialize()),
      },
      Self::Exp(value) => format!("exp({})", value.serialize()),
      Self::Trig {
        function, value, ..
      } => {
        format!("{}({})", function.to_css_str(), value.serialize())
      }
      Self::InverseTrig { function, value } => {
        format!("{}({})", function.to_css_str(), value.serialize())
      }
      Self::Atan2 { y, x } => {
        format!("atan2({}, {})", y.serialize(), x.serialize())
      }
    }
  }

  /// CSSOM serialization (`calc()`, named functions, or a lone literal).
  /// https://www.w3.org/TR/css-values-4/#calc-serialize
  fn to_css_string(&self) -> String {
    match self {
      // A bare operator tree is only valid inside `calc()`.
      Self::Sum(_) | Self::Product(_) | Self::Invert(_) | Self::Negate(_) => {
        format!("calc({})", self.serialize())
      }
      _ => self.serialize(),
    }
  }
}

/// Specified numeric value; relative units resolve when used.
#[derive(Clone, Debug, PartialEq)]
pub struct SpecifiedNumericValue {
  /// The type the value parsed as. A `CalcNode` carries no dimension of its
  /// own, so this is what `expect_*` checks against.
  numeric_type: NumericType,
  tree: Rc<CalcNode>,
}

impl SpecifiedNumericValue {
  /// Parses a numeric value, keeping its symbolic form so relative units
  /// resolve when it is used. [`NumericValue::parse`]'s counterpart.
  pub fn parse<'i, 't>(
    input: &mut Parser<'i, 't>,
    opts: ParseOptions,
  ) -> Result<Self, CSSParseError<'i>> {
    let result =
      NumericAccumulator::parse(input, &mut ParseState::new(opts, true))?;
    match result.into_specified() {
      Ok(value) => Ok(value),
      Err(error) => Err(input.new_custom_error(error)),
    }
  }

  #[inline]
  fn new(numeric_type: NumericType, tree: CalcNode) -> Self {
    Self {
      numeric_type,
      tree: Rc::new(tree),
    }
  }

  #[inline]
  fn literal(value: NumericValue) -> Self {
    Self::new(value.numeric_type(), CalcNode::Leaf(value))
  }

  #[inline]
  pub(crate) fn zero() -> Self {
    Self::from_pixels(0.0)
  }

  #[inline]
  pub(crate) fn from_pixels(value: f64) -> Self {
    Self::literal(Length::from_pixels(value).into())
  }

  /// Whether this is a `<length>`, so that [`Self::resolve`] yields pixels.
  #[inline]
  pub fn is_length(&self) -> bool {
    self.numeric_type == NumericType::Length
  }

  /// Whether resolving reads the resolution context, i.e. whether the caller has
  /// to build the font metrics first.
  #[inline]
  pub fn is_relative_length(&self) -> bool {
    self.tree.is_relative_length()
  }

  /// The value in the canonical unit of its own type -- `px` for a `<length>`.
  pub fn resolve(&self, resolution: &LengthResolution) -> f64 {
    self.tree.resolve(resolution)
  }

  pub fn to_css_string(&self) -> String {
    self.tree.to_css_string()
  }
}

/// https://www.w3.org/TR/css-values-4/#round-func
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RoundStrategy {
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

/// https://www.w3.org/TR/css-values-4/#funcdef-sign
#[inline]
fn sign(value: f64) -> f64 {
  if value == 0.0 { value } else { value.signum() }
}

fn serialize_list(terms: &[CalcNode]) -> String {
  terms
    .iter()
    .map(CalcNode::serialize)
    .collect::<Vec<_>>()
    .join(", ")
}

fn collect_nodes(operands: Vec<MathValue>) -> Box<[CalcNode]> {
  operands
    .into_iter()
    .map(MathValue::into_node)
    .collect::<Vec<_>>()
    .into_boxed_slice()
}

#[cfg(test)]
mod tests {
  use approx::assert_relative_eq;
  use cssparser::BasicParseErrorKind;
  use cssparser::ParseErrorKind;

  use super::*;

  /// Viewport- and container-percentage units.
  /// https://www.w3.org/TR/css-values-4/#viewport-relative-lengths
  /// https://drafts.csswg.org/css-conditional-5/#container-lengths
  const BOX_RELATIVE_UNIT_NAMES: [&str; 30] = [
    "vw", "vh", "vi", "vb", "vmin", "vmax", "svw", "svh", "svi", "svb",
    "svmin", "svmax", "lvw", "lvh", "lvi", "lvb", "lvmin", "lvmax", "dvw",
    "dvh", "dvi", "dvb", "dvmin", "dvmax", "cqw", "cqh", "cqi", "cqb", "cqmin",
    "cqmax",
  ];

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
      Length {
        value: -1.0,
        unit: LengthUnit::Cm,
      }
    );
    assert_relative_eq!(length.to_pixels().unwrap(), -96.0 / 2.54);
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

    // Convert to the canonical unit (f32, matching cssparser).
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
  fn box_relative_unit_names_round_trip() {
    // Guards `parse` and `to_css_str` against drifting apart, and keeps every
    // one of these units resolvable only through the viewport size.
    let resolution = LengthResolution::new(FontMetrics::fallback(10.0));
    for name in BOX_RELATIVE_UNIT_NAMES {
      let unit = LengthUnit::parse(name).unwrap();
      assert_eq!(unit.to_css_str(), name);
      assert_eq!(LengthUnit::parse(&name.to_ascii_uppercase()), Some(unit));
      assert!(!unit.is_absolute(), "{name}");
      assert_eq!(unit.px_factor(), None, "{name}");
      assert!(unit.percentage_basis(&resolution).is_some(), "{name}");
    }
  }

  #[test]
  fn box_relative_units_read_the_resolution() {
    // In-tree callers use a zero viewport; only hand-built resolutions differ.
    // https://www.w3.org/TR/css-values-4/#viewport-relative-lengths
    // https://drafts.csswg.org/css-conditional-5/#container-lengths
    fn px(css: &str, resolution: &LengthResolution) -> f64 {
      let mut input = ParserInput::new(css);
      let mut parser = Parser::new(&mut input);
      NumericValue::parse(
        &mut parser,
        ParseOptions {
          length_resolution: Some(*resolution),
          ..Default::default()
        },
      )
      .unwrap()
      .expect_length(false)
      .unwrap()
      .resolve_to_pixels(resolution)
    }

    let resolution = LengthResolution {
      viewport: BoxSize {
        width: 800.0,
        height: 600.0,
      },
      ..LengthResolution::new(FontMetrics::fallback(10.0))
    };

    // Every unit measures the axis its name asks for. The small, large and
    // dynamic viewports coincide, and container units share the same size.
    for (unit, expected) in [
      ("w", 80.0),
      ("h", 60.0),
      // No writing mode, so the inline axis is horizontal.
      ("i", 80.0),
      ("b", 60.0),
      ("min", 60.0),
      ("max", 80.0),
    ] {
      for prefix in ["v", "sv", "lv", "dv", "cq"] {
        let css = format!("10{prefix}{unit}");
        assert_relative_eq!(px(&css, &resolution), expected);
      }
    }

    // Inside a math function the value is folded against the same resolution.
    assert_relative_eq!(px("calc(10vw + 2px)", &resolution), 82.0);
    // Font-relative units are unaffected.
    assert_relative_eq!(px("2em", &resolution), 20.0);
  }

  #[test]
  fn box_relative_units_default_to_zero() {
    let resolution = LengthResolution::new(FontMetrics::fallback(10.0));
    assert_eq!(resolution.viewport, BoxSize::default());
    for unit in BOX_RELATIVE_UNIT_NAMES {
      let css = format!("10{unit}");
      let mut input = ParserInput::new(&css);
      let mut parser = Parser::new(&mut input);
      let length = NumericValue::parse(
        &mut parser,
        ParseOptions {
          length_resolution: Some(resolution),
          ..Default::default()
        },
      )
      .unwrap()
      .expect_length(false)
      .unwrap();
      assert_eq!(length.to_css_string(), css, "serializing {css}");
      assert_eq!(
        length.resolve_to_pixels(&resolution),
        0.0,
        "resolving {css}"
      );
    }
  }

  /// A `<length-percentage>` context: `%` is 1% of 40px, `em` is 10px.
  fn length_percentage_resolution() -> LengthResolution {
    LengthResolution {
      percentage_basis: Some(40.0),
      ..LengthResolution::new(FontMetrics::fallback(10.0))
    }
  }

  fn parse_with(
    css: &str,
    resolution: &LengthResolution,
  ) -> Result<NumericValue, ()> {
    let mut input = ParserInput::new(css);
    let mut parser = Parser::new(&mut input);
    NumericValue::parse(
      &mut parser,
      ParseOptions {
        length_resolution: Some(*resolution),
        ..Default::default()
      },
    )
    .map_err(|_| ())
  }

  /// Like [`parse_with`], but keeping the symbolic form.
  fn parse_specified_with(
    css: &str,
    resolution: &LengthResolution,
  ) -> Result<SpecifiedNumericValue, ()> {
    let mut input = ParserInput::new(css);
    let mut parser = Parser::new(&mut input);
    SpecifiedNumericValue::parse(
      &mut parser,
      ParseOptions {
        length_resolution: Some(*resolution),
        ..Default::default()
      },
    )
    .map_err(|_| ())
    .and_then(|value| value.is_length().then_some(value).ok_or(()))
  }

  #[test]
  fn specified_is_length() {
    // `SpecifiedNumericValue::parse` accepts any numeric type, so `is_length`
    // is the dimension check, mirroring `NumericValue::expect_length`.
    let resolution = LengthResolution::new(FontMetrics::fallback(10.0));
    let parse = |css: &str| {
      let mut input = ParserInput::new(css);
      let mut parser = Parser::new(&mut input);
      SpecifiedNumericValue::parse(
        &mut parser,
        ParseOptions {
          length_resolution: Some(resolution),
          ..Default::default()
        },
      )
      .unwrap()
    };

    assert!(parse("1em").is_length());
    for css in ["1", "50%", "45deg", "1s", "1hz", "1dppx", "1fr"] {
      assert!(!parse(css).is_length(), "{css}");
    }

    // `<zero>` takes the `<length>` reading, so it counts and serializes as one.
    let zero = parse("0");
    assert!(zero.is_length());
    assert_eq!(zero.to_css_string(), "0px");
    assert_eq!(zero.resolve(&resolution), 0.0);
  }

  #[test]
  fn absolute_expressions_build_no_tree() {
    // An expression over absolute units is exact in pixels, so no tree is
    // built. Serializing as a single literal is the observable half of that.
    let resolution = LengthResolution::new(FontMetrics::fallback(10.0));
    for (css, expected) in [
      ("calc(1px + 2 * 3px)", "7px"),
      ("min(1px, 2px)", "1px"),
      ("clamp(1px, 2px, 3px)", "2px"),
      ("round(-1.5px)", "-1px"),
      ("hypot(3px, 4px)", "5px"),
      ("abs(-3px)", "3px"),
      ("calc(sqrt(4px / 1px) * 1px)", "2px"),
    ] {
      let length = parse_specified_with(css, &resolution).expect(css);
      assert_eq!(length.to_css_string(), expected, "serializing {css}");
      assert!(!length.is_relative_length(), "{css}");
    }

    // A relative unit anywhere is what makes the tree necessary.
    for css in ["calc(1em + 2px)", "calc(sqrt(1em / 1px) * 1px)"] {
      let length = parse_specified_with(css, &resolution).expect(css);
      assert_eq!(length.to_css_string(), css, "serializing {css}");
      assert!(length.is_relative_length(), "{css}");
    }
  }

  #[test]
  fn plain_parse_builds_no_tree() {
    // `NumericValue::parse` builds no tree, even for relative units.
    let resolution = length_percentage_resolution();
    for (css, expected) in [
      ("calc(1em + 2px)", 12.0),
      ("min(1em, 15px)", 10.0),
      ("clamp(5px, 1em, 15px)", 10.0),
      ("calc(sqrt(1em / 1px) * 1px)", 10.0_f64.sqrt()),
      ("calc(atan2(1em, 1px) / 1deg * 1px)", 84.28940686250036),
      ("abs(-1em)", 10.0),
      ("hypot(3em, 4em)", 50.0),
      ("calc(1em + 50%)", 30.0),
    ] {
      let Ok(NumericValue::Length(length)) = parse_with(css, &resolution)
      else {
        panic!("expect length: {css}");
      };
      // Already folded, so the result needs no metrics at all.
      assert!(length.is_absolute(), "{css}");
      assert_relative_eq!(length.to_pixels().unwrap(), expected);
    }
  }

  #[test]
  fn percentage_is_a_length_only_with_a_basis() {
    // https://www.w3.org/TR/css-values-4/#mixed-percentages
    let resolution = length_percentage_resolution();
    let Ok(NumericValue::Length(length)) = parse_with("50%", &resolution)
    else {
      panic!("expect length");
    };
    assert_eq!(length.to_css_string(), "50%");
    assert_relative_eq!(length.resolve_to_pixels(&resolution), 20.0);

    // Without a basis the same token stays a percentage, so it never satisfies
    // a `<length>`.
    let plain = LengthResolution::new(FontMetrics::fallback(10.0));
    assert_eq!(plain.percentage_basis, None);
    assert_eq!(parse_with("50%", &plain), Ok(NumericValue::Percent(0.5)));
    assert!(parse_with("calc(1em + 50%)", &plain).is_err());
  }

  #[test]
  fn percentage_mixes_with_lengths() {
    let resolution = length_percentage_resolution();
    for (css, expected) in [
      ("calc(1em + 50%)", 30.0),
      ("calc(50% - 1em)", 10.0),
      ("min(1em, 50%)", 10.0),
      ("max(50%, 1em)", 20.0),
      ("clamp(1em, 50%, 15px)", 15.0),
      ("calc(50% * 2)", 40.0),
      // Dividing out the percentage leaves a plain scalar factor.
      ("calc(100% / 50% * 1em)", 20.0),
    ] {
      let Ok(NumericValue::Length(length)) = parse_with(css, &resolution)
      else {
        panic!("expect length: {css}");
      };
      assert_relative_eq!(length.resolve_to_pixels(&resolution), expected);
    }

    // A percentage carries a length exponent, so these are type errors.
    for css in ["calc(1px * 50%)", "calc(50% + 1)", "calc(50% + 1deg)"] {
      assert!(parse_with(css, &resolution).is_err(), "{css}");
    }
  }

  #[test]
  fn percentage_keeps_its_symbolic_form() {
    // `%` sorts ahead of every dimension (ASCII order).
    // https://www.w3.org/TR/css-values-4/#sort-a-calculations-children
    let resolution = length_percentage_resolution();
    for (css, expected) in [
      ("calc(1em + 50%)", "calc(50% + 1em)"),
      ("calc(2px + 50%)", "calc(50% + 2px)"),
      ("min(1em, 50%)", "min(1em, 50%)"),
    ] {
      let length = parse_specified_with(css, &resolution).expect(css);
      assert_eq!(length.to_css_string(), expected);
    }
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
      Ok(NumericValue::Length(Length {
        value: 7.0,
        unit: LengthUnit::Px,
      }))
    );
  }

  #[test]
  fn calc_parenthesis() {
    let mut input = ParserInput::new("calc((1px + 2px) * 3)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser, ParseOptions::default());
    assert_eq!(
      result,
      Ok(NumericValue::Length(Length {
        value: 9.0,
        unit: LengthUnit::Px,
      }))
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
      Ok(NumericValue::Length(Length {
        value: 1.0,
        unit: LengthUnit::Px,
      }))
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
      Ok(NumericValue::Length(Length {
        value: -2.0,
        unit: LengthUnit::Px,
      }))
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
      Ok(NumericValue::Length(Length {
        value: 3.0,
        unit: LengthUnit::Px,
      }))
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
      Ok(NumericValue::Length(Length {
        value: -1.0,
        unit: LengthUnit::Px,
      }))
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
      Ok(NumericValue::Length(Length {
        value: -1.0,
        unit: LengthUnit::Px,
      }))
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
      Ok(NumericValue::Length(Length {
        value: 1.0,
        unit: LengthUnit::Px,
      }))
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
      Ok(NumericValue::Length(Length {
        value: 1.0,
        unit: LengthUnit::Px,
      }))
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
    assert_relative_eq!(length.to_pixels().unwrap(), 13.0);
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
      Ok(NumericValue::Length(Length {
        value: 3.0,
        unit: LengthUnit::Px,
      }))
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
