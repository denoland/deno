// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use super::compiler_worker::CompilerWorker;
use crate::compilers::CompiledModule;
use crate::file_fetcher::SourceFile;
use crate::global_state::GlobalState;
use crate::startup_data;
use crate::state::*;
use crate::tokio_util;
use crate::worker::WorkerEvent;
use crate::worker::WorkerHandle;
use deno_core::Buf;
use deno_core::ErrBox;
use deno_core::ModuleSpecifier;
use serde_derive::Deserialize;
use serde_json;
use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use url::Url;

// TODO(ry) The entire concept of spawning a thread, sending data to JS,
// compiling WASM there, and moving the data back into the calling thread is
// completelly wrong. V8 has native facilities for getting this information.
// We might be lacking bindings for this currently in rusty_v8 but ultimately
// this "compiler" should be calling into rusty_v8 directly, not spawning
// threads.

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
  fn setup_worker(global_state: GlobalState) -> CompilerWorker {
    let entry_point =
      ModuleSpecifier::resolve_url_or_path("./__$deno$wasm_compiler.ts")
        .unwrap();
    let worker_state = State::new(global_state.clone(), None, entry_point)
      .expect("Unable to create worker state");

    // Count how many times we start the compiler worker.
    global_state.compiler_starts.fetch_add(1, Ordering::SeqCst);

    let mut worker = CompilerWorker::new(
      "WASM".to_string(),
      startup_data::compiler_isolate_init(),
      worker_state,
    );
    worker.execute("bootstrapWasmCompilerRuntime()").unwrap();
    worker
  }

  pub async fn compile(
    &self,
    global_state: GlobalState,
    source_file: &SourceFile,
  ) -> Result<CompiledModule, ErrBox> {
    let cache = self.cache.clone();
    let cache_ = self.cache.clone();
    let source_file = source_file.clone();

    let maybe_cached = { cache.lock().unwrap().get(&source_file.url).cloned() };
    if let Some(m) = maybe_cached {
      return Ok(m);
    }
    debug!(">>>>> wasm_compile START");
    let base64_data = base64::encode(&source_file.source_code);
    let url = source_file.url.clone();
    let req_msg = serde_json::to_string(&base64_data)
      .unwrap()
      .into_boxed_str()
      .into_boxed_bytes();
    let msg = execute_in_thread(global_state.clone(), req_msg).await?;
    debug!("Received message from worker");
    let module_info: WasmModuleInfo = serde_json::from_slice(&msg).unwrap();
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
    debug!("<<<<< wasm_compile END");
    Ok(module)
  }
}

async fn execute_in_thread(
  global_state: GlobalState,
  req: Buf,
) -> Result<Buf, ErrBox> {
  let (handle_sender, handle_receiver) =
    std::sync::mpsc::sync_channel::<Result<WorkerHandle, ErrBox>>(1);
  let builder =
    std::thread::Builder::new().name("deno-wasm-compiler".to_string());
  let join_handle = builder.spawn(move || {
    let worker = WasmCompiler::setup_worker(global_state);
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
