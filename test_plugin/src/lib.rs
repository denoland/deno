// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use deno_core::plugin_api::Interface;
use deno_core::plugin_api::Op;
use deno_core::plugin_api::ZeroCopyBuf;
use futures::future::FutureExt;

#[no_mangle]
pub fn deno_plugin_init(interface: &mut dyn Interface) {
  interface.register_op("testSync", op_test_sync);
  interface.register_op("testAsync", op_test_async);
}

fn op_test_sync(
  _interface: &mut dyn Interface,
  zero_copy: &mut [ZeroCopyBuf],
) -> Op {
  if !zero_copy.is_empty() {
    println!("Hello from plugin.");
  }
  let zero_copy = zero_copy.to_vec();
  for (idx, buf) in zero_copy.iter().enumerate() {
    let buf_str = std::str::from_utf8(&buf[..]).unwrap();
    println!("zero_copy[{}]: {}", idx, buf_str);
  }
  let result = b"test";
  let result_box: Box<[u8]> = Box::new(*result);
  Op::Sync(result_box)
}

fn op_test_async(
  _interface: &mut dyn Interface,
  zero_copy: &mut [ZeroCopyBuf],
) -> Op {
  if !zero_copy.is_empty() {
    println!("Hello from plugin.");
  }
  let zero_copy = zero_copy.to_vec();
  let fut = async move {
    for (idx, buf) in zero_copy.iter().enumerate() {
      let buf_str = std::str::from_utf8(&buf[..]).unwrap();
      println!("zero_copy[{}]: {}", idx, buf_str);
    }
    let (tx, rx) = futures::channel::oneshot::channel::<Result<(), ()>>();
    std::thread::spawn(move || {
      std::thread::sleep(std::time::Duration::from_secs(1));
      tx.send(Ok(())).unwrap();
    });
    assert!(rx.await.is_ok());
    let result = b"test";
    let result_box: Box<[u8]> = Box::new(*result);
    result_box
  };

  Op::Async(fut.boxed())
}
