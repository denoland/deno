use deno::CoreOp;
use deno::Named;
use deno::Op;
use deno::OpDispatcher;
use deno::PinnedBuf;

struct OpListen {}

struct OpNewStateWorker {}

impl OpDispatcher for OpNewStateWorker {
  fn dispatch(args: &[u8], buf: Option<PinnedBuf>) -> CoreOp {}
}
