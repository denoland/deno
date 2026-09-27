// Copyright 2018-2026 the Deno authors. MIT license.

//! A tiny, hand-rolled, borrow-first serialization format for the snapshot
//! sidecar data.
//!
//! This replaces `bincode` for the two snapshot sidecar call sites. The data is
//! entirely internal to `deno_core` and is versioned by the snapshot hash, so
//! there is no backwards-compatibility requirement — but we still write a
//! format version word up front so a mismatch fails loudly instead of
//! producing garbage.
//!
//! # Why hand-rolled
//!
//! The snapshot blob handed to `JsRuntime::new` is `&'static [u8]`, which means
//! every string in the sidecar can be a *borrow* of the blob rather than a heap
//! copy. `serde`'s `Deserialize` for [`FastString`] cannot express that (it
//! goes through a `&'de str` proxy whose lifetime is tied to the deserializer,
//! not to `'static`), so every `JsRuntime::new` used to copy the entire module
//! table onto the heap. Decoding straight out of a `&'static [u8]` lets us hand
//! back `FastString::Static`/`StaticAscii` variants that point into the blob.
//!
//! # Wire format
//!
//! Everything is little-endian and length-prefixed.
//!
//! - `u8`/`u32`/`i32`/`u64`: fixed-width LE.
//! - `usize`: encoded as `u64`.
//! - `bool`: one byte, `0` or `1`.
//! - `Option<T>`: one tag byte (`0` = none, `1` = some) followed by `T`.
//! - sequences (`Vec`, `HashMap`): `u32` element count followed by the
//!   elements. Maps are encoded as key/value pairs.
//! - byte slices: `u32` length followed by the raw bytes.
//! - strings: a `u32` header whose low 31 bits are the byte length and whose
//!   high bit is set when the string contains non-ASCII bytes, followed by the
//!   UTF-8 bytes. The ASCII bit is computed once at snapshot-creation time (a
//!   cold path) so that rehydration never has to re-scan the string to pick the
//!   right [`FastString`] representation. UTF-8 is validated once at decode.
//! - enums: a `u8` discriminant followed by the payload, if any.

use std::borrow::Cow;
use std::collections::HashMap;

use crate::FastString;

/// Bumped whenever the layout below changes. Snapshots are already keyed by a
/// build hash, so this only exists to turn "silently decoded nonsense" into a
/// clean panic.
pub(crate) const SNAPSHOT_FORMAT_VERSION: u32 = 2;

/// A string longer than this cannot be encoded (the top bit of the length
/// header is the non-ASCII flag).
const MAX_STR_LEN: usize = (1 << 31) - 1;
const NON_ASCII_FLAG: u32 = 1 << 31;

#[derive(Debug, thiserror::Error)]
pub(crate) enum SnapshotDecodeError {
  #[error(
    "snapshot sidecar format version mismatch: expected {expected}, found {found}"
  )]
  VersionMismatch { expected: u32, found: u32 },
  #[error("unexpected end of snapshot sidecar data")]
  UnexpectedEof,
  #[error("invalid UTF-8 in snapshot sidecar data")]
  InvalidUtf8,
  #[error("invalid discriminant {value} for {ty} in snapshot sidecar data")]
  InvalidDiscriminant { ty: &'static str, value: u32 },
  #[error("invalid URL in snapshot sidecar data: {0}")]
  InvalidUrl(String),
}

pub(crate) type SnapshotResult<T> = std::result::Result<T, SnapshotDecodeError>;
use SnapshotResult as Result;

// ---------------------------------------------------------------------------
// Encoder
// ---------------------------------------------------------------------------

/// The write half of the format. Only used while *creating* a snapshot, which
/// is a cold, once-per-build path, so this is deliberately simple.
pub(crate) struct Encoder {
  buf: Vec<u8>,
}

impl Encoder {
  pub fn new() -> Self {
    let mut this = Self {
      buf: Vec::with_capacity(64 * 1024),
    };
    this.u32(SNAPSHOT_FORMAT_VERSION);
    this
  }

  pub fn into_bytes(self) -> Box<[u8]> {
    self.buf.into_boxed_slice()
  }

  pub fn u8(&mut self, v: u8) {
    self.buf.push(v);
  }

  pub fn bool(&mut self, v: bool) {
    self.u8(v as u8);
  }

  pub fn u32(&mut self, v: u32) {
    self.buf.extend_from_slice(&v.to_le_bytes());
  }

  pub fn i32(&mut self, v: i32) {
    self.buf.extend_from_slice(&v.to_le_bytes());
  }

  pub fn u64(&mut self, v: u64) {
    self.buf.extend_from_slice(&v.to_le_bytes());
  }

  pub fn usize(&mut self, v: usize) {
    self.u64(v as u64);
  }

  pub fn bytes(&mut self, v: &[u8]) {
    self.u32(v.len() as u32);
    self.buf.extend_from_slice(v);
  }

