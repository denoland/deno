// Copyright 2018-2026 the Deno authors. MIT license.

use cssparser::Parser;
pub use cssparser::ParserInput;
use cssparser::match_ignore_ascii_case;

use super::color::Color;
use super::color::parse_css_color;
use super::error::CSSCustomError;
use super::error::CSSParseError;
use super::value::Angle;
use super::value::Length;
use super::value::NumericValue;
use super::value::ParseOptions;

/// A parsed CSS filter function.
/// https://www.w3.org/TR/filter-effects-1/#filter-functions
#[derive(Clone, Debug, PartialEq)]
pub enum CssFilterFunction {
  Blur(Length),
  Brightness(f64),
  Contrast(f64),
  DropShadow {
    offset_x: Length,
    offset_y: Length,
    blur_radius: Length,
    color: Color,
  },
  Grayscale(f64),
  HueRotate(Angle),
  Invert(f64),
  Opacity(f64),
  Saturate(f64),
  Sepia(f64),
}

impl CssFilterFunction {
  #[inline]
  fn parse<'i, 't>(
    input: &mut Parser<'i, 't>,
  ) -> Result<Self, CSSParseError<'i>> {
    let name = input.expect_function()?;
    match_ignore_ascii_case! { &name,
      "blur" => {
        input.parse_nested_block(|args| {
          if args.is_exhausted() {
            return Ok(CssFilterFunction::Blur(Length::zero()));
          }
          let value = NumericValue::parse(args, ParseOptions::default())?;
          let length = try_extract!(value, expect_length(true), args);
          if length.to_pixels() < 0.0 {
            return Err(args.new_custom_error(CSSCustomError::UnexpectedNumericType));
          }
          args.expect_exhausted()?;
          Ok(CssFilterFunction::Blur(length))
        })
      },
      "brightness" => {
        input.parse_nested_block(|args| {
          if args.is_exhausted() {
            return Ok(CssFilterFunction::Brightness(1.0));
          }
          let value = NumericValue::parse(args, ParseOptions::default())?;
          let v = try_extract!(value, expect_number_or_percent(), args);
          args.expect_exhausted()?;
          Ok(CssFilterFunction::Brightness(v))
        })
      },
      "contrast" => {
        input.parse_nested_block(|args| {
          if args.is_exhausted() {
            return Ok(CssFilterFunction::Contrast(1.0));
          }
          let value = NumericValue::parse(args, ParseOptions::default())?;
          let v = try_extract!(value, expect_number_or_percent(), args);
          args.expect_exhausted()?;
          Ok(CssFilterFunction::Contrast(v))
        })
      },
      "drop-shadow" => {
        input.parse_nested_block(parse_drop_shadow)
      },
      "grayscale" => {
        input.parse_nested_block(|args| {
          if args.is_exhausted() {
            return Ok(CssFilterFunction::Grayscale(1.0));
          }
          let value = NumericValue::parse(args, ParseOptions::default())?;
          let v = try_extract!(value, expect_number_or_percent(), args);
          args.expect_exhausted()?;
          Ok(CssFilterFunction::Grayscale(v))
        })
      },
      "hue-rotate" => {
        input.parse_nested_block(|args| {
          if args.is_exhausted() {
            return Ok(CssFilterFunction::HueRotate(Angle::zero()));
          }
          let value = NumericValue::parse(args, ParseOptions::default())?;
          let angle = try_extract!(value, expect_angle(true), args);
          args.expect_exhausted()?;
          Ok(CssFilterFunction::HueRotate(angle))
        })
      },
      "invert" => {
        input.parse_nested_block(|args| {
          if args.is_exhausted() {
            return Ok(CssFilterFunction::Invert(1.0));
          }
          let value = NumericValue::parse(args, ParseOptions::default())?;
          let v = try_extract!(value, expect_number_or_percent(), args);
          args.expect_exhausted()?;
          Ok(CssFilterFunction::Invert(v))
        })
      },
      "opacity" => {
        input.parse_nested_block(|args| {
          if args.is_exhausted() {
            return Ok(CssFilterFunction::Opacity(1.0));
          }
          let value = NumericValue::parse(args, ParseOptions::default())?;
          let v = try_extract!(value, expect_number_or_percent(), args);
          args.expect_exhausted()?;
          Ok(CssFilterFunction::Opacity(v))
        })
      },
      "saturate" => {
        input.parse_nested_block(|args| {
          if args.is_exhausted() {
            return Ok(CssFilterFunction::Saturate(1.0));
          }
          let value = NumericValue::parse(args, ParseOptions::default())?;
          let v = try_extract!(value, expect_number_or_percent(), args);
          args.expect_exhausted()?;
          Ok(CssFilterFunction::Saturate(v))
        })
      },
      "sepia" => {
        input.parse_nested_block(|args| {
          if args.is_exhausted() {
            return Ok(CssFilterFunction::Sepia(1.0));
          }
          let value = NumericValue::parse(args, ParseOptions::default())?;
          let v = try_extract!(value, expect_number_or_percent(), args);
          args.expect_exhausted()?;
          Ok(CssFilterFunction::Sepia(v))
        })
      },
      _ => {
        let name = name.to_string();
        Err(input.new_custom_error(CSSCustomError::InvalidFunction(name)))
      },
    }
  }
}

