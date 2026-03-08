// Copyright 2018-2026 the Deno authors. MIT license.

use std::f32;
use std::fmt;
use std::ops;

use cssparser::ParseError;
use cssparser::Parser;
pub use cssparser::ParserInput;
use cssparser::Token;
use cssparser::match_ignore_ascii_case;

pub type CSSValueError<'i> = ParseError<'i, CSSValueCustomError>;

#[derive(Debug, thiserror::Error)]
#[cfg_attr(test, derive(PartialEq))]
pub enum CSSValueCustomError {
  #[error("unexpected token")]
  UnexpectedToken,
  #[error("unexpected numeric type")]
  UnexpectedNumericType,
  #[error("numeric types do not match")]
  NumericTypeMismatch,
  #[error("contains relative <length> values")]
  ContainsRelativeLengthValues,
  #[error("contains unsupported <dimension> values")]
  UnsupportedDimension,
  #[error("the dimensions of the calculation results are incorrect")]
  InvalidDimension,
  #[error("contains unsupported function")]
  UnsupportedFunction,
  #[error("contains invalid function")]
  InvalidFunction,
}

#[derive(Clone, Copy, Debug)]
#[cfg_attr(test, derive(PartialEq))]
pub struct Number(f32);

impl ops::Deref for Number {
  type Target = f32;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

#[derive(Clone, Debug)]
#[cfg_attr(test, derive(PartialEq))]
pub struct Length {
  value: f32,
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
  const INCH_TO_PX: f32 = 96.0;
  const INCH_TO_CM: f32 = 2.54;

  pub fn to_pixels(&self) -> f32 {
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
  value: f32,
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
  const TURN_TO_DEG: f32 = 360.0;
  const TURN_TO_GRAD: f32 = 400.0;

  pub fn to_degrees(&self) -> f32 {
    let value = self.value;
    match self.unit {
      AngleUnit::Deg => value,
      AngleUnit::Grad => value * (Self::TURN_TO_DEG / Self::TURN_TO_GRAD),
      AngleUnit::Rad => value.to_degrees(),
      AngleUnit::Turn => value * Self::TURN_TO_DEG,
    }
  }

  pub fn to_radians(&self) -> f32 {
    let value = self.value;
    match self.unit {
      AngleUnit::Deg => value.to_radians(),
      AngleUnit::Grad => {
        (value * (Self::TURN_TO_DEG / Self::TURN_TO_GRAD)).to_radians()
      }
      AngleUnit::Rad => value,
      AngleUnit::Turn => value * f32::consts::TAU,
    }
  }
}

#[derive(Clone, Copy)]
#[cfg_attr(test, derive(PartialEq))]
pub struct Percent(f32);

impl ops::Deref for Percent {
  type Target = f32;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl fmt::Debug for Percent {
  fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
    fmt
      .debug_tuple("Percent")
      .field(&format_args!("{}%", self.0 * 100.0))
      .finish()
  }
}

// Currently, units for <time>, <frequency>, <resolution>, and <flex> are not supported
// as are combined units such as <length-percentage>
#[derive(Debug)]
#[cfg_attr(test, derive(PartialEq))]
pub enum NumericValue {
  Zero,
  Number(Number),
  Length(Length),
  Angle(Angle),
  Percent(Percent),
}

impl NumericValue {
  #[inline]
  fn expect_number(
    self,
    allow_percent: bool,
  ) -> Result<Number, CSSValueCustomError> {
    match self {
      NumericValue::Zero => Ok(Number(0.0)),
      NumericValue::Number(number) => Ok(number),
      NumericValue::Percent(percent) => {
        if allow_percent {
          Ok(Number(*percent))
        } else {
          Err(CSSValueCustomError::UnexpectedNumericType)
        }
      }
      _ => Err(CSSValueCustomError::UnexpectedNumericType),
    }
  }

  #[inline]
  fn expect_length(
    self,
    allow_zero: bool,
  ) -> Result<Length, CSSValueCustomError> {
    match self {
      NumericValue::Zero => {
        if allow_zero {
          Ok(Length {
            value: 0.0,
            unit: LengthUnit::Px,
          })
        } else {
          Err(CSSValueCustomError::UnexpectedNumericType)
        }
      }
      NumericValue::Length(length) => Ok(length),
      _ => Err(CSSValueCustomError::UnexpectedNumericType),
    }
  }

  #[inline]
  fn expect_angle(
    self,
    allow_zero: bool,
  ) -> Result<Angle, CSSValueCustomError> {
    match self {
      NumericValue::Zero => {
        if allow_zero {
          Ok(Angle {
            value: 0.0,
            unit: AngleUnit::Deg,
          })
        } else {
          Err(CSSValueCustomError::UnexpectedNumericType)
        }
      }
      NumericValue::Angle(angle) => Ok(angle),
      _ => Err(CSSValueCustomError::UnexpectedNumericType),
    }
  }

  #[inline]
  fn expect_percent(self) -> Result<Percent, CSSValueCustomError> {
    match self {
      NumericValue::Percent(percent) => Ok(percent),
      _ => Err(CSSValueCustomError::UnexpectedNumericType),
    }
  }
}

// Currently, units for <time>, <frequency>, <resolution>, and <flex> are not supported
// https://drafts.css-houdini.org/css-typed-om-1/#numeric-typing
#[derive(Debug, Default, PartialEq)]
struct Dimension {
  length: i8,
  angle: i8,
  percent: i8,
}

impl ops::AddAssign<&Dimension> for Dimension {
  fn add_assign(&mut self, rhs: &Self) {
    self.length += rhs.length;
    self.angle += rhs.angle;
    self.percent += rhs.percent;
  }
}

impl ops::SubAssign<&Dimension> for Dimension {
  fn sub_assign(&mut self, rhs: &Self) {
    self.length -= rhs.length;
    self.angle -= rhs.angle;
    self.percent -= rhs.percent;
  }
}

// Struct for intermediate representations of calculations like `calc(1px / 1px * 1px)`
// Currently, combined units such as <length-percentage> are not supported
#[derive(Debug)]
#[cfg_attr(test, derive(PartialEq))]
struct MathValue {
  value: f32,
  dimension: Dimension,
}

impl From<NumericValue> for MathValue {
  fn from(value: NumericValue) -> Self {
    match value {
      NumericValue::Zero => MathValue {
        value: 0.0,
        dimension: Default::default(),
      },
      NumericValue::Number(Number(value)) => MathValue {
        value,
        dimension: Default::default(),
      },
      NumericValue::Length(length) => {
        let value = length.to_pixels();
        MathValue {
          value,
          dimension: Dimension {
            length: 1,
            angle: 0,
            percent: 0,
          },
        }
      }
      NumericValue::Angle(angle) => {
        let value = angle.to_degrees();
        MathValue {
          value,
          dimension: Dimension {
            length: 0,
            angle: 1,
            percent: 0,
          },
        }
      }
      NumericValue::Percent(Percent(value)) => MathValue {
        value,
        dimension: Dimension {
          length: 0,
          angle: 0,
          percent: 1,
        },
      },
    }
  }
}

impl TryFrom<MathValue> for NumericValue {
  type Error = CSSValueCustomError;

