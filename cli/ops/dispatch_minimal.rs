// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
// Do not add flatbuffer dependencies to this module.
//! Connects to js/dispatch_minimal.ts sendAsyncMinimal This acts as a faster
//! alternative to flatbuffers using a very simple list of int32s to lay out
//! messages. The first i32 is used to determine if a message a flatbuffer
//! message or a "minimal" message.
use deno::Buf;
use deno::CoreOp;
use deno::ErrBox;
use deno::Op;
use deno::PinnedBuf;
use futures::Future;

pub type MinimalOp = dyn Future<Item = i32, Error = ErrBox> + Send;
pub type Dispatcher = fn(i32, Option<PinnedBuf>) -> Box<MinimalOp>;

#[derive(Copy, Clone, Debug, PartialEq)]
// This corresponds to RecordMinimal on the TS side.
pub struct Record {
  pub promise_id: i32,
  pub arg: i32,
  pub result: i32,
}

impl Into<Buf> for Record {
  fn into(self) -> Buf {
    let vec = vec![self.promise_id, self.arg, self.result];
    let buf32 = vec.into_boxed_slice();
    let ptr = Box::into_raw(buf32) as *mut [u8; 3 * 4];
    unsafe { Box::from_raw(ptr) }
  }
}

pub fn parse_min_record(bytes: &[u8]) -> Option<Record> {
  if bytes.len() % std::mem::size_of::<i32>() != 0 {
    return None;
  }
  let p = bytes.as_ptr();
  #[allow(clippy::cast_ptr_alignment)]
  let p32 = p as *const i32;
  let s = unsafe { std::slice::from_raw_parts(p32, bytes.len() / 4) };

  if s.len() != 3 {
    return None;
  }
  let ptr = s.as_ptr();
  let ints = unsafe { std::slice::from_raw_parts(ptr, 3) };
  Some(Record {
    promise_id: ints[0],
    arg: ints[1],
    result: ints[2],
  })
}

#[test]
fn test_parse_min_record() {
  let buf = vec![1, 0, 0, 0, 3, 0, 0, 0, 4, 0, 0, 0];
  assert_eq!(
    parse_min_record(&buf),
    Some(Record {
      promise_id: 1,
      arg: 3,
      result: 4,
    })
  );

  let buf = vec![];
  assert_eq!(parse_min_record(&buf), None);

  let buf = vec![5];
  assert_eq!(parse_min_record(&buf), None);
}

pub fn minimal_op(
  d: Dispatcher,
) -> impl Fn(&[u8], Option<PinnedBuf>) -> CoreOp {
  move |control: &[u8], zero_copy: Option<PinnedBuf>| {
    let mut record = parse_min_record(control).unwrap();
    let is_sync = record.promise_id == 0;
    let rid = record.arg;
    let min_op = d(rid, zero_copy);

    // Convert to CoreOp
    let fut = Box::new(min_op.then(move |result| -> Result<Buf, ()> {
      match result {
        Ok(r) => {
          record.result = r;
        }
        Err(err) => {
          // TODO(ry) The dispatch_minimal doesn't properly pipe errors back to
          // the caller.
          debug!("swallowed err {}", err);
          record.result = -1;
        }
      }
      Ok(record.into())
    }));

    if is_sync {
      // Warning! Possible deadlocks can occur if we try to wait for a future
      // while in a future. The safe but expensive alternative is to use
      // tokio_util::block_on.
      // This block is only exercised for readSync and writeSync, which I think
      // works since they're simple polling futures.
      Op::Sync(fut.wait().unwrap())
    } else {
      Op::Async(fut)
    }
  }
}
