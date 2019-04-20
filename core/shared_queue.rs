// Copyright 2018 the Deno authors. All rights reserved. MIT license.
/*
SharedQueue Binary Layout
+-------------------------------+-------------------------------+
|                        NUM_RECORDS (32)                       |
+---------------------------------------------------------------+
|                        NUM_SHIFTED_OFF (32)                   |
+---------------------------------------------------------------+
|                        HEAD (32)                              |
+---------------------------------------------------------------+
|                        OFFSETS (32)                           |
+---------------------------------------------------------------+
|                        RECORD_ENDS (*MAX_RECORDS)           ...
+---------------------------------------------------------------+
|                        RECORDS (*MAX_RECORDS)               ...
+---------------------------------------------------------------+
 */

use crate::libdeno::deno_buf;

const MAX_RECORDS: usize = 100;
/// Total number of records added.
const INDEX_NUM_RECORDS: usize = 0;
/// Number of records that have been shifted off.
const INDEX_NUM_SHIFTED_OFF: usize = 1;
/// The head is the number of initialized bytes in SharedQueue.
/// It grows monotonically.
const INDEX_HEAD: usize = 2;
const INDEX_OFFSETS: usize = 3;
const INDEX_RECORDS: usize = 3 + MAX_RECORDS;
/// Byte offset of where the records begin. Also where the head starts.
const HEAD_INIT: usize = 4 * INDEX_RECORDS;
/// A rough guess at how big we should make the shared buffer in bytes.
pub const RECOMMENDED_SIZE: usize = 128 * MAX_RECORDS;

pub struct SharedQueue {
  bytes: Vec<u8>,
}

impl SharedQueue {
  pub fn new(len: usize) -> Self {
    let mut bytes = Vec::new();
    bytes.resize(HEAD_INIT + len, 0);
    let mut q = Self { bytes };
    q.reset();
    q
  }

  pub fn as_deno_buf(&self) -> deno_buf {
    let ptr = self.bytes.as_ptr();
    let len = self.bytes.len();
    unsafe { deno_buf::from_raw_parts(ptr, len) }
  }

  fn reset(&mut self) {
    let s: &mut [u32] = self.as_u32_slice_mut();
    s[INDEX_NUM_RECORDS] = 0;
    s[INDEX_NUM_SHIFTED_OFF] = 0;
    s[INDEX_HEAD] = HEAD_INIT as u32;
  }

  fn as_u32_slice(&self) -> &[u32] {
    let p = self.bytes.as_ptr();
    // Assert pointer is 32 bit aligned before casting.
    assert_eq!((p as usize) % std::mem::align_of::<u32>(), 0);
    #[allow(clippy::cast_ptr_alignment)]
    let p32 = p as *const u32;
    unsafe { std::slice::from_raw_parts(p32, self.bytes.len() / 4) }
  }

  fn as_u32_slice_mut(&mut self) -> &mut [u32] {
    let p = self.bytes.as_mut_ptr();
    // Assert pointer is 32 bit aligned before casting.
    assert_eq!((p as usize) % std::mem::align_of::<u32>(), 0);
    #[allow(clippy::cast_ptr_alignment)]
    let p32 = p as *mut u32;
    unsafe { std::slice::from_raw_parts_mut(p32, self.bytes.len() / 4) }
  }

  pub fn size(&self) -> usize {
    let s = self.as_u32_slice();
    (s[INDEX_NUM_RECORDS] - s[INDEX_NUM_SHIFTED_OFF]) as usize
  }

  fn num_records(&self) -> usize {
    let s = self.as_u32_slice();
    s[INDEX_NUM_RECORDS] as usize
  }

  fn head(&self) -> usize {
    let s = self.as_u32_slice();
    s[INDEX_HEAD] as usize
  }

  fn set_end(&mut self, index: usize, end: usize) {
    let s = self.as_u32_slice_mut();
    s[INDEX_OFFSETS + index] = end as u32;
  }

  fn get_end(&self, index: usize) -> Option<usize> {
    if index < self.num_records() {
      let s = self.as_u32_slice();
      Some(s[INDEX_OFFSETS + index] as usize)
    } else {
      None
    }
  }

