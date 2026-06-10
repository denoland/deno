// Copyright 2018-2026 the Deno authors. MIT license.

//! `%c` CSS handling ported from `01_console.js`
//! (`parseCssColor`, `parseCss`, `cssToAnsi`).

use serde::Deserialize;
use serde::Serialize;

/// A CSS color as stored on the parsed css object: either a raw (not yet
/// parsed) string or an `[r, g, b]` triple. JS code stores `null` for absent.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CssColor {
  Rgb([u8; 3]),
  Raw(String),
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Css {
  pub background_color: Option<CssColor>,
  pub color: Option<CssColor>,
  pub font_weight: Option<String>,
  pub font_style: Option<String>,
  pub text_decoration_color: Option<[u8; 3]>,
  pub text_decoration_line: Vec<String>,
}

const COLOR_KEYWORDS: &[(&str, &str)] = &[
  ("black", "#000000"),
  ("silver", "#c0c0c0"),
  ("gray", "#808080"),
  ("white", "#ffffff"),
  ("maroon", "#800000"),
  ("red", "#ff0000"),
  ("purple", "#800080"),
  ("fuchsia", "#ff00ff"),
  ("green", "#008000"),
  ("lime", "#00ff00"),
  ("olive", "#808000"),
  ("yellow", "#ffff00"),
  ("navy", "#000080"),
  ("blue", "#0000ff"),
  ("teal", "#008080"),
  ("aqua", "#00ffff"),
  ("orange", "#ffa500"),
  ("aliceblue", "#f0f8ff"),
  ("antiquewhite", "#faebd7"),
  ("aquamarine", "#7fffd4"),
  ("azure", "#f0ffff"),
  ("beige", "#f5f5dc"),
  ("bisque", "#ffe4c4"),
  ("blanchedalmond", "#ffebcd"),
  ("blueviolet", "#8a2be2"),
  ("brown", "#a52a2a"),
  ("burlywood", "#deb887"),
  ("cadetblue", "#5f9ea0"),
  ("chartreuse", "#7fff00"),
  ("chocolate", "#d2691e"),
  ("coral", "#ff7f50"),
  ("cornflowerblue", "#6495ed"),
  ("cornsilk", "#fff8dc"),
  ("crimson", "#dc143c"),
  ("cyan", "#00ffff"),
  ("darkblue", "#00008b"),
  ("darkcyan", "#008b8b"),
  ("darkgoldenrod", "#b8860b"),
  ("darkgray", "#a9a9a9"),
  ("darkgreen", "#006400"),
  ("darkgrey", "#a9a9a9"),
  ("darkkhaki", "#bdb76b"),
  ("darkmagenta", "#8b008b"),
  ("darkolivegreen", "#556b2f"),
  ("darkorange", "#ff8c00"),
  ("darkorchid", "#9932cc"),
  ("darkred", "#8b0000"),
  ("darksalmon", "#e9967a"),
  ("darkseagreen", "#8fbc8f"),
  ("darkslateblue", "#483d8b"),
  ("darkslategray", "#2f4f4f"),
  ("darkslategrey", "#2f4f4f"),
  ("darkturquoise", "#00ced1"),
  ("darkviolet", "#9400d3"),
  ("deeppink", "#ff1493"),
  ("deepskyblue", "#00bfff"),
  ("dimgray", "#696969"),
  ("dimgrey", "#696969"),
  ("dodgerblue", "#1e90ff"),
  ("firebrick", "#b22222"),
  ("floralwhite", "#fffaf0"),
  ("forestgreen", "#228b22"),
  ("gainsboro", "#dcdcdc"),
  ("ghostwhite", "#f8f8ff"),
  ("gold", "#ffd700"),
  ("goldenrod", "#daa520"),
  ("greenyellow", "#adff2f"),
  ("grey", "#808080"),
  ("honeydew", "#f0fff0"),
  ("hotpink", "#ff69b4"),
  ("indianred", "#cd5c5c"),
  ("indigo", "#4b0082"),
  ("ivory", "#fffff0"),
  ("khaki", "#f0e68c"),
  ("lavender", "#e6e6fa"),
  ("lavenderblush", "#fff0f5"),
  ("lawngreen", "#7cfc00"),
  ("lemonchiffon", "#fffacd"),
  ("lightblue", "#add8e6"),
  ("lightcoral", "#f08080"),
  ("lightcyan", "#e0ffff"),
  ("lightgoldenrodyellow", "#fafad2"),
  ("lightgray", "#d3d3d3"),
  ("lightgreen", "#90ee90"),
  ("lightgrey", "#d3d3d3"),
  ("lightpink", "#ffb6c1"),
  ("lightsalmon", "#ffa07a"),
  ("lightseagreen", "#20b2aa"),
  ("lightskyblue", "#87cefa"),
  ("lightslategray", "#778899"),
  ("lightslategrey", "#778899"),
  ("lightsteelblue", "#b0c4de"),
  ("lightyellow", "#ffffe0"),
  ("limegreen", "#32cd32"),
  ("linen", "#faf0e6"),
  ("magenta", "#ff00ff"),
  ("mediumaquamarine", "#66cdaa"),
  ("mediumblue", "#0000cd"),
  ("mediumorchid", "#ba55d3"),
  ("mediumpurple", "#9370db"),
  ("mediumseagreen", "#3cb371"),
  ("mediumslateblue", "#7b68ee"),
  ("mediumspringgreen", "#00fa9a"),
  ("mediumturquoise", "#48d1cc"),
  ("mediumvioletred", "#c71585"),
  ("midnightblue", "#191970"),
  ("mintcream", "#f5fffa"),
  ("mistyrose", "#ffe4e1"),
  ("moccasin", "#ffe4b5"),
  ("navajowhite", "#ffdead"),
  ("oldlace", "#fdf5e6"),
  ("olivedrab", "#6b8e23"),
  ("orangered", "#ff4500"),
  ("orchid", "#da70d6"),
  ("palegoldenrod", "#eee8aa"),
  ("palegreen", "#98fb98"),
  ("paleturquoise", "#afeeee"),
  ("palevioletred", "#db7093"),
  ("papayawhip", "#ffefd5"),
  ("peachpuff", "#ffdab9"),
  ("peru", "#cd853f"),
  ("pink", "#ffc0cb"),
  ("plum", "#dda0dd"),
  ("powderblue", "#b0e0e6"),
  ("rosybrown", "#bc8f8f"),
  ("royalblue", "#4169e1"),
  ("saddlebrown", "#8b4513"),
  ("salmon", "#fa8072"),
  ("sandybrown", "#f4a460"),
  ("seagreen", "#2e8b57"),
  ("seashell", "#fff5ee"),
  ("sienna", "#a0522d"),
  ("skyblue", "#87ceeb"),
  ("slateblue", "#6a5acd"),
  ("slategray", "#708090"),
  ("slategrey", "#708090"),
  ("snow", "#fffafa"),
  ("springgreen", "#00ff7f"),
  ("steelblue", "#4682b4"),
  ("tan", "#d2b48c"),
  ("thistle", "#d8bfd8"),
  ("tomato", "#ff6347"),
  ("turquoise", "#40e0d0"),
  ("violet", "#ee82ee"),
  ("wheat", "#f5deb3"),
  ("whitesmoke", "#f5f5f5"),
  ("yellowgreen", "#9acd32"),
  ("rebeccapurple", "#663399"),
];

