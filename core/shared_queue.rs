// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
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
|                        RECORD_ENDS (*MAX_RECORDS)             |
+---------------------------------------------------------------+
|                        RECORDS (*MAX_RECORDS)                 |
+---------------------------------------------------------------+
 */

use crate::bindings;
use crate::ops::OpId;
use rusty_v8 as v8;
use std::convert::TryInto;

const MAX_RECORDS: usize = 100;
/// Total number of records added.
const INDEX_NUM_RECORDS: usize = 0;
/// Number of records that have been shifted off.
const INDEX_NUM_SHIFTED_OFF: usize = 1;
/// The head is the number of initialized bytes in SharedQueue.
/// It grows monotonically.
const INDEX_HEAD: usize = 2;
const INDEX_OFFSETS: usize = 3;
const INDEX_RECORDS: usize = INDEX_OFFSETS + 2 * MAX_RECORDS;
/// Byte offset of where the records begin. Also where the head starts.
const HEAD_INIT: usize = 4 * INDEX_RECORDS;
/// A rough guess at how big we should make the shared buffer in bytes.
pub const RECOMMENDED_SIZE: usize = 128 * MAX_RECORDS;

pub struct SharedQueue {
  buf: v8::SharedRef<v8::BackingStore>,
}

impl SharedQueue {
  pub fn new(len: usize) -> Self {
    let buf = vec![0; HEAD_INIT + len].into_boxed_slice();
    let buf = v8::SharedArrayBuffer::new_backing_store_from_boxed_slice(buf);
    let mut q = Self {
      buf: buf.make_shared(),
    };
    q.reset();
    q
  }

  pub fn get_backing_store(&mut self) -> &mut v8::SharedRef<v8::BackingStore> {
    &mut self.buf
  }

  pub fn bytes(&self) -> &[u8] {
    unsafe {
      bindings::get_backing_store_slice(&self.buf, 0, self.buf.byte_length())
    }
  }

  pub fn bytes_mut(&mut self) -> &mut [u8] {
    unsafe {
      bindings::get_backing_store_slice_mut(
        &self.buf,
        0,
        self.buf.byte_length(),
      )
    }
  }

  fn reset(&mut self) {
    debug!("rust:shared_queue:reset");
    let s: &mut [u32] = self.as_u32_slice_mut();
    s[INDEX_NUM_RECORDS] = 0;
    s[INDEX_NUM_SHIFTED_OFF] = 0;
    s[INDEX_HEAD] = HEAD_INIT as u32;
  }

  fn as_u32_slice(&self) -> &[u32] {
    let p = self.bytes().as_ptr();
    // Assert pointer is 32 bit aligned before casting.
    assert_eq!((p as usize) % std::mem::align_of::<u32>(), 0);
    #[allow(clippy::cast_ptr_alignment)]
    let p32 = p as *const u32;
    unsafe { std::slice::from_raw_parts(p32, self.bytes().len() / 4) }
  }

