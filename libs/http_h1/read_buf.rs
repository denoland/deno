// Copyright 2018-2026 the Deno authors. MIT license.

/// Reusable single-threaded read buffer with stable cursor indices.
///
/// This intentionally avoids ref-counted byte buffers. Parsed request data can
/// borrow from `filled()` until the caller consumes those bytes.
#[derive(Debug, Default)]
pub struct ReadBuf {
  buf: Vec<u8>,
  start: usize,
  end: usize,
}

impl ReadBuf {
  pub fn with_capacity(capacity: usize) -> Self {
    Self {
      buf: Vec::with_capacity(capacity),
      start: 0,
      end: 0,
    }
  }

  pub fn filled(&self) -> &[u8] {
    &self.buf[self.start..self.end]
  }

  pub fn len(&self) -> usize {
    self.end - self.start
  }

  pub fn is_empty(&self) -> bool {
    self.start == self.end
  }

  pub fn capacity(&self) -> usize {
    self.buf.capacity()
  }

  pub fn clear(&mut self) {
    self.buf.clear();
    self.start = 0;
    self.end = 0;
  }

  pub fn append(&mut self, bytes: &[u8]) {
    self.compact_if_needed(bytes.len());
    self.buf.extend_from_slice(bytes);
    self.end += bytes.len();
  }

  pub fn spare_capacity_mut(
    &mut self,
    min_len: usize,
  ) -> &mut [std::mem::MaybeUninit<u8>] {
    self.compact_if_needed(min_len);
    if self.buf.capacity().saturating_sub(self.end) < min_len {
      self.buf.reserve(min_len);
    }
    &mut self.buf.spare_capacity_mut()[..]
  }

  /// # Safety
  ///
  /// The caller must guarantee that exactly `len` bytes in the spare capacity
  /// returned by [`Self::spare_capacity_mut`] have been initialized.
  pub unsafe fn advance_filled(&mut self, len: usize) {
    let new_len = self.end + len;
    debug_assert!(new_len <= self.buf.capacity());
    // SAFETY: the caller guarantees the newly exposed bytes are initialized.
    unsafe { self.buf.set_len(new_len) };
    self.end = new_len;
  }

  pub fn consume(&mut self, len: usize) {
    assert!(len <= self.len());
    self.start += len;
    if self.start == self.end {
      self.clear();
    }
  }

  pub fn compact(&mut self) {
    if self.start == 0 {
      return;
    }
    self.buf.copy_within(self.start..self.end, 0);
    self.end -= self.start;
    self.buf.truncate(self.end);
    self.start = 0;
  }

  fn compact_if_needed(&mut self, additional: usize) {
    if self.buf.capacity().saturating_sub(self.end) >= additional {
      return;
    }
    self.compact();
  }
}

#[cfg(test)]
mod tests {
  use super::ReadBuf;

  #[test]
  fn consume_keeps_pipelined_bytes() {
    let mut buf = ReadBuf::with_capacity(16);
    buf.append(b"firstsecond");
    buf.consume(5);
    assert_eq!(buf.filled(), b"second");
    buf.append(b"third");
    assert_eq!(buf.filled(), b"secondthird");
  }
}
