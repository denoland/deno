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
extern crate serde;
extern crate serde_derive;
extern crate tokio;
extern crate url;

mod checksum;
pub mod colors;
pub mod compilers;
pub mod deno_dir;
pub mod deno_error;
pub mod diagnostics;
mod disk_cache;
mod file_fetcher;
pub mod flags;
pub mod fmt_errors;
mod fs;
mod global_state;
mod global_timer;
mod http_body;
mod http_util;
mod import_map;
mod js;
mod lockfile;
mod metrics;
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
pub mod test_util;
mod tokio_util;
pub mod version;
pub mod worker;

use crate::deno_error::js_check;
use crate::deno_error::print_err_and_exit;
use crate::global_state::ThreadSafeGlobalState;
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

fn create_worker_and_state(
  flags: DenoFlags,
  argv: Vec<String>,
) -> (Worker, ThreadSafeGlobalState) {
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

  let global_state = ThreadSafeGlobalState::new(flags, argv, progress)
    .map_err(deno_error::print_err_and_exit)
    .unwrap();

  let state = ThreadSafeState::new(
    global_state.clone(),
    global_state.main_module.clone(),
    true,
  )
  .map_err(deno_error::print_err_and_exit)
  .unwrap();

  let worker =
    Worker::new("main".to_string(), startup_data::deno_isolate_init(), state);

  (worker, global_state)
}

fn types_command() {
  let content = crate::js::get_asset("lib.deno_runtime.d.ts").unwrap();
  println!("{}", content);
}

fn print_cache_info(worker: Worker) {
  let state = &worker.state.global_state;

  println!(
    "{} {:?}",
    colors::bold("DENO_DIR location:".to_string()),
    state.dir.root
  );
  println!(
    "{} {:?}",
    colors::bold("Remote modules cache:".to_string()),
    state.dir.deps_cache.location
  );
  println!(
    "{} {:?}",
    colors::bold("TypeScript compiler cache:".to_string()),
    state.dir.gen_cache.location
  );
}

pub fn print_file_info(
  worker: Worker,
  module_specifier: &ModuleSpecifier,
) -> impl Future<Item = Worker, Error = ()> {
  let global_state_ = worker.state.global_state.clone();
  let state_ = worker.state.clone();
  let module_specifier_ = module_specifier.clone();

  global_state_
    .file_fetcher
    .fetch_source_file_async(&module_specifier)
    .map_err(|err| println!("{}", err))
    .and_then(|out| {
      println!(
        "{} {}",
        colors::bold("local:".to_string()),
        out.filename.to_str().unwrap()
      );

      println!(
        "{} {}",
        colors::bold("type:".to_string()),
        msg::enum_name_media_type(out.media_type)
      );

      global_state_
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
              && global_state_.ts_compiler.compile_js)
          {
            let compiled_source_file = global_state_
              .ts_compiler
              .get_compiled_source_file(&out.url)
              .unwrap();

            println!(
              "{} {}",
              colors::bold("compiled:".to_string()),
              compiled_source_file.filename.to_str().unwrap(),
            );
          }

          if let Ok(source_map) = global_state_
            .clone()
            .ts_compiler
            .get_source_map_file(&module_specifier_)
          {
            println!(
              "{} {}",
              colors::bold("map:".to_string()),
              source_map.filename.to_str().unwrap()
            );
          }

          if let Some(deps) =
            state_.modules.lock().unwrap().deps(&compiled.name)
          {
            println!("{}{}", colors::bold("deps:\n".to_string()), deps.name);
            if let Some(ref depsdeps) = deps.deps {
              for d in depsdeps {
                println!("{}", d);
              }
            }
          } else {
            println!(
              "{} cannot retrieve full dependency graph",
              colors::bold("deps:".to_string()),
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

  let main_module = state.main_module.as_ref().unwrap().clone();
  let main_future = lazy(move || {
    // Setup runtime.
    js_check(worker.execute("denoMain()"));
    debug!("main_module {}", main_module);

    worker
      .execute_mod_async(&main_module, None, true)
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

  let main_module = state.main_module.as_ref().unwrap().clone();
  let main_future = lazy(move || {
    // Setup runtime.
    js_check(worker.execute("denoMain()"));
    debug!("main_module {}", main_module);

    worker
      .execute_mod_async(&main_module, None, true)
      .then(|result| {
        js_check(result);
        Ok(())
      })
  });
  tokio_util::run(main_future);
}

fn eval_command(flags: DenoFlags, argv: Vec<String>) {
  let (mut worker, state) = create_worker_and_state(flags, argv);
  let ts_source = state.argv[1].clone();
  // Force TypeScript compile.
  let main_module =
    ModuleSpecifier::resolve_url_or_path("./__$deno$eval.ts").unwrap();

  let main_future = lazy(move || {
    js_check(worker.execute("denoMain()"));
    debug!("main_module {}", &main_module);

    let mut worker_ = worker.clone();
    worker
      .execute_mod_async(&main_module, Some(ts_source), false)
      .and_then(move |()| {
        js_check(worker.execute("window.dispatchEvent(new Event('load'))"));
        worker.then(move |result| {
          js_check(result);
          js_check(
            worker_.execute("window.dispatchEvent(new Event('unload'))"),
          );
          Ok(())
        })
      })
      .map_err(print_err_and_exit)
  });
  tokio_util::run(main_future);
}

fn bundle_command(flags: DenoFlags, argv: Vec<String>) {
  let (worker, state) = create_worker_and_state(flags, argv);

  let main_module = state.main_module.as_ref().unwrap().clone();
  assert!(state.argv.len() >= 3);
  let out_file = state.argv[2].clone();
  debug!(">>>>> bundle_async START");
  // NOTE: we need to poll `worker` otherwise TS compiler worker won't run properly
  let main_future = lazy(move || {
    worker.then(move |result| {
      js_check(result);
      state
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
        })
    })
  });
  tokio_util::run(main_future);
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

  let main_module = state.main_module.as_ref().unwrap().clone();
  // Normal situation of executing a module.
  let main_future = lazy(move || {
    // Setup runtime.
    js_check(worker.execute("denoMain()"));
    debug!("main_module {}", main_module);

    let mut worker_ = worker.clone();

    worker
      .execute_mod_async(&main_module, None, false)
      .and_then(move |()| {
        if state.flags.lock_write {
          if let Some(ref lockfile) = state.lockfile {
            let g = lockfile.lock().unwrap();
            g.write()?;
          } else {
            eprintln!("--lock flag must be specified when using --lock-write");
            std::process::exit(11);
          }
        }
        Ok(())
      })
      .and_then(move |()| {
        js_check(worker.execute("window.dispatchEvent(new Event('load'))"));
        worker.then(move |result| {
          js_check(result);
          js_check(
            worker_.execute("window.dispatchEvent(new Event('unload'))"),
          );
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
  println!("typescript: {}", version::TYPESCRIPT);
}

pub fn main() {
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
  }
}
