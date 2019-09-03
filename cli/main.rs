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
extern crate deno_typescript;
extern crate indexmap;
#[cfg(unix)]
extern crate nix;
extern crate rand;
extern crate serde;
extern crate serde_derive;
extern crate url;

#[cfg(test)]
mod integration_tests;

mod ansi;
mod assets;
pub mod compilers;
pub mod deno_dir;
pub mod deno_error;
pub mod diagnostics;
mod disk_cache;
mod file_fetcher;
pub mod flags;
pub mod fmt_errors;
mod fs;
mod global_timer;
mod http_body;
mod http_util;
mod import_map;
pub mod msg;
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

use crate::progress::Progress;
use crate::state::ThreadSafeState;
use crate::worker::Worker;
use deno::v8_set_flags;
use deno::ErrBox;
use deno::ModuleSpecifier;
use flags::DenoFlags;
use flags::DenoSubcommand;
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

fn print_err_and_exit(err: ErrBox) {
  eprintln!("{}", err.to_string());
  std::process::exit(1);
}

fn js_check(r: Result<(), ErrBox>) {
  if let Err(err) = r {
    print_err_and_exit(err);
  }
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
  // TODO(kevinkassimo): maybe make include_deno_namespace also configurable?
  let state = ThreadSafeState::new(flags, argv, progress, true)
    .map_err(print_err_and_exit)
    .unwrap();
  let worker = Worker::new(
    "main".to_string(),
    startup_data::deno_isolate_init(),
    state.clone(),
  );

  (worker, state)
}

fn types_command() {
  let content = assets::get_source_code("lib.deno_runtime.d.ts").unwrap();
  println!("{}", content);
}

fn print_cache_info(worker: Worker) {
  let state = worker.state;

  println!(
    "{} {:?}",
    ansi::bold("DENO_DIR location:".to_string()),
    state.dir.root
  );
  println!(
    "{} {:?}",
    ansi::bold("Remote modules cache:".to_string()),
    state.dir.deps_cache.location
  );
  println!(
    "{} {:?}",
    ansi::bold("TypeScript compiler cache:".to_string()),
    state.dir.gen_cache.location
  );
}

pub fn print_file_info(
  worker: Worker,
  module_specifier: &ModuleSpecifier,
) -> impl Future<Item = Worker, Error = ()> {
  let state_ = worker.state.clone();
  let module_specifier_ = module_specifier.clone();

  state_
    .file_fetcher
    .fetch_source_file_async(&module_specifier)
    .map_err(|err| println!("{}", err))
    .and_then(|out| {
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

      state_
        .clone()
        .fetch_compiled_module(&module_specifier_)
        .map_err(|e| {
          debug!("compiler error exiting!");
          eprintln!("\n{}", e.to_string());
          std::process::exit(1);
        })
        .and_then(move |compiled| {
          if out.media_type == msg::MediaType::TypeScript
            || (out.media_type == msg::MediaType::JavaScript
              && state_.ts_compiler.compile_js)
          {
            let compiled_source_file = state_
              .ts_compiler
              .get_compiled_source_file(&out.url)
              .unwrap();

            println!(
              "{} {}",
              ansi::bold("compiled:".to_string()),
              compiled_source_file.filename.to_str().unwrap(),
            );
          }

          if let Ok(source_map) = state_
            .clone()
            .ts_compiler
            .get_source_map_file(&module_specifier_)
          {
            println!(
              "{} {}",
              ansi::bold("map:".to_string()),
              source_map.filename.to_str().unwrap()
            );
          }

          if let Some(deps) =
            worker.state.modules.lock().unwrap().deps(&compiled.name)
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
        })
    })
}

fn info_command(flags: DenoFlags, argv: Vec<String>) {
  let (mut worker, state) = create_worker_and_state(flags, argv.clone());

  // If it was just "deno info" print location of caches and exit
  if argv.len() == 1 {
    return print_cache_info(worker);
  }

  let main_module = state.main_module().unwrap();
  let main_future = lazy(move || {
    // Setup runtime.
    js_check(worker.execute("denoMain()"));
    debug!("main_module {}", main_module);

    worker
      .execute_mod_async(&main_module, true)
      .map_err(print_err_and_exit)
      .and_then(move |()| print_file_info(worker, &main_module))
      .and_then(|worker| {
        worker.then(|result| {
          js_check(result);
          Ok(())
        })
      })
  });
  tokio_util::run(main_future);
}

fn fetch_command(flags: DenoFlags, argv: Vec<String>) {
  let (mut worker, state) = create_worker_and_state(flags, argv.clone());

  let main_module = state.main_module().unwrap();
  let main_future = lazy(move || {
    // Setup runtime.
    js_check(worker.execute("denoMain()"));
    debug!("main_module {}", main_module);

    worker.execute_mod_async(&main_module, true).then(|result| {
      js_check(result);
      Ok(())
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
      })
      .map_err(print_err_and_exit)
  });
  tokio_util::run(main_future);
}

fn bundle_command(flags: DenoFlags, argv: Vec<String>) {
  let (mut _worker, state) = create_worker_and_state(flags, argv);

  let main_module = state.main_module().unwrap();
  assert!(state.argv.len() >= 3);
  let out_file = state.argv[2].clone();
  debug!(">>>>> bundle_async START");
  let bundle_future = state
    .ts_compiler
    .bundle_async(state.clone(), main_module.to_string(), out_file)
    .map_err(|err| {
      debug!("diagnostics returned, exiting!");
      eprintln!("");
      print_err_and_exit(err);
    })
    .and_then(move |_| {
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
      })
      .map_err(|(err, _worker): (ErrBox, Worker)| print_err_and_exit(err))
  });
  tokio_util::run(main_future);
}

fn run_script(flags: DenoFlags, argv: Vec<String>) {
  let use_current_thread = flags.current_thread;
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
        js_check(worker.execute("window.dispatchEvent(new Event('load'))"));
        worker.then(|result| {
          js_check(result);
          Ok(())
        })
      })
      .map_err(print_err_and_exit)
  });

  if use_current_thread {
    tokio_util::run_on_current_thread(main_future);
  } else {
    tokio_util::run(main_future);
  }
}

fn version_command() {
  println!("deno: {}", version::DENO);
  println!("v8: {}", version::v8());
  println!("typescript: {}", version::typescript());
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
    DenoSubcommand::Fetch => fetch_command(flags, argv),
    DenoSubcommand::Info => info_command(flags, argv),
    DenoSubcommand::Repl => run_repl(flags, argv),
    DenoSubcommand::Run => run_script(flags, argv),
    DenoSubcommand::Types => types_command(),
    DenoSubcommand::Version => version_command(),
    DenoSubcommand::Xeval => xeval_command(flags, argv),
  }
}