  fn try_from(accumulator: MathValue) -> Result<Self, Self::Error> {
    let value = accumulator.value;
    match accumulator.dimension {
      Dimension {
        length: 0,
        angle: 0,
        percent: 0,
      } => Ok(NumericValue::Number(Number(value))),
      Dimension {
        length: 1,
        angle: 0,
        percent: 0,
      } => Ok(NumericValue::Length(Length {
        value,
        unit: LengthUnit::Px,
      })),
      Dimension {
        length: 0,
        angle: 1,
        percent: 0,
      } => Ok(NumericValue::Angle(Angle {
        value,
        unit: AngleUnit::Deg,
      })),
      Dimension {
        length: 0,
        angle: 0,
        percent: 1,
      } => Ok(NumericValue::Percent(Percent(value))),
      _ => Err(CSSValueCustomError::InvalidDimension),
    }
  }
}

impl MathValue {
  #[inline]
  fn try_add_assign(
    &mut self,
    other: &MathValue,
  ) -> Result<(), CSSValueCustomError> {
    if self.dimension != other.dimension {
      return Err(CSSValueCustomError::NumericTypeMismatch);
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
      return Err(CSSValueCustomError::NumericTypeMismatch);
    }
    self.value -= other.value;
    Ok(())
  }

  fn expect_number(
    self,
    allow_percent: bool,
  ) -> Result<Number, CSSValueCustomError> {
    if self.dimension != Dimension::default() {
      if allow_percent {
        let percent = self.expect_percent()?;
        return Ok(Number(*percent));
      }
      return Err(CSSValueCustomError::UnexpectedNumericType);
    }
    Ok(Number(self.value))
  }

  fn expect_length(self) -> Result<Length, CSSValueCustomError> {
    let dimension = Dimension {
      length: 1,
      angle: 0,
      percent: 0,
    };
    if self.dimension != dimension {
      return Err(CSSValueCustomError::UnexpectedNumericType);
    }
    Ok(Length {
      value: self.value,
      unit: LengthUnit::Px,
    })
  }

  fn expect_angle(self) -> Result<Angle, CSSValueCustomError> {
    let dimension = Dimension {
      length: 0,
      angle: 1,
      percent: 0,
    };
    if self.dimension != dimension {
      return Err(CSSValueCustomError::UnexpectedNumericType);
    }
    Ok(Angle {
      value: self.value,
      unit: AngleUnit::Deg,
    })
  }

  fn expect_percent(self) -> Result<Percent, CSSValueCustomError> {
    let dimension = Dimension {
      length: 0,
      angle: 0,
      percent: 1,
    };
    if self.dimension != dimension {
      return Err(CSSValueCustomError::UnexpectedNumericType);
    }
    Ok(Percent(self.value))
  }
}

impl ops::MulAssign<&MathValue> for MathValue {
  #[inline]
  fn mul_assign(&mut self, other: &MathValue) {
    self.value *= other.value;
    self.dimension += &other.dimension;
  }
}

impl ops::DivAssign<&MathValue> for MathValue {
  #[inline]
  fn div_assign(&mut self, other: &MathValue) {
    self.value /= other.value;
    self.dimension -= &other.dimension;
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
      NumericAccumulator::Math(math) => NumericValue::try_from(math),
    }
  }

