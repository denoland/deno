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
  UnsupportedUnit,
  ContainsRelativeValues,
  NumericTypeMismatch,
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
      return Err(CSSValueError::NumericTypeMismatch);
    }
    self.value += other.value;
    Ok(())
  }

  #[inline]
  fn try_sub_assign(&mut self, other: &MathValue) -> Result<(), CSSValueError> {
    if self.dimensions != other.dimensions {
      return Err(CSSValueError::NumericTypeMismatch);
    }
    self.value -= other.value;
    Ok(())
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
  fn into_math(self) -> MathValue {
    match self {
      NumericAccumulator::Numeric(numeric) => MathValue::from(numeric),
      NumericAccumulator::Math(math) => math,
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
        // Due to historical reasons, we need to allow the literal `0` for length and angle
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
          "cqw" | "cqh" | "cqi" | "cqb" | "cqmin" | "cqmax" => Err(input.new_custom_error(CSSValueError::ContainsRelativeValues)),
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
          "dpi" | "dpcm" | "dppx" | "x" => Err(input.new_custom_error(CSSValueError::UnsupportedUnit)),
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
    let Ok(NumericValue::Number(Number(value))) = result else { panic!("expect number") };
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
      == ParseErrorKind::Custom(CSSValueError::NumericTypeMismatch)));
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
}
