// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use crate::compilers::CompiledModule;
use crate::compilers::CompiledModuleFuture;
use crate::deno_error::DenoError;
use crate::diagnostics::Diagnostic;
use crate::disk_cache::DiskCache;
use crate::file_fetcher::SourceFile;
use crate::file_fetcher::SourceFileFetcher;
use crate::msg;
use crate::msg::ErrorKind;
use crate::resources;
use crate::source_maps::SourceMapGetter;
use crate::startup_data;
use crate::state::*;
use crate::version;
use crate::worker::Worker;
use deno::Buf;
use deno::ErrBox;
use deno::ModuleSpecifier;
use futures::Future;
use futures::Stream;
use ring;
use std::collections::HashSet;
use std::fmt::Write;
use std::fs;
use std::path::PathBuf;
use std::str;
use std::sync::atomic::Ordering;
use std::sync::Mutex;
use url::Url;

/// Struct which represents the state of the compiler
/// configuration where the first is canonical name for the configuration file,
/// second is a vector of the bytes of the contents of the configuration file,
/// third is bytes of the hash of contents.
#[derive(Clone)]
pub struct CompilerConfig {
  pub path: Option<PathBuf>,
  pub content: Option<Vec<u8>>,
  pub hash: Vec<u8>,
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
      Some(config_file) => Some(config_file.canonicalize().unwrap().to_owned()),
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

    let ts_config = Self {
      path: config_path,
      content: config,
      hash: config_hash,
    };

    Ok(ts_config)
  }

  pub fn json(self: &Self) -> Result<serde_json::Value, ErrBox> {
    if self.content.is_none() {
      return Ok(serde_json::Value::Null);
    }

    let bytes = self.content.clone().unwrap();
    let json_string = std::str::from_utf8(&bytes)?;
    match serde_json::from_str(&json_string) {
      Ok(json_map) => Ok(json_map),
      Err(_) => Err(
        DenoError::new(
          ErrorKind::InvalidInput,
          "Compiler config is not a valid JSON".to_string(),
        ).into(),
      ),
    }
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

static SOURCE_PATH: &'static str = "source_path";
static VERSION_HASH: &'static str = "version_hash";

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

  pub fn to_json_string(self: &Self) -> Result<String, serde_json::Error> {
    let mut value_map = serde_json::map::Map::new();

    value_map.insert(SOURCE_PATH.to_owned(), json!(&self.source_path));
    value_map.insert(VERSION_HASH.to_string(), json!(&self.version_hash));
    serde_json::to_string(&value_map)
  }
}
/// Creates the JSON message send to compiler.ts's onmessage.
fn req(
  root_names: Vec<String>,
  compiler_config: CompilerConfig,
  bundle: Option<String>,
) -> Buf {
  let j = match (compiler_config.path, compiler_config.content) {
    (Some(config_path), Some(config_data)) => json!({
        "rootNames": root_names,
        "bundle": bundle,
        "configPath": config_path,
        "config": str::from_utf8(&config_data).unwrap(),
      }),
    _ => json!({
        "rootNames": root_names,
        "bundle": bundle,
      }),
  };

  j.to_string().into_boxed_str().into_boxed_bytes()
}

fn gen_hash(v: Vec<&[u8]>) -> String {
  let mut ctx = ring::digest::Context::new(&ring::digest::SHA1);
  for src in v.iter() {
    ctx.update(src);
  }
  let digest = ctx.finish();
  let mut out = String::new();
  // TODO There must be a better way to do this...
  for byte in digest.as_ref() {
    write!(&mut out, "{:02x}", byte).unwrap();
  }
  out
}

/// Emit a SHA1 hash based on source code, deno version and TS config.
/// Used to check if a recompilation for source code is needed.
pub fn source_code_version_hash(
  source_code: &[u8],
  version: &str,
  config_hash: &[u8],
) -> String {
  gen_hash(vec![source_code, version.as_bytes(), config_hash])
}

