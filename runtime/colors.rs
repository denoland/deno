// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
use regex::Regex;
use std::env;
use std::fmt;
use std::io::Write;
use termcolor::Color::{Ansi256, Black, Blue, Cyan, Green, Red, White, Yellow};
use termcolor::{Ansi, ColorSpec, WriteColor};

#[cfg(windows)]
use termcolor::{BufferWriter, ColorChoice};

lazy_static::lazy_static! {
  // STRIP_ANSI_RE and strip_ansi_codes are lifted from the "console" crate.
  // Copyright 2017 Armin Ronacher <armin.ronacher@active-4.com>. MIT License.
  static ref STRIP_ANSI_RE: Regex = Regex::new(
          r"[\x1b\x9b][\[()#;?]*(?:[0-9]{1,4}(?:;[0-9]{0,4})*)?[0-9A-PRZcf-nqry=><]"
  ).unwrap();
  static ref NO_COLOR: bool = env::var_os("NO_COLOR").is_some();
}

/// Helper function to strip ansi codes.
#[cfg(test)]
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

pub fn red_bold(s: &str) -> impl fmt::Display {
  let mut style_spec = ColorSpec::new();
  style_spec.set_fg(Some(Red)).set_bold(true);
  style(&s, style_spec)
}

pub fn green_bold(s: &str) -> impl fmt::Display {
  let mut style_spec = ColorSpec::new();
  style_spec.set_fg(Some(Green)).set_bold(true);
  style(&s, style_spec)
}

pub fn italic_bold(s: &str) -> impl fmt::Display {
  let mut style_spec = ColorSpec::new();
  style_spec.set_bold(true).set_italic(true);
  style(&s, style_spec)
}

pub fn white_on_red(s: &str) -> impl fmt::Display {
  let mut style_spec = ColorSpec::new();
  style_spec.set_bg(Some(Red)).set_fg(Some(White));
  style(&s, style_spec)
}

pub fn black_on_green(s: &str) -> impl fmt::Display {
  let mut style_spec = ColorSpec::new();
  style_spec.set_bg(Some(Green)).set_fg(Some(Black));
  style(&s, style_spec)
}

pub fn yellow(s: &str) -> impl fmt::Display {
  let mut style_spec = ColorSpec::new();
  style_spec.set_fg(Some(Yellow));
  style(&s, style_spec)
}

pub fn cyan(s: &str) -> impl fmt::Display {
  let mut style_spec = ColorSpec::new();
  style_spec.set_fg(Some(Cyan));
  style(&s, style_spec)
}

pub fn red(s: &str) -> impl fmt::Display {
  let mut style_spec = ColorSpec::new();
  style_spec.set_fg(Some(Red));
  style(&s, style_spec)
}

pub fn green(s: &str) -> impl fmt::Display {
  let mut style_spec = ColorSpec::new();
  style_spec.set_fg(Some(Green));
  style(&s, style_spec)
}

pub fn bold(s: &str) -> impl fmt::Display {
  let mut style_spec = ColorSpec::new();
  style_spec.set_bold(true);
  style(&s, style_spec)
}

pub fn gray(s: &str) -> impl fmt::Display {
  let mut style_spec = ColorSpec::new();
  style_spec.set_fg(Some(Ansi256(8)));
  style(&s, style_spec)
}

pub fn italic_bold_gray(s: &str) -> impl fmt::Display {
  let mut style_spec = ColorSpec::new();
  style_spec
    .set_fg(Some(Ansi256(8)))
    .set_bold(true)
    .set_italic(true);
  style(&s, style_spec)
}

pub fn intense_blue(s: &str) -> impl fmt::Display {
  let mut style_spec = ColorSpec::new();
  style_spec.set_fg(Some(Blue)).set_intense(true);
  style(&s, style_spec)
}
