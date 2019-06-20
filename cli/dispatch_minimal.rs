// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
// Do not add flatbuffer dependencies to this module.
//! Connects to js/dispatch_minimal.ts sendAsyncMinimal This acts as a faster
//! alternative to flatbuffers using a very simple list of int32s to lay out
//! messages. The first i32 is used to determine if a message a flatbuffer
//! message or a "minimal" message.
use crate::state::ThreadSafeState;
use deno::Buf;
use deno::CoreOp;
use deno::Op;
use deno::PinnedBuf;
use futures::Future;

const DISPATCH_MINIMAL_TOKEN: i32 = 0xCAFE;
const OP_READ: i32 = 1;
const OP_WRITE: i32 = 2;

#[derive(Copy, Clone, Debug, PartialEq)]
// This corresponds to RecordMinimal on the TS side.
pub struct Record {
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
    let buf32 = vec.into_boxed_slice();
    let ptr = Box::into_raw(buf32) as *mut [u8; 5 * 4];
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

  if s.len() < 5 {
    return None;
  }
  let ptr = s.as_ptr();
  let ints = unsafe { std::slice::from_raw_parts(ptr, 5) };
  if ints[0] != DISPATCH_MINIMAL_TOKEN {
    return None;
  }
  Some(Record {
    promise_id: ints[1],
    op_id: ints[2],
    arg: ints[3],
    result: ints[4],
  })
}

#[test]
fn test_parse_min_record() {
  let buf = vec![
    0xFE, 0xCA, 0, 0, 1, 0, 0, 0, 2, 0, 0, 0, 3, 0, 0, 0, 4, 0, 0, 0,
  ];
  assert_eq!(
    parse_min_record(&buf),
    Some(Record {
      promise_id: 1,
      op_id: 2,
      arg: 3,
      result: 4,
    })
  );

  let buf = vec![];
  assert_eq!(parse_min_record(&buf), None);

  let buf = vec![5];
  assert_eq!(parse_min_record(&buf), None);
}

pub fn dispatch_minimal(
  state: &ThreadSafeState,
  mut record: Record,
  zero_copy: Option<PinnedBuf>,
) -> CoreOp {
  let is_sync = record.promise_id == 0;
  let min_op = match record.op_id {
    OP_READ => ops::read(record.arg, zero_copy),
    OP_WRITE => ops::write(record.arg, zero_copy),
    _ => unimplemented!(),
  };

  let state = state.clone();

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
    let buf: Buf = record.into();
    state.metrics_op_completed(buf.len());
    Ok(buf)
  }));
  if is_sync {
    Op::Sync(fut.wait().unwrap())
  } else {
    Op::Async(fut)
  }
}

mod ops {
  use crate::deno_error;
  use crate::resources;
  use crate::tokio_write;
  use deno::PinnedBuf;
  use futures::Future;

  type MinimalOp = dyn Future<Item = i32, Error = deno_error::DenoError> + Send;

  pub fn read(rid: i32, zero_copy: Option<PinnedBuf>) -> Box<MinimalOp> {
    debug!("read rid={}", rid);
    let zero_copy = match zero_copy {
      None => {
        return Box::new(futures::future::err(deno_error::no_buffer_specified()))
      }
      Some(buf) => buf,
    };
    match resources::lookup(rid as u32) {
      None => Box::new(futures::future::err(deno_error::bad_resource())),
      Some(resource) => Box::new(
        tokio::io::read(resource, zero_copy)
          .map_err(deno_error::DenoError::from)
          .and_then(move |(_resource, _buf, nread)| Ok(nread as i32)),
      ),
    }
  }

  pub fn write(rid: i32, zero_copy: Option<PinnedBuf>) -> Box<MinimalOp> {
    debug!("write rid={}", rid);
    let zero_copy = match zero_copy {
      None => {
        return Box::new(futures::future::err(deno_error::no_buffer_specified()))
      }
      Some(buf) => buf,
    };
    match resources::lookup(rid as u32) {
      None => Box::new(futures::future::err(deno_error::bad_resource())),
      Some(resource) => Box::new(
        tokio_write::write(resource, zero_copy)
          .map_err(deno_error::DenoError::from)
          .and_then(move |(_resource, _buf, nwritten)| Ok(nwritten as i32)),
      ),
    }
  }
}
