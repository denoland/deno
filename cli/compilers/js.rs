// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use crate::compilers::CompiledModule;
use crate::compilers::CompiledModuleFuture;
use crate::file_fetcher::SourceFile;
use std::str;

pub struct JsCompiler {}

impl JsCompiler {
  pub fn compile_async(
    self: &Self,
    source_file: &SourceFile,
  ) -> Box<CompiledModuleFuture> {
    let module = CompiledModule {
      code: str::from_utf8(&source_file.source_code)
        .unwrap()
        .to_string(),
      name: source_file.url.to_string(),
    };

    Box::new(futures::future::ok(module))
  }
}
