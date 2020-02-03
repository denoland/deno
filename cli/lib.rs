// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
#![deny(warnings)]

#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate log;
extern crate futures;
#[macro_use]
extern crate serde_json;
extern crate clap;
extern crate deno_core;
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
mod fmt;
pub mod fmt_errors;
mod fs;
mod global_state;
mod global_timer;
mod http_util;
mod import_map;
pub mod installer;
mod js;
mod lockfile;
mod metrics;
pub mod msg;
pub mod ops;
pub mod permissions;
mod progress;
mod repl;
pub mod resolve_addr;
mod shell;
pub mod signal;
pub mod source_maps;
mod startup_data;
pub mod state;
pub mod test_util;
mod tokio_util;
pub mod version;
mod web_worker;
pub mod worker;

use crate::compilers::TargetLib;
use crate::deno_error::js_check;
use crate::deno_error::{print_err_and_exit, print_msg_and_exit};
use crate::global_state::ThreadSafeGlobalState;
use crate::ops::io::get_stdio;
use crate::progress::Progress;
use crate::state::ThreadSafeState;
use crate::worker::MainWorker;
use deno_core::v8_set_flags;
use deno_core::ErrBox;
use deno_core::ModuleSpecifier;
use flags::DenoFlags;
use flags::DenoSubcommand;
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
) -> (MainWorker, ThreadSafeGlobalState) {
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

  let global_state = ThreadSafeGlobalState::new(flags, progress)
    .map_err(deno_error::print_err_and_exit)
    .unwrap();

  let (int, ext) = ThreadSafeState::create_channels();
  let state = ThreadSafeState::new(
    global_state.clone(),
    None,
    global_state.main_module.clone(),
    int,
  )
  .map_err(deno_error::print_err_and_exit)
  .unwrap();

  let state_ = state.clone();
  {
    let mut resource_table = state_.lock_resource_table();
    let (stdin, stdout, stderr) = get_stdio();
    resource_table.add("stdin", Box::new(stdin));
    resource_table.add("stdout", Box::new(stdout));
    resource_table.add("stderr", Box::new(stderr));
  }

  let worker = MainWorker::new(
    "main".to_string(),
    startup_data::deno_isolate_init(),
    state,
    ext,
  );

  (worker, global_state)
}

fn types_command() {
  println!(
    "{}\n{}\n{}",
    crate::js::DENO_NS_LIB,
    crate::js::SHARED_GLOBALS_LIB,
    crate::js::WINDOW_LIB
  );
}

