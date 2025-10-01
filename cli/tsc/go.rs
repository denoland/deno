use crate::tsc::RequestNpmState;
use crate::tsc::ResolveNonGraphSpecifierTypesError;
use crate::tsc::get_lazily_loaded_asset;

use super::Request;
use super::Response;
use deno_ast::MediaType;
use deno_ast::ModuleSpecifier;
use deno_config::deno_json::CompilerOptions;
use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_graph::Module;
use deno_graph::ModuleGraph;
use deno_resolver::npm::ResolvePkgFolderFromDenoReqError;
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use typescript_go_client_rust::CallbackHandler;
use typescript_go_client_rust::SyncRpcChannel;
use typescript_go_client_rust::types::GetImpliedNodeFormatForFilePayload;
use typescript_go_client_rust::types::Project;
use typescript_go_client_rust::types::ResolveModuleNamePayload;
use typescript_go_client_rust::types::ResolveTypeReferenceDirectivePayload;

macro_rules! jsons {
  ($($arg:tt)*) => {
    serde_json::to_string(&json!($($arg)*))
  };
}

fn deser<T: serde::de::DeserializeOwned>(
  payload: impl AsRef<str>,
) -> Result<T, serde_json::Error> {
  serde_json::from_str::<T>(payload.as_ref())
}

fn synthetic_config(
  config: &CompilerOptions,
  root_names: &[String],
) -> Result<String, serde_json::Error> {
  let mut config = serde_json::to_value(config)?;
  config
    .as_object_mut()
    .unwrap()
    .insert("allowImportingTsExtensions".to_string(), json!(true));
  // config.as_object_mut().unwrap().remove("lib");
  // config
  //   .as_object_mut()
  //   .unwrap()
  //   .insert("lib".to_string(), json!(["dom", "dom.iterable", "esnext"]));
  // let mut root_names =
  //   root_names.iter().map(|s| s.to_string()).collect::<Vec<_>>();
  log::debug!("config: {}", serde_json::to_string(&config)?);
  // root_names.push("asset:///lib.deno.window.d.ts".to_string());
  serde_json::to_string(&json!({

    "compilerOptions": config,
    "files": root_names,
  }))
}

pub fn exec_request(
  request: Request,
  root_names: Vec<String>,
  root_map: HashMap<String, ModuleSpecifier>,
  remapped_specifiers: HashMap<String, ModuleSpecifier>,
) -> Result<Response, super::ExecError> {
  exec_request_inner(request, root_names, root_map, remapped_specifiers)
    .map_err(super::ExecError::Go)
}

fn exec_request_inner(
  request: Request,
  root_names: Vec<String>,
  root_map: HashMap<String, ModuleSpecifier>,
  remapped_specifiers: HashMap<String, ModuleSpecifier>,
) -> Result<Response, ExecError> {
  let handler = Handler::new(
    "/virtual/tsconfig.json".to_string(),
    synthetic_config(request.config.as_ref(), &root_names)?,
    remapped_specifiers,
    root_map,
    std::env::current_dir().unwrap(),
    request.graph.clone(),
    request.maybe_npm,
  );

  let callbacks = handler.supported_callbacks();
  let mut channel = SyncRpcChannel::new(
    "/Users/nathanwhit/Documents/Code/typescript-go/built/local/tsgo",
    vec!["--api"],
    handler,
  )?;

  channel.request_sync(
    "configure",
    jsons!({
      "callbacks": callbacks.into_iter().collect::<Vec<_>>(),
      "logFile": ""
    })?,
  )?;

  let project = channel.request_sync(
    "loadProject",
    jsons!({
      "configFileName": "/virtual/tsconfig.json",
    })?,
  )?;
  let project = deser::<Project>(project)?;

  let diagnostics = channel.request_sync(
    "getDiagnostics",
    jsons!({
      "project": &project.id,
    })?,
  )?;
  // eprintln!("diagnostics: {}", diagnostics);
  let diagnostics =
    deser::<Vec<typescript_go_client_rust::types::Diagnostic>>(diagnostics)?;

  Ok(Response {
    diagnostics: convert_diagnostics(diagnostics),
    maybe_tsbuildinfo: None,
    ambient_modules: vec![],
    stats: super::Stats::default(),
  })
}