fn hex2(s: &str) -> Option<u8> {
  u8::from_str_radix(s, 16).ok()
}

/// `parseCssColor` from 01_console.js. Returns `[r, g, b]` or `None`.
pub fn parse_css_color(color_string: &str) -> Option<[u8; 3]> {
  let mut color_string = color_string.to_lowercase();
  if let Some((_, hex)) =
    COLOR_KEYWORDS.iter().find(|(k, _)| *k == color_string)
  {
    color_string = (*hex).to_string();
  }
  let s = color_string.as_str();

  // HASH_PATTERN: #RRGGBB(AA)?
  if let Some(rest) = s.strip_prefix('#') {
    let bytes: Vec<char> = rest.chars().collect();
    if (bytes.len() == 6 || bytes.len() == 8)
      && bytes.iter().all(|c| c.is_ascii_hexdigit())
    {
      return Some([
        hex2(&rest[0..2])?,
        hex2(&rest[2..4])?,
        hex2(&rest[4..6])?,
      ]);
    }
    // SMALL_HASH_PATTERN: #RGB(A)?
    if (bytes.len() == 3 || bytes.len() == 4)
      && bytes.iter().all(|c| c.is_ascii_hexdigit())
    {
      let d = |c: char| -> Option<u8> {
        let v = c.to_digit(16)? as u8;
        Some(v * 16 + v)
      };
      return Some([d(bytes[0])?, d(bytes[1])?, d(bytes[2])?]);
    }
    return None;
  }

  // RGB_PATTERN: rgba?( n, n, n (, a)? )
  if let Some(args) = parse_fn_args(s, "rgb") {
    if args.len() == 3 || args.len() == 4 {
      let clamp = |v: f64| -> u8 { v.clamp(0.0, 255.0).round_ties_even_js() };
      return Some([clamp(args[0]), clamp(args[1]), clamp(args[2])]);
    }
    return None;
  }

  // HSL_PATTERN: hsla?( h, s%, l% (, a)? )
  if let Some((h_raw, s_pct, l_pct)) = parse_hsl(s) {
    let mut h = h_raw % 360.0;
    if h < 0.0 {
      h += 360.0;
    }
    let sat = s_pct.clamp(0.0, 100.0) / 100.0;
    let l = l_pct.clamp(0.0, 100.0) / 100.0;
    let c = (1.0 - (2.0 * l - 1.0).abs()) * sat;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = l - c / 2.0;
    let (r_, g_, b_) = if h < 60.0 {
      (c, x, 0.0)
    } else if h < 120.0 {
      (x, c, 0.0)
    } else if h < 180.0 {
      (0.0, c, x)
    } else if h < 240.0 {
      (0.0, x, c)
    } else if h < 300.0 {
      (x, 0.0, c)
    } else {
      (c, 0.0, x)
    };
    return Some([
      ((r_ + m) * 255.0).round_ties_even_js(),
      ((g_ + m) * 255.0).round_ties_even_js(),
      ((b_ + m) * 255.0).round_ties_even_js(),
    ]);
  }

  None
}

