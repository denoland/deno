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
use crate::global_state::GlobalState;
use crate::ops::io::get_stdio;
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

fn create_global_state(flags: DenoFlags) -> GlobalState {
  GlobalState::new(flags)
    .map_err(deno_error::print_err_and_exit)
    .unwrap()
}

fn create_main_worker(
  global_state: GlobalState,
  main_module: ModuleSpecifier,
) -> MainWorker {
  let state = ThreadSafeState::new(global_state, None, main_module)
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

  MainWorker::new("main".to_string(), startup_data::deno_isolate_init(), state)
}

fn types_command() {
  println!(
    "{}\n{}\n{}",
    crate::js::DENO_NS_LIB,
    crate::js::SHARED_GLOBALS_LIB,
    crate::js::WINDOW_LIB
  );
}

fn print_cache_info(state: &GlobalState) {
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

async fn info_command(flags: DenoFlags, file: Option<String>) {
  let global_state = create_global_state(flags);
  // If it was just "deno info" print location of caches and exit
  if file.is_none() {
    return print_cache_info(&global_state);
  }
  // Setup runtime.
  let main_module = ModuleSpecifier::resolve_url_or_path(&file.unwrap())
    .expect("Bad specifier");
  let mut worker = create_main_worker(global_state, main_module.clone());

  // TODO(bartlomieju): not needed?
  js_check(worker.execute("bootstrapMainRuntime()"));

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
  fetch_flags.reload = true;
  fetch_command(fetch_flags, vec![module_url.to_string()]).await;

  let install_result =
    installer::install(flags, dir, &exe_name, &module_url, args);
  if let Err(e) = install_result {
    print_msg_and_exit(&e.to_string());
  }
}

async fn fetch_command(flags: DenoFlags, files: Vec<String>) {
  let main_module =
    ModuleSpecifier::resolve_url_or_path("./__$deno$fetch.ts").unwrap();
  let global_state = create_global_state(flags);
  let mut worker =
    create_main_worker(global_state.clone(), main_module.clone());

  // TODO(bartlomieju): not needed?
  js_check(worker.execute("bootstrapMainRuntime()"));

  for file in files {
    let specifier = ModuleSpecifier::resolve_url_or_path(&file).unwrap();
    let result = worker.execute_mod_async(&specifier, None, true).await;
    js_check(result);
  }

  if global_state.flags.lock_write {
    if let Some(ref lockfile) = global_state.lockfile {
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

async fn eval_command(flags: DenoFlags, code: String) {
  // Force TypeScript compile.
  let main_module =
    ModuleSpecifier::resolve_url_or_path("./__$deno$eval.ts").unwrap();
  let global_state = create_global_state(flags);
  let mut worker = create_main_worker(global_state, main_module.clone());

  js_check(worker.execute("bootstrapMainRuntime()"));
  debug!("main_module {}", &main_module);

  let exec_result = worker
    .execute_mod_async(&main_module, Some(code), false)
    .await;
  if let Err(e) = exec_result {
    print_err_and_exit(e);
  }
  js_check(worker.execute("window.dispatchEvent(new Event('load'))"));
  let result = (&mut *worker).await;
  js_check(result);
  js_check(worker.execute("window.dispatchEvent(new Event('unload'))"));
}

async fn bundle_command(
  flags: DenoFlags,
  source_file: String,
  out_file: Option<String>,
) {
  debug!(">>>>> bundle_async START");
  let source_file_specifier =
    ModuleSpecifier::resolve_url_or_path(&source_file).expect("Bad specifier");
  let global_state = create_global_state(flags);
  let mut worker =
    create_main_worker(global_state.clone(), source_file_specifier.clone());

  // NOTE: we need to poll `worker` otherwise TS compiler worker won't run properly
  let result = (&mut *worker).await;
  js_check(result);
  let bundle_result = global_state
    .ts_compiler
    .bundle_async(
      global_state.clone(),
      source_file_specifier.to_string(),
      out_file,
    )
    .await;
  if let Err(err) = bundle_result {
    debug!("diagnostics returned, exiting!");
    eprintln!("");
    print_err_and_exit(err);
  }
  debug!(">>>>> bundle_async END");
}

async fn run_repl(flags: DenoFlags) {
  let main_module =
    ModuleSpecifier::resolve_url_or_path("./__$deno$repl.ts").unwrap();
  let global_state = create_global_state(flags);
  let mut worker = create_main_worker(global_state, main_module);
  js_check(worker.execute("bootstrapMainRuntime()"));
  loop {
    let result = (&mut *worker).await;
    if let Err(err) = result {
      eprintln!("{}", err.to_string());
    }
  }
}

async fn run_script(flags: DenoFlags, script: String) {
  let main_module = ModuleSpecifier::resolve_url_or_path(&script).unwrap();
  let global_state = create_global_state(flags);
  let mut worker =
    create_main_worker(global_state.clone(), main_module.clone());

  // Setup runtime.
  js_check(worker.execute("bootstrapMainRuntime()"));
  debug!("main_module {}", main_module);

  let mod_result = worker.execute_mod_async(&main_module, None, false).await;
  if let Err(err) = mod_result {
    print_err_and_exit(err);
  }
  if global_state.flags.lock_write {
    if let Some(ref lockfile) = global_state.lockfile {
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
      DenoSubcommand::Bundle {
        source_file,
        out_file,
      } => bundle_command(flags, source_file, out_file).await,
      DenoSubcommand::Completions { buf } => {
        print!("{}", std::str::from_utf8(&buf).unwrap());
      }
      DenoSubcommand::Eval { code } => eval_command(flags, code).await,
      DenoSubcommand::Fetch { files } => fetch_command(flags, files).await,
      DenoSubcommand::Format { check, files } => {
        fmt_command(files, check).await
      }
      DenoSubcommand::Info { file } => info_command(flags, file).await,
      DenoSubcommand::Install {
        dir,
        exe_name,
        module_url,
        args,
      } => install_command(flags, dir, exe_name, module_url, args).await,
      DenoSubcommand::Repl => run_repl(flags).await,
      DenoSubcommand::Run { script } => run_script(flags, script).await,
      DenoSubcommand::Types => types_command(),
      _ => panic!("bad subcommand"),
    }
  };
  tokio_util::run_basic(fut);
}