#[inline]
fn parse_drop_shadow<'i, 't>(
  args: &mut Parser<'i, 't>,
) -> Result<CssFilterFunction, CSSParseError<'i>> {
  let offset_x = NumericValue::parse(args, ParseOptions::default())?;
  let offset_x = try_extract!(offset_x, expect_length(true), args);

  let offset_y = NumericValue::parse(args, ParseOptions::default())?;
  let offset_y = try_extract!(offset_y, expect_length(true), args);

  let mut blur_radius = Length::zero();
  let mut color = Color::BLACK;

  if !args.is_exhausted() {
    let state = args.state();
    if let Ok(value) = NumericValue::parse(args, ParseOptions::default()) {
      match value.expect_length(true) {
        Ok(length) => {
          if length.to_pixels() < 0.0 {
            return Err(
              args.new_custom_error(CSSCustomError::UnexpectedNumericType),
            );
          }
          blur_radius = length;
        }
        Err(_) => {
          args.reset(&state);
        }
      }
    } else {
      args.reset(&state);
    }
  }

  if !args.is_exhausted() {
    let start = args.position();
    while args.next().is_ok() {}
    let color_str = args.slice_from(start);
    color = parse_css_color(color_str).map_err(|e| args.new_custom_error(e))?;
  }

  Ok(CssFilterFunction::DropShadow {
    offset_x,
    offset_y,
    blur_radius,
    color,
  })
}

pub struct FilterValueListParser<'i, 't> {
  parser: Parser<'i, 't>,
  has_function: bool,
  finished: bool,
}

impl<'i: 't, 't> FilterValueListParser<'i, 't> {
  #[inline]
  pub fn new(input: &'t mut ParserInput<'i>) -> Self {
    Self {
      parser: Parser::new(input),
      has_function: false,
      finished: false,
    }
  }
}

impl<'i, 't> Iterator for FilterValueListParser<'i, 't> {
  type Item = Result<CssFilterFunction, CSSParseError<'i>>;

  #[inline]
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

    let result = CssFilterFunction::parse(input);
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

  fn parse(input: &str) -> Option<Vec<CssFilterFunction>> {
    let mut parser_input = ParserInput::new(input);
    let results: Result<Vec<_>, _> =
      FilterValueListParser::new(&mut parser_input).collect();
    results.ok()
  }

  fn px(v: f64) -> Length {
    Length::from_pixels(v)
  }

  // --- none / empty ---

  #[test]
  fn filter_none_keyword() {
    assert_eq!(parse("none"), Some(vec![]));
    assert_eq!(parse("NONE"), Some(vec![]));
    assert_eq!(parse("  none  "), Some(vec![]));
  }

  #[test]
  fn filter_empty_is_none() {
    assert_eq!(parse(""), None);
    assert_eq!(parse("   "), None);
  }

  // --- blur ---

