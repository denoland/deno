use crate::OpFn;
use crate::OpState;
use anyhow::Error;

pub type SourcePair = (&'static str, Box<SourceLoadFn>);
pub type SourceLoadFn = dyn Fn() -> Result<String, Error>;
pub type OpPair = (&'static str, OpFn);
pub type OpInitFn = dyn Fn(&mut RegisterCtx) + 'static;
pub type OpMiddlewareFn = dyn Fn(&'static str, OpFn) -> OpFn;
pub type OpStateFn = dyn Fn(&mut OpState) -> Result<(), Error>;

#[derive(Default)]
pub struct Extension {
  js_files: Option<Vec<SourcePair>>,
  ops_init: Option<Box<OpInitFn>>,
  opstate_fn: Option<Box<OpStateFn>>,
  middleware_fn: Option<Box<OpMiddlewareFn>>,
  initialized: bool,
}

// Note: this used to be a trait, but we "downgraded" it to a single concrete type
// for the initial iteration, it will likely become a trait in the future
impl Extension {
  pub fn builder() -> ExtensionBuilder {
    Default::default()
  }

  /// returns JS source code to be loaded into the isolate (either at snapshotting,
  /// or at startup).  as a vector of a tuple of the file name, and the source code.
  pub fn init_js(&self) -> &[SourcePair] {
    match &self.js_files {
      Some(files) => files,
      None => &[],
    }
  }

  /// Called at JsRuntime startup to initialize ops in the isolate.
  pub fn init_ops(
    &mut self,
    scope: &mut v8::HandleScope,
    obj: v8::Local<v8::Object>,
  ) {
    // TODO(@AaronO): maybe make op registration idempotent
    if self.initialized {
      panic!("init_ops called twice: not idempotent or correct");
    }
    self.initialized = true;

    if let Some(init) = &self.ops_init {
      let mut ctx = RegisterCtx::Init { scope, obj };
      init(&mut ctx);
    }
  }

  pub fn init_external_references(
    &mut self,
    references: &mut Vec<v8::ExternalReference>,
  ) {
    if let Some(init) = &self.ops_init {
      let mut ctx = RegisterCtx::ExternalReference { references };
      init(&mut ctx);
    }
  }

  /// Allows setting up the initial op-state of an isolate at startup.
  pub fn init_state(&self, state: &mut OpState) -> Result<(), Error> {
    match &self.opstate_fn {
      Some(ofn) => ofn(state),
      None => Ok(()),
    }
  }

  /// init_middleware lets us middleware op registrations, it's called before init_ops
  pub fn init_middleware(&mut self) -> Option<Box<OpMiddlewareFn>> {
    self.middleware_fn.take()
  }
}

// Provides a convenient builder pattern to declare Extensions
#[derive(Default)]
pub struct ExtensionBuilder {
  js: Vec<SourcePair>,
  ops_init: Option<Box<OpInitFn>>,
  state: Option<Box<OpStateFn>>,
  middleware: Option<Box<OpMiddlewareFn>>,
}

pub enum RegisterCtx<'a, 'b, 'c> {
  Init {
    scope: &'a mut v8::HandleScope<'b>,
    obj: v8::Local<'c, v8::Object>,
  },
  ExternalReference {
    references: &'a mut Vec<v8::ExternalReference<'b>>,
  },
}

impl<'a, 'b, 'c> RegisterCtx<'a, 'b, 'c> {
  pub fn register<F>(&mut self, name: &'static str, op_fn: F)
  where
    F: v8::MapFnTo<v8::FunctionCallback>,
  {
    match self {
      RegisterCtx::Init { scope, obj } => {
        crate::bindings::set_func(scope, *obj, name, op_fn)
      }
      RegisterCtx::ExternalReference { references } => {
        references.push(v8::ExternalReference {
          function: op_fn.map_fn_to(),
        });
      }
    }
  }
}

impl ExtensionBuilder {
  pub fn js(&mut self, js_files: Vec<SourcePair>) -> &mut Self {
    self.js.extend(js_files);
    self
  }

  pub fn ops<F>(&mut self, ops: F) -> &mut Self
  where
    F: Fn(&mut RegisterCtx) + 'static,
  {
    self.ops_init = Some(Box::new(ops));
    self
  }

  pub fn state<F>(&mut self, opstate_fn: F) -> &mut Self
  where
    F: Fn(&mut OpState) -> Result<(), Error> + 'static,
  {
    self.state = Some(Box::new(opstate_fn));
    self
  }

  pub fn middleware<F>(&mut self, middleware_fn: F) -> &mut Self
  where
    F: Fn(&'static str, OpFn) -> OpFn + 'static,
  {
    self.middleware = Some(Box::new(middleware_fn));
    self
  }

  pub fn build(&mut self) -> Extension {
    let js_files = Some(std::mem::take(&mut self.js));
    Extension {
      js_files,
      ops_init: self.ops_init.take(),
      opstate_fn: self.state.take(),
      middleware_fn: self.middleware.take(),
      initialized: false,
    }
  }
}
/// Helps embed JS files in an extension. Returns Vec<(&'static str, Box<SourceLoadFn>)>
/// representing the filename and source code. This is only meant for extensions
/// that will be snapshotted, as code will be loaded at runtime.
///
/// Example:
/// ```ignore
/// include_js_files!(
///   prefix "deno:extensions/hello",
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
        Box::new(|| {
          let c = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
          let path = c.join($file);
          // println!("cargo:rerun-if-changed={}", path.display());
          let src = std::fs::read_to_string(path)?;
          Ok(src)
        }),
      ),)+
    ]
  };
}
