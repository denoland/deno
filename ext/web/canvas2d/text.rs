// Copyright 2018-2026 the Deno authors. MIT license.

use std::borrow::Cow;
use std::cell::RefCell;
use std::rc::Rc;

use parley::FontContext;
use parley::Language;
use parley::Layout;
use parley::LayoutContext;
use parley::PositionedLayoutItem;
use parley::StyleProperty;
use parley::style::FontFamily;
use parley::style::FontFamilyName;
use parley::style::FontFeature;
use parley::style::FontFeatures;
use parley::style::FontWeight;
use parley::style::GenericFamily;

use super::font_metrics::length_resolution;
use super::state::TextAlign;
use super::state::TextBaseline;
use crate::canvas2d::text_metrics::TextMetrics;
use crate::css::font::FontKerning;
use crate::css::font::FontState;
use crate::css::font::FontVariantCaps;
use crate::css::font::TextDirection;
use crate::css::value::FontMetrics;
use crate::css::value::LengthResolution;

/// ASCII whitespace -> U+0020 (not collapsed); drop U+0000.
/// https://html.spec.whatwg.org/multipage/canvas.html#text-preparation-algorithm
pub(super) fn prepare_text(text: &str) -> Cow<'_, str> {
  let needs_rewrite = text
    .bytes()
    .any(|b| matches!(b, b'\0' | b'\t' | b'\n' | b'\x0C' | b'\r'));
  if !needs_rewrite {
    return Cow::Borrowed(text);
  }
  Cow::Owned(
    text
      .chars()
      .filter(|&c| c != '\0')
      .map(|c| match c {
        '\t' | '\n' | '\u{000C}' | '\r' => ' ',
        c => c,
      })
      .collect(),
  )
}

/// "inherit" / empty -> no locale.
fn resolve_locale(lang: &str) -> Option<Language> {
  let tag = lang.trim();
  if tag.is_empty() || tag.eq_ignore_ascii_case("inherit") {
    return None;
  }
  Language::parse(tag).ok()
}

/// Uppercase synthesis when the face has no `smcp` (no scaled small forms).
fn synthesize_caps_text(text: &str, caps: FontVariantCaps) -> Option<String> {
  match caps {
    FontVariantCaps::Normal | FontVariantCaps::TitlingCaps => None,
    FontVariantCaps::SmallCaps
    | FontVariantCaps::AllSmallCaps
    | FontVariantCaps::PetiteCaps
    | FontVariantCaps::AllPetiteCaps
    | FontVariantCaps::Unicase => {
      let upper = text.to_uppercase();
      if upper == text { None } else { Some(upper) }
    }
  }
}

fn font_variant_caps_features(caps: FontVariantCaps) -> Vec<FontFeature> {
  // https://drafts.csswg.org/css-fonts-4/#font-variant-caps-prop
  match caps {
    FontVariantCaps::Normal => vec![],
    FontVariantCaps::SmallCaps => {
      vec![FontFeature::new(parley::setting::Tag::new(b"smcp"), 1)]
    }
    FontVariantCaps::AllSmallCaps => vec![
      FontFeature::new(parley::setting::Tag::new(b"smcp"), 1),
      FontFeature::new(parley::setting::Tag::new(b"c2sc"), 1),
    ],
    FontVariantCaps::PetiteCaps => {
      vec![FontFeature::new(parley::setting::Tag::new(b"pcap"), 1)]
    }
    FontVariantCaps::AllPetiteCaps => vec![
      FontFeature::new(parley::setting::Tag::new(b"pcap"), 1),
      FontFeature::new(parley::setting::Tag::new(b"c2pc"), 1),
    ],
    FontVariantCaps::Unicase => {
      vec![FontFeature::new(parley::setting::Tag::new(b"unic"), 1)]
    }
    FontVariantCaps::TitlingCaps => {
      vec![FontFeature::new(parley::setting::Tag::new(b"titl"), 1)]
    }
  }
}

