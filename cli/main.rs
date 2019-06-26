// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate log;
#[macro_use]
extern crate futures;
#[macro_use]
extern crate serde_json;
extern crate clap;
extern crate deno;
extern crate indexmap;
#[cfg(unix)]
extern crate nix;
extern crate rand;

mod ansi;
pub mod compiler;
pub mod deno_dir;
pub mod deno_error;
pub mod diagnostics;
mod dispatch_minimal;
pub mod flags;
pub mod fmt_errors;
mod fs;
mod global_timer;
mod http_body;
mod http_util;
mod import_map;
pub mod msg;
pub mod msg_util;
pub mod ops;
pub mod permissions;
mod progress;
mod repl;
pub mod resolve_addr;
pub mod resources;
mod shell;
mod signal;
pub mod source_maps;
mod startup_data;
pub mod state;
mod tokio_util;
mod tokio_write;
pub mod version;
pub mod worker;

use crate::compiler::bundle_async;
use crate::deno_error::DenoError;
use crate::progress::Progress;
use crate::state::ThreadSafeState;
use crate::worker::Worker;
use deno::v8_set_flags;
use deno::ModuleSpecifier;
use flags::DenoFlags;
use flags::DenoSubcommand;
use futures::future;
use futures::lazy;
use futures::Future;
use log::Level;
use log::Metadata;
use log::Record;
use std::env;

static LOGGER: Logger = Logger;

struct Logger;

impl log::Log for Logger {
  fn enabled(&self, metadata: &Metadata) -> bool {
    metadata.level() <= log::max_level()
  }

  fn log(&self, record: &Record) {
    if self.enabled(record.metadata()) {
      let mut target = record.target().to_string();

      if let Some(line_no) = record.line() {
        target.push_str(":");
        target.push_str(&line_no.to_string());
      }

      println!("{} RS - {} - {}", record.level(), target, record.args());
    }
  }
  fn flush(&self) {}
}

fn print_err_and_exit(err: DenoError) {
  eprintln!("{}", err.to_string());
  std::process::exit(1);
}

fn js_check<E>(r: Result<(), E>)
where
  E: Into<DenoError>,
{
  if let Err(err) = r {
    print_err_and_exit(err.into());
  }
}

pub fn print_file_info(
  worker: Worker,
  module_specifier: &ModuleSpecifier,
) -> impl Future<Item = Worker, Error = ()> {
  state::fetch_module_meta_data_and_maybe_compile_async(
    &worker.state,
    module_specifier,
  ).and_then(move |out| {
    println!(
      "{} {}",
      ansi::bold("local:".to_string()),
      out.filename.to_str().unwrap()
    );

    println!(
      "{} {}",
      ansi::bold("type:".to_string()),
      msg::enum_name_media_type(out.media_type)
    );

    if out.maybe_output_code_filename.is_some() {
      println!(
        "{} {}",
        ansi::bold("compiled:".to_string()),
        out.maybe_output_code_filename.unwrap().to_str().unwrap(),
      );
    }

    if out.maybe_source_map_filename.is_some() {
      println!(
        "{} {}",
        ansi::bold("map:".to_string()),
        out.maybe_source_map_filename.unwrap().to_str().unwrap()
      );
    }

    if let Some(deps) =
      worker.state.modules.lock().unwrap().deps(&out.module_name)
    {
      println!("{}{}", ansi::bold("deps:\n".to_string()), deps.name);
      if let Some(ref depsdeps) = deps.deps {
        for d in depsdeps {
          println!("{}", d);
        }
      }
    } else {
      println!(
        "{} cannot retrieve full dependency graph",
        ansi::bold("deps:".to_string()),
      );
    }
    Ok(worker)
  }).map_err(|err| println!("{}", err))
}

fn create_worker_and_state(
  flags: DenoFlags,
  argv: Vec<String>,
) -> (Worker, ThreadSafeState) {
  use crate::shell::Shell;
  use std::sync::Arc;
  use std::sync::Mutex;
  let shell = Arc::new(Mutex::new(Shell::new()));
  let progress = Progress::new();
  progress.set_callback(move |_done, _completed, _total, status, msg| {
    if !status.is_empty() {
      let mut s = shell.lock().unwrap();
      s.status(status, msg).expect("shell problem");
    }
  });
  let state = ThreadSafeState::new(flags, argv, ops::op_selector_std, progress);
  let worker = Worker::new(
    "main".to_string(),
    startup_data::deno_isolate_init(),
    state.clone(),
  );

  (worker, state)
}

fn types_command() {
  let content = include_str!(concat!(
    env!("GN_OUT_DIR"),
    "/gen/cli/lib/lib.deno_runtime.d.ts"
  ));
  println!("{}", content);
}

