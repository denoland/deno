// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate log;
#[macro_use]
extern crate futures;
#[macro_use]
extern crate serde_json;

mod ansi;
pub mod cli;
pub mod compiler;
pub mod deno_dir;
pub mod errors;
pub mod flags;
mod fs;
mod global_timer;
mod http_body;
mod http_util;
pub mod isolate;
pub mod isolate_init;
pub mod isolate_state;
pub mod js_errors;
pub mod modules;
pub mod msg;
pub mod msg_util;
pub mod ops;
pub mod permissions;
mod repl;
pub mod resolve_addr;
pub mod resources;
mod tokio_util;
mod tokio_write;
pub mod version;
pub mod workers;

use crate::cli::Cli;
use crate::errors::RustOrJsError;
use crate::isolate::Isolate;
use crate::isolate_state::IsolateState;
use futures::lazy;
use futures::Future;
use log::{LevelFilter, Metadata, Record};
use std::env;
use std::sync::Arc;

static LOGGER: Logger = Logger;

struct Logger;

impl log::Log for Logger {
  fn enabled(&self, metadata: &Metadata) -> bool {
    metadata.level() <= log::max_level()
  }

  fn log(&self, record: &Record) {
    if self.enabled(record.metadata()) {
      println!("{} RS - {}", record.level(), record.args());
    }
  }
  fn flush(&self) {}
}

fn print_err_and_exit(err: RustOrJsError) {
  eprintln!("{}", err.to_string());
  std::process::exit(1);
}

fn js_check<E>(r: Result<(), E>)
where
  E: Into<RustOrJsError>,
{
  if let Err(err) = r {
    print_err_and_exit(err.into());
  }
}

fn main() {
  #[cfg(windows)]
  ansi_term::enable_ansi_support().ok(); // For Windows 10

  log::set_logger(&LOGGER).unwrap();
  let args = env::args().collect();
  let (mut flags, mut rest_argv, usage_string) = flags::set_flags(args)
    .unwrap_or_else(|err| {
      eprintln!("{}", err);
      std::process::exit(1)
    });

  if flags.help {
    println!("{}", &usage_string);
    std::process::exit(0);
  }

  log::set_max_level(if flags.log_debug {
    LevelFilter::Debug
  } else {
    LevelFilter::Warn
  });

  if flags.fmt {
    rest_argv.insert(1, "https://deno.land/std/prettier/main.ts".to_string());
    flags.allow_read = true;
    flags.allow_write = true;
  }

  let should_prefetch = flags.prefetch || flags.info;
  let should_display_info = flags.info;

  let state = Arc::new(IsolateState::new(flags, rest_argv, None));
  let state_ = state.clone();
  let isolate_init = isolate_init::deno_isolate_init();
  let permissions = permissions::DenoPermissions::from_flags(&state.flags);
  let cli = Cli::new(isolate_init, state_, permissions);
  let mut isolate = Isolate::new(cli);

  let main_future = lazy(move || {
    // Setup runtime.
    js_check(isolate.execute("denoMain()"));

    // Execute main module.
    if let Some(main_module) = state.main_module() {
      debug!("main_module {}", main_module);
      js_check(isolate.execute_mod(&main_module, should_prefetch));
      if should_display_info {
        // Display file info and exit. Do not run file
        isolate.print_file_info(&main_module);
        std::process::exit(0);
      }
    }

    isolate.then(|result| {
      js_check(result);
      Ok(())
    })
  });

  tokio_util::run(main_future);
}