/// Resolves letter/word spacing in pixels against the current font, touching
/// the font collection only when a font-relative unit is actually used.
fn resolve_spacing(
  font_ctx: &mut FontContext,
  fstate: &FontState,
) -> (f32, f32) {
  let resolution = if fstate.letter_spacing.is_relative_length()
    || fstate.word_spacing.is_relative_length()
  {
    length_resolution(font_ctx, fstate)
  } else {
    LengthResolution::new(FontMetrics::fallback(fstate.size as f64))
  };
  (
    fstate.letter_spacing.resolve(&resolution) as f32,
    fstate.word_spacing.resolve(&resolution) as f32,
  )
}

/// Builds a parley layout for canvas text (`lang`: canvas `lang` attribute).
pub(super) fn build_text_layout(
  font_ctx: &mut FontContext,
  layout_ctx: &mut LayoutContext<()>,
  text: &str,
  fstate: &FontState,
  lang: &str,
) -> Layout<()> {
  let text = prepare_text(text);
  let synthesized = synthesize_caps_text(&text, fstate.font_variant_caps);
  let text: &str = synthesized.as_deref().unwrap_or(&text);

  // Spacing is resolved before the builder borrows the font context, because
  // font-relative units have to query the collection for the current face.
  let (letter_spacing_px, word_spacing_px) = resolve_spacing(font_ctx, fstate);

  let mut builder = layout_ctx.ranged_builder(font_ctx, text, 1.0, true);

  // Full family list so missing faces fall back to later entries.
  let names = fstate
    .families
    .iter()
    .map(|name| match GenericFamily::parse(name) {
      Some(generic) => FontFamilyName::Generic(generic),
      None => FontFamilyName::Named(Cow::Borrowed(name.as_str())),
    })
    .collect::<Vec<_>>();
  let family = match names.len() {
    0 => FontFamily::Single(FontFamilyName::Generic(GenericFamily::SansSerif)),
    1 => FontFamily::Single(names.into_iter().next().unwrap()),
    _ => FontFamily::List(Cow::Owned(names)),
  };
  builder.push_default(StyleProperty::FontFamily(family));
  builder.push_default(StyleProperty::FontSize(fstate.size));
  builder.push_default(StyleProperty::FontWeight(FontWeight::new(
    fstate.weight as f32,
  )));
  builder.push_default(StyleProperty::FontStyle(fstate.style.to_parley()));
  builder.push_default(StyleProperty::FontWidth(fstate.width.to_parley()));

  if letter_spacing_px != 0.0 {
    builder.push_default(StyleProperty::LetterSpacing(letter_spacing_px));
  }

  if word_spacing_px != 0.0 {
    builder.push_default(StyleProperty::WordSpacing(word_spacing_px));
  }

  let mut features = font_variant_caps_features(fstate.font_variant_caps);
  if fstate.font_kerning == FontKerning::None {
    features.push(FontFeature::new(parley::setting::Tag::new(b"kern"), 0));
  }
  if !features.is_empty() {
    builder.push_default(StyleProperty::FontFeatures(FontFeatures::List(
      Cow::Owned(features),
    )));
  }

  if let Some(locale) = resolve_locale(lang) {
    builder.push_default(StyleProperty::Locale(Some(locale)));
  }

  let mut layout = builder.build(text);
  layout.break_all_lines(None);
  layout.align(
    parley::Alignment::Start,
    parley::AlignmentOptions::default(),
  );
  layout
}

/// Em / baseline offsets from the alphabetic baseline (OpenType Y+ up).
#[derive(Clone, Copy, Debug)]
pub(super) struct FontMetricOffsets {
  pub em_height_ascent: f64,
  pub em_height_descent: f64,
  pub hanging_from_alphabetic: f64,
  pub ideographic_from_alphabetic: f64,
}

impl FontMetricOffsets {
  fn fallback(font_size: f64) -> Self {
    Self {
      em_height_ascent: font_size * 0.8,
      em_height_descent: font_size * 0.2,
      hanging_from_alphabetic: font_size * 0.8,
      ideographic_from_alphabetic: -(font_size * 0.2),
    }
  }
}