pub struct TsCompiler {
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

impl TsCompiler {
  pub fn new(
    file_fetcher: SourceFileFetcher,
    disk_cache: DiskCache,
    use_disk_cache: bool,
    config_path: Option<String>,
  ) -> Result<Self, ErrBox> {
    let config = CompilerConfig::load(config_path)?;

    // If `checkJs` is set to true in `compilerOptions` then we're gonna be compiling
    // JavaScript files as well
    let config_json = config.json()?;
    let compile_js = match &config_json.get("compilerOptions") {
      Some(serde_json::Value::Object(m)) => match m.get("checkJs") {
        Some(serde_json::Value::Bool(bool_)) => *bool_,
        _ => false,
      },
      _ => false,
    };

    let compiler = Self {
      file_fetcher,
      disk_cache,
      config,
      compiled: Mutex::new(HashSet::new()),
      use_disk_cache,
      compile_js,
    };

    Ok(compiler)
  }

  /// Create a new V8 worker with snapshot of TS compiler and setup compiler's runtime.
  fn setup_worker(state: ThreadSafeState) -> Worker {
    // Count how many times we start the compiler worker.
    state.metrics.compiler_starts.fetch_add(1, Ordering::SeqCst);

    let mut worker = Worker::new(
      "TS".to_string(),
      startup_data::compiler_isolate_init(),
      // TODO(ry) Maybe we should use a separate state for the compiler.
      // as was done previously.
      state.clone(),
    );
    worker.execute("denoMain()").unwrap();
    worker.execute("workerMain()").unwrap();
    worker.execute("compilerMain()").unwrap();
    worker
  }

  pub fn bundle_async(
    self: &Self,
    state: ThreadSafeState,
    module_name: String,
    out_file: String,
  ) -> impl Future<Item = (), Error = ErrBox> {
    debug!(
      "Invoking the compiler to bundle. module_name: {}",
      module_name
    );

    let root_names = vec![module_name.clone()];
    let req_msg = req(root_names, self.config.clone(), Some(out_file));

    let worker = TsCompiler::setup_worker(state.clone());
    let resource = worker.state.resource.clone();
    let compiler_rid = resource.rid;
    let first_msg_fut =
      resources::post_message_to_worker(compiler_rid, req_msg)
        .then(move |_| worker)
        .then(move |result| {
          if let Err(err) = result {
            // TODO(ry) Need to forward the error instead of exiting.
            eprintln!("{}", err.to_string());
            std::process::exit(1);
          }
          debug!("Sent message to worker");
          let stream_future =
            resources::get_message_stream_from_worker(compiler_rid)
              .into_future();
          stream_future.map(|(f, _rest)| f).map_err(|(f, _rest)| f)
        });

    first_msg_fut.map_err(|_| panic!("not handled")).and_then(
      move |maybe_msg: Option<Buf>| {
        debug!("Received message from worker");

        if let Some(msg) = maybe_msg {
          let json_str = std::str::from_utf8(&msg).unwrap();
          debug!("Message: {}", json_str);
          if let Some(diagnostics) = Diagnostic::from_emit_result(json_str) {
            return Err(ErrBox::from(diagnostics));
          }
        }

        Ok(())
      },
    )
  }

  /// Mark given module URL as compiled to avoid multiple compilations of same module
  /// in single run.
  fn mark_compiled(&self, url: &Url) {
    let mut c = self.compiled.lock().unwrap();
    c.insert(url.clone());
  }

  /// Check if given module URL has already been compiled and can be fetched directly from disk.
  fn has_compiled(&self, url: &Url) -> bool {
    let c = self.compiled.lock().unwrap();
    c.contains(url)
  }

