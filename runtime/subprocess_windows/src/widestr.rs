// Copyright 2018-2025 the Deno authors. MIT license.

use std::ffi::OsStr;
use std::fmt;
use std::ops::Index;
use std::ops::Range;
use std::ops::RangeFrom;
use std::ops::RangeTo;
use std::os::windows::ffi::OsStrExt;

#[derive(Clone, PartialEq, Eq)]
pub struct WCString {
  buf: Box<[u16]>,
}

impl fmt::Display for WCString {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(
      f,
      "{}",
      String::from_utf16_lossy(&self.buf[..self.buf.len() - 1])
    )
  }
}

impl fmt::Debug for WCString {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(
      f,
      "\"{:?}\"",
      String::from_utf16_lossy(&self.buf[..self.buf.len() - 1])
    )
  }
}

impl WCString {
  pub fn new<T: AsRef<OsStr>>(s: T) -> Self {
    let buf = s.as_ref().encode_wide().chain(Some(0)).collect::<Vec<_>>();
    Self {
      buf: buf.into_boxed_slice(),
    }
  }

  pub fn from_vec(vec: Vec<u16>) -> Self {
    if vec.last().unwrap_or(&1) == &0 {
      Self {
        buf: vec.into_boxed_slice(),
      }
    } else {
      let mut buf = vec;
      buf.push(0);
      Self {
        buf: buf.into_boxed_slice(),
      }
    }
  }

  pub fn as_ptr(&self) -> *const u16 {
    self.buf.as_ptr()
  }

  pub fn len_no_nul(&self) -> usize {
    self.buf.len() - 1
  }

  #[allow(dead_code)]
  pub fn as_wcstr(&self) -> &WCStr {
    WCStr::from_wchars(&self.buf)
  }

  pub fn as_slice_no_nul(&self) -> &[u16] {
    &self.buf[..self.len_no_nul()]
  }
}

#[repr(transparent)]
pub struct WCStr {
  buf: [u16],
}

impl WCStr {
  // pub fn new<B: ?Sized + AsRef<[u16]>(buf: &B) -> &Self {
  // }

  pub fn len(&self) -> usize {
    if self.has_nul() {
      self.buf.len() - 1
    } else {
      self.buf.len()
    }
  }

  pub fn from_wchars(wchars: &[u16]) -> &WCStr {
    if wchars.last().unwrap_or(&1) == &0 {
      unsafe { &*(wchars as *const [u16] as *const WCStr) }
    } else {
      panic!("wchars must have a null terminator");
    }
  }

  pub fn as_ptr(&self) -> *const u16 {
    self.buf.as_ptr()
  }

  pub fn has_nul(&self) -> bool {
    if self.buf.is_empty() {
      false
    } else {
      self.buf[self.buf.len() - 1] == 0
    }
  }

  pub fn wchars_no_null(&self) -> &[u16] {
    if self.buf.is_empty() {
      return &[];
    }
    if self.has_nul() {
      &self.buf[0..self.buf.len() - 1]
    } else {
      &self.buf
    }
  }
}

impl Index<usize> for WCStr {
  type Output = u16;

  fn index(&self, index: usize) -> &Self::Output {
    &self.buf[index]
  }
}

impl Index<Range<usize>> for WCStr {
  type Output = [u16];

  fn index(&self, index: Range<usize>) -> &Self::Output {
    &self.buf[index]
  }
}

impl Index<RangeTo<usize>> for WCStr {
  type Output = [u16];

  fn index(&self, index: RangeTo<usize>) -> &Self::Output {
    &self.buf[index]
  }
}

impl Index<RangeFrom<usize>> for WCStr {
  type Output = [u16];

  fn index(&self, index: RangeFrom<usize>) -> &Self::Output {
    &self.buf[index]
  }
}
