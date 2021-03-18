// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use crate::error::AnyError;
use crate::futures::future::FutureExt;
use crate::BufVec;
use crate::Op;
use crate::OpFn;
use crate::OpState;
use crate::ZeroCopyBuf;
use std::boxed::Box;
use std::cell::RefCell;
use std::convert::TryInto;
use std::future::Future;
use std::rc::Rc;

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct RequestHeader {
  pub request_id: u64,
  pub argument: u32,
}

impl RequestHeader {
  pub fn from_raw(bytes: &[u8]) -> Option<Self> {
    if bytes.len() < 3 * 4 {
      return None;
    }

    Some(Self {
      request_id: u64::from_le_bytes(bytes[0..8].try_into().unwrap()),
      argument: u32::from_le_bytes(bytes[8..12].try_into().unwrap()),
    })
  }
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct ResponseHeader {
  pub request_id: u64,
  pub status: u32,
  pub result: u32,
}

impl Into<[u8; 16]> for ResponseHeader {
  fn into(self) -> [u8; 16] {
    let mut resp_header = [0u8; 16];
    resp_header[0..8].copy_from_slice(&self.request_id.to_le_bytes());
    resp_header[8..12].copy_from_slice(&self.status.to_le_bytes());
    resp_header[12..16].copy_from_slice(&self.result.to_le_bytes());
    resp_header
  }
}

pub trait ValueOrVector {
  fn value(&self) -> u32;
  fn vector(self) -> Option<Vec<u8>>;
}

impl ValueOrVector for Vec<u8> {
  fn value(&self) -> u32 {
    self.len() as u32
  }
  fn vector(self) -> Option<Vec<u8>> {
    Some(self)
  }
}

impl ValueOrVector for u32 {
  fn value(&self) -> u32 {
    *self
  }
  fn vector(self) -> Option<Vec<u8>> {
    None
  }
}

fn gen_padding_32bit(len: usize) -> &'static [u8] {
  &[b' ', b' ', b' '][0..(4 - (len & 3)) & 3]
}

/// Creates an op that passes data synchronously using raw ui8 buffer.
///
/// The provided function `op_fn` has the following parameters:
/// * `&mut OpState`: the op state, can be used to read/write resources in the runtime from an op.
/// * `argument`: the i32 value that is passed to the Rust function.
/// * `&mut [ZeroCopyBuf]`: raw bytes passed along.
///
/// `op_fn` returns an array buffer value, which is directly returned to JavaScript.
///
/// When registering an op like this...
/// ```ignore
/// let mut runtime = JsRuntime::new(...);
/// runtime.register_op("hello", deno_core::buffer_op_sync(Self::hello_op));
/// ```
///
/// ...it can be invoked from JS using the provided name, for example:
/// ```js
/// Deno.core.ops();
/// let result = Deno.core.bufferOpSync("function_name", args);
/// ```
///
/// The `Deno.core.ops()` statement is needed once before any op calls, for initialization.
/// A more complete example is available in the examples directory.
pub fn buffer_op_sync<F, R>(op_fn: F) -> Box<OpFn>
where
  F: Fn(&mut OpState, u32, &mut [ZeroCopyBuf]) -> Result<R, AnyError> + 'static,
  R: ValueOrVector,
{
  Box::new(move |state: Rc<RefCell<OpState>>, bufs: BufVec| -> Op {
    let mut bufs_iter = bufs.into_iter();
    let record_buf = bufs_iter.next().expect("Expected record at position 0");
    let mut zero_copy = bufs_iter.collect::<BufVec>();

    let req_header = match RequestHeader::from_raw(&record_buf) {
      Some(r) => r,
      None => {
        let error_class = b"TypeError";
        let error_message = b"Unparsable control buffer";
        let len = error_class.len() + error_message.len();
        let padding = gen_padding_32bit(len);
        let resp_header = ResponseHeader {
          request_id: 0,
          status: 1,
          result: error_class.len() as u32,
        };
        return Op::Sync(
          error_class
            .iter()
            .chain(error_message.iter())
            .chain(padding)
            .chain(&Into::<[u8; 16]>::into(resp_header))
            .cloned()
            .collect(),
        );
      }
    };

    match op_fn(&mut state.borrow_mut(), req_header.argument, &mut zero_copy) {
      Ok(possibly_vector) => {
        let resp_header = ResponseHeader {
          request_id: req_header.request_id,
          status: 0,
          result: possibly_vector.value(),
        };
        let resp_encoded_header = Into::<[u8; 16]>::into(resp_header);

        let resp_vector = match possibly_vector.vector() {
          Some(mut vector) => {
            let padding = gen_padding_32bit(vector.len());
            vector.extend(padding);
            vector.extend(&resp_encoded_header);
            vector
          }
          None => resp_encoded_header.to_vec(),
        };
        Op::Sync(resp_vector.into_boxed_slice())
      }
      Err(error) => {
        let error_class =
          (state.borrow().get_error_class_fn)(&error).as_bytes();
        let error_message = error.to_string().as_bytes().to_owned();
        let len = error_class.len() + error_message.len();
        let padding = gen_padding_32bit(len);
        let resp_header = ResponseHeader {
          request_id: req_header.request_id,
          status: 1,
          result: error_class.len() as u32,
        };
        return Op::Sync(
          error_class
            .iter()
            .chain(error_message.iter())
            .chain(padding)
            .chain(&Into::<[u8; 16]>::into(resp_header))
            .cloned()
            .collect(),
        );
      }
    }
  })
}

