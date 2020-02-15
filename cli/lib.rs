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
mod test_runner;
pub mod test_util;
mod tokio_util;
pub mod version;
mod web_worker;
pub mod worker;

use crate::compilers::TargetLib;
use crate::deno_error::js_check;
use crate::fs as deno_fs;
use crate::global_state::GlobalState;
use crate::ops::io::get_stdio;
use crate::state::State;
use crate::worker::MainWorker;
use deno_core::v8_set_flags;
use deno_core::ErrBox;
use deno_core::ModuleSpecifier;
use flags::DenoFlags;
use flags::DenoSubcommand;
use futures::future::FutureExt;
use log::Level;
use log::Metadata;
use log::Record;
use std::env;
use std::fs as std_fs;
use std::path::PathBuf;

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

fn create_main_worker(
  global_state: GlobalState,
  main_module: ModuleSpecifier,
) -> MainWorker {
  let state = State::new(global_state, None, main_module)
    .map_err(deno_error::print_err_and_exit)
    .unwrap();

  let state_ = state.clone();
  {
    let mut state = state_.borrow_mut();
    let (stdin, stdout, stderr) = get_stdio();
    state.resource_table.add("stdin", Box::new(stdin));
    state.resource_table.add("stdout", Box::new(stdout));
    state.resource_table.add("stderr", Box::new(stderr));
  }

  let mut worker = MainWorker::new(
    "main".to_string(),
    startup_data::deno_isolate_init(),
    state,
  );
  js_check(worker.execute("bootstrapMainRuntime()"));
  worker
}

fn types_command() {
  let types = format!(
    "{}\n{}\n{}",
    crate::js::DENO_NS_LIB,
    crate::js::SHARED_GLOBALS_LIB,
    crate::js::WINDOW_LIB
  );
  use std::io::Write;
  let _r = std::io::stdout().write_all(types.as_bytes());
  // TODO(ry) Only ignore SIGPIPE. Currently ignoring all errors.
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
) -> Result<(), ErrBox> {
  let global_state = worker.state.borrow().global_state.clone();

  let out = global_state
    .file_fetcher
    .fetch_source_file_async(&module_specifier, None)
    .await?;

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
  global_state
    .clone()
    .fetch_compiled_module(module_specifier_, None, TargetLib::Main)
    .await?;

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

  Ok(())
}

async fn info_command(
  flags: DenoFlags,
  file: Option<String>,
) -> Result<(), ErrBox> {
  let global_state = GlobalState::new(flags)?;
  // If it was just "deno info" print location of caches and exit
  if file.is_none() {
    print_cache_info(&global_state);
    return Ok(());
  }

  let main_module = ModuleSpecifier::resolve_url_or_path(&file.unwrap())?;
  let mut worker = create_main_worker(global_state, main_module.clone());
  worker.preload_module(&main_module).await?;
  print_file_info(&worker, main_module.clone()).await?;
  (&mut *worker).await
}

async fn install_command(
  flags: DenoFlags,
  dir: Option<PathBuf>,
  exe_name: String,
  module_url: String,
  args: Vec<String>,
  force: bool,
) -> Result<(), ErrBox> {
  // Firstly fetch and compile module, this
  // ensures the module exists.
  let mut fetch_flags = flags.clone();
  fetch_flags.reload = true;
  fetch_command(fetch_flags, vec![module_url.to_string()]).await?;
  installer::install(flags, dir, &exe_name, &module_url, args, force)
    .map_err(ErrBox::from)
}

async fn fetch_command(
  flags: DenoFlags,
  files: Vec<String>,
) -> Result<(), ErrBox> {
  let main_module =
    ModuleSpecifier::resolve_url_or_path("./__$deno$fetch.ts").unwrap();
  let global_state = GlobalState::new(flags)?;
  let mut worker =
    create_main_worker(global_state.clone(), main_module.clone());

  for file in files {
    let specifier = ModuleSpecifier::resolve_url_or_path(&file)?;
    worker.preload_module(&specifier).await.map(|_| ())?;
  }

  if global_state.flags.lock_write {
    if let Some(ref lockfile) = global_state.lockfile {
      let g = lockfile.lock().unwrap();
      g.write()?;
    } else {
      eprintln!("--lock flag must be specified when using --lock-write");
      std::process::exit(11);
    }
  }

  Ok(())
}

async fn eval_command(flags: DenoFlags, code: String) -> Result<(), ErrBox> {
  // Force TypeScript compile.
  let main_module =
    ModuleSpecifier::resolve_url_or_path("./__$deno$eval.ts").unwrap();
  let global_state = GlobalState::new(flags)?;
  let mut worker = create_main_worker(global_state, main_module.clone());

  debug!("main_module {}", &main_module);

  worker.execute_module_from_code(&main_module, code).await?;
  worker.execute("window.dispatchEvent(new Event('load'))")?;
  (&mut *worker).await?;
  worker.execute("window.dispatchEvent(new Event('unload'))")?;
  Ok(())
}