/// JS `Math.round` rounds half-up (towards +Infinity), unlike Rust's
/// `round()` which rounds half away from zero.
trait JsRound {
  fn round_ties_even_js(self) -> u8;
}
impl JsRound for f64 {
  fn round_ties_even_js(self) -> u8 {
    // Math.round(x) == floor(x + 0.5) for finite x.
    (self + 0.5).floor().clamp(0.0, 255.0) as u8
  }
}

/// Parse `rgb(a)?\(\s*n\s*,\s*n\s*,\s*n\s*(,\s*n\s*)?\)` where each `n` is
/// `[+\-]?\d*\.?\d+`.
fn parse_fn_args(s: &str, name: &str) -> Option<Vec<f64>> {
  let rest = s.strip_prefix(name)?;
  let rest = rest.strip_prefix('a').unwrap_or(rest);
  let rest = rest.strip_prefix('(')?;
  let rest = rest.strip_suffix(')')?;
  let parts: Vec<&str> = rest.split(',').collect();
  if parts.len() < 3 || parts.len() > 4 {
    return None;
  }
  let mut out = Vec::with_capacity(parts.len());
  for p in parts {
    out.push(parse_css_number(p.trim())?);
  }
  Some(out)
}

/// `hsla?\(\s*h\s*,\s*s%\s*,\s*l%\s*(,\s*a\s*)?\)`
fn parse_hsl(s: &str) -> Option<(f64, f64, f64)> {
  let rest = s.strip_prefix("hsl")?;
  let rest = rest.strip_prefix('a').unwrap_or(rest);
  let rest = rest.strip_prefix('(')?;
  let rest = rest.strip_suffix(')')?;
  let parts: Vec<&str> = rest.split(',').collect();
  if parts.len() < 3 || parts.len() > 4 {
    return None;
  }
  let h = parse_css_number(parts[0].trim())?;
  let s_pct = parse_css_number(parts[1].trim().strip_suffix('%')?)?;
  let l_pct = parse_css_number(parts[2].trim().strip_suffix('%')?)?;
  if let Some(alpha) = parts.get(3) {
    parse_css_number(alpha.trim())?;
  }
  Some((h, s_pct, l_pct))
}

/// `[+\-]?\d*\.?\d+`
fn parse_css_number(s: &str) -> Option<f64> {
  if s.is_empty() {
    return None;
  }
  let rest = s.strip_prefix(['+', '-']).unwrap_or(s);
  if rest.is_empty() {
    return None;
  }
  let (int_part, frac_part) = match rest.split_once('.') {
    Some((i, f)) => (i, Some(f)),
    None => (rest, None),
  };
  let digits_ok =
    |p: &str| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit());
  match frac_part {
    Some(f) => {
      if !digits_ok(f) || !(int_part.is_empty() || digits_ok(int_part)) {
        return None;
      }
    }
    None => {
      if !digits_ok(int_part) {
        return None;
      }
    }
  }
  s.parse::<f64>().ok()
}