fn fetch_or_info_command(
  flags: DenoFlags,
  argv: Vec<String>,
  print_info: bool,
) {
  let (mut worker, state) = create_worker_and_state(flags, argv);

  let main_module = state.main_module().unwrap();
  let main_future = lazy(move || {
    // Setup runtime.
    js_check(worker.execute("denoMain()"));
    debug!("main_module {}", main_module);

    worker
      .execute_mod_async(&main_module, true)
      .map_err(print_err_and_exit)
      .and_then(move |()| {
        if print_info {
          future::Either::A(print_file_info(worker, &main_module))
        } else {
          future::Either::B(future::ok(worker))
        }
      }).and_then(|worker| {
        worker.then(|result| {
          js_check(result);
          Ok(())
        })
      })
  });
  tokio_util::run(main_future);
}

fn eval_command(flags: DenoFlags, argv: Vec<String>) {
  let (mut worker, state) = create_worker_and_state(flags, argv);
  // Wrap provided script in async function so asynchronous methods
  // work. This is required until top-level await is not supported.
  let js_source = format!(
    "async function _topLevelWrapper(){{
        {}
      }}
      _topLevelWrapper();
      ",
    &state.argv[1]
  );

  let main_future = lazy(move || {
    js_check(worker.execute("denoMain()"));
    // ATM imports in `deno eval` are not allowed
    // TODO Support ES modules once Worker supports evaluating anonymous modules.
    js_check(worker.execute(&js_source));
    worker.then(|result| {
      js_check(result);
      Ok(())
    })
  });
  tokio_util::run(main_future);
}

fn xeval_command(flags: DenoFlags, argv: Vec<String>) {
  let xeval_replvar = flags.xeval_replvar.clone().unwrap();
  let (mut worker, state) = create_worker_and_state(flags, argv);
  let xeval_source = format!(
    "window._xevalWrapper = async function ({}){{
        {}
      }}",
    &xeval_replvar, &state.argv[1]
  );

  let main_future = lazy(move || {
    // Setup runtime.
    js_check(worker.execute(&xeval_source));
    js_check(worker.execute("denoMain()"));
    worker
      .then(|result| {
        js_check(result);
        Ok(())
      }).map_err(|(err, _worker): (DenoError, Worker)| print_err_and_exit(err))
  });
  tokio_util::run(main_future);
}

fn bundle_command(flags: DenoFlags, argv: Vec<String>) {
  let (mut _worker, state) = create_worker_and_state(flags, argv);

  let main_module = state.main_module().unwrap();
  assert!(state.argv.len() >= 3);
  let out_file = state.argv[2].clone();
  debug!(">>>>> bundle_async START");
  let bundle_future = bundle_async(state, main_module.to_string(), out_file)
    .map_err(|e| {
      debug!("diagnostics returned, exiting!");
      eprintln!("\n{}", e.to_string());
      std::process::exit(1);
    }).and_then(move |_| {
      debug!(">>>>> bundle_async END");
      Ok(())
    });
  tokio_util::run(bundle_future);
}

fn run_repl(flags: DenoFlags, argv: Vec<String>) {
  let (mut worker, _state) = create_worker_and_state(flags, argv);

  // REPL situation.
  let main_future = lazy(move || {
    // Setup runtime.
    js_check(worker.execute("denoMain()"));
    worker
      .then(|result| {
        js_check(result);
        Ok(())
      }).map_err(|(err, _worker): (DenoError, Worker)| print_err_and_exit(err))
  });
  tokio_util::run(main_future);
}

fn run_script(flags: DenoFlags, argv: Vec<String>) {
  let (mut worker, state) = create_worker_and_state(flags, argv);

  let main_module = state.main_module().unwrap();
  // Normal situation of executing a module.
  let main_future = lazy(move || {
    // Setup runtime.
    js_check(worker.execute("denoMain()"));
    debug!("main_module {}", main_module);

    worker
      .execute_mod_async(&main_module, false)
      .and_then(move |()| {
        worker.then(|result| {
          js_check(result);
          Ok(())
        })
      }).map_err(print_err_and_exit)
  });
  tokio_util::run(main_future);
}

fn main() {
  #[cfg(windows)]
  ansi_term::enable_ansi_support().ok(); // For Windows 10

  log::set_logger(&LOGGER).unwrap();
  let args: Vec<String> = env::args().collect();
  let (flags, subcommand, argv) = flags::flags_from_vec(args);

  if let Some(ref v8_flags) = flags.v8_flags {
    v8_set_flags(v8_flags.clone());
  }

  let log_level = match flags.log_level {
    Some(level) => level,
    None => Level::Warn,
  };
  log::set_max_level(log_level.to_level_filter());

  match subcommand {
    DenoSubcommand::Bundle => bundle_command(flags, argv),
    DenoSubcommand::Completions => {}
    DenoSubcommand::Eval => eval_command(flags, argv),
    DenoSubcommand::Fetch => fetch_or_info_command(flags, argv, false),
    DenoSubcommand::Info => fetch_or_info_command(flags, argv, true),
    DenoSubcommand::Install => run_script(flags, argv),
    DenoSubcommand::Repl => run_repl(flags, argv),
    DenoSubcommand::Run => run_script(flags, argv),
    DenoSubcommand::Types => types_command(),
    DenoSubcommand::Version => run_repl(flags, argv),
    DenoSubcommand::Xeval => xeval_command(flags, argv),
  }
}
