use std::cell::RefCell;
use std::rc::Rc;

use crate::error::AnyError;
use crate::{OpFn, OpId, OpState};

pub type SourcePair = (&'static str, &'static str);
pub type OpPair = (&'static str, Box<OpFn>);
pub type RcOpRegistrar = Rc<RefCell<dyn OpRegistrar>>;
pub type OpMiddlewareFn = dyn Fn(&'static str, Box<OpFn>) -> Box<OpFn>;
pub type OpStateFn = dyn Fn(&mut OpState) -> Result<(), AnyError>;

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
  fn init_registrar(&mut self, registrar: RcOpRegistrar) -> RcOpRegistrar {
    // default implementation is to not change the registrar
    registrar
  }

  /// This function gets called at startup to initialize the ops in the isolate.
  fn init_ops(&mut self, _registrar: RcOpRegistrar) -> Result<(), AnyError> {
    // default implementation of `init_ops` is to load no runtime code
    Ok(())
  }
}

// A simple JsRuntimeModule
#[derive(Default)]
pub struct BasicModule {
  js_files: Option<Vec<SourcePair>>,
  ops: Option<Vec<OpPair>>,
  opstate_fn: Option<Box<OpStateFn>>,
  middleware_fn: Option<Box<OpMiddlewareFn>>,
}

impl BasicModule {
  pub fn new(
    js_files: Option<Vec<SourcePair>>,
    ops: Option<Vec<OpPair>>,
    opstate_fn: Option<Box<OpStateFn>>,
  ) -> Self {
    Self {
      js_files,
      ops,
      opstate_fn,
      middleware_fn: None,
    }
  }

  pub fn pure_js(js_files: Vec<SourcePair>) -> Self {
    Self::new(Some(js_files), None, None)
  }

  pub fn with_ops(
    js_files: Vec<SourcePair>,
    ops: Vec<OpPair>,
    opstate_fn: Option<Box<OpStateFn>>,
  ) -> Self {
    Self::new(Some(js_files), Some(ops), opstate_fn)
  }
  
  pub fn builder() -> BasicModuleBuilder {
    BasicModuleBuilder { ..Default::default() }
  }
}

impl JsRuntimeModule for BasicModule {
  fn init_js(&self) -> Result<Vec<SourcePair>, AnyError> {
    Ok(match &self.js_files {
      Some(files) => files.clone(),
      None => vec![],
    })
  }

  fn init_ops(&mut self, registrar: RcOpRegistrar) -> Result<(), AnyError> {
    // NOTE: not idempotent
    // TODO: fail if called twice ?
    if let Some(ops) = self.ops.take() {
      for (name, opfn) in ops {
        registrar.borrow_mut().register_op(name, opfn);
      }
    }
    Ok(())
  }

  fn init_state(&self, state: &mut OpState) -> Result<(), AnyError> {
    match &self.opstate_fn {
      Some(ofn) => ofn(state),
      None => Ok(()),
    }
  }
  
  fn init_registrar(&mut self, registrar: RcOpRegistrar) -> RcOpRegistrar {
    match self.middleware_fn.take() {
      Some(middleware_fn) => Rc::new(RefCell::new(OpMiddleware{ 
        registrar,
        middleware_fn,
      })),
      None => registrar,
    }
  }
}

// Provides a convenient builder pattern to declare BasicModules
#[derive(Default)]
pub struct BasicModuleBuilder {
  js_files: Vec<SourcePair>,
  ops: Vec<OpPair>,
  opstate_fn: Option<Box<OpStateFn>>,
  middleware_fn: Option<Box<OpMiddlewareFn>>,
}

impl BasicModuleBuilder {
  pub fn js(&mut self, js_files: Vec<SourcePair>) -> &mut Self {
    self.js_files.extend(js_files);
    self
  }
  
  pub fn ops(&mut self, ops: Vec<OpPair>) -> &mut Self {
    self.ops.extend(ops);
    self
  }
  
  pub fn state<F>(&mut self, opstate_fn: F) -> &mut Self
    where F: Fn(&mut OpState) -> Result<(), AnyError> + 'static {
    self.opstate_fn = Some(Box::new(opstate_fn));
    self
  }
  
  pub fn middleware<F>(&mut self, middleware_fn: F) -> &mut Self
    where F: Fn(&'static str, Box<OpFn>) -> Box<OpFn> + 'static {
    self.middleware_fn = Some(Box::new(middleware_fn));
    self
  }
  
  pub fn build(&mut self) -> BasicModule {
    let js_files = Some(self.js_files.drain(..).collect());
    let ops = Some(self.ops.drain(..).collect());
    BasicModule {
      js_files,
      ops,
      opstate_fn: self.opstate_fn.take(),
      middleware_fn: self.middleware_fn.take(),
    }
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
    let modules = modules
      .drain(..)
      .map(|m| Box::<dyn JsRuntimeModule + 's>::from(Box::new(m)))
      .collect();
    MultiModule { modules }
  }
}

impl JsRuntimeModule for MultiModule<'_> {
  fn init_js(&self) -> Result<Vec<SourcePair>, AnyError> {
    Ok(
      self
        .modules
        .iter()
        .map(|m| m.init_js().unwrap())
        .flatten()
        .collect(),
    )
  }

  fn init_ops(&mut self, registrar: RcOpRegistrar) -> Result<(), AnyError> {
    for m in self.modules.iter_mut() {
      m.init_ops(registrar.clone())?;
    }
    Ok(())
  }
  
  fn init_state(&self, state: &mut OpState) -> Result<(), AnyError> {
    for m in self.modules.iter() {
      m.init_state(state)?;
    }
    Ok(())
  }
  
  fn init_registrar(&mut self, registrar: RcOpRegistrar) -> RcOpRegistrar {
    let mut registrar = registrar;
    for m in self.modules.iter_mut() {
      registrar = m.init_registrar(registrar.clone());
    }
    registrar
  }
}

// The OpRegistrar trait allows building op "middleware" such as:
// OpMetrics, OpTracing or OpDisabler that wrap OpFns for profiling, debugging, etc...
// JsRuntime is itself an OpRegistrar
pub trait OpRegistrar {
  fn register_op(&mut self, name: &'static str, op_fn: Box<OpFn>) -> OpId;
  // register_minimal_op_sync(...)
  // register_minimal_op_async(...)
  // register_json_op_sync(...)
  // register_json_op_async(...)
}

// OpMiddleware wraps an original OpRegistrar with an OpMiddlewareFn
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

////
// Helper macros to reduce verbosity / redundant decls
////

// include_js_files! helps embed JS files in an extension
// Example:
// ```
// include_js_files!(
//   prefix "deno:op_crates/hello",
//   "01_hello.js",
//   "02_goodbye.js",
// )
// ```
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

// declare_ops! helps declare ops for an extension.
// Example:
// ```
//  declare_ops(json_op_sync[
//    op_foo,
//    op_bar,
//  ]),
// ```
// TODO: improve robustness by handling different func patterns in a single block
#[macro_export]
macro_rules! declare_ops {
  // Match plain function identifiers, e.g: op_foo
  ($wrapper:path[$($opfn:ident,)+]) => {
    vec![$((
      stringify!($opfn),
      $wrapper($opfn),
    ),)+]
  };

  // Match prefixed function identifiers, e.g: mod_a::op_foo
  ($wrapper:path[$($path:ident::$opfn:ident,)+]) => {
    vec![$((
      stringify!($opfn),
      $wrapper($path::$opfn),
    ),)+]
  };

  // TODO: support matching funcs with type-parameters (e.g: permissions)
}

// Groups a sequence of declare_ops!() calls into a single vec.
// Example:
// ```
// declare_ops_group(vec![
//  declare_ops(json_op_sync[
//    ...,
//  ]),
//  declare_ops(json_op_async[
//    ...,
//  ]),
// ])
// ```
pub fn declare_ops_group(
  groups: Vec<Vec<(&'static str, Box<OpFn>)>>,
) -> Vec<(&'static str, Box<OpFn>)> {
  groups
    .into_iter()
    .fold(vec![].into_iter(), |v, g| {
      v.chain(g.into_iter())
        .collect::<Vec<(&'static str, Box<OpFn>)>>()
        .into_iter()
    })
    .collect()
}
