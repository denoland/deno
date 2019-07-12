// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use crate::deno_dir::get_cache_filename;
use crate::deno_dir::DenoDir;
use crate::deno_dir::SourceFile;
use crate::deno_dir::SourceFileFetcher;
use crate::diagnostics::Diagnostic;
use crate::fs as deno_fs;
use crate::msg;
use crate::resources;
use crate::source_maps::SourceMapGetter;
use crate::startup_data;
use crate::state::*;
use crate::version;
use crate::worker::Worker;
use deno::Buf;
use deno::ErrBox;
use deno::ModuleSpecifier;
use futures::future::Either;
use futures::Future;
use futures::Stream;
use ring;
use std::fmt::Write;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::str;
use std::sync::atomic::Ordering;
use url::Url;

/// Optional tuple which represents the state of the compiler
/// configuration where the first is canonical name for the configuration file
/// and a vector of the bytes of the contents of the configuration file.
type CompilerConfig = Option<(String, Vec<u8>)>;

/// Information associated with compiled file in cache.
/// Includes source code path and state hash.
/// version_hash is used to validate versions of the file
/// and could be used to remove stale file in cache.
pub struct CompiledFileMetadata {
  pub source_path: String,
  pub version_hash: String,
}

static SOURCE_PATH: &'static str = "source_path";
static VERSION_HASH: &'static str = "version_hash";

impl CompiledFileMetadata {
  // TODO: use serde for deserialization
  /// Get compiled file metadata from meta_path.
  /// If operation failed or metadata file is corrupted,
  /// return None.
  pub fn load<P: AsRef<Path>>(meta_path: P) -> Option<CompiledFileMetadata> {
    let meta_path = meta_path.as_ref();
    let maybe_metadata_string = fs::read_to_string(meta_path).ok();
    maybe_metadata_string.as_ref()?;
    let maybe_metadata_json: serde_json::Result<serde_json::Value> =
      serde_json::from_str(&maybe_metadata_string.unwrap());
    if let Ok(metadata_json) = maybe_metadata_json {
      let source_path = metadata_json[SOURCE_PATH].as_str().map(String::from);
      let version_hash = metadata_json[VERSION_HASH].as_str().map(String::from);
      if source_path.is_none() || version_hash.is_none() {
        return None;
      }
      return Some(CompiledFileMetadata {
        source_path: source_path.unwrap(),
        version_hash: version_hash.unwrap(),
      });
    } else {
      return None;
    }
  }

  /// Save compiled file metadata to meta_path.
  pub fn save<P: AsRef<Path>>(self: &Self, meta_path: P) {
    let meta_path = meta_path.as_ref();
    // Remove possibly existing stale meta file.
    // May not exist. DON'T unwrap.
    let _ = std::fs::remove_file(&meta_path);
    let mut value_map = serde_json::map::Map::new();

    value_map.insert(SOURCE_PATH.to_owned(), json!(&self.source_path));
    value_map.insert(VERSION_HASH.to_string(), json!(&self.version_hash));

    let _ = serde_json::to_string(&value_map).map(|s| {
      let _ = deno_fs::write_file(meta_path, s, 0o666);
    });
  }
}