  pub fn str(&mut self, v: &str) {
    assert!(
      v.len() <= MAX_STR_LEN,
      "string too long for snapshot sidecar"
    );
    let mut header = v.len() as u32;
    if !v.is_ascii() {
      header |= NON_ASCII_FLAG;
    }
    self.u32(header);
    self.buf.extend_from_slice(v.as_bytes());
  }

  pub fn option<T>(&mut self, v: Option<T>, f: impl FnOnce(&mut Self, T)) {
    match v {
      None => self.u8(0),
      Some(v) => {
        self.u8(1);
        f(self, v);
      }
    }
  }

  pub fn seq<T>(
    &mut self,
    items: impl ExactSizeIterator<Item = T>,
    mut f: impl FnMut(&mut Self, T),
  ) {
    self.u32(items.len() as u32);
    for item in items {
      f(self, item);
    }
  }
}

// ---------------------------------------------------------------------------
// Decoder
// ---------------------------------------------------------------------------

/// The read half of the format.
///
/// The decoder is deliberately pinned to `&'static [u8]` rather than being
/// generic over a lifetime: the snapshot blob *is* `'static`, and pinning it
/// here is what lets every string decode into a `'static` borrow (and therefore
/// into a non-allocating [`FastString`]) without any lifetime plumbing through
/// the sidecar structs.
#[derive(Debug)]
pub(crate) struct Decoder {
  buf: &'static [u8],
  pos: usize,
}

impl Decoder {
  pub fn new(buf: &'static [u8]) -> Result<Self> {
    let mut this = Self { buf, pos: 0 };
    let found = this.u32()?;
    if found != SNAPSHOT_FORMAT_VERSION {
      return Err(SnapshotDecodeError::VersionMismatch {
        expected: SNAPSHOT_FORMAT_VERSION,
        found,
      });
    }
    Ok(this)
  }

