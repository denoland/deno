// Copyright 2018-2026 the Deno authors. MIT license.

//! On Windows, Rust's `std::io::Stdout` rejects non-UTF-8 bytes when it is
//! attached to a console that is not in UTF-8 mode. Because `Stdout` is
//! wrapped in a `LineWriter`, the rejected bytes stay in its buffer and every
//! later flush fails, so a single bad write permanently breaks stdout and
//! makes `println!` panic (https://github.com/denoland/deno/issues/20134).
//!
//! To avoid that, the bytes written to stdout are validated before they reach
//! `Stdout`, and only complete UTF-8 characters are passed on. A character
//! split across writes is held back here until it is complete, so std never
//! holds an incomplete character that a following `println!` would fail on.

use std::io;

pub struct ConsoleUtf8Writer {
  incomplete: [u8; 4],
  len: usize,
}

impl ConsoleUtf8Writer {
  pub const fn new() -> Self {
    Self {
      incomplete: [0; 4],
      len: 0,
    }
  }

  /// Passes the complete UTF-8 characters of `buf`, following the bytes held
  /// back from previous writes, to `write_all`, and holds back a trailing
  /// incomplete character. On invalid UTF-8, nothing is written, the held
  /// back bytes are dropped (as std does) and an `InvalidData` error is
  /// returned.
  pub fn write(
    &mut self,
    buf: &[u8],
    mut write_all: impl FnMut(&[u8]) -> io::Result<()>,
  ) -> io::Result<()> {
    let mut rest = buf;
    let mut completed = None;
    if self.len > 0 {
      let width = utf8_char_width(self.incomplete[0]);
      let needed = width - self.len;
      let (head, tail) = rest.split_at(needed.min(rest.len()));
      if head.iter().any(|byte| byte >> 6 != 0b10) {
        // not a continuation byte
        return Err(self.reject());
      }
      let mut bytes = self.incomplete;
      bytes[self.len..self.len + head.len()].copy_from_slice(head);
      if head.len() < needed {
        // still incomplete
        self.incomplete = bytes;
        self.len += head.len();
        return Ok(());
      }
      if std::str::from_utf8(&bytes[..width]).is_err() {
        return Err(self.reject());
      }
      completed = Some((bytes, width));
      rest = tail;
    }

    let (valid, tail) = match std::str::from_utf8(rest) {
      Ok(_) => (rest, &[][..]),
      Err(err) if err.error_len().is_none() => rest.split_at(err.valid_up_to()),
      Err(_) => return Err(self.reject()),
    };
    self.incomplete[..tail.len()].copy_from_slice(tail);
    self.len = tail.len();
    if let Some((bytes, width)) = completed {
      write_all(&bytes[..width])?;
    }
    if !valid.is_empty() {
      write_all(valid)?;
    }
    Ok(())
  }

  fn reject(&mut self) -> io::Error {
    self.len = 0;
    // same error std returns, so the behavior matches writing to `Stdout`
    io::Error::new(
      io::ErrorKind::InvalidData,
      "Windows stdio in console mode does not support writing non-UTF-8 byte sequences",
    )
  }
}

fn utf8_char_width(first_byte: u8) -> usize {
  match first_byte {
    0xC2..=0xDF => 2,
    0xE0..=0xEF => 3,
    0xF0..=0xF4 => 4,
    _ => 1,
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  /// Writes each buffer, returning whether it was accepted and every chunk
  /// that was passed on.
  fn write_each(writes: &[&[u8]]) -> (Vec<bool>, Vec<Vec<u8>>) {
    let mut writer = ConsoleUtf8Writer::new();
    let mut output = Vec::new();
    let accepted = writes
      .iter()
      .map(|buf| {
        writer
          .write(buf, |bytes| {
            output.push(bytes.to_vec());
            Ok(())
          })
          .is_ok()
      })
      .collect();
    (accepted, output)
  }

  #[test]
  fn passes_valid_utf8() {
    let (accepted, output) = write_each(&[b"hello", "\u{20ac}\n".as_bytes()]);
    assert_eq!(accepted, [true, true]);
    assert_eq!(output, [b"hello".to_vec(), "\u{20ac}\n".into()]);
  }

  #[test]
  fn rejects_invalid_bytes() {
    assert_eq!(write_each(&[&[128]]), (vec![false], vec![]));
    assert_eq!(write_each(&[b"abc\x80"]), (vec![false], vec![]));
    assert_eq!(write_each(&[&[0xFF, b'a']]), (vec![false], vec![]));
    let err = ConsoleUtf8Writer::new()
      .write(&[128], |_| Ok(()))
      .unwrap_err();
    assert_eq!(err.kind(), io::ErrorKind::InvalidData);
  }

  #[test]
  fn recovers_after_rejected_write() {
    let (accepted, output) = write_each(&[&[128], b"ok\n"]);
    assert_eq!(accepted, [false, true]);
    assert_eq!(output, [b"ok\n".to_vec()]);
  }

  #[test]
  fn holds_back_character_split_across_writes() {
    // U+20AC EURO SIGN is E2 82 AC
    let (accepted, output) = write_each(&[&[0xE2], &[0x82], &[0xAC, b'\n']]);
    assert_eq!(accepted, [true, true, true]);
    assert_eq!(output, ["\u{20ac}".as_bytes().to_vec(), b"\n".to_vec()]);

    // U+1F600 is F0 9F 98 80
    let (accepted, output) =
      write_each(&[b"a\xE2\x82", b"\xACb", &[0xF0, 0x9F], &[0x98, 0x80, b'x']]);
    assert_eq!(accepted, [true, true, true, true]);
    assert_eq!(
      output,
      [
        b"a".to_vec(),
        "\u{20ac}".into(),
        b"b".to_vec(),
        "\u{1f600}".into(),
        b"x".to_vec(),
      ]
    );
  }

  #[test]
  fn rejects_broken_split_character() {
    // the incomplete character is dropped after the rejected write
    let (accepted, output) = write_each(&[&[0xE2], b"a", b"b"]);
    assert_eq!(accepted, [true, false, true]);
    assert_eq!(output, [b"b".to_vec()]);

    // overlong encoding completed by the next write
    let (accepted, output) = write_each(&[&[0xE0], &[0x80, 0x80], b"c"]);
    assert_eq!(accepted, [true, false, true]);
    assert_eq!(output, [b"c".to_vec()]);
  }
}
