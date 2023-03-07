// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
use crate::OpState;
use anyhow::Context as _;
use anyhow::Error;
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::task::Context;
use v8::fast_api::FastFunction;

#[derive(Clone, Debug)]
pub enum ExtensionFileSourceCode {
  /// Source code is included in the binary produced. Either by being defined
  /// inline, or included using `include_str!()`. If you are snapshotting, this
  /// will result in two copies of the source code being included - one in the
  /// snapshot, the other the static string in the `Extension`.
  IncludedInBinary(&'static str),

  // Source code is loaded from a file on disk. It's meant to be used if the
  // embedder is creating snapshots. Files will be loaded from the filesystem
  // during the build time and they will only be present in the V8 snapshot.
  LoadedFromFsDuringSnapshot(PathBuf),
}

impl ExtensionFileSourceCode {
  pub fn load(&self) -> Result<String, Error> {
    match self {
      ExtensionFileSourceCode::IncludedInBinary(code) => Ok(code.to_string()),
      ExtensionFileSourceCode::LoadedFromFsDuringSnapshot(path) => {
        let msg = format!("Failed to read \"{}\"", path.display());
        let code = std::fs::read_to_string(path).context(msg)?;
        Ok(code)
      }
    }
  }
}

#[derive(Clone, Debug)]
pub struct ExtensionFileSource {
  pub specifier: String,
  pub code: ExtensionFileSourceCode,
}
pub type OpFnRef = v8::FunctionCallback;
pub type OpMiddlewareFn = dyn Fn(OpDecl) -> OpDecl;
pub type OpStateFn = dyn Fn(&mut OpState);
pub type OpEventLoopFn = dyn Fn(Rc<RefCell<OpState>>, &mut Context) -> bool;

pub struct OpDecl {
  pub name: &'static str,
  pub v8_fn_ptr: OpFnRef,
  pub enabled: bool,
  pub is_async: bool,
  pub is_unstable: bool,
  pub is_v8: bool,
  pub fast_fn: Option<Box<dyn FastFunction>>,
}

impl OpDecl {
  pub fn enabled(self, enabled: bool) -> Self {
    Self { enabled, ..self }
  }

  pub fn disable(self) -> Self {
    self.enabled(false)
  }
}

#[derive(Default)]
pub struct Extension {
  js_files: Option<Vec<ExtensionFileSource>>,
  esm_files: Option<Vec<ExtensionFileSource>>,
  esm_entry_point: Option<&'static str>,
  ops: Option<Vec<OpDecl>>,
  opstate_fn: Option<Box<OpStateFn>>,
  middleware_fn: Option<Box<OpMiddlewareFn>>,
  event_loop_middleware: Option<Box<OpEventLoopFn>>,
  initialized: bool,
  enabled: bool,
  name: &'static str,
  deps: Option<Vec<&'static str>>,
}

// Note: this used to be a trait, but we "downgraded" it to a single concrete type
// for the initial iteration, it will likely become a trait in the future
impl Extension {
  pub fn builder(name: &'static str) -> ExtensionBuilder {
    ExtensionBuilder {
      name,
      ..Default::default()
    }
  }

  /// Check if dependencies have been loaded, and errors if either:
  /// - The extension is depending on itself or an extension with the same name.
  /// - A dependency hasn't been loaded yet.
  pub fn check_dependencies(&self, previous_exts: &[&mut Extension]) {
    if let Some(deps) = &self.deps {
      'dep_loop: for dep in deps {
        if dep == &self.name {
          panic!("Extension '{}' is either depending on itself or there is another extension with the same name", self.name);
        }

        for ext in previous_exts {
          if dep == &ext.name {
            continue 'dep_loop;
          }
        }

        panic!("Extension '{}' is missing dependency '{dep}'", self.name);
      }
    }
  }

