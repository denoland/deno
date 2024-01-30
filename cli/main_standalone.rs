// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
mod standalone;

mod args;
mod auth_tokens;
mod cache;
mod emit;
mod errors;
mod file_fetcher;
mod http_util;
mod js;
mod node;
mod npm;
mod resolver;
mod util;
mod version;
mod worker;

use crate::args::Flags;
use deno_runtime::colors;
pub use deno_runtime::UNSTABLE_GRANULAR_FLAGS;

pub(crate) fn unstable_exit_cb(feature: &str, api_name: &str) {
  eprintln!(
    "Unstable API '{api_name}'. The `--unstable-{}` flag must be provided.",
    feature
  );
  std::process::exit(70);
}

use deno_runtime::tokio_util::create_and_run_current_thread_with_maybe_metrics;
use std::env;
use std::env::current_exe;

fn main() {
  let args: Vec<String> = env::args().collect();
  let future = async move {
    let current_exe_path = current_exe().unwrap();
    let standalone_res =
      match standalone::extract_standalone(&current_exe_path, args).await {
        Ok(Some((metadata, eszip))) => standalone::run(eszip, metadata).await,
        Ok(None) => Ok(()),
        Err(err) => Err(err),
      };
  };

  create_and_run_current_thread_with_maybe_metrics(future);
}
