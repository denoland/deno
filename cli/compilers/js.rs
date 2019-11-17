// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use crate::compilers::CompiledModule;
use crate::compilers::CompiledModuleFuture;
use crate::file_fetcher::SourceFile;
use crate::futures::future::FutureExt;
use std::pin::Pin;
use std::str;

pub struct JsCompiler {}

impl JsCompiler {
  pub fn compile_async(
    self: &Self,
    source_file: &SourceFile,
  ) -> Pin<Box<CompiledModuleFuture>> {
    let module = CompiledModule {
      code: str::from_utf8(&source_file.source_code)
        .unwrap()
        .to_string(),
      name: source_file.url.to_string(),
    };

    futures::future::ok(module).boxed()
  }
}
