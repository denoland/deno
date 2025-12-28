// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use deno_core::FastString;
use deno_core::JsRuntime;
use deno_core::ModuleSpecifier;
use deno_core::OpState;
use deno_core::RuntimeOptions;
use deno_core::anyhow::Context;
use deno_core::located_script_name;
use deno_core::op2;
use deno_core::serde::Deserialize;
use deno_core::serde::Serialize;
use deno_core::serde_json::json;
use deno_core::url::Url;
use deno_graph::GraphKind;
use deno_graph::ModuleGraph;
use deno_lib::util::hash::FastInsecureHasher;
use deno_lib::worker::create_isolate_create_params;
use deno_path_util::resolve_url_or_path;
use deno_resolver::deno_json::JsxImportSourceConfigResolver;
use node_resolver::ResolutionMode;

use super::ResolveArgs;
use super::ResolveError;
use crate::args::TypeCheckMode;
use crate::tsc::Diagnostics;
use crate::tsc::ExecError;
use crate::tsc::LoadError;
use crate::tsc::Request;
use crate::tsc::RequestNpmState;
use crate::tsc::Response;
use crate::tsc::Stats;
use crate::tsc::get_hash;

#[op2]
#[string]
fn op_remap_specifier(
  state: &mut OpState,
  #[string] specifier: &str,
) -> Option<String> {
  let state = state.borrow::<State>();
  state
    .maybe_remapped_specifier(specifier)
    .map(|url| url.to_string())
}

#[op2]
#[serde]
fn op_libs() -> Vec<String> {
  crate::tsc::lib_names()
}

