// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use deno_core::error::AnyError;
use deno_core::futures::future::FutureExt;
use deno_core::BufVec;
use deno_core::Op;
use deno_core::OpFn;
use deno_core::OpState;
use std::cell::RefCell;
use std::future::Future;
use std::iter::repeat;
use std::mem::size_of_val;
use std::pin::Pin;
use std::rc::Rc;
use std::slice;

pub enum MinimalOp {
  Sync(Result<i32, AnyError>),
  Async(Pin<Box<dyn Future<Output = Result<i32, AnyError>>>>),
}

#[derive(Copy, Clone, Debug, PartialEq)]
// This corresponds to RecordMinimal on the TS side.
pub struct Record {
  pub promise_id: i32,
  pub arg: i32,
  pub result: i32,
}

impl Into<Box<[u8]>> for Record {
  fn into(self) -> Box<[u8]> {
    let vec = vec![self.promise_id, self.arg, self.result];
    let buf32 = vec.into_boxed_slice();
    let ptr = Box::into_raw(buf32) as *mut [u8; 3 * 4];
    unsafe { Box::from_raw(ptr) }
  }
}

pub struct ErrorRecord {
  pub promise_id: i32,
  pub arg: i32,
  pub error_len: i32,
  pub error_class: &'static [u8],
  pub error_message: Vec<u8>,
}

impl Into<Box<[u8]>> for ErrorRecord {
  fn into(self) -> Box<[u8]> {
    let Self {
      promise_id,
      arg,
      error_len,
      error_class,
      error_message,
      ..
    } = self;
    let header_i32 = [promise_id, arg, error_len];
    let header_u8 = unsafe {
      slice::from_raw_parts(
        &header_i32 as *const _ as *const u8,
        size_of_val(&header_i32),
      )
    };
    let padded_len =
      (header_u8.len() + error_class.len() + error_message.len() + 3usize)
        & !3usize;
    header_u8
      .iter()
      .cloned()
      .chain(error_class.iter().cloned())
      .chain(error_message.into_iter())
      .chain(repeat(b' '))
      .take(padded_len)
      .collect()
  }
}

#[test]
fn test_error_record() {
  let expected = vec![
    1, 0, 0, 0, 255, 255, 255, 255, 11, 0, 0, 0, 66, 97, 100, 82, 101, 115,
    111, 117, 114, 99, 101, 69, 114, 114, 111, 114,
  ];
  let err_record = ErrorRecord {
    promise_id: 1,
    arg: -1,
    error_len: 11,
    error_class: b"BadResource",
    error_message: b"Error".to_vec(),
  };
  let buf: Box<[u8]> = err_record.into();
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
      result: 4
    })
  );

  let buf = vec![];
  assert_eq!(parse_min_record(&buf), None);

  let buf = vec![5];
  assert_eq!(parse_min_record(&buf), None);
}

pub fn minimal_op<F>(op_fn: F) -> Box<OpFn>
where
  F: Fn(Rc<RefCell<OpState>>, bool, i32, BufVec) -> MinimalOp + 'static,
{
  Box::new(move |state: Rc<RefCell<OpState>>, bufs: BufVec| {
    let mut bufs_iter = bufs.into_iter();
    let record_buf = bufs_iter.next().expect("Expected record at position 0");
    let zero_copy = bufs_iter.collect::<BufVec>();

    let mut record = match parse_min_record(&record_buf) {
      Some(r) => r,
      None => {
        let error_class = b"TypeError";
        let error_message = b"Unparsable control buffer";
        let error_record = ErrorRecord {
          promise_id: 0,
          arg: -1,
          error_len: error_class.len() as i32,
          error_class,
          error_message: error_message[..].to_owned(),
        };
        return Op::Sync(error_record.into());
      }
    };
    let is_sync = record.promise_id == 0;
    let rid = record.arg;
    let min_op = op_fn(state.clone(), is_sync, rid, zero_copy);

    match min_op {
      MinimalOp::Sync(sync_result) => Op::Sync(match sync_result {
        Ok(r) => {
          record.result = r;
          record.into()
        }
        Err(err) => {
          let error_class = (state.borrow().get_error_class_fn)(&err);
          let error_record = ErrorRecord {
            promise_id: record.promise_id,
            arg: -1,
            error_len: error_class.len() as i32,
            error_class: error_class.as_bytes(),
            error_message: err.to_string().as_bytes().to_owned(),
          };
          error_record.into()
        }
      }),
      MinimalOp::Async(min_fut) => {
        let fut = async move {
          match min_fut.await {
            Ok(r) => {
              record.result = r;
              record.into()
            }
            Err(err) => {
              let error_class = (state.borrow().get_error_class_fn)(&err);
              let error_record = ErrorRecord {
                promise_id: record.promise_id,
                arg: -1,
                error_len: error_class.len() as i32,
                error_class: error_class.as_bytes(),
                error_message: err.to_string().as_bytes().to_owned(),
              };
              error_record.into()
            }
          }
        };
        Op::Async(fut.boxed_local())
      }
    }
  })
}
