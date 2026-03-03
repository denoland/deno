// Copyright 2018-2026 the Deno authors. MIT license.

use std::f32;
use std::fmt;
use std::ops;

use cssparser::ParseError;
use cssparser::Parser;
use cssparser::Token;
use cssparser::match_ignore_ascii_case;

#[derive(Debug, Eq, PartialEq)]
pub enum CSSValueError {
  UnexpectedToken,
  ContainsRelativeValues,
  UnsupportedDimension,
  DimensionMismatch,
  InvalidDimensions,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Number(pub f32);

#[derive(Debug, PartialEq)]
pub struct Length {
  value: f32,
  unit: LengthUnit,
}

// Currently, only Absolute Length Units are supported.
#[derive(Clone, Debug, Eq, PartialEq)]
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

#[derive(Debug, PartialEq)]
pub struct Angle {
  value: f32,
  unit: AngleUnit,
}

#[derive(Clone, Debug, Eq, PartialEq)]
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

#[derive(PartialEq)]
pub struct Percent(pub f32);

impl fmt::Debug for Percent {
  fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
    fmt
      .debug_tuple("Percent")
      .field(&format_args!("{}%", self.0 * 100.0))
      .finish()
  }
}

// Currently, units for time, frequency, and resolution are not supported
#[derive(Debug, PartialEq)]
pub enum NumericValue {
  Zero,
  Number(Number),
  Length(Length),
  Angle(Angle),
  Percent(Percent),
}

impl NumericValue {
  #[inline]
  fn expect_number(self) -> Result<Number, CSSValueError> {
    match self {
      NumericValue::Zero => Ok(Number(0.0)),
      NumericValue::Number(number) => Ok(number),
      _ => Err(CSSValueError::DimensionMismatch),
    }
  }

  #[inline]
  fn expect_length(self, allow_zero: bool) -> Result<Length, CSSValueError> {
    match self {
      NumericValue::Zero => {
        if allow_zero {
          Ok(Length {
            value: 0.0,
            unit: LengthUnit::Px,
          })
        } else {
          Err(CSSValueError::DimensionMismatch)
        }
      }
      NumericValue::Length(length) => Ok(length),
      _ => Err(CSSValueError::DimensionMismatch),
    }
  }

  #[inline]
  fn expect_angle(self, allow_zero: bool) -> Result<Angle, CSSValueError> {
    match self {
      NumericValue::Zero => {
        if allow_zero {
          Ok(Angle {
            value: 0.0,
            unit: AngleUnit::Deg,
          })
        } else {
          Err(CSSValueError::DimensionMismatch)
        }
      }
      NumericValue::Angle(angle) => Ok(angle),
      _ => Err(CSSValueError::DimensionMismatch),
    }
  }

  #[inline]
  fn expect_percent(self) -> Result<Percent, CSSValueError> {
    match self {
      NumericValue::Percent(percent) => Ok(percent),
      _ => Err(CSSValueError::DimensionMismatch),
    }
  }
}

// Currently, units for time, frequency, and resolution are not supported
#[derive(Debug, Default, PartialEq)]
struct Dimensions {
  length: i8,
  angle: i8,
  percent: i8,
}

impl ops::AddAssign<&Dimensions> for Dimensions {
  fn add_assign(&mut self, rhs: &Self) {
    self.length += rhs.length;
    self.angle += rhs.angle;
    self.percent += rhs.percent;
  }
}

impl ops::SubAssign<&Dimensions> for Dimensions {
  fn sub_assign(&mut self, rhs: &Self) {
    self.length -= rhs.length;
    self.angle -= rhs.angle;
    self.percent -= rhs.percent;
  }
}

#[derive(Debug, PartialEq)]
struct MathValue {
  value: f32,
  dimensions: Dimensions,
}

impl From<NumericValue> for MathValue {
  fn from(value: NumericValue) -> Self {
    match value {
      NumericValue::Zero => MathValue {
        value: 0.0,
        dimensions: Default::default(),
      },
      NumericValue::Number(Number(value)) => MathValue {
        value,
        dimensions: Default::default(),
      },
      NumericValue::Length(length) => {
        let value = length.to_pixels();
        MathValue {
          value,
          dimensions: Dimensions {
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
          dimensions: Dimensions {
            length: 0,
            angle: 1,
            percent: 0,
          },
        }
      }
      NumericValue::Percent(Percent(value)) => MathValue {
        value,
        dimensions: Dimensions {
          length: 0,
          angle: 0,
          percent: 1,
        },
      },
    }
  }
}

impl TryFrom<MathValue> for NumericValue {
  type Error = CSSValueError;