  fn get_offset(&self, index: usize) -> Option<usize> {
    if index < self.num_records() {
      Some(if index == 0 {
        HEAD_INIT
      } else {
        let s = self.as_u32_slice();
        s[INDEX_OFFSETS + index - 1] as usize
      })
    } else {
      None
    }
  }

  /// Returns none if empty.
  pub fn shift(&mut self) -> Option<&[u8]> {
    let u32_slice = self.as_u32_slice();
    let i = u32_slice[INDEX_NUM_SHIFTED_OFF] as usize;
    if self.size() == 0 {
      assert_eq!(i, 0);
      return None;
    }

    let off = self.get_offset(i).unwrap();
    let end = self.get_end(i).unwrap();

    if self.size() > 1 {
      let u32_slice = self.as_u32_slice_mut();
      u32_slice[INDEX_NUM_SHIFTED_OFF] += 1;
    } else {
      self.reset();
    }

    Some(&self.bytes[off..end])
  }

  pub fn push(&mut self, record: &[u8]) -> bool {
    let off = self.head();
    let end = off + record.len();
    if end > self.bytes.len() || self.num_records() >= MAX_RECORDS {
      debug!("WARNING the sharedQueue overflowed");
      return false;
    }
    let index = self.num_records();
    self.set_end(index, end);
    assert_eq!(end - off, record.len());
    self.bytes[off..end].copy_from_slice(record);
    let u32_slice = self.as_u32_slice_mut();
    u32_slice[INDEX_NUM_RECORDS] += 1;
    u32_slice[INDEX_HEAD] = end as u32;
    true
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::isolate::Buf;

  #[test]
  fn basic() {
    let mut q = SharedQueue::new(RECOMMENDED_SIZE);

    let h = q.head();
    assert!(h > 0);

    let r = vec![1u8, 2, 3, 4, 5].into_boxed_slice();
    let len = r.len() + h;
    assert!(q.push(&r));
    assert_eq!(q.head(), len);

    let r = vec![6, 7].into_boxed_slice();
    assert!(q.push(&r));

    let r = vec![8, 9, 10, 11].into_boxed_slice();
    assert!(q.push(&r));
    assert_eq!(q.num_records(), 3);
    assert_eq!(q.size(), 3);

    let r = q.shift().unwrap();
    assert_eq!(r, vec![1, 2, 3, 4, 5].as_slice());
    assert_eq!(q.num_records(), 3);
    assert_eq!(q.size(), 2);

    let r = q.shift().unwrap();
    assert_eq!(r, vec![6, 7].as_slice());
    assert_eq!(q.num_records(), 3);
    assert_eq!(q.size(), 1);

    let r = q.shift().unwrap();
    assert_eq!(r, vec![8, 9, 10, 11].as_slice());
    assert_eq!(q.num_records(), 0);
    assert_eq!(q.size(), 0);

    assert!(q.shift().is_none());
    assert!(q.shift().is_none());

    assert_eq!(q.num_records(), 0);
    assert_eq!(q.size(), 0);
  }

  fn alloc_buf(byte_length: usize) -> Buf {
    let mut v = Vec::new();
    v.resize(byte_length, 0);
    v.into_boxed_slice()
  }

  #[test]
  fn overflow() {
    let mut q = SharedQueue::new(RECOMMENDED_SIZE);
    assert!(q.push(&alloc_buf(RECOMMENDED_SIZE - 1)));
    assert_eq!(q.size(), 1);
    assert!(!q.push(&alloc_buf(2)));
    assert_eq!(q.size(), 1);
    assert!(q.push(&alloc_buf(1)));
    assert_eq!(q.size(), 2);

    assert_eq!(q.shift().unwrap().len(), RECOMMENDED_SIZE - 1);
    assert_eq!(q.size(), 1);

    assert!(!q.push(&alloc_buf(1)));

    assert_eq!(q.shift().unwrap().len(), 1);
    assert_eq!(q.size(), 0);
  }

  #[test]
  fn full_records() {
    let mut q = SharedQueue::new(RECOMMENDED_SIZE);
    for _ in 0..MAX_RECORDS {
      assert!(q.push(&alloc_buf(1)))
    }
    assert_eq!(q.push(&alloc_buf(1)), false);
    // Even if we shift one off, we still cannot push a new record.
    assert_eq!(q.shift().unwrap().len(), 1);
    assert_eq!(q.push(&alloc_buf(1)), false);
  }
}
