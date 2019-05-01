// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
// Do not add flatbuffer dependencies to this module.
//! Connects to js/dispatch_minimal.ts sendAsyncMinimal This acts as a faster
//! alternative to flatbuffers using a very simple list of int32s to lay out
//! messages. The first i32 is used to determine if a message a flatbuffer
//! message or a "minimal" message, see `has_minimal_token()`.
use crate::state::ThreadSafeState;
use deno::Buf;
use deno::Op;
use deno::PinnedBuf;
use futures::Future;

const DISPATCH_MINIMAL_TOKEN: i32 = 0xCAFE;
const OP_READ: i32 = 1;
const OP_WRITE: i32 = 2;

pub fn has_minimal_token(s: &[i32]) -> bool {
  s[0] == DISPATCH_MINIMAL_TOKEN
}

#[derive(Clone, Debug, PartialEq)]
struct Record {
  pub promise_id: i32,
  pub op_id: i32,
  pub arg: i32,
  pub result: i32,
}

impl Into<Buf> for Record {
  fn into(self) -> Buf {
    let vec = vec![
      DISPATCH_MINIMAL_TOKEN,
      self.promise_id,
      self.op_id,
      self.arg,
      self.result,
    ];
    //let len = vec.len();
    let buf32 = vec.into_boxed_slice();
    let ptr = Box::into_raw(buf32) as *mut [u8; 5 * 4];
    unsafe { Box::from_raw(ptr) }
  }
}

impl From<&[i32]> for Record {
  fn from(s: &[i32]) -> Record {
    let ptr = s.as_ptr();
    let ints = unsafe { std::slice::from_raw_parts(ptr, 5) };
    assert_eq!(ints[0], DISPATCH_MINIMAL_TOKEN);
    Record {
      promise_id: ints[1],
      op_id: ints[2],
      arg: ints[3],
      result: ints[4],
    }
  }
}

pub fn dispatch_minimal(
  state: &ThreadSafeState,
  control32: &[i32],
  zero_copy: Option<PinnedBuf>,
) -> Op {
  let record = Record::from(control32);
  let is_sync = record.promise_id == 0;
  let min_op = match record.op_id {
    OP_READ => ops::read(record.arg, zero_copy),
    OP_WRITE => ops::write(record.arg, zero_copy),
    _ => unimplemented!(),
  };

  let mut record_a = record.clone();
  let mut record_b = record.clone();
  let state = state.clone();

  let fut = Box::new(
    min_op
      .and_then(move |result| {
        record_a.result = result;
        Ok(record_a)
      }).or_else(|err| -> Result<Record, ()> {
        debug!("unexpected err {}", err);
        record_b.result = -1;
        Ok(record_b)
      }).then(move |result| -> Result<Buf, ()> {
        let record = result.unwrap();
        let buf: Buf = record.into();
        state.metrics_op_completed(buf.len());
        Ok(buf)
      }),
  );
  if is_sync {
    Op::Sync(fut.wait().unwrap())
  } else {
    Op::Async(fut)
  }
}

mod ops {
  use crate::errors;
  use crate::resources;
  use crate::tokio_write;
  use deno::PinnedBuf;
  use futures::Future;

  type MinimalOp = dyn Future<Item = i32, Error = errors::DenoError> + Send;

  pub fn read(rid: i32, zero_copy: Option<PinnedBuf>) -> Box<MinimalOp> {
    debug!("read rid={}", rid);
    let zero_copy = zero_copy.unwrap();
    match resources::lookup(rid as u32) {
      None => Box::new(futures::future::err(errors::bad_resource())),
      Some(resource) => Box::new(
        tokio::io::read(resource, zero_copy)
          .map_err(|err| err.into())
          .and_then(move |(_resource, _buf, nread)| Ok(nread as i32)),
      ),
    }
  }

  pub fn write(rid: i32, zero_copy: Option<PinnedBuf>) -> Box<MinimalOp> {
    debug!("write rid={}", rid);
    let zero_copy = zero_copy.unwrap();
    match resources::lookup(rid as u32) {
      None => Box::new(futures::future::err(errors::bad_resource())),
      Some(resource) => Box::new(
        tokio_write::write(resource, zero_copy)
          .map_err(|err| err.into())
          .and_then(move |(_resource, _buf, nwritten)| Ok(nwritten as i32)),
      ),
    }
  }
}
