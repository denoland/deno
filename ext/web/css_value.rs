// Copyright 2018-2026 the Deno authors. MIT license.

use std::f32;
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
  #[error("unexpected numeric type")]
  UnexpectedNumericType,
  #[error(
    "contains relative <length> values that cannot be resolved at parse time"
  )]
  ContainsRelativeLengthValues,
  #[error("the {0} dimension is currently not supported")]
  UnsupportedDimension(&'static str),
  #[error(
    "contains <length-percentage> calculations that cannot be resolved at parse time"
  )]
  UnresolvableAtParseTime,
  #[error("cannot add or subtract different numeric types")]
  NumericTypeMismatch,
  #[error("the dimension of the calculation result is incorrect")]
  InvalidDimension,
  #[error("contains invalid function name: {0}")]
  InvalidFunction(String),
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

// Currently, units for <time>, <frequency>, <resolution>, and <flex> are not supported
// as are combined units such as <length-percentage>
#[derive(Debug)]
#[cfg_attr(test, derive(PartialEq))]
pub enum NumericValue {
  Zero,
  Number(f32),
  Length(Length),
  Angle(Angle),
  Percent(f32),
}

impl NumericValue {
  #[inline]
  pub fn expect_number(self) -> Result<f32, CSSValueCustomError> {
    match self {
      NumericValue::Zero => Ok(0.0),
      NumericValue::Number(number) => Ok(number),
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
  pub fn expect_angle(
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
  pub fn expect_percent(self) -> Result<f32, CSSValueCustomError> {
    match self {
      NumericValue::Percent(percent) => Ok(percent),
      _ => Err(CSSValueCustomError::UnexpectedNumericType),
    }
  }

  #[inline]
  pub fn expect_number_or_percent(self) -> Result<f32, CSSValueCustomError> {
    match self {
      NumericValue::Zero => Ok(0.0),
      NumericValue::Number(number) => Ok(number),
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
      NumericValue::Number(value) => MathValue {
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
      NumericValue::Percent(value) => MathValue {
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
      } => Ok(NumericValue::Number(value)),
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
      } => Ok(NumericValue::Percent(value)),
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
      // <length-percentage>
      if self.is_percent() && other.is_length()
        || self.is_length() && other.is_percent()
      {
        return Err(CSSValueCustomError::UnresolvableAtParseTime);
      }
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
      // <length-percentage>
      if self.is_percent() && other.is_length()
        || self.is_length() && other.is_percent()
      {
        return Err(CSSValueCustomError::UnresolvableAtParseTime);
      }
      return Err(CSSValueCustomError::NumericTypeMismatch);
    }
    self.value -= other.value;
    Ok(())
  }

  #[inline]
  fn is_number(&self) -> bool {
    matches!(
      self.dimension,
      Dimension {
        length: 0,
        angle: 0,
        percent: 0
      }
    )
  }

  #[inline]
  fn is_length(&self) -> bool {
    matches!(
      self.dimension,
      Dimension {
        length: 1,
        angle: 0,
        percent: 0
      }
    )
  }

  #[inline]
  fn is_angle(&self) -> bool {
    matches!(
      self.dimension,
      Dimension {
        length: 0,
        angle: 1,
        percent: 0
      }
    )
  }

  #[inline]
  fn is_percent(&self) -> bool {
    matches!(
      self.dimension,
      Dimension {
        length: 0,
        angle: 0,
        percent: 1
      }
    )
  }

  #[inline]
  fn expect_number(self) -> Result<f32, CSSValueCustomError> {
    if !self.is_number() {
      return Err(CSSValueCustomError::UnexpectedNumericType);
    }
    Ok(self.value)
  }

  #[inline]
  fn expect_length(self) -> Result<Length, CSSValueCustomError> {
    if !self.is_length() {
      return Err(CSSValueCustomError::UnexpectedNumericType);
    }
    Ok(Length {
      value: self.value,
      unit: LengthUnit::Px,
    })
  }

  #[inline]
  fn expect_angle(self) -> Result<Angle, CSSValueCustomError> {
    if !self.is_angle() {
      return Err(CSSValueCustomError::UnexpectedNumericType);
    }
    Ok(Angle {
      value: self.value,
      unit: AngleUnit::Deg,
    })
  }

  #[inline]
  fn expect_percent(self) -> Result<f32, CSSValueCustomError> {
    if !self.is_percent() {
      return Err(CSSValueCustomError::UnexpectedNumericType);
    }
    Ok(self.value)
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
  fn expect_number(self) -> Result<f32, CSSValueCustomError> {
    match self {
      NumericAccumulator::Numeric(numeric) => numeric.expect_number(),
      NumericAccumulator::Math(math) => math.expect_number(),
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
  fn expect_percent(self) -> Result<f32, CSSValueCustomError> {
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
        Ok(NumericValue::Number(*value).into())
      }
      Token::Dimension { value, unit, .. } => {
        match_ignore_ascii_case! { &unit,
          // https://www.w3.org/TR/css-values-4/#absolute-lengths
          "cm" => Ok(NumericValue::Length(Length { value: *value, unit: LengthUnit::Cm }).into()),
          "mm" => Ok(NumericValue::Length(Length { value: *value, unit: LengthUnit::Mm }).into()),
          "q" => Ok(NumericValue::Length(Length { value: *value, unit: LengthUnit::Q }).into()),
          "in" => Ok(NumericValue::Length(Length { value: *value, unit: LengthUnit::In }).into()),
          "pc" => Ok(NumericValue::Length(Length { value: *value, unit: LengthUnit::Pc }).into()),
          "pt" => Ok(NumericValue::Length(Length { value: *value, unit: LengthUnit::Pt }).into()),
          "px" => Ok(NumericValue::Length(Length { value: *value, unit: LengthUnit::Px }).into()),
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
          "s" | "ms" => Err(input.new_custom_error(CSSValueCustomError::UnsupportedDimension("<time>"))),
          // https://www.w3.org/TR/css-values-4/#frequency
          "hz" | "khz" => Err(input.new_custom_error(CSSValueCustomError::UnsupportedDimension("<frequency>"))),
          // https://www.w3.org/TR/css-values-4/#resolution
          "dpi" | "dpcm" | "dppx" | "x" => Err(input.new_custom_error(CSSValueCustomError::UnsupportedDimension("<resolution>"))),
          // https://www.w3.org/TR/css-grid-2/#fr-unit
          "fr" => Err(input.new_custom_error(CSSValueCustomError::UnsupportedDimension("<flex>"))),
          _ => {
            let token = token.clone();
            Err(input.new_unexpected_token_error(token))
          }
        }
      }
      Token::Percentage { unit_value, .. } => {
        Ok(NumericValue::Percent(*unit_value).into())
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
                  let mut current = number;
                  while !arguments.is_exhausted() {
                    arguments.expect_comma()?;
                    let value = Self::parse_additive_expression(arguments, state)?;
                    let value = match value.expect_number() {
                      Ok(number) => number,
                      Err(error) => return Err(arguments.new_custom_error(error)),
                    };
                    current = minimum(current, value);
                  }
                  NumericValue::Number(current).into()
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
                  let mut current = percent;
                  while !arguments.is_exhausted() {
                    arguments.expect_comma()?;
                    let value = Self::parse_additive_expression(arguments, state)?;
                    let value = match value.expect_percent() {
                      Ok(percent) => percent,
                      Err(error) => return Err(arguments.new_custom_error(error)),
                    };
                    current = minimum(current, value);
                  }
                  NumericValue::Percent(current).into()
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
                  let mut current = number;
                  while !arguments.is_exhausted() {
                    arguments.expect_comma()?;
                    let value = Self::parse_additive_expression(arguments, state)?;
                    let value = match value.expect_number() {
                      Ok(number) => number,
                      Err(error) => return Err(arguments.new_custom_error(error)),
                    };
                    current = maximum(current, value);
                  }
                  NumericValue::Number(current).into()
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
                  let mut current = percent;
                  while !arguments.is_exhausted() {
                    arguments.expect_comma()?;
                    let value = Self::parse_additive_expression(arguments, state)?;
                    let value = match value.expect_percent() {
                      Ok(percent) => percent,
                      Err(error) => return Err(arguments.new_custom_error(error)),
                    };
                    current = maximum(current, value);
                  }
                  NumericValue::Percent(current).into()
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
                NumericValue::Number(value) => {
                  let min = match min {
                    Some(numeric) => {
                      match numeric.expect_number() {
                        Ok(number) => number,
                        Err(error) => return Err(arguments.new_custom_error(error)),
                      }
                    },
                    None => f32::NEG_INFINITY,
                  };
                  let max = match max {
                    Some(numeric) => {
                      match numeric.expect_number() {
                        Ok(number) => number,
                        Err(error) => return Err(arguments.new_custom_error(error)),
                      }
                    },
                    None => f32::INFINITY,
                  };
                  NumericValue::Number(maximum(min, minimum(value, max))).into()
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
                NumericValue::Percent(value) => {
                  let min = match min {
                    Some(numeric) => {
                      match numeric.expect_percent() {
                        Ok(percent) => percent,
                        Err(error) => return Err(arguments.new_custom_error(error)),
                      }
                    },
                    None => f32::NEG_INFINITY,
                  };
                  let max = match max {
                    Some(numeric) => {
                      match numeric.expect_percent() {
                        Ok(percent) => percent,
                        Err(error) => return Err(arguments.new_custom_error(error)),
                      }
                    },
                    None => f32::INFINITY,
                  };
                  NumericValue::Percent(maximum(min, minimum(value, max))).into()
                },
              };
              Ok(result)
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
            fn round(strategy: &RoundStrategy, value: f32, interval: f32) -> f32 {
              if interval == 0.0 || value.is_nan() || interval.is_nan() || value.is_infinite() && interval.is_infinite() {
                return f32::NAN;
              }
              if value.is_infinite() {
                return value;
              }
              if interval.is_infinite() {
                return match strategy {
                  RoundStrategy::Up => if value > 0.0 { f32::INFINITY } else if value == 0.0 && value.is_sign_positive() { 0.0 } else { -0.0 },
                  RoundStrategy::Down => if value < 0.0 { f32::NEG_INFINITY } else if value == 0.0 && value.is_sign_negative() { -0.0 } else { 0.0 },
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
              let value = Self::parse_additive_expression(arguments, state)?;
              let value = match value.expect_numeric() {
                Ok(numeric) => numeric,
                Err(error) => return Err(arguments.new_custom_error(error)),
              };
              let interval = if !arguments.is_exhausted() {
                arguments.expect_comma()?;
                let interval = Self::parse_additive_expression(arguments, state)?;
                let interval = match value {
                  NumericValue::Zero => unreachable!(),
                  NumericValue::Number(_) => {
                    match interval.expect_number() {
                      Ok(number) => number,
                      Err(error) => return Err(arguments.new_custom_error(error)),
                    }
                  },
                  NumericValue::Length(_) => {
                    match interval.expect_length(false) {
                      Ok(length) => length.to_pixels(),
                      Err(error) => return Err(arguments.new_custom_error(error)),
                    }
                  },
                  NumericValue::Angle(_) => {
                    match interval.expect_angle(false) {
                      Ok(angle) => angle.to_degrees(),
                      Err(error) => return Err(arguments.new_custom_error(error)),
                    }
                  },
                  NumericValue::Percent(_) => {
                    match interval.expect_percent() {
                      Ok(percent) => percent,
                      Err(error) => return Err(arguments.new_custom_error(error)),
                    }
                  }
                };
                arguments.expect_exhausted()?;
                interval
              } else { 1.0 };
              let result: NumericAccumulator = match value {
                NumericValue::Zero => unreachable!(),
                NumericValue::Number(value) => {
                  NumericValue::Number(round(&strategy, value, interval)).into()
                },
                NumericValue::Length(value) => {
                  NumericValue::Length(Length {
                    value: round(&strategy, value.to_pixels(), interval),
                    unit: LengthUnit::Px,
                  }).into()
                },
                NumericValue::Angle(value) => {
                  NumericValue::Angle(Angle {
                    value: round(&strategy, value.to_degrees(), interval),
                    unit: AngleUnit::Deg,
                  }).into()
                },
                NumericValue::Percent(value) => {
                  NumericValue::Percent(round(&strategy, value, interval)).into()
                }
              };
              Ok(result)
            })
          },
          "mod" => {
            input.parse_nested_block(|arguments| {
              let dividend = Self::parse_additive_expression(arguments, state)?;
              let dividend = match dividend.expect_numeric() {
                Ok(numeric) => numeric,
                Err(error) => return Err(arguments.new_custom_error(error)),
              };
              arguments.expect_comma()?;
              let result: NumericAccumulator = match dividend {
                NumericValue::Zero => unreachable!(),
                NumericValue::Number(dividend) => {
                  let divisor = Self::parse_additive_expression(arguments, state)?;
                  let divisor = match divisor.expect_number() {
                    Ok(number) => number,
                    Err(error) => return Err(arguments.new_custom_error(error)),
                  };
                  arguments.expect_exhausted()?;
                  NumericValue::Number(dividend.rem_euclid(divisor)).into()
                },
                NumericValue::Length(dividend) => {
                  let dividend = dividend.to_pixels();
                  let divisor = Self::parse_additive_expression(arguments, state)?;
                  let divisor = match divisor.expect_length(false) {
                    Ok(length) => length.to_pixels(),
                    Err(error) => return Err(arguments.new_custom_error(error)),
                  };
                  arguments.expect_exhausted()?;
                  NumericValue::Length(Length {
                    value: dividend.rem_euclid(divisor),
                    unit: LengthUnit::Px,
                  }).into()
                },
                NumericValue::Angle(dividend) => {
                  let dividend = dividend.to_degrees();
                  let divisor = Self::parse_additive_expression(arguments, state)?;
                  let divisor = match divisor.expect_angle(false) {
                    Ok(angle) => angle.to_degrees(),
                    Err(error) => return Err(arguments.new_custom_error(error)),
                  };
                  arguments.expect_exhausted()?;
                  NumericValue::Angle(Angle {
                    value: dividend.rem_euclid(divisor),
                    unit: AngleUnit::Deg,
                  }).into()
                },
                NumericValue::Percent(dividend) => {
                  let divisor = Self::parse_additive_expression(arguments, state)?;
                  let divisor = match divisor.expect_percent() {
                    Ok(percent) => percent,
                    Err(error) => return Err(arguments.new_custom_error(error)),
                  };
                  arguments.expect_exhausted()?;
                  NumericValue::Percent(dividend.rem_euclid(divisor)).into()
                },
              };
              Ok(result)
            })
          },
          "rem" => {
            input.parse_nested_block(|arguments| {
              let dividend = Self::parse_additive_expression(arguments, state)?;
              let dividend = match dividend.expect_numeric() {
                Ok(numeric) => numeric,
                Err(error) => return Err(arguments.new_custom_error(error)),
              };
              arguments.expect_comma()?;
              let result: NumericAccumulator = match dividend {
                NumericValue::Zero => unreachable!(),
                NumericValue::Number(dividend) => {
                  let divisor = Self::parse_additive_expression(arguments, state)?;
                  let divisor = match divisor.expect_number() {
                    Ok(number) => number,
                    Err(error) => return Err(arguments.new_custom_error(error)),
                  };
                  arguments.expect_exhausted()?;
                  NumericValue::Number(dividend % divisor).into()
                },
                NumericValue::Length(dividend) => {
                  let dividend = dividend.to_pixels();
                  let divisor = Self::parse_additive_expression(arguments, state)?;
                  let divisor = match divisor.expect_length(false) {
                    Ok(length) => length.to_pixels(),
                    Err(error) => return Err(arguments.new_custom_error(error)),
                  };
                  arguments.expect_exhausted()?;
                  NumericValue::Length(Length {
                    value: dividend % divisor,
                    unit: LengthUnit::Px,
                  }).into()
                },
                NumericValue::Angle(dividend) => {
                  let dividend = dividend.to_degrees();
                  let divisor = Self::parse_additive_expression(arguments, state)?;
                  let divisor = match divisor.expect_angle(false) {
                    Ok(angle) => angle.to_degrees(),
                    Err(error) => return Err(arguments.new_custom_error(error)),
                  };
                  arguments.expect_exhausted()?;
                  NumericValue::Angle(Angle {
                    value: dividend % divisor,
                    unit: AngleUnit::Deg,
                  }).into()
                },
                NumericValue::Percent(dividend) => {
                  let divisor = Self::parse_additive_expression(arguments, state)?;
                  let divisor = match divisor.expect_percent() {
                    Ok(percent) => percent,
                    Err(error) => return Err(arguments.new_custom_error(error)),
                  };
                  arguments.expect_exhausted()?;
                  NumericValue::Percent(dividend % divisor).into()
                },
              };
              Ok(result)
            })
          },
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
                NumericValue::Number(number) => {
                  NumericValue::Number(number.sin()).into()
                },
                NumericValue::Angle(angle) => {
                  NumericValue::Number(angle.to_radians().sin()).into()
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
                NumericValue::Number(number) => {
                  NumericValue::Number(number.cos()).into()
                },
                NumericValue::Angle(angle) => {
                  NumericValue::Number(angle.to_radians().cos()).into()
                },
                NumericValue::Length(_) |
                NumericValue::Percent(_) => return Err(arguments.new_custom_error(CSSValueCustomError::UnexpectedNumericType)),
              };
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
                NumericValue::Number(number) => {
                  NumericValue::Number(number.tan()).into()
                },
                NumericValue::Angle(angle) => {
                  NumericValue::Number(angle.to_radians().tan()).into()
                },
                NumericValue::Length(_) |
                NumericValue::Percent(_) => return Err(arguments.new_custom_error(CSSValueCustomError::UnexpectedNumericType)),
              };
              Ok(result)
            })
          },
          "asin" => {
            input.parse_nested_block(|arguments| {
              let value = Self::parse_additive_expression(arguments, state)?;
              let number = match value.expect_number() {
                Ok(number) => number,
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
              let number = match value.expect_number() {
                Ok(number) => number,
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
              let number = match value.expect_number() {
                Ok(number) => number,
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
                (NumericValue::Number(y), NumericValue::Number(x)) => {
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
                (NumericValue::Percent(y), NumericValue::Percent(x)) => {
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
              let base = match base.expect_number() {
                Ok(number) => number,
                Err(error) => return Err(arguments.new_custom_error(error)),
              };
              arguments.expect_comma()?;
              let exponent = Self::parse_additive_expression(arguments, state)?;
              let exponent = match exponent.expect_number() {
                Ok(number) => number,
                Err(error) => return Err(arguments.new_custom_error(error)),
              };
              arguments.expect_exhausted()?;
              let result = NumericValue::Number(base.powf(exponent)).into();
              Ok(result)
            })
          },
          "sqrt" => {
            input.parse_nested_block(|arguments| {
              let value = Self::parse_additive_expression(arguments, state)?;
              let value = match value.expect_number() {
                Ok(number) => number,
                Err(error) => return Err(arguments.new_custom_error(error)),
              };
              arguments.expect_exhausted()?;
              let result = NumericValue::Number(value.sqrt()).into();
              Ok(result)
            })
          },
          "hypot" => {
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

            input.parse_nested_block(|arguments| {
              let first = Self::parse_additive_expression(arguments, state)?;
              let first = match first.expect_numeric() {
                Ok(numeric) => numeric,
                Err(error) => return Err(arguments.new_custom_error(error)),
              };
              let result: NumericAccumulator = match first {
                NumericValue::Zero => unreachable!(),
                NumericValue::Number(first) => {
                  let mut args = vec![first];
                  while !arguments.is_exhausted() {
                    arguments.expect_comma()?;
                    let value = Self::parse_additive_expression(arguments, state)?;
                    let value = match value.expect_number() {
                      Ok(number) => number,
                      Err(error) => return Err(arguments.new_custom_error(error)),
                    };
                    args.push(value);
                  }
                  NumericValue::Number(hypot(&args)).into()
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
                  let mut args = vec![first];
                  while !arguments.is_exhausted() {
                    arguments.expect_comma()?;
                    let value = Self::parse_additive_expression(arguments, state)?;
                    let value = match value.expect_percent() {
                      Ok(percent) => percent,
                      Err(error) => return Err(arguments.new_custom_error(error)),
                    };
                    args.push(value);
                  }
                  NumericValue::Number(hypot(&args)).into()
                },
              };
              Ok(result)
            })
          },
          "log" => {
            input.parse_nested_block(|arguments| {
              let value = Self::parse_additive_expression(arguments, state)?;
              let value = match value.expect_number() {
                Ok(number) => number,
                Err(error) => return Err(arguments.new_custom_error(error)),
              };
              let result: NumericAccumulator = if !arguments.is_exhausted() {
                arguments.expect_comma()?;
                let base = Self::parse_additive_expression(arguments, state)?;
                let base = match base.expect_number() {
                  Ok(number) => number,
                  Err(error) => return Err(arguments.new_custom_error(error)),
                };
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
              let value = Self::parse_additive_expression(arguments, state)?;
              let number = match value.expect_number() {
                Ok(number) => number,
                Err(error) => return Err(arguments.new_custom_error(error)),
              };
              arguments.expect_exhausted()?;
              let result: NumericAccumulator = NumericValue::Number(number.exp()).into();
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
                NumericValue::Number(number) => {
                  NumericValue::Number(number.abs()).into()
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
                NumericValue::Percent(percent) => {
                  NumericValue::Percent(percent.abs()).into()
                },
              };
              Ok(result)
            })
          },
          "sign" => {
            #[inline]
            fn sign(value: f32) -> f32 {
              if value == 0.0 { value } else { value.signum() }
            }

            input.parse_nested_block(|arguments| {
              let value = Self::parse_additive_expression(arguments, state)?;
              let value = match value.expect_numeric() {
                Ok(numeric) => numeric,
                Err(error) => return Err(arguments.new_custom_error(error)),
              };
              arguments.expect_exhausted()?;
              let result: NumericAccumulator = match value {
                NumericValue::Zero => unreachable!(),
                NumericValue::Number(number) => {
                  NumericValue::Number(sign(number)).into()
                },
                NumericValue::Length(length) => {
                  NumericValue::Number(sign(length.value)).into()
                },
                NumericValue::Angle(angle) => {
                  NumericValue::Number(sign(angle.value)).into()
                },
                NumericValue::Percent(percent) => {
                  NumericValue::Number(sign(percent)).into()
                },
              };
              Ok(result)
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
          let value = Self::parse_additive_expression(arguments, state)?;
          arguments.expect_exhausted()?;
          Ok(value)
        })
      }
      Token::Ident(ident) => {
        if state.function_depth == 0 {
          let token = token.clone();
          return Err(input.new_unexpected_token_error(token));
        }
        match_ignore_ascii_case! { &ident,
          // https://www.w3.org/TR/css-values-4/#calc-constants
          "e" => Ok(NumericValue::Number(f32::consts::E).into()),
          "pi" => Ok(NumericValue::Number(f32::consts::PI).into()),
          // https://www.w3.org/TR/css-values-4/#calc-error-constants
          "infinity" => Ok(NumericValue::Number(f32::INFINITY).into()),
          "-infinity" => Ok(NumericValue::Number(f32::NEG_INFINITY).into()),
          "nan" => Ok(NumericValue::Number(f32::NAN).into()),
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
    assert_eq!(result, Ok(NumericValue::Percent(0.1)));
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
    assert_eq!(result, Ok(NumericValue::Number(f32::consts::E)));
  }

  #[test]
  fn calc_const_pi() {
    let mut input = ParserInput::new("calc(pi)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser);
    assert_eq!(result, Ok(NumericValue::Number(f32::consts::PI)));
  }

  #[test]
  fn calc_const_infinity() {
    let mut input = ParserInput::new("calc(infinity)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser);
    assert_eq!(result, Ok(NumericValue::Number(f32::INFINITY)));
  }

  #[test]
  fn calc_const_neg_infinity() {
    let mut input = ParserInput::new("calc(-infinity)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser);
    assert_eq!(result, Ok(NumericValue::Number(f32::NEG_INFINITY)));
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
    assert_relative_eq!(value, 10.0_f32.ln());
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
    assert_relative_eq!(value, 2.0_f32.exp());
  }

  #[test]
  fn abs() {
    let mut input = ParserInput::new("abs(-3)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser);
    let Ok(NumericValue::Number(value)) = result else {
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
    let Ok(NumericValue::Number(value)) = result else {
      panic!("expect number: {:?}", result);
    };
    assert_eq!(value, -1.0);
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
    let Ok(NumericValue::Number(value)) = result else {
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
  Scale(f32, Option<f32>),
  ScaleX(f32),
  ScaleY(f32),
  ScaleZ(f32),
  Scale3d(f32, f32, f32),
  Rotate(Angle),
  RotateX(Angle),
  RotateY(Angle),
  RotateZ(Angle),
  Rotate3d(f32, f32, f32, Angle),
  Skew(Angle, Option<Angle>),
  SkewX(Angle),
  SkewY(Angle),
  Perspective(Option<Length>),
  Matrix([f32; 6]),
  Matrix3d([f32; 16]),
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
          let x = match x.expect_number_or_percent() {
            Ok(value) => value,
            Err(error) => return Err(arguments.new_custom_error(error)),
          };
          let y = if !arguments.is_exhausted() {
            arguments.expect_comma()?;
            let value = NumericValue::parse(arguments)?;
            let value = match value.expect_number_or_percent() {
              Ok(value) => value,
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
          let value = match value.expect_number_or_percent() {
            Ok(value) => value,
            Err(error) => return Err(arguments.new_custom_error(error)),
          };
          arguments.expect_exhausted()?;
          Ok(Transform::ScaleX(value))
        })
      },
      "scaley" => {
        input.parse_nested_block(|arguments| {
          let value = NumericValue::parse(arguments)?;
          let value = match value.expect_number_or_percent() {
            Ok(value) => value,
            Err(error) => return Err(arguments.new_custom_error(error)),
          };
          arguments.expect_exhausted()?;
          Ok(Transform::ScaleY(value))
        })
      },
      "scalez" => {
        input.parse_nested_block(|arguments| {
          let value = NumericValue::parse(arguments)?;
          let value = match value.expect_number_or_percent() {
            Ok(value) => value,
            Err(error) => return Err(arguments.new_custom_error(error)),
          };
          arguments.expect_exhausted()?;
          Ok(Transform::ScaleZ(value))
        })
      },
      "scale3d" => {
        input.parse_nested_block(|arguments| {
          let x = NumericValue::parse(arguments)?;
          let x = match x.expect_number_or_percent() {
            Ok(value) => value,
            Err(error) => return Err(arguments.new_custom_error(error)),
          };
          arguments.expect_comma()?;
          let y = NumericValue::parse(arguments)?;
          let y = match y.expect_number_or_percent() {
            Ok(value) => value,
            Err(error) => return Err(arguments.new_custom_error(error)),
          };
          arguments.expect_comma()?;
          let z = NumericValue::parse(arguments)?;
          let z = match z.expect_number_or_percent() {
            Ok(value) => value,
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
          let x = match x.expect_number() {
            Ok(number) => number,
            Err(error) => return Err(arguments.new_custom_error(error)),
          };
          arguments.expect_comma()?;
          let y = NumericValue::parse(arguments)?;
          let y = match y.expect_number() {
            Ok(number) => number,
            Err(error) => return Err(arguments.new_custom_error(error)),
          };
          arguments.expect_comma()?;
          let z = NumericValue::parse(arguments)?;
          let z = match z.expect_number() {
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
          let mut result = [0.0; 6];
          for (i, slot) in result.iter_mut().enumerate() {
            if i != 0 {
              arguments.expect_comma()?;
            }
            let value = NumericValue::parse(arguments)?;
            let number = match value.expect_number() {
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
          let mut result = [0.0; 16];
          for (i, slot) in result.iter_mut().enumerate() {
            if i != 0 {
              arguments.expect_comma()?;
            }
            let value = NumericValue::parse(arguments)?;
            let number = match value.expect_number() {
              Ok(number) => number,
              Err(error) => return Err(arguments.new_custom_error(error)),
            };
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
      return None;
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
