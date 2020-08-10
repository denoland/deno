// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use crate::colors;
use crate::diagnostics::Diagnostic;
use crate::diagnostics::DiagnosticItem;
use crate::disk_cache::DiskCache;
use crate::doc::Location;
use crate::file_fetcher::SourceFile;
use crate::file_fetcher::SourceFileFetcher;
use crate::flags::Flags;
use crate::fmt_errors::JSError;
use crate::global_state::GlobalState;
use crate::module_graph::ModuleGraph;
use crate::module_graph::ModuleGraphLoader;
use crate::msg;
use crate::msg::MediaType;
use crate::op_error::OpError;
use crate::ops;
use crate::permissions::Permissions;
use crate::source_maps::SourceMapGetter;
use crate::startup_data;
use crate::state::State;
use crate::swc_util::AstParser;
use crate::swc_util::SwcDiagnosticBuffer;
use crate::version;
use crate::worker::Worker;
use core::task::Context;
use deno_core::ErrBox;
use deno_core::ModuleSpecifier;
use deno_core::StartupData;
use futures::future::Future;
use futures::future::FutureExt;
use log::debug;
use log::info;
use log::Level;
use regex::Regex;
use serde::Deserialize;
use serde::Serialize;
use serde_json::json;
use serde_json::Value;
use sourcemap::SourceMap;
use std::collections::HashMap;
use std::collections::HashSet;
use std::fs;
use std::io;
use std::ops::Deref;
use std::ops::DerefMut;
use std::path::PathBuf;
use std::pin::Pin;
use std::str;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::sync::Mutex;
use std::task::Poll;
use swc_common::comments::CommentKind;
use swc_common::Span;
use swc_ecmascript::visit::Node;
use swc_ecmascript::visit::Visit;
use url::Url;

pub const AVAILABLE_LIBS: &[&str] = &[
  "deno.ns",
  "deno.window",
  "deno.worker",
  "deno.shared_globals",
  "deno.unstable",
  "dom",
  "dom.iterable",
  "es5",
  "es6",
  "esnext",
  "es2020",
  "es2020.full",
  "es2019",
  "es2019.full",
  "es2018",
  "es2018.full",
  "es2017",
  "es2017.full",
  "es2016",
  "es2016.full",
  "es2015",
  "es2015.collection",
  "es2015.core",
  "es2015.generator",
  "es2015.iterable",
  "es2015.promise",
  "es2015.proxy",
  "es2015.reflect",
  "es2015.symbol",
  "es2015.symbol.wellknown",
  "es2016.array.include",
  "es2017.intl",
  "es2017.object",
  "es2017.sharedmemory",
  "es2017.string",
  "es2017.typedarrays",
  "es2018.asyncgenerator",
  "es2018.asynciterable",
  "es2018.intl",
  "es2018.promise",
  "es2018.regexp",
  "es2019.array",
  "es2019.object",
  "es2019.string",
  "es2019.symbol",
  "es2020.bigint",
  "es2020.promise",
  "es2020.string",
  "es2020.symbol.wellknown",
  "esnext.array",
  "esnext.asynciterable",
  "esnext.bigint",
  "esnext.intl",
  "esnext.promise",
  "esnext.string",
  "esnext.symbol",
  "scripthost",
  "webworker",
  "webworker.importscripts",
];

#[derive(Debug, Clone)]
pub struct CompiledModule {
  pub code: String,
  pub name: String,
}

pub struct CompilerWorker {
  worker: Worker,
  response: Arc<Mutex<Option<String>>>,
}

impl CompilerWorker {
  pub fn new(name: String, startup_data: StartupData, state: State) -> Self {
    let state_ = state.clone();
    let mut worker = Worker::new(name, startup_data, state_);
    let response = Arc::new(Mutex::new(None));
    {
      let isolate = &mut worker.isolate;
      ops::runtime::init(isolate, &state);
      ops::errors::init(isolate, &state);
      ops::timers::init(isolate, &state);
      ops::compiler::init(isolate, &state, response.clone());
    }

    Self { worker, response }
  }

  pub fn get_response(&mut self) -> String {
    let mut maybe_response = self.response.lock().unwrap();
    assert!(
      maybe_response.is_some(),
      "Unexpected missing response from TS compiler"
    );
    maybe_response.take().unwrap()
  }
}

impl Deref for CompilerWorker {
  type Target = Worker;
  fn deref(&self) -> &Self::Target {
    &self.worker
  }
}

impl DerefMut for CompilerWorker {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.worker
  }
}

impl Future for CompilerWorker {
  type Output = Result<(), ErrBox>;

  fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
    let inner = self.get_mut();
    inner.worker.poll_unpin(cx)
  }
}