fn convert_diagnostic(
  diagnostic: typescript_go_client_rust::types::Diagnostic,
  _diagnostics: &[typescript_go_client_rust::types::Diagnostic],
) -> super::Diagnostic {
  let (start, end) = if diagnostic.start.line == 0
    && diagnostic.start.character == 0
    && diagnostic.end.line == 0
    && diagnostic.end.character == 0
  {
    (None, None)
  } else {
    (Some(diagnostic.start), Some(diagnostic.end))
  };

  super::Diagnostic {
    category: match diagnostic.category.as_str() {
      "error" => super::DiagnosticCategory::Error,
      "warning" => super::DiagnosticCategory::Warning,
      "message" => super::DiagnosticCategory::Message,
      "suggestion" => super::DiagnosticCategory::Suggestion,
      _ => unreachable!(),
    },
    code: diagnostic.code as u64,
    start: start.map(|s| super::Position {
      line: s.line,
      character: s.character,
    }),
    end: end.map(|e| super::Position {
      line: e.line,
      character: e.character,
    }),
    original_source_start: None,
    message_text: Some(diagnostic.message),
    message_chain: None,
    file_name: Some(diagnostic.file_name),
    missing_specifier: None,
    other: Default::default(),
    related_information: None,
    reports_deprecated: Some(diagnostic.reports_deprecated),
    reports_unnecessary: Some(diagnostic.reports_unnecessary),
    source: None,
    source_line: Some(diagnostic.source_line),
  }
}

fn convert_diagnostics(
  diagnostics: Vec<typescript_go_client_rust::types::Diagnostic>,
) -> super::Diagnostics {
  super::diagnostics::Diagnostics::from(
    diagnostics
      .iter()
      .map(|diagnostic| convert_diagnostic(diagnostic.clone(), &diagnostics))
      .collect::<Vec<_>>(),
  )
}

struct Handler {
  state: RefCell<HandlerState>,
}

impl Handler {
  fn new(
    config_path: String,
    synthetic_config: String,
    remapped_specifiers: HashMap<String, ModuleSpecifier>,
    root_map: HashMap<String, ModuleSpecifier>,
    current_dir: PathBuf,
    graph: Arc<ModuleGraph>,
    maybe_npm: Option<super::RequestNpmState>,
  ) -> Self {
    if maybe_npm.is_none() {
      panic!("no npm state");
    }
    Self {
      state: RefCell::new(HandlerState {
        config_path,
        synthetic_config,
        remapped_specifiers,
        root_map,
        current_dir,
        graph,
        maybe_npm,
        module_kind_map: HashMap::new(),
      }),
    }
  }
}

fn get_package_json_scope_if_applicable(
  state: &mut HandlerState,
  payload: String,
) -> Result<String, typescript_go_client_rust::Error> {
  if let Some(maybe_npm) = state.maybe_npm.as_ref() {
    let file_path = deser::<String>(&payload)?;
    let file_path = if let Ok(specifier) = ModuleSpecifier::parse(&file_path) {
      deno_path_util::url_to_file_path(&specifier).ok()
    } else {
      Some(PathBuf::from(file_path))
    };
    let Some(file_path) = file_path else {
      return Ok(jsons!(None::<String>)?);
    };
    if let Some(package_json) = maybe_npm
      .package_json_resolver
      .get_closest_package_json_path(&file_path)
    {
      let package_directory = package_json.parent();
      let contents = std::fs::read_to_string(&package_json).ok();
      if let Some(contents) = contents {
        return Ok(jsons!({
          "packageDirectory": package_directory,
          "directoryExists": true,
          "contents": contents,
        })?);
      }
    }
  }

  Ok(jsons!(None::<String>)?)
}

