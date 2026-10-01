// Copyright 2018-2026 the Deno authors. MIT license.

use std::borrow::Cow;
use std::cell::RefCell;
use std::ops::Range;
use std::rc::Rc;

use parley::Cluster;
use parley::ClusterSide;
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
use unicode_segmentation::UnicodeSegmentation;

use super::font_metrics::length_resolution;
use super::state::TextAlign;
use super::state::TextBaseline;
use crate::canvas2d::error::Canvas2DError;
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

struct PreparedText {
  /// The string handed to parley.
  shaped: String,
  /// UTF-16 length of the input.
  utf16_len: usize,
  /// UTF-8 byte offset in `shaped` for every UTF-16 index of the input, plus a
  /// terminator, or `None` when the two index spaces coincide.
  utf16_to_utf8: Option<Vec<u32>>,
}

/// `prepare_text` plus `synthesize_caps_text`, recording the UTF-16 index map
/// the `TextMetrics` range APIs are specified against.
fn prepare_text_indexed(text: &str, caps: FontVariantCaps) -> PreparedText {
  // One UTF-8 byte per UTF-16 code unit for ASCII, the same shortcut
  // `op_encoding_encode_into_fast` takes for its `read` count. Preparation only
  // substitutes whitespace here, and uppercasing ASCII is 1:1, so no index
  // moves and the map is the identity.
  if text.is_ascii() && !text.contains('\0') {
    let prepared = prepare_text(text);
    let shaped = synthesize_caps_text(&prepared, caps)
      .unwrap_or_else(|| prepared.into_owned());
    debug_assert_eq!(shaped.len(), text.len());
    let utf16_len = shaped.len();
    return PreparedText {
      shaped,
      utf16_len,
      utf16_to_utf8: None,
    };
  }

  let upcase = synthesizes_small_caps(caps);
  let mut shaped = String::with_capacity(text.len());
  let mut utf16_to_utf8 = Vec::with_capacity(text.len() + 1);
  for c in text.chars() {
    // Both halves of a surrogate pair map to the start of the character.
    for _ in 0..c.len_utf16() {
      utf16_to_utf8.push(shaped.len() as u32);
    }
    match c {
      '\0' => {}
      '\t' | '\n' | '\u{000C}' | '\r' => shaped.push(' '),
      c if upcase => shaped.extend(c.to_uppercase()),
      c => shaped.push(c),
    }
  }
  utf16_to_utf8.push(shaped.len() as u32);
  PreparedText {
    shaped,
    utf16_len: utf16_to_utf8.len() - 1,
    utf16_to_utf8: Some(utf16_to_utf8),
  }
}

/// "inherit" / empty -> no locale.
fn resolve_locale(lang: &str) -> Option<Language> {
  let tag = lang.trim();
  if tag.is_empty() || tag.eq_ignore_ascii_case("inherit") {
    return None;
  }
  Language::parse(tag).ok()
}

/// Whether the variant draws small forms that we synthesize by uppercasing.
fn synthesizes_small_caps(caps: FontVariantCaps) -> bool {
  match caps {
    FontVariantCaps::Normal | FontVariantCaps::TitlingCaps => false,
    FontVariantCaps::SmallCaps
    | FontVariantCaps::AllSmallCaps
    | FontVariantCaps::PetiteCaps
    | FontVariantCaps::AllPetiteCaps
    | FontVariantCaps::Unicase => true,
  }
}

/// Uppercase synthesis when the face has no `smcp` (no scaled small forms).
fn synthesize_caps_text(text: &str, caps: FontVariantCaps) -> Option<String> {
  if !synthesizes_small_caps(caps) {
    return None;
  }
  let upper = text.to_uppercase();
  if upper == text { None } else { Some(upper) }
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
  build_prepared_text_layout(font_ctx, layout_ctx, text, fstate, lang)
}

