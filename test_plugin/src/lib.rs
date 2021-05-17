// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use std::borrow::Cow;
use std::cell::RefCell;
use std::rc::Rc;

use deno_core::error::bad_resource_id;
use deno_core::error::AnyError;
use deno_core::op_async;
use deno_core::op_sync;
use deno_core::Extension;
use deno_core::OpState;
use deno_core::Resource;
use deno_core::ResourceId;
use deno_core::ZeroCopyBuf;
use serde::Deserialize;

#[no_mangle]
pub fn init() -> Extension {
  Extension::builder()
    .ops(vec![
      ("op_test_sync", op_sync(op_test_sync)),
      ("op_test_async", op_async(op_test_async)),
      (
        "op_test_resource_table_add",
        op_sync(op_test_resource_table_add),
      ),
      (
        "op_test_resource_table_get",
        op_sync(op_test_resource_table_get),
      ),
    ])
    .build()
}

#[derive(Debug, Deserialize)]
struct TestArgs {
  val: String,
}

fn op_test_sync(
  _state: &mut OpState,
  args: TestArgs,
  zero_copy: Option<ZeroCopyBuf>,
) -> Result<String, AnyError> {
  println!("Hello from sync plugin op.");

  println!("args: {:?}", args);

  if let Some(buf) = zero_copy {
    let buf_str = std::str::from_utf8(&buf[..])?;
    println!("zero_copy: {}", buf_str);
  }

  Ok("test".to_string())
}

async fn op_test_async(
  _state: Rc<RefCell<OpState>>,
  args: TestArgs,
  zero_copy: Option<ZeroCopyBuf>,
) -> Result<String, AnyError> {
  println!("Hello from async plugin op.");

  println!("args: {:?}", args);

  if let Some(buf) = zero_copy {
    let buf_str = std::str::from_utf8(&buf[..])?;
    println!("zero_copy: {}", buf_str);
  }

  let (tx, rx) = futures::channel::oneshot::channel::<Result<(), ()>>();
  std::thread::spawn(move || {
    std::thread::sleep(std::time::Duration::from_secs(1));
    tx.send(Ok(())).unwrap();
  });
  assert!(rx.await.is_ok());

  Ok("test".to_string())
}

struct TestResource(String);
impl Resource for TestResource {
  fn name(&self) -> Cow<str> {
    "TestResource".into()
  }
}

#[allow(clippy::unnecessary_wraps)]
fn op_test_resource_table_add(
  state: &mut OpState,
  text: String,
  _: (),
) -> Result<u32, AnyError> {
  println!("Hello from resource_table.add plugin op.");

  Ok(state.resource_table.add(TestResource(text)))
}

fn op_test_resource_table_get(
  state: &mut OpState,
  rid: ResourceId,
  _: (),
) -> Result<String, AnyError> {
  println!("Hello from resource_table.get plugin op.");

  Ok(
    state
      .resource_table
      .get::<TestResource>(rid)
      .ok_or_else(bad_resource_id)?
      .0
      .clone(),
  )
}
