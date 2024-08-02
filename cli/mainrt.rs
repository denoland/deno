// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

// Allow unused code warnings because we share
// code between the two bin targets.
#![allow(dead_code)]
#![allow(unused_imports)]

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
mod task_runner;
mod util;
mod version;
mod worker;

use deno_core::error::generic_error;
use deno_core::error::AnyError;
use deno_core::error::JsError;
use deno_runtime::fmt_errors::format_js_error;
use deno_runtime::tokio_util::create_and_run_current_thread_with_maybe_metrics;
pub use deno_runtime::UNSTABLE_GRANULAR_FLAGS;
use deno_terminal::colors;

use std::borrow::Cow;
use std::collections::HashMap;
use std::env;
use std::env::current_exe;

use crate::args::Flags;

#[allow(clippy::print_stderr)]
pub(crate) fn unstable_exit_cb(feature: &str, api_name: &str) {
  eprintln!(
    "Unstable API '{api_name}'. The `--unstable-{}` flag must be provided.",
    feature
  );
  std::process::exit(70);
}

#[allow(clippy::print_stderr)]
fn exit_with_message(message: &str, code: i32) -> ! {
  eprintln!(
    "{}: {}",
    colors::red_bold("error"),
    message.trim_start_matches("error: ")
  );
  std::process::exit(code);
}

fn unwrap_or_exit<T>(result: Result<T, AnyError>) -> T {
  match result {
    Ok(value) => value,
    Err(error) => {
      let mut error_string = format!("{:?}", error);

      if let Some(e) = error.downcast_ref::<JsError>() {
        error_string = format_js_error(e);
      }

      exit_with_message(&error_string, 1);
    }
  }
}

fn load_env_vars(env_vars: &HashMap<String, String>) {
  env_vars.iter().for_each(|env_var| {
    if env::var(env_var.0).is_err() {
      std::env::set_var(env_var.0, env_var.1);
    }
  })
}

fn main() {
  let args: Vec<_> = env::args_os().collect();
  let standalone = standalone::extract_standalone(Cow::Owned(args));
  let future = async move {
    match standalone {
      Ok(Some(future)) => {
        let (metadata, eszip) = future.await?;
        util::logger::init(metadata.log_level);
        load_env_vars(&metadata.env_vars_from_env_file);
        let exit_code = standalone::run(eszip, metadata).await?;
        std::process::exit(exit_code);
      }
      Ok(None) => Ok(()),
      Err(err) => Err(err),
    }
  };

  unwrap_or_exit(create_and_run_current_thread_with_maybe_metrics(future));
}
