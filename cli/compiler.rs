// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use crate::deno_dir::DenoDir;
use crate::deno_dir::ModuleMetaData;
use crate::diagnostics::Diagnostic;
use crate::msg;
use crate::resources;
use crate::startup_data;
use crate::state::*;
use crate::worker::Worker;
use deno::Buf;
use deno::ErrBox;
use futures::future::Either;
use futures::Future;
use futures::Stream;
use std::str;
use std::sync::atomic::Ordering;

/// Optional tuple which represents the state of the compiler
/// configuration where the first is canonical name for the configuration file
/// and a vector of the bytes of the contents of the configuration file.
type CompilerConfig = Option<(String, Vec<u8>)>;

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

pub struct TsCompiler {
  pub deno_dir: DenoDir,
  pub config: CompilerConfig,
}

impl TsCompiler {
  // TODO: reading of config file should be done in TsCompiler::new instead of state, refactor
  // DenoDir to not require compiler config in initializer
  pub fn new(
    deno_dir: DenoDir,
    config_path: Option<String>,
    config: Option<Vec<u8>>,
  ) -> Self {
    let compiler_config = match (&config_path, &config) {
      (Some(config_path), Some(config)) => {
        Some((config_path.to_string(), config.to_vec()))
      }
      _ => None,
    };

    Self {
      deno_dir,
      config: compiler_config,
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
    module_meta_data: &ModuleMetaData,
    use_cache: bool,
  ) -> impl Future<Item = ModuleMetaData, Error = ErrBox> {
    if module_meta_data.media_type != msg::MediaType::TypeScript {
      return Either::A(futures::future::ok(module_meta_data.clone()));
    }

    if use_cache {
      // try to load cached version
      match self
        .deno_dir
        .get_compiled_module_meta_data(&module_meta_data)
      {
        Ok(compiled_module) => {
          debug!(
            "found cached compiled module: {:?}",
            compiled_module.clone().filename
          );
          return Either::A(futures::future::ok(compiled_module));
        }
        Err(_) => {}
      }
    }

    let module_meta_data_ = module_meta_data.clone();

    debug!(">>>>> compile_sync START");
    let module_url = module_meta_data.url.clone();

    debug!(
      "Running rust part of compile_sync, module specifier: {}",
      &module_meta_data.url
    );

    let root_names = vec![module_url.to_string()];
    let req_msg = req(root_names, self.config.clone(), None);

    let worker = TsCompiler::setup_worker(state.clone());
    let compiling_job = state.progress.add("Compile", &module_url.to_string());
    let deno_dir_ = self.deno_dir.clone();

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
        // TODO: must fetch compiled JS file from DENO_DIR, this can't file
        //  but if we get rid of gen/ and store JS files alongside TS files
        //  when we request local JS file it is fetched from original location on disk
        //  example:
        //  compiling file:///dev/foo.ts
        //  it's fetched from /dev/foo.ts
        //  we want to fetch $DENO_DIR/src/file/dev/foo.js
        //  so it would be natural to request equivalent JS file (file://dev/foo.js),
        //  but it's local file so it will be fetched from /dev/foo.js (NotFound!)
        //
        //  simple solution:
        //  always get files from $DENO_DIR
        //
        //
        //  compiling file:///dev/foo.ts
        //  looking for already compiled file file:///dev/foo.js
        //  looking at $DENO_DIR/src/file/dev/foo.js (NotFound!)
        //  fallback to /dev/foo.js (NotFound!)
        //  create worker and compile file:///dev/foo.ts
        //  looking at $DENO_DIR/src/file/dev/foo.ts (NotFound)
        //  fetch from /dev/foo.ts, cache in $DENO_DIR along with headers with modified timestamp
        //  to avoid copying if not needed
        //  worker writes compiled file to disk at $DENO_DIR/src/file/dev/foo.js
        //  looking for already compiled file file:///dev/foo.js
        //  looking at $DENO_DIR/src/file/dev/foo.ts (found, return file)
        //
        // This solution wouldn't play nicely with CheckJS when there's JS->JS compilation. Because
        // first we'd access the file from original location on disk, on subsequent access we'd
        // have to force skipping in-process cache to re-fetch from disk and only then we'd
        // get compiled module.
        //
        // We
        // still need to handle that JS and JSON files are compiled by TS compiler now (and they're
        // not used).
        deno_dir_.get_compiled_module_meta_data(&module_meta_data_)
          .map_err(|e| {
            // TODO(95th) Instead of panicking, We could translate this error to Diagnostic.
            panic!("{}", e)
          })
      }).and_then(move |module_meta_data_after_compile| {
        // Explicit drop to keep reference alive until future completes.
        drop(compiling_job);

        Ok(module_meta_data_after_compile)
      }).then(move |r| {
        debug!(">>>>> compile_sync END");
        // TODO(ry) do this in worker's destructor.
        // resource.close();
        r
      });

    Either::B(fut)
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
      module_meta_data: &ModuleMetaData,
      use_cache: bool,
    ) -> Result<ModuleMetaData, ErrBox> {
      tokio_util::block_on(self.compile_async(
        state,
        module_meta_data,
        use_cache,
      ))
    }
  }

  #[test]
  fn test_compile_sync() {
    tokio_util::init(|| {
      let specifier =
        ModuleSpecifier::resolve_url_or_path("./tests/002_hello.ts").unwrap();

      let mut out = ModuleMetaData {
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
}
