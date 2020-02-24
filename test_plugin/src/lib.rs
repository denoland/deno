extern crate futures;

use deno_core::*;
use futures::future::FutureExt;
use serde::Deserialize;
use serde_json::json;
use serde_json::Value;

fn init(context: &mut dyn PluginInitContext) {
  context.register_op("testSync", Box::new(op_test_sync));
  context.register_op("testAsync", Box::new(op_test_async));
  context.register_op("jsonTest", Box::new(json_op(op_json_test)))
}
init_fn!(init);

pub fn op_test_sync(data: &[u8], zero_copy: Option<ZeroCopyBuf>) -> CoreOp {
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

pub fn op_test_async(data: &[u8], zero_copy: Option<ZeroCopyBuf>) -> CoreOp {
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
    Ok(result_box)
  };

  Op::Async(fut.boxed())
}

#[derive(Deserialize)]
struct TestJsonOpArgs {
  pub size: i32,
  pub name: String,
}

pub fn op_json_test(
  args: Value,
  zero_copy: Option<ZeroCopyBuf>,
) -> Result<JsonOp, ErrBox> {
  let args: TestJsonOpArgs = serde_json::from_value(args)?;

  if let Some(buf) = zero_copy {
    let buf_str = std::str::from_utf8(&buf[..]).unwrap();
    println!(
      "Hello from json op. size: {} | name: {} | zero_copy: {}",
      args.size, args.name, buf_str
    );
  }

  Ok(JsonOp::Sync(json!({"id": 21, "name": args.name})))
}
