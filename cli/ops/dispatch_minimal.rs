// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
// Do not add flatbuffer dependencies to this module.
//! Connects to js/dispatch_minimal.ts sendAsyncMinimal This acts as a faster
//! alternative to flatbuffers using a very simple list of int32s to lay out
//! messages. The first i32 is used to determine if a message a flatbuffer
//! message or a "minimal" message.
use crate::deno_error::GetErrorKind;
use crate::msg::ErrorKind;
use byteorder::{LittleEndian, WriteBytesExt};
use deno_core::Buf;
use deno_core::CoreOp;
use deno_core::ErrBox;
use deno_core::Op;
use deno_core::ZeroCopyBuf;
use futures::future::FutureExt;
use std::future::Future;
use std::pin::Pin;

pub type MinimalOp = dyn Future<Output = Result<i32, ErrBox>> + Send;

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

pub struct ErrorRecord {
  pub promise_id: i32,
  pub arg: i32,
  pub error_code: i32,
  pub error_message: Vec<u8>,
}

impl Into<Buf> for ErrorRecord {
  fn into(self) -> Buf {
    let v32: Vec<i32> = vec![self.promise_id, self.arg, self.error_code];
    let mut v8: Vec<u8> = Vec::new();
    for n in v32 {
      v8.write_i32::<LittleEndian>(n).unwrap();
    }
    let mut message = self.error_message;
    // Align to 32bit word, padding with the space character.
    message.resize((message.len() + 3usize) & !3usize, b' ');
    v8.append(&mut message);
    v8.into_boxed_slice()
  }
}

#[test]
fn test_error_record() {
  let expected = vec![
    1, 0, 0, 0, 255, 255, 255, 255, 10, 0, 0, 0, 69, 114, 114, 111, 114, 32,
    32, 32,
  ];
  let err_record = ErrorRecord {
    promise_id: 1,
    arg: -1,
    error_code: 10,
    error_message: "Error".to_string().as_bytes().to_owned(),
  };
  let buf: Buf = err_record.into();
  assert_eq!(buf, expected.into_boxed_slice());
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

pub fn minimal_op<D>(d: D) -> impl Fn(&[u8], Option<ZeroCopyBuf>) -> CoreOp
where
  D: Fn(i32, Option<ZeroCopyBuf>) -> Pin<Box<MinimalOp>>,
{
  move |control: &[u8], zero_copy: Option<ZeroCopyBuf>| {
    let mut record = match parse_min_record(control) {
      Some(r) => r,
      None => {
        let error_record = ErrorRecord {
          promise_id: 0,
          arg: -1,
          error_code: ErrorKind::InvalidInput as i32,
          error_message: "Unparsable control buffer"
            .to_string()
            .as_bytes()
            .to_owned(),
        };
        return Op::Sync(error_record.into());
      }
    };
    let is_sync = record.promise_id == 0;
    let rid = record.arg;
    let min_op = d(rid, zero_copy);

    // Convert to CoreOp
    let fut = async move {
      match min_op.await {
        Ok(r) => {
          record.result = r;
          Ok(record.into())
        }
        Err(err) => {
          let error_record = ErrorRecord {
            promise_id: record.promise_id,
            arg: -1,
            error_code: err.kind() as i32,
            error_message: err.to_string().as_bytes().to_owned(),
          };
          Ok(error_record.into())
        }
      }
    };

    if is_sync {
      // Warning! Possible deadlocks can occur if we try to wait for a future
      // while in a future. The safe but expensive alternative is to use
      // tokio_util::block_on.
      // This block is only exercised for readSync and writeSync, which I think
      // works since they're simple polling futures.
      Op::Sync(futures::executor::block_on(fut).unwrap())
    } else {
      Op::Async(fut.boxed())
    }
  }
}