  fn try_from(accumulator: MathValue) -> Result<Self, Self::Error> {
    let value = accumulator.value;
    match accumulator.dimensions {
      Dimensions {
        length: 0,
        angle: 0,
        percent: 0,
      } => Ok(NumericValue::Number(Number(value))),
      Dimensions {
        length: 1,
        angle: 0,
        percent: 0,
      } => Ok(NumericValue::Length(Length {
        value,
        unit: LengthUnit::Px,
      })),
      Dimensions {
        length: 0,
        angle: 1,
        percent: 0,
      } => Ok(NumericValue::Angle(Angle {
        value,
        unit: AngleUnit::Deg,
      })),
      Dimensions {
        length: 0,
        angle: 0,
        percent: 1,
      } => Ok(NumericValue::Percent(Percent(value))),
      _ => Err(CSSValueError::InvalidDimensions),
    }
  }
}

impl MathValue {
  #[inline]
  fn try_add_assign(&mut self, other: &MathValue) -> Result<(), CSSValueError> {
    if self.dimensions != other.dimensions {
      return Err(CSSValueError::DimensionMismatch);
    }
    self.value += other.value;
    Ok(())
  }

  #[inline]
  fn try_sub_assign(&mut self, other: &MathValue) -> Result<(), CSSValueError> {
    if self.dimensions != other.dimensions {
      return Err(CSSValueError::DimensionMismatch);
    }
    self.value -= other.value;
    Ok(())
  }

  fn expect_number(self) -> Result<Number, CSSValueError> {
    if self.dimensions != Dimensions::default() {
      return Err(CSSValueError::DimensionMismatch);
    }
    Ok(Number(self.value))
  }

  fn expect_length(self) -> Result<Length, CSSValueError> {
    let dimensions = Dimensions {
      length: 1,
      angle: 0,
      percent: 0,
    };
    if self.dimensions != dimensions {
      return Err(CSSValueError::DimensionMismatch);
    }
    Ok(Length {
      value: self.value,
      unit: LengthUnit::Px,
    })
  }

  fn expect_angle(self) -> Result<Angle, CSSValueError> {
    let dimensions = Dimensions {
      length: 0,
      angle: 1,
      percent: 0,
    };
    if self.dimensions != dimensions {
      return Err(CSSValueError::DimensionMismatch);
    }
    Ok(Angle {
      value: self.value,
      unit: AngleUnit::Deg,
    })
  }

  fn expect_percent(self) -> Result<Percent, CSSValueError> {
    let dimensions = Dimensions {
      length: 0,
      angle: 0,
      percent: 1,
    };
    if self.dimensions != dimensions {
      return Err(CSSValueError::DimensionMismatch);
    }
    Ok(Percent(self.value))
  }
}

impl ops::MulAssign<&MathValue> for MathValue {
  #[inline]
  fn mul_assign(&mut self, other: &MathValue) {
    self.value *= other.value;
    self.dimensions += &other.dimensions;
  }
}

impl ops::DivAssign<&MathValue> for MathValue {
  #[inline]
  fn div_assign(&mut self, other: &MathValue) {
    self.value /= other.value;
    self.dimensions -= &other.dimensions;
  }
}

#[derive(Debug, PartialEq)]
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
  fn expect_numeric(self) -> Result<NumericValue, CSSValueError> {
    match self {
      NumericAccumulator::Numeric(numeric) => Ok(numeric),
      NumericAccumulator::Math(math) => NumericValue::try_from(math),
    }
  }

  #[inline]
  fn expect_number(self) -> Result<Number, CSSValueError> {
    match self {
      NumericAccumulator::Numeric(numeric) => numeric.expect_number(),
      NumericAccumulator::Math(math) => math.expect_number(),
    }
  }

  #[inline]
  fn expect_length(self, allow_zero: bool) -> Result<Length, CSSValueError> {
    match self {
      NumericAccumulator::Numeric(numeric) => numeric.expect_length(allow_zero),
      NumericAccumulator::Math(math) => math.expect_length(),
    }
  }

  #[inline]
  fn expect_angle(self, allow_zero: bool) -> Result<Angle, CSSValueError> {
    match self {
      NumericAccumulator::Numeric(numeric) => numeric.expect_angle(allow_zero),
      NumericAccumulator::Math(math) => math.expect_angle(),
    }
  }

