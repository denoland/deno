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

/// Declares a block of Deno `#[op]`s. The first parameter determines the name of the
/// op declaration block, and is usually `deno_ops`. This block generates a function that
/// returns a [`Vec<OpDecl>`].
///
/// This can be either a compact form like:
///
/// ```no_compile
/// # use deno_core::*;
/// #[op]
/// fn op_xyz() {}
///
/// deno_core::ops!(deno_ops, [
///   op_xyz
/// ]);
///
/// // Use the ops:
/// deno_ops()
/// ```
///
/// ... or a parameterized form like so that allows passing a number of type parameters
/// to each `#[op]`:
///
/// ```no_compile
/// # use deno_core::*;
/// #[op]
/// fn op_xyz<P>() where P: Clone {}
///
/// deno_core::ops!(deno_ops,
///   parameters = [P: Clone],
///   ops = [
///     op_xyz<P>
///   ]
/// );
///
/// // Use the ops, with `String` as the parameter `P`:
/// deno_ops::<String>()
/// ```
#[macro_export]
macro_rules! ops {
  ($name:ident, parameters = [ $( $param:ident : $type:ident ),+ ], ops = [ $( $(#[$m:meta])* $( $op:ident )::+ $( < $op_param:ident > )?  ),+ $(,)? ]) => {
    pub(crate) fn $name < $( $param : $type + 'static ),+ > () -> Vec<$crate::OpDecl> {
      let mut v = vec![];
      $(
        $( #[ $m ] )*
        {
          let decl = $( $op )::+ :: decl $( :: <$op_param> )? ();
          v.push(decl);
        }
      )+
      v
    }
  };
  ($name:ident, [ $( $(#[$m:meta])* $( $op:ident )::+ ),+ $(,)? ] ) => {
    pub(crate) fn $name() -> Vec<$crate::OpDecl> {
      vec![
        $( $( #[ $m ] )* $( $op )::+ :: decl(), )+
      ]
    }
  }
}

/// Declares a bundle of Deno `#[op]` blocks.
#[macro_export]
macro_rules! ops_bundle {
  ($name:ident, parameters = [ $( $param:ident : $type:ident ),+ ], ops = [ $( $(#[$m:meta])* $( $op:ident )::+ $( < $op_param:ident > )?  ),+ $(,)? ]) => {
    pub(crate) fn $name < $( $param : $type + 'static ),+ > () -> Vec<$crate::OpDecl> {
      let mut v = vec![];
      $(
        $( #[ $m ] )*
        {
          let decl = $( $op )::+ $( :: <$op_param> )? ();
          v.extend(decl);
        }
      )+
      v
    }
  };
  ($name:ident, [ $( $(#[$m:meta])* $( $op:ident )::+ ),+ $(,)? ] ) => {
    pub(crate) fn $name() -> Vec<$crate::OpDecl> {
      let mut v = vec![];
      $(
        $( #[ $m ] )*
        {
          v.extend($( $op )::+ ());
        }
      )+
      v
    }
  }
}

#[macro_export]
macro_rules! extension {
  (
    $name:ident,
    $(deps = [ $( $dep:ident ),* ],)?
    $(parameters = [ $( $param:ident : $type:ident ),+ ], )?
    $(ops_fn = $ops_symbol:ident $( < $ops_param:ident > )?,)?
    $(esm = [ $( dir $dir_esm:literal , )? $( $esm:literal ),* $(,)? ],)?
    $(js = [ $( dir $dir_js:literal , )? $( $js:literal ),* $(,)? ],)?
    $(config = { $( $config_id:ident : $config_type:ty ),* $(,)? },)?
    $(state = $state_fn:expr, )?
    $(event_loop_middleware = $event_loop_middleware_fn:ident, )?
  ) => {
    /// Extension struct.
    pub mod $name {
      use super::*;

      fn ext() -> $crate::ExtensionBuilder {
        $crate::Extension::builder_with_deps(stringify!($name), &[ $( $( stringify!($dep) ),* )? ])
      }

      /// If ESM or JS was specified, add those files to the extension.
      #[inline(always)]
      fn with_js(ext: &mut $crate::ExtensionBuilder) {
        $( ext.esm(
          $crate::include_js_files!( $( dir $dir_esm , )? $( $esm , )* )
        ); )?
        $( ext.js(
          $crate::include_js_files!( $( dir $dir_js , )? $( $js , )* )
        ); )?
      }

      // If ops were specified, add those ops to the extension.
      #[inline(always)]
      fn with_ops $( <  $( $param : $type + Clone + 'static ),+ > )?(ext: &mut $crate::ExtensionBuilder) {
        $crate::extension!(__ops__ ext $( $ops_symbol $( < $ops_param > )? )? __eot__);
      }

      // Includes the state and middleware functions, if defined.
      #[inline(always)]
      #[allow(unused_variables)]
      fn with_state_and_middleware$( <  $( $param : $type + Clone + 'static ),+ > )?(ext: &mut $crate::ExtensionBuilder, $( $( $config_id : $config_type ),* )? ) {
        #[allow(unused_variables)]
        let config = Config {
          $( $( $config_id , )* )?
          $( __phantom_data: ::std::marker::PhantomData::<($( $param ),+)>::default() )?
        };

        $(
          ext.state(move |state: &mut $crate::OpState| {
            config.clone().call_callback(state, $state_fn)
          });
        )?

        $(
          ext.event_loop_middleware($event_loop_middleware_fn);
        )?
      }

      #[doc(hidden)]
      #[derive(Clone)]
      struct Config $( <  $( $param : $type + Clone + 'static ),+ > )? {
        $( $( pub $config_id : $config_type , )* )?
        $( __phantom_data: ::std::marker::PhantomData<($( $param ),+)>, )?
      }

      impl $( <  $( $param : $type + Clone + 'static ),+ > )? Config $( <  $( $param ),+ > )? {
        /// Call a function of |state, ...| using the fields of this configuration structure.
        #[allow(dead_code)]
        #[doc(hidden)]
        fn call_callback(self, state: &mut $crate::OpState, f: fn(&mut $crate::OpState, $( $( $config_type ),* )? )) {
          f(state, $( $( self. $config_id ),* )? )
        }
      }

      #[allow(dead_code)]
      pub fn init_js_only $( <  $( $param : $type + Clone + 'static ),+ > )? () -> $crate::Extension {
        let mut ext = ext();
        // If esm or JS was specified, add JS files
        with_js(&mut ext);
        with_ops $( ::<($( $param ),+)> )?(&mut ext);
        ext.build()
      }

      #[allow(dead_code)]
      pub fn init_ops_and_esm $( <  $( $param : $type + Clone + 'static ),+ > )? ( $( $( $config_id : $config_type ),* )? ) -> $crate::Extension {
        let mut ext = ext();
        // If esm or JS was specified, add JS files
        with_js(&mut ext);
        with_ops $( ::<($( $param ),+)> )?(&mut ext);
        with_state_and_middleware $( ::<($( $param ),+)> )?(&mut ext, $( $( $config_id , )* )? );
        ext.build()
      }

      #[allow(dead_code)]
      pub fn init_ops $( <  $( $param : $type + Clone + 'static ),+ > )? ( $( $( $config_id : $config_type ),* )? ) -> $crate::Extension {
        let mut ext = ext();
        with_ops $( ::<($( $param ),+)> )?(&mut ext);
        with_state_and_middleware $( ::<($( $param ),+)> )?(&mut ext, $( $( $config_id , )* )? );
        ext.build()
      }
    }
  };

  (__ops__ $ext:ident __eot__) => {
    $ext
  };

  (__ops__ $ext:ident $ops_symbol:ident __eot__) => {
    $ext.ops($ops_symbol())
  };

  (__ops__ $ext:ident $ops_symbol:ident < $ops_param:ident > __eot__) => {
    $ext.ops($ops_symbol::<$ops_param>())
  };
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
  deps: Option<&'static [&'static str]>,
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

  pub fn builder_with_deps(
    name: &'static str,
    deps: &'static [&'static str],
  ) -> ExtensionBuilder {
    ExtensionBuilder {
      name,
      deps,
      ..Default::default()
    }
  }

  /// Check if dependencies have been loaded, and errors if either:
  /// - The extension is depending on itself or an extension with the same name.
  /// - A dependency hasn't been loaded yet.
  pub fn check_dependencies(&self, previous_exts: &[Extension]) {
    if let Some(deps) = self.deps {
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
  pub fn get_js_sources(&self) -> Option<&Vec<ExtensionFileSource>> {
    self.js_files.as_ref()
  }

  pub fn get_esm_sources(&self) -> Option<&Vec<ExtensionFileSource>> {
    self.esm_files.as_ref()
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
  deps: &'static [&'static str],
}

impl ExtensionBuilder {
  pub fn js(&mut self, js_files: Vec<ExtensionFileSource>) -> &mut Self {
    let js_files =
      // TODO(bartlomieju): if we're automatically remapping here, then we should
      // use a different result struct that `ExtensionFileSource` as it's confusing
      // when (and why) the remapping happens.
      js_files.into_iter().map(|file_source| ExtensionFileSource {
        specifier: format!("ext:{}/{}", self.name, file_source.specifier),
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
        specifier: format!("ext:{}/{}", self.name, file_source.specifier),
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
/// specified files are rewritten into "ext:<extension_name>/<file_name>".
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
/// - "ext:my_extension/01_hello.js"
/// - "ext:my_extension/02_goodbye.js"
///
/// /// Example with "dir" option (for "my_extension"):
/// ```ignore
/// include_js_files!(
///   dir "js",
///   "01_hello.js",
///   "02_goodbye.js",
/// )
/// // Produces following specifiers:
/// - "ext:my_extension/js/01_hello.js"
/// - "ext:my_extension/js/02_goodbye.js"
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
