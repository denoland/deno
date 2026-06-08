// Copyright 2018-2026 the Deno authors. MIT license.

use std::f64;
use std::ops;

use cssparser::ParseError;
use cssparser::Parser;
pub use cssparser::ParserInput;
use cssparser::Token;
use cssparser::match_ignore_ascii_case;

use crate::f64::maximum;
use crate::f64::minimum;

pub type CSSValueError<'i> = ParseError<'i, CSSValueCustomError>;

#[derive(Debug, thiserror::Error)]
#[cfg_attr(test, derive(PartialEq))]
pub enum CSSValueCustomError {
  #[error("unexpected numeric type")]
  UnexpectedNumericType,
  #[error(
    "contains relative <length> values that cannot be resolved at parse time"
  )]
  ContainsRelativeLengthValues,
  #[error("contains {0} calculations that cannot be resolved at parse time")]
  ContainPercentAndDimensionCalculations(&'static str),
  #[error("cannot add or subtract different numeric types")]
  NumericTypeMismatch,
  #[error("the dimension of the calculation result is incorrect")]
  InvalidDimension,
  #[error("contains invalid function: {0}")]
  InvalidFunction(String),
}

#[derive(Clone, Debug)]
#[cfg_attr(test, derive(PartialEq))]
pub struct Length {
  value: f64,
  unit: LengthUnit,
}

// Currently, only Absolute Length Units are supported
#[derive(Clone, Copy, Debug)]
#[cfg_attr(test, derive(Eq, PartialEq))]
enum LengthUnit {
  Cm,
  Mm,
  Q,
  In,
  Pc,
  Pt,
  Px,
}

impl Length {
  const INCH_TO_PX: f64 = 96.0;
  const INCH_TO_CM: f64 = 2.54;

  #[inline]
  fn from_pixels(value: f64) -> Self {
    Self {
      value,
      unit: LengthUnit::Px,
    }
  }

  #[inline]
  pub fn to_pixels(&self) -> f64 {
    let value = self.value;
    match self.unit {
      LengthUnit::Cm => value * (Self::INCH_TO_PX / Self::INCH_TO_CM),
      LengthUnit::Mm => value * (Self::INCH_TO_PX / Self::INCH_TO_CM / 10.0),
      LengthUnit::Q => value * (Self::INCH_TO_PX / Self::INCH_TO_CM / 40.0),
      LengthUnit::In => value * Self::INCH_TO_PX,
      LengthUnit::Pc => value * (Self::INCH_TO_PX / 6.0),
      LengthUnit::Pt => value * (Self::INCH_TO_PX / 72.0),
      LengthUnit::Px => value,
    }
  }
}

#[derive(Clone, Debug)]
#[cfg_attr(test, derive(PartialEq))]
pub struct Angle {
  value: f64,
  unit: AngleUnit,
}

#[derive(Clone, Copy, Debug)]
#[cfg_attr(test, derive(Eq, PartialEq))]
enum AngleUnit {
  Deg,
  Grad,
  Rad,
  Turn,
}

