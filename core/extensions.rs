use crate::error::AnyError;
use crate::{OpFn, OpState};

pub type SourcePair = (&'static str, &'static str);
pub type OpPair = (&'static str, Box<OpFn>);
pub type OpMiddlewareFn = dyn Fn(&'static str, Box<OpFn>) -> Box<OpFn>;
pub type OpStateFn = dyn Fn(&mut OpState) -> Result<(), AnyError>;

#[derive(Default)]
pub struct Extension {
  js_files: Option<Vec<SourcePair>>,
  ops: Option<Vec<OpPair>>,
  opstate_fn: Option<Box<OpStateFn>>,
  middleware_fn: Option<Box<OpMiddlewareFn>>,
  initialized: bool,
}

impl Extension {
  pub fn new(
    js_files: Option<Vec<SourcePair>>,
    ops: Option<Vec<OpPair>>,
    opstate_fn: Option<Box<OpStateFn>>,
    middleware_fn: Option<Box<OpMiddlewareFn>>,
  ) -> Self {
    Self {
      js_files,
      ops,
      opstate_fn,
      middleware_fn,
      initialized: false,
    }
  }

  pub fn pure_js(js_files: Vec<SourcePair>) -> Self {
    Self::new(Some(js_files), None, None, None)
  }

  pub fn with_ops(
    js_files: Vec<SourcePair>,
    ops: Vec<OpPair>,
    opstate_fn: Option<Box<OpStateFn>>,
  ) -> Self {
    Self::new(Some(js_files), Some(ops), opstate_fn, None)
  }
}

// Note: this used to be a trait, but we "downgraded" it to a single concrete type
// for the initial iteration, it will likely become a trait in the future
impl Extension {
  /// returns JS source code to be loaded into the isolate (either at snapshotting,
  /// or at startup).  as a vector of a tuple of the file name, and the source code.
  pub(crate) fn init_js(&self) -> Vec<SourcePair> {
    match &self.js_files {
      Some(files) => files.clone(),
      None => vec![],
    }
  }

  /// Called at JsRuntime startup to initialize ops in the isolate.
  pub(crate) fn init_ops(&mut self) -> Option<Vec<OpPair>> {
    // TODO(@AaronO): maybe make op registration idempotent
    if self.initialized {
      panic!("init_ops called twice: not idempotent or correct");
    }
    self.initialized = true;

    self.ops.take()
  }

  /// Allows setting up the initial op-state of an isolate at startup.
  pub(crate) fn init_state(&self, state: &mut OpState) -> Result<(), AnyError> {
    match &self.opstate_fn {
      Some(ofn) => ofn(state),
      None => Ok(()),
    }
  }

  /// init_middleware lets us middleware op registrations, it's called before init_ops
  pub(crate) fn init_middleware(&mut self) -> Option<Box<OpMiddlewareFn>> {
    self.middleware_fn.take()
  }
}

/// Helps embed JS files in an extension. Returns Vec<(&'static str, &'static str)>
/// representing the filename and source code.
///
/// Example:
/// ```ignore
/// include_js_files!(
///   prefix "deno:op_crates/hello",
///   "01_hello.js",
///   "02_goodbye.js",
/// )
/// ```
#[macro_export]
macro_rules! include_js_files {
  (prefix $prefix:literal, $($file:literal,)+) => {
    vec![
      $((
        concat!($prefix, "/", $file),
        include_str!($file),
      ),)+
    ]
  };
}