  /// returns JS source code to be loaded into the isolate (either at snapshotting,
  /// or at startup).  as a vector of a tuple of the file name, and the source code.
  pub fn get_js_sources(&self) -> &[ExtensionFileSource] {
    match &self.js_files {
      Some(files) => files,
      None => &[],
    }
  }

  pub fn get_esm_sources(&self) -> &[ExtensionFileSource] {
    match &self.esm_files {
      Some(files) => files,
      None => &[],
    }
  }

  pub fn get_esm_entry_point(&self) -> Option<&'static str> {
    self.esm_entry_point
  }

  /// Called at JsRuntime startup to initialize ops in the isolate.
  pub fn init_ops(&mut self) -> Option<Vec<OpDecl>> {
    // TODO(@AaronO): maybe make op registration idempotent
    if self.initialized {
      panic!("init_ops called twice: not idempotent or correct");
    }
    self.initialized = true;

    let mut ops = self.ops.take()?;
    for op in ops.iter_mut() {
      op.enabled = self.enabled && op.enabled;
    }
    Some(ops)
  }

  /// Allows setting up the initial op-state of an isolate at startup.
  pub fn init_state(&self, state: &mut OpState) {
    if let Some(op_fn) = &self.opstate_fn {
      op_fn(state);
    }
  }

  /// init_middleware lets us middleware op registrations, it's called before init_ops
  pub fn init_middleware(&mut self) -> Option<Box<OpMiddlewareFn>> {
    self.middleware_fn.take()
  }

  pub fn init_event_loop_middleware(&mut self) -> Option<Box<OpEventLoopFn>> {
    self.event_loop_middleware.take()
  }

  pub fn run_event_loop_middleware(
    &self,
    op_state_rc: Rc<RefCell<OpState>>,
    cx: &mut Context,
  ) -> bool {
    self
      .event_loop_middleware
      .as_ref()
      .map(|f| f(op_state_rc, cx))
      .unwrap_or(false)
  }

  pub fn enabled(self, enabled: bool) -> Self {
    Self { enabled, ..self }
  }

  pub fn disable(self) -> Self {
    self.enabled(false)
  }
}

// Provides a convenient builder pattern to declare Extensions
#[derive(Default)]
pub struct ExtensionBuilder {
  js: Vec<ExtensionFileSource>,
  esm: Vec<ExtensionFileSource>,
  esm_entry_point: Option<&'static str>,
  ops: Vec<OpDecl>,
  state: Option<Box<OpStateFn>>,
  middleware: Option<Box<OpMiddlewareFn>>,
  event_loop_middleware: Option<Box<OpEventLoopFn>>,
  name: &'static str,
  deps: Vec<&'static str>,
}

impl ExtensionBuilder {
  pub fn dependencies(&mut self, dependencies: Vec<&'static str>) -> &mut Self {
    self.deps.extend(dependencies);
    self
  }

  pub fn js(&mut self, js_files: Vec<ExtensionFileSource>) -> &mut Self {
    let js_files =
      // TODO(bartlomieju): if we're automatically remapping here, then we should
      // use a different result struct that `ExtensionFileSource` as it's confusing
      // when (and why) the remapping happens.
      js_files.into_iter().map(|file_source| ExtensionFileSource {
        specifier: format!("internal:{}/{}", self.name, file_source.specifier),
        code: file_source.code,
      });
    self.js.extend(js_files);
    self
  }

  pub fn esm(&mut self, esm_files: Vec<ExtensionFileSource>) -> &mut Self {
    let esm_files = esm_files
      .into_iter()
      // TODO(bartlomieju): if we're automatically remapping here, then we should
      // use a different result struct that `ExtensionFileSource` as it's confusing
      // when (and why) the remapping happens.
      .map(|file_source| ExtensionFileSource {
        specifier: format!("internal:{}/{}", self.name, file_source.specifier),
        code: file_source.code,
      });
    self.esm.extend(esm_files);
    self
  }

  pub fn esm_entry_point(&mut self, entry_point: &'static str) -> &mut Self {
    self.esm_entry_point = Some(entry_point);
    self
  }

