use std::cell::RefCell;
use std::rc::Rc;

use crate::error::AnyError;
use crate::{OpFn, OpId, OpState};

pub type SourcePair = (&'static str, &'static str);
pub type OpPair = (&'static str, Box<OpFn>);
pub type RcOpRegistrar = Rc<RefCell<dyn OpRegistrar>>;
pub type OpMiddlewareFn = dyn Fn(&'static str, Box<OpFn>) -> Box<OpFn>;
pub type OpStateFn = dyn Fn(&mut OpState) -> Result<(), AnyError>;

#[derive(Default)]
pub struct Extension {
  js_files: Option<Vec<SourcePair>>,
  ops: Option<Vec<OpPair>>,
  opstate_fn: Option<Box<OpStateFn>>,
  middleware_fn: Option<Box<OpMiddlewareFn>>,
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
// for the initial iteration, it will like become a trait in the future
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
  pub(crate) fn init_ops(&mut self, registrar: RcOpRegistrar) {
    // NOTE: not idempotent
    // TODO: fail if called twice ?
    if let Some(ops) = self.ops.take() {
      for (name, opfn) in ops {
        registrar.borrow_mut().register_op(name, opfn);
      }
    }
  }

  /// Allows setting up the initial op-state of an isolate at startup.
  pub(crate) fn init_state(&self, state: &mut OpState) -> Result<(), AnyError> {
    match &self.opstate_fn {
      Some(ofn) => ofn(state),
      None => Ok(()),
    }
  }

  /// init_registrar lets us middleware op registrations, it's called before init_ops
  pub(crate) fn init_registrar(
    &mut self,
    registrar: RcOpRegistrar,
  ) -> RcOpRegistrar {
    match self.middleware_fn.take() {
      Some(middleware_fn) => Rc::new(RefCell::new(OpMiddleware {
        registrar,
        middleware_fn,
      })),
      None => registrar,
    }
  }
}

/// The OpRegistrar trait allows building op "middleware" such as:
/// OpMetrics, OpTracing or OpDisabler that wrap OpFns for profiling, debugging, etc...
/// JsRuntime is itself an OpRegistrar
pub trait OpRegistrar {
  fn register_op(&mut self, name: &'static str, op_fn: Box<OpFn>) -> OpId;
}

/// OpMiddleware wraps an original OpRegistrar with an OpMiddlewareFn
pub struct OpMiddleware {
  registrar: RcOpRegistrar,
  middleware_fn: Box<OpMiddlewareFn>,
}

impl OpRegistrar for OpMiddleware {
  fn register_op(&mut self, name: &'static str, op_fn: Box<OpFn>) -> OpId {
    let new_op = (self.middleware_fn)(name, op_fn);
    self.registrar.borrow_mut().register_op(name, new_op)
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
