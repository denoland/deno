// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use regex::Regex;
use std::env;
use std::fmt;
use std::io::Write;
use termcolor::Color::{Ansi256, Black, Magenta, Red, White};
use termcolor::{Ansi, ColorSpec, WriteColor};

#[cfg(windows)]
use termcolor::{BufferWriter, ColorChoice};

lazy_static! {
        // STRIP_ANSI_RE and strip_ansi_codes are lifted from the "console" crate.
        // Copyright 2017 Armin Ronacher <armin.ronacher@active-4.com>. MIT License.
        static ref STRIP_ANSI_RE: Regex = Regex::new(
                r"[\x1b\x9b][\[()#;?]*(?:[0-9]{1,4}(?:;[0-9]{0,4})*)?[0-9A-PRZcf-nqry=><]"
        ).unwrap();
        static ref NO_COLOR: bool = {
                env::var_os("NO_COLOR").is_some()
        };
}

/// Helper function to strip ansi codes.
pub fn strip_ansi_codes(s: &str) -> std::borrow::Cow<str> {
  STRIP_ANSI_RE.replace_all(s, "")
}

pub fn use_color() -> bool {
  !(*NO_COLOR)
}

#[cfg(windows)]
pub fn enable_ansi() {
  BufferWriter::stdout(ColorChoice::AlwaysAnsi);
}

fn style(s: &str, colorspec: ColorSpec) -> impl fmt::Display {
  if !use_color() {
    return String::from(s);
  }
  let mut v = Vec::new();
  let mut ansi_writer = Ansi::new(&mut v);
  ansi_writer.set_color(&colorspec).unwrap();
  ansi_writer.write_all(s.as_bytes()).unwrap();
  ansi_writer.reset().unwrap();
  String::from_utf8_lossy(&v).into_owned()
}

pub fn red_bold(s: String) -> impl fmt::Display {
  let mut style_spec = ColorSpec::new();
  style_spec.set_fg(Some(Red)).set_bold(true);
  style(&s, style_spec)
}

pub fn green_bold(s: String) -> impl fmt::Display {
  let mut style_spec = ColorSpec::new();
  style_spec.set_fg(Some(Ansi256(10))).set_bold(true);
  style(&s, style_spec)
}

pub fn italic_bold(s: String) -> impl fmt::Display {
  let mut style_spec = ColorSpec::new();
  style_spec.set_bold(true).set_italic(true);
  style(&s, style_spec)
}

pub fn black_on_white(s: String) -> impl fmt::Display {
  let mut style_spec = ColorSpec::new();
  style_spec.set_bg(Some(White)).set_fg(Some(Black));
  style(&s, style_spec)
}

pub fn white_on_red(s: String) -> impl fmt::Display {
  let mut style_spec = ColorSpec::new();
  style_spec.set_bg(Some(Red)).set_fg(Some(White));
  style(&s, style_spec)
}

pub fn white_on_green(s: String) -> impl fmt::Display {
  let mut style_spec = ColorSpec::new();
  style_spec.set_bg(Some(Ansi256(10))).set_fg(Some(White));
  style(&s, style_spec)
}

pub fn yellow(s: String) -> impl fmt::Display {
  let mut style_spec = ColorSpec::new();
  style_spec.set_fg(Some(Ansi256(11)));
  style(&s, style_spec)
}

pub fn cyan(s: String) -> impl fmt::Display {
  let mut style_spec = ColorSpec::new();
  style_spec.set_fg(Some(Ansi256(14)));
  style(&s, style_spec)
}

pub fn red(s: String) -> impl fmt::Display {
  let mut style_spec = ColorSpec::new();
  style_spec.set_fg(Some(Red));
  style(&s, style_spec)
}

pub fn green(s: String) -> impl fmt::Display {
  let mut style_spec = ColorSpec::new();
  style_spec.set_fg(Some(Ansi256(10)));
  style(&s, style_spec)
}

pub fn magenta(s: String) -> impl fmt::Display {
  let mut style_spec = ColorSpec::new();
  style_spec.set_fg(Some(Magenta));
  style(&s, style_spec)
}

pub fn bold(s: String) -> impl fmt::Display {
  let mut style_spec = ColorSpec::new();
  style_spec.set_bold(true);
  style(&s, style_spec)
}

pub fn gray(s: String) -> impl fmt::Display {
  let mut style_spec = ColorSpec::new();
  style_spec.set_fg(Some(Ansi256(8)));
  style(&s, style_spec)
}

pub fn italic_bold_gray(s: String) -> impl fmt::Display {
  let mut style_spec = ColorSpec::new();
  style_spec
    .set_fg(Some(Ansi256(8)))
    .set_bold(true)
    .set_italic(true);
  style(&s, style_spec)
}
