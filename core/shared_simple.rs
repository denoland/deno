use crate::deno_buf;
use crate::shared::Shared;
use std::mem;

// Used for Default
const INDEX_LEN: usize = 0;
const NUM_RECORDS: usize = 128;
const RECORD_SIZE: usize = 4; // num i32 in SharedSimpleRecord

#[derive(Default, Clone, PartialEq, Debug)]
pub struct SharedSimpleRecord {
  pub promise_id: i32,
  pub op_id: i32,
  pub arg: i32,
  pub result: i32,
}

/// Represents the shared buffer between JS and Rust.
/// Used for FFI.
pub struct SharedSimple(Vec<i32>);

impl SharedSimple {
  pub fn new() -> SharedSimple {
    let mut vec = Vec::<i32>::new();
    let n = 1 + 4 * NUM_RECORDS;
    vec.resize(n, 0);
    vec[INDEX_LEN] = 0;
    SharedSimple(vec)
  }
}

fn idx(i: usize, off: usize) -> usize {
  1 + i * RECORD_SIZE + off
}

impl Shared<SharedSimpleRecord> for SharedSimple {
  fn as_deno_buf(&self) -> deno_buf {
    let ptr = self.0.as_ptr() as *const u8;
    let len = mem::size_of::<i32>() * self.0.len();
    unsafe { deno_buf::from_raw_parts(ptr, len) }
  }

  fn js() -> (&'static str, &'static str) {
    ("core/shared_simple.js", include_str!("shared_simple.js"))
  }

  fn push(&mut self, record: &SharedSimpleRecord) -> bool {
    debug!("push {:?}", record);
    let i = self.len();
    if i >= NUM_RECORDS {
      return false;
    }
    self.0[idx(i, 0)] = record.promise_id;
    self.0[idx(i, 1)] = record.op_id;
    self.0[idx(i, 2)] = record.arg;
    self.0[idx(i, 3)] = record.result;
    self.0[INDEX_LEN] += 1;
    true
  }

  /// Gets an element.
  fn pop(&mut self) -> Option<SharedSimpleRecord> {
    if self.len() == 0 {
      return None;
    }
    self.0[INDEX_LEN] -= 1;
    let i = self.len();

    Some(SharedSimpleRecord {
      promise_id: self.0[idx(i, 0)],
      op_id: self.0[idx(i, 1)],
      arg: self.0[idx(i, 2)],
      result: self.0[idx(i, 3)],
    })
  }

  fn reset(&mut self) {
    self.0[INDEX_LEN] = 0;
  }

  /// Returns number of elements.
  fn len(&self) -> usize {
    self.0[INDEX_LEN] as usize
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::Isolate;
  use crate::JSError;

  fn inc_counter(isolate: &mut Isolate, zero_copy_buf: deno_buf) {
    assert_eq!(zero_copy_buf.len(), 0);
    isolate.test_send_counter += 1; // TODO ideally store this in isolate.state?
  }

  fn js_check(r: Result<(), JSError>) {
    if let Err(e) = r {
      panic!(e.to_string());
    }
  }

  #[test]
  fn test_execute() {
    let shared = SharedSimple::new();
    let mut isolate = Isolate::new(shared, inc_counter, None);
    let (setup_filename, setup_source) = SharedSimple::js();
    js_check(isolate.execute(setup_filename, setup_source));
    js_check(isolate.execute("x.js", "Deno.sharedSimple.push(1, 2, 3, 4)"));
    assert_eq!(isolate.shared.len(), 1);
    js_check(isolate.execute("x.js", "Deno.sharedSimple.push(-1, -2, -3, -4)"));
    assert_eq!(isolate.shared.len(), 2);
    assert_eq!(
      isolate.shared.pop().unwrap(),
      SharedSimpleRecord {
        promise_id: -1,
        op_id: -2,
        arg: -3,
        result: -4
      }
    );
    assert_eq!(isolate.shared.len(), 1);
    assert_eq!(
      isolate.shared.pop().unwrap(),
      SharedSimpleRecord {
        promise_id: 1,
        op_id: 2,
        arg: 3,
        result: 4
      }
    );
    assert_eq!(isolate.shared.len(), 0);

    js_check(isolate.execute("x.js", "Deno.sharedSimple.push(1, 2, 3, 4)"));
    assert_eq!(isolate.shared.len(), 1);
    js_check(isolate.execute("x.js", "Deno.sharedSimple.push(-1, -2, -3, -4)"));

    js_check(isolate.execute(
      "x.js",
      r#"
        let r = Deno.sharedSimple.pop();
        if (r.promiseId != -1) throw "err";
        if (r.opId != -2) throw "err";
        if (r.arg != -3) throw "err";
        if (r.result != -4) throw "err";
      "#,
    ));

    js_check(isolate.execute(
      "x.js",
      r#"
        if (Deno.sharedSimple.size() != 1) throw "err";
      "#,
    ));

    js_check(isolate.execute("x.js", "Deno.sharedSimple.reset()"));
    assert_eq!(isolate.shared.len(), 0);
    assert_eq!(isolate.test_send_counter, 0);
  }
}