/// Creates an op that passes data asynchronously using raw ui8 buffer.
///
/// The provided function `op_fn` has the following parameters:
/// * `Rc<RefCell<OpState>>`: the op state, can be used to read/write resources in the runtime from an op.
/// * `argument`: the i32 value that is passed to the Rust function.
/// * `BufVec`: raw bytes passed along, usually not needed if the JSON value is used.
///
/// `op_fn` returns a future, whose output is a JSON value. This value will be asynchronously
/// returned to JavaScript.
///
/// When registering an op like this...
/// ```ignore
/// let mut runtime = JsRuntime::new(...);
/// runtime.register_op("hello", deno_core::json_op_async(Self::hello_op));
/// ```
///
/// ...it can be invoked from JS using the provided name, for example:
/// ```js
/// Deno.core.ops();
/// let future = Deno.core.jsonOpAsync("function_name", args);
/// ```
///
/// The `Deno.core.ops()` statement is needed once before any op calls, for initialization.
/// A more complete example is available in the examples directory.
pub fn buffer_op_async<F, R, RV>(op_fn: F) -> Box<OpFn>
where
  F: Fn(Rc<RefCell<OpState>>, u32, BufVec) -> R + 'static,
  R: Future<Output = Result<RV, AnyError>> + 'static,
  RV: ValueOrVector,
{
  Box::new(move |state: Rc<RefCell<OpState>>, bufs: BufVec| -> Op {
    let mut bufs_iter = bufs.into_iter();
    let record_buf = bufs_iter.next().expect("Expected record at position 0");
    let zero_copy = bufs_iter.collect::<BufVec>();

    let req_header = match RequestHeader::from_raw(&record_buf) {
      Some(r) => r,
      None => {
        let error_class = b"TypeError";
        let error_message = b"Unparsable control buffer";
        let len = error_class.len() + error_message.len();
        let padding = gen_padding_32bit(len);
        let resp_header = ResponseHeader {
          request_id: 0,
          status: 1,
          result: error_class.len() as u32,
        };
        return Op::Sync(
          error_class
            .iter()
            .chain(error_message.iter())
            .chain(padding)
            .chain(&Into::<[u8; 16]>::into(resp_header))
            .cloned()
            .collect(),
        );
      }
    };

    let fut =
      op_fn(state.clone(), req_header.argument, zero_copy).map(move |result| {
        match result {
          Ok(possibly_vector) => {
            let resp_header = ResponseHeader {
              request_id: req_header.request_id,
              status: 0,
              result: possibly_vector.value(),
            };
            let resp_encoded_header = Into::<[u8; 16]>::into(resp_header);

            let resp_vector = match possibly_vector.vector() {
              Some(mut vector) => {
                let padding = gen_padding_32bit(vector.len());
                vector.extend(padding);
                vector.extend(&resp_encoded_header);
                vector
              }
              None => resp_encoded_header.to_vec(),
            };
            resp_vector.into_boxed_slice()
          }
          Err(error) => {
            let error_class =
              (state.borrow().get_error_class_fn)(&error).as_bytes();
            let error_message = error.to_string().as_bytes().to_owned();
            let len = error_class.len() + error_message.len();
            let padding = gen_padding_32bit(len);
            let resp_header = ResponseHeader {
              request_id: req_header.request_id,
              status: 1,
              result: error_class.len() as u32,
            };

            error_class
              .iter()
              .chain(error_message.iter())
              .chain(padding)
              .chain(&Into::<[u8; 16]>::into(resp_header))
              .cloned()
              .collect()
          }
        }
      });
    let temp = Box::pin(fut);
    Op::Async(temp)
  })
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn padding() {
    assert_eq!(gen_padding_32bit(0), &[] as &[u8]);
    assert_eq!(gen_padding_32bit(1), &[b' ', b' ', b' ']);
    assert_eq!(gen_padding_32bit(2), &[b' ', b' ']);
    assert_eq!(gen_padding_32bit(3), &[b' ']);
    assert_eq!(gen_padding_32bit(4), &[] as &[u8]);
    assert_eq!(gen_padding_32bit(5), &[b' ', b' ', b' ']);
  }

  #[test]
  fn response_header_to_bytes() {
    // Max size of an js Number is 1^53 - 1, so use this value as max for 64bit ´request_id´
    let resp_header = ResponseHeader {
      request_id: 0x0102030405060708u64,
      status: 0x090A0B0Cu32,
      result: 0x0D0E0F10u32,
    };

    // All numbers are always little-endian encoded, as the js side also wants this to be fixed
    assert_eq!(
      &Into::<[u8; 16]>::into(resp_header),
      &[8, 7, 6, 5, 4, 3, 2, 1, 12, 11, 10, 9, 16, 15, 14, 13]
    );
  }

  #[test]
  fn response_header_to_bytes_max_value() {
    // Max size of an js Number is 1^53 - 1, so use this value as max for 64bit ´request_id´
    let resp_header = ResponseHeader {
      request_id: (1u64 << 53u64) - 1u64,
      status: 0xFFFFFFFFu32,
      result: 0xFFFFFFFFu32,
    };

    // All numbers are always little-endian encoded, as the js side also wants this to be fixed
    assert_eq!(
      &Into::<[u8; 16]>::into(resp_header),
      &[
        255, 255, 255, 255, 255, 255, 31, 0, 255, 255, 255, 255, 255, 255, 255,
        255
      ]
    );
  }

  #[test]
  fn request_header_from_bytes() {
    let req_header =
      RequestHeader::from_raw(&[8, 7, 6, 5, 4, 3, 2, 1, 12, 11, 10, 9])
        .unwrap();

    assert_eq!(req_header.request_id, 0x0102030405060708u64);
    assert_eq!(req_header.argument, 0x090A0B0Cu32);
  }

  #[test]
  fn request_header_from_bytes_max_value() {
    let req_header = RequestHeader::from_raw(&[
      255, 255, 255, 255, 255, 255, 31, 0, 255, 255, 255, 255,
    ])
    .unwrap();

    assert_eq!(req_header.request_id, (1u64 << 53u64) - 1u64);
    assert_eq!(req_header.argument, 0xFFFFFFFFu32);
  }

  #[test]
  fn request_header_from_bytes_too_short() {
    let req_header =
      RequestHeader::from_raw(&[8, 7, 6, 5, 4, 3, 2, 1, 12, 11, 10]);

    assert_eq!(req_header, None);
  }

  #[test]
  fn request_header_from_bytes_long() {
    let req_header = RequestHeader::from_raw(&[
      8, 7, 6, 5, 4, 3, 2, 1, 12, 11, 10, 9, 13, 14, 15, 16, 17, 18, 19, 20, 21,
    ])
    .unwrap();

    assert_eq!(req_header.request_id, 0x0102030405060708u64);
    assert_eq!(req_header.argument, 0x090A0B0Cu32);
  }
}