  /// Asynchronously compile module and all it's dependencies.
  ///
  /// This method compiled every module at most once.
  ///
  /// If `--reload` flag was provided then compiler will not on-disk cache and force recompilation.
  ///
  /// If compilation is required then new V8 worker is spawned with fresh TS compiler.
  pub fn compile_async(
    self: &Self,
    state: ThreadSafeState,
    source_file: &SourceFile,
  ) -> Box<CompiledModuleFuture> {
    if self.has_compiled(&source_file.url) {
      return match self.get_compiled_module(&source_file.url) {
        Ok(compiled) => Box::new(futures::future::ok(compiled)),
        Err(err) => Box::new(futures::future::err(err)),
      };
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
            return Box::new(futures::future::ok(compiled_module));
          }
        }
      }
    }

    let source_file_ = source_file.clone();

    debug!(">>>>> compile_sync START");
    let module_url = source_file.url.clone();

    debug!(
      "Running rust part of compile_sync, module specifier: {}",
      &source_file.url
    );

    let root_names = vec![module_url.to_string()];
    let req_msg = req(root_names, self.config.clone(), None);

    let worker = TsCompiler::setup_worker(state.clone());
    let compiling_job = state.progress.add("Compile", &module_url.to_string());
    let state_ = state.clone();

    let resource = worker.state.resource.clone();
    let compiler_rid = resource.rid;
    let first_msg_fut =
      resources::post_message_to_worker(compiler_rid, req_msg)
        .then(move |_| worker)
        .then(move |result| {
          if let Err(err) = result {
            // TODO(ry) Need to forward the error instead of exiting.
            eprintln!("{}", err.to_string());
            std::process::exit(1);
          }
          debug!("Sent message to worker");
          let stream_future =
            resources::get_message_stream_from_worker(compiler_rid)
              .into_future();
          stream_future.map(|(f, _rest)| f).map_err(|(f, _rest)| f)
        });

    let fut = first_msg_fut
      .map_err(|_| panic!("not handled"))
      .and_then(move |maybe_msg: Option<Buf>| {
        debug!("Received message from worker");

        if let Some(msg) = maybe_msg {
          let json_str = std::str::from_utf8(&msg).unwrap();
          debug!("Message: {}", json_str);
          if let Some(diagnostics) = Diagnostic::from_emit_result(json_str) {
            return Err(ErrBox::from(diagnostics));
          }
        }

        Ok(())
      }).and_then(move |_| {
        // if we are this far it means compilation was successful and we can
        // load compiled filed from disk
        state_
          .ts_compiler
          .get_compiled_module(&source_file_.url)
          .map_err(|e| {
            // TODO: this situation shouldn't happen
            panic!("Expected to find compiled file: {}", e)
          })
      }).and_then(move |compiled_module| {
        // Explicit drop to keep reference alive until future completes.
        drop(compiling_job);

        Ok(compiled_module)
      }).then(move |r| {
        debug!(">>>>> compile_sync END");
        // TODO(ry) do this in worker's destructor.
        // resource.close();
        r
      });

    Box::new(fut)
  }

  /// Get associated `CompiledFileMetadata` for given module if it exists.
  pub fn get_metadata(self: &Self, url: &Url) -> Option<CompiledFileMetadata> {
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
    self: &Self,
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
    self: &Self,
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
    };

    Ok(compiled_module)
  }

  /// Save compiled JS file for given TS module to on-disk cache.
  ///
  /// Along compiled file a special metadata file is saved as well containing
  /// hash that can be validated to avoid unnecessary recompilation.
  fn cache_compiled_file(
    self: &Self,
    module_specifier: &ModuleSpecifier,
    contents: &str,
  ) -> std::io::Result<()> {
    let js_key = self
      .disk_cache
      .get_cache_filename_with_extension(module_specifier.as_url(), "js");
    self
      .disk_cache
      .set(&js_key, contents.as_bytes())
      .and_then(|_| {
        self.mark_compiled(module_specifier.as_url());

        let source_file = self
          .file_fetcher
          .fetch_source_file(&module_specifier)
          .expect("Source file not found");

        let version_hash = source_code_version_hash(
          &source_file.source_code,
          version::DENO,
          &self.config.hash,
        );

        let compiled_file_metadata = CompiledFileMetadata {
          source_path: source_file.filename.to_owned(),
          version_hash,
        };
        let meta_key = self
          .disk_cache
          .get_cache_filename_with_extension(module_specifier.as_url(), "meta");
        self.disk_cache.set(
          &meta_key,
          compiled_file_metadata.to_json_string()?.as_bytes(),
        )
      })
  }

  /// Return associated source map file for given TS module.
  // TODO: ideally we shouldn't construct SourceFile by hand, but it should be delegated to
  // SourceFileFetcher
  pub fn get_source_map_file(
    self: &Self,
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
    };

    Ok(source_map_file)
  }

  /// Save source map file for given TS module to on-disk cache.
  fn cache_source_map(
    self: &Self,
    module_specifier: &ModuleSpecifier,
    contents: &str,
  ) -> std::io::Result<()> {
    let source_map_key = self
      .disk_cache
      .get_cache_filename_with_extension(module_specifier.as_url(), "js.map");
    self.disk_cache.set(&source_map_key, contents.as_bytes())
  }

  /// This method is called by TS compiler via an "op".
  pub fn cache_compiler_output(
    self: &Self,
    module_specifier: &ModuleSpecifier,
    extension: &str,
    contents: &str,
  ) -> std::io::Result<()> {
    match extension {
      ".map" => self.cache_source_map(module_specifier, contents),
      ".js" => self.cache_compiled_file(module_specifier, contents),
      _ => unreachable!(),
    }
  }
}

