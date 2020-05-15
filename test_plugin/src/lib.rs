use deno_core::dispatch_json::plugin_json_op;
use deno_core::dispatch_json::JsonError;
use deno_core::dispatch_json::JsonErrorKind;
use deno_core::dispatch_json::JsonOp;
use deno_core::plugin_api::Buf;
use deno_core::plugin_api::DispatchOpFn;
use deno_core::plugin_api::Interface;
use deno_core::plugin_api::Op;
use deno_core::plugin_api::ZeroCopyBuf;
use futures::future::FutureExt;
use serde::Deserialize;
use serde_json::json;
use serde_json::Value;

#[no_mangle]
pub fn deno_plugin_init(interface: &mut dyn Interface) {
  interface.register_op("testSync", op_test_sync.box_op());
  interface.register_op("testAsync", op_test_async.box_op());
  interface.register_op("jsonTest", plugin_json_op(op_json_test));
}

fn op_test_sync(
  _interface: &mut dyn Interface,
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

fn op_test_async(
  _interface: &mut dyn Interface,
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

#[derive(Clone, Copy, PartialEq, Debug)]
enum TestPluginErrorKind {
  JsonError = 1,
  ExpectedZeroCopy = 44,
}

#[derive(Debug)]
pub struct TestPluginError {
  kind: TestPluginErrorKind,
  msg: String,
}

impl TestPluginError {
  fn expected_zero_copy() -> Self {
    Self {
      kind: TestPluginErrorKind::ExpectedZeroCopy,
      msg: "Expected zero copy value".to_string(),
    }
  }
}

impl JsonError for TestPluginError {
  fn kind(&self) -> JsonErrorKind {
    (self.kind as u32).into()
  }

  fn msg(&self) -> String {
    self.msg.clone()
  }
}

impl From<serde_json::Error> for TestPluginError {
  fn from(e: serde_json::Error) -> Self {
    Self {
      kind: TestPluginErrorKind::JsonError,
      msg: e.to_string(),
    }
  }
}

#[derive(Deserialize)]
struct TestJsonOpArgs {
  pub size: i32,
  pub name: String,
}

pub fn op_json_test(
  _interface: &mut dyn Interface,
  args: Value,
  zero_copy: Option<ZeroCopyBuf>,
) -> Result<JsonOp<TestPluginError>, TestPluginError> {
  let args: TestJsonOpArgs = serde_json::from_value(args)?;

  if let Some(buf) = zero_copy {
    let buf_str = std::str::from_utf8(&buf[..]).unwrap();
    println!(
      "Hello from json op. size: {} | name: {} | zero_copy: {}",
      args.size, args.name, buf_str
    );
  } else {
    return Err(TestPluginError::expected_zero_copy());
  }

  Ok(JsonOp::Sync(json!({"id": 21, "name": args.name})))
}