  fn as_u32_slice_mut(&mut self) -> &mut [u32] {
    let p = self.bytes_mut().as_mut_ptr();
    // Assert pointer is 32 bit aligned before casting.
    assert_eq!((p as usize) % std::mem::align_of::<u32>(), 0);
    #[allow(clippy::cast_ptr_alignment)]
    let p32 = p as *mut u32;
    unsafe { std::slice::from_raw_parts_mut(p32, self.bytes().len() / 4) }
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

  fn num_shifted_off(&self) -> usize {
    let s = self.as_u32_slice();
    s[INDEX_NUM_SHIFTED_OFF] as usize
  }

  fn set_meta(&mut self, index: usize, end: usize, op_id: OpId) {
    let s = self.as_u32_slice_mut();
    s[INDEX_OFFSETS + 2 * index] = end as u32;
    s[INDEX_OFFSETS + 2 * index + 1] = op_id.try_into().unwrap();
  }

  #[cfg(test)]
  fn get_meta(&self, index: usize) -> Option<(OpId, usize)> {
    if index < self.num_records() {
      let s = self.as_u32_slice();
      let end = s[INDEX_OFFSETS + 2 * index] as usize;
      let op_id = s[INDEX_OFFSETS + 2 * index + 1] as OpId;
      Some((op_id, end))
    } else {
      None
    }
  }

  #[cfg(test)]
  fn get_offset(&self, index: usize) -> Option<usize> {
    if index < self.num_records() {
      Some(if index == 0 {
        HEAD_INIT
      } else {
        let s = self.as_u32_slice();
        let prev_end = s[INDEX_OFFSETS + 2 * (index - 1)] as usize;
        (prev_end + 3) & !3
      })
    } else {
      None
    }
  }

  /// Returns none if empty.
  #[cfg(test)]
  pub fn shift(&mut self) -> Option<(OpId, &[u8])> {
    let u32_slice = self.as_u32_slice();
    let i = u32_slice[INDEX_NUM_SHIFTED_OFF] as usize;
    if self.size() == 0 {
      assert_eq!(i, 0);
      return None;
    }

    let off = self.get_offset(i).unwrap();
    let (op_id, end) = self.get_meta(i).unwrap();
    if self.size() > 1 {
      let u32_slice = self.as_u32_slice_mut();
      u32_slice[INDEX_NUM_SHIFTED_OFF] += 1;
    } else {
      self.reset();
    }
    println!(
      "rust:shared_queue:shift: num_records={}, num_shifted_off={}, head={}",
      self.num_records(),
      self.num_shifted_off(),
      self.head()
    );
    Some((op_id, &self.bytes()[off..end]))
  }

  /// Because JS-side may cast popped message to Int32Array it is required
  /// that every message is aligned to 4-bytes.
  pub fn push(&mut self, op_id: OpId, record: &[u8]) -> bool {
    let off = self.head();
    assert_eq!(off % 4, 0);
    let end = off + record.len();
    let aligned_end = (end + 3) & !3;
    debug!(
      "rust:shared_queue:pre-push: op={}, off={}, end={}, len={}, aligned_end={}",
      op_id,
      off,
      end,
      record.len(),
      aligned_end,
    );
    let index = self.num_records();
    if aligned_end > self.bytes().len() || index >= MAX_RECORDS {
      debug!("WARNING the sharedQueue overflowed");
      return false;
    }
    assert_eq!(aligned_end % 4, 0);
    self.set_meta(index, end, op_id);
    assert_eq!(end - off, record.len());
    self.bytes_mut()[off..end].copy_from_slice(record);
    let u32_slice = self.as_u32_slice_mut();
    u32_slice[INDEX_NUM_RECORDS] += 1;
    u32_slice[INDEX_HEAD] = aligned_end as u32;
    debug!(
      "rust:shared_queue:push: num_records={}, num_shifted_off={}, head={}",
      self.num_records(),
      self.num_shifted_off(),
      self.head()
    );
    true
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn basic() {
    let mut q = SharedQueue::new(RECOMMENDED_SIZE);

    let h = q.head();
    assert!(h > 0);

    let r = vec![1u8, 2, 3, 4].into_boxed_slice();
    let len = r.len() + h;
    assert!(q.push(0, &r));
    assert_eq!(q.head(), len);

    let r = vec![5, 6, 7, 8].into_boxed_slice();
    assert!(q.push(0, &r));

    let r = vec![9, 10, 11, 12].into_boxed_slice();
    assert!(q.push(0, &r));
    assert_eq!(q.num_records(), 3);
    assert_eq!(q.size(), 3);

    let (_op_id, r) = q.shift().unwrap();
    assert_eq!(r, vec![1, 2, 3, 4].as_slice());
    assert_eq!(q.num_records(), 3);
    assert_eq!(q.size(), 2);

    let (_op_id, r) = q.shift().unwrap();
    assert_eq!(r, vec![5, 6, 7, 8].as_slice());
    assert_eq!(q.num_records(), 3);
    assert_eq!(q.size(), 1);

    let (_op_id, r) = q.shift().unwrap();
    assert_eq!(r, vec![9, 10, 11, 12].as_slice());
    assert_eq!(q.num_records(), 0);
    assert_eq!(q.size(), 0);

    assert!(q.shift().is_none());
    assert!(q.shift().is_none());

    assert_eq!(q.num_records(), 0);
    assert_eq!(q.size(), 0);
  }

  fn alloc_buf(byte_length: usize) -> Box<[u8]> {
    vec![0; byte_length].into_boxed_slice()
  }

  #[test]
  fn overflow() {
    let mut q = SharedQueue::new(RECOMMENDED_SIZE);
    assert!(q.push(0, &alloc_buf(RECOMMENDED_SIZE - 5)));
    assert_eq!(q.size(), 1);
    assert!(!q.push(0, &alloc_buf(6)));
    assert_eq!(q.size(), 1);
    assert!(q.push(0, &alloc_buf(1)));
    assert_eq!(q.size(), 2);

    let (_op_id, buf) = q.shift().unwrap();
    assert_eq!(buf.len(), RECOMMENDED_SIZE - 5);
    assert_eq!(q.size(), 1);

    assert!(!q.push(0, &alloc_buf(1)));

    let (_op_id, buf) = q.shift().unwrap();
    assert_eq!(buf.len(), 1);
    assert_eq!(q.size(), 0);
  }

  #[test]
  fn full_records() {
    let mut q = SharedQueue::new(RECOMMENDED_SIZE);
    for _ in 0..MAX_RECORDS {
      assert!(q.push(0, &alloc_buf(1)))
    }
    assert_eq!(q.push(0, &alloc_buf(1)), false);
    // Even if we shift one off, we still cannot push a new record.
    let _ignored = q.shift().unwrap();
    assert_eq!(q.push(0, &alloc_buf(1)), false);
  }

  #[test]
  fn allow_any_buf_length() {
    let mut q = SharedQueue::new(RECOMMENDED_SIZE);
    // Check that `record` that has length not a multiple of 4 will
    // not cause panic. Still make sure that records are always
    // aligned to 4 bytes.
    for i in 1..9 {
      q.push(0, &alloc_buf(i));
      assert_eq!(q.num_records(), i);
      assert_eq!(q.head() % 4, 0);
    }
  }
}