impl SourceMapGetter for TsCompiler {
  fn get_source_map(&self, script_name: &str) -> Option<Vec<u8>> {
    self
      .try_to_resolve_and_get_source_map(script_name)
      .and_then(|out| Some(out.source_code))
  }

  fn get_source_line(&self, script_name: &str, line: usize) -> Option<String> {
    self
      .try_resolve_and_get_source_file(script_name)
      .and_then(|out| {
        str::from_utf8(&out.source_code).ok().and_then(|v| {
          let lines: Vec<&str> = v.lines().collect();
          assert!(lines.len() > line);
          Some(lines[line].to_string())
        })
      })
  }
}

// `SourceMapGetter` related methods
impl TsCompiler {
  fn try_to_resolve(self: &Self, script_name: &str) -> Option<ModuleSpecifier> {
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
      return match self.file_fetcher.fetch_source_file(&module_specifier) {
        Ok(out) => Some(out),
        Err(_) => None,
      };
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

#[cfg(test)]
mod tests {
  use super::*;
  use crate::tokio_util;
  use deno::ModuleSpecifier;
  use std::path::PathBuf;

  impl TsCompiler {
    fn compile_sync(
      self: &Self,
      state: ThreadSafeState,
      source_file: &SourceFile,
    ) -> Result<CompiledModule, ErrBox> {
      tokio_util::block_on(self.compile_async(state, source_file))
    }
  }

  #[test]
  fn test_compile_sync() {
    tokio_util::init(|| {
      let specifier =
        ModuleSpecifier::resolve_url_or_path("./tests/002_hello.ts").unwrap();

      let out = SourceFile {
        url: specifier.as_url().clone(),
        filename: PathBuf::from("/tests/002_hello.ts"),
        media_type: msg::MediaType::TypeScript,
        source_code: include_bytes!("../../tests/002_hello.ts").to_vec(),
      };

      let mock_state = ThreadSafeState::mock(vec![
        String::from("./deno"),
        String::from("hello.js"),
      ]);
      let compiled = mock_state
        .ts_compiler
        .compile_sync(mock_state.clone(), &out)
        .unwrap();
      assert!(
        compiled
          .code
          .as_bytes()
          .starts_with("console.log(\"Hello World\");".as_bytes())
      );
    })
  }

  #[test]
  fn test_bundle_async() {
    let specifier = "./tests/002_hello.ts";
    use deno::ModuleSpecifier;
    let module_name = ModuleSpecifier::resolve_url_or_path(specifier)
      .unwrap()
      .to_string();

    let state = ThreadSafeState::mock(vec![
      String::from("./deno"),
      String::from("./tests/002_hello.ts"),
      String::from("$deno$/bundle.js"),
    ]);
    let out = state.ts_compiler.bundle_async(
      state.clone(),
      module_name,
      String::from("$deno$/bundle.js"),
    );
    assert!(tokio_util::block_on(out).is_ok());
  }

  #[test]
  fn test_source_code_version_hash() {
    assert_eq!(
      "08574f9cdeb94fd3fb9cdc7a20d086daeeb42bca",
      source_code_version_hash(b"1+2", "0.4.0", b"{}")
    );
    // Different source_code should result in different hash.
    assert_eq!(
      "d8abe2ead44c3ff8650a2855bf1b18e559addd06",
      source_code_version_hash(b"1", "0.4.0", b"{}")
    );
    // Different version should result in different hash.
    assert_eq!(
      "d6feffc5024d765d22c94977b4fe5975b59d6367",
      source_code_version_hash(b"1", "0.1.0", b"{}")
    );
    // Different config should result in different hash.
    assert_eq!(
      "3b35db249b26a27decd68686f073a58266b2aec2",
      source_code_version_hash(b"1", "0.4.0", b"{\"compilerOptions\": {}}")
    );
  }
}
