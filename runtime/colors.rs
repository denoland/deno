// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use atty;
use once_cell::sync::Lazy;
use std::fmt;
use std::io::Write;
use termcolor::Ansi;
use termcolor::Color::Ansi256;
use termcolor::Color::Black;
use termcolor::Color::Blue;
use termcolor::Color::Cyan;
use termcolor::Color::Green;
use termcolor::Color::Magenta;
use termcolor::Color::Red;
use termcolor::Color::White;
use termcolor::Color::Yellow;
use termcolor::ColorSpec;
use termcolor::WriteColor;

#[cfg(windows)]
use termcolor::BufferWriter;
#[cfg(windows)]
use termcolor::ColorChoice;

static NO_COLOR: Lazy<bool> =
  Lazy::new(|| std::env::var_os("NO_COLOR").is_some());

static IS_TTY: Lazy<bool> = Lazy::new(|| atty::is(atty::Stream::Stdout));

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum TTYColorLevel {
  None,
  Basic,
  Ansi256,
  TrueColor,
}

static SUPPORT_LEVEL: Lazy<TTYColorLevel> = Lazy::new(|| {
  if *NO_COLOR {
    return TTYColorLevel::None;
  }

  fn get_os_env_var(var_name: &str) -> Option<String> {
    let var = std::env::var_os(var_name);

    var.and_then(|s| {
      let maybe_str = s.to_str();
      maybe_str.map(|s| s.to_string())
    })
  }

  detect_color_support(get_os_env_var)
});

fn detect_color_support(
  get_env_var: impl Fn(&str) -> Option<String>,
) -> TTYColorLevel {
  // Windows supports 24bit True Colors since Windows 10 #14931,
  // see https://devblogs.microsoft.com/commandline/24-bit-color-in-the-windows-console/
  if cfg!(target_os = "windows") {
    return TTYColorLevel::TrueColor;
  }

  if let Some(color_term) = get_env_var("COLORTERM") {
    if color_term == "truecolor" || color_term == "24bit" {
      return TTYColorLevel::TrueColor;
    }
  }

  if let Some(term) = get_env_var("TERM") {
    if term.ends_with("256") || term.ends_with("256color") {
      return TTYColorLevel::Ansi256;
    }

    // CI systems commonly set TERM=dumb although they support
    // full colors. They usually do their own mapping.
    if get_env_var("CI").is_some() {
      return TTYColorLevel::TrueColor;
    }

    if term != "dumb" {
      return TTYColorLevel::Basic;
    }
  }

  TTYColorLevel::None
}

pub fn is_tty() -> bool {
  *IS_TTY
}

pub fn use_color() -> bool {
  !(*NO_COLOR)
}

pub fn use_color_support_level() -> TTYColorLevel {
  *SUPPORT_LEVEL
}

#[cfg(windows)]
pub fn enable_ansi() {
  BufferWriter::stdout(ColorChoice::AlwaysAnsi);
}

fn style<S: AsRef<str>>(s: S, colorspec: ColorSpec) -> impl fmt::Display {
  if !use_color() {
    return String::from(s.as_ref());
  }
  let mut v = Vec::new();
  let mut ansi_writer = Ansi::new(&mut v);
  ansi_writer.set_color(&colorspec).unwrap();
  ansi_writer.write_all(s.as_ref().as_bytes()).unwrap();
  ansi_writer.reset().unwrap();
  String::from_utf8_lossy(&v).into_owned()
}

pub fn red_bold<S: AsRef<str>>(s: S) -> impl fmt::Display {
  let mut style_spec = ColorSpec::new();
  style_spec.set_fg(Some(Red)).set_bold(true);
  style(s, style_spec)
}

pub fn green_bold<S: AsRef<str>>(s: S) -> impl fmt::Display {
  let mut style_spec = ColorSpec::new();
  style_spec.set_fg(Some(Green)).set_bold(true);
  style(s, style_spec)
}

pub fn italic<S: AsRef<str>>(s: S) -> impl fmt::Display {
  let mut style_spec = ColorSpec::new();
  style_spec.set_italic(true);
  style(s, style_spec)
}

pub fn italic_gray<S: AsRef<str>>(s: S) -> impl fmt::Display {
  let mut style_spec = ColorSpec::new();
  style_spec.set_fg(Some(Ansi256(8))).set_italic(true);
  style(s, style_spec)
}

pub fn italic_bold<S: AsRef<str>>(s: S) -> impl fmt::Display {
  let mut style_spec = ColorSpec::new();
  style_spec.set_bold(true).set_italic(true);
  style(s, style_spec)
}