  #[test]
  fn filter_blur_default() {
    assert_eq!(
      parse("blur()"),
      Some(vec![CssFilterFunction::Blur(px(0.0))])
    );
  }

  #[test]
  fn filter_blur_px() {
    assert_eq!(
      parse("blur(5px)"),
      Some(vec![CssFilterFunction::Blur(px(5.0))])
    );
  }

  #[test]
  fn filter_blur_zero() {
    assert_eq!(
      parse("blur(0)"),
      Some(vec![CssFilterFunction::Blur(px(0.0))])
    );
  }

  #[test]
  fn filter_blur_negative_rejected() {
    assert_eq!(parse("blur(-1px)"), None);
  }

  #[test]
  fn filter_blur_non_px_rejected() {
    assert_eq!(parse("blur(5em)"), None);
  }

  // --- brightness / contrast / grayscale / invert / opacity / saturate / sepia ---

  #[test]
  fn filter_brightness_number() {
    assert_eq!(
      parse("brightness(0.5)"),
      Some(vec![CssFilterFunction::Brightness(0.5)])
    );
  }

  #[test]
  fn filter_brightness_percent() {
    assert_eq!(
      parse("brightness(50%)"),
      Some(vec![CssFilterFunction::Brightness(0.5)])
    );
  }

  #[test]
  fn filter_brightness_default() {
    assert_eq!(
      parse("brightness()"),
      Some(vec![CssFilterFunction::Brightness(1.0)])
    );
  }

  #[test]
  fn filter_contrast_default() {
    assert_eq!(
      parse("contrast()"),
      Some(vec![CssFilterFunction::Contrast(1.0)])
    );
  }

  #[test]
  fn filter_grayscale_default() {
    assert_eq!(
      parse("grayscale()"),
      Some(vec![CssFilterFunction::Grayscale(1.0)])
    );
  }

  #[test]
  fn filter_invert_default() {
    assert_eq!(
      parse("invert()"),
      Some(vec![CssFilterFunction::Invert(1.0)])
    );
  }

  #[test]
  fn filter_opacity_default() {
    assert_eq!(
      parse("opacity()"),
      Some(vec![CssFilterFunction::Opacity(1.0)])
    );
  }

  #[test]
  fn filter_saturate_default() {
    assert_eq!(
      parse("saturate()"),
      Some(vec![CssFilterFunction::Saturate(1.0)])
    );
  }

  #[test]
  fn filter_sepia_default() {
    assert_eq!(parse("sepia()"), Some(vec![CssFilterFunction::Sepia(1.0)]));
  }

  // --- hue-rotate ---

  #[test]
  fn filter_hue_rotate_deg() {
    let result = parse("hue-rotate(90deg)");
    let Some(funcs) = result else {
      panic!("expected functions");
    };
    let CssFilterFunction::HueRotate(angle) = &funcs[0] else {
      panic!("expected HueRotate");
    };
    assert!((angle.to_degrees() - 90.0).abs() < 1e-10);
  }

  #[test]
  fn filter_hue_rotate_rad() {
    let result = parse("hue-rotate(1rad)");
    let Some(funcs) = result else {
      panic!("expected functions");
    };
    let CssFilterFunction::HueRotate(angle) = &funcs[0] else {
      panic!("expected HueRotate");
    };
    assert!((angle.to_degrees() - 1.0f64.to_degrees()).abs() < 1e-10);
  }

  #[test]
  fn filter_hue_rotate_grad() {
    let result = parse("hue-rotate(400grad)");
    let Some(funcs) = result else {
      panic!("expected functions");
    };
    let CssFilterFunction::HueRotate(angle) = &funcs[0] else {
      panic!("expected HueRotate");
    };
    assert!((angle.to_degrees() - 360.0).abs() < 1e-10);
  }

  #[test]
  fn filter_hue_rotate_turn() {
    let result = parse("hue-rotate(0.5turn)");
    let Some(funcs) = result else {
      panic!("expected functions");
    };
    let CssFilterFunction::HueRotate(angle) = &funcs[0] else {
      panic!("expected HueRotate");
    };
    assert!((angle.to_degrees() - 180.0).abs() < 1e-10);
  }

