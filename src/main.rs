// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
#![allow(unused_variables)]

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
mod http_body;
mod http_util;
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

#[cfg(unix)]
mod eager_unix;

use crate::cli::Cli;
use crate::isolate_state::IsolateState;
use deno_core::JSError;
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

fn print_err_and_exit(err: errors::RustOrJsError) {
  eprintln!("{}", err.to_string());
  std::process::exit(1);
}

fn js_check(r: Result<(), JSError>) {
  if let Err(e) = r {
    print_err_and_exit(e.into())
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

  let main_future = lazy(move || {
    println!("Hello world");
    let state = Arc::new(IsolateState::new(flags, rest_argv, None));
    let isolate_init = isolate_init::deno_isolate_init();
    let permissions = permissions::DenoPermissions::from_flags(&state.flags);
    let cli = Cli::new(isolate_init, state, permissions);
    let isolate = deno_core::Isolate::new(cli);

    // Setup runtime.
    js_check(isolate.execute("<anonymous>", "denoMain()"));

    isolate.then(|r| {
      js_check(r);
      Ok(())
    })
  });

  // tokio::runtime::current_thread::run(main_future);
  tokio::run(main_future);

  /*
  tokio_util::init(|| {
    isolate
      .execute("denoMain();")
      .map_err(errors::RustOrJsError::from)
      .unwrap_or_else(print_err_and_exit);

    // Execute main module.
    if let Some(main_module) = isolate.state.main_module() {
      debug!("main_module {}", main_module);
      isolate
        .execute_mod(&main_module, should_prefetch)
        .unwrap_or_else(print_err_and_exit);
      if should_display_info {
        // Display file info and exit. Do not run file
        modules::print_file_info(
          &isolate.modules.borrow(),
          &isolate.state.dir,
          main_module,
        );
        std::process::exit(0);
      }
    }

    isolate
      .event_loop()
      .map_err(errors::RustOrJsError::from)
      .unwrap_or_else(print_err_and_exit);
  });
  */
}
