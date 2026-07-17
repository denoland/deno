// Copyright 2018-2026 the Deno authors. MIT license.

//! Origin color channel resolution for the CSS relative color syntax.
//!
//! https://www.w3.org/TR/css-color-5/#relative-colors

use color::ColorSpaceTag;
use color::DynamicColor;
use color::Missing;

use crate::css::value::ChannelKeywords;

/// Channel values of an origin color converted into a color function's color
/// space, exposed to component parsing as `<number>` keywords.
#[derive(Clone, Copy, Debug)]
pub(super) struct OriginChannels {
  keywords: ChannelKeywords,
  names: [&'static str; 3],
  missing: Missing,
  alpha: f32,
}

impl OriginChannels {
  /// Converts `origin` into `tag` and captures its channels under `names`
  /// (plus `alpha`). `scale` maps stored channel values to the keyword
  /// number scale (e.g. `255.0` for `rgb()`, whose components are stored as
  /// `0..1` but expose `0..255` numbers).
  pub(super) fn new(
    origin: DynamicColor,
    tag: ColorSpaceTag,
    names: [&'static str; 3],
    scale: f64,
  ) -> Self {
    let converted = origin.convert(tag);
    let missing = converted.flags.missing();
    // Missing origin components resolve to 0 inside math functions; a bare
    // keyword instead carries the missing component forward (handled by
    // `parse_component`).
    let component = |i: usize| {
      if missing.contains(i) {
        0.0
      } else {
        converted.components[i] as f64
      }
    };
    let keywords = ChannelKeywords::new([
      Some((names[0], component(0) * scale)),
      Some((names[1], component(1) * scale)),
      Some((names[2], component(2) * scale)),
      Some(("alpha", component(3))),
    ]);
    Self {
      keywords,
      names,
      missing,
      alpha: converted.components[3],
    }
  }

  #[inline]
  pub(super) fn keywords(&self) -> ChannelKeywords {
    self.keywords
  }

  /// Whether `ident` is a channel keyword whose origin component is missing.
  #[inline]
  pub(super) fn is_missing_channel(&self, ident: &str) -> bool {
    if ident.eq_ignore_ascii_case("alpha") {
      return self.missing.contains(3);
    }
    self
      .names
      .iter()
      .position(|name| ident.eq_ignore_ascii_case(name))
      .is_some_and(|i| self.missing.contains(i))
  }

  /// The default for an omitted alpha, which is the origin color's alpha.
  #[inline]
  pub(super) fn default_alpha(&self) -> super::parse::Component {
    if self.missing.contains(3) {
      super::parse::Component::None
    } else {
      super::parse::Component::Number(self.alpha as f64)
    }
  }
}