  #[test]
  fn filter_hue_rotate_zero() {
    let result = parse("hue-rotate(0)");
    let Some(funcs) = result else {
      panic!("expected functions");
    };
    let CssFilterFunction::HueRotate(angle) = &funcs[0] else {
      panic!("expected HueRotate");
    };
    assert!((angle.to_degrees()).abs() < 1e-10);
  }

  #[test]
  fn filter_hue_rotate_default() {
    let result = parse("hue-rotate()");
    let Some(funcs) = result else {
      panic!("expected functions");
    };
    let CssFilterFunction::HueRotate(angle) = &funcs[0] else {
      panic!("expected HueRotate");
    };
    assert!((angle.to_degrees()).abs() < 1e-10);
  }

  // --- drop-shadow ---

  #[test]
  fn filter_drop_shadow_offsets_only() {
    assert_eq!(
      parse("drop-shadow(2px 4px)"),
      Some(vec![CssFilterFunction::DropShadow {
        offset_x: px(2.0),
        offset_y: px(4.0),
        blur_radius: px(0.0),
        color: Color::BLACK,
      }])
    );
  }

  #[test]
  fn filter_drop_shadow_with_blur() {
    assert_eq!(
      parse("drop-shadow(2px 4px 3px)"),
      Some(vec![CssFilterFunction::DropShadow {
        offset_x: px(2.0),
        offset_y: px(4.0),
        blur_radius: px(3.0),
        color: Color::BLACK,
      }])
    );
  }

  #[test]
  fn filter_drop_shadow_with_blur_and_color() {
    assert_eq!(
      parse("drop-shadow(2px 4px 3px red)"),
      Some(vec![CssFilterFunction::DropShadow {
        offset_x: px(2.0),
        offset_y: px(4.0),
        blur_radius: px(3.0),
        color: Color::from_rgba8(255, 0, 0, 255),
      }])
    );
  }

  #[test]
  fn filter_drop_shadow_negative_blur_rejected() {
    assert_eq!(parse("drop-shadow(2px 4px -1px)"), None);
  }

  #[test]
  fn filter_drop_shadow_color_only_third_token() {
    assert_eq!(
      parse("drop-shadow(2px 4px blue)"),
      Some(vec![CssFilterFunction::DropShadow {
        offset_x: px(2.0),
        offset_y: px(4.0),
        blur_radius: px(0.0),
        color: Color::from_rgba8(0, 0, 255, 255),
      }])
    );
  }

  #[test]
  fn filter_drop_shadow_rgb_color() {
    assert_eq!(
      parse("drop-shadow(1px 2px 0px rgb(10, 20, 30))"),
      Some(vec![CssFilterFunction::DropShadow {
        offset_x: px(1.0),
        offset_y: px(2.0),
        blur_radius: px(0.0),
        color: Color::from_rgba8(10, 20, 30, 255),
      }])
    );
  }

  // --- multiple functions ---

  #[test]
  fn filter_multiple_functions() {
    assert_eq!(
      parse("blur(5px) brightness(0.5)"),
      Some(vec![
        CssFilterFunction::Blur(px(5.0)),
        CssFilterFunction::Brightness(0.5),
      ])
    );
  }

  #[test]
  fn filter_multiple_functions_with_extra_spaces() {
    assert_eq!(
      parse("  blur( 5px )  brightness( 50% )  "),
      Some(vec![
        CssFilterFunction::Blur(px(5.0)),
        CssFilterFunction::Brightness(0.5),
      ])
    );
  }

  // --- invalid inputs ---

  #[test]
  fn filter_unknown_function_rejected() {
    assert_eq!(parse("unknownfn(1)"), None);
  }

  #[test]
  fn filter_missing_paren_accepted() {
    // cssparser treats EOF as an implicit closing paren per CSS spec
    assert_eq!(
      parse("blur(5px"),
      Some(vec![CssFilterFunction::Blur(px(5.0))])
    );
  }

  #[test]
  fn filter_garbage_rejected() {
    assert_eq!(parse("!!!"), None);
  }
}
