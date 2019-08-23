use deno::Buf;
use deno::CoreOp;
use deno::ErrBox;
use deno::Op;
use deno::PinnedBuf;
use futures::future::Future;

#[derive(Copy, Clone, Debug, PartialEq)]
// This corresponds to RecordMinimal on the TS side.
pub struct Record {
  pub promise_id: i32,
  pub arg: i32,
  pub result: i32,
}

impl Into<Buf> for Record {
  fn into(self) -> Buf {
    let vec = vec![self.promise_id, self.arg, self.result];
    let buf32 = vec.into_boxed_slice();
    let ptr = Box::into_raw(buf32) as *mut [u8; 3 * 4];
    unsafe { Box::from_raw(ptr) }
  }
}

pub fn parse_min_record(bytes: &[u8]) -> Option<Record> {
  if bytes.len() % std::mem::size_of::<i32>() != 0 {
    return None;
  }
  let p = bytes.as_ptr();
  #[allow(clippy::cast_ptr_alignment)]
  let p32 = p as *const i32;
  let s = unsafe { std::slice::from_raw_parts(p32, bytes.len() / 4) };

  if s.len() != 3 {
    return None;
  }
  let ptr = s.as_ptr();
  let ints = unsafe { std::slice::from_raw_parts(ptr, 3) };
  Some(Record {
    promise_id: ints[0],
    arg: ints[1],
    result: ints[2],
  })
}

pub type MinimalOp = dyn Future<Item = i32, Error = ErrBox> + Send;

pub fn wrap_minimal<D>(
  d: D,
  control: &[u8],
  zero_copy: Option<PinnedBuf>,
) -> CoreOp
where
  D: FnOnce(i32, Option<PinnedBuf>) -> Box<MinimalOp>,
{
  let mut record = parse_min_record(control).unwrap();
  let is_sync = record.promise_id == 0;

  let rid = record.arg;
  let min_op = d(rid, zero_copy);

  let fut = Box::new(min_op.then(move |result| -> Result<Buf, ()> {
    match result {
      Ok(r) => {
        record.result = r;
      }
      Err(err) => {
        // TODO(ry) The dispatch_minimal doesn't properly pipe errors back to
        // the caller.
        dbg!(format!("swallowed err {}", err));
        record.result = -1;
      }
    }
    let buf: Buf = record.into();
    Ok(buf)
  }));
  if is_sync {
    Op::Sync(fut.wait().unwrap())
  } else {
    Op::Async(fut)
  }
}