/// OS/2 sTypo + BASE hang/ideo from the first glyph run's face.
pub(super) fn font_metric_offsets(
  layout: &Layout<()>,
  font_size: f64,
) -> FontMetricOffsets {
  let mut offsets = FontMetricOffsets::fallback(font_size);

  let Some(line) = layout.lines().next() else {
    return offsets;
  };
  for item in line.items() {
    let PositionedLayoutItem::GlyphRun(glyph_run) = item else {
      continue;
    };
    let font = glyph_run.run().font();
    let Ok(face) = ttf_parser::Face::parse(font.data.as_ref(), font.index)
    else {
      continue;
    };
    let units = f64::from(face.units_per_em());
    if units <= 0.0 {
      continue;
    }
    let scale = font_size / units;

    // sTypo ratio, normalized so ascent + descent == font-size.
    if let (Some(asc), Some(desc)) =
      (face.typographic_ascender(), face.typographic_descender())
    {
      let asc = f64::from(asc);
      let desc = f64::from(desc); // typically negative
      let sum = asc + desc.abs();
      if sum > 0.0 {
        offsets.em_height_ascent = asc / sum * font_size;
        offsets.em_height_descent = desc.abs() / sum * font_size;
      }
    }

    if let Some((hang, ideo)) = parse_base_hang_ideo(&face) {
      offsets.hanging_from_alphabetic = f64::from(hang) * scale;
      offsets.ideographic_from_alphabetic = f64::from(ideo) * scale;
    } else {
      offsets.hanging_from_alphabetic = offsets.em_height_ascent * 0.8;
      offsets.ideographic_from_alphabetic = -offsets.em_height_descent;
    }
    break;
  }
  offsets
}

/// OpenType BASE horizontal `hang` / `ideo` (relative to alphabetic).
fn parse_base_hang_ideo(face: &ttf_parser::Face<'_>) -> Option<(i16, i16)> {
  let data = face
    .raw_face()
    .table(ttf_parser::Tag::from_bytes(b"BASE"))?;
  if data.len() < 8 {
    return None;
  }
  let major = u16::from_be_bytes([data[0], data[1]]);
  if major != 1 {
    return None;
  }
  let horiz_off = usize::from(u16::from_be_bytes([data[4], data[5]]));
  if horiz_off == 0 || horiz_off + 4 > data.len() {
    return None;
  }
  let axis = &data[horiz_off..];
  let tag_list_off = usize::from(u16::from_be_bytes([axis[0], axis[1]]));
  let script_list_off = usize::from(u16::from_be_bytes([axis[2], axis[3]]));
  if tag_list_off == 0 || script_list_off == 0 {
    return None;
  }
  if tag_list_off + 2 > axis.len() {
    return None;
  }
  let tag_list = &axis[tag_list_off..];
  let tag_count = usize::from(u16::from_be_bytes([tag_list[0], tag_list[1]]));
  if tag_list.len() < 2 + tag_count * 4 {
    return None;
  }
  let mut hang_idx = None;
  let mut ideo_idx = None;
  for i in 0..tag_count {
    let t = &tag_list[2 + i * 4..2 + i * 4 + 4];
    match t {
      b"hang" => hang_idx = Some(i),
      b"ideo" => ideo_idx = Some(i),
      _ => {}
    }
  }
  if script_list_off + 2 > axis.len() {
    return None;
  }
  let script_list = &axis[script_list_off..];
  let script_count =
    usize::from(u16::from_be_bytes([script_list[0], script_list[1]]));
  if script_count == 0 || script_list.len() < 2 + 6 {
    return None;
  }
  // First BaseScriptRecord: tag(4) + offset from BaseScriptList.
  let base_script_off =
    usize::from(u16::from_be_bytes([script_list[6], script_list[7]]));
  if base_script_off + 2 > script_list.len() {
    return None;
  }
  let base_script = &script_list[base_script_off..];
  let base_values_off =
    usize::from(u16::from_be_bytes([base_script[0], base_script[1]]));
  if base_values_off == 0 || base_values_off + 4 > base_script.len() {
    return None;
  }
  let base_values = &base_script[base_values_off..];
  let coord_count =
    usize::from(u16::from_be_bytes([base_values[2], base_values[3]]));
  if base_values.len() < 4 + coord_count * 2 {
    return None;
  }
  let read_coord = |idx: usize| -> Option<i16> {
    if idx >= coord_count {
      return None;
    }
    let off = usize::from(u16::from_be_bytes([
      base_values[4 + idx * 2],
      base_values[4 + idx * 2 + 1],
    ]));
    if off + 4 > base_values.len() {
      return None;
    }
    let coord = &base_values[off..];
    let format = u16::from_be_bytes([coord[0], coord[1]]);
    if format != 1 {
      return None;
    }
    Some(i16::from_be_bytes([coord[2], coord[3]]))
  };

  let hang = hang_idx.and_then(read_coord).unwrap_or(0);
  let ideo = ideo_idx.and_then(read_coord).unwrap_or(0);
  Some((hang, ideo))
}