struct HandlerState {
  config_path: String,
  synthetic_config: String,
  remapped_specifiers: HashMap<String, ModuleSpecifier>,
  root_map: HashMap<String, ModuleSpecifier>,
  current_dir: PathBuf,
  graph: Arc<ModuleGraph>,
  maybe_npm: Option<super::RequestNpmState>,

  module_kind_map:
    HashMap<String, typescript_go_client_rust::types::ResolutionMode>,
}

impl typescript_go_client_rust::CallbackHandler for Handler {
  fn supported_callbacks(
    &self,
  ) -> std::collections::HashSet<std::string::String> {
    [
      "readFile",
      "resolveModuleName",
      "getPackageJsonScopeIfApplicable",
      "getPackageScopeForPath",
      "resolveTypeReferenceDirective",
      "getImpliedNodeFormatForFile",
      "isNodeSourceFile",
    ]
    .into_iter()
    .map(|s| s.to_owned())
    .collect()
  }

  fn handle_callback(
    &self,
    name: &str,
    payload: String,
  ) -> Result<String, typescript_go_client_rust::Error> {
    // eprintln!("handle_callback: {} : {}", name, payload);
    let mut state = self.state.borrow_mut();
    match name {
      "readFile" => {
        let payload = deser::<String>(payload)?;
        if payload == state.config_path {
          Ok(jsons!(&state.synthetic_config)?)
        } else {
          // eprintln!("readFile: {}", payload);
          // let payload =
          //   deno_path_util::resolve_url_or_path(&payload, &state.current_dir)
          //     .map_err(adhoc)?;
          // let path =
          //   deno_path_util::url_to_file_path(&payload).map_err(adhoc)?;
          // let contents = match std::fs::read_to_string(path) {
          //   Ok(contents) => contents,
          //   Err(e) => match e.kind() {
          //     std::io::ErrorKind::NotFound => {
          //       return Ok(jsons!(None::<String>)?);
          //     }
          //     _ => {
          //       return Err(typescript_go_client_rust::Error::AdHoc(
          //         e.to_string(),
          //       ));
          //     }
          //   },
          // };
          let contents = load_inner(&mut *state, &payload).map_err(adhoc)?;
          // eprintln!("result for {}: {:?}", payload, contents);
          if let Some(contents) = contents {
            Ok(jsons!(&contents)?)
          } else {
            let path = Path::new(&payload);
            if let Ok(contents) = std::fs::read_to_string(path) {
              // eprintln!("fallback result for {}: Some", payload,);
              Ok(jsons!(&contents)?)
            } else {
              // eprintln!("fallback result for {}: None", payload);
              Ok(jsons!(None::<String>)?)
            }
          }
        }
      }
      "resolveModuleName" => {
        let payload = deser::<ResolveModuleNamePayload>(payload)?;
        let (out_name, extension) = resolve_name(&mut *state, payload)?;

        Ok(jsons!({
          "resolvedFileName": out_name,
          "extension": extension,
        })?)
      }
      "getPackageJsonScopeIfApplicable" => {
        get_package_json_scope_if_applicable(&mut *state, payload)
      }
      "getPackageScopeForPath" => {
        get_package_json_scope_if_applicable(&mut *state, payload)
      }
      "resolveTypeReferenceDirective" => {
        log::debug!("resolveTypeReferenceDirective: {}", payload);
        let payload = deser::<ResolveTypeReferenceDirectivePayload>(payload)?;
        let payload = ResolveModuleNamePayload {
          module_name: payload.type_reference_directive_name,
          containing_file: payload.containing_file,
          resolution_mode: payload.resolution_mode,
        };
        let (out_name, extension) = resolve_name(&mut *state, payload)?;
        // let url = deno_core::ModuleSpecifier::parse(&out_name).unwrap();
        // let file_path = deno_path_util::url_to_file_path(&url).unwrap();
        // let out_name = file_path.to_string_lossy();
        log::debug!(
          "resolveTypeReferenceDirective: {:?}",
          (&out_name, &extension)
        );
        Ok(jsons!({
          "resolvedFileName": out_name,
          "primary": true,
        })?)
      }
      "getImpliedNodeFormatForFile" => {
        let payload = deser::<GetImpliedNodeFormatForFilePayload>(payload)?;
        let module_kind = state
          .module_kind_map
          .get(&payload.file_name)
          .unwrap_or(&typescript_go_client_rust::types::ResolutionMode::ESM);
        Ok(jsons!(&module_kind)?)
      }
      "isNodeSourceFile" => {
        let path = deser::<String>(payload)?;
        let state = &*state;
        let result = ModuleSpecifier::parse(&path)
          .ok()
          .or_else(|| {
            deno_path_util::resolve_url_or_path(&path, &state.current_dir).ok()
          })
          .and_then(|specifier| {
            state
              .maybe_npm
              .as_ref()
              .map(|n| n.node_resolver.in_npm_package(&specifier))
          })
          .unwrap_or(false);
        Ok(jsons!(result)?)
      }
      _ => unreachable!(),
    }
  }
}

