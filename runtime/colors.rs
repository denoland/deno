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

#[derive(Copy, Clone)]
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

  // Windows supports 24bit True Colors since Windows 10 #14931,
  // see https://devblogs.microsoft.com/commandline/24-bit-color-in-the-windows-console/
  if cfg!(target_os = "windows") {
    return TTYColorLevel::TrueColor;
  }

  if let Some(color_term) = std::env::var_os("COLORTERM") {
    if color_term == "truecolor" || color_term == "24bit" {
      return TTYColorLevel::TrueColor;
    }
  }

  if let Some(term) = std::env::var_os("TERM") {
    let term_value = term.to_str().unwrap();
    if term_value.ends_with("256") || term_value.ends_with("256color") {
      return TTYColorLevel::Ansi256;
    }

    // CI systems commonly set TERM=dumb although they support
    // full colors. They usually do their own mapping.
    if std::env::var_os("CI").is_some() {
      return TTYColorLevel::TrueColor;
    }

    if term != "dumb" {
      return TTYColorLevel::Basic;
    }
  }

  TTYColorLevel::None
});

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
