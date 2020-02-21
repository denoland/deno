extern crate futures;

use deno_core::*;
use futures::future::FutureExt;

fn init(context: &mut dyn PluginInitContext) {
  context.register_op("testSync", Box::new(op_test_sync));
  context.register_op("testAsync", Box::new(op_test_async));
  context.register_op(
    "createResource",
    context.stateful_op(Box::new(op_create_resource)),
  );
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

struct TestResource {
  pub name: String,
}

impl Drop for TestResource {
  fn drop(&mut self) {
    println!("Dropped resource: {}", self.name)
  }
}

pub fn op_create_resource(
  state: &PluginState,
  data: &[u8],
  _zero_copy: Option<ZeroCopyBuf>,
) -> CoreOp {
  let name = std::str::from_utf8(&data[..]).unwrap().to_string();
  let _table = state.resource_table();
  let mut table = _table.borrow_mut();
  let resource = TestResource { name };
  let rid = table.add("testResource", Box::new(resource));
  Op::Sync(Box::new(rid.to_be_bytes()))
}
