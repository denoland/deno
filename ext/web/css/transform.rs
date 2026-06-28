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

#[cfg(test)]
mod tests {
  use super::*;

  fn parse(input: &str) -> Option<Vec<Transform>> {
    let mut parser_input = ParserInput::new(input);
    let results: Result<Vec<_>, _> =
      TransformListParser::new(&mut parser_input).collect();
    results.ok()
  }

  fn px(v: f64) -> Length {
    Length::from_pixels(v)
  }

  // --- none / empty ---

  #[test]
  fn transform_none_keyword() {
    assert_eq!(parse("none"), Some(vec![]));
    assert_eq!(parse("NONE"), Some(vec![]));
    assert_eq!(parse("  none  "), Some(vec![]));
  }

  #[test]
  fn transform_empty_is_none() {
    assert_eq!(parse(""), None);
    assert_eq!(parse("   "), None);
  }

  #[test]
  fn transform_none_followed_by_function_rejected() {
    assert_eq!(parse("none rotate(0)"), None);
  }

  // --- translate ---

  #[test]
  fn transform_translate_x() {
    assert_eq!(
      parse("translate(10px)"),
      Some(vec![Transform::Translate(px(10.0), None)])
    );
  }

  #[test]
  fn transform_translate_xy() {
    assert_eq!(
      parse("translate(10px, 20px)"),
      Some(vec![Transform::Translate(px(10.0), Some(px(20.0)))])
    );
  }

  #[test]
  fn transform_translate_zero() {
    assert_eq!(
      parse("translate(0)"),
      Some(vec![Transform::Translate(px(0.0), None)])
    );
  }

  #[test]
  fn transform_translatex() {
    assert_eq!(
      parse("translateX(5px)"),
      Some(vec![Transform::TranslateX(px(5.0))])
    );
  }

  #[test]
  fn transform_translatey() {
    assert_eq!(
      parse("translateY(5px)"),
      Some(vec![Transform::TranslateY(px(5.0))])
    );
  }

  #[test]
  fn transform_translatez() {
    assert_eq!(
      parse("translateZ(5px)"),
      Some(vec![Transform::TranslateZ(px(5.0))])
    );
  }

  #[test]
  fn transform_translate3d() {
    assert_eq!(
      parse("translate3d(1px, 2px, 3px)"),
      Some(vec![Transform::Translate3d(px(1.0), px(2.0), px(3.0))])
    );
  }

  // --- scale ---

  #[test]
  fn transform_scale_uniform() {
    assert_eq!(parse("scale(2)"), Some(vec![Transform::Scale(2.0, None)]));
  }

  #[test]
  fn transform_scale_xy() {
    assert_eq!(
      parse("scale(2, 0.5)"),
      Some(vec![Transform::Scale(2.0, Some(0.5))])
    );
  }

  #[test]
  fn transform_scale_percent() {
    assert_eq!(parse("scale(50%)"), Some(vec![Transform::Scale(0.5, None)]));
  }

  #[test]
  fn transform_scalex() {
    assert_eq!(parse("scaleX(2)"), Some(vec![Transform::ScaleX(2.0)]));
  }

  #[test]
  fn transform_scaley() {
    assert_eq!(parse("scaleY(0.5)"), Some(vec![Transform::ScaleY(0.5)]));
  }

  #[test]
  fn transform_scalez() {
    assert_eq!(parse("scaleZ(3)"), Some(vec![Transform::ScaleZ(3.0)]));
  }

  #[test]
  fn transform_scale3d() {
    assert_eq!(
      parse("scale3d(1, 2, 3)"),
      Some(vec![Transform::Scale3d(1.0, 2.0, 3.0)])
    );
  }

  // --- rotate ---

  #[test]
  fn transform_rotate_deg() {
    let result = parse("rotate(90deg)");
    let Some(ref funcs) = result else {
      panic!("expected functions");
    };
    let Transform::Rotate(angle) = &funcs[0] else {
      panic!("expected Rotate");
    };
    assert!((angle.to_degrees() - 90.0).abs() < 1e-10);
  }

  #[test]
  fn transform_rotate_zero() {
    let result = parse("rotate(0)");
    let Some(ref funcs) = result else {
      panic!("expected functions");
    };
    let Transform::Rotate(angle) = &funcs[0] else {
      panic!("expected Rotate");
    };
    assert!((angle.to_degrees()).abs() < 1e-10);
  }

  #[test]
  fn transform_rotate_rad() {
    let result = parse("rotate(1rad)");
    let Some(ref funcs) = result else {
      panic!("expected functions");
    };
    let Transform::Rotate(angle) = &funcs[0] else {
      panic!("expected Rotate");
    };
    assert!((angle.to_degrees() - 1.0f64.to_degrees()).abs() < 1e-10);
  }

  #[test]
  fn transform_rotatex() {
    let result = parse("rotateX(45deg)");
    let Some(ref funcs) = result else {
      panic!("expected functions");
    };
    let Transform::RotateX(angle) = &funcs[0] else {
      panic!("expected RotateX");
    };
    assert!((angle.to_degrees() - 45.0).abs() < 1e-10);
  }

