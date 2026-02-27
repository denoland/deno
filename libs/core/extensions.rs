// Copyright 2018-2025 the Deno authors. MIT license.

use crate::FastStaticString;
use crate::OpState;
use crate::modules::IntoModuleCodeString;
use crate::modules::ModuleCodeString;
use crate::ops::OpMetadata;
use crate::runtime::bindings;
use std::borrow::Cow;
use std::marker::PhantomData;
use std::sync::Arc;
use v8::MapFnTo;
use v8::fast_api::CFunction;
use v8::fast_api::CFunctionInfo;
use v8::fast_api::Int64Representation;
use v8::fast_api::Type;

#[derive(Clone)]
pub enum ExtensionFileSourceCode {
  /// Source code is included in the binary produced. Either by being defined
  /// inline, or included using `include_str!()`. If you are snapshotting, this
  /// will result in two copies of the source code being included - one in the
  /// snapshot, the other the static string in the `Extension`.
  #[deprecated = "Use ExtensionFileSource::new"]
  IncludedInBinary(FastStaticString),

  // Source code is loaded from a file on disk. It's meant to be used if the
  // embedder is creating snapshots. Files will be loaded from the filesystem
  // during the build time and they will only be present in the V8 snapshot.
  LoadedFromFsDuringSnapshot(&'static str), // <- Path

  // Source code was loaded from memory. It's meant to be used if the
  // embedder is creating snapshots. Files will be loaded from memory
  // during the build time and they will only be present in the V8 snapshot.
  LoadedFromMemoryDuringSnapshot(FastStaticString),

  /// Source code may be computed at runtime.
  Computed(Arc<str>),
}

#[allow(deprecated)]
impl std::fmt::Debug for ExtensionFileSourceCode {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match *self {
      Self::IncludedInBinary(..) => write!(f, "IncludedInBinary(..)"),
      Self::LoadedFromFsDuringSnapshot(path) => {
        write!(f, "LoadedFromFsDuringSnapshot({path})")
      }
      Self::LoadedFromMemoryDuringSnapshot(..) => {
        write!(f, "LoadedFromMemoryDuringSnapshot(..)")
      }
      Self::Computed(..) => write!(f, "Computed(..)"),
    }
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExtensionSourceType {
  LazyEsm,
  Js,
  Esm,
}

#[derive(Clone, Debug)]
pub struct ExtensionFileSource {
  pub specifier: &'static str,
  pub code: ExtensionFileSourceCode,
  _unconstructable_use_new: PhantomData<()>,
}

impl ExtensionFileSource {
  pub const fn new(specifier: &'static str, code: FastStaticString) -> Self {
    #[allow(deprecated)]
    Self {
      specifier,
      code: ExtensionFileSourceCode::IncludedInBinary(code),
      _unconstructable_use_new: PhantomData,
    }
  }

  pub const fn new_computed(specifier: &'static str, code: Arc<str>) -> Self {
    #[allow(deprecated)]
    Self {
      specifier,
      code: ExtensionFileSourceCode::Computed(code),
      _unconstructable_use_new: PhantomData,
    }
  }

  pub const fn loaded_during_snapshot(
    specifier: &'static str,
    path: &'static str,
  ) -> Self {
    #[allow(deprecated)]
    Self {
      specifier,
      code: ExtensionFileSourceCode::LoadedFromFsDuringSnapshot(path),
      _unconstructable_use_new: PhantomData,
    }
  }

  pub const fn loaded_from_memory_during_snapshot(
    specifier: &'static str,
    code: FastStaticString,
  ) -> Self {
    #[allow(deprecated)]
    Self {
      specifier,
      code: ExtensionFileSourceCode::LoadedFromMemoryDuringSnapshot(code),
      _unconstructable_use_new: PhantomData,
    }
  }

  fn find_non_ascii(s: &str) -> String {
    s.chars().filter(|c| !c.is_ascii()).collect::<String>()
  }