/// `parseCss` from 01_console.js.
pub fn parse_css(css_string: &str) -> Css {
  let mut css = Css::default();

  let mut raw_entries: Vec<(String, String)> = Vec::new();
  let mut in_value = false;
  let mut current_key: Option<String> = None;
  let mut parentheses_depth = 0u32;
  let mut current_part = String::new();
  for c in css_string.chars() {
    if c == '(' {
      parentheses_depth += 1;
    } else if parentheses_depth > 0 {
      if c == ')' {
        parentheses_depth -= 1;
      }
    } else if in_value {
      if c == ';' {
        let value = current_part.trim().to_string();
        if !value.is_empty() {
          raw_entries.push((current_key.take().unwrap_or_default(), value));
        }
        current_key = None;
        current_part = String::new();
        in_value = false;
        continue;
      }
    } else if c == ':' {
      current_key = Some(current_part.trim().to_string());
      current_part = String::new();
      in_value = true;
      continue;
    }
    current_part.push(c);
  }
  if in_value && parentheses_depth == 0 {
    let value = current_part.trim().to_string();
    if !value.is_empty() {
      raw_entries.push((current_key.take().unwrap_or_default(), value));
    }
  }

  for (key, value) in raw_entries {
    match key.as_str() {
      "background-color" => {
        css.background_color = Some(CssColor::Raw(value));
      }
      "color" => {
        css.color = Some(CssColor::Raw(value));
      }
      "font-weight" => {
        if value == "bold" {
          css.font_weight = Some(value);
        }
      }
      "font-style" => {
        if value == "italic" || value == "oblique" || value == "oblique 14deg" {
          css.font_style = Some("italic".to_string());
        }
      }
      "text-decoration-line" => {
        css.text_decoration_line.clear();
        for line_type in value.split_whitespace() {
          if matches!(line_type, "line-through" | "overline" | "underline") {
            css.text_decoration_line.push(line_type.to_string());
          }
        }
      }
      "text-decoration-color" => {
        if let Some(color) = parse_css_color(&value) {
          css.text_decoration_color = Some(color);
        }
      }
      "text-decoration" => {
        css.text_decoration_color = None;
        css.text_decoration_line.clear();
        for arg in value.split_whitespace() {
          if let Some(color) = parse_css_color(arg) {
            css.text_decoration_color = Some(color);
          } else if matches!(arg, "line-through" | "overline" | "underline") {
            css.text_decoration_line.push(arg.to_string());
          }
        }
      }
      _ => {}
    }
  }

  css
}

/// `cssColorEquals` from 01_console.js: compare colors by resolved `[r,g,b]`
/// when both are strings; otherwise structural equality (mirroring the JS
/// `colorEquals` index-based comparison).
fn css_color_equals(a: &Option<CssColor>, b: &Option<CssColor>) -> bool {
  match (a, b) {
    (Some(CssColor::Raw(s1)), Some(CssColor::Raw(s2))) => {
      parse_css_color(s1) == parse_css_color(s2)
    }
    (None, None) => true,
    (Some(CssColor::Rgb(r1)), Some(CssColor::Rgb(r2))) => r1 == r2,
    // Mixed string/array/None cases: the JS `colorEquals` compares
    // color1?.[0..2] == color2?.[0..2] with `==` coercion; a string vs array
    // comparison is char-vs-number and only equal when both undefined.
    // None vs Some(...) is never equal unless the string indexes are
    // undefined, which can't happen for non-empty strings.
    (Some(CssColor::Raw(s)), None) | (None, Some(CssColor::Raw(s))) => {
      // JS: `color?.[0] == undefined` etc. — true only for empty string.
      s.is_empty()
    }
    (Some(CssColor::Rgb(_)), None) | (None, Some(CssColor::Rgb(_))) => false,
    (Some(CssColor::Raw(s)), Some(CssColor::Rgb(rgb)))
    | (Some(CssColor::Rgb(rgb)), Some(CssColor::Raw(s))) => {
      // JS `==` between a char and a number coerces the char to a number.
      let chars: Vec<char> = s.chars().collect();
      (0..3).all(|i| {
        chars
          .get(i)
          .and_then(|c| c.to_string().parse::<f64>().ok())
          .map(|n| n == rgb[i] as f64)
          .unwrap_or(false)
      })
    }
  }
}

