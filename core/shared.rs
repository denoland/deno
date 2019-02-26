use crate::libdeno::deno_buf;
use std::mem;

// TODO this is where we abstract flatbuffers at.
// TODO make these constants private to this file.
const INDEX_NUM_RECORDS: usize = 0;
const INDEX_RECORDS: usize = 1;
pub const RECORD_OFFSET_PROMISE_ID: usize = 0;
pub const RECORD_OFFSET_OP: usize = 1;
pub const RECORD_OFFSET_ARG: usize = 2;
pub const RECORD_OFFSET_RESULT: usize = 3;
const RECORD_SIZE: usize = 4;
const NUM_RECORDS: usize = 100;

/// Represents the shared buffer between JS and Rust.
/// Used for FFI.
pub struct Shared(Vec<i32>);

impl Shared {
  pub fn new() -> Shared {
    let mut vec = Vec::<i32>::new();
    vec.resize(INDEX_RECORDS + RECORD_SIZE * NUM_RECORDS, 0);
    Shared(vec)
  }

  pub fn set_record(&mut self, i: usize, off: usize, value: i32) {
    assert!(i < NUM_RECORDS);
    self.0[INDEX_RECORDS + RECORD_SIZE * i + off] = value;
  }

  pub fn get_record(&self, i: usize, off: usize) -> i32 {
    assert!(i < NUM_RECORDS);
    return self.0[INDEX_RECORDS + RECORD_SIZE * i + off];
  }

  pub fn set_num_records(&mut self, num_records: i32) {
    self.0[INDEX_NUM_RECORDS] = num_records;
  }

  pub fn get_num_records(&self) -> i32 {
    return self.0[INDEX_NUM_RECORDS];
  }

  pub fn as_deno_buf(&mut self) -> deno_buf {
    let ptr = self.0.as_mut_ptr() as *mut u8;
    let len = mem::size_of::<i32>() * self.0.len();
    unsafe { deno_buf::from_raw_parts(ptr, len) }
  }
}
