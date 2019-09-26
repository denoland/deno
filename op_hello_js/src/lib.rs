extern crate deno;
extern crate deno_std;
use deno::*;

pub fn init(&mut isolate: Isolate) -> Result<(), ErrBox> {
  isolate.register_op("hello", op_hello); // register_op defined by #3002

  isolate.execute("src/hello.js")
}

fn op_hello(_control_buf: &[u8], _zero_copy_buf: Option<PinnedBuf>) -> CoreOp {
  println!("Hello world");
  CoreOp::Sync(Box::new([]))
}

#[test]
fn rust_test() {
  match op_hello() {
    CoreOp::Sync(buf) => {
      assert_eq!(buf.len(), 0);
    }
    CoreOp::Async(_) => unreachable!(),
  }
}

#[test]
fn js_test() {
  isolate.execute("src/hello_test.js")
}