/// Glyph ink relative to alphabetic origin (CSS px; Y+ up).
#[derive(Clone, Copy, Debug)]
struct InkBounds {
  min_x: f64,
  max_x: f64,
  min_y: f64,
  max_y: f64,
}

impl InkBounds {
  fn empty() -> Self {
    Self {
      min_x: f64::INFINITY,
      max_x: f64::NEG_INFINITY,
      min_y: f64::INFINITY,
      max_y: f64::NEG_INFINITY,
    }
  }

  fn is_empty(&self) -> bool {
    self.min_x > self.max_x
  }

  fn include(&mut self, x0: f64, y0: f64, x1: f64, y1: f64) {
    self.min_x = self.min_x.min(x0);
    self.max_x = self.max_x.max(x1);
    self.min_y = self.min_y.min(y0);
    self.max_y = self.max_y.max(y1);
  }
}

fn compute_ink_bounds(layout: &Layout<()>) -> InkBounds {
  let mut ink = InkBounds::empty();
  for line in layout.lines() {
    let layout_baseline = f64::from(line.metrics().baseline);
    for item in line.items() {
      let PositionedLayoutItem::GlyphRun(glyph_run) = item else {
        continue;
      };
      let run = glyph_run.run();
      let font = run.font();
      let font_size = f64::from(run.font_size());
      let Ok(face) = ttf_parser::Face::parse(font.data.as_ref(), font.index)
      else {
        continue;
      };
      let units = f64::from(face.units_per_em());
      if units <= 0.0 {
        continue;
      }
      let scale = font_size / units;
      for g in glyph_run.positioned_glyphs() {
        let id = ttf_parser::GlyphId(g.id as u16);
        let Some(bbox) = face.glyph_bounding_box(id) else {
          continue;
        };
        let x0 = f64::from(g.x) + f64::from(bbox.x_min) * scale;
        let x1 = f64::from(g.x) + f64::from(bbox.x_max) * scale;
        // Line-space g.y -> alphabetic; font bbox is Y+ up from the origin.
        let y_down = f64::from(g.y) - layout_baseline;
        let y0 = f64::from(bbox.y_min) * scale - y_down;
        let y1 = f64::from(bbox.y_max) * scale - y_down;
        ink.include(x0, y0, x1, y1);
      }
    }
  }
  ink
}