#[op2]
#[serde]
fn op_resolve(
  state: &mut OpState,
  #[string] base: &str,
  #[serde] specifiers: Vec<(bool, String)>,
) -> Result<Vec<(String, Option<&'static str>)>, ResolveError> {
  op_resolve_inner(state, ResolveArgs { base, specifiers })
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TscConstants {
  types_node_ignorable_names: Vec<&'static str>,
  node_only_globals: Vec<&'static str>,
  ignored_diagnostic_codes: Vec<u64>,
}

impl TscConstants {
  pub fn new() -> Self {
    Self {
      types_node_ignorable_names: super::TYPES_NODE_IGNORABLE_NAMES.to_vec(),
      node_only_globals: super::NODE_ONLY_GLOBALS.to_vec(),
      ignored_diagnostic_codes: super::IGNORED_DIAGNOSTIC_CODES
        .iter()
        .copied()
        .collect(),
    }
  }
}

#[op2]
#[serde]
fn op_tsc_constants() -> TscConstants {
  TscConstants::new()
}

#[inline]
fn op_resolve_inner(
  state: &mut OpState,
  args: ResolveArgs<'_>,
) -> Result<Vec<(String, Option<&'static str>)>, ResolveError> {
  let state = state.borrow_mut::<State>();
  let mut resolved: Vec<(String, Option<&'static str>)> =
    Vec::with_capacity(args.specifiers.len());
  let referrer = if let Some(remapped_specifier) =
    state.maybe_remapped_specifier(args.base)
  {
    remapped_specifier.clone()
  } else {
    resolve_url_or_path(args.base, &state.current_dir)?
  };
  let referrer_module = state.graph.get(&referrer);
  for (is_cjs, specifier) in args.specifiers {
    let result = super::resolve_specifier_for_tsc(
      specifier,
      &referrer,
      &state.graph,
      if is_cjs {
        ResolutionMode::Require
      } else {
        ResolutionMode::Import
      },
      state.maybe_npm.as_ref(),
      referrer_module,
      &mut state.remapped_specifiers,
    )?;
    resolved.push(result);
  }

  Ok(resolved)
}

#[op2]
#[string]
fn op_resolve_jsx_import_source(
  state: &mut OpState,
  #[string] referrer: &str,
) -> Option<String> {
  let state = state.borrow::<State>();
  let referrer = if let Some(remapped_specifier) =
    state.maybe_remapped_specifier(referrer)
  {
    Cow::Borrowed(remapped_specifier)
  } else {
    Cow::Owned(resolve_url_or_path(referrer, &state.current_dir).ok()?)
  };
  state
    .jsx_import_source_config_resolver
    .for_specifier(&referrer)?
    .specifier()
    .map(|s| s.to_string())
}

deno_core::extension!(deno_cli_tsc,
  ops = [
    op_create_hash,
    op_emit,
    op_is_node_file,
    op_load,
    op_remap_specifier,
    op_resolve,
    op_resolve_jsx_import_source,
    op_tsc_constants,
    op_respond,
    op_libs,
  ],
  options = {
    request: Request,
    root_map: HashMap<String, Url>,
    remapped_specifiers: HashMap<String, Url>,
  },
  state = |state, options| {
    state.put(State::new(
      options.request.graph,
      options.request.jsx_import_source_config_resolver,
      options.request.hash_data,
      options.request.maybe_npm,
      options.request.maybe_tsbuildinfo,
      options.root_map,
      options.remapped_specifiers,
      std::env::current_dir()
        .context("Unable to get CWD")
        .unwrap(),
    ));
  },
  customizer = |ext: &mut deno_core::Extension| {
    use deno_core::ExtensionFileSource;
    ext.esm_files.to_mut().push(ExtensionFileSource::new_computed("ext:deno_cli_tsc/99_main_compiler.js", crate::tsc::MAIN_COMPILER_SOURCE.as_str().into()));
    ext.esm_files.to_mut().push(ExtensionFileSource::new_computed("ext:deno_cli_tsc/97_ts_host.js", crate::tsc::TS_HOST_SOURCE.as_str().into()));
    ext.esm_files.to_mut().push(ExtensionFileSource::new_computed("ext:deno_cli_tsc/98_lsp.js", crate::tsc::LSP_SOURCE.as_str().into()));
    ext.js_files.to_mut().push(ExtensionFileSource::new_computed("ext:deno_cli_tsc/00_typescript.js", crate::tsc::TYPESCRIPT_SOURCE.as_str().into()));
    ext.esm_entry_point = Some("ext:deno_cli_tsc/99_main_compiler.js");
  }
);
// TODO(bartlomieju): this mechanism is questionable.
// Can't we use something more efficient here?
#[op2]
fn op_respond(state: &mut OpState, #[serde] args: RespondArgs) {
  op_respond_inner(state, args)
}

#[inline]
fn op_respond_inner(state: &mut OpState, args: RespondArgs) {
  let state = state.borrow_mut::<State>();
  state.maybe_response = Some(args);
}

#[op2]
#[string]
fn op_create_hash(s: &mut OpState, #[string] text: &str) -> String {
  op_create_hash_inner(s, text)
}

#[inline]
fn op_create_hash_inner(s: &mut OpState, text: &str) -> String {
  let state = s.borrow_mut::<State>();
  get_hash(text, state.hash_data)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EmitArgs {
  /// The text data/contents of the file.
  data: String,
  /// The _internal_ filename for the file.  This will be used to determine how
  /// the file is cached and stored.
  file_name: String,
}

#[op2(fast)]
fn op_emit(
  state: &mut OpState,
  #[string] data: String,
  #[string] file_name: String,
) -> bool {
  op_emit_inner(state, EmitArgs { data, file_name })
}

#[inline]
fn op_emit_inner(state: &mut OpState, args: EmitArgs) -> bool {
  let state = state.borrow_mut::<State>();
  match args.file_name.as_ref() {
    "internal:///.tsbuildinfo" => state.maybe_tsbuildinfo = Some(args.data),
    _ => {
      if cfg!(debug_assertions) {
        panic!("Unhandled emit write: {}", args.file_name);
      }
    }
  }

  true
}

#[op2(fast)]
fn op_is_node_file(state: &mut OpState, #[string] path: &str) -> bool {
  let state = state.borrow::<State>();
  ModuleSpecifier::parse(path)
    .ok()
    .map(|specifier| {
      state
        .maybe_npm
        .as_ref()
        .map(|n| n.node_resolver.in_npm_package(&specifier))
        .unwrap_or(false)
        || specifier.as_str().starts_with("asset:///node/")
    })
    .unwrap_or(false)
}

#[derive(Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
struct RespondArgs {
  pub diagnostics: Diagnostics,
  pub ambient_modules: Vec<String>,
  pub stats: Stats,
}

impl super::LoadContent for FastString {
  fn from_static(source: &'static str) -> Self {
    FastString::from_static(source)
  }
  fn from_string(source: String) -> Self {
    FastString::from(source)
  }
  fn from_arc_str(source: Arc<str>) -> Self {
    FastString::from(source)
  }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct LoadResponse {
  data: FastString,
  version: Option<String>,
  script_kind: i32,
  is_cjs: bool,
}

#[op2]
#[serde]
fn op_load(
  state: &mut OpState,
  #[string] load_specifier: &str,
) -> Result<Option<LoadResponse>, LoadError> {
  op_load_inner(state, load_specifier)
}

impl super::Mapper for State {
  fn maybe_remapped_specifier(
    &self,
    specifier: &str,
  ) -> Option<&ModuleSpecifier> {
    self.maybe_remapped_specifier(specifier)
  }
}

fn op_load_inner(
  state: &mut OpState,
  load_specifier: &str,
) -> Result<Option<LoadResponse>, LoadError> {
  let state = state.borrow::<State>();
  Ok(
    super::load_for_tsc::<FastString, _>(
      load_specifier,
      state.maybe_npm.as_ref(),
      &state.current_dir,
      &state.graph,
      state.maybe_tsbuildinfo.as_deref(),
      state.hash_data,
      state,
    )?
    .map(|res| LoadResponse {
      data: res.data,
      version: res.version,
      is_cjs: res.is_cjs,
      script_kind: super::as_ts_script_kind(res.media_type),
    }),
  )
}

pub fn exec_request(
  request: Request,
  root_names: Vec<String>,
  root_map: HashMap<String, ModuleSpecifier>,
  remapped_specifiers: HashMap<String, ModuleSpecifier>,
  code_cache: Option<Arc<dyn deno_runtime::code_cache::CodeCache>>,
) -> Result<Response, ExecError> {
  let request_value = json!({
    "config": request.config,
    "debug": request.debug,
    "rootNames": root_names,
    "localOnly": request.check_mode == TypeCheckMode::Local,
  });
  let exec_source = format!("globalThis.exec({request_value})");

  let mut extensions =
    deno_runtime::snapshot_info::get_extensions_in_snapshot();
  extensions.push(deno_cli_tsc::init(request, root_map, remapped_specifiers));
  let extension_code_cache = code_cache.map(|cache| {
    Rc::new(TscExtCodeCache::new(cache)) as Rc<dyn deno_core::ExtCodeCache>
  });
  let mut runtime = JsRuntime::new(RuntimeOptions {
    extensions,
    create_params: create_isolate_create_params(&crate::sys::CliSys::default()),
    startup_snapshot: deno_snapshots::CLI_SNAPSHOT,
    extension_code_cache,
    ..Default::default()
  });

  runtime
    .execute_script(located_script_name!(), exec_source)
    .map_err(ExecError::Js)?;

  let op_state = runtime.op_state();
  let mut op_state = op_state.borrow_mut();
  let state = op_state.take::<State>();

  if let Some(response) = state.maybe_response {
    let diagnostics = response.diagnostics;
    let ambient_modules = response.ambient_modules;
    let maybe_tsbuildinfo = state.maybe_tsbuildinfo;
    let stats = response.stats;

    Ok(Response {
      diagnostics,
      ambient_modules,
      maybe_tsbuildinfo,
      stats,
    })
  } else {
    Err(ExecError::ResponseNotSet)
  }
}

pub struct TscExtCodeCache {
  cache: Arc<dyn deno_runtime::code_cache::CodeCache>,
}

impl TscExtCodeCache {
  pub fn new(cache: Arc<dyn deno_runtime::code_cache::CodeCache>) -> Self {
    Self { cache }
  }
}

impl deno_core::ExtCodeCache for TscExtCodeCache {
  fn get_code_cache_info(
    &self,
    specifier: &ModuleSpecifier,
    code: &deno_core::ModuleSourceCode,
    esm: bool,
  ) -> deno_core::SourceCodeCacheInfo {
    use deno_runtime::code_cache::CodeCacheType;
    let code_hash = FastInsecureHasher::new_deno_versioned()
      .write_hashable(code)
      .finish();
    let data = self
      .cache
      .get_sync(
        specifier,
        if esm {
          CodeCacheType::EsModule
        } else {
          CodeCacheType::Script
        },
        code_hash,
      )
      .map(Cow::from)
      .inspect(|_| {
        log::debug!(
          "V8 code cache hit for Extension module: {specifier}, [{code_hash:?}]"
        );
      });
    deno_core::SourceCodeCacheInfo {
      hash: code_hash,
      data,
    }
  }

  fn code_cache_ready(
    &self,
    specifier: ModuleSpecifier,
    source_hash: u64,
    code_cache: &[u8],
    esm: bool,
  ) {
    use deno_runtime::code_cache::CodeCacheType;

    log::debug!(
      "Updating V8 code cache for Extension module: {specifier}, [{source_hash:?}]"
    );
    self.cache.set_sync(
      specifier,
      if esm {
        CodeCacheType::EsModule
      } else {
        CodeCacheType::Script
      },
      source_hash,
      code_cache,
    );
  }
}

// TODO(bartlomieju): we have similar struct in `tsc.rs` - maybe at least change
// the name of the struct to avoid confusion?
#[derive(Debug)]
struct State {
  hash_data: u64,
  graph: Arc<ModuleGraph>,
  jsx_import_source_config_resolver: Arc<JsxImportSourceConfigResolver>,
  maybe_tsbuildinfo: Option<String>,
  maybe_response: Option<RespondArgs>,
  maybe_npm: Option<RequestNpmState>,
  // todo(dsherret): it looks like the remapped_specifiers and
  // root_map could be combined... what is the point of the separation?
  remapped_specifiers: HashMap<String, ModuleSpecifier>,
  root_map: HashMap<String, ModuleSpecifier>,
  current_dir: PathBuf,
}

impl Default for State {
  fn default() -> Self {
    Self {
      hash_data: Default::default(),
      graph: Arc::new(ModuleGraph::new(GraphKind::All)),
      jsx_import_source_config_resolver: Default::default(),
      maybe_tsbuildinfo: Default::default(),
      maybe_response: Default::default(),
      maybe_npm: Default::default(),
      remapped_specifiers: Default::default(),
      root_map: Default::default(),
      current_dir: Default::default(),
    }
  }
}

impl State {
  #[allow(clippy::too_many_arguments)]
  pub fn new(
    graph: Arc<ModuleGraph>,
    jsx_import_source_config_resolver: Arc<JsxImportSourceConfigResolver>,
    hash_data: u64,
    maybe_npm: Option<RequestNpmState>,
    maybe_tsbuildinfo: Option<String>,
    root_map: HashMap<String, ModuleSpecifier>,
    remapped_specifiers: HashMap<String, ModuleSpecifier>,
    current_dir: PathBuf,
  ) -> Self {
    State {
      hash_data,
      graph,
      jsx_import_source_config_resolver,
      maybe_npm,
      maybe_tsbuildinfo,
      maybe_response: None,
      remapped_specifiers,
      root_map,
      current_dir,
    }
  }

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

#[cfg(test)]
mod tests {
  use deno_ast::MediaType;
  use deno_core::OpState;
  use deno_core::futures::future;
  use deno_core::parking_lot::Mutex;
  use deno_core::serde_json;
  use deno_error::JsErrorBox;
  use deno_graph::GraphKind;
  use deno_graph::ModuleGraph;
  use deno_runtime::code_cache::CodeCacheType;
  use test_util::PathRef;

  use super::super::Diagnostic;
  use super::super::DiagnosticCategory;
  use super::*;
  use crate::args::CompilerOptions;
  use crate::tsc::MISSING_DEPENDENCY_SPECIFIER;
  use crate::tsc::get_lazily_loaded_asset;

  #[derive(Debug, Default)]
  pub struct MockLoader {
    pub fixtures: PathRef,
  }

  impl deno_graph::source::Loader for MockLoader {
    fn load(
      &self,
      specifier: &ModuleSpecifier,
      _options: deno_graph::source::LoadOptions,
    ) -> deno_graph::source::LoadFuture {
      let specifier_text = specifier
        .to_string()
        .replace(":///", "_")
        .replace("://", "_")
        .replace('/', "-");
      let source_path = self.fixtures.join(specifier_text);
      let response = source_path
        .read_to_bytes_if_exists()
        .map(|c| {
          Some(deno_graph::source::LoadResponse::Module {
            specifier: specifier.clone(),
            mtime: None,
            maybe_headers: None,
            content: c.into(),
          })
        })
        .map_err(|e| {
          deno_graph::source::LoadError::Other(Arc::new(JsErrorBox::generic(
            e.to_string(),
          )))
        });
      Box::pin(future::ready(response))
    }
  }

  async fn setup(
    maybe_specifier: Option<ModuleSpecifier>,
    maybe_hash_data: Option<u64>,
    maybe_tsbuildinfo: Option<String>,
  ) -> OpState {
    let specifier = maybe_specifier
      .unwrap_or_else(|| ModuleSpecifier::parse("file:///main.ts").unwrap());
    let hash_data = maybe_hash_data.unwrap_or(0);
    let fixtures = test_util::testdata_path().join("tsc2");
    let loader = MockLoader { fixtures };
    let mut graph = ModuleGraph::new(GraphKind::TypesOnly);
    graph
      .build(vec![specifier], Vec::new(), &loader, Default::default())
      .await;
    let state = State::new(
      Arc::new(graph),
      Default::default(),
      hash_data,
      None,
      maybe_tsbuildinfo,
      HashMap::new(),
      HashMap::new(),
      std::env::current_dir()
        .context("Unable to get CWD")
        .unwrap(),
    );
    let mut op_state = OpState::new(None);
    op_state.put(state);
    op_state
  }

  async fn test_exec(
    specifier: &ModuleSpecifier,
  ) -> Result<Response, ExecError> {
    test_exec_with_cache(specifier, None).await
  }

  async fn test_exec_with_cache(
    specifier: &ModuleSpecifier,
    code_cache: Option<Arc<dyn deno_runtime::code_cache::CodeCache>>,
  ) -> Result<Response, ExecError> {
    let hash_data = 123; // something random
    let fixtures = test_util::testdata_path().join("tsc2");
    let loader = MockLoader { fixtures };
    let mut graph = ModuleGraph::new(GraphKind::TypesOnly);
    graph
      .build(
        vec![specifier.clone()],
        Vec::new(),
        &loader,
        Default::default(),
      )
      .await;
    let config = Arc::new(CompilerOptions::new(json!({
      "allowJs": true,
      "checkJs": false,
      "esModuleInterop": true,
      "emitDecoratorMetadata": false,
      "incremental": true,
      "jsx": "react",
      "jsxFactory": "React.createElement",
      "jsxFragmentFactory": "React.Fragment",
      "lib": ["deno.window"],
      "noEmit": true,
      "outDir": "internal:///",
      "strict": true,
      "target": "esnext",
      "tsBuildInfoFile": "internal:///.tsbuildinfo",
    })));
    let request = Request {
      config,
      debug: false,
      graph: Arc::new(graph),
      jsx_import_source_config_resolver: Default::default(),
      hash_data,
      maybe_npm: None,
      maybe_tsbuildinfo: None,
      root_names: vec![(specifier.clone(), MediaType::TypeScript)],
      check_mode: TypeCheckMode::All,
      initial_cwd: std::env::current_dir().unwrap(),
    };
    crate::tsc::exec(request, code_cache, None)
  }

  #[tokio::test]
  async fn test_create_hash() {
    let mut state = setup(None, Some(123), None).await;
    let actual = op_create_hash_inner(&mut state, "some sort of content");
    assert_eq!(actual, "11905938177474799758");
  }

  #[tokio::test]
  async fn test_hash_url() {
    let specifier = deno_core::resolve_url(
      "data:application/javascript,console.log(\"Hello%20Deno\");",
    )
    .unwrap();
    assert_eq!(
      crate::tsc::hash_url(&specifier, MediaType::JavaScript),
      "data:///d300ea0796bd72b08df10348e0b70514c021f2e45bfe59cec24e12e97cd79c58.js"
    );
  }

  #[tokio::test]
  async fn test_emit_tsbuildinfo() {
    let mut state = setup(None, None, None).await;
    let actual = op_emit_inner(
      &mut state,
      EmitArgs {
        data: "some file content".to_string(),
        file_name: "internal:///.tsbuildinfo".to_string(),
      },
    );
    assert!(actual);
    let state = state.borrow::<State>();
    assert_eq!(
      state.maybe_tsbuildinfo,
      Some("some file content".to_string())
    );
  }

  #[tokio::test]
  async fn test_load() {
    let mut state = setup(
      Some(ModuleSpecifier::parse("https://deno.land/x/mod.ts").unwrap()),
      None,
      Some("some content".to_string()),
    )
    .await;
    let actual =
      op_load_inner(&mut state, "https://deno.land/x/mod.ts").unwrap();
    assert_eq!(
      serde_json::to_value(actual).unwrap(),
      json!({
        "data": "console.log(\"hello deno\");\n",
        "version": "7821807483407828376",
        "scriptKind": 3,
        "isCjs": false,
      })
    );
  }

  #[tokio::test]
  async fn test_load_asset() {
    let mut state = setup(
      Some(ModuleSpecifier::parse("https://deno.land/x/mod.ts").unwrap()),
      None,
      Some("some content".to_string()),
    )
    .await;
    let actual = op_load_inner(&mut state, "asset:///lib.dom.d.ts")
      .expect("should have invoked op")
      .expect("load should have succeeded");
    let expected = get_lazily_loaded_asset("lib.dom.d.ts").unwrap();
    assert_eq!(actual.data.to_string(), expected.to_string());
    assert!(actual.version.is_some());
    assert_eq!(actual.script_kind, 3);
  }

  #[tokio::test]
  async fn test_load_tsbuildinfo() {
    let mut state = setup(
      Some(ModuleSpecifier::parse("https://deno.land/x/mod.ts").unwrap()),
      None,
      Some("some content".to_string()),
    )
    .await;
    let actual = op_load_inner(&mut state, "internal:///.tsbuildinfo")
      .expect("should have invoked op")
      .expect("load should have succeeded");
    assert_eq!(
      serde_json::to_value(actual).unwrap(),
      json!({
        "data": "some content",
        "version": null,
        "scriptKind": 0,
        "isCjs": false,
      })
    );
  }

  #[tokio::test]
  async fn test_load_missing_specifier() {
    let mut state = setup(None, None, None).await;
    let actual = op_load_inner(&mut state, "https://deno.land/x/mod.ts")
      .expect("should have invoked op");
    assert_eq!(serde_json::to_value(actual).unwrap(), json!(null));
  }

  #[tokio::test]
  async fn test_resolve() {
    let mut state = setup(
      Some(ModuleSpecifier::parse("https://deno.land/x/a.ts").unwrap()),
      None,
      None,
    )
    .await;
    let actual = op_resolve_inner(
      &mut state,
      ResolveArgs {
        base: "https://deno.land/x/a.ts",
        specifiers: vec![(false, "./b.ts".to_string())],
      },
    )
    .expect("should have invoked op");
    assert_eq!(
      actual,
      vec![("https://deno.land/x/b.ts".into(), Some(".ts"))]
    );
  }

  #[tokio::test]
  async fn test_resolve_empty() {
    let mut state = setup(
      Some(ModuleSpecifier::parse("https://deno.land/x/a.ts").unwrap()),
      None,
      None,
    )
    .await;
    let actual = op_resolve_inner(
      &mut state,
      ResolveArgs {
        base: "https://deno.land/x/a.ts",
        specifiers: vec![(false, "./bad.ts".to_string())],
      },
    )
    .expect("should have not errored");
    assert_eq!(
      actual,
      vec![(MISSING_DEPENDENCY_SPECIFIER.into(), Some(".d.ts"))]
    );
  }

  #[tokio::test]
  async fn test_respond() {
    let mut state = setup(None, None, None).await;
    let args = serde_json::from_value(json!({
      "diagnostics": [
        {
          "messageText": "Unknown compiler option 'invalid'.",
          "category": 1,
          "code": 5023
        }
      ],
      "stats": [["a", 12]],
      "ambientModules": []
    }))
    .unwrap();
    op_respond_inner(&mut state, args);
    let state = state.borrow::<State>();
    assert_eq!(
      state.maybe_response,
      Some(RespondArgs {
        diagnostics: Diagnostics::new(vec![Diagnostic {
          category: DiagnosticCategory::Error,
          code: 5023,
          start: None,
          end: None,
          original_source_start: None,
          message_text: Some(
            "Unknown compiler option \'invalid\'.".to_string()
          ),
          message_chain: None,
          source: None,
          source_line: None,
          file_name: None,
          related_information: None,
          reports_deprecated: None,
          reports_unnecessary: None,
          other: Default::default(),
          missing_specifier: None,
        }]),
        ambient_modules: vec![],
        stats: Stats(vec![("a".to_string(), 12)])
      })
    );
  }

  #[tokio::test]
  async fn test_exec_basic() {
    let specifier = ModuleSpecifier::parse("https://deno.land/x/a.ts").unwrap();
    let actual = test_exec(&specifier)
      .await
      .expect("exec should not have errored");
    assert!(!actual.diagnostics.has_diagnostic());
    assert!(actual.maybe_tsbuildinfo.is_some());
    assert_eq!(actual.stats.0.len(), 12);
  }

  #[tokio::test]
  async fn test_exec_reexport_dts() {
    let specifier = ModuleSpecifier::parse("file:///reexports.ts").unwrap();
    let actual = test_exec(&specifier)
      .await
      .expect("exec should not have errored");
    assert!(!actual.diagnostics.has_diagnostic());
    assert!(actual.maybe_tsbuildinfo.is_some());
    assert_eq!(actual.stats.0.len(), 12);
  }

  #[tokio::test]
  async fn fix_lib_ref() {
    let specifier = ModuleSpecifier::parse("file:///libref.ts").unwrap();
    let actual = test_exec(&specifier)
      .await
      .expect("exec should not have errored");
    assert!(!actual.diagnostics.has_diagnostic());
  }

  pub type SpecifierWithType = (ModuleSpecifier, CodeCacheType);

  #[derive(Default)]
  struct TestExtCodeCache {
    cache: Mutex<HashMap<(SpecifierWithType, u64), Vec<u8>>>,

    hits: Mutex<HashMap<SpecifierWithType, usize>>,
    misses: Mutex<HashMap<SpecifierWithType, usize>>,
  }

  impl deno_runtime::code_cache::CodeCache for TestExtCodeCache {
    fn get_sync(
      &self,
      specifier: &ModuleSpecifier,
      code_cache_type: CodeCacheType,
      source_hash: u64,
    ) -> Option<Vec<u8>> {
      let result = self
        .cache
        .lock()
        .get(&((specifier.clone(), code_cache_type), source_hash))
        .cloned();
      if result.is_some() {
        *self
          .hits
          .lock()
          .entry((specifier.clone(), code_cache_type))
          .or_default() += 1;
      } else {
        *self
          .misses
          .lock()
          .entry((specifier.clone(), code_cache_type))
          .or_default() += 1;
      }
      result
    }

    fn set_sync(
      &self,
      specifier: ModuleSpecifier,
      code_cache_type: CodeCacheType,
      source_hash: u64,
      data: &[u8],
    ) {
      self
        .cache
        .lock()
        .insert(((specifier, code_cache_type), source_hash), data.to_vec());
    }
  }

  #[tokio::test]
  async fn test_exec_code_cache() {
    let code_cache = Arc::new(TestExtCodeCache::default());
    let specifier = ModuleSpecifier::parse("https://deno.land/x/a.ts").unwrap();
    let actual = test_exec_with_cache(&specifier, Some(code_cache.clone()))
      .await
      .expect("exec should not have errored");
    assert!(!actual.diagnostics.has_diagnostic());

    let expect = [
      (
        "ext:deno_cli_tsc/99_main_compiler.js",
        CodeCacheType::EsModule,
      ),
      ("ext:deno_cli_tsc/98_lsp.js", CodeCacheType::EsModule),
      ("ext:deno_cli_tsc/97_ts_host.js", CodeCacheType::EsModule),
      ("ext:deno_cli_tsc/00_typescript.js", CodeCacheType::Script),
    ];

    {
      let mut files = HashMap::new();

      for (((specifier, ty), _), _) in code_cache.cache.lock().iter() {
        let specifier = specifier.to_string();
        if files.contains_key(&specifier) {
          panic!("should have only 1 entry per specifier");
        }
        files.insert(specifier, *ty);
      }

      // 99_main_compiler, 98_lsp, 97_ts_host, 00_typescript
      assert_eq!(files.len(), 4);
      assert_eq!(code_cache.hits.lock().len(), 0);
      assert_eq!(code_cache.misses.lock().len(), 4);

      for (specifier, ty) in &expect {
        assert_eq!(files.get(*specifier), Some(ty));
      }

      code_cache.hits.lock().clear();
      code_cache.misses.lock().clear();
    }

    {
      let _ = test_exec_with_cache(&specifier, Some(code_cache.clone()))
        .await
        .expect("exec should not have errored");

      // 99_main_compiler, 98_lsp, 97_ts_host, 00_typescript
      assert_eq!(code_cache.hits.lock().len(), 4);
      assert_eq!(code_cache.misses.lock().len(), 0);

      for (specifier, ty) in expect {
        let url = ModuleSpecifier::parse(specifier).unwrap();
        assert_eq!(code_cache.hits.lock().get(&(url, ty)), Some(&1));
      }
    }
  }
}
