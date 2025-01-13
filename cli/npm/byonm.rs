// Copyright 2018-2025 the Deno authors. MIT license.

use std::sync::Arc;

use deno_core::serde_json;
use deno_resolver::npm::ByonmNpmResolver;
use deno_resolver::npm::ByonmNpmResolverCreateOptions;
use deno_runtime::ops::process::NpmProcessStateProvider;

use crate::args::NpmProcessState;
use crate::args::NpmProcessStateKind;
use crate::sys::CliSys;

pub type CliByonmNpmResolverCreateOptions =
  ByonmNpmResolverCreateOptions<CliSys>;
pub type CliByonmNpmResolver = ByonmNpmResolver<CliSys>;

#[derive(Debug)]
pub struct CliByonmNpmProcessStateProvider(pub Arc<CliByonmNpmResolver>);

impl NpmProcessStateProvider for CliByonmNpmProcessStateProvider {
  fn get_npm_process_state(&self) -> String {
    serde_json::to_string(&NpmProcessState {
      kind: NpmProcessStateKind::Byonm,
      local_node_modules_path: self
        .0
        .root_node_modules_path()
        .map(|p| p.to_string_lossy().to_string()),
    })
    .unwrap()
  }
}