  #[inline]
  fn expect_percent(self) -> Result<Percent, CSSValueError> {
    match self {
      NumericAccumulator::Numeric(numeric) => numeric.expect_percent(),
      NumericAccumulator::Math(math) => math.expect_percent(),
    }
  }
}

#[derive(Debug, Default)]
struct ParseState {
  function_depth: u8,
}

impl NumericValue {
  pub fn parse<'i, 't>(
    input: &mut Parser<'i, 't>,
  ) -> Result<Self, ParseError<'i, CSSValueError>> {
    let result = Self::parse_inner(input, &mut ParseState::default())?;
    match result {
      NumericAccumulator::Numeric(numeric) => Ok(numeric),
      NumericAccumulator::Math(math) => match NumericValue::try_from(math) {
        Ok(numeric) => Ok(numeric),
        Err(error) => Err(input.new_custom_error(error)),
      },
    }
  }

  fn parse_inner<'i, 't>(
    input: &mut Parser<'i, 't>,
    state: &mut ParseState,
  ) -> Result<NumericAccumulator, ParseError<'i, CSSValueError>> {
    let token = input.next()?;
    match token {
      Token::Number { value, .. } => {
        // Due to historical reasons, <transform-function> must allow the literal `0` for length and angle
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
          "cqw" | "cqh" | "cqi" | "cqb" | "cqmin" | "cqmax" |
          // https://www.w3.org/TR/css-grid-2/#fr-unit
          "fr"
          => Err(input.new_custom_error(CSSValueError::ContainsRelativeValues)),
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
          "dpi" | "dpcm" | "dppx" | "x" => Err(input.new_custom_error(CSSValueError::UnsupportedDimension)),
          _ => Err(input.new_custom_error(CSSValueError::UnexpectedToken))
        }
      }
      Token::Percentage { unit_value, .. } => {
        Ok(NumericValue::Percent(Percent(*unit_value)).into())
      }
      Token::Function(name) => {
        state.function_depth += 1;
        let result = match_ignore_ascii_case! { &name,
          "calc" => {
            input.parse_nested_block(|arguments| {
              let value = Self::parse_additive_expression(arguments, state)?;
              arguments.expect_exhausted()?;
              Ok(value)
            })
          },
          "min" => {
            input.parse_nested_block(|arguments| {
              let value = Self::parse_additive_expression(arguments, state)?;
              let numeric = match value.expect_numeric() {
                Ok(numeric) => numeric,
                Err(error) => return Err(arguments.new_custom_error(error)),
              };
              let result: NumericAccumulator = match numeric {
                NumericValue::Number(number) => {
                  let mut current = number.0;
                  while !arguments.is_exhausted() {
                    arguments.expect_comma()?;
                    let value = Self::parse_additive_expression(arguments, state)?;
                    let value = match value.expect_number() {
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
                  let mut current = percent.0;
                  while !arguments.is_exhausted() {
                    arguments.expect_comma()?;
                    let value = Self::parse_additive_expression(arguments, state)?;
                    let value = match value.expect_percent() {
                      Ok(length) => length.0,
                      Err(error) => return Err(arguments.new_custom_error(error)),
                    };
                    current = minimum(current, value);
                  }
                  NumericValue::Percent(Percent(current)).into()
                },
                _ => unreachable!()
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
                NumericValue::Number(number) => {
                  let mut current = number.0;
                  while !arguments.is_exhausted() {
                    arguments.expect_comma()?;
                    let value = Self::parse_additive_expression(arguments, state)?;
                    let value = match value.expect_number() {
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
                  let mut current = percent.0;
                  while !arguments.is_exhausted() {
                    arguments.expect_comma()?;
                    let value = Self::parse_additive_expression(arguments, state)?;
                    let value = match value.expect_percent() {
                      Ok(length) => length.0,
                      Err(error) => return Err(arguments.new_custom_error(error)),
                    };
                    current = maximum(current, value);
                  }
                  NumericValue::Percent(Percent(current)).into()
                },
                _ => unreachable!()
              };
              Ok(result)
            })
          },
          "clamp" => {
            input.parse_nested_block(|arguments| {
              let min: Option<NumericValue> = {
                let start = arguments.state();
                let token = arguments.next()?;
                if let Token::Ident(ident) = token {
                  match_ignore_ascii_case! { &ident,
                    "none" => None,
                    _ => return Err(arguments.new_custom_error(CSSValueError::UnexpectedToken)),
                  }
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
                let token = arguments.next()?;
                if let Token::Ident(ident) = token {
                  match_ignore_ascii_case! { &ident,
                    "none" => None,
                    _ => return Err(arguments.new_custom_error(CSSValueError::UnexpectedToken)),
                  }
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
                NumericValue::Number(Number(value)) => {
                  let min = match min {
                    Some(numeric) => {
                      match numeric.expect_number() {
                        Ok(numeric) => numeric.0,
                        Err(error) => return Err(arguments.new_custom_error(error)),
                      }
                    },
                    None => f32::NEG_INFINITY,
                  };
                  let max = match max {
                    Some(numeric) => {
                      match numeric.expect_number() {
                        Ok(numeric) => numeric.0,
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
                        Ok(percent) => percent.0,
                        Err(error) => return Err(arguments.new_custom_error(error)),
                      }
                    },
                    None => f32::NEG_INFINITY,
                  };
                  let max = match max {
                    Some(numeric) => {
                      match numeric.expect_percent() {
                        Ok(percent) => percent.0,
                        Err(error) => return Err(arguments.new_custom_error(error)),
                      }
                    },
                    None => f32::INFINITY,
                  };
                  NumericValue::Percent(Percent(maximum(min, minimum(value, max)))).into()
                },
                _ => unreachable!()
              };
              Ok(result)
            })
          },
          _ => todo!("parse_leaf: function")
        };
        state.function_depth -= 1;
        result
      }
      Token::ParenthesisBlock => {
        if state.function_depth == 0 {
          return Err(input.new_custom_error(CSSValueError::UnexpectedToken));
        }
        input.parse_nested_block(|arguments| {
          let value = Self::parse_additive_expression(arguments, state)?;
          arguments.expect_exhausted()?;
          Ok(value)
        })
      }
      Token::Ident(ident) => {
        if state.function_depth == 0 {
          return Err(input.new_custom_error(CSSValueError::UnexpectedToken));
        }
        match_ignore_ascii_case! { &ident,
          // https://www.w3.org/TR/css-values-4/#calc-constants
          "e" => Ok(NumericValue::Number(Number(f32::consts::E)).into()),
          "pi" => Ok(NumericValue::Number(Number(f32::consts::PI)).into()),
          // https://www.w3.org/TR/css-values-4/#calc-error-constants
          "infinity" => Ok(NumericValue::Number(Number(f32::INFINITY)).into()),
          "-infinity" => Ok(NumericValue::Number(Number(f32::NEG_INFINITY)).into()),
          "nan" => Ok(NumericValue::Number(Number(f32::NAN)).into()),
          _ => Err(input.new_custom_error(CSSValueError::UnexpectedToken))
        }
      }
      _ => Err(input.new_custom_error(CSSValueError::UnexpectedToken)),
    }
  }

  fn parse_additive_expression<'i, 't>(
    input: &mut Parser<'i, 't>,
    state: &mut ParseState,
  ) -> Result<NumericAccumulator, ParseError<'i, CSSValueError>> {
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
  ) -> Result<NumericAccumulator, ParseError<'i, CSSValueError>> {
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
  use cssparser::BasicParseErrorKind;
  use cssparser::ParseErrorKind;
  use cssparser::ParserInput;

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
    let mut input = ParserInput::new("3.14");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser);
    assert_eq!(result, Ok(NumericValue::Number(Number(3.14))));
  }

  #[test]
  fn length() {
    let mut input = ParserInput::new("-1px");
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
  fn angle() {
    let mut input = ParserInput::new("180deg");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser);
    assert_eq!(
      result,
      Ok(NumericValue::Angle(Angle {
        value: 180.0,
        unit: AngleUnit::Deg,
      }))
    );
  }

  #[test]
  fn percent() {
    let mut input = ParserInput::new("10%");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser);
    assert_eq!(result, Ok(NumericValue::Percent(Percent(0.1))));
  }

  #[test]
  fn calc_single_term() {
    let mut input = ParserInput::new("calc(1px)");
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
      panic!("expect number")
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
      == ParseErrorKind::Custom(CSSValueError::DimensionMismatch)));
  }

  #[test]
  fn calc_dimensions() {
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
  fn calc_failed_by_dimensions() {
    let mut input = ParserInput::new("calc(1px * 1deg * 1% / 1deg)");
    let mut parser = Parser::new(&mut input);
    let result = NumericValue::parse(&mut parser);
    assert!(result.is_err_and(|error| error.kind
      == ParseErrorKind::Custom(CSSValueError::InvalidDimensions)));
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
      panic!("expect number")
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
      panic!("expect number")
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
      panic!("expect number")
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
}