fn adhoc(err: impl std::error::Error) -> typescript_go_client_rust::Error {
  typescript_go_client_rust::Error::AdHoc(err.to_string())
}

fn resolve_name(
  handler: &mut HandlerState,
  payload: ResolveModuleNamePayload,
) -> Result<(String, Option<&'static str>), typescript_go_client_rust::Error> {
  let graph = &handler.graph;
  let maybe_npm = handler.maybe_npm.as_ref();
  let referrer = if let Some(remapped_specifier) =
    handler.maybe_remapped_specifier(&payload.containing_file)
  {
    remapped_specifier.clone()
  } else {
    deno_path_util::resolve_url_or_path(
      &payload.containing_file,
      &handler.current_dir,
    )
    .map_err(adhoc)?
  };
  let referrer_module = graph.get(&referrer);
  let specifier = payload.module_name;
  if specifier.starts_with("node:") {
    return Ok((
      super::MISSING_DEPENDENCY_SPECIFIER.to_string(),
      Some(MediaType::Dts.as_ts_extension()),
    ));
  }

  if specifier.starts_with("asset:///") {
    let ext = MediaType::from_str(&specifier).as_ts_extension();
    return Ok((specifier, Some(ext)));
  }

  let resolved_dep = referrer_module
    .and_then(|m| match m {
      Module::Js(m) => m.dependencies_prefer_fast_check().get(&specifier),
      Module::Json(_) => None,
      Module::Wasm(m) => m.dependencies.get(&specifier),
      Module::Npm(_) | Module::Node(_) | Module::External(_) => None,
    })
    .and_then(|d| d.maybe_type.ok().or_else(|| d.maybe_code.ok()));
  let resolution_mode = if matches!(
    payload.resolution_mode,
    typescript_go_client_rust::types::ResolutionMode::CommonJS
  ) {
    super::ResolutionMode::Require
  } else {
    super::ResolutionMode::Import
  };

  let maybe_result = match resolved_dep {
    Some(deno_graph::ResolutionResolved { specifier, .. }) => {
      super::resolve_graph_specifier_types(
        specifier,
        &referrer,
        // we could get this from the resolved dep, but for now assume
        // the value resolved in TypeScript is better
        resolution_mode,
        &graph,
        maybe_npm,
      )
      .map_err(adhoc)?
    }
    _ => {
      match super::resolve_non_graph_specifier_types(
        &specifier,
        &referrer,
        resolution_mode,
        maybe_npm,
      ) {
        Ok(maybe_result) => maybe_result,
        Err(
          err
          @ ResolveNonGraphSpecifierTypesError::ResolvePkgFolderFromDenoReq(
            ResolvePkgFolderFromDenoReqError::Managed(_),
          ),
        ) => {
          // it's most likely requesting the jsxImportSource, which isn't loaded
          // into the graph when not using jsx, so just ignore this error
          if specifier.ends_with("/jsx-runtime") {
            None
          } else {
            return Err(adhoc(err));
          }
        }
        Err(err) => return Err(adhoc(err)),
      }
    }
  };
  let result = match maybe_result {
    Some((specifier, media_type)) => {
      let specifier_str = match specifier.scheme() {
        "data" | "blob" => {
          let specifier_str = super::hash_url(&specifier, media_type);
          handler
            .remapped_specifiers
            .insert(specifier_str.clone(), specifier);
          specifier_str
        }
        _ => {
          if let Some(specifier_str) =
            super::mapped_specifier_for_tsc(&specifier, media_type)
          {
            handler
              .remapped_specifiers
              .insert(specifier_str.clone(), specifier);
            specifier_str
          } else {
            specifier.to_string()
          }
        }
      };
      (
        specifier_str,
        match media_type {
          MediaType::Css => Some(".js"), // surface these as .js for typescript
          MediaType::Unknown => None,
          media_type => Some(media_type.as_ts_extension()),
        },
      )
    }
    None => (
      super::MISSING_DEPENDENCY_SPECIFIER.to_string(),
      Some(MediaType::Dts.as_ts_extension()),
    ),
  };
  log::debug!("Resolved {} from {} to {:?}", specifier, referrer, result);

  Ok(result)
}

