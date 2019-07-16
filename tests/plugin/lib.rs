// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use deno::CoreOp;
use deno::Op;
use deno::{Buf, PinnedBuf};
use futures::future::lazy;

#[macro_use]
extern crate deno;

pub fn op_test_op(data: &[u8], zero_copy: Option<PinnedBuf>) -> CoreOp {
  if let Some(buf) = zero_copy {
    let data_str = std::str::from_utf8(&data[..]).unwrap();
    let buf_str = std::str::from_utf8(&buf[..]).unwrap();
    println!(
      "Hello from native bindings. data: {} | zero_copy: {}",
      data_str, buf_str
    );
  }
  let result = b"test";
  let result_box: Buf = Box::new(*result);
  Op::Sync(result_box)
}

declare_plugin_op!(test_op, op_test_op);

pub fn op_async_test_op(data: &[u8], zero_copy: Option<PinnedBuf>) -> CoreOp {
  if let Some(buf) = zero_copy {
    let data_str = std::str::from_utf8(&data[..]).unwrap();
    let buf_str = std::str::from_utf8(&buf[..]).unwrap();
    println!(
      "Hello from native bindings. data: {} | zero_copy: {}",
      data_str, buf_str
    );
  }
  let op = Box::new(lazy(move || {
    let result = b"test";
    let result_box: Buf = Box::new(*result);
    Ok(result_box)
  }));
  Op::Async(op)
}

declare_plugin_op!(async_test_op, op_async_test_op);
