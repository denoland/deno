// Copyright 2018-2026 the Deno authors. MIT license.

//! CSS system colors, resolved to fixed values because canvas has no style
//! context to derive them from. The values match a light color scheme in
//! major browsers; WPT only requires that they resolve to *some* color.
//!
//! https://www.w3.org/TR/css-color-4/#css-system-colors

use cssparser::match_ignore_ascii_case;

/// Returns whether `s` is a CSS system color keyword, including the
/// deprecated ones.
#[inline]
pub fn is_css_system_color(s: &str) -> bool {
  lookup(s.trim()).is_some()
}

/// Resolves a system color keyword to an opaque sRGB value.
pub(super) fn lookup(ident: &str) -> Option<(u8, u8, u8)> {
  const WHITE: (u8, u8, u8) = (0xff, 0xff, 0xff);
  const BLACK: (u8, u8, u8) = (0x00, 0x00, 0x00);
  const ACCENT: (u8, u8, u8) = (0x00, 0x75, 0xff);
  const BUTTON_BORDER: (u8, u8, u8) = (0x76, 0x76, 0x76);
  const BUTTON_FACE: (u8, u8, u8) = (0xef, 0xef, 0xef);
  const GRAY_TEXT: (u8, u8, u8) = (0x80, 0x80, 0x80);
  const ACTIVE_TEXT: (u8, u8, u8) = (0xff, 0x00, 0x00);
  const LINK_TEXT: (u8, u8, u8) = (0x00, 0x00, 0xee);
  const MARK: (u8, u8, u8) = (0xff, 0xff, 0x00);
  const VISITED_TEXT: (u8, u8, u8) = (0x55, 0x1a, 0x8b);

  Some(match_ignore_ascii_case! { ident,
    "accentcolor" => ACCENT,
    "accentcolortext" => WHITE,
    "activetext" => ACTIVE_TEXT,
    "buttonborder" => BUTTON_BORDER,
    "buttonface" => BUTTON_FACE,
    "buttontext" => BLACK,
    "canvas" => WHITE,
    "canvastext" => BLACK,
    "field" => WHITE,
    "fieldtext" => BLACK,
    "graytext" => GRAY_TEXT,
    "highlight" => ACCENT,
    "highlighttext" => WHITE,
    "linktext" => LINK_TEXT,
    "mark" => MARK,
    "marktext" => BLACK,
    "selecteditem" => ACCENT,
    "selecteditemtext" => WHITE,
    "visitedtext" => VISITED_TEXT,
    // Deprecated system colors map to their modern equivalents.
    // https://www.w3.org/TR/css-color-4/#deprecated-system-colors
    "activeborder" | "inactiveborder" | "threeddarkshadow"
    | "threedhighlight" | "threedlightshadow" | "threedshadow"
    | "windowframe" => BUTTON_BORDER,
    "activecaption" | "appworkspace" | "background" | "inactivecaption"
    | "infobackground" | "menu" | "scrollbar" | "window" => WHITE,
    "buttonhighlight" | "buttonshadow" | "threedface" => BUTTON_FACE,
    "captiontext" | "infotext" | "menutext" | "windowtext" => BLACK,
    "inactivecaptiontext" => GRAY_TEXT,
    _ => return None,
  })
}
