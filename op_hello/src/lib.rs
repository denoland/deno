extern crate deno;
extern crate deno_std;
use deno::*;

pub fn init(&mut isolate: Isolate) -> Result<(), ErrBox> {
  isolate.register_op("hello", op_hello); // register_op defined by #3002

  // Explicitly link the deno_std crate so it can be used in hello_test.ts
  // Its usage looks like this:
  //
  //   import { test } from "crate://deno_std/testing/mod.ts";
  //
  // In the future it might make sense to automate this function away, but I think
  // it would be prudent to make the crate URL resolution as obvious as
  // possible.
  isolate.register_crate_url("deno_std", deno_std::get_file);

  // TODO The ability to run typescript doesn't exist in deno core.
  isolate.run("src/hello.ts")
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
  deno_test("src/hello_test.ts"); // TODO implement deno_test
}
