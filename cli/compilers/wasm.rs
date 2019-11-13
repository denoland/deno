// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use crate::compilers::CompiledModule;
use crate::compilers::CompiledModuleFuture;
use crate::file_fetcher::SourceFile;

// TODO(kevinkassimo): This is a hack to encode/decode data as base64 string.
// (Since Deno namespace might not be available, Deno.read can fail).
// Binary data is already available through source_file.source_code.
// If this is proven too wasteful in practice, refactor this.

// Ref: https://webassembly.github.io/esm-integration/js-api/index.html#esm-integration

// Only default exports is support ATM.
// Node.js supports named import since its dynamic module creation allows
// running some code before transformation:
// https://github.com/nodejs/node/blob/35ec01097b2a397ad0a22aac536fe07514876e21/lib/internal/modules/esm/translators.js#L190-L210
// We need to expose worker to compilers to achieve that.

pub struct WasmCompiler {}

impl WasmCompiler {
  pub fn compile_async(
    self: &Self,
    source_file: &SourceFile,
  ) -> Box<CompiledModuleFuture> {
    let code = wrap_wasm_code(&source_file.source_code);
    let module = CompiledModule {
      code,
      name: source_file.url.to_string(),
    };
    Box::new(futures::future::ok(module))
  }
}

pub fn wrap_wasm_code<T: ?Sized + AsRef<[u8]>>(input: &T) -> String {
  format!(include_str!("./wasm_wrap.js"), base64::encode(input))
}
