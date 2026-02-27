// Copyright 2018-2025 the Deno authors. MIT license.

use crate::_ops::OpMethodDecl;
use crate::ExtensionFileSource;
use crate::FastString;
use crate::ModuleCodeString;
use crate::OpDecl;
use crate::OpMetricsFactoryFn;
use crate::OpState;
use crate::SourceMapData;
use crate::error::CoreError;
use crate::error::CoreErrorKind;
use crate::extensions::Extension;
use crate::extensions::ExtensionSourceType;
use crate::extensions::GlobalObjectMiddlewareFn;
use crate::extensions::GlobalTemplateMiddlewareFn;
use crate::extensions::OpMiddlewareFn;
use crate::modules::ModuleName;
use crate::ops::OpCtx;
use crate::runtime::ExtensionTranspiler;
use crate::runtime::JsRuntimeState;
use crate::runtime::OpDriverImpl;
use std::cell::RefCell;
use std::iter::Chain;
use std::rc::Rc;

/// Contribute to the `OpState` from each extension.
pub fn setup_op_state(
  op_state: &mut OpState,
  extensions: &mut [Extension],
) -> Vec<&'static str> {
  let mut lazy_extensions = Vec::with_capacity(extensions.len());
  for ext in extensions {
    if ext.needs_lazy_init {
      lazy_extensions.push(ext.name);
    }
    ext.take_state(op_state);
  }
  lazy_extensions
}

// TODO(bartlomieju): `deno_core_ext` ops should be returned as a separate
// vector - they need to be special cased and attached to `Deno.core.ops`,
// but not added to "ext:core/ops" virtual module.
/// Collects ops from extensions & applies middleware
pub fn init_ops(
  deno_core_ops: &'static [OpDecl],
  extensions: &mut [Extension],
) -> (Vec<OpDecl>, Vec<OpMethodDecl>) {
  // In debug build verify there that inter-Extension dependencies
  // are setup correctly.
  #[cfg(debug_assertions)]
  check_extensions_dependencies(extensions);

  let no_of_ops = extensions
    .iter()
    .map(|e| e.op_count())
    .fold(0, |ext_ops_count, count| count + ext_ops_count);
  let mut ops = Vec::with_capacity(no_of_ops + deno_core_ops.len());

  let no_of_methods = extensions
    .iter()
    .map(|e| e.method_op_count())
    .fold(0, |ext_ops_count, count| count + ext_ops_count);
  let mut op_methods = Vec::with_capacity(no_of_methods);

  // Collect all middlewares - deno_core extension must not have a middleware!
  let middlewares: Vec<Box<OpMiddlewareFn>> = extensions
    .iter_mut()
    .filter_map(|e| e.take_middleware())
    .collect();

  // Create a single macroware out of all middleware functions.
  let macroware = move |d| middlewares.iter().fold(d, |d, m| m(d));

  // Collect ops from all extensions and apply a macroware to each of them.
  for core_op in deno_core_ops {
    ops.push(OpDecl {
      name: core_op.name,
      name_fast: core_op.name_fast,
      ..macroware(*core_op)
    });
  }

  for ext in extensions.iter_mut() {
    let ext_ops = ext.init_ops();
    for ext_op in ext_ops {
      ops.push(OpDecl {
        name: ext_op.name,
        name_fast: ext_op.name_fast,
        ..macroware(*ext_op)
      });
    }

    let ext_method_ops = ext.init_method_ops();
    for ext_op in ext_method_ops {
      op_methods.push(*ext_op);
    }
  }

  // In debug build verify there are no duplicate ops.
  #[cfg(debug_assertions)]
  check_no_duplicate_op_names(&ops);

  (ops, op_methods)
}

/// This functions panics if any of the extensions is missing its dependencies.
#[cfg(debug_assertions)]
fn check_extensions_dependencies(exts: &[Extension]) {
  for (index, ext) in exts.iter().enumerate() {
    let previous_exts = &exts[..index];
    ext.check_dependencies(previous_exts);
  }
}

