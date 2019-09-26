extern crate deno;
use deno::*;

pub fn init(&mut isolate: Isolate) -> Result<(), ErrBox> {
  isolate.register_op("hello", op_hello);

  // TODO Somehow define register_deno_global
  isolate.register_deno_global("src/hello.ts", "hello")
}

fn op_hello(_control_buf: &[u8], _zero_copy_buf: Option<PinnedBuf>) -> CoreOp {
  println!("Hello world");
  CoreOp::Sync(Box::new([]))
}

#[test]
fn hello_rust() {
  match op_hello() {
    CoreOp::Sync(buf) => {
      assert_eq!(buf.len(), 0);
    }
    CoreOp::Async(_) => unreachable!(),
  }
}

#[test]
fn hello_js() {
  // TODO need to define run_js_test somehow...
  run_js_test("src/hello_test.ts");
}
