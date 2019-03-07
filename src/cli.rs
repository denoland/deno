use crate::isolate_init::IsolateInit;
use crate::isolate_state::IsolateState;
use crate::permissions::DenoPermissions;
use crate::modules::Modules;
use deno_core::deno_buf;

/// Implements deno_core::Behavior for the main Deno command-line.
struct Cli {
  // maybe need this: timeout_due: Cell<Option<Instant>>,
  shared: Vec<u8>, // Pin<Vec<u8>> ?
  init: IsolateInit,
  pub modules: RefCell<Modules>,
  pub state: Arc<IsolateState>,
  pub permissions: Arc<DenoPermissions>,
}

impl Cli {
  fn new(init: IsolateInit, state: Arc<IsolateState>, permissions: DenoPermissions) -> Self {
    let shared = Vec::new(1024 * 1024);
    Self {
      init,
      shared,
      modules: RefCell::new(Modules::new()),
      state,
      permissions: Arc::new(permissions),
    }
  }
}

impl Behavior<Buf> for Cli {
  fn startup_snapshot(&self) -> Option<deno_buf> {
    self.init.snapshot
  }

  fn startup_shared(&self) -> Option<deno_buf> {
    let ptr = self.shared.as_ptr() as *const u8;
    let len = self.shared.len();
    Some(unsafe { deno_buf::from_raw_parts(ptr, len) })
  }

  fn resolve(&mut self, specifier: &str, referrer: deno_mod) -> deno_mod;

  fn recv(&mut self, record: R, zero_copy_buf: deno_buf) -> (bool, Box<Op<R>>);

  fn records_reset(&mut self);
  fn records_push(&mut self, record: R) -> bool;
  fn records_pop(&mut self) -> Option<R>;
}
