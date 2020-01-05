// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use deno_core::ErrBox;
use futures::Future;

mod js;
mod json;
mod ts;
mod wasm;

pub use js::JsCompiler;
pub use json::JsonCompiler;
pub use ts::TsCompiler;
pub use wasm::WasmCompiler;

#[derive(Debug, Clone)]
pub struct CompiledModule {
  pub code: String,
  pub name: String,
}

pub type CompiledModuleFuture =
  dyn Future<Output = Result<CompiledModule, ErrBox>> + Send;
