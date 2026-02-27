// Copyright 2018-2025 the Deno authors. MIT license.

use crate::JsRuntime;
use crate::RuntimeOptions;
use crate::op2;
use deno_error::JsErrorBox;
use std::future::poll_fn;
use std::task::Poll;

#[tokio::test]
async fn test_error_builder() {
  #[op2(fast)]
  fn op_err() -> Result<(), JsErrorBox> {
    Err(JsErrorBox::new("DOMExceptionOperationError", "abc"))
  }

  deno_core::extension!(test_ext, ops = [op_err]);
  let mut runtime = JsRuntime::new(RuntimeOptions {
    extensions: vec![test_ext::init()],
    ..Default::default()
  });
  poll_fn(move |cx| {
    runtime
      .execute_script(
        "error_builder_test.js",
        include_str!("error_builder_test.js"),
      )
      .unwrap();
    if let Poll::Ready(Err(_)) = runtime.poll_event_loop(cx, Default::default())
    {
      unreachable!();
    }
    Poll::Ready(())
  })
  .await;
}

#[test]
fn syntax_error() {
  let mut runtime = JsRuntime::new(Default::default());
  let src = "hocuspocus(";
  let js_error = runtime.execute_script("i.js", src).unwrap_err();
  let frame = js_error.frames.first().unwrap();
  assert_eq!(frame.column_number, Some(12));
}
