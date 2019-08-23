use crate::tokio_util;
use deno::Buf;
use deno::ErrBox;
use deno::Op;
use deno::OpResult;
use futures::Poll;

pub type CliOpResult = OpResult<ErrBox>;

#[inline]
pub fn ok_buf(buf: Buf) -> CliOpResult {
  Ok(Op::Sync(buf))
}

#[inline]
pub fn empty_buf() -> Buf {
  Box::new([])
}

// This is just type conversion. Implement From trait?
// See https://github.com/tokio-rs/tokio/blob/ffd73a64e7ec497622b7f939e38017afe7124dc4/tokio-fs/src/lib.rs#L76-L85
pub fn convert_blocking<F>(f: F) -> Poll<Buf, ErrBox>
where
  F: FnOnce() -> Result<Buf, ErrBox>,
{
  use futures::Async::*;
  match tokio_threadpool::blocking(f) {
    Ok(Ready(Ok(v))) => Ok(v.into()),
    Ok(Ready(Err(err))) => Err(err),
    Ok(NotReady) => Ok(NotReady),
    Err(err) => panic!("blocking error {}", err),
  }
}

pub fn blocking<F>(is_sync: bool, f: F) -> CliOpResult
where
  F: 'static + Send + FnOnce() -> Result<Buf, ErrBox>,
{
  if is_sync {
    let result_buf = f()?;
    Ok(Op::Sync(result_buf))
  } else {
    Ok(Op::Async(Box::new(futures::sync::oneshot::spawn(
      tokio_util::poll_fn(move || convert_blocking(f)),
      &tokio_executor::DefaultExecutor::current(),
    ))))
  }
}
