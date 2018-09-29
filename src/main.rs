// Copyright 2018 the Deno authors. All rights reserved. MIT license.
extern crate flatbuffers;
extern crate futures;
extern crate hyper;
extern crate libc;
extern crate msg_rs as msg;
extern crate rand;
extern crate tempfile;
extern crate tokio;
extern crate tokio_executor;
extern crate tokio_fs;
extern crate tokio_io;
extern crate tokio_threadpool;
extern crate url;
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate log;
extern crate dirs;
extern crate hyper_rustls;
extern crate remove_dir_all;
extern crate ring;

mod deno_dir;
mod errors;
mod files;
mod flags;
mod fs;
pub mod handlers;
mod http;
mod isolate;
mod libdeno;
mod tokio_util;
mod version;

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
  let mut isolate = isolate::Isolate::new(args, handlers::msg_from_js);
  flags::process(&isolate.state.flags);
  tokio_util::init(|| {
    isolate
      .execute("deno_main.js", "denoMain();")
      .unwrap_or_else(|err| {
        error!("{}", err);
        std::process::exit(1);
      });
    isolate.event_loop();
  });
}
