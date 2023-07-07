// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use deno_core::v8;
use deno_core::ModuleSpecifier;
use std::thread;

use crate::colors;

/// The log level to use when printing diagnostic log messages, warnings,
/// or errors in the worker.
///
/// Note: This is disconnected with the log crate's log level and the Rust code
/// in this crate will respect that value instead. To specify that, use
/// `log::set_max_level`.
#[derive(Debug, Default, Clone, Copy)]
pub enum WorkerLogLevel {
  // WARNING: Ensure this is kept in sync with
  // the JS values (search for LogLevel).
  Error = 1,
  Warn = 2,
  #[default]
  Info = 3,
  Debug = 4,
}

impl From<log::Level> for WorkerLogLevel {
  fn from(value: log::Level) -> Self {
    match value {
      log::Level::Error => WorkerLogLevel::Error,
      log::Level::Warn => WorkerLogLevel::Warn,
      log::Level::Info => WorkerLogLevel::Info,
      log::Level::Debug => WorkerLogLevel::Debug,
      log::Level::Trace => WorkerLogLevel::Debug,
    }
  }
}

/// Common bootstrap options for MainWorker & WebWorker
#[derive(Clone)]
pub struct BootstrapOptions {
  /// Sets `Deno.args` in JS runtime.
  pub args: Vec<String>,
  pub cpu_count: usize,
  pub log_level: WorkerLogLevel,
  pub enable_testing_features: bool,
  pub locale: String,
  pub location: Option<ModuleSpecifier>,
  /// Sets `Deno.noColor` in JS runtime.
  pub no_color: bool,
  pub is_tty: bool,
  /// Sets `Deno.version.deno` in JS runtime.
  pub runtime_version: String,
  /// Sets `Deno.version.typescript` in JS runtime.
  pub ts_version: String,
  pub unstable: bool,
  pub user_agent: String,
  pub inspect: bool,
}

impl Default for BootstrapOptions {
  fn default() -> Self {
    let cpu_count = thread::available_parallelism()
      .map(|p| p.get())
      .unwrap_or(1);

    let runtime_version = env!("CARGO_PKG_VERSION").into();
    let user_agent = format!("Deno/{runtime_version}");

    Self {
      runtime_version,
      user_agent,
      cpu_count,
      no_color: !colors::use_color(),
      is_tty: colors::is_tty(),
      enable_testing_features: Default::default(),
      log_level: Default::default(),
      ts_version: Default::default(),
      locale: "en".to_string(),
      location: Default::default(),
      unstable: Default::default(),
      inspect: Default::default(),
      args: Default::default(),
    }
  }
}

impl BootstrapOptions {
  pub fn as_v8<'s>(
    &self,
    scope: &mut v8::HandleScope<'s>,
  ) -> v8::Local<'s, v8::Array> {
    let array = v8::Array::new(scope, 16);

    {
      let args = v8::Array::new(scope, self.args.len() as i32);
      for (idx, arg) in self.args.iter().enumerate() {
        let arg_str = v8::String::new(scope, arg).unwrap();
        args.set_index(scope, idx as u32, arg_str.into());
      }
      array.set_index(scope, 0, args.into());
    }

    {
      let val = v8::Integer::new(scope, self.cpu_count as i32);
      array.set_index(scope, 1, val.into());
    }

    {
      let val = v8::Integer::new(scope, self.log_level as i32);
      array.set_index(scope, 2, val.into());
    }

    {
      let val = v8::String::new_from_one_byte(
        scope,
        self.runtime_version.as_bytes(),
        v8::NewStringType::Internalized,
      )
      .unwrap();
      array.set_index(scope, 3, val.into());
    }

    {
      let val = v8::String::new_from_one_byte(
        scope,
        self.locale.as_bytes(),
        v8::NewStringType::Normal,
      )
      .unwrap();
      array.set_index(scope, 4, val.into());
    }

    {
      let val: v8::Local<v8::Value> = if let Some(location) = &self.location {
        v8::String::new(scope, location.as_str()).unwrap().into()
      } else {
        v8::undefined(scope).into()
      };

      array.set_index(scope, 5, val);
    }

    {
      let val = v8::Boolean::new(scope, self.no_color);
      array.set_index(scope, 6, val.into());
    }

    {
      let val = v8::Boolean::new(scope, self.is_tty);
      array.set_index(scope, 7, val.into());
    }

    {
      let val = v8::String::new_from_one_byte(
        scope,
        self.ts_version.as_bytes(),
        v8::NewStringType::Normal,
      )
      .unwrap();
      array.set_index(scope, 8, val.into());
    }

    {
      let val = v8::Boolean::new(scope, self.unstable);
      array.set_index(scope, 9, val.into());
    }

    {
      let val = v8::Integer::new(scope, std::process::id() as i32);
      array.set_index(scope, 10, val.into());
    }

    {
      let val = v8::String::new_external_onebyte_static(
        scope,
        env!("TARGET").as_bytes(),
      )
      .unwrap();
      array.set_index(scope, 11, val.into());
    }

    {
      let val = v8::String::new_from_one_byte(
        scope,
        deno_core::v8_version().as_bytes(),
        v8::NewStringType::Normal,
      )
      .unwrap();
      array.set_index(scope, 12, val.into());
    }

    {
      let val = v8::String::new_from_one_byte(
        scope,
        self.user_agent.as_bytes(),
        v8::NewStringType::Normal,
      )
      .unwrap();
      array.set_index(scope, 13, val.into());
    }

    {
      let val = v8::Boolean::new(scope, self.inspect);
      array.set_index(scope, 14, val.into());
    }

    {
      let val = v8::Boolean::new(scope, self.enable_testing_features);
      array.set_index(scope, 15, val.into());
    }

    array
  }
}
