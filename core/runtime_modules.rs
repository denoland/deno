use std::cell::RefCell;
use std::rc::Rc;

use crate::error::AnyError;
use crate::{OpId, OpState, OpFn};

pub type SourcePair = (&'static str, &'static str);
pub type OpPair = (&'static str, Box<OpFn>);
pub type RcOpRegistrar = Rc<RefCell<dyn OpRegistrar>>;
pub trait OpStateFn: Fn(&mut OpState) -> Result<(), AnyError> {}

// JsRuntimeModule defines a common interface followed by all op_crates
// so the JsRuntime can handle initialization consistently (e.g: snapshots or not)
// this makes op_crates plug-n-play, since modules can be simply passed to a JsRuntime:
// ```
// JsRuntime:new(RuntimeOptions{
//  modules: vec![deno_url::init(), deno_webgpu::init(), ... ],
//  ..Default::default()
// })
pub trait JsRuntimeModule {
  /// This function returns JS source code to be loaded into the isolate (either at snapshotting, or at startup).
  /// as a vector of a tuple of the file name, and the source code. 
  fn init_js(&self) -> Result<Vec<SourcePair>, AnyError> {
    // default implementation of `init_js` is to load no runtime code
    Ok(vec![])
  }

  /// This function can set up the initial op-state of an isolate at startup. 
  fn init_state(&self, _state: &mut OpState) -> Result<(), AnyError> {
    // default implementation of `init_state is to not mutate the state
    Ok(())
  }

  /// This function lets you middleware the op registrations. This function gets called before this module's init_ops.
  fn init_op_registrar_middleware(&self, registrar: RcOpRegistrar) -> RcOpRegistrar {
    // default implementation is to not change the registrar
    registrar
  }

  /// This function gets called at startup to initialize the ops in the isolate.
  fn init_ops(&mut self, _registrar: RcOpRegistrar) -> Result<(), AnyError> {
    // default implementation of `init_ops` is to load no runtime code
    Ok(())
  }
}

// A simple JsRuntimeModule that containing only JS
pub struct PureJSModule {
  js_files: Vec<SourcePair>,
}

impl PureJSModule {
  pub fn new(js_files: Vec<SourcePair>) -> Self {
    PureJSModule { js_files }
  }
}

impl JsRuntimeModule for PureJSModule {
  fn init_js(&self) -> Result<Vec<SourcePair>, AnyError> {
    Ok(self.js_files.clone())
  }
}

// A simple JsRuntimeModule with JS & ops, but no op-state
pub struct SimpleOpModule {
  js: PureJSModule,
  ops: Vec<OpPair>,
}

impl SimpleOpModule {
  pub fn new(js_files: Vec<SourcePair>, ops: Vec<OpPair>) -> Self {
    Self {
      js: PureJSModule::new(js_files),
      ops,
    }
  }
}

impl JsRuntimeModule for SimpleOpModule {
  fn init_js(&self) -> Result<Vec<SourcePair>, AnyError> {
    self.js.init_js()
  }
  
  fn init_ops(&mut self, registrar: RcOpRegistrar) -> Result<(), AnyError> {
    // NOTE: not idempotent
    // TODO: fail if self.ops is empty
    for (name, opfn) in self.ops.drain(..) {
      registrar.borrow_mut().register_op(name, opfn);
    }
    Ok(())
  }
}

// A simple JsRuntimeModule with JS, ops & op-state
// TODO: review this name
// has originally named it SimpleOpStateModule which is a bit verbose
// but clarified that it was more involved than SimpleOpModule ...
pub struct SimpleModule {
  opmod: SimpleOpModule,
  opstate_fn: Box<dyn OpStateFn>,
}

impl SimpleModule {
  pub fn new(
    js_files: Vec<SourcePair>, ops: Vec<OpPair>,
    opstate_fn: Box<dyn OpStateFn>) -> Self {
    Self {
      opmod: SimpleOpModule::new(js_files, ops),
      opstate_fn,
    }
  }
}

impl JsRuntimeModule for SimpleModule {
  fn init_js(&self) -> Result<Vec<SourcePair>, AnyError> {
    self.opmod.init_js()
  }
  
  fn init_ops(&mut self, registrar: RcOpRegistrar) -> Result<(), AnyError> {
    self.opmod.init_ops(registrar)
  }
  
  fn init_state(&self, state: &mut OpState) -> Result<(), AnyError> {
    let ofn = &self.opstate_fn;
    ofn(state)
  }
}


// MultiModule allows grouping multiple sub-JsRuntimeModules into one,
// allowing things such as:
// ```
// fn web_modules(args: WebModuleArgs) -> MultiModule {
//  MultiModule::new(vec![deno_url::init(), deno_fetch::init(...), ...])
// }
// ```
pub struct MultiModule<'s> {
  pub modules: Vec<Box<dyn JsRuntimeModule + 's>>,
}

impl MultiModule<'_> {
  fn new<'s>(modules: &mut Vec<impl JsRuntimeModule + 's>) -> MultiModule<'s> {
    let modules = modules.drain(..).map(|m| Box::<dyn JsRuntimeModule + 's>::from(Box::new(m))).collect();
    MultiModule { modules }
  }
}

impl JsRuntimeModule for MultiModule<'_> {
  fn init_js(&self) -> Result<Vec<SourcePair>, AnyError> {
    Ok(self.modules.iter().map(|m| m.init_js().unwrap()).flatten().collect())
  }
  
  fn init_ops(&mut self, registrar: RcOpRegistrar) -> Result<(), AnyError> {
    for m in self.modules.iter_mut() {
      m.init_ops(registrar.clone())?;
    }
    Ok(())
  }
}

// The OpRegistrar trait allows building op "middleware" such as:
// OpMetrics, OpTracing or OpDisabler that wrap OpFns for profiling, debugging, etc...
// JsRuntime is itself an OpRegistrar
pub trait OpRegistrar {
  fn register_op(&mut self, name: &str, op_fn: Box<OpFn>) -> OpId;
  // register_minimal_op_sync(...)
  // register_minimal_op_async(...)
  // register_json_op_sync(...)
  // register_json_op_async(...)
}