  #[test]
  fn transform_rotatey() {
    let result = parse("rotateY(45deg)");
    let Some(ref funcs) = result else {
      panic!("expected functions");
    };
    let Transform::RotateY(angle) = &funcs[0] else {
      panic!("expected RotateY");
    };
    assert!((angle.to_degrees() - 45.0).abs() < 1e-10);
  }

  #[test]
  fn transform_rotatez() {
    let result = parse("rotateZ(45deg)");
    let Some(ref funcs) = result else {
      panic!("expected functions");
    };
    let Transform::RotateZ(angle) = &funcs[0] else {
      panic!("expected RotateZ");
    };
    assert!((angle.to_degrees() - 45.0).abs() < 1e-10);
  }

  #[test]
  fn transform_rotate3d() {
    let result = parse("rotate3d(1, 0, 0, 90deg)");
    let Some(ref funcs) = result else {
      panic!("expected functions");
    };
    let Transform::Rotate3d(x, y, z, angle) = &funcs[0] else {
      panic!("expected Rotate3d");
    };
    assert_eq!(*x, 1.0);
    assert_eq!(*y, 0.0);
    assert_eq!(*z, 0.0);
    assert!((angle.to_degrees() - 90.0).abs() < 1e-10);
  }

  // --- skew ---

  #[test]
  fn transform_skew_x_only() {
    let result = parse("skew(30deg)");
    let Some(ref funcs) = result else {
      panic!("expected functions");
    };
    let Transform::Skew(x, y) = &funcs[0] else {
      panic!("expected Skew");
    };
    assert!((x.to_degrees() - 30.0).abs() < 1e-10);
    assert!(y.is_none());
  }

  #[test]
  fn transform_skew_xy() {
    let result = parse("skew(30deg, 15deg)");
    let Some(ref funcs) = result else {
      panic!("expected functions");
    };
    let Transform::Skew(x, y) = &funcs[0] else {
      panic!("expected Skew");
    };
    assert!((x.to_degrees() - 30.0).abs() < 1e-10);
    let y = y.as_ref().unwrap();
    assert!((y.to_degrees() - 15.0).abs() < 1e-10);
  }

  #[test]
  fn transform_skewx() {
    let result = parse("skewX(30deg)");
    let Some(ref funcs) = result else {
      panic!("expected functions");
    };
    let Transform::SkewX(angle) = &funcs[0] else {
      panic!("expected SkewX");
    };
    assert!((angle.to_degrees() - 30.0).abs() < 1e-10);
  }

  #[test]
  fn transform_skewy() {
    let result = parse("skewY(15deg)");
    let Some(ref funcs) = result else {
      panic!("expected functions");
    };
    let Transform::SkewY(angle) = &funcs[0] else {
      panic!("expected SkewY");
    };
    assert!((angle.to_degrees() - 15.0).abs() < 1e-10);
  }

  // --- perspective ---

  #[test]
  fn transform_perspective_length() {
    assert_eq!(
      parse("perspective(100px)"),
      Some(vec![Transform::Perspective(Some(px(100.0)))])
    );
  }

  #[test]
  fn transform_perspective_none() {
    assert_eq!(
      parse("perspective(none)"),
      Some(vec![Transform::Perspective(None)])
    );
  }

  // --- matrix ---

  #[test]
  fn transform_matrix() {
    assert_eq!(
      parse("matrix(1, 0, 0, 1, 10, 20)"),
      Some(vec![Transform::Matrix([1.0, 0.0, 0.0, 1.0, 10.0, 20.0])])
    );
  }

  #[test]
  fn transform_matrix3d() {
    let result = parse("matrix3d(1,0,0,0, 0,1,0,0, 0,0,1,0, 10,20,30,1)");
    assert_eq!(
      result,
      Some(vec![Transform::Matrix3d([
        1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 10.0, 20.0,
        30.0, 1.0,
      ])])
    );
  }

  // --- multiple transforms ---

  #[test]
  fn transform_multiple_functions() {
    let result = parse("translate(10px) scale(2)");
    let Some(ref funcs) = result else {
      panic!("expected functions");
    };
    assert_eq!(funcs.len(), 2);
    assert_eq!(funcs[0], Transform::Translate(px(10.0), None));
    assert_eq!(funcs[1], Transform::Scale(2.0, None));
  }

  #[test]
  fn transform_multiple_with_extra_spaces() {
    let result = parse("  translate( 10px )  scale( 2 )  ");
    let Some(ref funcs) = result else {
      panic!("expected functions");
    };
    assert_eq!(funcs.len(), 2);
    assert_eq!(funcs[0], Transform::Translate(px(10.0), None));
    assert_eq!(funcs[1], Transform::Scale(2.0, None));
  }

  // --- invalid inputs ---

  #[test]
  fn transform_unknown_function_rejected() {
    assert_eq!(parse("unknownfn(1)"), None);
  }

  #[test]
  fn transform_garbage_rejected() {
    assert_eq!(parse("!!!"), None);
  }

  #[test]
  fn transform_translate_no_unit_rejected() {
    assert_eq!(parse("translate(10)"), None);
  }

  #[test]
  fn transform_rotate_no_unit_rejected() {
    assert_eq!(parse("rotate(10)"), None);
  }

  #[test]
  fn transform_case_insensitive() {
    assert_eq!(parse("SCALE(2)"), Some(vec![Transform::Scale(2.0, None)]));
  }
}
