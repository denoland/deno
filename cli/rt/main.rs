// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;
use std::env;
use std::sync::Arc;

use deno_core::error::AnyError;
use deno_core::error::CoreError;
use deno_lib::util::result::any_and_jserrorbox_downcast_ref;
use deno_lib::version::otel_runtime_config;
use deno_runtime::deno_telemetry::OtelConfig;
use deno_runtime::fmt_errors::format_js_error;
use deno_runtime::tokio_util::create_and_run_current_thread_with_maybe_metrics;
use deno_terminal::colors;
use indexmap::IndexMap;

use self::binary::extract_standalone;
use self::file_system::DenoRtSys;

mod binary;
mod code_cache;
mod file_system;
mod node;
mod run;

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
  let standalone = extract_standalone(Cow::Owned(args));
  let future = async move {
    match standalone {
      Ok(data) => {
        deno_runtime::deno_telemetry::init(
          otel_runtime_config(),
          data.metadata.otel_config.clone(),
        )?;
        init_logging(
          data.metadata.log_level,
          Some(data.metadata.otel_config.clone()),
        );
        load_env_vars(&data.metadata.env_vars_from_env_file);
        let sys = DenoRtSys::new(data.vfs.clone());
        let exit_code = run::run(Arc::new(sys.clone()), sys, data).await?;
        deno_runtime::exit(exit_code);
      }
      Err(err) => {
        init_logging(None, None);
        Err(err)
      }
    }
  };

  unwrap_or_exit::<()>(create_and_run_current_thread_with_maybe_metrics(
    future,
  ));
}

fn init_logging(
  maybe_level: Option<log::Level>,
  otel_config: Option<OtelConfig>,
) {
  deno_lib::util::logger::init(deno_lib::util::logger::InitLoggingOptions {
    maybe_level,
    otel_config,
    on_log_start: || {},
    on_log_end: || {},
  })
}
