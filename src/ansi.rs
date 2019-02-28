// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use ansi_term::Color::Fixed;
use ansi_term::Color::Red;
use ansi_term::Style;
use regex::Regex;
use std::borrow::Cow;
use std::env;
use std::fmt;

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
#[allow(dead_code)]
pub fn strip_ansi_codes(s: &str) -> Cow<str> {
  STRIP_ANSI_RE.replace_all(s, "")
}

pub fn use_color() -> bool {
  !(*NO_COLOR)
}

pub fn red_bold(s: String) -> impl fmt::Display {
  let mut style = Style::new();
  if use_color() {
    style = style.bold().fg(Red);
  }
  style.paint(s)
}

pub fn italic_bold(s: String) -> impl fmt::Display {
  let mut style = Style::new();
  if use_color() {
    style = style.italic().bold();
  }
  style.paint(s)
}

pub fn yellow(s: String) -> impl fmt::Display {
  let mut style = Style::new();
  if use_color() {
    // matches TypeScript's ForegroundColorEscapeSequences.Yellow
    style = style.fg(Fixed(11));
  }
  style.paint(s)
}

pub fn cyan(s: String) -> impl fmt::Display {
  let mut style = Style::new();
  if use_color() {
    // matches TypeScript's ForegroundColorEscapeSequences.Cyan
    style = style.fg(Fixed(14));
  }
  style.paint(s)
}

pub fn bold(s: String) -> impl fmt::Display {
  let mut style = Style::new();
  if use_color() {
    style = style.bold();
  }
  style.paint(s)
}