  pub fn ops(&mut self, ops: Vec<OpDecl>) -> &mut Self {
    self.ops.extend(ops);
    self
  }

  pub fn state<F>(&mut self, opstate_fn: F) -> &mut Self
  where
    F: Fn(&mut OpState) + 'static,
  {
    self.state = Some(Box::new(opstate_fn));
    self
  }

  pub fn middleware<F>(&mut self, middleware_fn: F) -> &mut Self
  where
    F: Fn(OpDecl) -> OpDecl + 'static,
  {
    self.middleware = Some(Box::new(middleware_fn));
    self
  }

  pub fn event_loop_middleware<F>(&mut self, middleware_fn: F) -> &mut Self
  where
    F: Fn(Rc<RefCell<OpState>>, &mut Context) -> bool + 'static,
  {
    self.event_loop_middleware = Some(Box::new(middleware_fn));
    self
  }

  pub fn build(&mut self) -> Extension {
    let js_files = Some(std::mem::take(&mut self.js));
    let esm_files = Some(std::mem::take(&mut self.esm));
    let ops = Some(std::mem::take(&mut self.ops));
    let deps = Some(std::mem::take(&mut self.deps));
    Extension {
      js_files,
      esm_files,
      esm_entry_point: self.esm_entry_point.take(),
      ops,
      opstate_fn: self.state.take(),
      middleware_fn: self.middleware.take(),
      event_loop_middleware: self.event_loop_middleware.take(),
      initialized: false,
      enabled: true,
      name: self.name,
      deps,
    }
  }
}

/// Helps embed JS files in an extension. Returns a vector of
/// `ExtensionFileSource`, that represent the filename and source code. All
/// specified files are rewritten into "internal:<extension_name>/<file_name>".
///
/// An optional "dir" option can be specified to prefix all files with a
/// directory name.
///
/// Example (for "my_extension"):
/// ```ignore
/// include_js_files!(
///   "01_hello.js",
///   "02_goodbye.js",
/// )
/// // Produces following specifiers:
/// - "internal:my_extension/01_hello.js"
/// - "internal:my_extension/02_goodbye.js"
///
/// /// Example with "dir" option (for "my_extension"):
/// ```ignore
/// include_js_files!(
///   dir "js",
///   "01_hello.js",
///   "02_goodbye.js",
/// )
/// // Produces following specifiers:
/// - "internal:my_extension/js/01_hello.js"
/// - "internal:my_extension/js/02_goodbye.js"
/// ```
#[cfg(not(feature = "include_js_files_for_snapshotting"))]
#[macro_export]
macro_rules! include_js_files {
  (dir $dir:literal, $($file:literal,)+) => {
    vec![
      $($crate::ExtensionFileSource {
        specifier: concat!($file).to_string(),
        code: $crate::ExtensionFileSourceCode::IncludedInBinary(
          include_str!(concat!($dir, "/", $file)
        )),
      },)+
    ]
  };

  ($($file:literal,)+) => {
    vec![
      $($crate::ExtensionFileSource {
        specifier: $file.to_string(),
        code: $crate::ExtensionFileSourceCode::IncludedInBinary(
          include_str!($file)
        ),
      },)+
    ]
  };
}

#[cfg(feature = "include_js_files_for_snapshotting")]
#[macro_export]
macro_rules! include_js_files {
  (dir $dir:literal, $($file:literal,)+) => {
    vec![
      $($crate::ExtensionFileSource {
        specifier: concat!($file).to_string(),
        code: $crate::ExtensionFileSourceCode::LoadedFromFsDuringSnapshot(
          std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join($dir).join($file)
        ),
      },)+
    ]
  };

  ($($file:literal,)+) => {
    vec![
      $($crate::ExtensionFileSource {
        specifier: $file.to_string(),
        code: $crate::ExtensionFileSourceCode::LoadedFromFsDuringSnapshot(
          std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join($file)
        ),
      },)+
    ]
  };
}
