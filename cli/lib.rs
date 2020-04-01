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
extern crate regex;
extern crate reqwest;
extern crate serde;
extern crate serde_derive;
extern crate tokio;
extern crate url;

mod checksum;
pub mod colors;
pub mod compilers;
pub mod deno_dir;
pub mod diagnostics;
mod disk_cache;
mod doc;
mod file_fetcher;
pub mod flags;
mod fmt;
pub mod fmt_errors;
mod fs;
mod global_state;
mod global_timer;
pub mod http_cache;
mod http_util;
mod import_map;
mod inspector;
pub mod installer;
mod js;
mod lockfile;
mod metrics;
pub mod msg;
pub mod op_error;
pub mod ops;
pub mod permissions;
mod repl;
pub mod resolve_addr;
pub mod signal;
pub mod source_maps;
mod startup_data;
pub mod state;
mod test_runner;
pub mod test_util;
mod tokio_util;
mod upgrade;
pub mod version;
mod web_worker;
pub mod worker;

use crate::compilers::TargetLib;
use crate::file_fetcher::SourceFile;
use crate::global_state::GlobalState;
use crate::msg::MediaType;
use crate::ops::io::get_stdio;
use crate::state::State;
use crate::worker::MainWorker;
use deno_core::v8_set_flags;
use deno_core::ErrBox;
use deno_core::ModuleSpecifier;
use flags::DenoSubcommand;
use flags::Flags;
use futures::future::FutureExt;
use log::Level;
use log::Metadata;
use log::Record;
use std::env;
use std::io::Write;
use std::path::PathBuf;
use upgrade::upgrade_command;
use url::Url;

static LOGGER: Logger = Logger;

// TODO(ry) Switch to env_logger or other standard crate.
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

      if record.level() >= Level::Info {
        eprintln!("{}", record.args());
      } else {
        eprintln!("{} RS - {} - {}", record.level(), target, record.args());
      }
    }
  }
  fn flush(&self) {}
}

fn write_to_stdout_ignore_sigpipe(bytes: &[u8]) -> Result<(), std::io::Error> {
  use std::io::ErrorKind;

  match std::io::stdout().write_all(bytes) {
    Ok(()) => Ok(()),
    Err(e) => match e.kind() {
      ErrorKind::BrokenPipe => Ok(()),
      _ => Err(e),
    },
  }
}

fn create_main_worker(
  global_state: GlobalState,
  main_module: ModuleSpecifier,
) -> Result<MainWorker, ErrBox> {
  let state = State::new(global_state, None, main_module)?;

  {
    let mut s = state.borrow_mut();
    let (stdin, stdout, stderr) = get_stdio();
    s.resource_table.add("stdin", Box::new(stdin));
    s.resource_table.add("stdout", Box::new(stdout));
    s.resource_table.add("stderr", Box::new(stderr));
  }

  let mut worker = MainWorker::new(
    "main".to_string(),
    startup_data::deno_isolate_init(),
    state,
  );
  worker.execute("bootstrapMainRuntime()")?;
  Ok(worker)
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
    state.file_fetcher.http_cache.location
  );
  println!(
    "{} {:?}",
    colors::bold("TypeScript compiler cache:".to_string()),
    state.dir.gen_cache.location
  );
}

// TODO(bartlomieju): this function de facto repeats
// whole compilation stack. Can this be done better somehow?
async fn print_file_info(
  worker: &MainWorker,
  module_specifier: ModuleSpecifier,
) -> Result<(), ErrBox> {
  let global_state = worker.state.borrow().global_state.clone();

  let out = global_state
    .file_fetcher
    .fetch_source_file(&module_specifier, None)
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
  flags: Flags,
  file: Option<String>,
) -> Result<(), ErrBox> {
  let global_state = GlobalState::new(flags)?;
  // If it was just "deno info" print location of caches and exit
  if file.is_none() {
    print_cache_info(&global_state);
    return Ok(());
  }

  let main_module = ModuleSpecifier::resolve_url_or_path(&file.unwrap())?;
  let mut worker = create_main_worker(global_state, main_module.clone())?;
  worker.preload_module(&main_module).await?;
  print_file_info(&worker, main_module.clone()).await
}