/// This function panics if there are ops with duplicate names
#[cfg(debug_assertions)]
fn check_no_duplicate_op_names(ops: &[OpDecl]) {
  use std::collections::HashMap;

  let mut count_by_name = HashMap::new();

  for op in ops.iter() {
    count_by_name.entry(op.name).or_insert(vec![]).push(op.name);
  }

  let mut duplicate_ops = vec![];
  for (op_name, _count) in count_by_name.iter().filter(|(_k, v)| v.len() > 1) {
    duplicate_ops.push(op_name.to_string());
  }
  if !duplicate_ops.is_empty() {
    let mut msg = "Found ops with duplicate names:\n".to_string();
    for op_name in duplicate_ops {
      msg.push_str(&format!("  - {}\n", op_name));
    }
    msg.push_str("Op names need to be unique.");
    panic!("{}", msg);
  }
}

#[allow(clippy::too_many_arguments)]
pub fn create_op_ctxs(
  op_decls: Vec<OpDecl>,
  op_method_decls: &mut [OpMethodDecl],
  op_metrics_factory_fn: Option<OpMetricsFactoryFn>,
  op_driver: Rc<OpDriverImpl>,
  op_state: Rc<RefCell<OpState>>,
  runtime_state: Rc<JsRuntimeState>,
  enable_stack_trace_in_ops: bool,
) -> (Box<[OpCtx]>, usize) {
  let op_count = op_decls.len() + op_method_decls.len();
  let mut op_ctxs = Vec::with_capacity(op_count);

  let runtime_state_ptr = runtime_state.as_ref() as *const _;
  let create_ctx = |index, decl| {
    let metrics_fn = op_metrics_factory_fn
      .as_ref()
      .and_then(|f| (f)(index as _, op_count, &decl));

    OpCtx::new(
      index as _,
      v8::UnsafeRawIsolatePtr::null(),
      op_driver.clone(),
      decl,
      op_state.clone(),
      runtime_state_ptr,
      metrics_fn,
      enable_stack_trace_in_ops,
    )
  };

  for (index, decl) in op_method_decls.iter_mut().enumerate() {
    if let Some(mut constructor) = decl.constructor {
      constructor.name = decl.name.0;
      constructor.name_fast = decl.name.1;

      op_ctxs.push(create_ctx(index, constructor));
    }

    for method in decl.methods {
      op_ctxs.push(create_ctx(index, *method));
    }
    for method in decl.static_methods {
      op_ctxs.push(create_ctx(index, *method));
    }
  }

  /* method op ctxs are stored before regular op ctxs */
  let methods_ctx_offset = op_ctxs.len();

  for (index, decl) in op_decls.into_iter().enumerate() {
    op_ctxs.push(create_ctx(index + methods_ctx_offset, decl));
  }

  (op_ctxs.into_boxed_slice(), methods_ctx_offset)
}

pub fn get_middlewares_and_external_refs(
  extensions: &mut [Extension],
) -> (
  Vec<GlobalTemplateMiddlewareFn>,
  Vec<GlobalObjectMiddlewareFn>,
  Vec<v8::ExternalReference>,
) {
  // TODO(bartlomieju): these numbers were chosen arbitrarily. This is a very
  // niche features and it's unlikely a lot of extensions use it.
  let mut global_template_middlewares = Vec::with_capacity(16);
  let mut global_object_middlewares = Vec::with_capacity(16);
  let mut additional_references = Vec::with_capacity(16);

  for extension in extensions {
    if let Some(middleware) = extension.get_global_template_middleware() {
      global_template_middlewares.push(middleware);
    }
    if let Some(middleware) = extension.get_global_object_middleware() {
      global_object_middlewares.push(middleware);
    }
    additional_references
      .extend_from_slice(extension.get_external_references());
  }

  (
    global_template_middlewares,
    global_object_middlewares,
    additional_references,
  )
}

#[derive(Debug)]
pub struct LoadedSource {
  pub source_type: ExtensionSourceType,
  pub specifier: ModuleName,
  pub code: ModuleCodeString,
  pub maybe_source_map: Option<SourceMapData>,
}

#[derive(Debug, Default)]
pub struct LoadedSources {
  pub js: Vec<LoadedSource>,
  pub esm: Vec<LoadedSource>,
  pub lazy_esm: Vec<LoadedSource>,
  pub esm_entry_points: Vec<FastString>,
}

impl LoadedSources {
  pub fn len(&self) -> usize {
    self.js.len() + self.esm.len() + self.lazy_esm.len()
  }