fn write_color(ansi: &mut String, color: &Option<CssColor>, fg: bool) {
  let (named, reset, rgb_code) = if fg {
    (30, "\x1b[39m", 38)
  } else {
    (40, "\x1b[49m", 48)
  };
  match color {
    None => ansi.push_str(reset),
    Some(CssColor::Raw(s)) => {
      let idx = match s.as_str() {
        "black" => Some(0),
        "red" => Some(1),
        "green" => Some(2),
        "yellow" => Some(3),
        "blue" => Some(4),
        "magenta" => Some(5),
        "cyan" => Some(6),
        "white" => Some(7),
        _ => None,
      };
      if let Some(i) = idx {
        ansi.push_str(&format!("\x1b[{}m", named + i));
      } else if let Some([r, g, b]) = parse_css_color(s) {
        ansi.push_str(&format!("\x1b[{};2;{};{};{}m", rgb_code, r, g, b));
      } else {
        ansi.push_str(reset);
      }
    }
    Some(CssColor::Rgb([r, g, b])) => {
      ansi.push_str(&format!("\x1b[{};2;{};{};{}m", rgb_code, r, g, b));
    }
  }
}

/// `cssToAnsi` from 01_console.js.
pub fn css_to_ansi(css: &Css, prev_css: Option<&Css>) -> String {
  let default_css = Css::default();
  let prev = prev_css.unwrap_or(&default_css);
  let mut ansi = String::new();
  if !css_color_equals(&css.background_color, &prev.background_color) {
    write_color(&mut ansi, &css.background_color, false);
  }
  if !css_color_equals(&css.color, &prev.color) {
    write_color(&mut ansi, &css.color, true);
  }
  if css.font_weight != prev.font_weight {
    if css.font_weight.as_deref() == Some("bold") {
      ansi.push_str("\x1b[1m");
    } else {
      ansi.push_str("\x1b[22m");
    }
  }
  if css.font_style != prev.font_style {
    if css.font_style.as_deref() == Some("italic") {
      ansi.push_str("\x1b[3m");
    } else {
      ansi.push_str("\x1b[23m");
    }
  }
  if css.text_decoration_color != prev.text_decoration_color {
    if let Some([r, g, b]) = css.text_decoration_color {
      ansi.push_str(&format!("\x1b[58;2;{};{};{}m", r, g, b));
    } else {
      ansi.push_str("\x1b[59m");
    }
  }
  for (line, on, off) in [
    ("line-through", "\x1b[9m", "\x1b[29m"),
    ("overline", "\x1b[53m", "\x1b[55m"),
    ("underline", "\x1b[4m", "\x1b[24m"),
  ] {
    let has = css.text_decoration_line.iter().any(|l| l == line);
    let prev_has = prev.text_decoration_line.iter().any(|l| l == line);
    if has != prev_has {
      ansi.push_str(if has { on } else { off });
    }
  }
  ansi
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn colors() {
    assert_eq!(parse_css_color("red"), Some([255, 0, 0]));
    assert_eq!(parse_css_color("#f00"), Some([255, 0, 0]));
    assert_eq!(parse_css_color("#ff0000"), Some([255, 0, 0]));
    assert_eq!(parse_css_color("rgb(255, 0, 0)"), Some([255, 0, 0]));
    assert_eq!(parse_css_color("rgba(255, 0, 0, 0.5)"), Some([255, 0, 0]));
    assert_eq!(parse_css_color("hsl(0, 100%, 50%)"), Some([255, 0, 0]));
    assert_eq!(parse_css_color("hsl(120, 100%, 50%)"), Some([0, 255, 0]));
    assert_eq!(parse_css_color("inherit"), None);
  }

  #[test]
  fn parse_css_basic() {
    let css = parse_css("color: red; font-weight: bold");
    assert_eq!(css.color, Some(CssColor::Raw("red".to_string())));
    assert_eq!(css.font_weight.as_deref(), Some("bold"));
  }

  #[test]
  fn ansi() {
    let css = parse_css("color: red");
    assert_eq!(css_to_ansi(&css, None), "\x1b[31m");
    let css = parse_css("color: #ff0000");
    assert_eq!(css_to_ansi(&css, None), "\x1b[38;2;255;0;0m");
  }
}
