// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate log;
#[macro_use]
extern crate futures;
#[macro_use]
extern crate serde_json;

pub mod compiler;
pub mod deno_dir;
pub mod errors;
pub mod flags;
mod fs;
mod http_body;
mod http_util;
pub mod isolate;
pub mod js_errors;
pub mod libdeno;
pub mod msg;
pub mod msg_util;
pub mod ops;
pub mod permissions;
mod repl;
pub mod resolve_addr;
pub mod resources;
pub mod snapshot;
mod tokio_util;
mod tokio_write;
pub mod version;
pub mod workers;

#[cfg(unix)]
mod eager_unix;

use std::env;
use std::sync::Arc;

static LOGGER: Logger = Logger;

struct Logger;

impl log::Log for Logger {
  fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
    metadata.level() <= log::max_level()
  }

  fn log(&self, record: &log::Record<'_>) {
    if self.enabled(record.metadata()) {
      println!("{} RS - {}", record.level(), record.args());
    }
  }
  fn flush(&self) {}
}

fn print_err_and_exit(err: js_errors::JSError) {
  eprintln!("{}", err.to_string());
  std::process::exit(1);
}

fn main() {
  log::set_logger(&LOGGER).unwrap();
  let args = env::args().collect();
  let (flags, rest_argv, usage_string) =
    flags::set_flags(args).unwrap_or_else(|err| {
      eprintln!("{}", err);
      std::process::exit(1)
    });

  if flags.help {
    println!("{}", &usage_string);
    std::process::exit(0);
  }

  log::set_max_level(if flags.log_debug {
    log::LevelFilter::Debug
  } else {
    log::LevelFilter::Warn
  });

  let should_prefetch = flags.prefetch;

  let state = Arc::new(isolate::IsolateState::new(flags, rest_argv, None));
  let snapshot = snapshot::deno_snapshot();
  let isolate = isolate::Isolate::new(snapshot, state, ops::dispatch);

  tokio_util::init(|| {
    // Setup runtime.
    isolate
      .execute("denoMain();")
      .unwrap_or_else(print_err_and_exit);

    // Execute input file.
    if isolate.state.argv.len() > 1 {
      let input_filename = &isolate.state.argv[1];
      isolate
        .execute_mod(input_filename, should_prefetch)
        .unwrap_or_else(print_err_and_exit);
    }

    isolate.event_loop().unwrap_or_else(print_err_and_exit);
  });
}
