// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use super::compiler_worker::CompilerWorker;
use crate::compilers::CompiledModule;
use crate::compilers::CompiledModuleFuture;
use crate::file_fetcher::SourceFile;
use crate::global_state::ThreadSafeGlobalState;
use crate::startup_data;
use crate::state::*;
use deno_core::ErrBox;
use deno_core::ModuleSpecifier;
use futures::FutureExt;
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
  fn setup_worker(global_state: ThreadSafeGlobalState) -> CompilerWorker {
    let (int, ext) = ThreadSafeState::create_channels();
    let entry_point =
      ModuleSpecifier::resolve_url_or_path("./__$deno$wasm_compiler.ts")
        .unwrap();
    let worker_state =
      ThreadSafeState::new(global_state.clone(), None, entry_point, int)
        .expect("Unable to create worker state");

    // Count how many times we start the compiler worker.
    global_state
      .metrics
      .compiler_starts
      .fetch_add(1, Ordering::SeqCst);

    let mut worker = CompilerWorker::new(
      "WASM".to_string(),
      startup_data::compiler_isolate_init(),
      worker_state,
      ext,
    );
    worker.execute("bootstrapWasmCompilerRuntime()").unwrap();
    worker
  }

  pub fn compile_async(
    &self,
    global_state: ThreadSafeGlobalState,
    source_file: &SourceFile,
  ) -> Pin<Box<CompiledModuleFuture>> {
    let cache = self.cache.clone();
    let source_file = source_file.clone();
    let maybe_cached = { cache.lock().unwrap().get(&source_file.url).cloned() };
    if let Some(m) = maybe_cached {
      return futures::future::ok(m).boxed();
    }
    let cache_ = self.cache.clone();

    let (load_sender, load_receiver) =
      tokio::sync::oneshot::channel::<Result<CompiledModule, ErrBox>>();

    std::thread::spawn(move || {
      debug!(">>>>> wasm_compile_async START");
      let base64_data = base64::encode(&source_file.source_code);
      let mut worker = WasmCompiler::setup_worker(global_state);
      let handle = worker.thread_safe_handle();
      let url = source_file.url.clone();

      let fut = async move {
        let _ = handle
          .post_message(
            serde_json::to_string(&base64_data)
              .unwrap()
              .into_boxed_str()
              .into_boxed_bytes(),
          )
          .await;

        if let Err(err) = (&mut *worker).await {
          load_sender.send(Err(err)).unwrap();
          return;
        }

        debug!("Sent message to worker");
        let json_msg = handle.get_message().await.expect("not handled");

        debug!("Received message from worker");
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
        load_sender.send(Ok(module)).unwrap();
      };

      crate::tokio_util::run_basic(fut);
    });
    let fut = async { load_receiver.await.unwrap() };
    fut.boxed_local()
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