/// `build_text_layout` for text that already went through preparation.
fn build_prepared_text_layout(
  font_ctx: &mut FontContext,
  layout_ctx: &mut LayoutContext<()>,
  text: &str,
  fstate: &FontState,
  lang: &str,
) -> Layout<()> {
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

/// x the glyph stream of a line starts at, matching `GlyphRun::offset`.
#[inline]
fn line_origin_x(line: &parley::Line<'_, ()>) -> f64 {
  let metrics = line.metrics();
  f64::from(metrics.inline_min_coord) + f64::from(metrics.offset)
}

/// Union of the ink boxes of the glyphs whose cluster overlaps `range`, or of
/// every glyph when `range` is `None`.
///
/// The x accumulation reproduces `GlyphRun::positioned_glyphs`: parley adds
/// letter and word spacing to both a cluster's advance and its last glyph's
/// advance, so walking clusters and walking the glyph stream stay in step.
fn compute_ink_bounds(
  layout: &Layout<()>,
  range: Option<&Range<usize>>,
) -> InkBounds {
  let mut ink = InkBounds::empty();
  for line in layout.lines() {
    let mut x = line_origin_x(&line);
    for run in line.runs() {
      let font = run.font();
      let font_size = f64::from(run.font_size());
      let face = ttf_parser::Face::parse(font.data.as_ref(), font.index).ok();
      let scale = face
        .as_ref()
        .map(|face| f64::from(face.units_per_em()))
        .filter(|&units| units > 0.0)
        .map(|units| font_size / units);
      for cluster in run.visual_clusters() {
        let text_range = cluster.text_range();
        let included = range.is_none_or(|range| overlaps(&text_range, range));
        for g in cluster.glyphs() {
          if let (true, Some(face), Some(scale)) = (included, &face, scale) {
            let id = ttf_parser::GlyphId(g.id as u16);
            if let Some(bbox) = face.glyph_bounding_box(id) {
              let gx = x + f64::from(g.x);
              // Glyph y is relative to the baseline; the font bbox is Y+ up.
              let y_down = f64::from(g.y);
              ink.include(
                gx + f64::from(bbox.x_min) * scale,
                f64::from(bbox.y_min) * scale - y_down,
                gx + f64::from(bbox.x_max) * scale,
                f64::from(bbox.y_max) * scale - y_down,
              );
            }
          }
          x += f64::from(g.advance);
        }
      }
    }
  }
  ink
}

#[inline]
fn overlaps(a: &Range<usize>, b: &Range<usize>) -> bool {
  a.start < b.end && a.end > b.start
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

/// A rectangle in the coordinate space `TextMetrics` reports in: the origin is
/// the anchor point the `textAlign` and `textBaseline` of the `measureText()`
/// call put the text at, and y grows downwards.
#[derive(Clone, Copy, Debug)]
pub(super) struct MetricsRect {
  pub x: f64,
  pub y: f64,
  pub width: f64,
  pub height: f64,
}

/// One minimal rendering unit of the measured text.
#[derive(Clone, Copy, Debug)]
pub(super) struct ClusterGeometry {
  /// UTF-16 range of the measured text, as `TextCluster.start`/`end`.
  pub start: u32,
  pub end: u32,
  pub x: f64,
  pub y: f64,
}

/// A shaping cluster together with the position its glyphs are drawn at.
struct ShapedCluster {
  /// UTF-8 byte range of the shaped text.
  text_range: Range<usize>,
  rtl: bool,
  /// Index of the containing run across the whole layout. Selection rectangles
  /// are emitted per run so that bidi text yields one per direction change.
  run_index: usize,
  /// Visual x of the leading edge, in the same space as the drawn glyphs.
  x: f64,
  advance: f64,
}

/// Walks the shaping clusters in visual order. See `compute_ink_bounds` for why
/// accumulating cluster advances tracks the glyph stream.
fn shaped_clusters(layout: &Layout<()>) -> Vec<ShapedCluster> {
  let mut clusters = Vec::new();
  let mut run_index = 0;
  for line in layout.lines() {
    let mut x = line_origin_x(&line);
    for run in line.runs() {
      for cluster in run.visual_clusters() {
        let advance = f64::from(cluster.advance());
        clusters.push(ShapedCluster {
          text_range: cluster.text_range(),
          rtl: cluster.is_rtl(),
          run_index,
          x,
          advance,
        });
        x += advance;
      }
      run_index += 1;
    }
  }
  clusters
}

/// Everything `measureText()` has to retain for the `TextMetrics` range and
/// cluster APIs, which are specified against the text and the
/// `CanvasTextDrawingStyles` that were in effect when it was called.
///
/// The shaped layout is kept rather than rebuilt on demand: the drawing styles
/// and the registered font faces can both change afterwards, and
/// `fillTextCluster()` has to reproduce the measurement either way.
pub(super) struct TextMeasurement {
  /// See `PreparedText::shaped`.
  shaped: String,
  /// See `PreparedText::utf16_len`.
  utf16_len: usize,
  /// See `PreparedText::utf16_to_utf8`.
  utf16_to_utf8: Option<Vec<u32>>,
  /// `None` when preparation left nothing to shape.
  layout: Option<Layout<()>>,
  offsets: FontMetricOffsets,
  /// Line advance, trailing spaces included.
  line_width: f64,
  font_bounding_box_ascent: f64,
  font_bounding_box_descent: f64,
  text_align: TextAlign,
  text_baseline: TextBaseline,
  rtl: bool,
}

impl TextMeasurement {
  /// Number of UTF-16 code units in the measured text.
  pub(super) fn utf16_len(&self) -> u32 {
    self.utf16_len as u32
  }

  pub(super) fn layout(&self) -> Option<&Layout<()>> {
    self.layout.as_ref()
  }

  pub(super) fn text_align(&self) -> TextAlign {
    self.text_align
  }

  pub(super) fn text_baseline(&self) -> TextBaseline {
    self.text_baseline
  }

  /// x of a `text_align` anchor in line space.
  pub(super) fn anchor_x(&self, text_align: TextAlign) -> f64 {
    alignment_anchor(self.line_width, text_align, self.rtl)
  }

  /// y of `baseline`, measured downwards from the alphabetic baseline.
  pub(super) fn baseline_offset(&self, baseline: TextBaseline) -> f64 {
    -compute_baseline_y(0.0, baseline, &self.offsets)
  }

  /// y of the origin of the reported coordinate space, measured downwards from
  /// the alphabetic baseline.
  fn origin_y(&self) -> f64 {
    self.baseline_offset(self.text_baseline)
  }

  /// The range check shared by `getSelectionRects()`, `getActualBoundingBox()`
  /// and `getTextClusters()`. An inverted range is allowed, an out-of-range
  /// index is not.
  fn check_range(&self, start: u32, end: u32) -> Result<(), Canvas2DError> {
    let len = self.utf16_len();
    if start >= len || end > len {
      return Err(Canvas2DError::TextMetricsIndexSize);
    }
    Ok(())
  }

  /// UTF-8 byte range of the shaped text for a UTF-16 range of the input. An
  /// inverted range collapses rather than wrapping around.
  pub(super) fn utf8_range(&self, start: u32, end: u32) -> Range<usize> {
    let (start, end) = match &self.utf16_to_utf8 {
      Some(map) => (map[start as usize] as usize, map[end as usize] as usize),
      None => (start as usize, end as usize),
    };
    start..end.max(start)
  }

  /// Inverse of `utf8_range`.
  fn utf16_index(&self, utf8_offset: usize) -> u32 {
    match &self.utf16_to_utf8 {
      Some(map) => {
        map.partition_point(|&offset| (offset as usize) < utf8_offset) as u32
      }
      None => utf8_offset as u32,
    }
  }

  /// https://html.spec.whatwg.org/multipage/canvas.html#dom-textmetrics-getselectionrects
  pub(super) fn selection_rects(
    &self,
    start: u32,
    end: u32,
  ) -> Result<Vec<MetricsRect>, Canvas2DError> {
    self.check_range(start, end)?;
    let Some(layout) = &self.layout else {
      return Ok(Vec::new());
    };
    let range = self.utf8_range(start, end);
    let anchor_x = self.anchor_x(self.text_align);
    // Selection follows the advance of the text, so it spans the font box.
    let y = -self.font_bounding_box_ascent - self.origin_y();
    let height = self.font_bounding_box_ascent + self.font_bounding_box_descent;
    let rect = |x: f64, width: f64| MetricsRect {
      x: x - anchor_x,
      y,
      width,
      height,
    };

    let clusters = shaped_clusters(layout);
    if range.is_empty() {
      // A collapsed range selects nothing; report the caret instead.
      let caret = clusters
        .iter()
        .find(|c| c.text_range.contains(&range.start))
        .map(|c| if c.rtl { c.x + c.advance } else { c.x })
        .or_else(|| clusters.last().map(|c| c.x + c.advance))
        .unwrap_or(0.0);
      return Ok(vec![rect(caret, 0.0)]);
    }

    let mut rects = Vec::new();
    let mut current: Option<(usize, f64, f64)> = None;
    for shaped in &clusters {
      if !overlaps(&shaped.text_range, &range) {
        continue;
      }
      let (min, max) = (shaped.x, shaped.x + shaped.advance);
      match current {
        Some((run_index, ref mut lo, ref mut hi))
          if run_index == shaped.run_index =>
        {
          *lo = lo.min(min);
          *hi = hi.max(max);
        }
        _ => {
          if let Some((_, lo, hi)) = current {
            rects.push(rect(lo, hi - lo));
          }
          current = Some((shaped.run_index, min, max));
        }
      }
    }
    if let Some((_, lo, hi)) = current {
      rects.push(rect(lo, hi - lo));
    }
    Ok(rects)
  }

  /// https://html.spec.whatwg.org/multipage/canvas.html#dom-textmetrics-getactualboundingbox
  pub(super) fn actual_bounding_box(
    &self,
    start: u32,
    end: u32,
  ) -> Result<MetricsRect, Canvas2DError> {
    self.check_range(start, end)?;
    let anchor_x = self.anchor_x(self.text_align);
    let origin_y = self.origin_y();
    let empty = MetricsRect {
      x: -anchor_x,
      y: -origin_y,
      width: 0.0,
      height: 0.0,
    };
    let Some(layout) = &self.layout else {
      return Ok(empty);
    };
    let range = self.utf8_range(start, end);
    if range.is_empty() {
      return Ok(empty);
    }
    let ink = compute_ink_bounds(layout, Some(&range));
    if ink.is_empty() {
      // No ink in the range: fall back to its advance, the way the
      // `actualBoundingBox*` attributes fall back to the font box.
      let (min_x, max_x) = shaped_clusters(layout)
        .iter()
        .filter(|shaped| overlaps(&shaped.text_range, &range))
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), shaped| {
          (lo.min(shaped.x), hi.max(shaped.x + shaped.advance))
        });
      if min_x > max_x {
        return Ok(empty);
      }
      return Ok(MetricsRect {
        x: min_x - anchor_x,
        y: -self.font_bounding_box_ascent - origin_y,
        width: max_x - min_x,
        height: self.font_bounding_box_ascent + self.font_bounding_box_descent,
      });
    }
    Ok(MetricsRect {
      x: ink.min_x - anchor_x,
      // Ink is Y+ up from the alphabetic baseline.
      y: -ink.max_y - origin_y,
      width: ink.max_x - ink.min_x,
      height: ink.max_y - ink.min_y,
    })
  }

  /// https://html.spec.whatwg.org/multipage/canvas.html#dom-textmetrics-getindexfromoffset
  pub(super) fn index_from_offset(&self, offset: f64) -> u32 {
    let Some(layout) = &self.layout else {
      return 0;
    };
    let x = (offset + self.anchor_x(self.text_align)) as f32;
    let Some((cluster, side)) = Cluster::from_point(layout, x, 0.0) else {
      return 0;
    };
    let text_range = cluster.text_range();
    // The visually leading edge of an RTL cluster is its logical end.
    let (leading, trailing) = if cluster.is_rtl() {
      (text_range.end, text_range.start)
    } else {
      (text_range.start, text_range.end)
    };
    self.utf16_index(match side {
      ClusterSide::Left => leading,
      ClusterSide::Right => trailing,
    })
  }

  /// https://html.spec.whatwg.org/multipage/canvas.html#dom-textmetrics-gettextclusters
  pub(super) fn text_clusters(
    &self,
    start: u32,
    end: u32,
    align: TextAlign,
    baseline: TextBaseline,
  ) -> Result<Vec<ClusterGeometry>, Canvas2DError> {
    self.check_range(start, end)?;
    let Some(layout) = &self.layout else {
      return Ok(Vec::new());
    };
    let range = self.utf8_range(start, end);
    if range.is_empty() {
      return Ok(Vec::new());
    }

    let shaped = shaped_clusters(layout);
    let anchor_x = self.anchor_x(self.text_align);
    let y = self.baseline_offset(baseline) - self.origin_y();

    let mut clusters = Vec::new();
    for boundary in cluster_boundaries(&self.shaped, &shaped, &range) {
      let (min_x, max_x) = shaped
        .iter()
        .filter(|shaped| overlaps(&shaped.text_range, &boundary))
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), shaped| {
          (lo.min(shaped.x), hi.max(shaped.x + shaped.advance))
        });
      if min_x > max_x {
        continue;
      }
      // `align` picks a point of the cluster's advance, not of its ink.
      let point = alignment_anchor(max_x - min_x, align, self.rtl) + min_x;
      clusters.push(ClusterGeometry {
        start: self.utf16_index(boundary.start),
        end: self.utf16_index(boundary.end),
        x: point - anchor_x,
        y,
      });
    }
    Ok(clusters)
  }
}

