extern crate deno;
use deno::*;

pub fn init(&mut isolate: Isolate) {
  isolate.register_op("hello", op_hello);
}

fn op_hello(_control_buf: &[u8], _zero_copy_buf: Option<PinnedBuf>) -> CoreOp {
  println!("Hello world");
  CoreOp::Sync(Box::new([]))
}

#[test]
fn basic_test() {
  match op_hello() {
    CoreOp::Sync(buf) => {
      assert_eq!(buf.len(), 0);
    }
    CoreOp::Async(_) => {
      unreachable!()
    }
  }
}