async fn bundle_command(
  flags: DenoFlags,
  source_file: String,
  out_file: Option<PathBuf>,
) -> Result<(), ErrBox> {
  let source_file_specifier =
    ModuleSpecifier::resolve_url_or_path(&source_file)?;
  let global_state = GlobalState::new(flags)?;
  let mut worker =
    create_main_worker(global_state.clone(), source_file_specifier.clone());

  // TODO(bartlomieju): no longer true?
  // NOTE: we need to poll `worker` otherwise TS compiler worker won't run properly
  (&mut *worker).await?;

  debug!(">>>>> bundle_async START");
  let bundle_result = global_state
    .ts_compiler
    .bundle_async(
      global_state.clone(),
      source_file_specifier.to_string(),
      out_file,
    )
    .await;
  debug!(">>>>> bundle_async END");
  bundle_result
}

async fn run_repl(flags: DenoFlags) -> Result<(), ErrBox> {
  let main_module =
    ModuleSpecifier::resolve_url_or_path("./__$deno$repl.ts").unwrap();
  let global_state = GlobalState::new(flags)?;
  let mut worker = create_main_worker(global_state, main_module);
  loop {
    (&mut *worker).await?;
  }
}

async fn run_command(flags: DenoFlags, script: String) -> Result<(), ErrBox> {
  let global_state = GlobalState::new(flags.clone())?;
  run_script(global_state.clone(), script).await?;
  if global_state.flags.lock_write {
    if let Some(ref lockfile) = global_state.lockfile {
      let g = lockfile.lock().unwrap();
      g.write()?;
    } else {
      eprintln!("--lock flag must be specified when using --lock-write");
      std::process::exit(11);
    }
  }
  Ok(())
}

async fn run_script(
  global_state: GlobalState,
  script: String,
) -> Result<(), ErrBox> {
  let main_module = ModuleSpecifier::resolve_url_or_path(&script).unwrap();
  let mut worker =
    create_main_worker(global_state.clone(), main_module.clone());
  debug!("main_module {}", main_module);
  worker.execute_module(&main_module).await?;
  worker.execute("window.dispatchEvent(new Event('load'))")?;
  (&mut *worker).await?;
  worker.execute("window.dispatchEvent(new Event('unload'))")?;
  Ok(())
}

async fn fmt_command(files: Vec<String>, check: bool) -> Result<(), ErrBox> {
  fmt::format_files(files, check)
}

async fn test_command(
  flags: DenoFlags,
  include: Option<Vec<String>>,
  fail_fast: bool,
  _quiet: bool,
  allow_none: bool,
) -> Result<(), ErrBox> {
  let global_state = GlobalState::new(flags.clone())?;
  let cwd = std::env::current_dir().expect("No current directory");
  let include = include.unwrap_or_else(|| vec![".".to_string()]);
  let test_modules =
    test_runner::prepare_test_modules_urls(include, cwd.clone())?;

  if test_modules.is_empty() {
    println!("No matching test modules found");
    // TODO(bartlomieju): replace with StaticError?
    if !allow_none {
      std::process::exit(1);
    }
    return Ok(());
  }

  let test_file = test_runner::render_test_file(test_modules, fail_fast);
  let test_file_path = cwd.join(".deno.test.ts");
  deno_fs::write_file(&test_file_path, test_file.as_bytes(), 0o666)
    .expect("Can't write test file");

  let mut flags = flags.clone();
  flags
    .argv
    .push(test_file_path.to_string_lossy().to_string());
  // TODO: call execute module manually here and delete test file immediately after?
  let result =
    run_script(global_state, test_file_path.to_string_lossy().to_string())
      .await;
  std_fs::remove_file(&test_file_path).expect("Failed to remove temp file");
  result
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

  let fut = match flags.clone().subcommand {
    DenoSubcommand::Bundle {
      source_file,
      out_file,
    } => bundle_command(flags, source_file, out_file).boxed_local(),
    DenoSubcommand::Eval { code } => eval_command(flags, code).boxed_local(),
    DenoSubcommand::Fetch { files } => {
      fetch_command(flags, files).boxed_local()
    }
    DenoSubcommand::Fmt { check, files } => {
      fmt_command(files, check).boxed_local()
    }
    DenoSubcommand::Info { file } => info_command(flags, file).boxed_local(),
    DenoSubcommand::Install {
      dir,
      exe_name,
      module_url,
      args,
      force,
    } => install_command(flags, dir, exe_name, module_url, args, force)
      .boxed_local(),
    DenoSubcommand::Repl => run_repl(flags).boxed_local(),
    DenoSubcommand::Run { script } => run_command(flags, script).boxed_local(),
    DenoSubcommand::Test {
      quiet,
      fail_fast,
      include,
      allow_none,
    } => {
      test_command(flags, include, fail_fast, quiet, allow_none).boxed_local()
    }
    DenoSubcommand::Completions { buf } => {
      print!("{}", std::str::from_utf8(&buf).unwrap());
      return;
    }
    DenoSubcommand::Types => {
      types_command();
      return;
    }
    _ => unreachable!(),
  };

  let result = tokio_util::run_basic(fut);
  if let Err(err) = result {
    eprintln!("{}", err.to_string());
    std::process::exit(1);
  }
}
