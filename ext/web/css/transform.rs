// Copyright 2018-2026 the Deno authors. MIT license.

use cssparser::Parser;
pub use cssparser::ParserInput;
use cssparser::match_ignore_ascii_case;

use super::error::CSSCustomError;
use super::error::CSSParseError;
use super::value::Angle;
use super::value::Length;
use super::value::NumericValue;
use super::value::ParseOptions;

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
  ) -> Result<Self, CSSParseError<'i>> {
    let name = input.expect_function()?;
    match_ignore_ascii_case! { &name,
      "translate" => {
        input.parse_nested_block(|arguments| {
          let x = NumericValue::parse(arguments, ParseOptions::default())?;
          let x = try_extract!(x, expect_length(true), arguments);
          let y = if !arguments.is_exhausted() {
            arguments.expect_comma()?;
            let value = NumericValue::parse(arguments, ParseOptions::default())?;
            let value = try_extract!(value, expect_length(true), arguments);
            arguments.expect_exhausted()?;
            Some(value)
          } else { None };
          Ok(Transform::Translate(x, y))
        })
      },
      "translatex" => {
        input.parse_nested_block(|arguments| {
          let value = NumericValue::parse(arguments, ParseOptions::default())?;
          let value = try_extract!(value, expect_length(true), arguments);
          arguments.expect_exhausted()?;
          Ok(Transform::TranslateX(value))
        })
      },
      "translatey" => {
        input.parse_nested_block(|arguments| {
          let value = NumericValue::parse(arguments, ParseOptions::default())?;
          let value = try_extract!(value, expect_length(true), arguments);
          arguments.expect_exhausted()?;
          Ok(Transform::TranslateY(value))
        })
      },
      "translatez" => {
        input.parse_nested_block(|arguments| {
          let value = NumericValue::parse(arguments, ParseOptions::default())?;
          let value = try_extract!(value, expect_length(true), arguments);
          arguments.expect_exhausted()?;
          Ok(Transform::TranslateZ(value))
        })
      },
      "translate3d" => {
        input.parse_nested_block(|arguments| {
          let x = NumericValue::parse(arguments, ParseOptions::default())?;
          let x = try_extract!(x, expect_length(true), arguments);
          arguments.expect_comma()?;
          let y = NumericValue::parse(arguments, ParseOptions::default())?;
          let y = try_extract!(y, expect_length(true), arguments);
          arguments.expect_comma()?;
          let z = NumericValue::parse(arguments, ParseOptions::default())?;
          let z = try_extract!(z, expect_length(true), arguments);
          arguments.expect_exhausted()?;
          Ok(Transform::Translate3d(x, y, z))
        })
      },
      "scale" => {
        input.parse_nested_block(|arguments| {
          let x = NumericValue::parse(arguments, ParseOptions::default())?;
          let x = try_extract!(x, expect_number_or_percent(), arguments);
          let y = if !arguments.is_exhausted() {
            arguments.expect_comma()?;
            let value = NumericValue::parse(arguments, ParseOptions::default())?;
            let value = try_extract!(value, expect_number_or_percent(), arguments);
            arguments.expect_exhausted()?;
            Some(value)
          } else { None };
          Ok(Transform::Scale(x, y))
        })
      },
      "scalex" => {
        input.parse_nested_block(|arguments| {
          let value = NumericValue::parse(arguments, ParseOptions::default())?;
          let value = try_extract!(value, expect_number_or_percent(), arguments);
          arguments.expect_exhausted()?;
          Ok(Transform::ScaleX(value))
        })
      },
      "scaley" => {
        input.parse_nested_block(|arguments| {
          let value = NumericValue::parse(arguments, ParseOptions::default())?;
          let value = try_extract!(value, expect_number_or_percent(), arguments);
          arguments.expect_exhausted()?;
          Ok(Transform::ScaleY(value))
        })
      },
      "scalez" => {
        input.parse_nested_block(|arguments| {
          let value = NumericValue::parse(arguments, ParseOptions::default())?;
          let value = try_extract!(value, expect_number_or_percent(), arguments);
          arguments.expect_exhausted()?;
          Ok(Transform::ScaleZ(value))
        })
      },
      "scale3d" => {
        input.parse_nested_block(|arguments| {
          let x = NumericValue::parse(arguments, ParseOptions::default())?;
          let x = try_extract!(x, expect_number_or_percent(), arguments);
          arguments.expect_comma()?;
          let y = NumericValue::parse(arguments, ParseOptions::default())?;
          let y = try_extract!(y, expect_number_or_percent(), arguments);
          arguments.expect_comma()?;
          let z = NumericValue::parse(arguments, ParseOptions::default())?;
          let z = try_extract!(z, expect_number_or_percent(), arguments);
          arguments.expect_exhausted()?;
          Ok(Transform::Scale3d(x, y, z))
        })
      },
      "rotate" => {
        input.parse_nested_block(|arguments| {
          let value = NumericValue::parse(arguments, ParseOptions::default())?;
          let value = try_extract!(value, expect_angle(true), arguments);
          arguments.expect_exhausted()?;
          Ok(Transform::Rotate(value))
        })
      },
      "rotatex" => {
        input.parse_nested_block(|arguments| {
          let value = NumericValue::parse(arguments, ParseOptions::default())?;
          let value = try_extract!(value, expect_angle(true), arguments);
          arguments.expect_exhausted()?;
          Ok(Transform::RotateX(value))
        })
      },
      "rotatey" => {
        input.parse_nested_block(|arguments| {
          let value = NumericValue::parse(arguments, ParseOptions::default())?;
          let value = try_extract!(value, expect_angle(true), arguments);
          arguments.expect_exhausted()?;
          Ok(Transform::RotateY(value))
        })
      },
      "rotatez" => {
        input.parse_nested_block(|arguments| {
          let value = NumericValue::parse(arguments, ParseOptions::default())?;
          let value = try_extract!(value, expect_angle(true), arguments);
          arguments.expect_exhausted()?;
          Ok(Transform::RotateZ(value))
        })
      },
      "rotate3d" => {
        input.parse_nested_block(|arguments| {
          let x = NumericValue::parse(arguments, ParseOptions::default())?;
          let x = try_extract!(x, expect_number(), arguments);
          arguments.expect_comma()?;
          let y = NumericValue::parse(arguments, ParseOptions::default())?;
          let y = try_extract!(y, expect_number(), arguments);
          arguments.expect_comma()?;
          let z = NumericValue::parse(arguments, ParseOptions::default())?;
          let z = try_extract!(z, expect_number(), arguments);
          arguments.expect_comma()?;
          let a = NumericValue::parse(arguments, ParseOptions::default())?;
          let a = try_extract!(a, expect_angle(true), arguments);
          arguments.expect_exhausted()?;
          Ok(Transform::Rotate3d(x, y, z, a))
        })
      },
      "skew" => {
        input.parse_nested_block(|arguments| {
          let x = NumericValue::parse(arguments, ParseOptions::default())?;
          let x = try_extract!(x, expect_angle(true), arguments);
          let y = if !arguments.is_exhausted() {
            arguments.expect_comma()?;
            let value = NumericValue::parse(arguments, ParseOptions::default())?;
            let value = try_extract!(value, expect_angle(true), arguments);
            arguments.expect_exhausted()?;
            Some(value)
          } else { None };
          Ok(Transform::Skew(x, y))
        })
      },
      "skewx" => {
        input.parse_nested_block(|arguments| {
          let value = NumericValue::parse(arguments, ParseOptions::default())?;
          let value = try_extract!(value, expect_angle(true), arguments);
          arguments.expect_exhausted()?;
          Ok(Transform::SkewX(value))
        })
      },
      "skewy" => {
        input.parse_nested_block(|arguments| {
          let value = NumericValue::parse(arguments, ParseOptions::default())?;
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
              let value = NumericValue::parse(arguments, ParseOptions::default())?;
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
            let value = NumericValue::parse(arguments, ParseOptions::default())?;
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
            let value = NumericValue::parse(arguments, ParseOptions::default())?;
            let number = try_extract!(value, expect_number(), arguments);
            *slot = number;
          }
          arguments.expect_exhausted()?;
          Ok(Transform::Matrix3d(result))
        })
      },
      _ => {
        let name = name.to_string();
        Err(input.new_custom_error(CSSCustomError::InvalidFunction(name)))
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
  type Item = Result<Transform, CSSParseError<'i>>;

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
