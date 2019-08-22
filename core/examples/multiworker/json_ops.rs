use deno::Buf;
use deno::CoreOp;
use deno::Op;
use deno::PinnedBuf;
use futures::future::Future;
use serde::Serialize;
use serde_json::Value;

pub type JsonOpAsyncFuture<V> = Box<dyn Future<Item = V, Error = ()> + Send>;

#[allow(dead_code)]
pub enum JsonOp<V>
where
  V: Serialize,
{
  Sync(V),
  Async(JsonOpAsyncFuture<V>),
}

pub fn wrap_json_op<D, V>(
  d: D,
  control: &[u8],
  buf: Option<PinnedBuf>,
) -> CoreOp
where
  D: FnOnce(Value, Option<PinnedBuf>) -> JsonOp<V>,
  V: Serialize + 'static,
{
  let args = serde_json::from_slice(control).unwrap();
  match d(args, buf) {
    JsonOp::<V>::Sync(value) => {
      let result_json = serde_json::to_string(&value).unwrap();
      Op::Sync(result_json.as_bytes().into())
    }
    JsonOp::<V>::Async(fut) => {
      let result_fut = Box::new(fut.and_then(move |value| {
        let result_json = serde_json::to_string(&value).unwrap();
        let result_buf: Buf = result_json.as_bytes().into();
        Ok(result_buf)
      }));
      Op::Async(result_fut)
    }
  }
}

#[derive(Serialize)]
pub struct EmptyResponse;
