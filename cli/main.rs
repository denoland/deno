// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
#![deny(warnings)]

extern crate dissimilar;
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
pub mod deno_dir;
pub mod diagnostics;
mod diff;
mod disk_cache;
mod doc;
mod file_fetcher;
pub mod flags;
mod fmt;
pub mod fmt_errors;
mod fs;
pub mod global_state;
mod global_timer;
pub mod http_cache;
mod http_util;
mod import_map;
mod inspector;
pub mod installer;
mod js;
mod lint;
mod lockfile;
mod metrics;
mod module_graph;
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
mod swc_util;
mod test_runner;
pub mod test_util;
mod tokio_util;
mod tsc;
mod upgrade;
pub mod version;
mod web_worker;
pub mod worker;

pub use dprint_plugin_typescript::swc_common;
pub use dprint_plugin_typescript::swc_ecma_ast;
pub use dprint_plugin_typescript::swc_ecma_parser;

use crate::doc::parser::DocFileLoader;
use crate::file_fetcher::SourceFile;
use crate::file_fetcher::SourceFileFetcher;
use crate::fs as deno_fs;
use crate::global_state::GlobalState;
use crate::msg::MediaType;
use crate::op_error::OpError;
use crate::permissions::Permissions;
use crate::tsc::TargetLib;
use crate::worker::MainWorker;
use deno_core::v8_set_flags;
use deno_core::ErrBox;
use deno_core::EsIsolate;
use deno_core::ModuleSpecifier;
use flags::DenoSubcommand;
use flags::Flags;
use futures::future::FutureExt;
use futures::Future;
use log::Level;
use log::Metadata;
use log::Record;
use std::env;
use std::io::Read;
use std::io::Write;
use std::path::PathBuf;
use std::pin::Pin;
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

