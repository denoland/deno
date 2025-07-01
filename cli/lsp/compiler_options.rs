// Copyright 2018-2025 the Deno authors. MIT license.

use deno_resolver::deno_json::CompilerOptionsResolver;

use crate::lsp::config::Config;
use crate::lsp::resolver::LspResolver;
use crate::sys::CliSys;

#[derive(Debug, Default)]
pub struct LspCompilerOptionsResolver {
  inner: CompilerOptionsResolver,
}

impl LspCompilerOptionsResolver {
  pub fn new(config: &Config, resolver: &LspResolver) -> Self {
    Self {
      inner: CompilerOptionsResolver::new_for_lsp(
        &CliSys::default(),
        config
          .tree
          .data_by_scope()
          .iter()
          .map(|(s, d)| (s.as_ref(), d.member_dir.as_ref()))
          .collect(),
        Box::new(|s| {
          resolver
            .get_scoped_resolver(Some(s))
            .as_node_resolver()
            .map(|r| r.as_ref())
        }),
      ),
    }
  }
}