  fn take(&mut self, n: usize) -> Result<&'static [u8]> {
    let end = self
      .pos
      .checked_add(n)
      .ok_or(SnapshotDecodeError::UnexpectedEof)?;
    if end > self.buf.len() {
      return Err(SnapshotDecodeError::UnexpectedEof);
    }
    let slice = &self.buf[self.pos..end];
    self.pos = end;
    Ok(slice)
  }

  pub fn u8(&mut self) -> Result<u8> {
    Ok(self.take(1)?[0])
  }

  pub fn bool(&mut self) -> Result<bool> {
    Ok(self.u8()? != 0)
  }

  pub fn u32(&mut self) -> Result<u32> {
    Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
  }

  pub fn i32(&mut self) -> Result<i32> {
    Ok(i32::from_le_bytes(self.take(4)?.try_into().unwrap()))
  }

  pub fn u64(&mut self) -> Result<u64> {
    Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap()))
  }

  pub fn usize(&mut self) -> Result<usize> {
    Ok(self.u64()? as usize)
  }

  pub fn bytes(&mut self) -> Result<&'static [u8]> {
    let len = self.u32()? as usize;
    self.take(len)
  }

  /// Decodes a string as a borrow of the snapshot blob. UTF-8 is validated
  /// here, once; the ASCII-ness of the string is carried in the length header
  /// so callers never have to re-scan it.
  pub fn str(&mut self) -> Result<(&'static str, bool)> {
    let header = self.u32()?;
    let is_ascii = header & NON_ASCII_FLAG == 0;
    let bytes = self.take((header & !NON_ASCII_FLAG) as usize)?;
    let s = std::str::from_utf8(bytes)
      .map_err(|_| SnapshotDecodeError::InvalidUtf8)?;
    Ok((s, is_ascii))
  }

  /// Decodes a string into a [`FastString`] that borrows the snapshot blob —
  /// no allocation, no copy.
  pub fn fast_string(&mut self) -> Result<FastString> {
    let (s, is_ascii) = self.str()?;
    Ok(if is_ascii {
      // SAFETY: the encoder set the ASCII flag only for ASCII-only strings, and
      // `str()` has already validated that the bytes are well-formed UTF-8.
      unsafe { FastString::from_ascii_static_unchecked(s) }
    } else {
      FastString::from_non_ascii_static(s)
    })
  }

  /// Decodes a string as a borrowed `Cow`, again pointing into the blob.
  pub fn cow_str(&mut self) -> Result<Cow<'static, str>> {
    Ok(Cow::Borrowed(self.str()?.0))
  }

  pub fn option<T>(
    &mut self,
    f: impl FnOnce(&mut Self) -> Result<T>,
  ) -> Result<Option<T>> {
    match self.u8()? {
      0 => Ok(None),
      _ => Ok(Some(f(self)?)),
    }
  }

  pub fn seq<T>(
    &mut self,
    mut f: impl FnMut(&mut Self) -> Result<T>,
  ) -> Result<Vec<T>> {
    let len = self.u32()? as usize;
    // Don't trust the length blindly: a corrupt header shouldn't be able to ask
    // for a multi-gigabyte allocation before we hit EOF.
    let mut out = Vec::with_capacity(len.min(64 * 1024));
    for _ in 0..len {
      out.push(f(self)?);
    }
    Ok(out)
  }

  pub fn map<K: std::hash::Hash + Eq, V>(
    &mut self,
    mut f: impl FnMut(&mut Self) -> Result<(K, V)>,
  ) -> Result<HashMap<K, V>> {
    let len = self.u32()? as usize;
    let mut out = HashMap::with_capacity(len.min(64 * 1024));
    for _ in 0..len {
      let (k, v) = f(self)?;
      out.insert(k, v);
    }
    Ok(out)
  }

  pub fn invalid_discriminant<T>(ty: &'static str, value: u32) -> Result<T> {
    Err(SnapshotDecodeError::InvalidDiscriminant { ty, value })
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  /// Leaks the buffer but records the pointer in a static, so the
  /// allocation stays reachable and miri's leak check stays clean.
  fn leak(b: Box<[u8]>) -> &'static [u8] {
    struct Keep(std::sync::Mutex<Vec<*const [u8]>>);
    // SAFETY: the pointers are only stored, never dereferenced.
    unsafe impl Sync for Keep {}
    static KEEP: Keep = Keep(std::sync::Mutex::new(Vec::new()));
    let r: &'static [u8] = Box::leak(b);
    KEEP.0.lock().unwrap().push(r as *const [u8]);
    r
  }

  #[test]
  fn roundtrip_scalars() {
    let mut e = Encoder::new();
    e.u8(7);
    e.bool(true);
    e.bool(false);
    e.u32(0xdead_beef);
    e.i32(-12345);
    e.usize(usize::MAX);
    let d = &mut Decoder::new(leak(e.into_bytes())).unwrap();
    assert_eq!(d.u8().unwrap(), 7);
    assert!(d.bool().unwrap());
    assert!(!d.bool().unwrap());
    assert_eq!(d.u32().unwrap(), 0xdead_beef);
    assert_eq!(d.i32().unwrap(), -12345);
    assert_eq!(d.usize().unwrap(), usize::MAX);
  }

  #[test]
  fn strings_borrow_the_buffer() {
    let mut e = Encoder::new();
    e.str("ext:core/01_core.js");
    e.str("héllo");
    let buf = leak(e.into_bytes());
    let d = &mut Decoder::new(buf).unwrap();

    let s = d.fast_string().unwrap();
    assert_eq!(s.as_str(), "ext:core/01_core.js");
    // The decoded string must be a borrow of the snapshot blob, not a copy.
    let borrowed = s.as_static_str().expect("must be a static borrow");
    let range = buf.as_ptr_range();
    assert!(range.contains(&borrowed.as_ptr()));

    let s = d.fast_string().unwrap();
    assert_eq!(s.as_str(), "héllo");
    assert!(s.as_static_str().is_some());
  }

  #[test]
  fn seq_and_option_and_map() {
    let mut e = Encoder::new();
    e.seq([1u32, 2, 3].into_iter(), |e, v| e.u32(v));
    e.option(Some(9u32), |e, v| e.u32(v));
    e.option(None::<u32>, |e, v| e.u32(v));
    e.seq([("a", 1u32), ("b", 2)].into_iter(), |e, (k, v)| {
      e.str(k);
      e.u32(v);
    });
    let d = &mut Decoder::new(leak(e.into_bytes())).unwrap();
    assert_eq!(d.seq(|d| d.u32()).unwrap(), vec![1, 2, 3]);
    assert_eq!(d.option(|d| d.u32()).unwrap(), Some(9));
    assert_eq!(d.option(|d| d.u32()).unwrap(), None);
    let m = d.map(|d| Ok((d.str()?.0, d.u32()?))).unwrap();
    assert_eq!(m.len(), 2);
    assert_eq!(m["a"], 1);
    assert_eq!(m["b"], 2);
  }

  #[test]
  fn version_mismatch_is_detected() {
    let mut bytes = Encoder::new().into_bytes().to_vec();
    bytes[0] = bytes[0].wrapping_add(1);
    let err = Decoder::new(leak(bytes.into_boxed_slice())).unwrap_err();
    assert!(matches!(err, SnapshotDecodeError::VersionMismatch { .. }));
  }

  #[test]
  fn truncated_input_is_detected() {
    let mut e = Encoder::new();
    e.str("hello");
    let bytes = e.into_bytes();
    let truncated = leak(bytes[..bytes.len() - 2].to_vec().into_boxed_slice());
    let d = &mut Decoder::new(truncated).unwrap();
    assert!(matches!(
      d.str().unwrap_err(),
      SnapshotDecodeError::UnexpectedEof
    ));
  }
}