  #[inline]
  fn expect_number(
    self,
    allow_percent: bool,
  ) -> Result<Number, CSSValueCustomError> {
    match self {
      NumericAccumulator::Numeric(numeric) => {
        numeric.expect_number(allow_percent)
      }
      NumericAccumulator::Math(math) => math.expect_number(allow_percent),
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
  fn expect_percent(self) -> Result<Percent, CSSValueCustomError> {
    match self {
      NumericAccumulator::Numeric(numeric) => numeric.expect_percent(),
      NumericAccumulator::Math(math) => math.expect_percent(),
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

impl NumericValue {
  fn parse<'i, 't>(
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
        Ok(NumericValue::Number(Number(*value)).into())
      }
      Token::Dimension { value, unit, .. } => {
        match_ignore_ascii_case! { &unit,
          // https://www.w3.org/TR/css-values-4/#absolute-lengths
          "cm" => Ok(NumericValue::Length(Length { value: *value, unit: LengthUnit::Cm}).into()),
          "mm" => Ok(NumericValue::Length(Length { value: *value, unit: LengthUnit::Mm}).into()),
          "q" => Ok(NumericValue::Length(Length { value: *value, unit: LengthUnit::Q}).into()),
          "in" => Ok(NumericValue::Length(Length { value: *value, unit: LengthUnit::In}).into()),
          "pc" => Ok(NumericValue::Length(Length { value: *value, unit: LengthUnit::Pc}).into()),
          "pt" => Ok(NumericValue::Length(Length { value: *value, unit: LengthUnit::Pt}).into()),
          "px" => Ok(NumericValue::Length(Length { value: *value, unit: LengthUnit::Px}).into()),
          // https://www.w3.org/TR/css-values-4/#relative-lengths
          "em" | "rem" | "ex" | "rex" | "cap" | "rcap" | "ch" | "rch" | "ic" | "ric" | "lh" | "rlh" |
          "vw" | "svw" | "lvw" | "dvw" | "vh" | "svh" | "lvh" | "dvh" | "vi" | "svi" | "lvi" | "dvi" |
          "vb" | "svb" | "lvb" | "dvb" | "vmin" | "svmin" | "lvmin" | "dvmin" | "vmax" | "svmax" | "lvmax" | "dvmax" |
          // https://www.w3.org/TR/css-contain-3/#container-lengths
          "cqw" | "cqh" | "cqi" | "cqb" | "cqmin" | "cqmax"
          => Err(input.new_custom_error(CSSValueCustomError::ContainsRelativeLengthValues)),
          // https://www.w3.org/TR/css-values-4/#angles
          "deg" => Ok(NumericValue::Angle(Angle { value: *value, unit: AngleUnit::Deg }).into()),
          "grad" => Ok(NumericValue::Angle(Angle { value: *value, unit: AngleUnit::Grad }).into()),
          "rad" => Ok(NumericValue::Angle(Angle { value: *value, unit: AngleUnit::Rad }).into()),
          "turn" => Ok(NumericValue::Angle(Angle { value: *value, unit: AngleUnit::Turn }).into()),
          // https://www.w3.org/TR/css-values-4/#time
          "s" | "ms" |
          // https://www.w3.org/TR/css-values-4/#frequency
          "hz" | "khz" |
          // https://www.w3.org/TR/css-values-4/#resolution
          "dpi" | "dpcm" | "dppx" | "x" |
          // https://www.w3.org/TR/css-grid-2/#fr-unit
          "fr" => Err(input.new_custom_error(CSSValueCustomError::UnsupportedDimension)),
          _ => Err(input.new_custom_error(CSSValueCustomError::UnexpectedToken))
        }
      }
      Token::Percentage { unit_value, .. } => {
        Ok(NumericValue::Percent(Percent(*unit_value)).into())
      }
      Token::Function(name) => {
        state.function_depth += 1;
        let result = match_ignore_ascii_case! { &name,
          // https://www.w3.org/TR/css-values-4/#calc-func
          "calc" => {
            input.parse_nested_block(|arguments| {
              let value = Self::parse_additive_expression(arguments, state)?;
              arguments.expect_exhausted()?;
              Ok(value)
            })
          },
          // https://www.w3.org/TR/css-values-4/#comp-func
          "min" => {
            input.parse_nested_block(|arguments| {
              let value = Self::parse_additive_expression(arguments, state)?;
              let numeric = match value.expect_numeric() {
                Ok(numeric) => numeric,
                Err(error) => return Err(arguments.new_custom_error(error)),
              };
              let result: NumericAccumulator = match numeric {
                NumericValue::Zero => unreachable!(),
                NumericValue::Number(number) => {
                  let mut current = *number;
                  while !arguments.is_exhausted() {
                    arguments.expect_comma()?;
                    let value = Self::parse_additive_expression(arguments, state)?;
                    let value = match value.expect_number(false) {
                      Ok(Number(value)) => value,
                      Err(error) => return Err(arguments.new_custom_error(error)),
                    };
                    current = minimum(current, value);
                  }
                  NumericValue::Number(Number(current)).into()
                },
                NumericValue::Length(length) => {
                  let mut current = length.to_pixels();
                  while !arguments.is_exhausted() {
                    arguments.expect_comma()?;
                    let value = Self::parse_additive_expression(arguments, state)?;
                    let value = match value.expect_length(false) {
                      Ok(length) => length.to_pixels(),
                      Err(error) => return Err(arguments.new_custom_error(error)),
                    };
                    current = minimum(current, value);
                  }
                  NumericValue::Length(Length { value: current, unit: LengthUnit::Px }).into()
                },
                NumericValue::Angle(angle) => {
                  let mut current = angle.to_degrees();
                  while !arguments.is_exhausted() {
                    arguments.expect_comma()?;
                    let value = Self::parse_additive_expression(arguments, state)?;
                    let value = match value.expect_angle(false) {
                      Ok(length) => length.to_degrees(),
                      Err(error) => return Err(arguments.new_custom_error(error)),
                    };
                    current = minimum(current, value);
                  }
                  NumericValue::Angle(Angle { value: current, unit: AngleUnit::Deg }).into()
                },
                NumericValue::Percent(percent) => {
                  let mut current = *percent;
                  while !arguments.is_exhausted() {
                    arguments.expect_comma()?;
                    let value = Self::parse_additive_expression(arguments, state)?;
                    let value = match value.expect_percent() {
                      Ok(length) => *length,
                      Err(error) => return Err(arguments.new_custom_error(error)),
                    };
                    current = minimum(current, value);
                  }
                  NumericValue::Percent(Percent(current)).into()
                },
              };
              Ok(result)
            })
          },
          "max" => {
            input.parse_nested_block(|arguments| {
              let value = Self::parse_additive_expression(arguments, state)?;
              let numeric = match value.expect_numeric() {
                Ok(numeric) => numeric,
                Err(error) => return Err(arguments.new_custom_error(error)),
              };
              let result: NumericAccumulator = match numeric {
                NumericValue::Zero => unreachable!(),
                NumericValue::Number(number) => {
                  let mut current = *number;
                  while !arguments.is_exhausted() {
                    arguments.expect_comma()?;
                    let value = Self::parse_additive_expression(arguments, state)?;
                    let value = match value.expect_number(false) {
                      Ok(Number(value)) => value,
                      Err(error) => return Err(arguments.new_custom_error(error)),
                    };
                    current = maximum(current, value);
                  }
                  NumericValue::Number(Number(current)).into()
                },
                NumericValue::Length(length) => {
                  let mut current = length.to_pixels();
                  while !arguments.is_exhausted() {
                    arguments.expect_comma()?;
                    let value = Self::parse_additive_expression(arguments, state)?;
                    let value = match value.expect_length(false) {
                      Ok(length) => length.to_pixels(),
                      Err(error) => return Err(arguments.new_custom_error(error)),
                    };
                    current = maximum(current, value);
                  }
                  NumericValue::Length(Length { value: current, unit: LengthUnit::Px }).into()
                },
                NumericValue::Angle(angle) => {
                  let mut current = angle.to_degrees();
                  while !arguments.is_exhausted() {
                    arguments.expect_comma()?;
                    let value = Self::parse_additive_expression(arguments, state)?;
                    let value = match value.expect_angle(false) {
                      Ok(length) => length.to_degrees(),
                      Err(error) => return Err(arguments.new_custom_error(error)),
                    };
                    current = maximum(current, value);
                  }
                  NumericValue::Angle(Angle { value: current, unit: AngleUnit::Deg }).into()
                },
                NumericValue::Percent(percent) => {
                  let mut current = *percent;
                  while !arguments.is_exhausted() {
                    arguments.expect_comma()?;
                    let value = Self::parse_additive_expression(arguments, state)?;
                    let value = match value.expect_percent() {
                      Ok(length) => *length,
                      Err(error) => return Err(arguments.new_custom_error(error)),
                    };
                    current = maximum(current, value);
                  }
                  NumericValue::Percent(Percent(current)).into()
                },
              };
              Ok(result)
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
                  let value = Self::parse_additive_expression(arguments, state)?;
                  let numeric = match value.expect_numeric() {
                    Ok(numeric) => numeric,
                    Err(error) => return Err(arguments.new_custom_error(error)),
                  };
                  Some(numeric)
                }
              };
              arguments.expect_comma()?;
              let value = Self::parse_additive_expression(arguments, state)?;
              let value = match value.expect_numeric() {
                Ok(numeric) => numeric,
                Err(error) => return Err(arguments.new_custom_error(error)),
              };
              arguments.expect_comma()?;
              let max: Option<NumericValue> = {
                let start = arguments.state();
                if arguments.expect_ident_matching("none").is_ok() {
                  None
                } else {
                  arguments.reset(&start);
                  let value = Self::parse_additive_expression(arguments, state)?;
                  let numeric = match value.expect_numeric() {
                    Ok(numeric) => numeric,
                    Err(error) => return Err(arguments.new_custom_error(error)),
                  };
                  Some(numeric)
                }
              };
              arguments.expect_exhausted()?;

              let result: NumericAccumulator = match value {
                NumericValue::Zero => unreachable!(),
                NumericValue::Number(Number(value)) => {
                  let min = match min {
                    Some(numeric) => {
                      match numeric.expect_number(false) {
                        Ok(numeric) => *numeric,
                        Err(error) => return Err(arguments.new_custom_error(error)),
                      }
                    },
                    None => f32::NEG_INFINITY,
                  };
                  let max = match max {
                    Some(numeric) => {
                      match numeric.expect_number(false) {
                        Ok(numeric) => *numeric,
                        Err(error) => return Err(arguments.new_custom_error(error)),
                      }
                    },
                    None => f32::INFINITY,
                  };
                  NumericValue::Number(Number(maximum(min, minimum(value, max)))).into()
                },
                NumericValue::Length(value) => {
                  let min = match min {
                    Some(numeric) => {
                      match numeric.expect_length(false) {
                        Ok(length) => length.to_pixels(),
                        Err(error) => return Err(arguments.new_custom_error(error)),
                      }
                    },
                    None => f32::NEG_INFINITY,
                  };
                  let max = match max {
                    Some(numeric) => {
                      match numeric.expect_length(false) {
                        Ok(length) => length.to_pixels(),
                        Err(error) => return Err(arguments.new_custom_error(error)),
                      }
                    },
                    None => f32::INFINITY,
                  };
                  NumericValue::Length(Length {
                    value: maximum(min, minimum(value.to_pixels(), max)),
                    unit: LengthUnit::Px,
                  }).into()
                },
                NumericValue::Angle(value) => {
                  let min = match min {
                    Some(numeric) => {
                      match numeric.expect_angle(false) {
                        Ok(angle) => angle.to_degrees(),
                        Err(error) => return Err(arguments.new_custom_error(error)),
                      }
                    },
                    None => f32::NEG_INFINITY,
                  };
                  let max = match max {
                    Some(numeric) => {
                      match numeric.expect_angle(false) {
                        Ok(angle) => angle.to_degrees(),
                        Err(error) => return Err(arguments.new_custom_error(error)),
                      }
                    },
                    None => f32::INFINITY,
                  };
                  NumericValue::Angle(Angle {
                    value: maximum(min, minimum(value.to_degrees(), max)),
                    unit: AngleUnit::Deg,
                  }).into()
                },
                NumericValue::Percent(Percent(value)) => {
                  let min = match min {
                    Some(numeric) => {
                      match numeric.expect_percent() {
                        Ok(percent) => *percent,
                        Err(error) => return Err(arguments.new_custom_error(error)),
                      }
                    },
                    None => f32::NEG_INFINITY,
                  };
                  let max = match max {
                    Some(numeric) => {
                      match numeric.expect_percent() {
                        Ok(percent) => *percent,
                        Err(error) => return Err(arguments.new_custom_error(error)),
                      }
                    },
                    None => f32::INFINITY,
                  };
                  NumericValue::Percent(Percent(maximum(min, minimum(value, max)))).into()
                },
              };
              Ok(result)
            })
          },
          // TODO(petamoriken): implement round functions
          // https://www.w3.org/TR/css-values-4/#round-func
          "round" | "mod" | "rem" => Err(input.new_custom_error(CSSValueCustomError::UnsupportedFunction)),
          // https://www.w3.org/TR/css-values-4/#trig-funcs
          "sin" => {
            input.parse_nested_block(|arguments| {
              let value = Self::parse_additive_expression(arguments, state)?;
              let numeric = match value.expect_numeric() {
                Ok(numeric) => numeric,
                Err(error) => return Err(arguments.new_custom_error(error)),
              };
              arguments.expect_exhausted()?;
              let result: NumericAccumulator = match numeric {
                NumericValue::Zero => unreachable!(),
                NumericValue::Number(Number(number)) => {
                  NumericValue::Number(Number(number.sin())).into()
                },
                NumericValue::Angle(angle) => {
                  NumericValue::Number(Number(angle.to_radians().sin())).into()
                },
                NumericValue::Length(_) |
                NumericValue::Percent(_) => return Err(arguments.new_custom_error(CSSValueCustomError::UnexpectedNumericType)),
              };
              Ok(result)
            })
          },
          "cos" => {
            input.parse_nested_block(|arguments| {
              let value = Self::parse_additive_expression(arguments, state)?;
              let numeric = match value.expect_numeric() {
                Ok(numeric) => numeric,
                Err(error) => return Err(arguments.new_custom_error(error)),
              };
              arguments.expect_exhausted()?;
              let result: NumericAccumulator = match numeric {
                NumericValue::Zero => unreachable!(),
                NumericValue::Number(Number(number)) => {
                  NumericValue::Number(Number(number.cos())).into()
                },
                NumericValue::Angle(angle) => {
                  NumericValue::Number(Number(angle.to_radians().cos())).into()
                },
                NumericValue::Length(_) |
                NumericValue::Percent(_) => return Err(arguments.new_custom_error(CSSValueCustomError::UnexpectedNumericType)),              };
              Ok(result)
            })
          },
          "tan" => {
            input.parse_nested_block(|arguments| {
              let value = Self::parse_additive_expression(arguments, state)?;
              let numeric = match value.expect_numeric() {
                Ok(numeric) => numeric,
                Err(error) => return Err(arguments.new_custom_error(error)),
              };
              arguments.expect_exhausted()?;
              let result: NumericAccumulator = match numeric {
                NumericValue::Zero => unreachable!(),
                NumericValue::Number(Number(number)) => {
                  NumericValue::Number(Number(number.tan())).into()
                },
                NumericValue::Angle(angle) => {
                  NumericValue::Number(Number(angle.to_radians().tan())).into()
                },
                NumericValue::Length(_) |
                NumericValue::Percent(_) => return Err(arguments.new_custom_error(CSSValueCustomError::UnexpectedNumericType)),              };
              Ok(result)
            })
          },
          "asin" => {
            input.parse_nested_block(|arguments| {
              let value = Self::parse_additive_expression(arguments, state)?;
              let number = match value.expect_number(false) {
                Ok(number) => *number,
                Err(error) => return Err(arguments.new_custom_error(error)),
              };
              arguments.expect_exhausted()?;
              let result: NumericAccumulator = NumericValue::Angle(Angle {
                value: number.asin(),
                unit: AngleUnit::Rad,
              }).into();
              Ok(result)
            })
          },
          "acos" => {
            input.parse_nested_block(|arguments| {
              let value = Self::parse_additive_expression(arguments, state)?;
              let number = match value.expect_number(false) {
                Ok(number) => *number,
                Err(error) => return Err(arguments.new_custom_error(error)),
              };
              arguments.expect_exhausted()?;
              let result: NumericAccumulator = NumericValue::Angle(Angle {
                value: number.acos(),
                unit: AngleUnit::Rad,
              }).into();
              Ok(result)
            })
          },
          "atan" => {
            input.parse_nested_block(|arguments| {
              let value = Self::parse_additive_expression(arguments, state)?;
              let number = match value.expect_number(false) {
                Ok(number) => *number,
                Err(error) => return Err(arguments.new_custom_error(error)),
              };
              arguments.expect_exhausted()?;
              let result: NumericAccumulator = NumericValue::Angle(Angle {
                value: number.atan(),
                unit: AngleUnit::Rad,
              }).into();
              Ok(result)
            })
          },
          "atan2" => {
            input.parse_nested_block(|arguments| {
              let y = Self::parse_additive_expression(arguments, state)?;
              let y = match y.expect_numeric() {
                Ok(numeric) => numeric,
                Err(error) => return Err(arguments.new_custom_error(error)),
              };
              arguments.expect_comma()?;
              let x = Self::parse_additive_expression(arguments, state)?;
              let x = match x.expect_numeric() {
                Ok(numeric) => numeric,
                Err(error) => return Err(arguments.new_custom_error(error)),
              };
              arguments.expect_exhausted()?;
              let result: NumericAccumulator = match (y, x) {
                (NumericValue::Number(Number(y)), NumericValue::Number(Number(x))) => {
                  NumericValue::Angle(Angle {
                    value: y.atan2(x),
                    unit: AngleUnit::Rad,
                  }).into()
                },
                (NumericValue::Length(y), NumericValue::Length(x)) => {
                  NumericValue::Angle(Angle {
                    value: y.to_pixels().atan2(x.to_pixels()),
                    unit: AngleUnit::Rad,
                  }).into()
                },
                (NumericValue::Angle(y), NumericValue::Angle(x)) => {
                  NumericValue::Angle(Angle {
                    value: y.to_degrees().atan2(x.to_degrees()),
                    unit: AngleUnit::Rad,
                  }).into()
                },
                (NumericValue::Percent(Percent(y)), NumericValue::Percent(Percent(x))) => {
                  NumericValue::Angle(Angle {
                    value: y.atan2(x),
                    unit: AngleUnit::Rad,
                  }).into()
                },
                _ => return Err(arguments.new_custom_error(CSSValueCustomError::UnexpectedNumericType)),
              };
              Ok(result)
            })
          },
          // https://www.w3.org/TR/css-values-4/#exponent-funcs
          "pow" => {
            input.parse_nested_block(|arguments| {
              let base = Self::parse_additive_expression(arguments, state)?;
              let base = match base.expect_number(false) {
                Ok(number) => *number,
                Err(error) => return Err(arguments.new_custom_error(error)),
              };
              arguments.expect_comma()?;
              let exponent = Self::parse_additive_expression(arguments, state)?;
              let exponent = match exponent.expect_number(false) {
                Ok(number) => *number,
                Err(error) => return Err(arguments.new_custom_error(error)),
              };
              arguments.expect_exhausted()?;
              let result = NumericValue::Number(Number(base.powf(exponent))).into();
              Ok(result)
            })
          },
          "sqrt" => {
            input.parse_nested_block(|arguments| {
              let value = Self::parse_additive_expression(arguments, state)?;
              let value = match value.expect_number(false) {
                Ok(number) => number,
                Err(error) => return Err(arguments.new_custom_error(error)),
              };
              arguments.expect_exhausted()?;
              let result = NumericValue::Number(Number(value.sqrt())).into();
              Ok(result)
            })
          },
          "hypot" => {
            input.parse_nested_block(|arguments| {
              let first = Self::parse_additive_expression(arguments, state)?;
              let first = match first.expect_numeric() {
                Ok(numeric) => numeric,
                Err(error) => return Err(arguments.new_custom_error(error)),
              };
              let result: NumericAccumulator = match first {
                NumericValue::Zero => unreachable!(),
                NumericValue::Number(first) => {
                  let mut args = vec![*first];
                  while !arguments.is_exhausted() {
                    arguments.expect_comma()?;
                    let value = Self::parse_additive_expression(arguments, state)?;
                    let value = match value.expect_number(false) {
                      Ok(number) => *number,
                      Err(error) => return Err(arguments.new_custom_error(error)),
                    };
                    args.push(value);
                  }
                  NumericValue::Number(Number(hypot(&args))).into()
                },
                NumericValue::Length(first) => {
                  let mut args = vec![first.to_pixels()];
                  while !arguments.is_exhausted() {
                    arguments.expect_comma()?;
                    let value = Self::parse_additive_expression(arguments, state)?;
                    let value = match value.expect_length(false) {
                      Ok(length) => length.to_pixels(),
                      Err(error) => return Err(arguments.new_custom_error(error)),
                    };
                    args.push(value);
                  }
                  NumericValue::Length(Length {
                    value: hypot(&args),
                    unit: LengthUnit::Px,
                  }).into()
                },
                NumericValue::Angle(first) => {
                  let mut args = vec![first.to_degrees()];
                  while !arguments.is_exhausted() {
                    arguments.expect_comma()?;
                    let value = Self::parse_additive_expression(arguments, state)?;
                    let value = match value.expect_angle(false) {
                      Ok(angle) => angle.to_degrees(),
                      Err(error) => return Err(arguments.new_custom_error(error)),
                    };
                    args.push(value);
                  }
                  NumericValue::Angle(Angle {
                    value: hypot(&args),
                    unit: AngleUnit::Deg,
                  }).into()
                },
                NumericValue::Percent(first) => {
                  let mut args = vec![*first];
                  while !arguments.is_exhausted() {
                    arguments.expect_comma()?;
                    let value = Self::parse_additive_expression(arguments, state)?;
                    let value = match value.expect_percent() {
                      Ok(number) => *number,
                      Err(error) => return Err(arguments.new_custom_error(error)),
                    };
                    args.push(value);
                  }
                  NumericValue::Number(Number(hypot(&args))).into()
                },
              };
              Ok(result)
            })
          },
          "log" => {
            input.parse_nested_block(|arguments| {
              let value = Self::parse_additive_expression(arguments, state)?;
              let value = match value.expect_number(false) {
                Ok(number) => *number,
                Err(error) => return Err(arguments.new_custom_error(error)),
              };
              let result: NumericAccumulator = if !arguments.is_exhausted() {
                arguments.expect_comma()?;
                let base = Self::parse_additive_expression(arguments, state)?;
                let base = match base.expect_number(false) {
                  Ok(number) => *number,
                  Err(error) => return Err(arguments.new_custom_error(error)),
                };
                arguments.expect_exhausted()?;
                NumericValue::Number(Number(value.log(base))).into()
              } else {
                NumericValue::Number(Number(value.ln())).into()
              };
              Ok(result)
            })
          },
          "exp" => {
            input.parse_nested_block(|arguments| {
              let value = Self::parse_additive_expression(arguments, state)?;
              let number = match value.expect_number(false) {
                Ok(number) => number,
                Err(error) => return Err(arguments.new_custom_error(error)),
              };
              arguments.expect_exhausted()?;
              let result: NumericAccumulator = NumericValue::Number(Number(number.exp())).into();
              Ok(result)
            })
          },
          // https://www.w3.org/TR/css-values-4/#sign-funcs
          "abs" => {
            input.parse_nested_block(|arguments| {
              let value = Self::parse_additive_expression(arguments, state)?;
              let value = match value.expect_numeric() {
                Ok(numeric) => numeric,
                Err(error) => return Err(arguments.new_custom_error(error)),
              };
              arguments.expect_exhausted()?;
              let result: NumericAccumulator = match value {
                NumericValue::Zero => unreachable!(),
                NumericValue::Number(Number(number)) => {
                  NumericValue::Number(Number(number.abs())).into()
                },
                NumericValue::Length(length) => {
                  NumericValue::Length(Length {
                    value: length.value.abs(),
                    unit: length.unit,
                  }).into()
                },
                NumericValue::Angle(angle) => {
                  NumericValue::Angle(Angle {
                    value: angle.value.abs(),
                    unit: angle.unit,
                  }).into()
                },
                NumericValue::Percent(Percent(percent)) => {
                  NumericValue::Percent(Percent(percent.abs())).into()
                },
              };
              Ok(result)
            })
          },
          "sign" => {
            input.parse_nested_block(|arguments| {
              let value = Self::parse_additive_expression(arguments, state)?;
              let value = match value.expect_numeric() {
                Ok(numeric) => numeric,
                Err(error) => return Err(arguments.new_custom_error(error)),
              };
              arguments.expect_exhausted()?;
              let result: NumericAccumulator = match value {
                NumericValue::Zero => unreachable!(),
                NumericValue::Number(Number(number)) => {
                  NumericValue::Number(Number(sign(number))).into()
                },
                NumericValue::Length(length) => {
                  NumericValue::Number(Number(sign(length.value))).into()
                },
                NumericValue::Angle(angle) => {
                  NumericValue::Number(Number(sign(angle.value))).into()
                },
                NumericValue::Percent(Percent(percent)) => {
                  NumericValue::Number(Number(sign(percent))).into()
                },
              };
              Ok(result)
            })
          },
          // https://www.w3.org/TR/css-variables-1/#using-variables
          "var" => return Err(input.new_custom_error(CSSValueCustomError::InvalidFunction)),
          _ => return Err(input.new_custom_error(CSSValueCustomError::UnexpectedToken)),
        };
        state.function_depth -= 1;
        result
      }
      Token::ParenthesisBlock => {
        if state.function_depth == 0 {
          return Err(
            input.new_custom_error(CSSValueCustomError::UnexpectedToken),
          );
        }
        input.parse_nested_block(|arguments| {
          let value = Self::parse_additive_expression(arguments, state)?;
          arguments.expect_exhausted()?;
          Ok(value)
        })
      }
      Token::Ident(ident) => {
        if state.function_depth == 0 {
          return Err(
            input.new_custom_error(CSSValueCustomError::UnexpectedToken),
          );
        }
        match_ignore_ascii_case! { &ident,
          // https://www.w3.org/TR/css-values-4/#calc-constants
          "e" => Ok(NumericValue::Number(Number(f32::consts::E)).into()),
          "pi" => Ok(NumericValue::Number(Number(f32::consts::PI)).into()),
          // https://www.w3.org/TR/css-values-4/#calc-error-constants
          "infinity" => Ok(NumericValue::Number(Number(f32::INFINITY)).into()),
          "-infinity" => Ok(NumericValue::Number(Number(f32::NEG_INFINITY)).into()),
          "nan" => Ok(NumericValue::Number(Number(f32::NAN)).into()),
          _ => Err(input.new_custom_error(CSSValueCustomError::UnexpectedToken))
        }
      }
      _ => Err(input.new_custom_error(CSSValueCustomError::UnexpectedToken)),
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

// TODO(petamoriken) Use f32::maximum instead https://github.com/rust-lang/rust/issues/91079
#[inline]
fn maximum(a: f32, b: f32) -> f32 {
  if a > b {
    a
  } else if b > a {
    b
  } else if a == b {
    if a.is_sign_positive() && b.is_sign_negative() {
      a
    } else {
      b
    }
  } else {
    // At least one input is NaN. Use `+` to perform NaN propagation and quieting.
    a + b
  }
}

// TODO(petamoriken) Use f32::minimum instead https://github.com/rust-lang/rust/issues/91079
#[inline]
fn minimum(a: f32, b: f32) -> f32 {
  if a < b {
    a
  } else if b < a {
    b
  } else if a == b {
    if a.is_sign_negative() && b.is_sign_positive() {
      a
    } else {
      b
    }
  } else {
    // At least one input is NaN. Use `+` to perform NaN propagation and quieting.
    a + b
  }
}

fn hypot(args: &[f32]) -> f32 {
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

#[inline]
fn sign(value: f32) -> f32 {
  if value == 0.0 { value } else { value.signum() }
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
    assert_eq!(result, Ok(NumericValue::Number(Number(42.0))));
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
    assert_relative_eq!(angle.to_radians(), f32::consts::PI);
  }

  #[test]
  fn percent() {
    let mut input = ParserInput::new("10%");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser);
    assert_eq!(result, Ok(NumericValue::Percent(Percent(0.1))));
  }

  #[test]
  fn calc_zero() {
    let mut input = ParserInput::new("calc(0)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser);
    assert_eq!(result, Ok(NumericValue::Number(Number(0.0))));
  }

  #[test]
  fn calc_const_e() {
    let mut input = ParserInput::new("calc(e)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser);
    assert_eq!(result, Ok(NumericValue::Number(Number(f32::consts::E))));
  }

  #[test]
  fn calc_const_pi() {
    let mut input = ParserInput::new("calc(pi)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser);
    assert_eq!(result, Ok(NumericValue::Number(Number(f32::consts::PI))));
  }

  #[test]
  fn calc_const_infinity() {
    let mut input = ParserInput::new("calc(infinity)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser);
    assert_eq!(result, Ok(NumericValue::Number(Number(f32::INFINITY))));
  }

  #[test]
  fn calc_const_neg_infinity() {
    let mut input = ParserInput::new("calc(-infinity)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser);
    assert_eq!(result, Ok(NumericValue::Number(Number(f32::NEG_INFINITY))));
  }

  #[test]
  fn calc_const_nan() {
    let mut input = ParserInput::new("calc(nan)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser);
    let Ok(NumericValue::Number(Number(value))) = result else {
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
    assert_eq!(result, Ok(NumericValue::Number(Number(2.0))));
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
    assert_eq!(result, Ok(NumericValue::Number(Number(-2.0))));
  }

  #[test]
  fn min_nan() {
    let mut input = ParserInput::new("min(-1, nan, 3)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser);
    let Ok(NumericValue::Number(Number(value))) = result else {
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
    assert_eq!(result, Ok(NumericValue::Number(Number(3.0))));
  }

  #[test]
  fn max_nan() {
    let mut input = ParserInput::new("max(-1, nan, 3)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser);
    let Ok(NumericValue::Number(Number(value))) = result else {
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
    assert_eq!(result, Ok(NumericValue::Number(Number(-1.0))));
  }

  #[test]
  fn clamp_none() {
    let mut input = ParserInput::new("clamp(none, 1 - 3, 3)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser);
    assert_eq!(result, Ok(NumericValue::Number(Number(-2.0))));
  }

  #[test]
  fn clamp_nan() {
    let mut input = ParserInput::new("clamp(-1, nan, 3)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser);
    let Ok(NumericValue::Number(Number(value))) = result else {
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
  fn sin() {
    let mut input = ParserInput::new("sin(pi / 2)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser);
    let Ok(NumericValue::Number(Number(value))) = result else {
      panic!("expect number: {:?}", result);
    };
    assert_relative_eq!(value, 1.0);
  }

  #[test]
  fn sin_angle() {
    let mut input = ParserInput::new("sin(90deg)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser);
    let Ok(NumericValue::Number(Number(value))) = result else {
      panic!("expect number: {:?}", result);
    };
    assert_relative_eq!(value, 1.0);
  }

  #[test]
  fn cos() {
    let mut input = ParserInput::new("cos(pi)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser);
    let Ok(NumericValue::Number(Number(value))) = result else {
      panic!("expect number: {:?}", result);
    };
    assert_relative_eq!(value, -1.0);
  }

  #[test]
  fn cos_angle() {
    let mut input = ParserInput::new("cos(180deg)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser);
    let Ok(NumericValue::Number(Number(value))) = result else {
      panic!("expect number: {:?}", result);
    };
    assert_relative_eq!(value, -1.0);
  }

  #[test]
  fn tan() {
    let mut input = ParserInput::new("tan(pi / 4)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser);
    let Ok(NumericValue::Number(Number(value))) = result else {
      panic!("expect number: {:?}", result);
    };
    assert_relative_eq!(value, 1.0);
  }

  #[test]
  fn tan_angle() {
    let mut input = ParserInput::new("tan(45deg)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser);
    let Ok(NumericValue::Number(Number(value))) = result else {
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
    let Ok(NumericValue::Number(Number(value))) = result else {
      panic!("expect number: {:?}", result);
    };
    assert_relative_eq!(value, 8.0);
  }

  #[test]
  fn sqrt() {
    let mut input = ParserInput::new("sqrt(4)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser);
    let Ok(NumericValue::Number(Number(value))) = result else {
      panic!("expect number: {:?}", result);
    };
    assert_relative_eq!(value, 2.0);
  }

  #[test]
  fn hypot() {
    let mut input = ParserInput::new("hypot(3, 4, 12)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser);
    let Ok(NumericValue::Number(Number(value))) = result else {
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
    let Ok(NumericValue::Number(Number(value))) = result else {
      panic!("expect number: {:?}", result);
    };
    assert_relative_eq!(value, 10.0_f32.ln());
  }

  #[test]
  fn log_multi_args() {
    let mut input = ParserInput::new("log(8, 2)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser);
    let Ok(NumericValue::Number(Number(value))) = result else {
      panic!("expect number: {:?}", result);
    };
    assert_relative_eq!(value, 3.0);
  }

  #[test]
  fn exp() {
    let mut input = ParserInput::new("exp(2)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser);
    let Ok(NumericValue::Number(Number(value))) = result else {
      panic!("expect number: {:?}", result);
    };
    assert_relative_eq!(value, 2.0_f32.exp());
  }

  #[test]
  fn abs() {
    let mut input = ParserInput::new("abs(-3)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser);
    let Ok(NumericValue::Number(Number(value))) = result else {
      panic!("expect number: {:?}", result);
    };
    assert_relative_eq!(value, 3.0);
  }

  #[test]
  fn abs_length() {
    let mut input = ParserInput::new("abs(-3px)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser);
    let Ok(NumericValue::Length(length)) = result else {
      panic!("expect length: {:?}", result);
    };
    assert_relative_eq!(length.to_pixels(), 3.0);
  }

  #[test]
  fn sign() {
    let mut input = ParserInput::new("sign(-2)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser);
    let Ok(NumericValue::Number(Number(value))) = result else {
      panic!("expect number: {:?}", result);
    };
    assert_eq!(value, -1.0);
  }

  #[test]
  fn sign_zero() {
    let mut input = ParserInput::new("sign(0)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser);
    let Ok(NumericValue::Number(Number(value))) = result else {
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
    let Ok(NumericValue::Number(Number(value))) = result else {
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
    let Ok(NumericValue::Number(Number(value))) = result else {
      panic!("expect number: {:?}", result);
    };
    assert_eq!(value, -1.0);
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
  Scale(Number, Option<Number>),
  ScaleX(Number),
  ScaleY(Number),
  ScaleZ(Number),
  Scale3d(Number, Number, Number),
  Rotate(Angle),
  RotateX(Angle),
  RotateY(Angle),
  RotateZ(Angle),
  Rotate3d(Number, Number, Number, Angle),
  Skew(Angle, Option<Angle>),
  SkewX(Angle),
  SkewY(Angle),
  Perspective(Option<Length>),
  Matrix([Number; 6]),
  Matrix3d([Number; 16]),
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
          let x = match x.expect_length(true) {
            Ok(length) => length,
            Err(error) => return Err(arguments.new_custom_error(error)),
          };
          let y = if !arguments.is_exhausted() {
            arguments.expect_comma()?;
            let value = NumericValue::parse(arguments)?;
            let value = match value.expect_length(true) {
              Ok(length) => length,
              Err(error) => return Err(arguments.new_custom_error(error)),
            };
            arguments.expect_exhausted()?;
            Some(value)
          } else { None };
          Ok(Transform::Translate(x, y))
        })
      },
      "translatex" => {
        input.parse_nested_block(|arguments| {
          let value = NumericValue::parse(arguments)?;
          let value = match value.expect_length(true) {
            Ok(length) => length,
            Err(error) => return Err(arguments.new_custom_error(error)),
          };
          arguments.expect_exhausted()?;
          Ok(Transform::TranslateX(value))
        })
      },
      "translatey" => {
        input.parse_nested_block(|arguments| {
          let value = NumericValue::parse(arguments)?;
          let value = match value.expect_length(true) {
            Ok(length) => length,
            Err(error) => return Err(arguments.new_custom_error(error)),
          };
          arguments.expect_exhausted()?;
          Ok(Transform::TranslateY(value))
        })
      },
      "translatez" => {
        input.parse_nested_block(|arguments| {
          let value = NumericValue::parse(arguments)?;
          let value = match value.expect_length(true) {
            Ok(length) => length,
            Err(error) => return Err(arguments.new_custom_error(error)),
          };
          arguments.expect_exhausted()?;
          Ok(Transform::TranslateZ(value))
        })
      },
      "translate3d" => {
        input.parse_nested_block(|arguments| {
          let x = NumericValue::parse(arguments)?;
          let x = match x.expect_length(true) {
            Ok(length) => length,
            Err(error) => return Err(arguments.new_custom_error(error)),
          };
          arguments.expect_comma()?;
          let y = NumericValue::parse(arguments)?;
          let y = match y.expect_length(true) {
            Ok(length) => length,
            Err(error) => return Err(arguments.new_custom_error(error)),
          };
          arguments.expect_comma()?;
          let z = NumericValue::parse(arguments)?;
          let z = match z.expect_length(true) {
            Ok(length) => length,
            Err(error) => return Err(arguments.new_custom_error(error)),
          };
          arguments.expect_exhausted()?;
          Ok(Transform::Translate3d(x, y, z))
        })
      },
      "scale" => {
        input.parse_nested_block(|arguments| {
          let x = NumericValue::parse(arguments)?;
          let x = match x.expect_number(true) {
            Ok(number) => number,
            Err(error) => return Err(arguments.new_custom_error(error)),
          };
          let y = if !arguments.is_exhausted() {
            arguments.expect_comma()?;
            let value = NumericValue::parse(arguments)?;
            let value = match value.expect_number(true) {
              Ok(number) => number,
              Err(error) => return Err(arguments.new_custom_error(error)),
            };
            arguments.expect_exhausted()?;
            Some(value)
          } else { None };
          Ok(Transform::Scale(x, y))
        })
      },
      "scalex" => {
        input.parse_nested_block(|arguments| {
          let value = NumericValue::parse(arguments)?;
          let value = match value.expect_number(true) {
            Ok(number) => number,
            Err(error) => return Err(arguments.new_custom_error(error)),
          };
          arguments.expect_exhausted()?;
          Ok(Transform::ScaleX(value))
        })
      },
      "scaley" => {
        input.parse_nested_block(|arguments| {
          let value = NumericValue::parse(arguments)?;
          let value = match value.expect_number(true) {
            Ok(number) => number,
            Err(error) => return Err(arguments.new_custom_error(error)),
          };
          arguments.expect_exhausted()?;
          Ok(Transform::ScaleY(value))
        })
      },
      "scalez" => {
        input.parse_nested_block(|arguments| {
          let value = NumericValue::parse(arguments)?;
          let value = match value.expect_number(true) {
            Ok(number) => number,
            Err(error) => return Err(arguments.new_custom_error(error)),
          };
          arguments.expect_exhausted()?;
          Ok(Transform::ScaleZ(value))
        })
      },
      "scale3d" => {
        input.parse_nested_block(|arguments| {
          let x = NumericValue::parse(arguments)?;
          let x = match x.expect_number(true) {
            Ok(number) => number,
            Err(error) => return Err(arguments.new_custom_error(error)),
          };
          arguments.expect_comma()?;
          let y = NumericValue::parse(arguments)?;
          let y = match y.expect_number(true) {
            Ok(number) => number,
            Err(error) => return Err(arguments.new_custom_error(error)),
          };
          arguments.expect_comma()?;
          let z = NumericValue::parse(arguments)?;
          let z = match z.expect_number(true) {
            Ok(number) => number,
            Err(error) => return Err(arguments.new_custom_error(error)),
          };
          arguments.expect_exhausted()?;
          Ok(Transform::Scale3d(x, y, z))
        })
      },
      "rotate" => {
        input.parse_nested_block(|arguments| {
          let value = NumericValue::parse(arguments)?;
          let value = match value.expect_angle(true) {
            Ok(angle) => angle,
            Err(error) => return Err(arguments.new_custom_error(error)),
          };
          arguments.expect_exhausted()?;
          Ok(Transform::Rotate(value))
        })
      },
      "rotatex" => {
        input.parse_nested_block(|arguments| {
          let value = NumericValue::parse(arguments)?;
          let value = match value.expect_angle(true) {
            Ok(angle) => angle,
            Err(error) => return Err(arguments.new_custom_error(error)),
          };
          arguments.expect_exhausted()?;
          Ok(Transform::RotateX(value))
        })
      },
      "rotatey" => {
        input.parse_nested_block(|arguments| {
          let value = NumericValue::parse(arguments)?;
          let value = match value.expect_angle(true) {
            Ok(angle) => angle,
            Err(error) => return Err(arguments.new_custom_error(error)),
          };
          arguments.expect_exhausted()?;
          Ok(Transform::RotateY(value))
        })
      },
      "rotatez" => {
        input.parse_nested_block(|arguments| {
          let value = NumericValue::parse(arguments)?;
          let value = match value.expect_angle(true) {
            Ok(angle) => angle,
            Err(error) => return Err(arguments.new_custom_error(error)),
          };
          arguments.expect_exhausted()?;
          Ok(Transform::RotateZ(value))
        })
      },
      "rotate3d" => {
        input.parse_nested_block(|arguments| {
          let x = NumericValue::parse(arguments)?;
          let x = match x.expect_number(false) {
            Ok(number) => number,
            Err(error) => return Err(arguments.new_custom_error(error)),
          };
          arguments.expect_comma()?;
          let y = NumericValue::parse(arguments)?;
          let y = match y.expect_number(false) {
            Ok(number) => number,
            Err(error) => return Err(arguments.new_custom_error(error)),
          };
          arguments.expect_comma()?;
          let z = NumericValue::parse(arguments)?;
          let z = match z.expect_number(false) {
            Ok(number) => number,
            Err(error) => return Err(arguments.new_custom_error(error)),
          };
          arguments.expect_comma()?;
          let a = NumericValue::parse(arguments)?;
          let a = match a.expect_angle(true) {
            Ok(angle) => angle,
            Err(error) => return Err(arguments.new_custom_error(error)),
          };
          arguments.expect_exhausted()?;
          Ok(Transform::Rotate3d(x, y, z, a))
        })
      },
      "skew" => {
        input.parse_nested_block(|arguments| {
          let x = NumericValue::parse(arguments)?;
          let x = match x.expect_angle(true) {
            Ok(angle) => angle,
            Err(error) => return Err(arguments.new_custom_error(error)),
          };
          let y = if !arguments.is_exhausted() {
            arguments.expect_comma()?;
            let value = NumericValue::parse(arguments)?;
            let value = match value.expect_angle(true) {
              Ok(angle) => angle,
              Err(error) => return Err(arguments.new_custom_error(error)),
            };
            arguments.expect_exhausted()?;
            Some(value)
          } else { None };
          Ok(Transform::Skew(x, y))
        })
      },
      "skewx" => {
        input.parse_nested_block(|arguments| {
          let value = NumericValue::parse(arguments)?;
          let value = match value.expect_angle(true) {
            Ok(angle) => angle,
            Err(error) => return Err(arguments.new_custom_error(error)),
          };
          arguments.expect_exhausted()?;
          Ok(Transform::SkewX(value))
        })
      },
      "skewy" => {
        input.parse_nested_block(|arguments| {
          let value = NumericValue::parse(arguments)?;
          let value = match value.expect_angle(true) {
            Ok(angle) => angle,
            Err(error) => return Err(arguments.new_custom_error(error)),
          };
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
              let value = match value.expect_length(true) {
                Ok(length) => length,
                Err(error) => return Err(arguments.new_custom_error(error)),
              };
              Some(value)
            }
          };
          arguments.expect_exhausted()?;
          Ok(Transform::Perspective(value))
        })
      },
      "matrix" => {
        input.parse_nested_block(|arguments| {
          let mut result = [Number(0.0); 6];
          for slot in &mut result {
            let value = NumericValue::parse(arguments)?;
            let number = match value.expect_number(false) {
              Ok(number) => number,
              Err(error) => return Err(arguments.new_custom_error(error)),
            };
            *slot = number;
          }
          arguments.expect_exhausted()?;
          Ok(Transform::Matrix(result))
        })
      },
      "matrix3d" => {
        input.parse_nested_block(|arguments| {
          let mut result = [Number(0.0); 16];
          for slot in &mut result {
            let value = NumericValue::parse(arguments)?;
            let number = match value.expect_number(false) {
              Ok(number) => number,
              Err(error) => return Err(arguments.new_custom_error(error)),
            };
            *slot = number;
          }
          arguments.expect_exhausted()?;
          Ok(Transform::Matrix3d(result))
        })
      },
      _ => Err(input.new_custom_error(CSSValueCustomError::UnexpectedToken)),
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
      return None;
    }

    if !self.has_function {
      let start = input.state();
      if input.expect_ident_matching("none").is_ok() {
        self.finished = true;
        return None;
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
