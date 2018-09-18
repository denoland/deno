// Copyright 2018 the Deno authors. All rights reserved. MIT license.
extern crate flatbuffers;
extern crate futures;
extern crate hyper;
extern crate libc;
extern crate msg_rs as msg;
extern crate rand;
extern crate tempfile;
extern crate tokio;
extern crate url;
#[macro_use]
extern crate log;
extern crate dirs;
extern crate hyper_rustls;
extern crate remove_dir_all;
extern crate ring;

mod deno_dir;
mod errors;
mod flags;
mod fs;
pub mod handlers;
mod isolate;
mod libdeno;
mod net;
mod version;

use isolate::Isolate;
use std::env;

static LOGGER: Logger = Logger;

struct Logger;

impl log::Log for Logger {
  fn enabled(&self, metadata: &log::Metadata) -> bool {
    metadata.level() <= log::max_level()
  }

  fn log(&self, record: &log::Record) {
    if self.enabled(record.metadata()) {
      println!("{} RS - {}", record.level(), record.args());
    }
  }
  fn flush(&self) {}
}

fn main() {
  log::set_logger(&LOGGER).unwrap();

  let js_args = flags::v8_set_flags(env::args().collect());

  let mut isolate = Isolate::new(js_args);

  if isolate.flags.help {
    flags::print_usage();
    std::process::exit(0);
  }

  if isolate.flags.version {
    version::print_version();
    std::process::exit(0);
  }

  let mut log_level = log::LevelFilter::Info;
  if isolate.flags.log_debug {
    log_level = log::LevelFilter::Debug;
  }
  log::set_max_level(log_level);

  isolate
    .execute("deno_main.js", "denoMain();")
    .unwrap_or_else(|err| {
      error!("{}", err);
      std::process::exit(1);
    });

  // Start the Tokio event loop
  isolate.rt.run().expect("err");
}
