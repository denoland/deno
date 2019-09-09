use deno::CoreOp;
use deno::Named;
use deno::OpDispatcher;
use deno::PinnedBuf;
use deno_dispatch_json::JsonErrBox;
use std::ops::Deref;
use std::sync::Arc;

pub type CheckReadFn =
  dyn Fn(&str) -> Result<(), JsonErrBox> + Send + Sync + 'static;
pub type CheckWriteFn =
  dyn Fn(&str) -> Result<(), JsonErrBox> + Send + Sync + 'static;

pub struct FsOpsState {
  check_read_fn: Box<CheckReadFn>,
  check_write_fn: Box<CheckWriteFn>,
}

// TODO(afinch7) maybe replace this with a common permissions
// trait?
/// Thread safe state for fs op dispatchers
pub struct TSFsOpsState(Arc<FsOpsState>);

impl Clone for TSFsOpsState {
  fn clone(&self) -> Self {
    TSFsOpsState(self.0.clone())
  }
}

impl Deref for TSFsOpsState {
  type Target = Arc<FsOpsState>;
  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl TSFsOpsState {
  pub fn new<R, W>(check_read_fn: R, check_write_fn: W) -> Self
  where
    R: Fn(&str) -> Result<(), JsonErrBox> + Send + Sync + 'static,
    W: Fn(&str) -> Result<(), JsonErrBox> + Send + Sync + 'static,
  {
    Self(Arc::new(FsOpsState {
      check_read_fn: Box::new(check_read_fn),
      check_write_fn: Box::new(check_write_fn),
    }))
  }

  #[inline]
  pub fn check_read(&self, filename: &str) -> Result<(), JsonErrBox> {
    (*self.check_read_fn)(filename)
  }

  #[inline]
  pub fn check_write(&self, filename: &str) -> Result<(), JsonErrBox> {
    (*self.check_write_fn)(filename)
  }

  pub fn wrap_op<D>(&self, d: D) -> WrappedFsOpDispatcher<D>
  where
    D: FsOpDispatcher,
  {
    WrappedFsOpDispatcher::new(d, self.clone())
  }
}

pub trait FsOpDispatcher: Send + Sync {
  fn dispatch(
    &self,
    state: &TSFsOpsState,
    args: &[u8],
    buf: Option<PinnedBuf>,
  ) -> CoreOp;

  const NAME: &'static str;
}

pub struct WrappedFsOpDispatcher<D: FsOpDispatcher> {
  inner: D,
  state: TSFsOpsState,
}

impl<D: FsOpDispatcher> WrappedFsOpDispatcher<D> {
  pub fn new(d: D, state: TSFsOpsState) -> Self {
    Self { inner: d, state }
  }
}

impl<D> OpDispatcher for WrappedFsOpDispatcher<D>
where
  D: FsOpDispatcher,
{
  fn dispatch(&self, control: &[u8], zero_copy: Option<PinnedBuf>) -> CoreOp {
    self.inner.dispatch(&self.state, control, zero_copy)
  }
}

impl<D> Named for WrappedFsOpDispatcher<D>
where
  D: FsOpDispatcher,
{
  const NAME: &'static str = D::NAME;
}
