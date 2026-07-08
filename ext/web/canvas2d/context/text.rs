// Copyright 2018-2026 the Deno authors. MIT license.

use std::sync::Arc;
use std::sync::Mutex;

use parley::FontContext;
use parley::Layout;
use parley::LayoutContext;
use parley::StyleProperty;
use parley::style::FontFamily;
use parley::style::FontFeature;
use parley::style::FontFeatures;
use parley::style::FontWeight;
use parley::style::GenericFamily;

use super::state::TextAlign;
use super::state::TextBaseline;
use crate::canvas2d::text_metrics::TextMetrics;
use crate::css::font::FontKerning;
use crate::css::font::FontState;
use crate::css::font::TextDirection;

pub(super) fn build_text_layout(
  font_ctx: &mut FontContext,
  layout_ctx: &mut LayoutContext<()>,
  text: &str,
  fstate: &FontState,
) -> Layout<()> {
  use std::borrow::Cow;

  use parley::style::FontFamilyName;

  let mut builder = layout_ctx.ranged_builder(font_ctx, text, 1.0, true);

  let family: FontFamily<'_> = match fstate.families.first().map(|s| s.as_str())
  {
    Some("serif") => GenericFamily::Serif.into(),
    Some("sans-serif") | None => GenericFamily::SansSerif.into(),
    Some("monospace") => GenericFamily::Monospace.into(),
    Some("cursive") => GenericFamily::Cursive.into(),
    Some("fantasy") => GenericFamily::Fantasy.into(),
    Some(name) => {
      FontFamily::Single(FontFamilyName::Named(Cow::Borrowed(name)))
    }
  };
  builder.push_default(StyleProperty::FontFamily(family));
  builder.push_default(StyleProperty::FontSize(fstate.size));
  builder.push_default(StyleProperty::FontWeight(FontWeight::new(
    fstate.weight as f32,
  )));
  builder.push_default(StyleProperty::FontStyle(fstate.style.to_parley()));
  builder.push_default(StyleProperty::FontWidth(fstate.stretch.to_parley()));

  let letter_spacing_px =
    fstate.letter_spacing.resolve_to_pixels(fstate.size as f64) as f32;
  if letter_spacing_px != 0.0 {
    builder.push_default(StyleProperty::LetterSpacing(letter_spacing_px));
  }

  let word_spacing_px =
    fstate.word_spacing.resolve_to_pixels(fstate.size as f64) as f32;
  if word_spacing_px != 0.0 {
    builder.push_default(StyleProperty::WordSpacing(word_spacing_px));
  }

  if fstate.font_kerning == FontKerning::None {
    let kern_off = FontFeature::new(parley::setting::Tag::new(b"kern"), 0);
    builder.push_default(StyleProperty::FontFeatures(FontFeatures::List(
      Cow::Owned(vec![kern_off]),
    )));
  }

  let mut layout = builder.build(text);
  layout.break_all_lines(None);
  layout.align(
    parley::Alignment::Start,
    parley::AlignmentOptions::default(),
  );
  layout
}

/// Adjusts the canvas-space y for textBaseline alignment.
pub(super) fn compute_baseline_y(
  fill_y: f64,
  layout: &Layout<()>,
  baseline: TextBaseline,
) -> f64 {
  let (ascent, descent) = if let Some(line) = layout.lines().next() {
    let m = line.metrics();
    (m.ascent as f64, m.descent as f64)
  } else {
    (0.0, 0.0)
  };

  match baseline {
    TextBaseline::Alphabetic => fill_y,
    TextBaseline::Top => fill_y + ascent,
    TextBaseline::Bottom => fill_y - descent,
    TextBaseline::Middle => fill_y + (ascent - descent) / 2.0,
    TextBaseline::Hanging => fill_y + ascent * 0.8,
    TextBaseline::Ideographic => fill_y - descent,
  }
}

pub(super) fn compute_text_metrics(
  text: &str,
  fstate: &FontState,
  text_align: TextAlign,
  font_ctx: &Arc<Mutex<FontContext>>,
  layout_ctx: &Arc<Mutex<LayoutContext<()>>>,
) -> TextMetrics {
  let mut fc = font_ctx.lock().unwrap();
  let mut lc = layout_ctx.lock().unwrap();
  let layout = build_text_layout(&mut fc, &mut lc, text, fstate);

  let mut width = 0.0f64;
  let mut font_bb_ascent = 0.0f64;
  let mut font_bb_descent = 0.0f64;

  for line in layout.lines() {
    let m = line.metrics();
    width = width.max((m.advance - m.trailing_whitespace) as f64);
    font_bb_ascent = font_bb_ascent.max(m.ascent as f64);
    font_bb_descent = font_bb_descent.max(m.descent as f64);
  }

  let em_ascent = fstate.size as f64 * 0.8;
  let em_descent = fstate.size as f64 * 0.2;

  // actualBoundingBoxLeft/Right are signed distances from the alignment
  // anchor given by textAlign and direction.
  // See <https://html.spec.whatwg.org/multipage/canvas.html#dom-textmetrics-actualboundingboxleft>
  let rtl = fstate.direction == TextDirection::Rtl;
  let anchor = match text_align {
    TextAlign::Left => 0.0,
    TextAlign::Right => width,
    TextAlign::Center => width / 2.0,
    TextAlign::Start if rtl => width,
    TextAlign::Start => 0.0,
    TextAlign::End if rtl => 0.0,
    TextAlign::End => width,
  };

  TextMetrics {
    width,
    actual_bounding_box_left: anchor,
    actual_bounding_box_right: width - anchor,
    font_bounding_box_ascent: font_bb_ascent,
    font_bounding_box_descent: font_bb_descent,
    actual_bounding_box_ascent: font_bb_ascent,
    actual_bounding_box_descent: font_bb_descent,
    em_height_ascent: em_ascent,
    em_height_descent: em_descent,
    hanging_baseline: em_ascent * 0.8,
    alphabetic_baseline: 0.0,
    ideographic_baseline: -em_descent,
  }
}