  pub fn is_empty(&self) -> bool {
    self.js.is_empty() && self.esm.is_empty() && self.lazy_esm.is_empty()
  }
}

type VecIntoIter<'a> = <&'a Vec<LoadedSource> as IntoIterator>::IntoIter;
type VecIntoIterMut<'a> = <&'a mut Vec<LoadedSource> as IntoIterator>::IntoIter;

impl<'a> IntoIterator for &'a LoadedSources {
  type Item = &'a LoadedSource;
  type IntoIter =
    Chain<Chain<VecIntoIter<'a>, VecIntoIter<'a>>, VecIntoIter<'a>>;
  fn into_iter(self) -> Self::IntoIter {
    self
      .js
      .iter()
      .chain(self.esm.iter())
      .chain(self.lazy_esm.iter())
  }
}

impl<'a> IntoIterator for &'a mut LoadedSources {
  type Item = &'a mut LoadedSource;
  type IntoIter =
    Chain<Chain<VecIntoIterMut<'a>, VecIntoIterMut<'a>>, VecIntoIterMut<'a>>;
  fn into_iter(self) -> Self::IntoIter {
    self
      .js
      .iter_mut()
      .chain(self.esm.iter_mut())
      .chain(self.lazy_esm.iter_mut())
  }
}

fn load(
  transpiler: Option<&ExtensionTranspiler>,
  source: &ExtensionFileSource,
  load_callback: &mut impl FnMut(&ExtensionFileSource),
) -> Result<(ModuleCodeString, Option<SourceMapData>), CoreError> {
  load_callback(source);
  let mut source_code = source.load()?;
  let mut source_map = None;
  if let Some(transpiler) = transpiler {
    (source_code, source_map) =
      transpiler(ModuleName::from_static(source.specifier), source_code)
        .map_err(CoreErrorKind::ExtensionTranspiler)?;
  }
  let mut maybe_source_map = None;
  if let Some(source_map) = source_map {
    maybe_source_map = Some(source_map);
  }
  Ok((source_code, maybe_source_map))
}

pub fn into_sources_and_source_maps(
  transpiler: Option<&ExtensionTranspiler>,
  extensions: &[Extension],
  extensions_in_snapshot: Option<&[&'static str]>,
  mut load_callback: impl FnMut(&ExtensionFileSource),
) -> Result<LoadedSources, CoreError> {
  let mut sources = LoadedSources::default();

  let extensions_in_snapshot = extensions_in_snapshot
    .unwrap_or_default()
    .iter()
    .map(Some)
    .chain(std::iter::repeat(None));

  for (extension, extension_in_snapshot) in
    extensions.iter().zip(extensions_in_snapshot)
  {
    for file in &*extension.lazy_loaded_esm_files {
      let (code, maybe_source_map) =
        load(transpiler, file, &mut load_callback)?;
      sources.lazy_esm.push(LoadedSource {
        source_type: ExtensionSourceType::LazyEsm,
        specifier: ModuleName::from_static(file.specifier),
        code,
        maybe_source_map,
      });
    }

    if let Some(name) = extension_in_snapshot {
      if extension.name != *name {
        return Err(
          CoreErrorKind::ExtensionSnapshotMismatch(
            crate::error::ExtensionSnapshotMismatchError {
              expected: name,
              actual: extension.name,
            },
          )
          .into_box(),
        );
      }
      continue;
    }

    if let Some(esm_entry_point) = extension.esm_entry_point {
      sources
        .esm_entry_points
        .push(FastString::from_static(esm_entry_point));
    }
    for file in &*extension.js_files {
      let (code, maybe_source_map) =
        load(transpiler, file, &mut load_callback)?;
      sources.js.push(LoadedSource {
        source_type: ExtensionSourceType::Js,
        specifier: ModuleName::from_static(file.specifier),
        code,
        maybe_source_map,
      });
    }
    for file in &*extension.esm_files {
      let (code, maybe_source_map) =
        load(transpiler, file, &mut load_callback)?;
      sources.esm.push(LoadedSource {
        source_type: ExtensionSourceType::Esm,
        specifier: ModuleName::from_static(file.specifier),
        code,
        maybe_source_map,
      });
    }
  }
  Ok(sources)
}