async fn install_command(
  flags: Flags,
  dir: Option<PathBuf>,
  exe_name: String,
  module_url: String,
  args: Vec<String>,
  force: bool,
) -> Result<(), ErrBox> {
  // Firstly fetch and compile module, this step ensures that module exists.
  let mut fetch_flags = flags.clone();
  fetch_flags.reload = true;
  let global_state = GlobalState::new(fetch_flags)?;
  let main_module = ModuleSpecifier::resolve_url_or_path(&module_url)?;
  let mut worker = create_main_worker(global_state, main_module.clone())?;
  worker.preload_module(&main_module).await?;
  installer::install(flags, dir, &exe_name, &module_url, args, force)
    .map_err(ErrBox::from)
}

async fn fetch_command(flags: Flags, files: Vec<String>) -> Result<(), ErrBox> {
  let main_module =
    ModuleSpecifier::resolve_url_or_path("./__$deno$fetch.ts").unwrap();
  let global_state = GlobalState::new(flags)?;
  let mut worker =
    create_main_worker(global_state.clone(), main_module.clone())?;

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

async fn eval_command(
  flags: Flags,
  code: String,
  as_typescript: bool,
) -> Result<(), ErrBox> {
  // Force TypeScript compile.
  let main_module =
    ModuleSpecifier::resolve_url_or_path("./__$deno$eval.ts").unwrap();
  let global_state = GlobalState::new(flags)?;
  let mut worker = create_main_worker(global_state, main_module.clone())?;
  let main_module_url = main_module.as_url().to_owned();
  // Create a dummy source file.
  let source_file = SourceFile {
    filename: main_module_url.to_file_path().unwrap(),
    url: main_module_url,
    types_url: None,
    media_type: if as_typescript {
      MediaType::TypeScript
    } else {
      MediaType::JavaScript
    },
    source_code: code.clone().into_bytes(),
  };
  // Save our fake file into file fetcher cache
  // to allow module access by TS compiler (e.g. op_fetch_source_files)
  worker
    .state
    .borrow()
    .global_state
    .file_fetcher
    .save_source_file_in_cache(&main_module, source_file);
  debug!("main_module {}", &main_module);
  worker.execute_module(&main_module).await?;
  worker.execute("window.dispatchEvent(new Event('load'))")?;
  (&mut *worker).await?;
  worker.execute("window.dispatchEvent(new Event('unload'))")?;
  Ok(())
}

async fn bundle_command(
  flags: Flags,
  source_file: String,
  out_file: Option<PathBuf>,
) -> Result<(), ErrBox> {
  let module_name = ModuleSpecifier::resolve_url_or_path(&source_file)?;
  let global_state = GlobalState::new(flags)?;
  debug!(">>>>> bundle START");
  let bundle_result = global_state
    .ts_compiler
    .bundle(global_state.clone(), module_name.to_string(), out_file)
    .await;
  debug!(">>>>> bundle END");
  bundle_result
}

async fn doc_command(
  flags: Flags,
  source_file: String,
  json: bool,
  maybe_filter: Option<String>,
) -> Result<(), ErrBox> {
  let global_state = GlobalState::new(flags.clone())?;
  let module_specifier =
    ModuleSpecifier::resolve_url_or_path(&source_file).unwrap();
  let source_file = global_state
    .file_fetcher
    .fetch_source_file(&module_specifier, None)
    .await?;
  let source_code = String::from_utf8(source_file.source_code)?;

  let doc_parser = doc::DocParser::default();
  let parse_result =
    doc_parser.parse(module_specifier.to_string(), source_code);

  let doc_nodes = match parse_result {
    Ok(nodes) => nodes,
    Err(e) => {
      eprintln!("Failed to parse documentation:");
      for diagnostic in e {
        eprintln!("{}", diagnostic.message());
      }

      std::process::exit(1);
    }
  };

  if json {
    let writer = std::io::BufWriter::new(std::io::stdout());
    serde_json::to_writer_pretty(writer, &doc_nodes).map_err(ErrBox::from)
  } else {
    let details = if let Some(filter) = maybe_filter {
      let node = doc::find_node_by_name_recursively(doc_nodes, filter.clone());
      if let Some(node) = node {
        doc::printer::format_details(node)
      } else {
        eprintln!("Node {} was not found!", filter);
        std::process::exit(1);
      }
    } else {
      doc::printer::format(doc_nodes)
    };

    write_to_stdout_ignore_sigpipe(details.as_bytes()).map_err(ErrBox::from)
  }
}

async fn run_repl(flags: Flags) -> Result<(), ErrBox> {
  let main_module =
    ModuleSpecifier::resolve_url_or_path("./__$deno$repl.ts").unwrap();
  let global_state = GlobalState::new(flags)?;
  let mut worker = create_main_worker(global_state, main_module)?;
  loop {
    (&mut *worker).await?;
  }
}

async fn run_command(flags: Flags, script: String) -> Result<(), ErrBox> {
  let global_state = GlobalState::new(flags.clone())?;
  let main_module = ModuleSpecifier::resolve_url_or_path(&script).unwrap();
  let mut worker =
    create_main_worker(global_state.clone(), main_module.clone())?;
  debug!("main_module {}", main_module);
  worker.execute_module(&main_module).await?;
  worker.execute("window.dispatchEvent(new Event('load'))")?;
  (&mut *worker).await?;
  worker.execute("window.dispatchEvent(new Event('unload'))")?;
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

async fn test_command(
  flags: Flags,
  include: Option<Vec<String>>,
  fail_fast: bool,
  allow_none: bool,
) -> Result<(), ErrBox> {
  let global_state = GlobalState::new(flags.clone())?;
  let cwd = std::env::current_dir().expect("No current directory");
  let include = include.unwrap_or_else(|| vec![".".to_string()]);
  let test_modules = test_runner::prepare_test_modules_urls(include, &cwd)?;

  if test_modules.is_empty() {
    println!("No matching test modules found");
    if !allow_none {
      std::process::exit(1);
    }
    return Ok(());
  }

  let test_file_path = cwd.join(".deno.test.ts");
  let test_file_url =
    Url::from_file_path(&test_file_path).expect("Should be valid file url");
  let test_file = test_runner::render_test_file(test_modules, fail_fast);
  let main_module =
    ModuleSpecifier::resolve_url(&test_file_url.to_string()).unwrap();
  let mut worker =
    create_main_worker(global_state.clone(), main_module.clone())?;
  // Create a dummy source file.
  let source_file = SourceFile {
    filename: test_file_url.to_file_path().unwrap(),
    url: test_file_url,
    types_url: None,
    media_type: MediaType::TypeScript,
    source_code: test_file.clone().into_bytes(),
  };
  // Save our fake file into file fetcher cache
  // to allow module access by TS compiler (e.g. op_fetch_source_files)
  worker
    .state
    .borrow()
    .global_state
    .file_fetcher
    .save_source_file_in_cache(&main_module, source_file);
  let execute_result = worker.execute_module(&main_module).await;
  execute_result?;
  worker.execute("window.dispatchEvent(new Event('load'))")?;
  (&mut *worker).await?;
  worker.execute("window.dispatchEvent(new Event('unload'))")
}

pub fn main() {
  #[cfg(windows)]
  colors::enable_ansi(); // For Windows 10

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
    None => Level::Info, // Default log level
  };
  log::set_max_level(log_level.to_level_filter());

  let fut = match flags.clone().subcommand {
    DenoSubcommand::Bundle {
      source_file,
      out_file,
    } => bundle_command(flags, source_file, out_file).boxed_local(),
    DenoSubcommand::Doc {
      source_file,
      json,
      filter,
    } => doc_command(flags, source_file, json, filter).boxed_local(),
    DenoSubcommand::Eval {
      code,
      as_typescript,
    } => eval_command(flags, code, as_typescript).boxed_local(),
    DenoSubcommand::Fetch { files } => {
      fetch_command(flags, files).boxed_local()
    }
    DenoSubcommand::Fmt { check, files } => {
      async move { fmt::format(files, check) }.boxed_local()
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
      fail_fast,
      include,
      allow_none,
    } => test_command(flags, include, fail_fast, allow_none).boxed_local(),
    DenoSubcommand::Completions { buf } => {
      if let Err(e) = write_to_stdout_ignore_sigpipe(&buf) {
        eprintln!("{}", e);
        std::process::exit(1);
      }
      return;
    }
    DenoSubcommand::Types => {
      let types = format!(
        "{}\n{}\n{}",
        crate::js::DENO_NS_LIB,
        crate::js::SHARED_GLOBALS_LIB,
        crate::js::WINDOW_LIB
      );
      if let Err(e) = write_to_stdout_ignore_sigpipe(types.as_bytes()) {
        eprintln!("{}", e);
        std::process::exit(1);
      }
      return;
    }
    DenoSubcommand::Upgrade { force, dry_run } => {
      upgrade_command(dry_run, force).boxed_local()
    }
    _ => unreachable!(),
  };

  let result = tokio_util::run_basic(fut);
  if let Err(err) = result {
    eprintln!("{}", err.to_string());
    std::process::exit(1);
  }
}