  #[allow(deprecated)]
  pub fn load(&self) -> Result<ModuleCodeString, std::io::Error> {
    match &self.code {
      ExtensionFileSourceCode::LoadedFromMemoryDuringSnapshot(code)
      | ExtensionFileSourceCode::IncludedInBinary(code) => {
        debug_assert!(
          code.is_ascii(),
          "Extension code must be 7-bit ASCII: {} (found {})",
          self.specifier,
          Self::find_non_ascii(code)
        );
        Ok(IntoModuleCodeString::into_module_code(*code))
      }
      ExtensionFileSourceCode::LoadedFromFsDuringSnapshot(path) => {
        let s = std::fs::read_to_string(path)?;
        debug_assert!(
          s.is_ascii(),
          "Extension code must be 7-bit ASCII: {} (found {})",
          self.specifier,
          Self::find_non_ascii(&s)
        );
        Ok(s.into())
      }
      ExtensionFileSourceCode::Computed(code) => {
        debug_assert!(
          code.is_ascii(),
          "Extension code must be 7-bit ASCII: {} (found {})",
          self.specifier,
          Self::find_non_ascii(code)
        );
        Ok(ModuleCodeString::from(code.clone()))
      }
    }
  }
}

pub type OpFnRef = v8::FunctionCallback;
pub type OpMiddlewareFn = dyn Fn(OpDecl) -> OpDecl;
pub type OpStateFn = dyn FnOnce(&mut OpState);
/// Trait implemented by all generated ops.
pub trait Op {
  const NAME: &'static str;
  const DECL: OpDecl;
}
pub type GlobalTemplateMiddlewareFn =
  for<'s, 'i> fn(
    &mut v8::PinScope<'s, 'i, ()>,
    v8::Local<'s, v8::ObjectTemplate>,
  ) -> v8::Local<'s, v8::ObjectTemplate>;
pub type GlobalObjectMiddlewareFn =
  for<'s, 'i> fn(&mut v8::PinScope<'s, 'i>, v8::Local<'s, v8::Object>);

extern "C" fn noop() {}

const NOOP_FN: CFunction = CFunction::new(
  noop as _,
  &CFunctionInfo::new(Type::Void.as_info(), &[], Int64Representation::Number),
);

// Declaration for object wrappers.
#[derive(Clone, Copy)]
pub struct OpMethodDecl {
  pub type_name: fn() -> &'static str,
  pub name: (&'static str, FastStaticString),
  pub constructor: Option<OpDecl>,
  pub methods: &'static [OpDecl],
  pub static_methods: &'static [OpDecl],
  pub inherits_type_name: fn() -> Option<&'static str>,
}

#[derive(Clone, Copy, PartialEq)]
pub enum AccessorType {
  Getter,
  Setter,
  None,
}

#[derive(Clone, Copy)]
pub struct OpDecl {
  pub name: &'static str,
  pub name_fast: FastStaticString,
  pub is_async: bool,
  pub is_reentrant: bool,
  pub symbol_for: bool,
  pub accessor_type: AccessorType,
  pub arg_count: u8,
  pub no_side_effects: bool,
  /// The slow dispatch call. If metrics are disabled, the `v8::Function` is created with this callback.
  pub(crate) slow_fn: OpFnRef,
  /// The slow dispatch call with metrics enabled. If metrics are enabled, the `v8::Function` is created with this callback.
  pub(crate) slow_fn_with_metrics: OpFnRef,
  /// The fast dispatch call. If metrics are disabled, the `v8::Function`'s fastcall is created with this callback.
  pub(crate) fast_fn: Option<CFunction>,
  /// The fast dispatch call with metrics enabled. If metrics are enabled, the `v8::Function`'s fastcall is created with this callback.
  pub(crate) fast_fn_with_metrics: Option<CFunction>,
  /// Any metadata associated with this op.
  pub metadata: OpMetadata,
}

impl OpDecl {
  /// For use by internal op implementation only.
  #[doc(hidden)]
  #[allow(clippy::too_many_arguments)]
  pub const fn new_internal_op2(
    name: (&'static str, FastStaticString),
    is_async: bool,
    is_reentrant: bool,
    symbol_for: bool,
    arg_count: u8,
    no_side_effects: bool,
    slow_fn: OpFnRef,
    slow_fn_with_metrics: OpFnRef,
    accessor_type: AccessorType,
    fast_fn: Option<CFunction>,
    fast_fn_with_metrics: Option<CFunction>,
    metadata: OpMetadata,
  ) -> Self {
    #[allow(deprecated)]
    Self {
      name: name.0,
      name_fast: name.1,
      is_async,
      is_reentrant,
      symbol_for,
      arg_count,
      no_side_effects,
      slow_fn,
      slow_fn_with_metrics,
      accessor_type,
      fast_fn,
      fast_fn_with_metrics,
      metadata,
    }
  }

  pub fn is_accessor(&self) -> bool {
    self.accessor_type != AccessorType::None
  }

  /// Returns a copy of this `OpDecl` that replaces underlying functions
  /// with noops.
  pub fn disable(self) -> Self {
    Self {
      slow_fn: bindings::op_disabled_fn.map_fn_to(),
      slow_fn_with_metrics: bindings::op_disabled_fn.map_fn_to(),
      // TODO(bartlomieju): Currently this fast fn won't throw like `op_disabled_fn`;
      // ideally we would add a fallback that would throw, but it's unclear
      // if disabled op (that throws in JS) would ever get optimized to become
      // a fast function.
      fast_fn: self.fast_fn.map(|_| NOOP_FN),
      fast_fn_with_metrics: self.fast_fn_with_metrics.map(|_| NOOP_FN),
      ..self
    }
  }

  /// Returns a copy of this `OpDecl` with the implementation function set to the function from another
  /// `OpDecl`.
  pub const fn with_implementation_from(mut self, from: &Self) -> Self {
    self.slow_fn = from.slow_fn;
    self.slow_fn_with_metrics = from.slow_fn_with_metrics;
    self.fast_fn = from.fast_fn;
    self.fast_fn_with_metrics = from.fast_fn_with_metrics;
    self
  }

  #[doc(hidden)]
  pub const fn fast_fn(&self) -> CFunction {
    let Some(f) = self.fast_fn else {
      panic!("Not a fast function");
    };
    f
  }

  #[doc(hidden)]
  pub const fn fast_fn_with_metrics(&self) -> CFunction {
    let Some(f) = self.fast_fn_with_metrics else {
      panic!("Not a fast function");
    };
    f
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
    pub(crate) fn $name < $( $param : $type + 'static ),+ > () -> ::std::vec::Vec<$crate::OpDecl> {
      vec![
      $(
        $( #[ $m ] )*
        $( $op )+ $( :: <$op_param> )? () ,
      )+
      ]
    }
  };
  ($name:ident, [ $( $(#[$m:meta])* $( $op:ident )::+ ),+ $(,)? ] ) => {
    pub(crate) fn $name() -> ::std::Vec<$crate::OpDecl> {
      use $crate::Op;
      vec![
        $( $( #[ $m ] )* $( $op )+() , )+
      ]
    }
  }
}

/// Return the first argument if not empty, otherwise the second.
#[macro_export]
macro_rules! or {
  ($e:expr_2021, $fallback:expr_2021) => {
    $e
  };
  (, $fallback:expr_2021) => {
    $fallback
  };
}

/// Defines a Deno extension. The first parameter is the name of the extension symbol namespace to create. This is the symbol you
/// will use to refer to the extension.
///
/// Most extensions will define a combination of ops and ESM files, like so:
///
/// ```no_compile
/// #[op]
/// fn op_xyz() {
/// }
///
/// deno_core::extension!(
///   my_extension,
///   ops = [ op_xyz ],
///   esm = [ "my_script.js" ],
///   docs = "A small sample extension"
/// );
/// ```
///
/// The following options are available for the [`extension`] macro:
///
///  * deps: a comma-separated list of module dependencies, eg: `deps = [ my_other_extension ]`
///  * parameters: a comma-separated list of parameters and base traits, eg: `parameters = [ P: MyTrait ]`
///  * bounds: a comma-separated list of additional type bounds, eg: `bounds = [ P::MyAssociatedType: MyTrait ]`
///  * ops: a comma-separated list of [`OpDecl`]s to provide, eg: `ops = [ op_foo, op_bar ]`
///  * esm: a comma-separated list of ESM module filenames (see [`include_js_files`]), eg: `esm = [ dir "dir", "my_file.js" ]`
///  * lazy_loaded_esm: a comma-separated list of ESM module filenames (see [`include_js_files`]), that will be included in
///    the produced binary, but not automatically evaluated. Eg: `lazy_loaded_esm = [ dir "dir", "my_file.js" ]`
///  * js: a comma-separated list of JS filenames (see [`include_js_files`]), eg: `js = [ dir "dir", "my_file.js" ]`
///  * config: a structure-like definition for configuration parameters which will be required when initializing this extension, eg: `config = { my_param: Option<usize> }`
///  * middleware: an [`OpDecl`] middleware function with the signature `fn (OpDecl) -> OpDecl`
///  * state: a state initialization function, with the signature `fn (&mut OpState, ...) -> ()`, where `...` are parameters matching the fields of the config struct
///  * global_template_middleware: a global template middleware function (see [`Extension::global_template_middleware`])
///  * global_object_middleware: a global object middleware function (see [`Extension::global_object_middleware`])
///  * docs: comma separated list of toplevel #[doc=...] tags to be applied to the extension's resulting struct
#[macro_export]
macro_rules! extension {
  (
    $name:ident
    $(, deps = [ $( $dep:ident ),* ] )?
    $(, parameters = [ $( $param:ident : $type:ident ),+ ] )?
    $(, bounds = [ $( $bound:path : $bound_type:ident ),+ ] )?
    $(, ops_fn = $ops_symbol:ident $( < $ops_param:ident > )? )?
    $(, ops = [ $( $(#[$m:meta])* $( $op:ident )::+ $( < $( $op_param:ident ),* > )?  ),+ $(,)? ] )?
    $(, objects = [ $( $(#[$masd:meta])* $( $object:ident )::+ ),+ $(,)? ] )?
    $(, esm_entry_point = $esm_entry_point:expr_2021 )?
    $(, esm = [ $($esm:tt)* ] )?
    $(, lazy_loaded_esm = [ $($lazy_loaded_esm:tt)* ] )?
    $(, js = [ $($js:tt)* ] )?
    $(, options = { $( $options_id:ident : $options_type:ty ),* $(,)? } )?
    $(, middleware = $middleware_fn:expr_2021 )?
    $(, state = $state_fn:expr_2021 )?
    $(, global_template_middleware = $global_template_middleware_fn:expr_2021 )?
    $(, global_object_middleware = $global_object_middleware_fn:expr_2021 )?
    $(, external_references = [ $( $external_reference:expr_2021 ),* $(,)? ] )?
    $(, customizer = $customizer_fn:expr_2021 )?
    $(, docs = $($docblocks:expr_2021),+)?
    $(,)?
  ) => {
    $( $(#[doc = $docblocks])+ )?
    ///
    /// An extension for use with the Deno JS runtime.
    /// To use it, provide it as an argument when instantiating your runtime:
    ///
    /// ```rust,ignore
    /// use deno_core::{ JsRuntime, RuntimeOptions };
    ///
    #[doc = concat!("let mut extensions = vec![", stringify!($name), "::init()];")]
    /// let mut js_runtime = JsRuntime::new(RuntimeOptions {
    ///   extensions,
    ///   ..Default::default()
    /// });
    /// ```
    ///
    #[allow(non_camel_case_types)]
    pub struct $name {
    }

    impl $name {
      fn ext $( <  $( $param : $type + 'static ),+ > )?() -> $crate::Extension {
        #[allow(unused_imports)]
        use $crate::Op;
        $crate::Extension {
          // Computed at compile-time, may be modified at runtime with `Cow`:
          name: ::std::stringify!($name),
          deps: &[ $( $( ::std::stringify!($dep) ),* )? ],
          // Use intermediary `const`s here to disable user expressions which
          // can't be evaluated at compile-time.
          js_files: {
            const JS: &'static [$crate::ExtensionFileSource] = &$crate::include_js_files!( $name $($($js)*)? );
            ::std::borrow::Cow::Borrowed(JS)
          },
          esm_files: {
            const JS: &'static [$crate::ExtensionFileSource] = &$crate::include_js_files!( $name $($($esm)*)? );
            ::std::borrow::Cow::Borrowed(JS)
          },
          lazy_loaded_esm_files: {
            const JS: &'static [$crate::ExtensionFileSource] = &$crate::include_lazy_loaded_js_files!( $name $($($lazy_loaded_esm)*)? );
            ::std::borrow::Cow::Borrowed(JS)
          },
          esm_entry_point: {
            const V: ::std::option::Option<&'static ::std::primitive::str> = $crate::or!($(::std::option::Option::Some($esm_entry_point))?, ::std::option::Option::None);
            V
          },
          ops: ::std::borrow::Cow::Owned(vec![$($({
            $( #[ $m ] )*
            $( $op )::+ $( :: < $($op_param),* > )? ()
          }),+)?]),
          objects: ::std::borrow::Cow::Borrowed(&[$($({
            $( $object )::+::DECL
          }),+)?]),
          external_references: ::std::borrow::Cow::Borrowed(&[ $( $external_reference ),* ]),
          global_template_middleware: ::std::option::Option::None,
          global_object_middleware: ::std::option::Option::None,
          // Computed at runtime:
          op_state_fn: ::std::option::Option::None,
          needs_lazy_init: false,
          middleware_fn: ::std::option::Option::None,
          enabled: true,
        }
      }

      // If ops were specified, add those ops to the extension.
      #[inline(always)]
      #[allow(unused_variables)]
      fn with_ops_fn $( <  $( $param : $type + 'static ),+ > )?(ext: &mut $crate::Extension)
      $( where $( $bound : $bound_type ),+ )?
      {
        // Use the ops_fn, if provided
        $crate::extension!(! __ops__ ext $( $ops_symbol $( < $ops_param > )? )? __eot__);
      }

      // Includes the state and middleware functions, if defined.
      #[inline(always)]
      #[allow(unused_variables)]
      fn with_middleware $( <  $( $param : $type + 'static ),+ > )?(ext: &mut $crate::Extension)
      {
        $(
          ext.global_template_middleware = ::std::option::Option::Some($global_template_middleware_fn);
        )?

        $(
          ext.global_object_middleware = ::std::option::Option::Some($global_object_middleware_fn);
        )?

        $(
          ext.middleware_fn = ::std::option::Option::Some(::std::boxed::Box::new($middleware_fn));
        )?
      }

      #[inline(always)]
      #[allow(unused_variables)]
      #[allow(clippy::redundant_closure_call)]
      fn with_customizer(ext: &mut $crate::Extension) {
        $( ($customizer_fn)(ext); )?
      }

      /// Initialize this extension for runtime or snapshot creation.
      ///
      /// # Returns
      /// an Extension object that can be used during instantiation of a JsRuntime
      #[allow(dead_code)]
      pub fn init $( <  $( $param : $type + 'static ),+ > )? ( $( $( $options_id : $options_type ),* )? ) -> $crate::Extension
      $( where $( $bound : $bound_type ),+ )?
      {
        let mut ext = Self::ext $( ::< $( $param ),+ > )?();
        Self::with_ops_fn $( ::< $( $param ),+ > )?(&mut ext);
        $crate::extension!(! __config__ ext $( parameters = [ $( $param : $type ),* ] )? $( config = { $( $options_id : $options_type ),* } )? $( state_fn = $state_fn )? );
        Self::with_middleware $( ::< $( $param ),+ > )?(&mut ext);
        Self::with_customizer(&mut ext);
        ext
      }

      /// Initialize this extension for runtime or snapshot creation.
      ///
      /// If this method is used, you must later call `JsRuntime::lazy_init_extensions`
      /// with the result of this extension's `args` method.
      ///
      /// # Returns
      /// an Extension object that can be used during instantiation of a JsRuntime
      #[allow(dead_code)]
      pub fn lazy_init $( <  $( $param : $type + 'static ),+ > )? () -> $crate::Extension
      $( where $( $bound : $bound_type ),+ )?
      {
        let mut ext = Self::ext $( ::< $( $param ),+ > )?();
        Self::with_ops_fn $( ::< $( $param ),+ > )?(&mut ext);
        ext.needs_lazy_init = true;
        Self::with_middleware$( ::< $( $param ),+ > )?(&mut ext);
        Self::with_customizer(&mut ext);
        ext
      }

      /// Create an `ExtensionArguments` value which must be passed to
      /// `JsRuntime::lazy_init_extensions`.
      #[allow(dead_code, unused_mut)]
      pub fn args $( <  $( $param : $type + 'static ),+ > )? ( $( $( $options_id : $options_type ),* )? ) -> $crate::ExtensionArguments
      $( where $( $bound : $bound_type ),+ )?
      {
        let mut args = $crate::ExtensionArguments {
          name: ::std::stringify!($name),
          op_state_fn: ::std::option::Option::None,
        };

        $crate::extension!(! __config__ args $( parameters = [ $( $param : $type ),* ] )? $( config = { $( $options_id : $options_type ),* } )? $( state_fn = $state_fn )? );

        args
      }
    }
  };

  // This branch of the macro generates a config object that calls the state function with itself.
  (! __config__ $args:ident $( parameters = [ $( $param:ident : $type:ident ),+ ] )? config = { $( $options_id:ident : $options_type:ty ),* } $( state_fn = $state_fn:expr_2021 )? ) => {
    {
      #[doc(hidden)]
      struct Config $( <  $( $param : $type + 'static ),+ > )? {
        $( pub $options_id : $options_type , )*
        $( __phantom_data: ::std::marker::PhantomData<($( $param ),+)>, )?
      }
      let config = Config {
        $( $options_id , )*
        $( __phantom_data: ::std::marker::PhantomData::<($( $param ),+)>::default() )?
      };

      let state_fn: fn(&mut $crate::OpState, Config $( <  $( $param ),+ > )? ) = $(  $state_fn  )?;

      $args.op_state_fn = ::std::option::Option::Some(::std::boxed::Box::new(move |state: &mut $crate::OpState| {
        state_fn(state, config);
      }));
    }
  };

  (! __config__ $args:ident $( parameters = [ $( $param:ident : $type:ident ),+ ] )? $( state_fn = $state_fn:expr_2021 )? ) => {
    $( $args.op_state_fn = ::std::option::Option::Some(::std::boxed::Box::new($state_fn)); )?
  };

  (! __ops__ $ext:ident __eot__) => {
  };

  (! __ops__ $ext:ident $ops_symbol:ident __eot__) => {
    $ext.ops.to_mut().extend($ops_symbol())
  };

  (! __ops__ $ext:ident $ops_symbol:ident < $ops_param:ident > __eot__) => {
    $ext.ops.to_mut().extend($ops_symbol::<$ops_param>())
  };
}

pub struct Extension {
  pub name: &'static str,
  pub deps: &'static [&'static str],
  pub js_files: Cow<'static, [ExtensionFileSource]>,
  pub esm_files: Cow<'static, [ExtensionFileSource]>,
  pub lazy_loaded_esm_files: Cow<'static, [ExtensionFileSource]>,
  pub esm_entry_point: Option<&'static str>,
  pub ops: Cow<'static, [OpDecl]>,
  pub objects: Cow<'static, [OpMethodDecl]>,
  pub external_references: Cow<'static, [v8::ExternalReference]>,
  pub global_template_middleware: Option<GlobalTemplateMiddlewareFn>,
  pub global_object_middleware: Option<GlobalObjectMiddlewareFn>,
  pub op_state_fn: Option<Box<OpStateFn>>,
  pub needs_lazy_init: bool,
  pub middleware_fn: Option<Box<OpMiddlewareFn>>,
  pub enabled: bool,
}

impl Extension {
  // Produces a new extension that is suitable for use during the warmup phase.
  //
  // JS sources are not included, and ops are include for external references only.
  pub(crate) fn for_warmup(&self) -> Extension {
    Self {
      op_state_fn: None,
      needs_lazy_init: self.needs_lazy_init,
      middleware_fn: None,
      name: self.name,
      deps: self.deps,
      js_files: Cow::Borrowed(&[]),
      esm_files: Cow::Borrowed(&[]),
      lazy_loaded_esm_files: Cow::Borrowed(&[]),
      esm_entry_point: None,
      ops: self.ops.clone(),
      objects: self.objects.clone(),
      external_references: self.external_references.clone(),
      global_template_middleware: self.global_template_middleware,
      global_object_middleware: self.global_object_middleware,
      enabled: self.enabled,
    }
  }
}

impl Default for Extension {
  fn default() -> Self {
    Self {
      name: "DEFAULT",
      deps: &[],
      js_files: Cow::Borrowed(&[]),
      esm_files: Cow::Borrowed(&[]),
      lazy_loaded_esm_files: Cow::Borrowed(&[]),
      esm_entry_point: None,
      ops: Cow::Borrowed(&[]),
      objects: Cow::Borrowed(&[]),
      external_references: Cow::Borrowed(&[]),
      global_template_middleware: None,
      global_object_middleware: None,
      op_state_fn: None,
      needs_lazy_init: false,
      middleware_fn: None,
      enabled: true,
    }
  }
}

// Note: this used to be a trait, but we "downgraded" it to a single concrete type
// for the initial iteration, it will likely become a trait in the future
impl Extension {
  /// Check if dependencies have been loaded, and errors if either:
  /// - The extension is depending on itself or an extension with the same name.
  /// - A dependency hasn't been loaded yet.
  pub fn check_dependencies(&self, previous_exts: &[Extension]) {
    'dep_loop: for dep in self.deps {
      if dep == &self.name {
        panic!(
          "Extension '{}' is either depending on itself or there is another extension with the same name",
          self.name
        );
      }

      for ext in previous_exts {
        if dep == &ext.name {
          continue 'dep_loop;
        }
      }

      panic!("Extension '{}' is missing dependency '{dep}'", self.name);
    }
  }

  /// returns JS source code to be loaded into the isolate (either at snapshotting,
  /// or at startup).  as a vector of a tuple of the file name, and the source code.
  pub fn get_js_sources(&self) -> &[ExtensionFileSource] {
    &self.js_files
  }

  pub fn get_esm_sources(&self) -> &[ExtensionFileSource] {
    &self.esm_files
  }

  pub fn get_lazy_loaded_esm_sources(&self) -> &[ExtensionFileSource] {
    &self.lazy_loaded_esm_files
  }

  pub fn get_esm_entry_point(&self) -> Option<&'static str> {
    self.esm_entry_point
  }

  pub fn op_count(&self) -> usize {
    self.ops.len()
  }

  pub fn method_op_count(&self) -> usize {
    self.objects.len()
  }

  /// Called at JsRuntime startup to initialize ops in the isolate.
  pub fn init_ops(&mut self) -> &[OpDecl] {
    if !self.enabled {
      for op in self.ops.to_mut() {
        op.disable();
      }
    }
    self.ops.as_ref()
  }

  /// Called at JsRuntime startup to initialize method ops in the isolate.
  pub fn init_method_ops(&self) -> &[OpMethodDecl] {
    self.objects.as_ref()
  }

  /// Allows setting up the initial op-state of an isolate at startup.
  pub fn take_state(&mut self, state: &mut OpState) {
    if let Some(op_fn) = self.op_state_fn.take() {
      op_fn(state);
    }
  }

  /// Middleware should be called before init
  pub fn take_middleware(&mut self) -> Option<Box<OpMiddlewareFn>> {
    self.middleware_fn.take()
  }

  pub fn get_global_template_middleware(
    &mut self,
  ) -> Option<GlobalTemplateMiddlewareFn> {
    self.global_template_middleware
  }

  pub fn get_global_object_middleware(
    &mut self,
  ) -> Option<GlobalObjectMiddlewareFn> {
    self.global_object_middleware
  }

  pub fn get_external_references(&mut self) -> &[v8::ExternalReference] {
    self.external_references.as_ref()
  }

  pub fn enabled(self, enabled: bool) -> Self {
    Self { enabled, ..self }
  }

  pub fn disable(self) -> Self {
    self.enabled(false)
  }
}

/// Holds configuration needed to initialize an extension. Must be passed to
/// `JsRuntime::lazy_init_extensions`.
pub struct ExtensionArguments {
  #[doc(hidden)]
  pub name: &'static str,
  #[doc(hidden)]
  pub op_state_fn: Option<Box<OpStateFn>>,
}

/// Helps embed JS files in an extension. Returns a vector of
/// [`ExtensionFileSource`], that represents the filename and source code.
///
/// ```
/// # use deno_core::include_js_files_doctest as include_js_files;
/// // Example (for "my_extension"):
/// let files = include_js_files!(
///   my_extension
///   "01_hello.js",
///   "02_goodbye.js",
/// );
///
/// // Produces following specifiers:
/// // - "ext:my_extension/01_hello.js"
/// // - "ext:my_extension/02_goodbye.js"
/// ```
///
/// An optional "dir" option can be specified to prefix all files with a
/// directory name.
///
/// ```
/// # use deno_core::include_js_files_doctest as include_js_files;
/// // Example with "dir" option (for "my_extension"):
/// include_js_files!(
///   my_extension
///   dir "js",
///   "01_hello.js",
///   "02_goodbye.js",
/// );
/// // Produces following specifiers:
/// // - "ext:my_extension/js/01_hello.js"
/// // - "ext:my_extension/js/02_goodbye.js"
/// ```
///
/// You may also override the specifiers for each file like so:
///
/// ```
/// # use deno_core::include_js_files_doctest as include_js_files;
/// // Example with "dir" option (for "my_extension"):
/// include_js_files!(
///   my_extension
///   "module:hello" = "01_hello.js",
///   "module:goodbye" = "02_goodbye.js",
/// );
/// // Produces following specifiers:
/// // - "module:hello"
/// // - "module:goodbye"
/// ```
#[macro_export]
macro_rules! include_js_files {
  // Valid inputs:
  //  - "file"
  //  - "file" with_specifier "specifier"
  //  - "specifier" = "file"
  //  - "specifier" = { source = "source" }
  ($name:ident $( dir $dir:literal, )? $(
    $s1:literal
    $(with_specifier $s2:literal)?
    $(= $config:tt)?
  ),* $(,)?) => {
    $crate::__extension_include_js_files_detect!(name=$name, dir=$crate::__extension_root_dir!($($dir)?), $([
      // These entries will be parsed in __extension_include_js_files_inner
      $s1 $(with_specifier $s2)? $(= $config)?
    ]),*)
  };
}

/// Helps embed JS files in an extension. Returns a vector of
/// `ExtensionFileSource`, that represent the filename and source code. All
/// specified files are rewritten into "ext:<extension_name>/<file_name>".
///
/// An optional "dir" option can be specified to prefix all files with a
/// directory name.
///
/// See [`include_js_files!`] for details on available options.
#[macro_export]
macro_rules! include_lazy_loaded_js_files {
  ($name:ident $( dir $dir:literal, )? $(
    $s1:literal
    $(with_specifier $s2:literal)?
    $(= $config:tt)?
  ),* $(,)?) => {
    $crate::__extension_include_js_files_inner!(mode=included, name=$name, dir=$crate::__extension_root_dir!($($dir)?), $([
      // These entries will be parsed in __extension_include_js_files_inner
      $s1 $(with_specifier $s2)? $(= $config)?
    ]),*)
  };
}

/// Used for doctests only. Won't try to load anything from disk.
#[doc(hidden)]
#[macro_export]
macro_rules! include_js_files_doctest {
  ($name:ident $( dir $dir:literal, )? $(
    $s1:literal
    $(with_specifier $s2:literal)?
    $(= $config:tt)?
  ),* $(,)?) => {
    $crate::__extension_include_js_files_inner!(mode=loaded, name=$name, dir=$crate::__extension_root_dir!($($dir)?), $([
      $s1 $(with_specifier $s2)? $(= $config)?
    ]),*)
  };
}

/// When `#[cfg(not(feature = "include_js_files_for_snapshotting"))]` matches, ie: the `include_js_files_for_snapshotting`
/// feature is not set, we want all JS files to be included.
///
/// Maps `(...)` to `(mode=included, ...)`
#[cfg(not(feature = "include_js_files_for_snapshotting"))]
#[doc(hidden)]
#[macro_export]
macro_rules! __extension_include_js_files_detect {
  ($($rest:tt)*) => { $crate::__extension_include_js_files_inner!(mode=included, $($rest)*) };
}

/// When `#[cfg(feature = "include_js_files_for_snapshotting")]` matches, ie: the `include_js_files_for_snapshotting`
/// feature is set, we want the pathnames for the JS files to be included and not the file contents.
///
/// Maps `(...)` to `(mode=loaded, ...)`
#[cfg(feature = "include_js_files_for_snapshotting")]
#[doc(hidden)]
#[macro_export]
macro_rules! __extension_include_js_files_detect {
  ($($rest:tt)*) => { $crate::__extension_include_js_files_inner!(mode=loaded, $($rest)*) };
}

/// This is the core of the [`include_js_files!`] and [`include_lazy_loaded_js_files`] macros. The first
/// rule is the entry point that receives a list of unparsed file entries. Each entry is extracted and
/// then parsed with the `@parse_item` rules.
#[doc(hidden)]
#[macro_export]
macro_rules! __extension_include_js_files_inner {
  // Entry point: (mode=, name=, dir=, [... files])
  (mode=$mode:ident, name=$name:ident, dir=$dir:expr_2021, $([
    $s1:literal
    $(with_specifier $s2:literal)?
    $(= $config:tt)?
  ]),*) => {
    [
      $(
        $crate::__extension_include_js_files_inner!(
          @parse_item
          mode=$mode,
          name=$name,
          dir=$dir,
          $s1 $(with_specifier $s2)? $(= $config)?
        )
      ),*
    ]
  };

  // @parse_item macros will parse a single file entry, and then call @item macros with the destructured data

  // "file" -> Include a file, use the generated specifier
  (@parse_item mode=$mode:ident, name=$name:ident, dir=$dir:expr_2021, $file:literal) => {
    $crate::__extension_include_js_files_inner!(@item mode=$mode, dir=$dir, specifier=concat!("ext:", stringify!($name), "/", $file), file=$file)
  };
  // "file" with_specifier "specifier" -> Include a file, use the provided specifier
  (@parse_item mode=$mode:ident, name=$name:ident, dir=$dir:expr_2021, $file:literal with_specifier $specifier:literal) => {
    {
      #[deprecated="When including JS files 'file with_specifier specifier' is deprecated: use 'specifier = file' instead"]
      struct WithSpecifierIsDeprecated {}
      _ = WithSpecifierIsDeprecated {};
      $crate::__extension_include_js_files_inner!(@item mode=$mode, dir=$dir, specifier=$specifier, file=$file)
    }
  };
  // "specifier" = "file" -> Include a file, use the provided specifier
  (@parse_item mode=$mode:ident, name=$name:ident, dir=$dir:expr_2021, $specifier:literal = $file:literal) => {
    $crate::__extension_include_js_files_inner!(@item mode=$mode, dir=$dir, specifier=$specifier, file=$file)
  };
  // "specifier" = { source = "source" } -> Include a file, use the provided specifier
  (@parse_item mode=$mode:ident, name=$name:ident, dir=$dir:expr_2021, $specifier:literal = { source = $source:literal }) => {
    $crate::__extension_include_js_files_inner!(@item mode=$mode, specifier=$specifier, source=$source)
  };

  // @item macros generate the final output

  // loaded, source
  (@item mode=loaded, specifier=$specifier:expr_2021, source=$source:expr_2021) => {
    $crate::ExtensionFileSource::loaded_from_memory_during_snapshot($specifier, $crate::ascii_str!($source))
  };
  // loaded, file
  (@item mode=loaded, dir=$dir:expr_2021, specifier=$specifier:expr_2021, file=$file:literal) => {
    $crate::ExtensionFileSource::loaded_during_snapshot($specifier, concat!($dir, "/", $file))
  };
  // included, source
  (@item mode=included, specifier=$specifier:expr_2021, source=$source:expr_2021) => {
    $crate::ExtensionFileSource::new($specifier, $crate::ascii_str!($source))
  };
  // included, file
  (@item mode=included, dir=$dir:expr_2021, specifier=$specifier:expr_2021, file=$file:literal) => {
    $crate::ExtensionFileSource::new($specifier, $crate::ascii_str_include!(concat!($dir, "/", $file)))
  };
}

/// Given an optional `$dir`, generates a crate-relative root directory.
#[doc(hidden)]
#[macro_export]
macro_rules! __extension_root_dir {
  () => {
    env!("CARGO_MANIFEST_DIR")
  };
  ($dir:expr_2021) => {
    concat!(env!("CARGO_MANIFEST_DIR"), "/", $dir)
  };
}

#[cfg(test)]
mod tests {
  #[test]
  fn test_include_js() {
    let files = include_js_files!(prefix "00_infra.js", "01_core.js",);
    assert_eq!("ext:prefix/00_infra.js", files[0].specifier);
    assert_eq!("ext:prefix/01_core.js", files[1].specifier);

    let files = include_js_files!(prefix dir ".", "00_infra.js", "01_core.js",);
    assert_eq!("ext:prefix/00_infra.js", files[0].specifier);
    assert_eq!("ext:prefix/01_core.js", files[1].specifier);

    let files = include_js_files!(prefix
      "a" = { source = "b" }
    );
    assert_eq!("a", files[0].specifier);
  }
}
