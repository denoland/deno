// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use crate::compilers::CompiledModule;
use crate::compilers::CompiledModuleFuture;
use crate::file_fetcher::SourceFile;
use crate::state::ThreadSafeState;
use std::str;

pub struct JsonCompiler {}

impl JsonCompiler {
  pub fn compile_async(
    self: &Self,
    _state: ThreadSafeState,
    source_file: &SourceFile,
  ) -> Box<CompiledModuleFuture> {
    let module = CompiledModule {
      code: format!(
        "export default {};",
        str::from_utf8(&source_file.source_code).unwrap()
      ),
      name: source_file.url.to_string(),
    };

    Box::new(futures::future::ok(module))
  }
}