fn write_lockfile(global_state: GlobalState) -> Result<(), std::io::Error> {
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
    .fetch_source_file(&module_specifier, None, Permissions::allow_all())
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
    .prepare_module_load(
      module_specifier_.clone(),
      None,
      TargetLib::Main,
      Permissions::allow_all(),
      false,
      global_state.maybe_import_map.clone(),
    )
    .await?;
  global_state
    .clone()
    .fetch_compiled_module(module_specifier_, None)
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

  let es_state_rc = EsIsolate::state(&worker.isolate);
  let es_state = es_state_rc.borrow();

  if let Some(deps) = es_state.modules.deps(&module_specifier) {
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

fn get_types(unstable: bool) -> String {
  if unstable {
    format!(
      "{}\n{}\n{}\n{}",
      crate::js::DENO_NS_LIB,
      crate::js::SHARED_GLOBALS_LIB,
      crate::js::WINDOW_LIB,
      crate::js::UNSTABLE_NS_LIB,
    )
  } else {
    format!(
      "{}\n{}\n{}",
      crate::js::DENO_NS_LIB,
      crate::js::SHARED_GLOBALS_LIB,
      crate::js::WINDOW_LIB,
    )
  }
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
  let mut worker = MainWorker::create(global_state, main_module.clone())?;
  worker.preload_module(&main_module).await?;
  print_file_info(&worker, main_module.clone()).await
}

async fn install_command(
  flags: Flags,
  module_url: String,
  args: Vec<String>,
  name: Option<String>,
  root: Option<PathBuf>,
  force: bool,
) -> Result<(), ErrBox> {
  // Firstly fetch and compile module, this step ensures that module exists.
  let mut fetch_flags = flags.clone();
  fetch_flags.reload = true;
  let global_state = GlobalState::new(fetch_flags)?;
  let main_module = ModuleSpecifier::resolve_url_or_path(&module_url)?;
  let mut worker = MainWorker::create(global_state, main_module.clone())?;
  worker.preload_module(&main_module).await?;
  installer::install(flags, &module_url, args, name, root, force)
    .map_err(ErrBox::from)
}

async fn lint_command(
  flags: Flags,
  files: Vec<String>,
  list_rules: bool,
) -> Result<(), ErrBox> {
  let global_state = GlobalState::new(flags)?;

  // TODO(bartlomieju): refactor, it's non-sense to create
  // state just to perform unstable check...
  use crate::state::State;
  let state = State::new(
    global_state,
    None,
    ModuleSpecifier::resolve_url("file:///dummy.ts").unwrap(),
    None,
    true,
  )?;

  state.check_unstable("lint");

  if list_rules {
    lint::print_rules_list();
    return Ok(());
  }

  lint::lint_files(files).await
}

async fn cache_command(flags: Flags, files: Vec<String>) -> Result<(), ErrBox> {
  let main_module =
    ModuleSpecifier::resolve_url_or_path("./__$deno$fetch.ts").unwrap();
  let global_state = GlobalState::new(flags)?;
  let mut worker =
    MainWorker::create(global_state.clone(), main_module.clone())?;

  for file in files {
    let specifier = ModuleSpecifier::resolve_url_or_path(&file)?;
    worker.preload_module(&specifier).await.map(|_| ())?;
  }

  write_lockfile(global_state)?;

  Ok(())
}

async fn eval_command(
  flags: Flags,
  code: String,
  as_typescript: bool,
  print: bool,
) -> Result<(), ErrBox> {
  // Force TypeScript compile.
  let main_module =
    ModuleSpecifier::resolve_url_or_path("./__$deno$eval.ts").unwrap();
  let global_state = GlobalState::new(flags)?;
  let mut worker = MainWorker::create(global_state, main_module.clone())?;
  let main_module_url = main_module.as_url().to_owned();
  // Create a dummy source file.
  let source_code = if print {
    "console.log(".to_string() + &code + ")"
  } else {
    code.clone()
  }
  .into_bytes();

  let source_file = SourceFile {
    filename: main_module_url.to_file_path().unwrap(),
    url: main_module_url,
    types_url: None,
    types_header: None,
    media_type: if as_typescript {
      MediaType::TypeScript
    } else {
      MediaType::JavaScript
    },
    source_code,
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
  let mut module_specifier =
    ModuleSpecifier::resolve_url_or_path(&source_file)?;
  let url = module_specifier.as_url();

  // TODO(bartlomieju): fix this hack in ModuleSpecifier
  if url.scheme() == "file" {
    let a = deno_fs::normalize_path(&url.to_file_path().unwrap());
    let u = Url::from_file_path(a).unwrap();
    module_specifier = ModuleSpecifier::from(u)
  }

  debug!(">>>>> bundle START");
  let compiler_config = tsc::CompilerConfig::load(flags.config_path.clone())?;

  let global_state = GlobalState::new(flags)?;

  info!("Bundling {}", module_specifier.to_string());

  let output = tsc::bundle(
    &global_state,
    compiler_config,
    module_specifier,
    global_state.maybe_import_map.clone(),
    global_state.flags.unstable,
  )
  .await?;

  debug!(">>>>> bundle END");

  if let Some(out_file_) = out_file.as_ref() {
    info!("Emitting bundle to {:?}", out_file_);
    let output_bytes = output.as_bytes();
    let output_len = output_bytes.len();
    deno_fs::write_file(out_file_, output_bytes, 0o666)?;
    info!("{} emitted.", human_size(output_len as f64));
  } else {
    println!("{}", output);
  }
  Ok(())
}

fn human_size(bytse: f64) -> String {
  let negative = if bytse.is_sign_positive() { "" } else { "-" };
  let bytse = bytse.abs();
  let units = ["Bytes", "KB", "MB", "GB", "TB", "PB", "EB", "ZB", "YB"];
  if bytse < 1_f64 {
    return format!("{}{} {}", negative, bytse, "Bytes");
  }
  let delimiter = 1024_f64;
  let exponent = std::cmp::min(
    (bytse.ln() / delimiter.ln()).floor() as i32,
    (units.len() - 1) as i32,
  );
  let pretty_bytes = format!("{:.2}", bytse / delimiter.powi(exponent))
    .parse::<f64>()
    .unwrap()
    * 1_f64;
  let unit = units[exponent as usize];
  format!("{}{} {}", negative, pretty_bytes, unit)
}

#[test]
fn human_size_test() {
  assert_eq!(human_size(16_f64), "16 Bytes");
  assert_eq!(human_size((16 * 1024) as f64), "16 KB");
  assert_eq!(human_size((16 * 1024 * 1024) as f64), "16 MB");
  assert_eq!(human_size(16_f64 * 1024_f64.powf(3.0)), "16 GB");
  assert_eq!(human_size(16_f64 * 1024_f64.powf(4.0)), "16 TB");
  assert_eq!(human_size(16_f64 * 1024_f64.powf(5.0)), "16 PB");
  assert_eq!(human_size(16_f64 * 1024_f64.powf(6.0)), "16 EB");
  assert_eq!(human_size(16_f64 * 1024_f64.powf(7.0)), "16 ZB");
  assert_eq!(human_size(16_f64 * 1024_f64.powf(8.0)), "16 YB");
}

async fn doc_command(
  flags: Flags,
  source_file: Option<String>,
  json: bool,
  maybe_filter: Option<String>,
) -> Result<(), ErrBox> {
  let global_state = GlobalState::new(flags.clone())?;
  let source_file = source_file.unwrap_or_else(|| "--builtin".to_string());

  impl DocFileLoader for SourceFileFetcher {
    fn load_source_code(
      &self,
      specifier: &str,
    ) -> Pin<Box<dyn Future<Output = Result<String, OpError>>>> {
      let specifier =
        ModuleSpecifier::resolve_url_or_path(specifier).expect("Bad specifier");
      let fetcher = self.clone();

      async move {
        let source_file = fetcher
          .fetch_source_file(&specifier, None, Permissions::allow_all())
          .await?;
        String::from_utf8(source_file.source_code)
          .map_err(|_| OpError::other("failed to parse".to_string()))
      }
      .boxed_local()
    }
  }

  let loader = Box::new(global_state.file_fetcher.clone());
  let doc_parser = doc::DocParser::new(loader);

  let parse_result = if source_file == "--builtin" {
    doc_parser.parse_source("lib.deno.d.ts", get_types(flags.unstable).as_str())
  } else {
    let module_specifier =
      ModuleSpecifier::resolve_url_or_path(&source_file).unwrap();
    doc_parser
      .parse_with_reexports(&module_specifier.to_string())
      .await
  };

  let doc_nodes = match parse_result {
    Ok(nodes) => nodes,
    Err(e) => {
      eprintln!("{}", e);
      std::process::exit(1);
    }
  };

  if json {
    let writer = std::io::BufWriter::new(std::io::stdout());
    serde_json::to_writer_pretty(writer, &doc_nodes).map_err(ErrBox::from)
  } else {
    let details = if let Some(filter) = maybe_filter {
      let nodes =
        doc::find_nodes_by_name_recursively(doc_nodes, filter.clone());
      if nodes.is_empty() {
        eprintln!("Node {} was not found!", filter);
        std::process::exit(1);
      }
      let mut details = String::new();
      for node in nodes {
        details.push_str(doc::printer::format_details(node).as_str());
      }
      details
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
  let mut worker = MainWorker::create(global_state, main_module)?;
  loop {
    (&mut *worker).await?;
  }
}

async fn run_command(flags: Flags, script: String) -> Result<(), ErrBox> {
  let global_state = GlobalState::new(flags.clone())?;
  let main_module = if script != "-" {
    ModuleSpecifier::resolve_url_or_path(&script).unwrap()
  } else {
    ModuleSpecifier::resolve_url_or_path("./__$deno$stdin.ts").unwrap()
  };
  let mut worker =
    MainWorker::create(global_state.clone(), main_module.clone())?;
  if script == "-" {
    let mut source = Vec::new();
    std::io::stdin().read_to_end(&mut source)?;
    let main_module_url = main_module.as_url().to_owned();
    // Create a dummy source file.
    let source_file = SourceFile {
      filename: main_module_url.to_file_path().unwrap(),
      url: main_module_url,
      types_url: None,
      types_header: None,
      media_type: MediaType::TypeScript,
      source_code: source,
    };
    // Save our fake file into file fetcher cache
    // to allow module access by TS compiler (e.g. op_fetch_source_files)
    worker
      .state
      .borrow()
      .global_state
      .file_fetcher
      .save_source_file_in_cache(&main_module, source_file);
  };

  debug!("main_module {}", main_module);
  worker.execute_module(&main_module).await?;
  write_lockfile(global_state)?;
  worker.execute("window.dispatchEvent(new Event('load'))")?;
  (&mut *worker).await?;
  worker.execute("window.dispatchEvent(new Event('unload'))")?;
  Ok(())
}

async fn test_command(
  flags: Flags,
  include: Option<Vec<String>>,
  fail_fast: bool,
  quiet: bool,
  allow_none: bool,
  filter: Option<String>,
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
  let test_file =
    test_runner::render_test_file(test_modules, fail_fast, quiet, filter);
  let main_module =
    ModuleSpecifier::resolve_url(&test_file_url.to_string()).unwrap();
  let mut worker =
    MainWorker::create(global_state.clone(), main_module.clone())?;
  // Create a dummy source file.
  let source_file = SourceFile {
    filename: test_file_url.to_file_path().unwrap(),
    url: test_file_url,
    types_url: None,
    types_header: None,
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
      print,
      code,
      as_typescript,
    } => eval_command(flags, code, as_typescript, print).boxed_local(),
    DenoSubcommand::Cache { files } => {
      cache_command(flags, files).boxed_local()
    }
    DenoSubcommand::Fmt { check, files } => {
      fmt::format(files, check).boxed_local()
    }
    DenoSubcommand::Info { file } => info_command(flags, file).boxed_local(),
    DenoSubcommand::Install {
      module_url,
      args,
      name,
      root,
      force,
    } => {
      install_command(flags, module_url, args, name, root, force).boxed_local()
    }
    DenoSubcommand::Lint { files, rules } => {
      lint_command(flags, files, rules).boxed_local()
    }
    DenoSubcommand::Repl => run_repl(flags).boxed_local(),
    DenoSubcommand::Run { script } => run_command(flags, script).boxed_local(),
    DenoSubcommand::Test {
      fail_fast,
      quiet,
      include,
      allow_none,
      filter,
    } => test_command(flags, include, fail_fast, quiet, allow_none, filter)
      .boxed_local(),
    DenoSubcommand::Completions { buf } => {
      if let Err(e) = write_to_stdout_ignore_sigpipe(&buf) {
        eprintln!("{}", e);
        std::process::exit(1);
      }
      return;
    }
    DenoSubcommand::Types => {
      let types = get_types(flags.unstable);
      if let Err(e) = write_to_stdout_ignore_sigpipe(types.as_bytes()) {
        eprintln!("{}", e);
        std::process::exit(1);
      }
      return;
    }
    DenoSubcommand::Upgrade {
      force,
      dry_run,
      version,
    } => upgrade_command(dry_run, force, version).boxed_local(),
    _ => unreachable!(),
  };

  let result = tokio_util::run_basic(fut);
  if let Err(err) = result {
    let msg = format!(
      "{}: {}",
      colors::red_bold("error".to_string()),
      err.to_string(),
    );
    eprintln!("{}", msg);
    std::process::exit(1);
  }
}
