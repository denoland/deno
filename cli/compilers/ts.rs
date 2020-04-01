// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use super::compiler_worker::CompilerWorker;
use crate::colors;
use crate::compilers::CompilationResultFuture;
use crate::compilers::CompiledModule;
use crate::diagnostics::Diagnostic;
use crate::disk_cache::DiskCache;
use crate::file_fetcher::SourceFile;
use crate::file_fetcher::SourceFileFetcher;
use crate::global_state::GlobalState;
use crate::msg;
use crate::op_error::OpError;
use crate::ops::JsonResult;
use crate::source_maps::SourceMapGetter;
use crate::startup_data;
use crate::state::*;
use crate::tokio_util;
use crate::version;
use crate::worker::WorkerEvent;
use crate::worker::WorkerHandle;
use deno_core::Buf;
use deno_core::ErrBox;
use deno_core::ModuleSpecifier;
use futures::future::FutureExt;
use log::info;
use regex::Regex;
use serde_json::json;
use std::collections::HashMap;
use std::collections::HashSet;
use std::fs;
use std::hash::BuildHasher;
use std::io;
use std::ops::Deref;
use std::path::PathBuf;
use std::pin::Pin;
use std::str;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::sync::Mutex;
use url::Url;

lazy_static! {
  static ref CHECK_JS_RE: Regex =
    Regex::new(r#""checkJs"\s*?:\s*?true"#).unwrap();
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
/// Includes source code path and state hash.
/// version_hash is used to validate versions of the file
/// and could be used to remove stale file in cache.
pub struct CompiledFileMetadata {
  pub source_path: PathBuf,
  pub version_hash: String,
}

static SOURCE_PATH: &str = "source_path";
static VERSION_HASH: &str = "version_hash";

impl CompiledFileMetadata {
  pub fn from_json_string(metadata_string: String) -> Option<Self> {
    // TODO: use serde for deserialization
    let maybe_metadata_json: serde_json::Result<serde_json::Value> =
      serde_json::from_str(&metadata_string);

    if let Ok(metadata_json) = maybe_metadata_json {
      let source_path = metadata_json[SOURCE_PATH].as_str().map(PathBuf::from);
      let version_hash = metadata_json[VERSION_HASH].as_str().map(String::from);

      if source_path.is_none() || version_hash.is_none() {
        return None;
      }

      return Some(CompiledFileMetadata {
        source_path: source_path.unwrap(),
        version_hash: version_hash.unwrap(),
      });
    }

    None
  }

  pub fn to_json_string(&self) -> Result<String, serde_json::Error> {
    let mut value_map = serde_json::map::Map::new();

    value_map.insert(SOURCE_PATH.to_owned(), json!(&self.source_path));
    value_map.insert(VERSION_HASH.to_string(), json!(&self.version_hash));
    serde_json::to_string(&value_map)
  }
}
/// Creates the JSON message send to compiler.ts's onmessage.
fn req(
  request_type: msg::CompilerRequestType,
  root_names: Vec<String>,
  compiler_config: CompilerConfig,
  out_file: Option<PathBuf>,
  target: &str,
  bundle: bool,
) -> Buf {
  let j = match (compiler_config.path, compiler_config.content) {
    (Some(config_path), Some(config_data)) => json!({
      "type": request_type as i32,
      "target": target,
      "rootNames": root_names,
      "outFile": out_file,
      "bundle": bundle,
      "configPath": config_path,
      "config": str::from_utf8(&config_data).unwrap(),
    }),
    _ => json!({
      "type": request_type as i32,
      "target": target,
      "rootNames": root_names,
      "outFile": out_file,
      "bundle": bundle,
    }),
  };

  j.to_string().into_boxed_str().into_boxed_bytes()
}

/// Emit a SHA256 hash based on source code, deno version and TS config.
/// Used to check if a recompilation for source code is needed.
pub fn source_code_version_hash(
  source_code: &[u8],
  version: &str,
  config_hash: &[u8],
) -> String {
  crate::checksum::gen(vec![source_code, version.as_bytes(), config_hash])
}

pub struct TsCompilerInner {
  pub file_fetcher: SourceFileFetcher,
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

impl TsCompiler {
  pub fn new(
    file_fetcher: SourceFileFetcher,
    disk_cache: DiskCache,
    use_disk_cache: bool,
    config_path: Option<String>,
  ) -> Result<Self, ErrBox> {
    let config = CompilerConfig::load(config_path)?;
    Ok(TsCompiler(Arc::new(TsCompilerInner {
      file_fetcher,
      disk_cache,
      compile_js: config.compile_js,
      config,
      compiled: Mutex::new(HashSet::new()),
      use_disk_cache,
    })))
  }

  /// Create a new V8 worker with snapshot of TS compiler and setup compiler's
  /// runtime.
  fn setup_worker(global_state: GlobalState) -> CompilerWorker {
    let entry_point =
      ModuleSpecifier::resolve_url_or_path("./__$deno$ts_compiler.ts").unwrap();
    let worker_state = State::new(global_state.clone(), None, entry_point)
      .expect("Unable to create worker state");

    // Count how many times we start the compiler worker.
    global_state.compiler_starts.fetch_add(1, Ordering::SeqCst);

    let mut worker = CompilerWorker::new(
      "TS".to_string(),
      startup_data::compiler_isolate_init(),
      worker_state,
    );
    worker.execute("bootstrapTsCompilerRuntime()").unwrap();
    worker
  }

  pub async fn bundle(
    &self,
    global_state: GlobalState,
    module_name: String,
    out_file: Option<PathBuf>,
  ) -> Result<(), ErrBox> {
    debug!(
      "Invoking the compiler to bundle. module_name: {}",
      module_name
    );

    let root_names = vec![module_name];
    let req_msg = req(
      msg::CompilerRequestType::Compile,
      root_names,
      self.config.clone(),
      out_file,
      "main",
      true,
    );

    let msg = execute_in_thread(global_state.clone(), req_msg).await?;
    let json_str = std::str::from_utf8(&msg).unwrap();
    debug!("Message: {}", json_str);
    if let Some(diagnostics) = Diagnostic::from_emit_result(json_str) {
      return Err(ErrBox::from(diagnostics));
    }
    Ok(())
  }

  /// Mark given module URL as compiled to avoid multiple compilations of same
  /// module in single run.
  fn mark_compiled(&self, url: &Url) {
    let mut c = self.compiled.lock().unwrap();
    c.insert(url.clone());
  }

  /// Check if given module URL has already been compiled and can be fetched
  /// directly from disk.
  fn has_compiled(&self, url: &Url) -> bool {
    let c = self.compiled.lock().unwrap();
    c.contains(url)
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
  ) -> Result<CompiledModule, ErrBox> {
    if self.has_compiled(&source_file.url) {
      return self.get_compiled_module(&source_file.url);
    }

    if self.use_disk_cache {
      // Try to load cached version:
      // 1. check if there's 'meta' file
      if let Some(metadata) = self.get_metadata(&source_file.url) {
        // 2. compare version hashes
        // TODO: it would probably be good idea to make it method implemented on SourceFile
        let version_hash_to_validate = source_code_version_hash(
          &source_file.source_code,
          version::DENO,
          &self.config.hash,
        );

        if metadata.version_hash == version_hash_to_validate {
          debug!("load_cache metadata version hash match");
          if let Ok(compiled_module) =
            self.get_compiled_module(&source_file.url)
          {
            self.mark_compiled(&source_file.url);
            return Ok(compiled_module);
          }
        }
      }
    }
    let source_file_ = source_file.clone();
    let module_url = source_file.url.clone();
    let target = match target {
      TargetLib::Main => "main",
      TargetLib::Worker => "worker",
    };
    let root_names = vec![module_url.to_string()];
    let req_msg = req(
      msg::CompilerRequestType::Compile,
      root_names,
      self.config.clone(),
      None,
      target,
      false,
    );

    let ts_compiler = self.clone();

    info!(
      "{} {}",
      colors::green("Compile".to_string()),
      module_url.to_string()
    );

    let msg = execute_in_thread(global_state.clone(), req_msg).await?;

    let json_str = std::str::from_utf8(&msg).unwrap();
    if let Some(diagnostics) = Diagnostic::from_emit_result(json_str) {
      return Err(ErrBox::from(diagnostics));
    }
    ts_compiler.get_compiled_module(&source_file_.url)
  }

  /// Get associated `CompiledFileMetadata` for given module if it exists.
  pub fn get_metadata(&self, url: &Url) -> Option<CompiledFileMetadata> {
    // Try to load cached version:
    // 1. check if there's 'meta' file
    let cache_key = self
      .disk_cache
      .get_cache_filename_with_extension(url, "meta");
    if let Ok(metadata_bytes) = self.disk_cache.get(&cache_key) {
      if let Ok(metadata) = std::str::from_utf8(&metadata_bytes) {
        if let Some(read_metadata) =
          CompiledFileMetadata::from_json_string(metadata.to_string())
        {
          return Some(read_metadata);
        }
      }
    }

    None
  }

  pub fn get_compiled_module(
    &self,
    module_url: &Url,
  ) -> Result<CompiledModule, ErrBox> {
    let compiled_source_file = self.get_compiled_source_file(module_url)?;

    let compiled_module = CompiledModule {
      code: str::from_utf8(&compiled_source_file.source_code)
        .unwrap()
        .to_string(),
      name: module_url.to_string(),
    };

    Ok(compiled_module)
  }

  /// Return compiled JS file for given TS module.
  // TODO: ideally we shouldn't construct SourceFile by hand, but it should be delegated to
  // SourceFileFetcher
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
      source_code: compiled_code,
      types_url: None,
    };

    Ok(compiled_module)
  }

  /// Save compiled JS file for given TS module to on-disk cache.
  ///
  /// Along compiled file a special metadata file is saved as well containing
  /// hash that can be validated to avoid unnecessary recompilation.
  async fn cache_compiled_file(
    &self,
    module_specifier: &ModuleSpecifier,
    contents: &str,
  ) -> std::io::Result<()> {
    let js_key = self
      .disk_cache
      .get_cache_filename_with_extension(module_specifier.as_url(), "js");
    self.disk_cache.set(&js_key, contents.as_bytes())?;
    self.mark_compiled(module_specifier.as_url());
    let source_file = self
      .file_fetcher
      .fetch_cached_source_file(&module_specifier)
      .await
      .expect("Source file not found");

    let version_hash = source_code_version_hash(
      &source_file.source_code,
      version::DENO,
      &self.config.hash,
    );

    let compiled_file_metadata = CompiledFileMetadata {
      source_path: source_file.filename,
      version_hash,
    };
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
      source_code,
      types_url: None,
    };

    Ok(source_map_file)
  }

  /// Save source map file for given TS module to on-disk cache.
  fn cache_source_map(
    &self,
    module_specifier: &ModuleSpecifier,
    contents: &str,
  ) -> std::io::Result<()> {
    let source_map_key = self
      .disk_cache
      .get_cache_filename_with_extension(module_specifier.as_url(), "js.map");
    self.disk_cache.set(&source_map_key, contents.as_bytes())
  }

  /// This method is called by TS compiler via an "op".
  pub async fn cache_compiler_output(
    &self,
    module_specifier: &ModuleSpecifier,
    extension: &str,
    contents: &str,
  ) -> std::io::Result<()> {
    match extension {
      ".map" => self.cache_source_map(module_specifier, contents),
      ".js" => self.cache_compiled_file(module_specifier, contents).await,
      _ => unreachable!(),
    }
  }
}

