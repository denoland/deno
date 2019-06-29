// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use crate::deno_error::err_check;
use crate::deno_error::DenoError;
use crate::diagnostics::Diagnostic;
use crate::msg;
use crate::resources;
use crate::startup_data;
use crate::state::*;
use crate::tokio_util;
use crate::worker::Worker;
use deno::Buf;
use futures::Future;
use futures::Stream;
use std::path::PathBuf;
use std::str;
use std::sync::atomic::Ordering;

// This corresponds to JS ModuleMetaData.
// TODO Rename one or the other so they correspond.
#[derive(Debug, Clone)]
pub struct ModuleMetaData {
  pub module_name: String,
  pub module_redirect_source_name: Option<String>, // source of redirect
  pub filename: PathBuf,
  pub media_type: msg::MediaType,
  pub source_code: Vec<u8>,
  pub maybe_output_code_filename: Option<PathBuf>,
  pub maybe_output_code: Option<Vec<u8>>,
  pub maybe_source_map_filename: Option<PathBuf>,
  pub maybe_source_map: Option<Vec<u8>>,
}

impl ModuleMetaData {
  pub fn has_output_code_and_source_map(&self) -> bool {
    self.maybe_output_code.is_some() && self.maybe_source_map.is_some()
  }

  pub fn js_source(&self) -> String {
    if self.media_type == msg::MediaType::Json {
      return format!(
        "export default {};",
        str::from_utf8(&self.source_code).unwrap()
      );
    }
    match self.maybe_output_code {
      None => str::from_utf8(&self.source_code).unwrap().to_string(),
      Some(ref output_code) => str::from_utf8(output_code).unwrap().to_string(),
    }
  }
}

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

/// Returns an optional tuple which represents the state of the compiler
/// configuration where the first is canonical name for the configuration file
/// and a vector of the bytes of the contents of the configuration file.
pub fn get_compiler_config(
  parent_state: &ThreadSafeState,
  _compiler_type: &str,
) -> CompilerConfig {
  // The compiler type is being passed to make it easier to implement custom
  // compilers in the future.
  match (&parent_state.config_path, &parent_state.config) {
    (Some(config_path), Some(config)) => {
      Some((config_path.to_string(), config.to_vec()))
    }
    _ => None,
  }
}

pub fn bundle_async(
  state: ThreadSafeState,
  module_name: String,
  out_file: String,
) -> impl Future<Item = (), Error = DenoError> {
  debug!(
    "Invoking the compiler to bundle. module_name: {}",
    module_name
  );

  let root_names = vec![module_name.clone()];
  let compiler_config = get_compiler_config(&state, "typescript");
  let req_msg = req(root_names, compiler_config, Some(out_file));

  // Count how many times we start the compiler worker.
  state.metrics.compiler_starts.fetch_add(1, Ordering::SeqCst);

  let mut worker = Worker::new(
    "TS".to_string(),
    startup_data::compiler_isolate_init(),
    // TODO(ry) Maybe we should use a separate state for the compiler.
    // as was done previously.
    state.clone(),
  );
  err_check(worker.execute("denoMain()"));
  err_check(worker.execute("workerMain()"));
  err_check(worker.execute("compilerMain()"));

  let resource = worker.state.resource.clone();
  let compiler_rid = resource.rid;
  let first_msg_fut = resources::post_message_to_worker(compiler_rid, req_msg)
    .then(move |_| worker)
    .then(move |result| {
      if let Err(err) = result {
        // TODO(ry) Need to forward the error instead of exiting.
        eprintln!("{}", err.to_string());
        std::process::exit(1);
      }
      debug!("Sent message to worker");
      let stream_future =
        resources::get_message_stream_from_worker(compiler_rid).into_future();
      stream_future.map(|(f, _rest)| f).map_err(|(f, _rest)| f)
    });

  first_msg_fut.map_err(|_| panic!("not handled")).and_then(
    move |maybe_msg: Option<Buf>| {
      debug!("Received message from worker");

      if let Some(msg) = maybe_msg {
        let json_str = std::str::from_utf8(&msg).unwrap();
        debug!("Message: {}", json_str);
        if let Some(diagnostics) = Diagnostic::from_emit_result(json_str) {
          return Err(DenoError::from(diagnostics));
        }
      }

      Ok(())
    },
  )
}