/// Creates the JSON message send to compiler.ts's onmessage.
fn req(
  root_names: Vec<String>,
  compiler_config: CompilerConfig,
  bundle: Option<String>,
) -> Buf {
  let j = if let Some((config_path, config_data)) = compiler_config {
    json!({
      "rootNames": root_names,
      "bundle": bundle,
      "configPath": config_path,
      "config": str::from_utf8(&config_data).unwrap(),
    })
  } else {
    json!({
      "rootNames": root_names,
      "bundle": bundle,
    })
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

fn load_config_file(
  config_path: Option<String>,
) -> (Option<String>, Option<Vec<u8>>) {
  // take the passed flag and resolve the file name relative to the cwd
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
    Some(config_file) => Some(
      config_file
        .canonicalize()
        .unwrap()
        .to_str()
        .unwrap()
        .to_owned(),
    ),
    _ => None,
  };

  // Load the contents of the configuration file
  let config = match &config_file {
    Some(config_file) => {
      debug!("Attempt to load config: {}", config_file.to_str().unwrap());
      match fs::read(&config_file) {
        Ok(config_data) => Some(config_data.to_owned()),
        _ => panic!(
          "Error retrieving compiler config file at \"{}\"",
          config_file.to_str().unwrap()
        ),
      }
    }
    _ => None,
  };

  (config_path, config)
}

pub struct TsCompiler {
  pub deno_dir: DenoDir,
  pub config: CompilerConfig,
  pub config_hash: Vec<u8>,
}

impl TsCompiler {
  pub fn new(deno_dir: DenoDir, config_path: Option<String>) -> Self {
    let compiler_config = match load_config_file(config_path) {
      (Some(config_path), Some(config)) => {
        Some((config_path.to_string(), config.to_vec()))
      }
      _ => None,
    };

    let config_bytes = match &compiler_config {
      Some((_, config)) => config.clone(),
      _ => b"".to_vec(),
    };

    Self {
      deno_dir,
      config: compiler_config,
      config_hash: config_bytes,
    }
  }

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

  pub fn compile_async(
    self: &Self,
    state: ThreadSafeState,
    source_file: &SourceFile,
    use_cache: bool,
  ) -> impl Future<Item = SourceFile, Error = ErrBox> {
    // TODO: maybe fetching of original SourceFile should be done here?

    if source_file.media_type != msg::MediaType::TypeScript {
      return Either::A(futures::future::ok(source_file.clone()));
    }

    if use_cache {
      // Try to load cached version:
      // 1. check if there's 'meta' file
      let (_, _, meta_cache_key) = self.cache_paths(&source_file.url);
      let meta_filename = self.deno_dir.gen.join(meta_cache_key);
      if let Some(read_metadata) = CompiledFileMetadata::load(meta_filename) {
        // 2. compare version hashes
        // TODO: it would probably be good idea to make it method implemented on SourceFile
        let version_hash_to_validate = source_code_version_hash(
          &source_file.source_code,
          version::DENO,
          &self.config_hash,
        );

        if read_metadata.version_hash == version_hash_to_validate {
          debug!("load_cache metadata version hash match");
          if let Ok(compiled_module) =
            self.get_compiled_source_file(&source_file)
          {
            debug!(
              "found cached compiled module: {:?}",
              compiled_module.clone().filename
            );
            // TODO: store in in-process cache for subsequent access
            return Either::A(futures::future::ok(compiled_module));
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
        // TODO: can this be somehow called using `self.`?
        state_
          .ts_compiler
          .get_compiled_source_file(&source_file_)
          .map_err(|e| {
            // TODO: this situation shouldn't happen
            panic!("Expected to find compiled file: {}", e)
          })
      }).and_then(move |source_file_after_compile| {
        // Explicit drop to keep reference alive until future completes.
        drop(compiling_job);

        Ok(source_file_after_compile)
      }).then(move |r| {
        debug!(">>>>> compile_sync END");
        // TODO(ry) do this in worker's destructor.
        // resource.close();
        r
      });

    Either::B(fut)
  }

  pub fn cache_paths(self: &Self, url: &Url) -> (PathBuf, PathBuf, PathBuf) {
    let compiled_cache_filename = get_cache_filename(url);
    (
      compiled_cache_filename.with_extension("js"),
      compiled_cache_filename.with_extension("js.map"),
      compiled_cache_filename.with_extension("meta"),
    )
  }

  // TODO: this should be done by some higher level function from `DiskCache` or DenoDir
  pub fn get_compiled_source_file(
    self: &Self,
    source_file: &SourceFile,
  ) -> Result<SourceFile, ErrBox> {
    let compiled_cache_filename =
      self.deno_dir.gen.join(get_cache_filename(&source_file.url));
    let compiled_code_filename = compiled_cache_filename.with_extension("js");
    debug!("compiled filename: {:?}", compiled_code_filename);

    let compiled_code = fs::read(&compiled_code_filename)?;

    let compiled_module = SourceFile {
      url: source_file.url.clone(),
      redirect_source_url: None,
      filename: compiled_code_filename,
      media_type: msg::MediaType::JavaScript,
      source_code: compiled_code,
      maybe_source_map: None, // TODO: this breaks deno info
      maybe_source_map_filename: None,
    };

    Ok(compiled_module)
  }

  // TODO: this should be done by some higher level function from `DiskCache` or DenoDir
  pub fn get_source_map_file(
    self: &Self,
    module_specifier: &ModuleSpecifier,
  ) -> Result<SourceFile, ErrBox> {
    let compiled_cache_filename = self
      .deno_dir
      .gen
      .join(get_cache_filename(module_specifier.as_url()));
    let source_map_filename = compiled_cache_filename.with_extension("js.map");
    debug!("source map filename: {:?}", source_map_filename);

    let source_map_url = Url::from_file_path(&source_map_filename)
      .expect("Path must be valid URL");
    let source_code = fs::read(&source_map_filename)?;

    let compiled_module = SourceFile {
      url: source_map_url,
      redirect_source_url: None,
      filename: source_map_filename,
      media_type: msg::MediaType::JavaScript,
      source_code,
      maybe_source_map_filename: None,
      maybe_source_map: None,
    };

    Ok(compiled_module)
  }

  pub fn cache_compiler_output(
    self: &Self,
    module_specifier: &ModuleSpecifier,
    extension: &str,
    contents: &str,
  ) -> std::io::Result<()> {
    let (js_cache_path, source_map_path, meta_data_path) =
      self.cache_paths(module_specifier.as_url());
    let (js_cache_path, source_map_path, meta_data_path) = (
      self.deno_dir.gen.join(js_cache_path),
      self.deno_dir.gen.join(source_map_path),
      self.deno_dir.gen.join(meta_data_path),
    );

    // TODO: factor out to Enum
    match extension {
      ".map" => {
        match source_map_path.parent() {
          Some(ref parent) => fs::create_dir_all(parent),
          None => unreachable!(),
        }?;
        fs::write(source_map_path, contents)?;
      }
      ".js" => {
        let source_file = self
          .deno_dir
          .fetch_source_file(&module_specifier, true, true)
          .expect("Source file not found");

        match js_cache_path.parent() {
          Some(ref parent) => fs::create_dir_all(parent),
          None => unreachable!(),
        }?;
        fs::write(js_cache_path, contents)?;

        // save .meta file
        let version_hash = source_code_version_hash(
          &source_file.source_code,
          version::DENO,
          &self.config_hash,
        );
        let compiled_file_metadata = CompiledFileMetadata {
          source_path: source_file.filename.to_str().unwrap().to_string(),
          version_hash,
        };
        compiled_file_metadata.save(&meta_data_path);
      }
      _ => unreachable!(),
    }

    Ok(())
  }

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
      return match self.deno_dir.fetch_source_file(
        &module_specifier,
        true,
        true,
      ) {
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

impl SourceMapGetter for TsCompiler {
  fn get_source_map(&self, script_name: &str) -> Option<Vec<u8>> {
    self
      .try_to_resolve_and_get_source_map(script_name)
      .and_then(|out| Some(out.source_code))
  }

  fn get_source_line(&self, script_name: &str, line: usize) -> Option<String> {
    // TODO: maybe it's worth caching vector of lines after first access
    // because this function might be called several times for each file
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
      use_cache: bool,
    ) -> Result<SourceFile, ErrBox> {
      tokio_util::block_on(self.compile_async(state, source_file, use_cache))
    }
  }

  #[test]
  fn test_compile_sync() {
    tokio_util::init(|| {
      let specifier =
        ModuleSpecifier::resolve_url_or_path("./tests/002_hello.ts").unwrap();

      let mut out = SourceFile {
        url: specifier.as_url().clone(),
        redirect_source_url: None,
        filename: PathBuf::from("/tests/002_hello.ts"),
        media_type: msg::MediaType::TypeScript,
        source_code: include_bytes!("../tests/002_hello.ts").to_vec(),
        maybe_source_map_filename: None,
        maybe_source_map: None,
      };

      let mock_state = ThreadSafeState::mock(vec![
        String::from("./deno"),
        String::from("hello.js"),
      ]);
      out = mock_state
        .ts_compiler
        .compile_sync(mock_state.clone(), &out, false)
        .unwrap();
      assert!(
        out
          .source_code
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
