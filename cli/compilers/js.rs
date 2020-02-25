// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use crate::compilers::CompiledModule;
use crate::file_fetcher::SourceFile;
use deno_core::ErrBox;
use std::str;

pub struct JsCompiler {}

impl JsCompiler {
  pub async fn compile(
    &self,
    source_file: SourceFile,
  ) -> Result<CompiledModule, ErrBox> {
    Ok(CompiledModule {
      code: str::from_utf8(&source_file.source_code)
        .unwrap()
        .to_string(),
      name: source_file.url.to_string(),
    })
  }
}