impl SourceMapGetter for TsCompiler {
  fn get_source_map(&self, script_name: &str) -> Option<Vec<u8>> {
    self
      .try_to_resolve_and_get_source_map(script_name)
      .map(|out| out.source_code)
  }

  fn get_source_line(&self, script_name: &str, line: usize) -> Option<String> {
    self
      .try_resolve_and_get_source_file(script_name)
      .and_then(|out| {
        str::from_utf8(&out.source_code).ok().and_then(|v| {
          // Do NOT use .lines(): it skips the terminating empty line.
          // (due to internally using .split_terminator() instead of .split())
          let lines: Vec<&str> = v.split('\n').collect();
          assert!(lines.len() > line);
          Some(lines[line].to_string())
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
      let fut = self
        .file_fetcher
        .fetch_cached_source_file(&module_specifier);
      return futures::executor::block_on(fut);
    }

    None
  }

  fn try_to_resolve_and_get_source_map(
    &self,
    script_name: &str,
  ) -> Option<SourceFile> {
    if let Some(module_specifier) = self.try_to_resolve(script_name) {
      return match self.get_source_map_file(&module_specifier) {
        Ok(out) => Some(out),
        Err(_) => None,
      };
    }

    None
  }
}

// TODO(bartlomieju): exactly same function is in `wasm.rs` - only difference
// it created WasmCompiler instead of TsCompiler - deduplicate
async fn execute_in_thread(
  global_state: GlobalState,
  req: Buf,
) -> Result<Buf, ErrBox> {
  let (handle_sender, handle_receiver) =
    std::sync::mpsc::sync_channel::<Result<WorkerHandle, ErrBox>>(1);
  let builder =
    std::thread::Builder::new().name("deno-ts-compiler".to_string());
  let join_handle = builder.spawn(move || {
    let worker = TsCompiler::setup_worker(global_state.clone());
    handle_sender.send(Ok(worker.thread_safe_handle())).unwrap();
    drop(handle_sender);
    tokio_util::run_basic(worker).expect("Panic in event loop");
  })?;
  let mut handle = handle_receiver.recv().unwrap()?;
  handle.post_message(req).await?;
  let event = handle.get_event().await.expect("Compiler didn't respond");
  let buf = match event {
    WorkerEvent::Message(buf) => Ok(buf),
    WorkerEvent::Error(error) => Err(error),
  }?;
  // Shutdown worker and wait for thread to finish
  handle.sender.close_channel();
  join_handle.join().unwrap();
  Ok(buf)
}

async fn execute_in_thread_json(
  req_msg: Buf,
  global_state: GlobalState,
) -> JsonResult {
  let msg = execute_in_thread(global_state, req_msg)
    .await
    .map_err(|e| OpError::other(e.to_string()))?;
  let json_str = std::str::from_utf8(&msg).unwrap();
  Ok(json!(json_str))
}

pub fn runtime_compile<S: BuildHasher>(
  global_state: GlobalState,
  root_name: &str,
  sources: &Option<HashMap<String, String, S>>,
  bundle: bool,
  options: &Option<String>,
) -> Pin<Box<CompilationResultFuture>> {
  let req_msg = json!({
    "type": msg::CompilerRequestType::RuntimeCompile as i32,
    "target": "runtime",
    "rootName": root_name,
    "sources": sources,
    "options": options,
    "bundle": bundle,
  })
  .to_string()
  .into_boxed_str()
  .into_boxed_bytes();

  execute_in_thread_json(req_msg, global_state).boxed_local()
}

pub fn runtime_transpile<S: BuildHasher>(
  global_state: GlobalState,
  sources: &HashMap<String, String, S>,
  options: &Option<String>,
) -> Pin<Box<CompilationResultFuture>> {
  let req_msg = json!({
    "type": msg::CompilerRequestType::RuntimeTranspile as i32,
    "sources": sources,
    "options": options,
  })
  .to_string()
  .into_boxed_str()
  .into_boxed_bytes();

  execute_in_thread_json(req_msg, global_state).boxed_local()
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::fs as deno_fs;
  use deno_core::ModuleSpecifier;
  use std::path::PathBuf;
  use tempfile::TempDir;

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
      source_code: include_bytes!("../tests/002_hello.ts").to_vec(),
      types_url: None,
    };
    let mock_state =
      GlobalState::mock(vec![String::from("deno"), String::from("hello.js")]);
    let result = mock_state
      .ts_compiler
      .compile(mock_state.clone(), &out, TargetLib::Main)
      .await;
    assert!(result.is_ok());
    assert!(result
      .unwrap()
      .code
      .as_bytes()
      .starts_with(b"\"use strict\";\nconsole.log(\"Hello World\");"));
  }

  #[tokio::test]
  async fn test_bundle() {
    let p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
      .parent()
      .unwrap()
      .join("cli/tests/002_hello.ts");
    use deno_core::ModuleSpecifier;
    let module_name = ModuleSpecifier::resolve_url_or_path(p.to_str().unwrap())
      .unwrap()
      .to_string();

    let state = GlobalState::mock(vec![
      String::from("deno"),
      p.to_string_lossy().into(),
      String::from("$deno$/bundle.js"),
    ]);

    let result = state
      .ts_compiler
      .bundle(
        state.clone(),
        module_name,
        Some(PathBuf::from("$deno$/bundle.js")),
      )
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
