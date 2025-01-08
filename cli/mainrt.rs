// Copyright 2018-2025 the Deno authors. MIT license.

// Allow unused code warnings because we share
// code between the two bin targets.
#![allow(dead_code)]
#![allow(unused_imports)]

mod standalone;

mod args;
mod cache;
mod emit;
mod file_fetcher;
mod http_util;
mod js;
mod node;
mod npm;
mod resolver;
mod shared;
mod sys;
mod task_runner;
mod util;
mod version;
mod worker;

use std::borrow::Cow;
use std::collections::HashMap;
use std::env;
use std::env::current_exe;
use std::sync::Arc;

use deno_core::error::AnyError;
use deno_core::error::CoreError;
use deno_core::error::JsError;
use deno_runtime::fmt_errors::format_js_error;
use deno_runtime::tokio_util::create_and_run_current_thread_with_maybe_metrics;
pub use deno_runtime::UNSTABLE_GRANULAR_FLAGS;
use deno_terminal::colors;
use indexmap::IndexMap;
use standalone::DenoCompileFileSystem;

use crate::args::Flags;
use crate::util::result::any_and_jserrorbox_downcast_ref;

pub(crate) fn unstable_exit_cb(feature: &str, api_name: &str) {
  log::error!(
    "Unstable API '{api_name}'. The `--unstable-{}` flag must be provided.",
    feature
  );
  deno_runtime::exit(70);
}

fn exit_with_message(message: &str, code: i32) -> ! {
  log::error!(
    "{}: {}",
    colors::red_bold("error"),
    message.trim_start_matches("error: ")
  );
  deno_runtime::exit(code);
}

fn unwrap_or_exit<T>(result: Result<T, AnyError>) -> T {
  match result {
    Ok(value) => value,
    Err(error) => {
      let mut error_string = format!("{:?}", error);

      if let Some(CoreError::Js(js_error)) =
        any_and_jserrorbox_downcast_ref::<CoreError>(&error)
      {
        error_string = format_js_error(js_error);
      }

      exit_with_message(&error_string, 1);
    }
  }
}

fn load_env_vars(env_vars: &IndexMap<String, String>) {
  env_vars.iter().for_each(|env_var| {
    if env::var(env_var.0).is_err() {
      std::env::set_var(env_var.0, env_var.1);
    }
  })
}

fn main() {
  deno_runtime::deno_permissions::mark_standalone();
  let args: Vec<_> = env::args_os().collect();
  let standalone = standalone::extract_standalone(Cow::Owned(args));
  let future = async move {
    match standalone {
      Ok(Some(data)) => {
        deno_telemetry::init(
          crate::args::otel_runtime_config(),
          &data.metadata.otel_config,
        )?;
        util::logger::init(
          data.metadata.log_level,
          Some(data.metadata.otel_config.clone()),
        );
        load_env_vars(&data.metadata.env_vars_from_env_file);
        let fs = DenoCompileFileSystem::new(data.vfs.clone());
        let sys = crate::sys::CliSys::DenoCompile(fs.clone());
        let exit_code = standalone::run(Arc::new(fs), sys, data).await?;
        deno_runtime::exit(exit_code);
      }
      Ok(None) => Ok(()),
      Err(err) => {
        util::logger::init(None, None);
        Err(err)
      }
    }
  };

  unwrap_or_exit(create_and_run_current_thread_with_maybe_metrics(future));
}