/// Alphabetic y so that `baseline` sits at canvas `fill_y` (Y+ down).
pub(super) fn compute_baseline_y(
  fill_y: f64,
  baseline: TextBaseline,
  offsets: &FontMetricOffsets,
) -> f64 {
  match baseline {
    TextBaseline::Alphabetic => fill_y,
    TextBaseline::Top => fill_y + offsets.em_height_ascent,
    TextBaseline::Bottom => fill_y - offsets.em_height_descent,
    TextBaseline::Middle => {
      fill_y + (offsets.em_height_ascent - offsets.em_height_descent) / 2.0
    }
    TextBaseline::Hanging => fill_y + offsets.hanging_from_alphabetic,
    TextBaseline::Ideographic => fill_y + offsets.ideographic_from_alphabetic,
  }
}

#[inline]
pub(super) fn alignment_anchor(
  width: f64,
  text_align: TextAlign,
  rtl: bool,
) -> f64 {
  match text_align {
    TextAlign::Left => 0.0,
    TextAlign::Right => width,
    TextAlign::Center => width / 2.0,
    TextAlign::Start if rtl => width,
    TextAlign::Start => 0.0,
    TextAlign::End if rtl => 0.0,
    TextAlign::End => width,
  }
}

pub(super) fn compute_text_metrics(
  text: &str,
  fstate: &FontState,
  text_align: TextAlign,
  lang: &str,
  font_ctx: &Rc<RefCell<FontContext>>,
  layout_ctx: &Rc<RefCell<LayoutContext<()>>>,
) -> TextMetrics {
  // Empty after prep -> zero width (parley still synthesizes a strut).
  let prepared = prepare_text(text);
  if prepared.is_empty() {
    let offsets = FontMetricOffsets::fallback(fstate.size as f64);
    return TextMetrics {
      width: 0.0,
      actual_bounding_box_left: 0.0,
      actual_bounding_box_right: 0.0,
      font_bounding_box_ascent: 0.0,
      font_bounding_box_descent: 0.0,
      actual_bounding_box_ascent: 0.0,
      actual_bounding_box_descent: 0.0,
      em_height_ascent: offsets.em_height_ascent,
      em_height_descent: offsets.em_height_descent,
      hanging_baseline: offsets.hanging_from_alphabetic,
      alphabetic_baseline: 0.0,
      ideographic_baseline: offsets.ideographic_from_alphabetic,
    };
  }

  let mut fc = font_ctx.borrow_mut();
  let mut lc = layout_ctx.borrow_mut();
  let layout = build_text_layout(&mut fc, &mut lc, text, fstate, lang);

  let mut width = 0.0f64;
  let mut font_bb_ascent = 0.0f64;
  let mut font_bb_descent = 0.0f64;

  for line in layout.lines() {
    let m = line.metrics();
    // Trailing spaces count (no collapse).
    width = width.max(f64::from(m.advance));
    font_bb_ascent = font_bb_ascent.max(f64::from(m.ascent));
    font_bb_descent = font_bb_descent.max(f64::from(m.descent));
  }

  let offsets = font_metric_offsets(&layout, fstate.size as f64);
  let ink = compute_ink_bounds(&layout);
  let rtl = fstate.direction == TextDirection::Rtl;
  let anchor = alignment_anchor(width, text_align, rtl);

  let (actual_left, actual_right, actual_ascent, actual_descent) =
    if ink.is_empty() {
      (anchor, width - anchor, font_bb_ascent, font_bb_descent)
    } else {
      // Positive left = ink left of the alignment point.
      (
        anchor - ink.min_x,
        ink.max_x - anchor,
        ink.max_y.max(0.0),
        (-ink.min_y).max(0.0),
      )
    };

  TextMetrics {
    width,
    actual_bounding_box_left: actual_left,
    actual_bounding_box_right: actual_right,
    font_bounding_box_ascent: font_bb_ascent,
    font_bounding_box_descent: font_bb_descent,
    actual_bounding_box_ascent: actual_ascent,
    actual_bounding_box_descent: actual_descent,
    em_height_ascent: offsets.em_height_ascent,
    em_height_descent: offsets.em_height_descent,
    hanging_baseline: offsets.hanging_from_alphabetic,
    alphabetic_baseline: 0.0,
    ideographic_baseline: offsets.ideographic_from_alphabetic,
  }
}
