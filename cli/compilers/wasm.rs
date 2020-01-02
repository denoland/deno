// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use crate::compilers::CompiledModule;
use crate::compilers::CompiledModuleFuture;
use crate::file_fetcher::SourceFile;
use crate::global_state::ThreadSafeGlobalState;
use crate::startup_data;
use crate::state::*;
use crate::worker::Worker;
use deno::Buf;
use futures::FutureExt;
use futures::TryFutureExt;
use serde_derive::Deserialize;
use serde_json;
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use url::Url;

// TODO(kevinkassimo): This is a hack to encode/decode data as base64 string.
// (Since Deno namespace might not be available, Deno.read can fail).
// Binary data is already available through source_file.source_code.
// If this is proven too wasteful in practice, refactor this.

// Ref: https://webassembly.github.io/esm-integration/js-api/index.html#esm-integration
// https://github.com/nodejs/node/blob/35ec01097b2a397ad0a22aac536fe07514876e21/lib/internal/modules/esm/translators.js#L190-L210

// Dynamically construct JS wrapper with custom static imports and named exports.
// Boots up an internal worker to resolve imports/exports through query from V8.

static WASM_WRAP: &str = include_str!("./wasm_wrap.js");

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct WasmModuleInfo {
  import_list: Vec<String>,
  export_list: Vec<String>,
}

#[derive(Default)]
pub struct WasmCompiler {
  cache: Arc<Mutex<HashMap<Url, CompiledModule>>>,
}

impl WasmCompiler {
  /// Create a new V8 worker with snapshot of WASM compiler and setup compiler's runtime.
  fn setup_worker(global_state: ThreadSafeGlobalState) -> Worker {
    let (int, ext) = ThreadSafeState::create_channels();
    let worker_state =
      ThreadSafeState::new(global_state.clone(), None, None, true, int)
        .expect("Unable to create worker state");

    // Count how many times we start the compiler worker.
    global_state
      .metrics
      .compiler_starts
      .fetch_add(1, Ordering::SeqCst);

    let mut worker = Worker::new(
      "WASM".to_string(),
      startup_data::compiler_isolate_init(),
      worker_state,
      ext,
    );
    worker.execute("denoMain('WASM')").unwrap();
    worker.execute("workerMain()").unwrap();
    worker.execute("wasmCompilerMain()").unwrap();
    worker
  }

  pub fn compile_async(
    self: &Self,
    global_state: ThreadSafeGlobalState,
    source_file: &SourceFile,
  ) -> Pin<Box<CompiledModuleFuture>> {
    let cache = self.cache.clone();
    let maybe_cached = { cache.lock().unwrap().get(&source_file.url).cloned() };
    if let Some(m) = maybe_cached {
      return futures::future::ok(m).boxed();
    }
    let cache_ = self.cache.clone();

    debug!(">>>>> wasm_compile_async START");
    let base64_data = base64::encode(&source_file.source_code);
    let worker = WasmCompiler::setup_worker(global_state);
    let worker_ = worker.clone();
    let url = source_file.url.clone();

    let fut = worker
      .post_message(
        serde_json::to_string(&base64_data)
          .unwrap()
          .into_boxed_str()
          .into_boxed_bytes(),
      )
      .then(|_| worker)
      .then(move |result| {
        if let Err(err) = result {
          // TODO(ry) Need to forward the error instead of exiting.
          eprintln!("{}", err.to_string());
          std::process::exit(1);
        }
        debug!("Sent message to worker");
        worker_.get_message()
      })
      .map_err(|_| panic!("not handled"))
      .and_then(move |maybe_msg: Option<Buf>| {
        debug!("Received message from worker");
        let json_msg = maybe_msg.unwrap();
        let module_info: WasmModuleInfo =
          serde_json::from_slice(&json_msg).unwrap();
        debug!("WASM module info: {:#?}", &module_info);
        let code = wrap_wasm_code(
          &base64_data,
          &module_info.import_list,
          &module_info.export_list,
        );
        debug!("Generated code: {}", &code);
        let module = CompiledModule {
          code,
          name: url.to_string(),
        };
        {
          cache_.lock().unwrap().insert(url.clone(), module.clone());
        }
        debug!("<<<<< wasm_compile_async END");
        futures::future::ok(module)
      });
    fut.boxed()
  }
}

fn build_single_import(index: usize, origin: &str) -> String {
  let origin_json = serde_json::to_string(origin).unwrap();
  format!(
    r#"import * as m{} from {};
importObject[{}] = m{};
"#,
    index, &origin_json, &origin_json, index
  )
}

fn build_imports(imports: &[String]) -> String {
  let mut code = String::from("");
  for (index, origin) in imports.iter().enumerate() {
    code.push_str(&build_single_import(index, origin));
  }
  code
}

fn build_single_export(name: &str) -> String {
  format!("export const {} = instance.exports.{};\n", name, name)
}

fn build_exports(exports: &[String]) -> String {
  let mut code = String::from("");
  for e in exports {
    code.push_str(&build_single_export(e));
  }
  code
}

fn wrap_wasm_code(
  base64_data: &str,
  imports: &[String],
  exports: &[String],
) -> String {
  let imports_code = build_imports(imports);
  let exports_code = build_exports(exports);
  String::from(WASM_WRAP)
    .replace("//IMPORTS\n", &imports_code)
    .replace("//EXPORTS\n", &exports_code)
    .replace("BASE64_DATA", base64_data)
}
