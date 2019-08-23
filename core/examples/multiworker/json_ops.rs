use deno::Buf;
use deno::CoreOp;
use deno::ErrBox;
use deno::PinnedBuf;
use futures::future::Future;
use serde::Deserialize;
use serde_json::json;
use serde_json::Value;

pub type AsyncJsonOp = Box<dyn Future<Item = Value, Error = ErrBox> + Send>;

#[allow(dead_code)]
pub enum JsonOp {
  Sync(Value),
  Async(AsyncJsonOp),
}

fn json_err(err: ErrBox) -> Value {
  json!({
    "message": err.to_string(),
  })
}

fn serialize_result(
  promise_id: Option<u64>,
  result: Result<Value, ErrBox>,
) -> Buf {
  let value = match result {
    Ok(v) => json!({ "ok": v, "promiseId": promise_id }),
    Err(err) => json!({ "err": json_err(err), "promiseId": promise_id }),
  };
  let vec = serde_json::to_vec(&value).unwrap();
  vec.into_boxed_slice()
}

pub fn wrap_json_op<D>(
  d: D,
  control: &[u8],
  zero_copy: Option<PinnedBuf>,
) -> CoreOp
where
  D: FnOnce(Value, Option<PinnedBuf>) -> Result<JsonOp, ErrBox>,
{
  let async_args: AsyncArgs = serde_json::from_slice(control).unwrap();
  let promise_id = async_args.promise_id;
  let is_sync = promise_id.is_none();

  let result = serde_json::from_slice(control)
    .map_err(ErrBox::from)
    .and_then(move |args| d(args, zero_copy));
  match result {
    Ok(JsonOp::Sync(sync_value)) => {
      assert!(promise_id.is_none());
      CoreOp::Sync(serialize_result(promise_id, Ok(sync_value)))
    }
    Ok(JsonOp::Async(fut)) => {
      assert!(promise_id.is_some());
      let fut2 = Box::new(fut.then(move |result| -> Result<Buf, ()> {
        Ok(serialize_result(promise_id, result))
      }));
      CoreOp::Async(fut2)
    }
    Err(sync_err) => {
      let buf = serialize_result(promise_id, Err(sync_err));
      if is_sync {
        CoreOp::Sync(buf)
      } else {
        CoreOp::Async(Box::new(futures::future::ok(buf)))
      }
    }
  }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AsyncArgs {
  promise_id: Option<u64>,
}
