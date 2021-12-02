// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use crate::cache;
use crate::config_file::IgnoredCompilerOptions;
use crate::diagnostics::Diagnostics;
use crate::emit;
use crate::errors::get_error_class_name;
use crate::flags;
use crate::proc_state::ProcState;
use crate::resolver::ImportMapResolver;
use crate::resolver::JsxResolver;

use deno_core::anyhow::Context;
use deno_core::error::custom_error;
use deno_core::error::generic_error;
use deno_core::error::AnyError;
use deno_core::resolve_url_or_path;
use deno_core::serde_json;
use deno_core::serde_json::Value;
use deno_core::ModuleSpecifier;
use deno_core::OpState;
use deno_graph;
use deno_runtime::permissions::Permissions;
use import_map::ImportMap;
use serde::Deserialize;
use serde::Serialize;
use std::cell::RefCell;
use std::collections::HashMap;
use std::collections::HashSet;
use std::rc::Rc;
use std::sync::Arc;

pub fn init(rt: &mut deno_core::JsRuntime) {
  super::reg_async(rt, "op_emit", op_emit);
}

#[derive(Debug, Deserialize)]
enum RuntimeBundleType {
  #[serde(rename = "module")]
  Module,
  #[serde(rename = "classic")]
  Classic,
}

impl<'a> From<&'a RuntimeBundleType> for emit::BundleType {
  fn from(bundle_type: &'a RuntimeBundleType) -> Self {
    match bundle_type {
      RuntimeBundleType::Classic => Self::Classic,
      RuntimeBundleType::Module => Self::Module,
    }
  }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EmitArgs {
  bundle: Option<RuntimeBundleType>,
  check: Option<bool>,
  compiler_options: Option<HashMap<String, Value>>,
  import_map: Option<Value>,
  import_map_path: Option<String>,
  root_specifier: String,
  sources: Option<HashMap<String, Arc<String>>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct EmitResult {
  diagnostics: Diagnostics,
  files: HashMap<String, String>,
  #[serde(rename = "ignoredOptions")]
  maybe_ignored_options: Option<IgnoredCompilerOptions>,
  stats: emit::Stats,
}

/// Provides inferred imported modules from configuration options, like the
/// `"types"` and `"jsxImportSource"` imports.
fn to_maybe_imports(
  referrer: &ModuleSpecifier,
  maybe_options: Option<&HashMap<String, Value>>,
) -> Option<Vec<(ModuleSpecifier, Vec<String>)>> {
  let options = maybe_options?;
  let mut imports = Vec::new();
  if let Some(types_value) = options.get("types") {
    if let Ok(types) =
      serde_json::from_value::<Vec<String>>(types_value.clone())
    {
      imports.extend(types);
    }
  }
  if let Some(jsx_value) = options.get("jsx") {
    if let Ok(jsx) = serde_json::from_value::<String>(jsx_value.clone()) {
      let jsx_import_source =
        if let Some(jsx_import_source_value) = options.get("jsxImportSource") {
          if let Ok(jsx_import_source) =
            serde_json::from_value::<String>(jsx_import_source_value.clone())
          {
            jsx_import_source
          } else {
            "react".to_string()
          }
        } else {
          "react".to_string()
        };
      match jsx.as_str() {
        "react-jsx" => {
          imports.push(format!("{}/jsx-runtime", jsx_import_source));
        }
        "react-jsxdev" => {
          imports.push(format!("{}/jsx-dev-runtime", jsx_import_source));
        }
        _ => (),
      }
    }
  }
  if !imports.is_empty() {
    Some(vec![(referrer.clone(), imports)])
  } else {
    None
  }
}

/// Converts the compiler options to the JSX import source module that will be
/// loaded when transpiling JSX.
fn to_maybe_jsx_import_source_module(
  maybe_options: Option<&HashMap<String, Value>>,
) -> Option<String> {
  let options = maybe_options?;
  let jsx_value = options.get("jsx")?;
  let jsx: String = serde_json::from_value(jsx_value.clone()).ok()?;
  match jsx.as_str() {
    "react-jsx" => Some("jsx-runtime".to_string()),
    "react-jsxdev" => Some("jsx-dev-runtime".to_string()),
    _ => None,
  }
}

async fn op_emit(
  state: Rc<RefCell<OpState>>,
  args: EmitArgs,
  _: (),
) -> Result<EmitResult, AnyError> {
  deno_runtime::ops::check_unstable2(&state, "Deno.emit");
  let root_specifier = args.root_specifier;
  let ps = {
    let state = state.borrow();
    state.borrow::<ProcState>().clone()
  };
  let mut runtime_permissions = {
    let state = state.borrow();
    state.borrow::<Permissions>().clone()
  };

  let mut cache: Box<dyn cache::CacherLoader> =
    if let Some(sources) = &args.sources {
      Box::new(cache::MemoryCacher::new(sources.clone()))
    } else {
      Box::new(cache::FetchCacher::new(
        ps.dir.gen_cache.clone(),
        ps.file_fetcher.clone(),
        runtime_permissions.clone(),
        runtime_permissions.clone(),
      ))
    };
  let maybe_import_map_resolver = if let Some(import_map_str) =
    args.import_map_path
  {
    let import_map_specifier = resolve_url_or_path(&import_map_str)
      .context(format!("Bad URL (\"{}\") for import map.", import_map_str))?;
    let import_map = if let Some(value) = args.import_map {
      ImportMap::from_json(import_map_specifier.as_str(), &value.to_string())?
    } else {
      let file = ps
        .file_fetcher
        .fetch(&import_map_specifier, &mut runtime_permissions)
        .await
        .map_err(|e| {
          generic_error(format!(
            "Unable to load '{}' import map: {}",
            import_map_specifier, e
          ))
        })?;
      ImportMap::from_json(import_map_specifier.as_str(), &file.source)?
    };
    Some(ImportMapResolver::new(Arc::new(import_map)))
  } else if args.import_map.is_some() {
    return Err(generic_error("An importMap was specified, but no importMapPath was provided, which is required."));
  } else {
    None
  };
  let maybe_jsx_resolver =
    to_maybe_jsx_import_source_module(args.compiler_options.as_ref())
      .map(|im| JsxResolver::new(im, maybe_import_map_resolver.clone()));
  let maybe_resolver = if maybe_jsx_resolver.is_some() {
    maybe_jsx_resolver.as_ref().map(|jr| jr.as_resolver())
  } else {
    maybe_import_map_resolver
      .as_ref()
      .map(|imr| imr.as_resolver())
  };
  let roots = vec![resolve_url_or_path(&root_specifier)?];
  let maybe_imports =
    to_maybe_imports(&roots[0], args.compiler_options.as_ref());
  let graph = Arc::new(
    deno_graph::create_graph(
      roots,
      true,
      maybe_imports,
      cache.as_mut_loader(),
      maybe_resolver,
      None,
      None,
    )
    .await,
  );
  // There are certain graph errors that we want to return as an error of an op,
  // versus something that gets returned as a diagnostic of the op, this is
  // handled here.
  if let Err(err) = graph.valid() {
    let err: AnyError = err.into();
    if get_error_class_name(&err) == "PermissionDenied" {
      return Err(err);
    }
  }
  let check = args.check.unwrap_or(true);
  let debug = ps.flags.log_level == Some(log::Level::Debug);
  let tsc_emit = check && args.bundle.is_none();
  let (ts_config, maybe_ignored_options) = emit::get_ts_config(
    emit::ConfigType::RuntimeEmit { tsc_emit },
    None,
    args.compiler_options.as_ref(),
  )?;
  let (files, mut diagnostics, stats) = if check && args.bundle.is_none() {
    let (diagnostics, stats) = if args.sources.is_none()
      && emit::valid_emit(
        graph.as_ref(),
        cache.as_cacher(),
        &ts_config,
        ps.flags.reload,
        &HashSet::default(),
      ) {
      log::debug!(
        "cache is valid for \"{}\", skipping check/emit",
        root_specifier
      );
      (Diagnostics::default(), emit::Stats::default())
    } else {
      let emit_result = emit::check_and_maybe_emit(
        graph.clone(),
        cache.as_mut_cacher(),
        emit::CheckOptions {
          check: flags::CheckFlag::All,
          debug,
          emit_with_diagnostics: true,
          maybe_config_specifier: None,
          ts_config,
          reload: true,
        },
      )?;
      (emit_result.diagnostics, emit_result.stats)
    };
    let files = emit::to_file_map(graph.as_ref(), cache.as_mut_cacher());
    (files, diagnostics, stats)
  } else if let Some(bundle) = &args.bundle {
    let (diagnostics, stats) = if check {
      if ts_config.get_declaration() {
        return Err(custom_error("TypeError", "The bundle option is set, but the compiler option of `declaration` is true which is not currently supported."));
      }
      let emit_result = emit::check_and_maybe_emit(
        graph.clone(),
        cache.as_mut_cacher(),
        emit::CheckOptions {
          check: flags::CheckFlag::All,
          debug,
          emit_with_diagnostics: true,
          maybe_config_specifier: None,
          ts_config: ts_config.clone(),
          reload: true,
        },
      )?;
      (emit_result.diagnostics, emit_result.stats)
    } else {
      (Diagnostics::default(), Default::default())
    };
    let (emit, maybe_map) = emit::bundle(
      graph.as_ref(),
      emit::BundleOptions {
        bundle_type: bundle.into(),
        ts_config,
      },
    )?;
    let mut files = HashMap::new();
    files.insert("deno:///bundle.js".to_string(), emit);
    if let Some(map) = maybe_map {
      files.insert("deno:///bundle.js.map".to_string(), map);
    }
    (files, diagnostics, stats)
  } else {
    if ts_config.get_declaration() {
      return Err(custom_error("TypeError", "The option of `check` is false, but the compiler option of `declaration` is true which is not currently supported."));
    }
    let emit_result = emit::emit(
      graph.as_ref(),
      cache.as_mut_cacher(),
      emit::EmitOptions {
        reload: ps.flags.reload,
        ts_config,
        reload_exclusions: HashSet::default(),
      },
    )?;
    let files = emit::to_file_map(graph.as_ref(), cache.as_mut_cacher());
    (files, emit_result.diagnostics, emit_result.stats)
  };

  // we want to add any errors that were returned as an `Err` earlier by adding
  // them to the diagnostics.
  diagnostics.extend_graph_errors(graph.errors());

  Ok(EmitResult {
    diagnostics,
    files,
    maybe_ignored_options,
    stats,
  })
}
