// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate log;
#[macro_use]
extern crate futures;
#[macro_use]
extern crate serde_json;

#[path = "../../src/ansi.rs"]
mod ansi;
#[path = "../../src/compiler.rs"]
pub mod compiler;
#[path = "../../src/deno_dir.rs"]
pub mod deno_dir;
#[path = "../../src/errors.rs"]
pub mod errors;
#[path = "../../src/flags.rs"]
pub mod flags;
#[path = "../../src/fs.rs"]
mod fs;
#[path = "../../src/http_body.rs"]
mod http_body;
#[path = "../../src/http_util.rs"]
mod http_util;
#[path = "../../src/isolate.rs"]
pub mod isolate;
#[path = "../../src/js_errors.rs"]
pub mod js_errors;
#[path = "../../src/libdeno.rs"]
pub mod libdeno;
#[path = "../../src/modules.rs"]
pub mod modules;
#[path = "../../src/msg.rs"]
pub mod msg;
#[path = "../../src/msg_util.rs"]
pub mod msg_util;
#[path = "../../src/ops.rs"]
pub mod ops;
#[path = "../../src/permissions.rs"]
pub mod permissions;
#[path = "../../src/repl.rs"]
mod repl;
#[path = "../../src/resolve_addr.rs"]
pub mod resolve_addr;
#[path = "../../src/resources.rs"]
pub mod resources;
#[path = "../../src/snapshot.rs"]
pub mod snapshot;
#[path = "../../src/tokio_util.rs"]
mod tokio_util;
#[path = "../../src/tokio_write.rs"]
mod tokio_write;
#[path = "../../src/version.rs"]
pub mod version;
#[path = "../../src/workers.rs"]
pub mod workers;

#[cfg(unix)]
#[path = "../../src/eager_unix.rs"]
mod eager_unix;

use log::{LevelFilter, Metadata, Record};
use std::env;
use std::sync::Arc;

use crate::libdeno::deno_buf;

fn tsc_snapshot() -> deno_buf {
  #[cfg(not(feature = "check-only"))]
  let data =
    include_bytes!(concat!(env!("GN_OUT_DIR"), "/gen/examples/tsc/snapshot_tsc.bin"));
  // The snapshot blob is not available when the Rust Language Server runs
  // 'cargo check'.
  #[cfg(feature = "check-only")]
  let data = vec![];

  unsafe { deno_buf::from_raw_parts(data.as_ptr(), data.len()) }
}

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
    rest_argv.insert(1, "https://deno.land/x/std/prettier/main.ts".to_string());
    flags.allow_read = true;
    flags.allow_write = true;
  }

  let should_prefetch = flags.prefetch || flags.info;
  let should_display_info = flags.info;

  let state = Arc::new(isolate::IsolateState::new(flags, rest_argv, None));
  let snapshot = tsc_snapshot();
  let mut isolate = isolate::Isolate::new(snapshot, state, ops::dispatch);

  tokio_util::init(|| {
    // Setup runtime.
    isolate
      .execute("denoMain();")
      .map_err(errors::RustOrJsError::from)
      .unwrap_or_else(print_err_and_exit);

    // Execute input file.
    if isolate.state.argv.len() > 1 {
      let input_filename = isolate.state.argv[1].clone();
      isolate
        .execute_mod(&input_filename, should_prefetch)
        .unwrap_or_else(print_err_and_exit);

      if should_display_info {
        // Display file info and exit. Do not run file
        modules::print_file_info(
          &isolate.modules.borrow(),
          &isolate.state.dir,
          input_filename,
        );
        std::process::exit(0);
      }
    }

    isolate
      .event_loop()
      .map_err(errors::RustOrJsError::from)
      .unwrap_or_else(print_err_and_exit);
  });
}
