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

  let args = env::args().collect();
  let mut isolate = Isolate::new(args);
  flags::process(&isolate.flags);

  isolate
    .execute("deno_main.js", "denoMain();")
    .unwrap_or_else(|err| {
      error!("{}", err);
      std::process::exit(1);
    });

  // Start the Tokio event loop
  isolate.rt.run().expect("err");
}
