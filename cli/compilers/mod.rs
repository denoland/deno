// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use deno::ErrBox;
use futures::Future;

mod js;
mod json;
mod ts;

pub use js::JsCompiler;
pub use json::JsonCompiler;
pub use ts::TsCompiler;

#[derive(Debug, Clone)]
pub struct CompiledModule {
  pub code: String,
  pub name: String,
}

pub type CompiledModuleFuture =
  dyn Future<Item = CompiledModule, Error = ErrBox> + Send;
