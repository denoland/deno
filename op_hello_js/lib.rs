extern crate deno;
use deno::*;

pub fn init(&mut isolate: Isolate) -> Result<(), ErrBox> {
  isolate.register_op("hello", op_hello); // register_op defined by #3002
  isolate.execute("hello.js")?;
  Ok(())
}

fn op_hello(_control_buf: &[u8], _zero_copy_buf: Option<PinnedBuf>) -> CoreOp {
  println!("Hello world");
  CoreOp::Sync(Box::new([]))
}

#[test]
fn js_test() {
  isolate.execute("hello_test.js")
}

#[test]
fn rust_test() {
  if let CoreOp::Sync(buf) = op_hello() {
    assert_eq!(buf.len(), 0);
  } else {
    unreachable!();
  }
}