pub fn white_on_red<S: AsRef<str>>(s: S) -> impl fmt::Display {
  let mut style_spec = ColorSpec::new();
  style_spec.set_bg(Some(Red)).set_fg(Some(White));
  style(s, style_spec)
}

pub fn black_on_green<S: AsRef<str>>(s: S) -> impl fmt::Display {
  let mut style_spec = ColorSpec::new();
  style_spec.set_bg(Some(Green)).set_fg(Some(Black));
  style(s, style_spec)
}

pub fn yellow<S: AsRef<str>>(s: S) -> impl fmt::Display {
  let mut style_spec = ColorSpec::new();
  style_spec.set_fg(Some(Yellow));
  style(s, style_spec)
}

pub fn cyan<S: AsRef<str>>(s: S) -> impl fmt::Display {
  let mut style_spec = ColorSpec::new();
  style_spec.set_fg(Some(Cyan));
  style(s, style_spec)
}
pub fn cyan_bold<S: AsRef<str>>(s: S) -> impl fmt::Display {
  let mut style_spec = ColorSpec::new();
  style_spec.set_fg(Some(Cyan)).set_bold(true);
  style(s, style_spec)
}

pub fn magenta<S: AsRef<str>>(s: S) -> impl fmt::Display {
  let mut style_spec = ColorSpec::new();
  style_spec.set_fg(Some(Magenta));
  style(s, style_spec)
}

pub fn red<S: AsRef<str>>(s: S) -> impl fmt::Display {
  let mut style_spec = ColorSpec::new();
  style_spec.set_fg(Some(Red));
  style(s, style_spec)
}

pub fn green<S: AsRef<str>>(s: S) -> impl fmt::Display {
  let mut style_spec = ColorSpec::new();
  style_spec.set_fg(Some(Green));
  style(s, style_spec)
}

pub fn bold<S: AsRef<str>>(s: S) -> impl fmt::Display {
  let mut style_spec = ColorSpec::new();
  style_spec.set_bold(true);
  style(s, style_spec)
}

pub fn gray<S: AsRef<str>>(s: S) -> impl fmt::Display {
  let mut style_spec = ColorSpec::new();
  style_spec.set_fg(Some(Ansi256(245)));
  style(s, style_spec)
}

pub fn intense_blue<S: AsRef<str>>(s: S) -> impl fmt::Display {
  let mut style_spec = ColorSpec::new();
  style_spec.set_fg(Some(Blue)).set_intense(true);
  style(s, style_spec)
}

pub fn white_bold_on_red<S: AsRef<str>>(s: S) -> impl fmt::Display {
  let mut style_spec = ColorSpec::new();
  style_spec
    .set_bold(true)
    .set_bg(Some(Red))
    .set_fg(Some(White));
  style(s, style_spec)
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::collections::HashMap;

  #[cfg(not(windows))]
  #[test]
  fn supports_true_color() {
    let vars = HashMap::from([("COLORTERM", "truecolor")]);
    assert_eq!(
      detect_color_support(|name| vars.get(name).map(|s| s.to_string())),
      TTYColorLevel::TrueColor
    );

    let vars = HashMap::from([("COLORTERM", "24bit")]);
    assert_eq!(
      detect_color_support(|name| vars.get(name).map(|s| s.to_string())),
      TTYColorLevel::TrueColor
    );
  }

  #[cfg(not(windows))]
  #[test]
  fn supports_ansi_256() {
    let vars = HashMap::from([("TERM", "xterm-256")]);
    assert_eq!(
      detect_color_support(|name| vars.get(name).map(|s| s.to_string())),
      TTYColorLevel::Ansi256
    );

    let vars = HashMap::from([("TERM", "xterm-256color")]);
    assert_eq!(
      detect_color_support(|name| vars.get(name).map(|s| s.to_string())),
      TTYColorLevel::Ansi256
    );
  }

  #[cfg(not(windows))]
  #[test]
  fn supports_ci_color() {
    let vars = HashMap::from([("CI", "1"), ("TERM", "dumb")]);
    assert_eq!(
      detect_color_support(|name| vars.get(name).map(|s| s.to_string())),
      TTYColorLevel::TrueColor
    );
  }

  #[cfg(not(windows))]
  #[test]
  fn supports_basic_ansi() {
    let vars = HashMap::from([("TERM", "xterm")]);
    assert_eq!(
      detect_color_support(|name| vars.get(name).map(|s| s.to_string())),
      TTYColorLevel::Basic
    );
  }

  #[cfg(not(windows))]
  #[test]
  fn supports_none() {
    let vars = HashMap::from([("TERM", "dumb")]);
    assert_eq!(
      detect_color_support(|name| vars.get(name).map(|s| s.to_string())),
      TTYColorLevel::None
    );
  }
}