pub fn compile_async(
  state: ThreadSafeState,
  module_meta_data: &ModuleMetaData,
) -> impl Future<Item = ModuleMetaData, Error = DenoError> {
  let module_name = module_meta_data.module_name.clone();

  debug!(
    "Running rust part of compile_sync. module_name: {}",
    &module_name
  );

  let root_names = vec![module_name.clone()];
  let compiler_config = get_compiler_config(&state, "typescript");
  let req_msg = req(root_names, compiler_config, None);

  // Count how many times we start the compiler worker.
  state.metrics.compiler_starts.fetch_add(1, Ordering::SeqCst);

  let mut worker = Worker::new(
    "TS".to_string(),
    startup_data::compiler_isolate_init(),
    // TODO(ry) Maybe we should use a separate state for the compiler.
    // as was done previously.
    state.clone(),
  );
  err_check(worker.execute("denoMain()"));
  err_check(worker.execute("workerMain()"));
  err_check(worker.execute("compilerMain()"));

  let compiling_job = state.progress.add("Compile", &module_name);

  let resource = worker.state.resource.clone();
  let compiler_rid = resource.rid;
  let first_msg_fut = resources::post_message_to_worker(compiler_rid, req_msg)
    .then(move |_| worker)
    .then(move |result| {
      if let Err(err) = result {
        // TODO(ry) Need to forward the error instead of exiting.
        eprintln!("{}", err.to_string());
        std::process::exit(1);
      }
      debug!("Sent message to worker");
      let stream_future =
        resources::get_message_stream_from_worker(compiler_rid).into_future();
      stream_future.map(|(f, _rest)| f).map_err(|(f, _rest)| f)
    });

  first_msg_fut
    .map_err(|_| panic!("not handled"))
    .and_then(move |maybe_msg: Option<Buf>| {
      debug!("Received message from worker");

      if let Some(msg) = maybe_msg {
        let json_str = std::str::from_utf8(&msg).unwrap();
        debug!("Message: {}", json_str);
        if let Some(diagnostics) = Diagnostic::from_emit_result(json_str) {
          return Err(DenoError::from(diagnostics));
        }
      }

      Ok(())
    }).and_then(move |_| {
      state.dir.fetch_module_meta_data_async(
        &module_name,
        true,
        true,
      ).map_err(|e| {
        // TODO(95th) Instead of panicking, We could translate this error to Diagnostic.
        panic!("{}", e)
      })
    }).and_then(move |module_meta_data_after_compile| {
      // Explicit drop to keep reference alive until future completes.
      drop(compiling_job);

      Ok(module_meta_data_after_compile)
    }).then(move |r| {
      // TODO(ry) do this in worker's destructor.
      // resource.close();
      r
    })
}

pub fn compile_sync(
  state: ThreadSafeState,
  module_meta_data: &ModuleMetaData,
) -> Result<ModuleMetaData, DenoError> {
  tokio_util::block_on(compile_async(state, module_meta_data))
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_compile_sync() {
    tokio_util::init(|| {
      let specifier = "./tests/002_hello.ts";
      use deno::ModuleSpecifier;
      let module_name = ModuleSpecifier::resolve_root(specifier)
        .unwrap()
        .to_string();

      let mut out = ModuleMetaData {
        module_name,
        module_redirect_source_name: None,
        filename: PathBuf::from("/tests/002_hello.ts"),
        media_type: msg::MediaType::TypeScript,
        source_code: include_bytes!("../tests/002_hello.ts").to_vec(),
        maybe_output_code_filename: None,
        maybe_output_code: None,
        maybe_source_map_filename: None,
        maybe_source_map: None,
      };

      out = compile_sync(
        ThreadSafeState::mock(vec![
          String::from("./deno"),
          String::from("hello.js"),
        ]),
        &out,
      ).unwrap();
      assert!(
        out
          .maybe_output_code
          .unwrap()
          .starts_with("console.log(\"Hello World\");".as_bytes())
      );
    })
  }

  #[test]
  fn test_get_compiler_config_no_flag() {
    let compiler_type = "typescript";
    let state = ThreadSafeState::mock(vec![
      String::from("./deno"),
      String::from("hello.js"),
    ]);
    let out = get_compiler_config(&state, compiler_type);
    assert_eq!(out, None);
  }

  #[test]
  fn test_bundle_async() {
    let specifier = "./tests/002_hello.ts";
    use deno::ModuleSpecifier;
    let module_name = ModuleSpecifier::resolve_root(specifier)
      .unwrap()
      .to_string();

    let state = ThreadSafeState::mock(vec![
      String::from("./deno"),
      String::from("./tests/002_hello.ts"),
      String::from("$deno$/bundle.js"),
    ]);
    let out =
      bundle_async(state, module_name, String::from("$deno$/bundle.js"));
    assert!(tokio_util::block_on(out).is_ok());
  }
}
