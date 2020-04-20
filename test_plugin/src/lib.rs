extern crate deno_core;
extern crate futures;

use deno_core::Buf;
use deno_core::Op;
use deno_core::ZeroCopyBuf;
use futures::future::FutureExt;

#[no_mangle]
pub fn deno_plugin_init(isolate: &mut deno_core::Isolate) {
  isolate.register_op("testSync", op_test_sync);
  isolate.register_op("testAsync", op_test_async);
}

pub fn op_test_sync(
  _isolate: &mut deno_core::Isolate,
  data: &[u8],
  zero_copy: Option<ZeroCopyBuf>,
) -> Op {
  if let Some(buf) = zero_copy {
    let data_str = std::str::from_utf8(&data[..]).unwrap();
    let buf_str = std::str::from_utf8(&buf[..]).unwrap();
    println!(
      "Hello from plugin. data: {} | zero_copy: {}",
      data_str, buf_str
    );
  }
  let result = b"test";
  let result_box: Buf = Box::new(*result);
  Op::Sync(result_box)
}

pub fn op_test_async(
  _isolate: &mut deno_core::Isolate,
  data: &[u8],
  zero_copy: Option<ZeroCopyBuf>,
) -> Op {
  let data_str = std::str::from_utf8(&data[..]).unwrap().to_string();
  let fut = async move {
    if let Some(buf) = zero_copy {
      let buf_str = std::str::from_utf8(&buf[..]).unwrap();
      println!(
        "Hello from plugin. data: {} | zero_copy: {}",
        data_str, buf_str
      );
    }
    let (tx, rx) = futures::channel::oneshot::channel::<Result<(), ()>>();
    std::thread::spawn(move || {
      std::thread::sleep(std::time::Duration::from_secs(1));
      tx.send(Ok(())).unwrap();
    });
    assert!(rx.await.is_ok());
    let result = b"test";
    let result_box: Buf = Box::new(*result);
    result_box
  };

  Op::Async(fut.boxed())
}