/// Splits the part of the shaped text that `range` covers into minimal
/// rendering units, as UTF-8 byte ranges.
///
/// A boundary has to be both a grapheme cluster boundary (UAX #29) and a
/// shaping cluster boundary: a ligature spans graphemes yet cannot be broken,
/// and a grapheme that shapes into several clusters is still one unit.
fn cluster_boundaries(
  text: &str,
  shaped: &[ShapedCluster],
  range: &Range<usize>,
) -> Vec<Range<usize>> {
  let mut starts = Vec::new();
  for (offset, _) in text.grapheme_indices(true) {
    let splits_shaping = shaped
      .iter()
      .all(|shaped| shaped.text_range.start != offset);
    if offset != 0 && splits_shaping {
      continue;
    }
    starts.push(offset);
  }
  starts.push(text.len());

  // A unit the range only partially covers is still included whole.
  starts
    .windows(2)
    .map(|pair| pair[0]..pair[1])
    .filter(|unit| unit.start < range.end && unit.end > range.start)
    .collect()
}

pub(super) fn compute_text_metrics(
  text: &str,
  fstate: &FontState,
  text_align: TextAlign,
  text_baseline: TextBaseline,
  lang: &str,
  font_ctx: &Rc<RefCell<FontContext>>,
  layout_ctx: &Rc<RefCell<LayoutContext<()>>>,
) -> TextMetrics {
  let prepared = prepare_text_indexed(text, fstate.font_variant_caps);
  let rtl = fstate.direction == TextDirection::Rtl;

  // Empty after prep -> zero width (parley still synthesizes a strut).
  if prepared.shaped.is_empty() {
    let offsets = FontMetricOffsets::fallback(fstate.size as f64);
    let measurement = TextMeasurement {
      shaped: prepared.shaped,
      utf16_len: prepared.utf16_len,
      utf16_to_utf8: prepared.utf16_to_utf8,
      layout: None,
      offsets,
      line_width: 0.0,
      font_bounding_box_ascent: 0.0,
      font_bounding_box_descent: 0.0,
      text_align,
      text_baseline,
      rtl,
    };
    let baseline = measurement.origin_y();
    return TextMetrics {
      width: 0.0,
      actual_bounding_box_left: 0.0,
      actual_bounding_box_right: 0.0,
      font_bounding_box_ascent: 0.0,
      font_bounding_box_descent: 0.0,
      actual_bounding_box_ascent: 0.0,
      actual_bounding_box_descent: 0.0,
      em_height_ascent: offsets.em_height_ascent + baseline,
      em_height_descent: offsets.em_height_descent - baseline,
      hanging_baseline: baseline + offsets.hanging_from_alphabetic,
      alphabetic_baseline: baseline,
      ideographic_baseline: baseline + offsets.ideographic_from_alphabetic,
      measurement: Rc::new(measurement),
    };
  }

  let mut fc = font_ctx.borrow_mut();
  let mut lc = layout_ctx.borrow_mut();
  let layout = build_prepared_text_layout(
    &mut fc,
    &mut lc,
    &prepared.shaped,
    fstate,
    lang,
  );

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
  let ink = compute_ink_bounds(&layout, None);
  let anchor = alignment_anchor(width, text_align, rtl);

  let measurement = TextMeasurement {
    shaped: prepared.shaped,
    utf16_len: prepared.utf16_len,
    utf16_to_utf8: prepared.utf16_to_utf8,
    layout: Some(layout),
    offsets,
    line_width: width,
    font_bounding_box_ascent: font_bb_ascent,
    font_bounding_box_descent: font_bb_descent,
    text_align,
    text_baseline,
    rtl,
  };
  // Every y-direction metric is measured from the `textBaseline` line, which
  // sits `baseline` below the alphabetic baseline.
  let baseline = measurement.origin_y();

  let (actual_left, actual_right, actual_ascent, actual_descent) =
    if ink.is_empty() {
      (anchor, width - anchor, font_bb_ascent, font_bb_descent)
    } else {
      // Positive left = ink left of the alignment point.
      (
        anchor - ink.min_x,
        ink.max_x - anchor,
        ink.max_y,
        -ink.min_y,
      )
    };

  TextMetrics {
    width,
    actual_bounding_box_left: actual_left,
    actual_bounding_box_right: actual_right,
    font_bounding_box_ascent: font_bb_ascent + baseline,
    font_bounding_box_descent: font_bb_descent - baseline,
    actual_bounding_box_ascent: actual_ascent + baseline,
    actual_bounding_box_descent: actual_descent - baseline,
    em_height_ascent: offsets.em_height_ascent + baseline,
    em_height_descent: offsets.em_height_descent - baseline,
    hanging_baseline: baseline + offsets.hanging_from_alphabetic,
    alphabetic_baseline: baseline,
    ideographic_baseline: baseline + offsets.ideographic_from_alphabetic,
    measurement: Rc::new(measurement),
  }
}
