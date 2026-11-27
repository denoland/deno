// Copyright 2018-2026 the Deno authors. MIT license.

use cssparser::Parser;
pub use cssparser::ParserInput;
use cssparser::match_ignore_ascii_case;

use super::color::Color;
use super::color::parse_color_value;
use super::error::CSSCustomError;
use super::error::CSSParseError;
use super::value::Angle;
use super::value::Length;
use super::value::LengthResolution;
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
    resolution: &LengthResolution,
  ) -> Result<Self, CSSParseError<'i>> {
    let name = input.expect_function()?;
    match_ignore_ascii_case! { &name,
      "blur" => {
        input.parse_nested_block(|args| {
          if args.is_exhausted() {
            return Ok(CssFilterFunction::Blur(Length::zero()));
          }
          let px = parse_length_pixels(args, resolution)?;
          if px < 0.0 {
            return Err(args.new_custom_error(CSSCustomError::UnexpectedNumericType));
          }
          args.expect_exhausted()?;
          Ok(CssFilterFunction::Blur(Length::from_pixels(px)))
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
        input.parse_nested_block(|args| parse_drop_shadow(args, resolution))
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

/// Parses a `<length>` and folds it to pixels at set time.
/// https://html.spec.whatwg.org/multipage/canvas.html#dom-context-2d-filter
#[inline]
fn parse_length_pixels<'i, 't>(
  args: &mut Parser<'i, 't>,
  resolution: &LengthResolution,
) -> Result<f64, CSSParseError<'i>> {
  let value = NumericValue::parse(
    args,
    ParseOptions {
      length_resolution: Some(*resolution),
      ..Default::default()
    },
  )?;
  let length = try_extract!(value, expect_length(true), args);
  Ok(length.resolve_to_pixels(resolution))
}

#[inline]
fn parse_drop_shadow<'i, 't>(
  args: &mut Parser<'i, 't>,
  resolution: &LengthResolution,
) -> Result<CssFilterFunction, CSSParseError<'i>> {
  let offset_x = Length::from_pixels(parse_length_pixels(args, resolution)?);
  let offset_y = Length::from_pixels(parse_length_pixels(args, resolution)?);

  let mut blur_radius = Length::zero();
  let mut color = Color::BLACK;

  if !args.is_exhausted() {
    let state = args.state();
    match parse_length_pixels(args, resolution) {
      Ok(px) => {
        if px < 0.0 {
          return Err(
            args.new_custom_error(CSSCustomError::UnexpectedNumericType),
          );
        }
        blur_radius = Length::from_pixels(px);
      }
      Err(_) => {
        args.reset(&state);
      }
    }
  }

  if !args.is_exhausted() {
    color = parse_color_value(args)?.to_srgb8();
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
  resolution: LengthResolution,
  has_function: bool,
  finished: bool,
}

impl<'i: 't, 't> FilterValueListParser<'i, 't> {
  #[inline]
  pub fn new(
    input: &'t mut ParserInput<'i>,
    resolution: LengthResolution,
  ) -> Self {
    Self {
      parser: Parser::new(input),
      resolution,
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

    let result = CssFilterFunction::parse(input, &self.resolution);
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
  use crate::css::font::default_font_resolution;

  fn parse(input: &str) -> Option<Vec<CssFilterFunction>> {
    let mut parser_input = ParserInput::new(input);
    let results: Result<Vec<_>, _> =
      FilterValueListParser::new(&mut parser_input, default_font_resolution())
        .collect();
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
  fn filter_font_relative_lengths() {
    // Resolve against the default `font` (`10px sans-serif`).
    // https://html.spec.whatwg.org/multipage/canvas.html#dom-context-2d-filter
    assert_eq!(
      parse("blur(5em)"),
      Some(vec![CssFilterFunction::Blur(px(50.0))])
    );
    assert_eq!(
      parse("blur(1ex)"),
      Some(vec![CssFilterFunction::Blur(px(5.0))])
    );
    assert_eq!(
      parse("blur(1lh)"),
      Some(vec![CssFilterFunction::Blur(px(12.0))])
    );
    assert_eq!(
      parse("blur(2rem)"),
      Some(vec![CssFilterFunction::Blur(px(20.0))])
    );
    assert_eq!(
      parse("drop-shadow(1em 2ex 1em red)"),
      Some(vec![CssFilterFunction::DropShadow {
        offset_x: px(10.0),
        offset_y: px(10.0),
        blur_radius: px(10.0),
        color: Color::from_rgba8(255, 0, 0, 255),
      }])
    );
    assert_eq!(parse("blur(-1em)"), None);
  }

  #[test]
  fn filter_viewport_lengths_are_zero() {
    // Canvas has no viewport, so these parse but contribute nothing.
    assert_eq!(
      parse("blur(5vw)"),
      Some(vec![CssFilterFunction::Blur(px(0.0))])
    );
    assert_eq!(
      parse("blur(5cqmin)"),
      Some(vec![CssFilterFunction::Blur(px(0.0))])
    );
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