impl HandlerState {
  pub fn maybe_remapped_specifier(
    &self,
    specifier: &str,
  ) -> Option<&ModuleSpecifier> {
    self
      .remapped_specifiers
      .get(specifier)
      .or_else(|| self.root_map.get(specifier))
  }
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum ExecError {
  #[class(generic)]
  #[error(transparent)]
  SerdeJson(#[from] serde_json::Error),
  #[class(generic)]
  #[error(transparent)]
  TsgoClient(#[from] typescript_go_client_rust::Error),

  #[class(generic)]
  #[error("failed to load from node module: {path}: {error}")]
  LoadFromNodeModule { path: String, error: std::io::Error },

  #[class(generic)]
  #[error(transparent)]
  PackageJsonLoad(#[from] deno_package_json::PackageJsonLoadError),

  #[class(generic)]
  #[error(transparent)]
  PackageJsonLoadError(#[from] node_resolver::errors::PackageJsonLoadError),
}

fn load_inner(
  state: &mut HandlerState,
  load_specifier: &str,
) -> Result<Option<String>, ExecError> {
  fn load_from_node_modules(
    specifier: &ModuleSpecifier,
    npm_state: Option<&RequestNpmState>,
    media_type: &mut MediaType,
    is_cjs: &mut bool,
  ) -> Result<Option<String>, ExecError> {
    *media_type = MediaType::from_specifier(specifier);
    let file_path = specifier.to_file_path().unwrap();
    let code = match std::fs::read_to_string(&file_path) {
      Ok(code) => code,
      Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
        return Ok(None);
      }
      Err(err) => {
        return Err(ExecError::LoadFromNodeModule {
          path: file_path.display().to_string(),
          error: err,
        });
      }
    };
    let code_arc = code.clone().into();
    *is_cjs = npm_state
      .map(|npm_state| {
        npm_state
          .cjs_tracker
          .is_cjs(specifier, *media_type, &code_arc)
      })
      .unwrap_or(false);
    Ok(Some(code.into()))
  }

  let specifier =
    deno_path_util::resolve_url_or_path(load_specifier, &state.current_dir)
      .map_err(adhoc)?;

  // let mut hash: Option<String> = None;
  let mut media_type = MediaType::Unknown;
  let graph = &state.graph;
  let mut is_cjs = false;

  let data = if load_specifier == "internal:///.tsbuildinfo" {
    // state
    //   .maybe_tsbuildinfo
    //   .as_deref()
    //   .map(|s| s.to_string().into())
    None
  // in certain situations we return a "blank" module to tsc and we need to
  // handle the request for that module here.
  } else if load_specifier == super::MISSING_DEPENDENCY_SPECIFIER {
    None
  } else if let Some(name) = load_specifier.strip_prefix("asset:///") {
    let maybe_source = get_lazily_loaded_asset(name);
    // hash = super::get_maybe_hash(maybe_source, 0);
    media_type = MediaType::from_str(load_specifier);
    maybe_source.map(String::from)
  } else if let Some(source) = super::load_raw_import_source(&specifier) {
    return Ok(Some(source.to_string()));
  } else {
    let specifier = if let Some(remapped_specifier) =
      state.maybe_remapped_specifier(load_specifier)
    {
      remapped_specifier
    } else {
      &specifier
    };
    let maybe_module = graph.try_get(specifier).ok().flatten();
    let maybe_source = if let Some(module) = maybe_module {
      match module {
        Module::Js(module) => {
          media_type = module.media_type;
          if let Some(npm_state) = &state.maybe_npm {
            is_cjs = npm_state.cjs_tracker.is_cjs_with_known_is_script(
              specifier,
              module.media_type,
              module.is_script,
            )?;
          }
          Some(
            module
              .fast_check_module()
              .map(|m| m.source.clone().to_string())
              .unwrap_or(module.source.text.clone().to_string()),
          )
        }
        Module::Json(module) => {
          media_type = MediaType::Json;
          Some(module.source.text.clone().to_string())
        }
        Module::Wasm(module) => {
          media_type = MediaType::Dts;
          Some(module.source_dts.clone().to_string())
        }
        Module::Npm(_) | Module::Node(_) => None,
        Module::External(module) => {
          if module.specifier.scheme() != "file" {
            None
          } else {
            // means it's Deno code importing an npm module
            let specifier = super::resolve_specifier_into_node_modules(
              &super::CliSys::default(),
              &module.specifier,
            );
            load_from_node_modules(
              &specifier,
              state.maybe_npm.as_ref(),
              &mut media_type,
              &mut is_cjs,
            )?
          }
        }
      }
    } else if let Some(npm) = state
      .maybe_npm
      .as_ref()
      .filter(|npm| npm.node_resolver.in_npm_package(specifier))
    {
      load_from_node_modules(
        specifier,
        Some(npm),
        &mut media_type,
        &mut is_cjs,
      )?
    } else {
      None
    };
    // hash = super::get_maybe_hash(maybe_source.as_deref(), state.hash_data);
    maybe_source
  };
  let module_kind = get_resolution_mode(is_cjs, media_type);
  state
    .module_kind_map
    .insert(load_specifier.to_string(), module_kind);
  let Some(data) = data else {
    return Ok(None);
  };
  Ok(Some(data.into()))
}

fn get_resolution_mode(
  is_cjs: bool,
  media_type: MediaType,
) -> typescript_go_client_rust::types::ResolutionMode {
  if is_cjs {
    typescript_go_client_rust::types::ResolutionMode::CommonJS
  } else {
    match media_type {
      MediaType::Cjs | MediaType::Dcts | MediaType::Cts => {
        typescript_go_client_rust::types::ResolutionMode::CommonJS
      }

      MediaType::Css
      | MediaType::Json
      | MediaType::Html
      | MediaType::Sql
      | MediaType::Wasm
      | MediaType::SourceMap
      | MediaType::Unknown => {
        typescript_go_client_rust::types::ResolutionMode::None
      }
      _ => typescript_go_client_rust::types::ResolutionMode::ESM,
    }
  }
}