impl Angle {
  const TURN_TO_DEG: f64 = 360.0;
  const TURN_TO_GRAD: f64 = 400.0;

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
    let value = self.value;
    match self.unit {
      AngleUnit::Deg => value,
      AngleUnit::Grad => value * (Self::TURN_TO_DEG / Self::TURN_TO_GRAD),
      AngleUnit::Rad => value.to_degrees(),
      AngleUnit::Turn => value * Self::TURN_TO_DEG,
    }
  }

  #[inline]
  pub fn to_radians(&self) -> f64 {
    let value = self.value;
    match self.unit {
      AngleUnit::Deg => value.to_radians(),
      AngleUnit::Grad => {
        (value * (Self::TURN_TO_DEG / Self::TURN_TO_GRAD)).to_radians()
      }
      AngleUnit::Rad => value,
      AngleUnit::Turn => value * f64::consts::TAU,
    }
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
    let value = self.value;
    match self.unit {
      TimeUnit::S => value,
      TimeUnit::Ms => value * 1000.0,
    }
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
    let value = self.value;
    match self.unit {
      FrequencyUnit::Hz => value,
      FrequencyUnit::Khz => value * 1000.0,
    }
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

impl Resolution {
  const INCH_TO_PX: f64 = 96.0;
  const INCH_TO_CM: f64 = 2.54;

  #[inline]
  fn from_dot_per_pixels(value: f64) -> Self {
    Self {
      value,
      unit: ResolutionUnit::Dppx,
    }
  }

  #[inline]
  fn to_dot_per_pixels(&self) -> f64 {
    let value = self.value;
    match self.unit {
      ResolutionUnit::Dpi => value / Self::INCH_TO_PX,
      ResolutionUnit::Dpcm => value / (Self::INCH_TO_PX / Self::INCH_TO_CM),
      ResolutionUnit::Dppx => value,
    }
  }
}

#[derive(Debug)]
#[cfg_attr(test, derive(PartialEq))]
pub enum NumericValue {
  Zero,
  Number(f64),
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
  #[inline]
  pub fn expect_number(self) -> Result<f64, CSSValueCustomError> {
    match self {
      NumericValue::Zero => Ok(0.0),
      NumericValue::Number(number) => Ok(number),
      _ => Err(CSSValueCustomError::UnexpectedNumericType),
    }
  }

  #[inline]
  pub fn expect_percent(self) -> Result<f64, CSSValueCustomError> {
    match self {
      NumericValue::Percent(percent) => Ok(percent),
      _ => Err(CSSValueCustomError::UnexpectedNumericType),
    }
  }

  #[inline]
  pub fn expect_number_or_percent(self) -> Result<f64, CSSValueCustomError> {
    match self {
      NumericValue::Zero => Ok(0.0),
      NumericValue::Number(number) => Ok(number),
      NumericValue::Percent(percent) => Ok(percent),
      _ => Err(CSSValueCustomError::UnexpectedNumericType),
    }
  }

  #[inline]
  pub fn expect_length(
    self,
    allow_zero: bool,
  ) -> Result<Length, CSSValueCustomError> {
    match self {
      NumericValue::Zero => {
        if allow_zero {
          Ok(Length::from_pixels(0.0))
        } else {
          Err(CSSValueCustomError::UnexpectedNumericType)
        }
      }
      NumericValue::Length(length) => Ok(length),
      _ => Err(CSSValueCustomError::UnexpectedNumericType),
    }
  }

  #[inline]
  pub fn expect_angle(
    self,
    allow_zero: bool,
  ) -> Result<Angle, CSSValueCustomError> {
    match self {
      NumericValue::Zero => {
        if allow_zero {
          Ok(Angle::from_degrees(0.0))
        } else {
          Err(CSSValueCustomError::UnexpectedNumericType)
        }
      }
      NumericValue::Angle(angle) => Ok(angle),
      _ => Err(CSSValueCustomError::UnexpectedNumericType),
    }
  }

  #[inline]
  pub fn expect_time(self) -> Result<Time, CSSValueCustomError> {
    match self {
      NumericValue::Time(time) => Ok(time),
      _ => Err(CSSValueCustomError::UnexpectedNumericType),
    }
  }

  #[inline]
  pub fn expect_frequency(self) -> Result<Frequency, CSSValueCustomError> {
    match self {
      NumericValue::Frequency(frequency) => Ok(frequency),
      _ => Err(CSSValueCustomError::UnexpectedNumericType),
    }
  }

  #[inline]
  pub fn expect_resolution(self) -> Result<Resolution, CSSValueCustomError> {
    match self {
      NumericValue::Resolution(resolution) => Ok(resolution),
      _ => Err(CSSValueCustomError::UnexpectedNumericType),
    }
  }

  #[inline]
  pub fn expect_flex(self) -> Result<f64, CSSValueCustomError> {
    match self {
      NumericValue::Flex(flex) => Ok(flex),
      _ => Err(CSSValueCustomError::UnexpectedNumericType),
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
}

impl From<NumericValue> for MathValue {
  fn from(value: NumericValue) -> Self {
    match value {
      NumericValue::Zero => MathValue {
        value: 0.0,
        dimension: Dimension::NUMBER,
      },
      NumericValue::Number(value) => MathValue {
        value,
        dimension: Dimension::NUMBER,
      },
      NumericValue::Percent(value) => MathValue {
        value,
        dimension: Dimension::PERCENT,
      },
      NumericValue::Length(length) => {
        let value = length.to_pixels();
        MathValue {
          value,
          dimension: Dimension::LENGTH,
        }
      }
      NumericValue::Angle(angle) => {
        let value = angle.to_degrees();
        MathValue {
          value,
          dimension: Dimension::ANGLE,
        }
      }
      NumericValue::Time(time) => {
        let value = time.to_seconds();
        MathValue {
          value,
          dimension: Dimension::TIME,
        }
      }
      NumericValue::Frequency(frequency) => {
        let value = frequency.to_hertz();
        MathValue {
          value,
          dimension: Dimension::FREQUENCY,
        }
      }
      NumericValue::Resolution(resolution) => {
        let value = resolution.to_dot_per_pixels();
        MathValue {
          value,
          dimension: Dimension::RESOLUTION,
        }
      }
      NumericValue::Flex(value) => MathValue {
        value,
        dimension: Dimension::FLEX,
      },
    }
  }
}

impl TryFrom<MathValue> for NumericValue {
  type Error = CSSValueCustomError;

  fn try_from(math: MathValue) -> Result<Self, Self::Error> {
    let value = math.value;
    if math.is_number() {
      Ok(NumericValue::Number(value))
    } else if math.is_percent() {
      Ok(NumericValue::Percent(value))
    } else if math.is_length() {
      Ok(Length::from_pixels(value).into())
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
      Err(CSSValueCustomError::InvalidDimension)
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

  fn dimension_mismatch_error(&self, other: &MathValue) -> CSSValueCustomError {
    if self.is_percent() || other.is_percent() {
      if self.is_length() || other.is_length() {
        return CSSValueCustomError::ContainPercentAndDimensionCalculations(
          "<length-percentage>",
        );
      } else if self.is_angle() || other.is_angle() {
        return CSSValueCustomError::ContainPercentAndDimensionCalculations(
          "<angle-percentage>",
        );
      } else if self.is_time() || other.is_time() {
        return CSSValueCustomError::ContainPercentAndDimensionCalculations(
          "<time-percentage>",
        );
      } else if self.is_frequency() || other.is_frequency() {
        return CSSValueCustomError::ContainPercentAndDimensionCalculations(
          "<frequency-percentage>",
        );
      }
    }
    CSSValueCustomError::NumericTypeMismatch
  }

  #[inline]
  fn try_add_assign(
    &mut self,
    other: &MathValue,
  ) -> Result<(), CSSValueCustomError> {
    if self.dimension != other.dimension {
      return Err(self.dimension_mismatch_error(other));
    }
    self.value += other.value;
    Ok(())
  }

  #[inline]
  fn try_sub_assign(
    &mut self,
    other: &MathValue,
  ) -> Result<(), CSSValueCustomError> {
    if self.dimension != other.dimension {
      return Err(self.dimension_mismatch_error(other));
    }
    self.value -= other.value;
    Ok(())
  }

  #[inline]
  fn expect_number(self) -> Result<f64, CSSValueCustomError> {
    if !self.is_number() {
      return Err(CSSValueCustomError::UnexpectedNumericType);
    }
    Ok(self.value)
  }

  #[inline]
  fn expect_percent(self) -> Result<f64, CSSValueCustomError> {
    if !self.is_percent() {
      return Err(CSSValueCustomError::UnexpectedNumericType);
    }
    Ok(self.value)
  }

  #[inline]
  fn expect_length(self) -> Result<Length, CSSValueCustomError> {
    if !self.is_length() {
      return Err(CSSValueCustomError::UnexpectedNumericType);
    }
    Ok(Length::from_pixels(self.value))
  }

  #[inline]
  fn expect_angle(self) -> Result<Angle, CSSValueCustomError> {
    if !self.is_angle() {
      return Err(CSSValueCustomError::UnexpectedNumericType);
    }
    Ok(Angle::from_degrees(self.value))
  }

  #[inline]
  fn expect_time(self) -> Result<Time, CSSValueCustomError> {
    if !self.is_time() {
      return Err(CSSValueCustomError::UnexpectedNumericType);
    }
    Ok(Time::from_seconds(self.value))
  }

  #[inline]
  fn expect_frequency(self) -> Result<Frequency, CSSValueCustomError> {
    if !self.is_frequency() {
      return Err(CSSValueCustomError::UnexpectedNumericType);
    }
    Ok(Frequency::from_hertz(self.value))
  }

  #[inline]
  fn expect_resolution(self) -> Result<Resolution, CSSValueCustomError> {
    if !self.is_resolution() {
      return Err(CSSValueCustomError::UnexpectedNumericType);
    }
    Ok(Resolution::from_dot_per_pixels(self.value))
  }

  #[inline]
  fn expect_flex(self) -> Result<f64, CSSValueCustomError> {
    if !self.is_flex() {
      return Err(CSSValueCustomError::UnexpectedNumericType);
    }
    Ok(self.value)
  }
}

impl ops::MulAssign<&MathValue> for MathValue {
  #[inline]
  fn mul_assign(&mut self, rhs: &Self) {
    self.value *= rhs.value;
    self.dimension += &rhs.dimension;
  }
}

impl ops::DivAssign<&MathValue> for MathValue {
  #[inline]
  fn div_assign(&mut self, rhs: &Self) {
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
  fn expect_numeric(self) -> Result<NumericValue, CSSValueCustomError> {
    match self {
      NumericAccumulator::Numeric(numeric) => Ok(numeric),
      NumericAccumulator::Math(math) => math.try_into(),
    }
  }

  #[inline]
  fn expect_number(self) -> Result<f64, CSSValueCustomError> {
    match self {
      NumericAccumulator::Numeric(numeric) => numeric.expect_number(),
      NumericAccumulator::Math(math) => math.expect_number(),
    }
  }

  #[inline]
  fn expect_percent(self) -> Result<f64, CSSValueCustomError> {
    match self {
      NumericAccumulator::Numeric(numeric) => numeric.expect_percent(),
      NumericAccumulator::Math(math) => math.expect_percent(),
    }
  }

  #[inline]
  fn expect_length(
    self,
    allow_zero: bool,
  ) -> Result<Length, CSSValueCustomError> {
    match self {
      NumericAccumulator::Numeric(numeric) => numeric.expect_length(allow_zero),
      NumericAccumulator::Math(math) => math.expect_length(),
    }
  }

  #[inline]
  fn expect_angle(
    self,
    allow_zero: bool,
  ) -> Result<Angle, CSSValueCustomError> {
    match self {
      NumericAccumulator::Numeric(numeric) => numeric.expect_angle(allow_zero),
      NumericAccumulator::Math(math) => math.expect_angle(),
    }
  }

  #[inline]
  fn expect_time(self) -> Result<Time, CSSValueCustomError> {
    match self {
      NumericAccumulator::Numeric(numeric) => numeric.expect_time(),
      NumericAccumulator::Math(math) => math.expect_time(),
    }
  }

  #[inline]
  fn expect_frequency(self) -> Result<Frequency, CSSValueCustomError> {
    match self {
      NumericAccumulator::Numeric(numeric) => numeric.expect_frequency(),
      NumericAccumulator::Math(math) => math.expect_frequency(),
    }
  }

  #[inline]
  fn expect_resolution(self) -> Result<Resolution, CSSValueCustomError> {
    match self {
      NumericAccumulator::Numeric(numeric) => numeric.expect_resolution(),
      NumericAccumulator::Math(math) => math.expect_resolution(),
    }
  }

  #[inline]
  fn expect_flex(self) -> Result<f64, CSSValueCustomError> {
    match self {
      NumericAccumulator::Numeric(numeric) => numeric.expect_flex(),
      NumericAccumulator::Math(math) => math.expect_flex(),
    }
  }
}

#[derive(Debug)]
struct ParseState {
  function_depth: u8,
}

impl ParseState {
  fn new() -> Self {
    Self { function_depth: 0 }
  }
}

macro_rules! try_extract {
  ($expr:expr, $method:ident($($arg:expr),*), $input:expr) => {
    match $expr.$method($($arg),*) {
      Ok(v) => v,
      Err(e) => return Err($input.new_custom_error(e)),
    }
  };
  ($expr:expr, $method:ident($($arg:expr),*), $map:ident(), $input:expr) => {
    match $expr.$method($($arg),*) {
      Ok(v) => v.$map(),
      Err(e) => return Err($input.new_custom_error(e)),
    }
  };
}

macro_rules! extract_as_raw {
  ($expr:expr) => {
    match &$expr {
      NumericValue::Zero => unreachable!(),
      NumericValue::Number(number) => *number,
      NumericValue::Percent(percent) => *percent,
      NumericValue::Length(length) => length.to_pixels(),
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
        try_extract!($expr, expect_length(false), to_pixels(), $input)
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

macro_rules! from_raw {
  ($value:expr, $type_ref:expr) => {
    match &$type_ref {
      NumericValue::Zero => unreachable!(),
      NumericValue::Number(_) => NumericValue::Number($value),
      NumericValue::Percent(_) => NumericValue::Percent($value),
      NumericValue::Length(_) => {
        NumericValue::Length(Length::from_pixels($value))
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
  ) -> Result<Self, ParseError<'i, CSSValueCustomError>> {
    let result = Self::parse_inner(input, &mut ParseState::new())?;
    match result.expect_numeric() {
      Ok(numeric) => Ok(numeric),
      Err(error) => Err(input.new_custom_error(error)),
    }
  }

  fn parse_inner<'i, 't>(
    input: &mut Parser<'i, 't>,
    state: &mut ParseState,
  ) -> Result<NumericAccumulator, ParseError<'i, CSSValueCustomError>> {
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
        match_ignore_ascii_case! { &unit,
          // https://www.w3.org/TR/css-values-4/#absolute-lengths
          "cm" => Ok(NumericValue::Length(Length { value, unit: LengthUnit::Cm }).into()),
          "mm" => Ok(NumericValue::Length(Length { value, unit: LengthUnit::Mm }).into()),
          "q" => Ok(NumericValue::Length(Length { value, unit: LengthUnit::Q }).into()),
          "in" => Ok(NumericValue::Length(Length { value, unit: LengthUnit::In }).into()),
          "pc" => Ok(NumericValue::Length(Length { value, unit: LengthUnit::Pc }).into()),
          "pt" => Ok(NumericValue::Length(Length { value, unit: LengthUnit::Pt }).into()),
          "px" => Ok(NumericValue::Length(Length { value, unit: LengthUnit::Px }).into()),
          // https://www.w3.org/TR/css-values-4/#relative-lengths
          "em" | "rem" | "ex" | "rex" | "cap" | "rcap" | "ch" | "rch" | "ic" | "ric" | "lh" | "rlh" |
          "vw" | "svw" | "lvw" | "dvw" | "vh" | "svh" | "lvh" | "dvh" | "vi" | "svi" | "lvi" | "dvi" |
          "vb" | "svb" | "lvb" | "dvb" | "vmin" | "svmin" | "lvmin" | "dvmin" | "vmax" | "svmax" | "lvmax" | "dvmax" |
          // https://www.w3.org/TR/css-contain-3/#container-lengths
          "cqw" | "cqh" | "cqi" | "cqb" | "cqmin" | "cqmax"
          => Err(input.new_custom_error(CSSValueCustomError::ContainsRelativeLengthValues)),
          // https://www.w3.org/TR/css-values-4/#angles
          "deg" => Ok(NumericValue::Angle(Angle { value, unit: AngleUnit::Deg }).into()),
          "grad" => Ok(NumericValue::Angle(Angle { value, unit: AngleUnit::Grad }).into()),
          "rad" => Ok(NumericValue::Angle(Angle { value, unit: AngleUnit::Rad }).into()),
          "turn" => Ok(NumericValue::Angle(Angle { value, unit: AngleUnit::Turn }).into()),
          // https://www.w3.org/TR/css-values-4/#time
          "s" => Ok(NumericValue::Time(Time { value, unit: TimeUnit::S }).into()),
          "ms" => Ok(NumericValue::Time(Time { value, unit: TimeUnit::Ms }).into()),
          // https://www.w3.org/TR/css-values-4/#frequency
          "hz" => Ok(NumericValue::Frequency(Frequency { value, unit: FrequencyUnit::Hz }).into()),
          "khz" => Ok(NumericValue::Frequency(Frequency { value, unit: FrequencyUnit::Khz }).into()),
          // https://www.w3.org/TR/css-values-4/#resolution
          "dpi" => Ok(NumericValue::Resolution(Resolution { value, unit: ResolutionUnit::Dpi }).into()),
          "dpcm" => Ok(NumericValue::Resolution(Resolution { value, unit: ResolutionUnit::Dpcm }).into()),
          "dppx" | "x" => Ok(NumericValue::Resolution(Resolution { value, unit: ResolutionUnit::Dppx }).into()),
          // https://www.w3.org/TR/css-grid-2/#fr-unit
          "fr" => Ok(NumericValue::Flex(value).into()),
          _ => {
            let token = token.clone();
            Err(input.new_unexpected_token_error(token))
          }
        }
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
              while !arguments.is_exhausted() {
                arguments.expect_comma()?;
                let acc = Self::parse_additive_expression(arguments, state)?;
                let value = try_extract_as_raw!(acc, numeric, arguments);
                current = minimum(current, value);
              }
              Ok(from_raw!(current, numeric).into())
            })
          },
          "max" => {
            input.parse_nested_block(|arguments| {
              let acc = Self::parse_additive_expression(arguments, state)?;
              let numeric = try_extract!(acc, expect_numeric(), arguments);
              let mut current = extract_as_raw!(numeric);
              while !arguments.is_exhausted() {
                arguments.expect_comma()?;
                let acc = Self::parse_additive_expression(arguments, state)?;
                let value = try_extract_as_raw!(acc, numeric, arguments);
                current = maximum(current, value);
              }
              Ok(from_raw!(current, numeric).into())
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

              let min = match min {
                Some(value) => try_extract_as_raw!(value, numeric, arguments),
                None => f64::NEG_INFINITY,
              };
              let max = match max {
                Some(value) => try_extract_as_raw!(value, numeric, arguments),
                None => f64::INFINITY,
              };
              let value = extract_as_raw!(numeric);
              let result = maximum(min, minimum(value, max));
              Ok(from_raw!(result, numeric).into())
            })
          },
          // https://www.w3.org/TR/css-values-4/#round-func
          "round" => {
            enum RoundStrategy {
              Nearest,
              Up,
              Down,
              ToZero,
            }
            fn round(strategy: &RoundStrategy, value: f64, interval: f64) -> f64 {
              if interval == 0.0 || value.is_nan() || interval.is_nan() || value.is_infinite() && interval.is_infinite() {
                return f64::NAN;
              }
              if value.is_infinite() {
                return value;
              }
              if interval.is_infinite() {
                return match strategy {
                  RoundStrategy::Up => if value > 0.0 { f64::INFINITY } else if value == 0.0 && value.is_sign_positive() { 0.0 } else { -0.0 },
                  RoundStrategy::Down => if value < 0.0 { f64::NEG_INFINITY } else if value == 0.0 && value.is_sign_negative() { -0.0 } else { 0.0 },
                  RoundStrategy::Nearest | RoundStrategy::ToZero => if value.is_sign_positive() { 0.0 } else { -0.0 },
                }
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
              let interval = if !arguments.is_exhausted() {
                arguments.expect_comma()?;
                let acc = Self::parse_additive_expression(arguments, state)?;
                let interval = try_extract_as_raw!(acc, numeric, arguments);
                arguments.expect_exhausted()?;
                interval
              } else { 1.0 };
              let value = extract_as_raw!(numeric);
              let result = round(&strategy, value, interval);
              Ok(from_raw!(result, numeric).into())
            })
          },
          "mod" => {
            input.parse_nested_block(|arguments| {
              let acc = Self::parse_additive_expression(arguments, state)?;
              let numeric = try_extract!(acc, expect_numeric(), arguments);
              let dividend = extract_as_raw!(numeric);
              arguments.expect_comma()?;
              let acc = Self::parse_additive_expression(arguments, state)?;
              let divisor = try_extract_as_raw!(acc, numeric, arguments);
              arguments.expect_exhausted()?;
              let result = dividend.rem_euclid(divisor);
              Ok(from_raw!(result, numeric).into())
            })
          },
          "rem" => {
            input.parse_nested_block(|arguments| {
              let acc = Self::parse_additive_expression(arguments, state)?;
              let numeric = try_extract!(acc, expect_numeric(), arguments);
              let dividend = extract_as_raw!(numeric);
              arguments.expect_comma()?;
              let acc = Self::parse_additive_expression(arguments, state)?;
              let divisor = try_extract_as_raw!(acc, numeric, arguments);
              arguments.expect_exhausted()?;
              let result = dividend % divisor;
              Ok(from_raw!(result, numeric).into())
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
                _ => return Err(arguments.new_custom_error(CSSValueCustomError::UnexpectedNumericType)),
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
                _ => return Err(arguments.new_custom_error(CSSValueCustomError::UnexpectedNumericType)),
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
                _ => return Err(arguments.new_custom_error(CSSValueCustomError::UnexpectedNumericType)),
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

            input.parse_nested_block(|arguments| {
              let acc = Self::parse_additive_expression(arguments, state)?;
              let numeric = try_extract!(acc, expect_numeric(), arguments);
              let value = extract_as_raw!(numeric);
              let mut args = vec![value];
              while !arguments.is_exhausted() {
                arguments.expect_comma()?;
                let acc = Self::parse_additive_expression(arguments, state)?;
                let value = try_extract_as_raw!(acc, numeric, arguments);
                args.push(value);
              }
              let result = hypot(&args);
              Ok(from_raw!(result, numeric).into())
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
                  NumericValue::Length(Length {
                    value: length.value.abs(),
                    unit: length.unit,
                  }).into()
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
                NumericValue::Length(length) => length.value,
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
            return Err(input.new_custom_error(CSSValueCustomError::InvalidFunction(name)))
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
  ) -> Result<NumericAccumulator, ParseError<'i, CSSValueCustomError>> {
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
            let right = rhs.into_math();
            if let Err(error) = left.try_add_assign(&right) {
              return Err(input.new_custom_error(error));
            }
            lhs = left.into();
          }
          Token::Delim('-') => {
            input.expect_whitespace()?;
            let rhs = Self::parse_multiplicative_expression(input, state)?;
            let mut left = lhs.into_math();
            let right = rhs.into_math();
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

    Ok(lhs)
  }

  fn parse_multiplicative_expression<'i, 't>(
    input: &mut Parser<'i, 't>,
    state: &mut ParseState,
  ) -> Result<NumericAccumulator, ParseError<'i, CSSValueCustomError>> {
    let mut lhs = Self::parse_inner(input, state)?;

    while !input.is_exhausted() {
      let start = input.state();
      let token = input.next()?;
      match token {
        Token::Delim('*') => {
          let rhs = Self::parse_inner(input, state)?;
          let mut left = lhs.into_math();
          let right = rhs.into_math();
          left *= &right;
          lhs = left.into();
        }
        Token::Delim('/') => {
          let rhs = Self::parse_inner(input, state)?;
          let mut left = lhs.into_math();
          let right = rhs.into_math();
          left /= &right;
          lhs = left.into();
        }
        _ => {
          input.reset(&start);
          break;
        }
      }
    }

    Ok(lhs)
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
    let result = NumericValue::parse(&mut parser);
    assert_eq!(result, Ok(NumericValue::Zero));
  }

  #[test]
  fn number() {
    let mut input = ParserInput::new("42");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser);
    assert_eq!(result, Ok(NumericValue::Number(42.0)));
  }

  #[test]
  fn percent() {
    let mut input = ParserInput::new("50%");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser);
    assert_eq!(result, Ok(NumericValue::Percent(0.5)));
  }

  #[test]
  fn length() {
    let mut input = ParserInput::new("-1cm");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser);
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
    assert_relative_eq!(length.to_pixels(), -96.0 / 2.54);
  }

  #[test]
  fn angle() {
    let mut input = ParserInput::new("180deg");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser);
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
    let result = NumericValue::parse(&mut parser);
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
    let result = NumericValue::parse(&mut parser);
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
    let result = NumericValue::parse(&mut parser);
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
  fn flex() {
    let mut input = ParserInput::new("1fr");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser);
    assert_eq!(result, Ok(NumericValue::Flex(1.0)));
  }

  #[test]
  fn calc_zero() {
    let mut input = ParserInput::new("calc(0)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser);
    assert_eq!(result, Ok(NumericValue::Number(0.0)));
  }

  #[test]
  fn calc_const_e() {
    let mut input = ParserInput::new("calc(e)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser);
    assert_eq!(result, Ok(NumericValue::Number(f64::consts::E)));
  }

  #[test]
  fn calc_const_pi() {
    let mut input = ParserInput::new("calc(pi)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser);
    assert_eq!(result, Ok(NumericValue::Number(f64::consts::PI)));
  }

  #[test]
  fn calc_const_infinity() {
    let mut input = ParserInput::new("calc(infinity)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser);
    assert_eq!(result, Ok(NumericValue::Number(f64::INFINITY)));
  }

  #[test]
  fn calc_const_neg_infinity() {
    let mut input = ParserInput::new("calc(-infinity)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser);
    assert_eq!(result, Ok(NumericValue::Number(f64::NEG_INFINITY)));
  }

  #[test]
  fn calc_const_nan() {
    let mut input = ParserInput::new("calc(nan)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser);
    let Ok(NumericValue::Number(value)) = result else {
      panic!("expect number: {:?}", result);
    };
    assert!(value.is_nan());
  }

  #[test]
  fn calc() {
    let mut input = ParserInput::new("calc(1px + 2 * 3px)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser);
    assert_eq!(
      result,
      Ok(NumericValue::Length(Length {
        value: 7.0,
        unit: LengthUnit::Px
      }))
    );
  }

  #[test]
  fn calc_parenthesis() {
    let mut input = ParserInput::new("calc((1px + 2px) * 3)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser);
    assert_eq!(
      result,
      Ok(NumericValue::Length(Length {
        value: 9.0,
        unit: LengthUnit::Px
      }))
    );
  }

  #[test]
  fn calc_failed_by_whitespace() {
    let mut input = ParserInput::new("calc(1+2)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser);
    assert!(result.is_err_and(|error| matches!(
      error.kind,
      ParseErrorKind::Basic(BasicParseErrorKind::UnexpectedToken(_))
    )));
  }

  #[test]
  fn calc_failed_by_type_mismatch() {
    let mut input = ParserInput::new("calc(1px + 2deg)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser);
    assert!(result.is_err_and(|error| error.kind
      == ParseErrorKind::Custom(CSSValueCustomError::NumericTypeMismatch)));
  }

  #[test]
  fn calc_dimension() {
    let mut input = ParserInput::new("calc(1px * 1deg * 1% / 1deg / 1%)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser);
    assert_eq!(
      result,
      Ok(NumericValue::Length(Length {
        value: 1.0,
        unit: LengthUnit::Px
      }))
    );
  }

  #[test]
  fn calc_zero_dimension() {
    let mut input = ParserInput::new("calc(2px / 1px)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser);
    assert_eq!(result, Ok(NumericValue::Number(2.0)));
  }

  #[test]
  fn calc_failed_by_dimension() {
    let mut input = ParserInput::new("calc(1px * 1deg * 1% / 1deg)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser);
    assert!(result.is_err_and(|error| error.kind
      == ParseErrorKind::Custom(CSSValueCustomError::InvalidDimension)));
  }

  #[test]
  fn min() {
    let mut input = ParserInput::new("min(-1, 1 - 3, 3)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser);
    assert_eq!(result, Ok(NumericValue::Number(-2.0)));
  }

  #[test]
  fn min_nan() {
    let mut input = ParserInput::new("min(-1, nan, 3)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser);
    let Ok(NumericValue::Number(value)) = result else {
      panic!("expect number: {:?}", result);
    };
    assert!(value.is_nan());
  }

  #[test]
  fn min_length() {
    let mut input = ParserInput::new("min(-1px, 1px - 3px, 3px)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser);
    assert_eq!(
      result,
      Ok(NumericValue::Length(Length {
        value: -2.0,
        unit: LengthUnit::Px
      }))
    );
  }

  #[test]
  fn max() {
    let mut input = ParserInput::new("max(-1, 1 - 3, 3)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser);
    assert_eq!(result, Ok(NumericValue::Number(3.0)));
  }

  #[test]
  fn max_nan() {
    let mut input = ParserInput::new("max(-1, nan, 3)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser);
    let Ok(NumericValue::Number(value)) = result else {
      panic!("expect number: {:?}", result);
    };
    assert!(value.is_nan());
  }

  #[test]
  fn max_length() {
    let mut input = ParserInput::new("max(-1px, 1px - 3px, 3px)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser);
    assert_eq!(
      result,
      Ok(NumericValue::Length(Length {
        value: 3.0,
        unit: LengthUnit::Px
      }))
    );
  }

  #[test]
  fn clamp() {
    let mut input = ParserInput::new("clamp(-1, 1 - 3, 3)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser);
    assert_eq!(result, Ok(NumericValue::Number(-1.0)));
  }

  #[test]
  fn clamp_none() {
    let mut input = ParserInput::new("clamp(none, 1 - 3, 3)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser);
    assert_eq!(result, Ok(NumericValue::Number(-2.0)));
  }

  #[test]
  fn clamp_nan() {
    let mut input = ParserInput::new("clamp(-1, nan, 3)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser);
    let Ok(NumericValue::Number(value)) = result else {
      panic!("expect number: {:?}", result);
    };
    assert!(value.is_nan());
  }

  #[test]
  fn clamp_length() {
    let mut input = ParserInput::new("clamp(-1px, 1px - 3px, 3px)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser);
    assert_eq!(
      result,
      Ok(NumericValue::Length(Length {
        value: -1.0,
        unit: LengthUnit::Px
      }))
    );
  }

  #[test]
  fn round() {
    let mut input = ParserInput::new("round(1.5)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser);
    assert_eq!(result, Ok(NumericValue::Number(2.0)));
  }

  #[test]
  fn round_with_interval() {
    let mut input = ParserInput::new("round(1, 2)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser);
    assert_eq!(result, Ok(NumericValue::Number(2.0)));
  }

  #[test]
  fn round_with_strategy() {
    let mut input = ParserInput::new("round(to-zero, 2.5, 5)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser);
    assert_eq!(result, Ok(NumericValue::Number(0.0)));
  }

  #[test]
  fn round_with_interval_infinity() {
    let mut input = ParserInput::new("round(down, 1, infinity)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser);
    assert_eq!(result, Ok(NumericValue::Number(0.0)));
  }

  #[test]
  fn round_nan() {
    let mut input = ParserInput::new("round(up, nan, 3)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser);
    let Ok(NumericValue::Number(value)) = result else {
      panic!("expect number: {:?}", result);
    };
    assert!(value.is_nan());
  }

  #[test]
  fn round_length() {
    let mut input = ParserInput::new("round(-1.5px)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser);
    assert_eq!(
      result,
      Ok(NumericValue::Length(Length {
        value: -1.0,
        unit: LengthUnit::Px
      }))
    );
  }

  #[test]
  fn modulo() {
    let mut input = ParserInput::new("mod(-3, 2)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser);
    assert_eq!(result, Ok(NumericValue::Number(1.0)));
  }

  #[test]
  fn modulo_zero() {
    let mut input = ParserInput::new("mod(2, 0)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser);
    let Ok(NumericValue::Number(value)) = result else {
      panic!("expect number: {:?}", result);
    };
    assert!(value.is_nan());
  }

  #[test]
  fn modulo_length() {
    let mut input = ParserInput::new("mod(3px, 2px)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser);
    assert_eq!(
      result,
      Ok(NumericValue::Length(Length {
        value: 1.0,
        unit: LengthUnit::Px
      }))
    );
  }

  #[test]
  fn rem() {
    let mut input = ParserInput::new("rem(-3, 2)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser);
    assert_eq!(result, Ok(NumericValue::Number(-1.0)));
  }

  #[test]
  fn rem_zero() {
    let mut input = ParserInput::new("rem(2, 0)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser);
    let Ok(NumericValue::Number(value)) = result else {
      panic!("expect number: {:?}", result);
    };
    assert!(value.is_nan());
  }

  #[test]
  fn rem_length() {
    let mut input = ParserInput::new("mod(3px, 2px)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser);
    assert_eq!(
      result,
      Ok(NumericValue::Length(Length {
        value: 1.0,
        unit: LengthUnit::Px
      }))
    );
  }

  #[test]
  fn sin() {
    let mut input = ParserInput::new("sin(pi / 2)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser);
    let Ok(NumericValue::Number(value)) = result else {
      panic!("expect number: {:?}", result);
    };
    assert_relative_eq!(value, 1.0);
  }

  #[test]
  fn sin_angle() {
    let mut input = ParserInput::new("sin(90deg)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser);
    let Ok(NumericValue::Number(value)) = result else {
      panic!("expect number: {:?}", result);
    };
    assert_relative_eq!(value, 1.0);
  }

  #[test]
  fn cos() {
    let mut input = ParserInput::new("cos(pi)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser);
    let Ok(NumericValue::Number(value)) = result else {
      panic!("expect number: {:?}", result);
    };
    assert_relative_eq!(value, -1.0);
  }

  #[test]
  fn cos_angle() {
    let mut input = ParserInput::new("cos(180deg)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser);
    let Ok(NumericValue::Number(value)) = result else {
      panic!("expect number: {:?}", result);
    };
    assert_relative_eq!(value, -1.0);
  }

  #[test]
  fn tan() {
    let mut input = ParserInput::new("tan(pi / 4)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser);
    let Ok(NumericValue::Number(value)) = result else {
      panic!("expect number: {:?}", result);
    };
    assert_relative_eq!(value, 1.0);
  }

  #[test]
  fn tan_angle() {
    let mut input = ParserInput::new("tan(45deg)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser);
    let Ok(NumericValue::Number(value)) = result else {
      panic!("expect number: {:?}", result);
    };
    assert_relative_eq!(value, 1.0);
  }

  #[test]
  fn asin() {
    let mut input = ParserInput::new("asin(-1)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser);
    let Ok(NumericValue::Angle(angle)) = result else {
      panic!("expect angle: {:?}", result);
    };
    assert_relative_eq!(angle.to_degrees(), -90.0);
  }

  #[test]
  fn acos() {
    let mut input = ParserInput::new("acos(-1)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser);
    let Ok(NumericValue::Angle(angle)) = result else {
      panic!("expect angle: {:?}", result);
    };
    assert_relative_eq!(angle.to_degrees(), 180.0);
  }

  #[test]
  fn atan() {
    let mut input = ParserInput::new("atan(-1)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser);
    let Ok(NumericValue::Angle(angle)) = result else {
      panic!("expect angle: {:?}", result);
    };
    assert_relative_eq!(angle.to_degrees(), -45.0);
  }

  #[test]
  fn atan2() {
    let mut input = ParserInput::new("atan2(1, -1)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser);
    let Ok(NumericValue::Angle(angle)) = result else {
      panic!("expect angle: {:?}", result);
    };
    assert_relative_eq!(angle.to_degrees(), 135.0);
  }

  #[test]
  fn atan2_length() {
    let mut input = ParserInput::new("atan2(1px, -1px)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser);
    let Ok(NumericValue::Angle(angle)) = result else {
      panic!("expect angle: {:?}", result);
    };
    assert_relative_eq!(angle.to_degrees(), 135.0);
  }

  #[test]
  fn pow() {
    let mut input = ParserInput::new("pow(2, 3)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser);
    let Ok(NumericValue::Number(value)) = result else {
      panic!("expect number: {:?}", result);
    };
    assert_relative_eq!(value, 8.0);
  }

  #[test]
  fn sqrt() {
    let mut input = ParserInput::new("sqrt(4)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser);
    let Ok(NumericValue::Number(value)) = result else {
      panic!("expect number: {:?}", result);
    };
    assert_relative_eq!(value, 2.0);
  }

  #[test]
  fn hypot() {
    let mut input = ParserInput::new("hypot(3, 4, 12)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser);
    let Ok(NumericValue::Number(value)) = result else {
      panic!("expect number: {:?}", result);
    };
    assert_relative_eq!(value, 13.0);
  }

  #[test]
  fn hypot_length() {
    let mut input = ParserInput::new("hypot(3px, 4px, 12px)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser);
    let Ok(NumericValue::Length(length)) = result else {
      panic!("expect length: {:?}", result);
    };
    assert_relative_eq!(length.to_pixels(), 13.0);
  }

  #[test]
  fn log() {
    let mut input = ParserInput::new("log(10)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser);
    let Ok(NumericValue::Number(value)) = result else {
      panic!("expect number: {:?}", result);
    };
    assert_relative_eq!(value, 10.0_f64.ln());
  }

  #[test]
  fn log_multi_args() {
    let mut input = ParserInput::new("log(8, 2)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser);
    let Ok(NumericValue::Number(value)) = result else {
      panic!("expect number: {:?}", result);
    };
    assert_relative_eq!(value, 3.0);
  }

  #[test]
  fn exp() {
    let mut input = ParserInput::new("exp(2)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser);
    let Ok(NumericValue::Number(value)) = result else {
      panic!("expect number: {:?}", result);
    };
    assert_relative_eq!(value, 2.0_f64.exp());
  }

  #[test]
  fn abs() {
    let mut input = ParserInput::new("abs(-3)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser);
    assert_eq!(result, Ok(NumericValue::Number(3.0)));
  }

  #[test]
  fn abs_length() {
    let mut input = ParserInput::new("abs(-3px)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser);
    assert_eq!(
      result,
      Ok(NumericValue::Length(Length {
        value: 3.0,
        unit: LengthUnit::Px
      }))
    );
  }

  #[test]
  fn sign() {
    let mut input = ParserInput::new("sign(-2)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser);
    assert_eq!(result, Ok(NumericValue::Number(-1.0)));
  }

  #[test]
  fn sign_zero() {
    let mut input = ParserInput::new("sign(0)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser);
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
    let result = NumericValue::parse(&mut parser);
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
    let result = NumericValue::parse(&mut parser);
    assert_eq!(result, Ok(NumericValue::Number(-1.0)));
  }
}

// Currently, combined units such as <length-percentage> are not supported
// https://www.w3.org/TR/css-transforms-1/#transform-functions
// https://drafts.csswg.org/css-transforms-2/#transform-functions
#[derive(Debug)]
#[cfg_attr(test, derive(PartialEq))]
pub enum Transform {
  Translate(Length, Option<Length>),
  TranslateX(Length),
  TranslateY(Length),
  TranslateZ(Length),
  Translate3d(Length, Length, Length),
  Scale(f64, Option<f64>),
  ScaleX(f64),
  ScaleY(f64),
  ScaleZ(f64),
  Scale3d(f64, f64, f64),
  Rotate(Angle),
  RotateX(Angle),
  RotateY(Angle),
  RotateZ(Angle),
  Rotate3d(f64, f64, f64, Angle),
  Skew(Angle, Option<Angle>),
  SkewX(Angle),
  SkewY(Angle),
  Perspective(Option<Length>),
  Matrix([f64; 6]),
  Matrix3d([f64; 16]),
}

impl Transform {
  fn parse<'i, 't>(
    input: &mut Parser<'i, 't>,
  ) -> Result<Self, ParseError<'i, CSSValueCustomError>> {
    let name = input.expect_function()?;
    match_ignore_ascii_case! { &name,
      "translate" => {
        input.parse_nested_block(|arguments| {
          let x = NumericValue::parse(arguments)?;
          let x = try_extract!(x, expect_length(true), arguments);
          let y = if !arguments.is_exhausted() {
            arguments.expect_comma()?;
            let value = NumericValue::parse(arguments)?;
            let value = try_extract!(value, expect_length(true), arguments);
            arguments.expect_exhausted()?;
            Some(value)
          } else { None };
          Ok(Transform::Translate(x, y))
        })
      },
      "translatex" => {
        input.parse_nested_block(|arguments| {
          let value = NumericValue::parse(arguments)?;
          let value = try_extract!(value, expect_length(true), arguments);
          arguments.expect_exhausted()?;
          Ok(Transform::TranslateX(value))
        })
      },
      "translatey" => {
        input.parse_nested_block(|arguments| {
          let value = NumericValue::parse(arguments)?;
          let value = try_extract!(value, expect_length(true), arguments);
          arguments.expect_exhausted()?;
          Ok(Transform::TranslateY(value))
        })
      },
      "translatez" => {
        input.parse_nested_block(|arguments| {
          let value = NumericValue::parse(arguments)?;
          let value = try_extract!(value, expect_length(true), arguments);
          arguments.expect_exhausted()?;
          Ok(Transform::TranslateZ(value))
        })
      },
      "translate3d" => {
        input.parse_nested_block(|arguments| {
          let x = NumericValue::parse(arguments)?;
          let x = try_extract!(x, expect_length(true), arguments);
          arguments.expect_comma()?;
          let y = NumericValue::parse(arguments)?;
          let y = try_extract!(y, expect_length(true), arguments);
          arguments.expect_comma()?;
          let z = NumericValue::parse(arguments)?;
          let z = try_extract!(z, expect_length(true), arguments);
          arguments.expect_exhausted()?;
          Ok(Transform::Translate3d(x, y, z))
        })
      },
      "scale" => {
        input.parse_nested_block(|arguments| {
          let x = NumericValue::parse(arguments)?;
          let x = try_extract!(x, expect_number_or_percent(), arguments);
          let y = if !arguments.is_exhausted() {
            arguments.expect_comma()?;
            let value = NumericValue::parse(arguments)?;
            let value = try_extract!(value, expect_number_or_percent(), arguments);
            arguments.expect_exhausted()?;
            Some(value)
          } else { None };
          Ok(Transform::Scale(x, y))
        })
      },
      "scalex" => {
        input.parse_nested_block(|arguments| {
          let value = NumericValue::parse(arguments)?;
          let value = try_extract!(value, expect_number_or_percent(), arguments);
          arguments.expect_exhausted()?;
          Ok(Transform::ScaleX(value))
        })
      },
      "scaley" => {
        input.parse_nested_block(|arguments| {
          let value = NumericValue::parse(arguments)?;
          let value = try_extract!(value, expect_number_or_percent(), arguments);
          arguments.expect_exhausted()?;
          Ok(Transform::ScaleY(value))
        })
      },
      "scalez" => {
        input.parse_nested_block(|arguments| {
          let value = NumericValue::parse(arguments)?;
          let value = try_extract!(value, expect_number_or_percent(), arguments);
          arguments.expect_exhausted()?;
          Ok(Transform::ScaleZ(value))
        })
      },
      "scale3d" => {
        input.parse_nested_block(|arguments| {
          let x = NumericValue::parse(arguments)?;
          let x = try_extract!(x, expect_number_or_percent(), arguments);
          arguments.expect_comma()?;
          let y = NumericValue::parse(arguments)?;
          let y = try_extract!(y, expect_number_or_percent(), arguments);
          arguments.expect_comma()?;
          let z = NumericValue::parse(arguments)?;
          let z = try_extract!(z, expect_number_or_percent(), arguments);
          arguments.expect_exhausted()?;
          Ok(Transform::Scale3d(x, y, z))
        })
      },
      "rotate" => {
        input.parse_nested_block(|arguments| {
          let value = NumericValue::parse(arguments)?;
          let value = try_extract!(value, expect_angle(true), arguments);
          arguments.expect_exhausted()?;
          Ok(Transform::Rotate(value))
        })
      },
      "rotatex" => {
        input.parse_nested_block(|arguments| {
          let value = NumericValue::parse(arguments)?;
          let value = try_extract!(value, expect_angle(true), arguments);
          arguments.expect_exhausted()?;
          Ok(Transform::RotateX(value))
        })
      },
      "rotatey" => {
        input.parse_nested_block(|arguments| {
          let value = NumericValue::parse(arguments)?;
          let value = try_extract!(value, expect_angle(true), arguments);
          arguments.expect_exhausted()?;
          Ok(Transform::RotateY(value))
        })
      },
      "rotatez" => {
        input.parse_nested_block(|arguments| {
          let value = NumericValue::parse(arguments)?;
          let value = try_extract!(value, expect_angle(true), arguments);
          arguments.expect_exhausted()?;
          Ok(Transform::RotateZ(value))
        })
      },
      "rotate3d" => {
        input.parse_nested_block(|arguments| {
          let x = NumericValue::parse(arguments)?;
          let x = try_extract!(x, expect_number(), arguments);
          arguments.expect_comma()?;
          let y = NumericValue::parse(arguments)?;
          let y = try_extract!(y, expect_number(), arguments);
          arguments.expect_comma()?;
          let z = NumericValue::parse(arguments)?;
          let z = try_extract!(z, expect_number(), arguments);
          arguments.expect_comma()?;
          let a = NumericValue::parse(arguments)?;
          let a = try_extract!(a, expect_angle(true), arguments);
          arguments.expect_exhausted()?;
          Ok(Transform::Rotate3d(x, y, z, a))
        })
      },
      "skew" => {
        input.parse_nested_block(|arguments| {
          let x = NumericValue::parse(arguments)?;
          let x = try_extract!(x, expect_angle(true), arguments);
          let y = if !arguments.is_exhausted() {
            arguments.expect_comma()?;
            let value = NumericValue::parse(arguments)?;
            let value = try_extract!(value, expect_angle(true), arguments);
            arguments.expect_exhausted()?;
            Some(value)
          } else { None };
          Ok(Transform::Skew(x, y))
        })
      },
      "skewx" => {
        input.parse_nested_block(|arguments| {
          let value = NumericValue::parse(arguments)?;
          let value = try_extract!(value, expect_angle(true), arguments);
          arguments.expect_exhausted()?;
          Ok(Transform::SkewX(value))
        })
      },
      "skewy" => {
        input.parse_nested_block(|arguments| {
          let value = NumericValue::parse(arguments)?;
          let value = try_extract!(value, expect_angle(true), arguments);
          arguments.expect_exhausted()?;
          Ok(Transform::SkewY(value))
        })
      },
      "perspective" => {
        input.parse_nested_block(|arguments| {
          let value = {
            let start = arguments.state();
            if arguments.expect_ident_matching("none").is_ok() {
              None
            } else {
              arguments.reset(&start);
              let value = NumericValue::parse(arguments)?;
              let value = try_extract!(value, expect_length(true), arguments);
              Some(value)
            }
          };
          arguments.expect_exhausted()?;
          Ok(Transform::Perspective(value))
        })
      },
      "matrix" => {
        input.parse_nested_block(|arguments| {
          let mut result = [0.0; 6];
          for (i, slot) in result.iter_mut().enumerate() {
            if i != 0 {
              arguments.expect_comma()?;
            }
            let value = NumericValue::parse(arguments)?;
            let number = try_extract!(value, expect_number(), arguments);
            *slot = number;
          }
          arguments.expect_exhausted()?;
          Ok(Transform::Matrix(result))
        })
      },
      "matrix3d" => {
        input.parse_nested_block(|arguments| {
          let mut result = [0.0; 16];
          for (i, slot) in result.iter_mut().enumerate() {
            if i != 0 {
              arguments.expect_comma()?;
            }
            let value = NumericValue::parse(arguments)?;
            let number = try_extract!(value, expect_number(), arguments);
            *slot = number;
          }
          arguments.expect_exhausted()?;
          Ok(Transform::Matrix3d(result))
        })
      },
      _ => {
        let name = name.to_string();
        Err(input.new_custom_error(CSSValueCustomError::InvalidFunction(name)))
      },
    }
  }
}

pub struct TransformListParser<'i, 't> {
  parser: Parser<'i, 't>,
  has_function: bool,
  finished: bool,
}

impl<'i: 't, 't> TransformListParser<'i, 't> {
  pub fn new(input: &'t mut ParserInput<'i>) -> Self {
    Self {
      parser: Parser::new(input),
      has_function: false,
      finished: false,
    }
  }
}

impl<'i, 't> Iterator for TransformListParser<'i, 't> {
  type Item = Result<Transform, ParseError<'i, CSSValueCustomError>>;

  fn next(&mut self) -> Option<Self::Item> {
    if self.finished {
      return None;
    }

    let input = &mut self.parser;
    if input.is_exhausted() {
      self.finished = true;
      if self.has_function {
        return None;
      } else {
        let token = match input.next_including_whitespace_and_comments() {
          Ok(token) => token.clone(),
          Err(e) => return Some(Err(e.into())),
        };
        return Some(Err(input.new_unexpected_token_error(token)));
      }
    }

    if !self.has_function {
      let start = input.state();
      if input.expect_ident_matching("none").is_ok() {
        self.finished = true;
        match input.expect_exhausted() {
          Ok(_) => return None,
          Err(error) => return Some(Err(error.into())),
        }
      } else {
        input.reset(&start);
      }
    }

    let result = Transform::parse(input);
    if result.is_ok() {
      self.has_function = true;
    } else {
      self.finished = true;
    }
    Some(result)
  }
}