fn print_cache_info(worker: MainWorker) {
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

async fn print_file_info(
  worker: &MainWorker,
  module_specifier: ModuleSpecifier,
) {
  let global_state = worker.state.global_state.clone();

  let maybe_source_file = global_state
    .file_fetcher
    .fetch_source_file_async(&module_specifier, None)
    .await;
  if let Err(err) = maybe_source_file {
    println!("{}", err);
    return;
  }
  let out = maybe_source_file.unwrap();
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

  let module_specifier_ = module_specifier.clone();
  let maybe_compiled = global_state
    .clone()
    .fetch_compiled_module(module_specifier_, None, TargetLib::Main)
    .await;
  if let Err(e) = maybe_compiled {
    debug!("compiler error exiting!");
    eprintln!("\n{}", e.to_string());
    std::process::exit(1);
  }
  if out.media_type == msg::MediaType::TypeScript
    || (out.media_type == msg::MediaType::JavaScript
      && global_state.ts_compiler.compile_js)
  {
    let compiled_source_file = global_state
      .ts_compiler
      .get_compiled_source_file(&out.url)
      .unwrap();

    println!(
      "{} {}",
      colors::bold("compiled:".to_string()),
      compiled_source_file.filename.to_str().unwrap(),
    );
  }

  if let Ok(source_map) = global_state
    .clone()
    .ts_compiler
    .get_source_map_file(&module_specifier)
  {
    println!(
      "{} {}",
      colors::bold("map:".to_string()),
      source_map.filename.to_str().unwrap()
    );
  }

  if let Some(deps) = worker.isolate.modules.deps(&module_specifier) {
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
}

async fn info_command(flags: DenoFlags) {
  let argv_len = flags.argv.len();
  let (mut worker, state) = create_worker_and_state(flags);

  // If it was just "deno info" print location of caches and exit
  if argv_len == 1 {
    return print_cache_info(worker);
  }

  let main_module = state.main_module.as_ref().unwrap().clone();

  // Setup runtime.
  js_check(worker.execute("bootstrapMainRuntime()"));
  debug!("main_module {}", main_module);

  let main_result = worker.execute_mod_async(&main_module, None, true).await;
  if let Err(e) = main_result {
    print_err_and_exit(e);
  }
  print_file_info(&worker, main_module.clone()).await;
  let result = (&mut *worker).await;
  js_check(result);
}

async fn install_command(
  flags: DenoFlags,
  dir: Option<String>,
  exe_name: String,
  module_url: String,
  args: Vec<String>,
) {
  // Firstly fetch and compile module, this
  // ensures the module exists.
  let mut fetch_flags = flags.clone();
  fetch_flags.argv.push(module_url.to_string());
  fetch_flags.reload = true;
  fetch_command(fetch_flags).await;

  let install_result =
    installer::install(flags, dir, &exe_name, &module_url, args);
  if let Err(e) = install_result {
    print_msg_and_exit(&e.to_string());
  }
}

async fn fetch_command(flags: DenoFlags) {
  let args = flags.argv.clone();

  let (mut worker, state) = create_worker_and_state(flags);

  let main_module = state.main_module.as_ref().unwrap().clone();

  // Setup runtime.
  js_check(worker.execute("bootstrapMainRuntime()"));
  debug!("main_module {}", main_module);

  let result = worker.execute_mod_async(&main_module, None, true).await;
  js_check(result);

  // resolve modules for rest of args if present
  let files_len = args.len();
  if files_len > 2 {
    for next_specifier in args.iter().take(files_len).skip(2) {
      let next_module =
        ModuleSpecifier::resolve_url_or_path(&next_specifier).unwrap();
      let result = worker.execute_mod_async(&next_module, None, true).await;
      js_check(result);
    }
  }

  if state.flags.lock_write {
    if let Some(ref lockfile) = state.lockfile {
      let g = lockfile.lock().unwrap();
      if let Err(e) = g.write() {
        print_err_and_exit(ErrBox::from(e));
      }
    } else {
      eprintln!("--lock flag must be specified when using --lock-write");
      std::process::exit(11);
    }
  }
}

async fn eval_command(flags: DenoFlags) {
  let ts_source = flags.argv[1].clone();
  let (mut worker, _state) = create_worker_and_state(flags);
  // Force TypeScript compile.
  let main_module =
    ModuleSpecifier::resolve_url_or_path("./__$deno$eval.ts").unwrap();

  js_check(worker.execute("bootstrapMainRuntime()"));
  debug!("main_module {}", &main_module);

  let exec_result = worker
    .execute_mod_async(&main_module, Some(ts_source), false)
    .await;
  if let Err(e) = exec_result {
    print_err_and_exit(e);
  }
  js_check(worker.execute("window.dispatchEvent(new Event('load'))"));
  let result = (&mut *worker).await;
  js_check(result);
  js_check(worker.execute("window.dispatchEvent(new Event('unload'))"));
}

async fn bundle_command(flags: DenoFlags) {
  let out_file = flags.bundle_output.clone();
  let (mut worker, state) = create_worker_and_state(flags);
  let main_module = state.main_module.as_ref().unwrap().clone();

  debug!(">>>>> bundle_async START");
  // NOTE: we need to poll `worker` otherwise TS compiler worker won't run properly
  let result = (&mut *worker).await;
  js_check(result);
  let bundle_result = state
    .ts_compiler
    .bundle_async(state.clone(), main_module.to_string(), out_file)
    .await;
  if let Err(err) = bundle_result {
    debug!("diagnostics returned, exiting!");
    eprintln!("");
    print_err_and_exit(err);
  }
  debug!(">>>>> bundle_async END");
}

async fn run_repl(flags: DenoFlags) {
  let (mut worker, _state) = create_worker_and_state(flags);
  js_check(worker.execute("bootstrapMainRuntime()"));
  loop {
    let result = (&mut *worker).await;
    if let Err(err) = result {
      eprintln!("{}", err.to_string());
    }
  }
}

async fn run_script(flags: DenoFlags) {
  let (mut worker, state) = create_worker_and_state(flags);

  let maybe_main_module = state.main_module.as_ref();
  if maybe_main_module.is_none() {
    print_msg_and_exit("Please provide a name to the main script to run.");
  }
  let main_module = maybe_main_module.unwrap().clone();
  // Normal situation of executing a module.

  // Setup runtime.
  js_check(worker.execute("bootstrapMainRuntime()"));
  debug!("main_module {}", main_module);

  let mod_result = worker.execute_mod_async(&main_module, None, false).await;
  if let Err(err) = mod_result {
    print_err_and_exit(err);
  }
  if state.flags.lock_write {
    if let Some(ref lockfile) = state.lockfile {
      let g = lockfile.lock().unwrap();
      if let Err(e) = g.write() {
        print_err_and_exit(ErrBox::from(e));
      }
    } else {
      eprintln!("--lock flag must be specified when using --lock-write");
      std::process::exit(11);
    }
  }
  js_check(worker.execute("window.dispatchEvent(new Event('load'))"));
  let result = (&mut *worker).await;
  js_check(result);
  js_check(worker.execute("window.dispatchEvent(new Event('unload'))"));
}

async fn fmt_command(files: Option<Vec<String>>, check: bool) {
  fmt::format_files(files, check);
}

pub fn main() {
  #[cfg(windows)]
  ansi_term::enable_ansi_support().ok(); // For Windows 10

  log::set_logger(&LOGGER).unwrap();
  let args: Vec<String> = env::args().collect();
  let flags = flags::flags_from_vec(args);

  if let Some(ref v8_flags) = flags.v8_flags {
    let mut v8_flags_ = v8_flags.clone();
    v8_flags_.insert(0, "UNUSED_BUT_NECESSARY_ARG0".to_string());
    v8_set_flags(v8_flags_);
  }

  let log_level = match flags.log_level {
    Some(level) => level,
    None => Level::Warn,
  };
  log::set_max_level(log_level.to_level_filter());

  let fut = async move {
    match flags.clone().subcommand {
      DenoSubcommand::Bundle => bundle_command(flags).await,
      DenoSubcommand::Completions => {}
      DenoSubcommand::Eval => eval_command(flags).await,
      DenoSubcommand::Fetch => fetch_command(flags).await,
      DenoSubcommand::Format { check, files } => {
        fmt_command(files, check).await
      }
      DenoSubcommand::Info => info_command(flags).await,
      DenoSubcommand::Install {
        dir,
        exe_name,
        module_url,
        args,
      } => install_command(flags, dir, exe_name, module_url, args).await,
      DenoSubcommand::Repl => run_repl(flags).await,
      DenoSubcommand::Run => run_script(flags).await,
      DenoSubcommand::Types => types_command(),
      _ => panic!("bad subcommand"),
    }
  };
  tokio_util::run_basic(fut);
}