lazy_static! {
  // TODO(bartlomieju): use JSONC parser from dprint instead of Regex
  static ref CHECK_JS_RE: Regex =
    Regex::new(r#""checkJs"\s*?:\s*?true"#).unwrap();
  static ref DENO_TYPES_RE: Regex =
    Regex::new(r"^\s*@deno-types\s?=\s?(\S+)\s*(.*)\s*$").unwrap();
  // These regexes were adapted from TypeScript
  // https://github.com/microsoft/TypeScript/blob/87fd1827f2f2f3dafa76c14f13b9defc69481766/src/compiler/parser.ts#L8780-L8781
  static ref XML_COMMENT_START_RE: Regex =
    Regex::new(r"^/\s*<(\S+)\s.*?/>").unwrap();
  static ref PATH_REFERENCE_RE: Regex =
    Regex::new(r#"(\spath\s*=\s*)('|")(.+?)('|")"#).unwrap();
  static ref TYPES_REFERENCE_RE: Regex =
    Regex::new(r#"(\stypes\s*=\s*)('|")(.+?)('|")"#).unwrap();
  static ref LIB_REFERENCE_RE: Regex =
    Regex::new(r#"(\slib\s*=\s*)('|")(.+?)('|")"#).unwrap();
}

/// Create a new worker with snapshot of TS compiler and setup compiler's
/// runtime.
fn create_compiler_worker(
  global_state: GlobalState,
  permissions: Permissions,
) -> CompilerWorker {
  // TODO(bartlomieju): these $deno$ specifiers should be unified for all subcommands
  // like 'eval', 'repl'
  let entry_point =
    ModuleSpecifier::resolve_url_or_path("./__$deno$ts_compiler.ts").unwrap();
  let worker_state = State::new(
    global_state.clone(),
    Some(permissions),
    entry_point,
    None,
    true,
  )
  .expect("Unable to create worker state");

  // TODO(bartlomieju): this metric is never used anywhere
  // Count how many times we start the compiler worker.
  global_state.compiler_starts.fetch_add(1, Ordering::SeqCst);

  let mut worker = CompilerWorker::new(
    "TS".to_string(),
    startup_data::compiler_isolate_init(),
    worker_state,
  );
  worker
    .execute("globalThis.bootstrapCompilerRuntime()")
    .unwrap();
  worker
}

#[derive(Clone)]
pub enum TargetLib {
  Main,
  Worker,
}

/// Struct which represents the state of the compiler
/// configuration where the first is canonical name for the configuration file,
/// second is a vector of the bytes of the contents of the configuration file,
/// third is bytes of the hash of contents.
#[derive(Clone)]
pub struct CompilerConfig {
  pub path: Option<PathBuf>,
  pub content: Option<Vec<u8>>,
  pub hash: Vec<u8>,
  pub compile_js: bool,
}

impl CompilerConfig {
  /// Take the passed flag and resolve the file name relative to the cwd.
  pub fn load(config_path: Option<String>) -> Result<Self, ErrBox> {
    let config_file = match &config_path {
      Some(config_file_name) => {
        debug!("Compiler config file: {}", config_file_name);
        let cwd = std::env::current_dir().unwrap();
        Some(cwd.join(config_file_name))
      }
      _ => None,
    };

    // Convert the PathBuf to a canonicalized string.  This is needed by the
    // compiler to properly deal with the configuration.
    let config_path = match &config_file {
      Some(config_file) => Some(config_file.canonicalize().map_err(|_| {
        io::Error::new(
          io::ErrorKind::InvalidInput,
          format!(
            "Could not find the config file: {}",
            config_file.to_string_lossy()
          ),
        )
      })),
      _ => None,
    };

    // Load the contents of the configuration file
    let config = match &config_file {
      Some(config_file) => {
        debug!("Attempt to load config: {}", config_file.to_str().unwrap());
        let config = fs::read(&config_file)?;
        Some(config)
      }
      _ => None,
    };

    let config_hash = match &config {
      Some(bytes) => bytes.clone(),
      _ => b"".to_vec(),
    };

    // If `checkJs` is set to true in `compilerOptions` then we're gonna be compiling
    // JavaScript files as well
    let compile_js = if let Some(config_content) = config.clone() {
      let config_str = std::str::from_utf8(&config_content)?;
      CHECK_JS_RE.is_match(config_str)
    } else {
      false
    };

    let ts_config = Self {
      path: config_path.unwrap_or_else(|| Ok(PathBuf::new())).ok(),
      content: config,
      hash: config_hash,
      compile_js,
    };

    Ok(ts_config)
  }
}

/// Information associated with compiled file in cache.
/// version_hash is used to validate versions of the file
/// and could be used to remove stale file in cache.
#[derive(Deserialize, Serialize)]
pub struct CompiledFileMetadata {
  pub version_hash: String,
}

impl CompiledFileMetadata {
  pub fn from_json_string(
    metadata_string: String,
  ) -> Result<Self, serde_json::Error> {
    serde_json::from_str::<Self>(&metadata_string)
  }

  pub fn to_json_string(&self) -> Result<String, serde_json::Error> {
    serde_json::to_string(self)
  }
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
struct TranspileSourceFile {
  pub source_code: String,
  pub file_name: String,
}

/// Emit a SHA256 hash based on source code, deno version and TS config.
/// Used to check if a recompilation for source code is needed.
fn source_code_version_hash(
  source_code: &[u8],
  version: &str,
  config_hash: &[u8],
) -> String {
  crate::checksum::gen(&[source_code, version.as_bytes(), config_hash])
}

fn maybe_log_stats(maybe_stats: Option<Vec<Stat>>) {
  if let Some(stats) = maybe_stats {
    debug!("DEBUG - Compilation Statistics:");
    for stat in stats {
      debug!("{}: {}", stat.key, stat.value);
    }
  }
}

pub struct TsCompilerInner {
  pub file_fetcher: SourceFileFetcher,
  pub flags: Flags,
  pub config: CompilerConfig,
  pub disk_cache: DiskCache,
  /// Set of all URLs that have been compiled. This prevents double
  /// compilation of module.
  pub compiled: Mutex<HashSet<Url>>,
  /// This setting is controlled by `--reload` flag. Unless the flag
  /// is provided disk cache is used.
  pub use_disk_cache: bool,
  /// This setting is controlled by `compilerOptions.checkJs`
  pub compile_js: bool,
}

#[derive(Clone)]
pub struct TsCompiler(Arc<TsCompilerInner>);

impl Deref for TsCompiler {
  type Target = TsCompilerInner;
  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct Stat {
  key: String,
  value: f64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct EmittedSource {
  filename: String,
  contents: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BundleResponse {
  diagnostics: Diagnostic,
  bundle_output: Option<String>,
  stats: Option<Vec<Stat>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CompileResponse {
  diagnostics: Diagnostic,
  emit_map: HashMap<String, EmittedSource>,
  build_info: Option<String>,
  stats: Option<Vec<Stat>>,
}

// TODO(bartlomieju): possible deduplicate once TS refactor is stabilized
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(unused)]
struct RuntimeBundleResponse {
  diagnostics: Vec<DiagnosticItem>,
  output: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeCompileResponse {
  diagnostics: Vec<DiagnosticItem>,
  emit_map: HashMap<String, EmittedSource>,
}

impl TsCompiler {
  pub fn new(
    file_fetcher: SourceFileFetcher,
    flags: Flags,
    disk_cache: DiskCache,
  ) -> Result<Self, ErrBox> {
    let config = CompilerConfig::load(flags.config_path.clone())?;
    let use_disk_cache = !flags.reload;

    Ok(TsCompiler(Arc::new(TsCompilerInner {
      file_fetcher,
      flags,
      disk_cache,
      compile_js: config.compile_js,
      config,
      compiled: Mutex::new(HashSet::new()),
      use_disk_cache,
    })))
  }

  /// Mark given module URL as compiled to avoid multiple compilations of same
  /// module in single run.
  fn mark_compiled(&self, url: &Url) {
    let mut c = self.compiled.lock().unwrap();
    c.insert(url.clone());
  }

  fn has_compiled(&self, url: &Url) -> bool {
    let c = self.compiled.lock().unwrap();
    c.contains(url)
  }

  /// Check if there is compiled source in cache that is valid and can be used
  /// again.
  fn has_compiled_source(&self, url: &Url) -> bool {
    let specifier = ModuleSpecifier::from(url.clone());
    if let Some(source_file) = self
      .file_fetcher
      .fetch_cached_source_file(&specifier, Permissions::allow_all())
    {
      if let Some(metadata) = self.get_metadata(&url) {
        // Compare version hashes
        let version_hash_to_validate = source_code_version_hash(
          &source_file.source_code.as_bytes(),
          version::DENO,
          &self.config.hash,
        );

        if metadata.version_hash == version_hash_to_validate {
          return true;
        }
      }
    }

    false
  }

  fn has_valid_cache(
    &self,
    url: &Url,
    build_info: &Option<String>,
  ) -> Result<bool, ErrBox> {
    if let Some(build_info_str) = build_info.as_ref() {
      let build_inf_json: Value = serde_json::from_str(build_info_str)?;
      let program_val = build_inf_json["program"].as_object().unwrap();
      let file_infos = program_val["fileInfos"].as_object().unwrap();

      if !self.has_compiled_source(url) {
        return Ok(false);
      }

      for (filename, file_info) in file_infos.iter() {
        if filename.starts_with("asset://") {
          continue;
        }

        let url = Url::parse(&filename).expect("Filename is not a valid url");
        let specifier = ModuleSpecifier::from(url);

        if let Some(source_file) = self
          .file_fetcher
          .fetch_cached_source_file(&specifier, Permissions::allow_all())
        {
          let existing_hash = crate::checksum::gen(&[
            &source_file.source_code.as_bytes(),
            version::DENO.as_bytes(),
          ]);
          let expected_hash =
            file_info["version"].as_str().unwrap().to_string();
          if existing_hash != expected_hash {
            // hashes don't match, somethings changed
            return Ok(false);
          }
        } else {
          // no cached source file
          return Ok(false);
        }
      }
    } else {
      // no build info
      return Ok(false);
    }

    Ok(true)
  }

  /// Asynchronously compile module and all it's dependencies.
  ///
  /// This method compiled every module at most once.
  ///
  /// If `--reload` flag was provided then compiler will not on-disk cache and
  /// force recompilation.
  ///
  /// If compilation is required then new V8 worker is spawned with fresh TS
  /// compiler.
  pub async fn compile(
    &self,
    global_state: GlobalState,
    source_file: &SourceFile,
    target: TargetLib,
    permissions: Permissions,
    module_graph: ModuleGraph,
    allow_js: bool,
  ) -> Result<(), ErrBox> {
    let module_url = source_file.url.clone();
    let build_info_key = self
      .disk_cache
      .get_cache_filename_with_extension(&module_url, "buildinfo");
    let build_info = match self.disk_cache.get(&build_info_key) {
      Ok(bytes) => Some(String::from_utf8(bytes)?),
      Err(_) => None,
    };

    // Only use disk cache if `--reload` flag was not used or this file has
    // already been compiled during current process lifetime.
    if (self.use_disk_cache || self.has_compiled(&source_file.url))
      && self.has_valid_cache(&source_file.url, &build_info)?
    {
      return Ok(());
    }

    let module_graph_json =
      serde_json::to_value(module_graph).expect("Failed to serialize data");
    let target = match target {
      TargetLib::Main => "main",
      TargetLib::Worker => "worker",
    };
    let root_names = vec![module_url.to_string()];
    let unstable = self.flags.unstable;
    let performance = matches!(self.flags.log_level, Some(Level::Debug));
    let compiler_config = self.config.clone();
    let cwd = std::env::current_dir().unwrap();

    let j = match (compiler_config.path, compiler_config.content) {
      (Some(config_path), Some(config_data)) => json!({
        "type": msg::CompilerRequestType::Compile,
        "allowJs": allow_js,
        "target": target,
        "rootNames": root_names,
        "unstable": unstable,
        "performance": performance,
        "configPath": config_path,
        "config": str::from_utf8(&config_data).unwrap(),
        "cwd": cwd,
        "sourceFileMap": module_graph_json,
        "buildInfo": if self.use_disk_cache { build_info } else { None },
      }),
      _ => json!({
        "type": msg::CompilerRequestType::Compile,
        "allowJs": allow_js,
        "target": target,
        "rootNames": root_names,
        "unstable": unstable,
        "performance": performance,
        "cwd": cwd,
        "sourceFileMap": module_graph_json,
        "buildInfo": if self.use_disk_cache { build_info } else { None },
      }),
    };

    let req_msg = j.to_string();

    // TODO(bartlomieju): lift this call up - TSC shouldn't print anything
    info!("{} {}", colors::green("Check"), module_url.to_string());

    let json_str =
      execute_in_same_thread(global_state, permissions, req_msg).await?;

    let compile_response: CompileResponse = serde_json::from_str(&json_str)?;

    if !compile_response.diagnostics.items.is_empty() {
      return Err(ErrBox::from(compile_response.diagnostics));
    }

    maybe_log_stats(compile_response.stats);

    if let Some(build_info) = compile_response.build_info {
      self.cache_build_info(&module_url, build_info)?;
    }
    self.cache_emitted_files(compile_response.emit_map)?;
    Ok(())
  }

  /// For a given module, generate a single file JavaScript output that includes
  /// all the dependencies for that module.
  pub async fn bundle(
    &self,
    global_state: GlobalState,
    module_specifier: ModuleSpecifier,
  ) -> Result<String, ErrBox> {
    debug!(
      "Invoking the compiler to bundle. module_name: {}",
      module_specifier.to_string()
    );

    let permissions = Permissions::allow_all();
    let mut module_graph_loader = ModuleGraphLoader::new(
      self.file_fetcher.clone(),
      global_state.maybe_import_map.clone(),
      permissions.clone(),
      false,
      true,
    );
    module_graph_loader
      .add_to_graph(&module_specifier, None)
      .await?;
    let module_graph = module_graph_loader.get_graph();
    let module_graph_files = module_graph.values().collect::<Vec<_>>();
    // Check integrity of every file in module graph
    if let Some(ref lockfile) = global_state.lockfile {
      let mut g = lockfile.lock().unwrap();

      for graph_file in &module_graph_files {
        let check_passed =
          g.check_or_insert(&graph_file.url, &graph_file.source_code);

        if !check_passed {
          eprintln!(
            "Subresource integrity check failed --lock={}\n{}",
            g.filename, graph_file.url
          );
          std::process::exit(10);
        }
      }
    }
    if let Some(ref lockfile) = global_state.lockfile {
      let g = lockfile.lock().unwrap();
      g.write()?;
    }
    let module_graph_json =
      serde_json::to_value(module_graph).expect("Failed to serialize data");

    let root_names = vec![module_specifier.to_string()];
    let target = "main";
    let cwd = std::env::current_dir().unwrap();
    let performance =
      matches!(global_state.flags.log_level, Some(Level::Debug));

    let compiler_config = self.config.clone();

    // TODO(bartlomieju): this is non-sense; CompilerConfig's `path` and `content` should
    // be optional
    let j = match (compiler_config.path, compiler_config.content) {
      (Some(config_path), Some(config_data)) => json!({
        "type": msg::CompilerRequestType::Bundle,
        "target": target,
        "rootNames": root_names,
        "unstable": self.flags.unstable,
        "performance": performance,
        "configPath": config_path,
        "config": str::from_utf8(&config_data).unwrap(),
        "cwd": cwd,
        "sourceFileMap": module_graph_json,
      }),
      _ => json!({
        "type": msg::CompilerRequestType::Bundle,
        "target": target,
        "rootNames": root_names,
        "unstable": self.flags.unstable,
        "performance": performance,
        "cwd": cwd,
        "sourceFileMap": module_graph_json,
      }),
    };

    let req_msg = j.to_string();

    let json_str =
      execute_in_same_thread(global_state, permissions, req_msg).await?;

    let bundle_response: BundleResponse = serde_json::from_str(&json_str)?;

    maybe_log_stats(bundle_response.stats);

    if !bundle_response.diagnostics.items.is_empty() {
      return Err(ErrBox::from(bundle_response.diagnostics));
    }

    assert!(bundle_response.bundle_output.is_some());
    let output = bundle_response.bundle_output.unwrap();
    Ok(output)
  }

  pub async fn transpile(
    &self,
    module_graph: ModuleGraph,
  ) -> Result<(), ErrBox> {
    let mut source_files: Vec<TranspileSourceFile> = Vec::new();
    for (_, value) in module_graph.iter() {
      let url = Url::parse(&value.url).expect("Filename is not a valid url");
      if !value.url.ends_with(".d.ts")
        && (!self.use_disk_cache || !self.has_compiled_source(&url))
      {
        source_files.push(TranspileSourceFile {
          source_code: value.source_code.clone(),
          file_name: value.url.clone(),
        });
      }
    }
    if source_files.is_empty() {
      return Ok(());
    }

    let mut emit_map = HashMap::new();

    for source_file in source_files {
      let parser = AstParser::default();
      let stripped_source = parser.strip_types(
        &source_file.file_name,
        MediaType::TypeScript,
        &source_file.source_code,
      )?;

      // TODO(bartlomieju): this is superfluous, just to make caching function happy
      let emitted_filename = PathBuf::from(&source_file.file_name)
        .with_extension("js")
        .to_string_lossy()
        .to_string();
      let emitted_source = EmittedSource {
        filename: source_file.file_name.to_string(),
        contents: stripped_source,
      };

      emit_map.insert(emitted_filename, emitted_source);
    }

    self.cache_emitted_files(emit_map)?;
    Ok(())
  }

  /// Get associated `CompiledFileMetadata` for given module if it exists.
  fn get_metadata(&self, url: &Url) -> Option<CompiledFileMetadata> {
    // Try to load cached version:
    // 1. check if there's 'meta' file
    let cache_key = self
      .disk_cache
      .get_cache_filename_with_extension(url, "meta");
    if let Ok(metadata_bytes) = self.disk_cache.get(&cache_key) {
      if let Ok(metadata) = std::str::from_utf8(&metadata_bytes) {
        if let Ok(read_metadata) =
          CompiledFileMetadata::from_json_string(metadata.to_string())
        {
          return Some(read_metadata);
        }
      }
    }

    None
  }

  fn cache_build_info(
    &self,
    url: &Url,
    build_info: String,
  ) -> std::io::Result<()> {
    let js_key = self
      .disk_cache
      .get_cache_filename_with_extension(url, "buildinfo");
    self.disk_cache.set(&js_key, build_info.as_bytes())?;

    Ok(())
  }

  fn cache_emitted_files(
    &self,
    emit_map: HashMap<String, EmittedSource>,
  ) -> std::io::Result<()> {
    for (emitted_name, source) in emit_map.iter() {
      let specifier = ModuleSpecifier::resolve_url(&source.filename)
        .expect("Should be a valid module specifier");

      let source_file = self
        .file_fetcher
        .fetch_cached_source_file(&specifier, Permissions::allow_all())
        .expect("Source file not found");

      // NOTE: JavaScript files are only cached to disk if `checkJs`
      // option in on
      if source_file.media_type == msg::MediaType::JavaScript
        && !self.compile_js
      {
        continue;
      }

      if emitted_name.ends_with(".map") {
        self.cache_source_map(&specifier, &source.contents)?;
      } else if emitted_name.ends_with(".js") {
        self.cache_compiled_file(&specifier, source_file, &source.contents)?;
      } else {
        panic!("Trying to cache unknown file type {}", emitted_name);
      }
    }

    Ok(())
  }

  pub fn get_compiled_module(
    &self,
    module_url: &Url,
  ) -> Result<CompiledModule, ErrBox> {
    let compiled_source_file = self.get_compiled_source_file(module_url)?;

    let compiled_module = CompiledModule {
      code: compiled_source_file.source_code.to_string()?,
      name: module_url.to_string(),
    };

    Ok(compiled_module)
  }

  /// Return compiled JS file for given TS module.
  // TODO: ideally we shouldn't construct SourceFile by hand, but it should be
  // delegated to SourceFileFetcher.
  pub fn get_compiled_source_file(
    &self,
    module_url: &Url,
  ) -> Result<SourceFile, ErrBox> {
    let cache_key = self
      .disk_cache
      .get_cache_filename_with_extension(&module_url, "js");
    let compiled_code = self.disk_cache.get(&cache_key)?;
    let compiled_code_filename = self.disk_cache.location.join(cache_key);
    debug!("compiled filename: {:?}", compiled_code_filename);

    let compiled_module = SourceFile {
      url: module_url.clone(),
      filename: compiled_code_filename,
      media_type: msg::MediaType::JavaScript,
      source_code: compiled_code.into(),
      types_header: None,
    };

    Ok(compiled_module)
  }

  /// Save compiled JS file for given TS module to on-disk cache.
  ///
  /// Along compiled file a special metadata file is saved as well containing
  /// hash that can be validated to avoid unnecessary recompilation.
  fn cache_compiled_file(
    &self,
    module_specifier: &ModuleSpecifier,
    source_file: SourceFile,
    contents: &str,
  ) -> std::io::Result<()> {
    let js_key = self
      .disk_cache
      .get_cache_filename_with_extension(module_specifier.as_url(), "js");
    self.disk_cache.set(&js_key, contents.as_bytes())?;
    self.mark_compiled(module_specifier.as_url());

    let version_hash = source_code_version_hash(
      &source_file.source_code.as_bytes(),
      version::DENO,
      &self.config.hash,
    );

    let compiled_file_metadata = CompiledFileMetadata { version_hash };
    let meta_key = self
      .disk_cache
      .get_cache_filename_with_extension(module_specifier.as_url(), "meta");
    self.disk_cache.set(
      &meta_key,
      compiled_file_metadata.to_json_string()?.as_bytes(),
    )
  }

  /// Return associated source map file for given TS module.
  // TODO: ideally we shouldn't construct SourceFile by hand, but it should be delegated to
  // SourceFileFetcher
  pub fn get_source_map_file(
    &self,
    module_specifier: &ModuleSpecifier,
  ) -> Result<SourceFile, ErrBox> {
    let cache_key = self
      .disk_cache
      .get_cache_filename_with_extension(module_specifier.as_url(), "js.map");
    let source_code = self.disk_cache.get(&cache_key)?;
    let source_map_filename = self.disk_cache.location.join(cache_key);
    debug!("source map filename: {:?}", source_map_filename);

    let source_map_file = SourceFile {
      url: module_specifier.as_url().to_owned(),
      filename: source_map_filename,
      media_type: msg::MediaType::JavaScript,
      source_code: source_code.into(),
      types_header: None,
    };

    Ok(source_map_file)
  }

  /// Save source map file for given TS module to on-disk cache.
  fn cache_source_map(
    &self,
    module_specifier: &ModuleSpecifier,
    contents: &str,
  ) -> std::io::Result<()> {
    let js_key = self
      .disk_cache
      .get_cache_filename_with_extension(module_specifier.as_url(), "js");
    let js_path = self.disk_cache.location.join(js_key);
    let js_file_url =
      Url::from_file_path(js_path).expect("Bad file URL for file");

    let source_map_key = self
      .disk_cache
      .get_cache_filename_with_extension(module_specifier.as_url(), "js.map");

    let mut sm = SourceMap::from_slice(contents.as_bytes())
      .expect("Invalid source map content");
    sm.set_file(Some(&js_file_url.to_string()));
    sm.set_source(0, &module_specifier.to_string());

    let mut output: Vec<u8> = vec![];
    sm.to_writer(&mut output)
      .expect("Failed to write source map");

    self.disk_cache.set(&source_map_key, &output)
  }
}

impl SourceMapGetter for TsCompiler {
  fn get_source_map(&self, script_name: &str) -> Option<Vec<u8>> {
    self.try_to_resolve_and_get_source_map(script_name)
  }

  fn get_source_line(&self, script_name: &str, line: usize) -> Option<String> {
    self
      .try_resolve_and_get_source_file(script_name)
      .and_then(|out| {
        out.source_code.to_str().ok().map(|v| {
          // Do NOT use .lines(): it skips the terminating empty line.
          // (due to internally using .split_terminator() instead of .split())
          let lines: Vec<&str> = v.split('\n').collect();
          assert!(lines.len() > line);
          lines[line].to_string()
        })
      })
  }
}

// `SourceMapGetter` related methods
impl TsCompiler {
  fn try_to_resolve(&self, script_name: &str) -> Option<ModuleSpecifier> {
    // if `script_name` can't be resolved to ModuleSpecifier it's probably internal
    // script (like `gen/cli/bundle/compiler.js`) so we won't be
    // able to get source for it anyway
    ModuleSpecifier::resolve_url(script_name).ok()
  }

  fn try_resolve_and_get_source_file(
    &self,
    script_name: &str,
  ) -> Option<SourceFile> {
    if let Some(module_specifier) = self.try_to_resolve(script_name) {
      return self
        .file_fetcher
        .fetch_cached_source_file(&module_specifier, Permissions::allow_all());
    }

    None
  }

  fn try_to_resolve_and_get_source_map(
    &self,
    script_name: &str,
  ) -> Option<Vec<u8>> {
    if let Some(module_specifier) = self.try_to_resolve(script_name) {
      return match self.get_source_map_file(&module_specifier) {
        Ok(out) => Some(out.source_code.into_bytes()),
        Err(_) => {
          // Check if map is inlined
          if let Ok(compiled_source) =
            self.get_compiled_module(module_specifier.as_url())
          {
            let mut content_lines = compiled_source
              .code
              .split('\n')
              .map(|s| s.to_string())
              .collect::<Vec<String>>();

            if !content_lines.is_empty() {
              let last_line = content_lines.pop().unwrap();
              if last_line.starts_with(
                "//# sourceMappingURL=data:application/json;base64,",
              ) {
                let encoded = last_line.trim_start_matches(
                  "//# sourceMappingURL=data:application/json;base64,",
                );
                let decoded_map =
                  base64::decode(encoded).expect("failed to parse source map");
                return Some(decoded_map);
              }
            }
          }

          None
        }
      };
    }

    None
  }
}

async fn execute_in_same_thread(
  global_state: GlobalState,
  permissions: Permissions,
  req: String,
) -> Result<String, ErrBox> {
  let mut worker = create_compiler_worker(global_state.clone(), permissions);
  let script = format!("globalThis.tsCompilerOnMessage({{ data: {} }});", req);
  worker.execute2("<compiler>", &script)?;
  (&mut *worker).await?;
  Ok(worker.get_response())
}

async fn create_runtime_module_graph(
  global_state: GlobalState,
  permissions: Permissions,
  root_name: &str,
  sources: &Option<HashMap<String, String>>,
  maybe_options: &Option<String>,
) -> Result<(Vec<String>, ModuleGraph), OpError> {
  let mut root_names = vec![];
  let mut module_graph_loader = ModuleGraphLoader::new(
    global_state.file_fetcher.clone(),
    None,
    permissions,
    false,
    false,
  );

  if let Some(s_map) = sources {
    root_names.push(root_name.to_string());
    module_graph_loader.build_local_graph(root_name, s_map)?;
  } else {
    let module_specifier =
      ModuleSpecifier::resolve_import(root_name, "<unknown>")?;
    root_names.push(module_specifier.to_string());
    module_graph_loader
      .add_to_graph(&module_specifier, None)
      .await?;
  }

  // download all additional files from TSconfig and add them to root_names
  if let Some(options) = maybe_options {
    let options_json: serde_json::Value = serde_json::from_str(options)?;
    if let Some(types_option) = options_json.get("types") {
      let types_arr = types_option.as_array().expect("types is not an array");

      for type_value in types_arr {
        let type_str = type_value
          .as_str()
          .expect("type is not a string")
          .to_string();
        let type_specifier = ModuleSpecifier::resolve_url_or_path(&type_str)?;
        module_graph_loader
          .add_to_graph(&type_specifier, None)
          .await?;
        root_names.push(type_specifier.to_string())
      }
    }
  }

  Ok((root_names, module_graph_loader.get_graph()))
}

/// Because TS compiler can raise runtime error, we need to
/// manually convert formatted JSError into and OpError.
fn js_error_to_op_error(error: ErrBox) -> OpError {
  match error.downcast::<JSError>() {
    Ok(js_error) => {
      let msg = format!("Error in TS compiler:\n{}", js_error);
      OpError::other(msg)
    }
    Err(error) => error.into(),
  }
}

/// This function is used by `Deno.compile()` API.
pub async fn runtime_compile(
  global_state: GlobalState,
  permissions: Permissions,
  root_name: &str,
  sources: &Option<HashMap<String, String>>,
  maybe_options: &Option<String>,
) -> Result<Value, OpError> {
  let (root_names, module_graph) = create_runtime_module_graph(
    global_state.clone(),
    permissions.clone(),
    root_name,
    sources,
    maybe_options,
  )
  .await?;
  let module_graph_json =
    serde_json::to_value(module_graph).expect("Failed to serialize data");

  let req_msg = json!({
    "type": msg::CompilerRequestType::RuntimeCompile,
    "target": "runtime",
    "rootNames": root_names,
    "sourceFileMap": module_graph_json,
    "options": maybe_options,
    "unstable": global_state.flags.unstable,
  })
  .to_string();

  let compiler = global_state.ts_compiler.clone();

  let json_str = execute_in_same_thread(global_state, permissions, req_msg)
    .await
    .map_err(js_error_to_op_error)?;

  let response: RuntimeCompileResponse = serde_json::from_str(&json_str)?;

  if response.diagnostics.is_empty() && sources.is_none() {
    compiler.cache_emitted_files(response.emit_map)?;
  }

  // We're returning `Ok()` instead of `Err()` because it's not runtime
  // error if there were diagnostics produced; we want to let user handle
  // diagnostics in the runtime.
  Ok(serde_json::from_str::<Value>(&json_str).unwrap())
}

/// This function is used by `Deno.bundle()` API.
pub async fn runtime_bundle(
  global_state: GlobalState,
  permissions: Permissions,
  root_name: &str,
  sources: &Option<HashMap<String, String>>,
  maybe_options: &Option<String>,
) -> Result<Value, OpError> {
  let (root_names, module_graph) = create_runtime_module_graph(
    global_state.clone(),
    permissions.clone(),
    root_name,
    sources,
    maybe_options,
  )
  .await?;
  let module_graph_json =
    serde_json::to_value(module_graph).expect("Failed to serialize data");

  let req_msg = json!({
    "type": msg::CompilerRequestType::RuntimeBundle,
    "target": "runtime",
    "rootNames": root_names,
    "sourceFileMap": module_graph_json,
    "options": maybe_options,
    "unstable": global_state.flags.unstable,
  })
  .to_string();

  let json_str = execute_in_same_thread(global_state, permissions, req_msg)
    .await
    .map_err(js_error_to_op_error)?;
  let _response: RuntimeBundleResponse = serde_json::from_str(&json_str)?;
  // We're returning `Ok()` instead of `Err()` because it's not runtime
  // error if there were diagnostics produced; we want to let user handle
  // diagnostics in the runtime.
  Ok(serde_json::from_str::<Value>(&json_str).unwrap())
}

/// This function is used by `Deno.transpileOnly()` API.
pub async fn runtime_transpile(
  global_state: GlobalState,
  permissions: Permissions,
  sources: &HashMap<String, String>,
  options: &Option<String>,
) -> Result<Value, OpError> {
  let req_msg = json!({
    "type": msg::CompilerRequestType::RuntimeTranspile,
    "sources": sources,
    "options": options,
  })
  .to_string();

  let json_str = execute_in_same_thread(global_state, permissions, req_msg)
    .await
    .map_err(js_error_to_op_error)?;
  let v = serde_json::from_str::<serde_json::Value>(&json_str)
    .expect("Error decoding JSON string.");
  Ok(v)
}

#[derive(Clone, Debug, PartialEq)]
enum DependencyKind {
  Import,
  DynamicImport,
  Export,
}

#[derive(Clone, Debug, PartialEq)]
struct DependencyDescriptor {
  span: Span,
  specifier: String,
  kind: DependencyKind,
}

struct DependencyVisitor {
  dependencies: Vec<DependencyDescriptor>,
}

impl Visit for DependencyVisitor {
  fn visit_import_decl(
    &mut self,
    import_decl: &swc_ecmascript::ast::ImportDecl,
    _parent: &dyn Node,
  ) {
    let src_str = import_decl.src.value.to_string();
    self.dependencies.push(DependencyDescriptor {
      specifier: src_str,
      kind: DependencyKind::Import,
      span: import_decl.span,
    });
  }

  fn visit_named_export(
    &mut self,
    named_export: &swc_ecmascript::ast::NamedExport,
    _parent: &dyn Node,
  ) {
    if let Some(src) = &named_export.src {
      let src_str = src.value.to_string();
      self.dependencies.push(DependencyDescriptor {
        specifier: src_str,
        kind: DependencyKind::Export,
        span: named_export.span,
      });
    }
  }

  fn visit_export_all(
    &mut self,
    export_all: &swc_ecmascript::ast::ExportAll,
    _parent: &dyn Node,
  ) {
    let src_str = export_all.src.value.to_string();
    self.dependencies.push(DependencyDescriptor {
      specifier: src_str,
      kind: DependencyKind::Export,
      span: export_all.span,
    });
  }

  fn visit_ts_import_type(
    &mut self,
    ts_import_type: &swc_ecmascript::ast::TsImportType,
    _parent: &dyn Node,
  ) {
    // TODO(bartlomieju): possibly add separate DependencyKind
    let src_str = ts_import_type.arg.value.to_string();
    self.dependencies.push(DependencyDescriptor {
      specifier: src_str,
      kind: DependencyKind::Import,
      span: ts_import_type.arg.span,
    });
  }

  fn visit_call_expr(
    &mut self,
    call_expr: &swc_ecmascript::ast::CallExpr,
    parent: &dyn Node,
  ) {
    use swc_ecmascript::ast::Expr::*;
    use swc_ecmascript::ast::ExprOrSuper::*;

    swc_ecmascript::visit::visit_call_expr(self, call_expr, parent);
    let boxed_expr = match call_expr.callee.clone() {
      Super(_) => return,
      Expr(boxed) => boxed,
    };

    match &*boxed_expr {
      Ident(ident) => {
        if &ident.sym.to_string() != "import" {
          return;
        }
      }
      _ => return,
    };

    if let Some(arg) = call_expr.args.get(0) {
      match &*arg.expr {
        Lit(lit) => {
          if let swc_ecmascript::ast::Lit::Str(str_) = lit {
            let src_str = str_.value.to_string();
            self.dependencies.push(DependencyDescriptor {
              specifier: src_str,
              kind: DependencyKind::DynamicImport,
              span: call_expr.span,
            });
          }
        }
        _ => return,
      }
    }
  }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ImportDesc {
  pub specifier: String,
  pub deno_types: Option<String>,
  pub location: Location,
}

#[derive(Clone, Debug, PartialEq)]
pub enum TsReferenceKind {
  Lib,
  Types,
  Path,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TsReferenceDesc {
  pub kind: TsReferenceKind,
  pub specifier: String,
  pub location: Location,
}

// TODO(bartlomieju): handle imports in ambient contexts/TS modules
/// This function is a port of `ts.preProcessFile()`
///
/// Additionally it captures `@deno-types` references directly
/// preceeding `import .. from` and `export .. from` statements.
pub fn pre_process_file(
  file_name: &str,
  media_type: MediaType,
  source_code: &str,
  analyze_dynamic_imports: bool,
) -> Result<(Vec<ImportDesc>, Vec<TsReferenceDesc>), SwcDiagnosticBuffer> {
  let parser = AstParser::default();
  let parse_result = parser.parse_module(file_name, media_type, source_code);
  let module = parse_result?;
  let mut collector = DependencyVisitor {
    dependencies: vec![],
  };
  let module_span = module.span;
  collector.visit_module(&module, &module);

  let dependency_descriptors = collector.dependencies;

  // for each import check if there's relevant @deno-types directive
  let imports = dependency_descriptors
    .iter()
    .filter(|desc| {
      if analyze_dynamic_imports {
        return true;
      }

      desc.kind != DependencyKind::DynamicImport
    })
    .map(|desc| {
      let location = parser.get_span_location(desc.span);
      let deno_types = get_deno_types(&parser, desc.span);
      ImportDesc {
        specifier: desc.specifier.to_string(),
        deno_types,
        location: location.into(),
      }
    })
    .collect();

  // analyze comment from beginning of the file and find TS directives
  let comments = parser
    .comments
    .with_leading(module_span.lo(), |cmts| cmts.to_vec());

  let mut references = vec![];
  for comment in comments {
    if comment.kind != CommentKind::Line {
      continue;
    }

    let text = comment.text.to_string();
    if let Some((kind, specifier)) = parse_ts_reference(text.trim()) {
      let location = parser.get_span_location(comment.span);
      references.push(TsReferenceDesc {
        kind,
        specifier,
        location: location.into(),
      });
    }
  }
  Ok((imports, references))
}

fn get_deno_types(parser: &AstParser, span: Span) -> Option<String> {
  let comments = parser.get_span_comments(span);

  if comments.is_empty() {
    return None;
  }

  // @deno-types must directly prepend import statement - hence
  // checking last comment for span
  let last = comments.last().unwrap();
  let comment = last.text.trim_start();
  parse_deno_types(&comment)
}

fn parse_ts_reference(comment: &str) -> Option<(TsReferenceKind, String)> {
  if !XML_COMMENT_START_RE.is_match(comment) {
    return None;
  }

  let (kind, specifier) =
    if let Some(capture_groups) = PATH_REFERENCE_RE.captures(comment) {
      (TsReferenceKind::Path, capture_groups.get(3).unwrap())
    } else if let Some(capture_groups) = TYPES_REFERENCE_RE.captures(comment) {
      (TsReferenceKind::Types, capture_groups.get(3).unwrap())
    } else if let Some(capture_groups) = LIB_REFERENCE_RE.captures(comment) {
      (TsReferenceKind::Lib, capture_groups.get(3).unwrap())
    } else {
      return None;
    };

  Some((kind, specifier.as_str().to_string()))
}

fn parse_deno_types(comment: &str) -> Option<String> {
  if let Some(capture_groups) = DENO_TYPES_RE.captures(comment) {
    if let Some(specifier) = capture_groups.get(1) {
      let s = specifier
        .as_str()
        .trim_start_matches('\"')
        .trim_start_matches('\'')
        .trim_end_matches('\"')
        .trim_end_matches('\'')
        .to_string();
      return Some(s);
    }
  }

  None
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::deno_dir;
  use crate::fs as deno_fs;
  use crate::http_cache;
  use deno_core::ModuleSpecifier;
  use std::path::PathBuf;
  use tempfile::TempDir;

  #[test]
  fn test_parse_deno_types() {
    assert_eq!(
      parse_deno_types("@deno-types=./a/b/c.d.ts"),
      Some("./a/b/c.d.ts".to_string())
    );
    assert_eq!(
      parse_deno_types("@deno-types=\"./a/b/c.d.ts\""),
      Some("./a/b/c.d.ts".to_string())
    );
    assert_eq!(
      parse_deno_types("@deno-types = https://dneo.land/x/some/package/a.d.ts"),
      Some("https://dneo.land/x/some/package/a.d.ts".to_string())
    );
    assert_eq!(
      parse_deno_types("@deno-types = ./a/b/c.d.ts"),
      Some("./a/b/c.d.ts".to_string())
    );
    assert!(parse_deno_types("asdf").is_none());
    assert!(parse_deno_types("// deno-types = fooo").is_none());
    assert_eq!(
      parse_deno_types("@deno-types=./a/b/c.d.ts some comment"),
      Some("./a/b/c.d.ts".to_string())
    );
    assert_eq!(
      parse_deno_types(
        "@deno-types=./a/b/c.d.ts // some comment after slashes"
      ),
      Some("./a/b/c.d.ts".to_string())
    );
  }

  #[test]
  fn test_parse_ts_reference() {
    assert_eq!(
      parse_ts_reference(r#"/ <reference lib="deno.shared_globals" />"#),
      Some((TsReferenceKind::Lib, "deno.shared_globals".to_string()))
    );
    assert_eq!(
      parse_ts_reference(r#"/ <reference path="./type/reference/dep.ts" />"#),
      Some((TsReferenceKind::Path, "./type/reference/dep.ts".to_string()))
    );
    assert_eq!(
      parse_ts_reference(r#"/ <reference types="./type/reference.d.ts" />"#),
      Some((TsReferenceKind::Types, "./type/reference.d.ts".to_string()))
    );
    assert!(parse_ts_reference("asdf").is_none());
    assert!(
      parse_ts_reference(r#"/ <reference unknown="unknown" />"#).is_none()
    );
  }

  #[tokio::test]
  async fn test_compile() {
    let p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
      .parent()
      .unwrap()
      .join("cli/tests/002_hello.ts");
    let specifier =
      ModuleSpecifier::resolve_url_or_path(p.to_str().unwrap()).unwrap();
    let out = SourceFile {
      url: specifier.as_url().clone(),
      filename: PathBuf::from(p.to_str().unwrap().to_string()),
      media_type: msg::MediaType::TypeScript,
      source_code: include_bytes!("./tests/002_hello.ts").to_vec().into(),
      types_header: None,
    };
    let dir =
      deno_dir::DenoDir::new(Some(test_util::new_deno_dir().path().to_owned()))
        .unwrap();
    let http_cache = http_cache::HttpCache::new(&dir.root.join("deps"));
    let mock_state = GlobalState::mock(
      vec![String::from("deno"), String::from("hello.ts")],
      None,
    );
    let file_fetcher = SourceFileFetcher::new(
      http_cache,
      true,
      mock_state.flags.cache_blocklist.clone(),
      false,
      false,
      None,
    )
    .unwrap();

    let mut module_graph_loader = ModuleGraphLoader::new(
      file_fetcher.clone(),
      None,
      Permissions::allow_all(),
      false,
      false,
    );
    module_graph_loader
      .add_to_graph(&specifier, None)
      .await
      .expect("Failed to create graph");
    let module_graph = module_graph_loader.get_graph();

    let ts_compiler = TsCompiler::new(
      file_fetcher,
      mock_state.flags.clone(),
      dir.gen_cache.clone(),
    )
    .unwrap();

    let result = ts_compiler
      .compile(
        mock_state.clone(),
        &out,
        TargetLib::Main,
        Permissions::allow_all(),
        module_graph,
        false,
      )
      .await;
    assert!(result.is_ok());
    let compiled_file = ts_compiler.get_compiled_module(&out.url).unwrap();
    let source_code = compiled_file.code;
    assert!(source_code
      .as_bytes()
      .starts_with(b"\"use strict\";\nconsole.log(\"Hello World\");"));
    let mut lines: Vec<String> =
      source_code.split('\n').map(|s| s.to_string()).collect();
    let last_line = lines.pop().unwrap();
    assert!(last_line
      .starts_with("//# sourceMappingURL=data:application/json;base64"));
  }

  #[tokio::test]
  async fn test_transpile() {
    let p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
      .parent()
      .unwrap()
      .join("cli/tests/002_hello.ts");
    let specifier =
      ModuleSpecifier::resolve_url_or_path(p.to_str().unwrap()).unwrap();
    let out = SourceFile {
      url: specifier.as_url().clone(),
      filename: PathBuf::from(p.to_str().unwrap().to_string()),
      media_type: msg::MediaType::TypeScript,
      source_code: include_bytes!("./tests/002_hello.ts").to_vec().into(),
      types_header: None,
    };
    let dir =
      deno_dir::DenoDir::new(Some(test_util::new_deno_dir().path().to_owned()))
        .unwrap();
    let http_cache = http_cache::HttpCache::new(&dir.root.join("deps"));
    let mock_state = GlobalState::mock(
      vec![String::from("deno"), String::from("hello.ts")],
      None,
    );
    let file_fetcher = SourceFileFetcher::new(
      http_cache,
      true,
      mock_state.flags.cache_blocklist.clone(),
      false,
      false,
      None,
    )
    .unwrap();

    let mut module_graph_loader = ModuleGraphLoader::new(
      file_fetcher.clone(),
      None,
      Permissions::allow_all(),
      false,
      false,
    );
    module_graph_loader
      .add_to_graph(&specifier, None)
      .await
      .expect("Failed to create graph");
    let module_graph = module_graph_loader.get_graph();

    let ts_compiler = TsCompiler::new(
      file_fetcher,
      mock_state.flags.clone(),
      dir.gen_cache.clone(),
    )
    .unwrap();

    let result = ts_compiler.transpile(module_graph).await;
    assert!(result.is_ok());
    let compiled_file = ts_compiler.get_compiled_module(&out.url).unwrap();
    let source_code = compiled_file.code;
    assert!(source_code
      .as_bytes()
      .starts_with(b"console.log(\"Hello World\");"));
    let mut lines: Vec<String> =
      source_code.split('\n').map(|s| s.to_string()).collect();
    let last_line = lines.pop().unwrap();
    assert!(last_line
      .starts_with("//# sourceMappingURL=data:application/json;base64"));
  }

  #[tokio::test]
  async fn test_bundle() {
    let p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
      .parent()
      .unwrap()
      .join("cli/tests/002_hello.ts");
    use deno_core::ModuleSpecifier;
    let module_name =
      ModuleSpecifier::resolve_url_or_path(p.to_str().unwrap()).unwrap();

    let mock_state = GlobalState::mock(
      vec![
        String::from("deno"),
        p.to_string_lossy().into(),
        String::from("$deno$/bundle.js"),
      ],
      None,
    );

    let result = mock_state
      .ts_compiler
      .bundle(mock_state.clone(), module_name)
      .await;

    assert!(result.is_ok());
  }

  #[test]
  fn test_source_code_version_hash() {
    assert_eq!(
      "0185b42de0686b4c93c314daaa8dee159f768a9e9a336c2a5e3d5b8ca6c4208c",
      source_code_version_hash(b"1+2", "0.4.0", b"{}")
    );
    // Different source_code should result in different hash.
    assert_eq!(
      "e58631f1b6b6ce2b300b133ec2ad16a8a5ba6b7ecf812a8c06e59056638571ac",
      source_code_version_hash(b"1", "0.4.0", b"{}")
    );
    // Different version should result in different hash.
    assert_eq!(
      "307e6200347a88dbbada453102deb91c12939c65494e987d2d8978f6609b5633",
      source_code_version_hash(b"1", "0.1.0", b"{}")
    );
    // Different config should result in different hash.
    assert_eq!(
      "195eaf104a591d1d7f69fc169c60a41959c2b7a21373cd23a8f675f877ec385f",
      source_code_version_hash(b"1", "0.4.0", b"{\"compilerOptions\": {}}")
    );
  }

  #[test]
  fn test_compile_js() {
    let temp_dir = TempDir::new().expect("tempdir fail");
    let temp_dir_path = temp_dir.path();

    let test_cases = vec![
      // valid JSON
      (r#"{ "compilerOptions": { "checkJs": true } } "#, true),
      // JSON with comment
      (
        r#"{ "compilerOptions": { // force .js file compilation by Deno "checkJs": true } } "#,
        true,
      ),
      // invalid JSON
      (r#"{ "compilerOptions": { "checkJs": true },{ } "#, true),
      // without content
      ("", false),
    ];

    let path = temp_dir_path.join("tsconfig.json");
    let path_str = path.to_str().unwrap().to_string();

    for (json_str, expected) in test_cases {
      deno_fs::write_file(&path, json_str.as_bytes(), 0o666).unwrap();
      let config = CompilerConfig::load(Some(path_str.clone())).unwrap();
      assert_eq!(config.compile_js, expected);
    }
  }

  #[test]
  fn test_compiler_config_load() {
    let temp_dir = TempDir::new().expect("tempdir fail");
    let temp_dir_path = temp_dir.path();
    let path = temp_dir_path.join("doesnotexist.json");
    let path_str = path.to_str().unwrap().to_string();
    let res = CompilerConfig::load(Some(path_str));
    assert!(res.is_err());
  }
}
