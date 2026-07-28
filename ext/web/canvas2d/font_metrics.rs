// Copyright 2018-2026 the Deno authors. MIT license.

use fontique::Attributes;
use fontique::QueryFamily;
use fontique::QueryStatus;
use parley::FontContext;
use parley::style::GenericFamily;

use crate::css::font::FontState;
use crate::css::value::FontMetrics;
use crate::css::value::LengthResolution;

/// `ch` is the advance measure of this glyph.
/// https://www.w3.org/TR/css-values-4/#ch
const CH_SAMPLE: char = '0';

/// `ic` is the advance measure of this glyph.
/// https://www.w3.org/TR/css-values-4/#ic
const IC_SAMPLE: char = '\u{6c34}';

/// Builds the `<length>` resolution context for a font state.
///
/// Canvas has no root element, so root-relative units resolve against the same
/// metrics, matching how Blink builds its canvas `CSSToLengthConversionData`.
pub(super) fn length_resolution(
  font_ctx: &mut FontContext,
  fstate: &FontState,
) -> LengthResolution {
  LengthResolution::new(font_metrics(
    font_ctx,
    &fstate.families,
    Attributes {
      width: fstate.stretch.to_parley(),
      style: fstate.style.to_parley(),
      weight: parley::FontWeight::new(fstate.weight as f32),
    },
    fstate.size as f64,
  ))
}

/// Reads the metrics that font-relative `<length>` units resolve against.
///
/// `cap`, `ex` and `lh` come from the first available font. `ch` and `ic` scan
/// further down the family list for a face that actually has the sample glyph,
/// the way Blink's `PrimaryFontWithDigitZero` / `PrimaryFontWithCjkWater` do;
/// anything still missing falls back to the ratios CSS mandates.
/// https://www.w3.org/TR/css-values-4/#font-relative-lengths
pub(super) fn font_metrics(
  font_ctx: &mut FontContext,
  families: &[String],
  attrs: Attributes,
  size: f64,
) -> FontMetrics {
  let mut metrics = FontMetrics::fallback(size);
  let mut first_available = true;
  let mut ch = None;
  let mut ic = None;

  let FontContext {
    collection,
    source_cache,
  } = font_ctx;
  let mut query = collection.query(source_cache);
  if families.is_empty() {
    query.set_families([QueryFamily::Generic(GenericFamily::SansSerif)]);
  } else {
    let names = families
      .iter()
      .map(|name| match GenericFamily::parse(name) {
        Some(generic) => QueryFamily::Generic(generic),
        None => QueryFamily::Named(name.as_str()),
      });
    query.set_families(names);
  }
  query.set_attributes(attrs);
  query.matches_with(|font| {
    let Ok(face) = ttf_parser::Face::parse(font.blob.as_ref(), font.index)
    else {
      return QueryStatus::Continue;
    };
    let units = f64::from(face.units_per_em());
    if units <= 0.0 {
      return QueryStatus::Continue;
    }
    let scale = size / units;

    if first_available {
      first_available = false;
      metrics.cap = match face.capital_height() {
        Some(cap) => f64::from(cap) * scale,
        // No OS/2 sCapHeight: the spec says to use the font's ascent.
        None => f64::from(face.ascender()) * scale,
      };
      if let Some(ex) = face.x_height() {
        metrics.ex = f64::from(ex) * scale;
      }
      // The canvas font shorthand forces 'line-height' to normal, so `lh` is
      // always the face's own line spacing.
      let line = f64::from(face.ascender()) - f64::from(face.descender())
        + f64::from(face.line_gap());
      if line > 0.0 {
        metrics.lh = line * scale;
      }
    }

    if ch.is_none() {
      ch = glyph_advance(&face, CH_SAMPLE).map(|advance| advance * scale);
    }
    if ic.is_none() {
      ic = glyph_advance(&face, IC_SAMPLE).map(|advance| advance * scale);
    }
    if ch.is_some() && ic.is_some() {
      QueryStatus::Stop
    } else {
      QueryStatus::Continue
    }
  });

  if let Some(ch) = ch {
    metrics.ch = ch;
  }
  if let Some(ic) = ic {
    metrics.ic = ic;
  }
  metrics
}

fn glyph_advance(face: &ttf_parser::Face<'_>, c: char) -> Option<f64> {
  let glyph = face.glyph_index(c)?;
  Some(f64::from(face.glyph_hor_advance(glyph)?))
}
