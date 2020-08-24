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
extern crate encoding_rs;
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
mod file_fetcher;
pub mod flags;
mod flags_allow_net;
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
mod op_fetch_asset;
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
mod text_encoding;
mod tokio_util;
mod tsc;
mod upgrade;
pub mod version;
mod web_worker;
pub mod worker;

use crate::file_fetcher::map_file_extension;
use crate::file_fetcher::SourceFile;
use crate::file_fetcher::SourceFileFetcher;
use crate::file_fetcher::TextDocument;
use crate::fs as deno_fs;
use crate::global_state::GlobalState;
use crate::msg::MediaType;
use crate::permissions::Permissions;
use crate::tsc::TargetLib;
use crate::worker::MainWorker;
use deno_core::v8_set_flags;
use deno_core::Deps;
use deno_core::ErrBox;
use deno_core::EsIsolate;
use deno_core::ModuleSpecifier;
use deno_doc as doc;
use deno_doc::parser::DocFileLoader;
use flags::DenoSubcommand;
use flags::Flags;
use futures::future::FutureExt;
use futures::Future;
use log::Level;
use log::Metadata;
use log::Record;
use state::exit_unstable;
use std::env;
use std::io::Read;
use std::io::Write;
use std::iter::once;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
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

fn write_json_to_stdout<T>(value: &T) -> Result<(), ErrBox>
where
  T: ?Sized + serde::ser::Serialize,
{
  let writer = std::io::BufWriter::new(std::io::stdout());
  serde_json::to_writer_pretty(writer, value).map_err(ErrBox::from)
}

fn print_cache_info(
  state: &Arc<GlobalState>,
  json: bool,
) -> Result<(), ErrBox> {
  let deno_dir = &state.dir.root;
  let modules_cache = &state.file_fetcher.http_cache.location;
  let typescript_cache = &state.dir.gen_cache.location;
  if json {
    let output = json!({
        "denoDir": deno_dir,
        "modulesCache": modules_cache,
        "typescriptCache": typescript_cache,
    });
    write_json_to_stdout(&output)
  } else {
    println!("{} {:?}", colors::bold("DENO_DIR location:"), deno_dir);
    println!(
      "{} {:?}",
      colors::bold("Remote modules cache:"),
      modules_cache
    );
    println!(
      "{} {:?}",
      colors::bold("TypeScript compiler cache:"),
      typescript_cache
    );
    Ok(())
  }
}

struct FileInfoOutput<'a> {
  local: &'a str,
  file_type: &'a str,
  compiled: Option<String>,
  map: Option<String>,
  deps: Option<Deps>,
}

// TODO(bartlomieju): this function de facto repeats
// whole compilation stack. Can this be done better somehow?
async fn print_file_info(
  worker: &MainWorker,
  module_specifier: ModuleSpecifier,
  json: bool,
) -> Result<(), ErrBox> {
  let global_state = worker.state.global_state.clone();

  let out = global_state
    .file_fetcher
    .fetch_source_file(&module_specifier, None, Permissions::allow_all())
    .await?;

  let mut output = FileInfoOutput {
    local: out.filename.to_str().unwrap(),
    file_type: msg::enum_name_media_type(out.media_type),
    compiled: None,
    map: None,
    deps: None,
  };

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
    output.compiled =
      compiled_source_file.filename.to_str().map(|s| s.to_owned());
  }

  if let Ok(source_map) = global_state
    .clone()
    .ts_compiler
    .get_source_map_file(&module_specifier)
  {
    output.map = source_map.filename.to_str().map(|s| s.to_owned());
  }
  let es_state_rc = EsIsolate::state(&worker.isolate);
  let es_state = es_state_rc.borrow();

  if let Some(deps) = es_state.modules.deps(&module_specifier) {
    output.deps = Some(deps);
  }

  if json {
    let output = json!({
      "local": output.local,
      "fileType": output.file_type,
      "compiled": output.compiled,
      "map": output.map,
      "deps": output.deps.map(|x| x.to_json())
    });
    write_json_to_stdout(&output)
  } else {
    println!("{} {}", colors::bold("local:"), output.local);
    println!("{} {}", colors::bold("type:"), output.file_type);
    if let Some(compiled) = output.compiled {
      println!("{} {}", colors::bold("compiled:"), compiled);
    }
    if let Some(map) = output.map {
      println!("{} {}", colors::bold("map:"), map);
    }
    if let Some(deps) = output.deps {
      println!("{}{}", colors::bold("deps:\n"), deps.name);
      if let Some(ref depsdeps) = deps.deps {
        for d in depsdeps {
          println!("{}", d);
        }
      }
    } else {
      println!(
        "{} cannot retrieve full dependency graph",
        colors::bold("deps:"),
      );
    }
    Ok(())
  }
}

fn get_types(unstable: bool) -> String {
  let mut types = format!(
    "{}\n{}\n{}\n{}",
    crate::js::DENO_NS_LIB,
    crate::js::DENO_WEB_LIB,
    crate::js::SHARED_GLOBALS_LIB,
    crate::js::WINDOW_LIB,
  );

  if unstable {
    types.push_str(&format!("\n{}", crate::js::UNSTABLE_NS_LIB,));
  }

  types
}

async fn info_command(
  flags: Flags,
  file: Option<String>,
  json: bool,
) -> Result<(), ErrBox> {
  if json && !flags.unstable {
    exit_unstable("--json");
  }
  let global_state = GlobalState::new(flags)?;
  // If it was just "deno info" print location of caches and exit
  if file.is_none() {
    print_cache_info(&global_state, json)
  } else {
    let main_module = ModuleSpecifier::resolve_url_or_path(&file.unwrap())?;
    let mut worker = MainWorker::create(&global_state, main_module.clone())?;
    worker.preload_module(&main_module).await?;
    print_file_info(&worker, main_module.clone(), json).await
  }
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
  let mut worker = MainWorker::create(&global_state, main_module.clone())?;
  worker.preload_module(&main_module).await?;
  installer::install(flags, &module_url, args, name, root, force)
    .map_err(ErrBox::from)
}

async fn lint_command(
  flags: Flags,
  files: Vec<String>,
  list_rules: bool,
  ignore: Vec<String>,
  json: bool,
) -> Result<(), ErrBox> {
  if !flags.unstable {
    exit_unstable("lint");
  }

  if list_rules {
    lint::print_rules_list();
    return Ok(());
  }

  lint::lint_files(files, ignore, json).await
}

async fn cache_command(flags: Flags, files: Vec<String>) -> Result<(), ErrBox> {
  let main_module =
    ModuleSpecifier::resolve_url_or_path("./__$deno$fetch.ts").unwrap();
  let global_state = GlobalState::new(flags)?;
  let mut worker = MainWorker::create(&global_state, main_module.clone())?;

  for file in files {
    let specifier = ModuleSpecifier::resolve_url_or_path(&file)?;
    worker.preload_module(&specifier).await.map(|_| ())?;
  }

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
  let mut worker = MainWorker::create(&global_state, main_module.clone())?;
  let main_module_url = main_module.as_url().to_owned();
  // Create a dummy source file.
  let source_code = if print {
    format!("console.log({})", code)
  } else {
    code
  }
  .into_bytes();

  let source_file = SourceFile {
    filename: main_module_url.to_file_path().unwrap(),
    url: main_module_url,
    types_header: None,
    media_type: if as_typescript {
      MediaType::TypeScript
    } else {
      MediaType::JavaScript
    },
    source_code: TextDocument::new(source_code, Some("utf-8")),
  };
  // Save our fake file into file fetcher cache
  // to allow module access by TS compiler.
  global_state
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
  let module_specifier = ModuleSpecifier::resolve_url_or_path(&source_file)?;

  debug!(">>>>> bundle START");
  let global_state = GlobalState::new(flags)?;

  info!(
    "{} {}",
    colors::green("Bundle"),
    module_specifier.to_string()
  );

  let output = global_state
    .ts_compiler
    .bundle(&global_state, module_specifier)
    .await?;

  debug!(">>>>> bundle END");

  if let Some(out_file_) = out_file.as_ref() {
    let output_bytes = output.as_bytes();
    let output_len = output_bytes.len();
    deno_fs::write_file(out_file_, output_bytes, 0o666)?;
    info!(
      "{} {:?} ({})",
      colors::green("Emit"),
      out_file_,
      colors::gray(&human_size(output_len as f64))
    );
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
  private: bool,
) -> Result<(), ErrBox> {
  let global_state = GlobalState::new(flags.clone())?;
  let source_file = source_file.unwrap_or_else(|| "--builtin".to_string());

  impl DocFileLoader for SourceFileFetcher {
    fn resolve(
      &self,
      specifier: &str,
      referrer: &str,
    ) -> Result<String, doc::DocError> {
      ModuleSpecifier::resolve_import(specifier, referrer)
        .map(|specifier| specifier.to_string())
        .map_err(|e| doc::DocError::Resolve(e.to_string()))
    }

    fn load_source_code(
      &self,
      specifier: &str,
    ) -> Pin<Box<dyn Future<Output = Result<String, doc::DocError>>>> {
      let fetcher = self.clone();
      let specifier = ModuleSpecifier::resolve_url_or_path(specifier)
        .expect("Expected valid specifier");
      async move {
        let source_file = fetcher
          .fetch_source_file(&specifier, None, Permissions::allow_all())
          .await
          .map_err(|e| {
            doc::DocError::Io(std::io::Error::new(
              std::io::ErrorKind::Other,
              e.to_string(),
            ))
          })?;
        source_file.source_code.to_string().map_err(|e| {
          doc::DocError::Io(std::io::Error::new(
            std::io::ErrorKind::Other,
            e.to_string(),
          ))
        })
      }
      .boxed_local()
    }
  }

  let loader = Box::new(global_state.file_fetcher.clone());
  let doc_parser = doc::DocParser::new(loader, private);

  let parse_result = if source_file == "--builtin" {
    let syntax = swc_util::get_syntax_for_dts();
    doc_parser.parse_source(
      "lib.deno.d.ts",
      syntax,
      get_types(flags.unstable).as_str(),
    )
  } else {
    let path = PathBuf::from(&source_file);
    let syntax = if path.ends_with("d.ts") {
      swc_util::get_syntax_for_dts()
    } else {
      let media_type = map_file_extension(&path);
      swc_util::get_syntax_for_media_type(media_type)
    };
    let module_specifier =
      ModuleSpecifier::resolve_url_or_path(&source_file).unwrap();
    doc_parser
      .parse_with_reexports(&module_specifier.to_string(), syntax)
      .await
  };

  let mut doc_nodes = match parse_result {
    Ok(nodes) => nodes,
    Err(e) => {
      eprintln!("{}", e);
      std::process::exit(1);
    }
  };

  if json {
    write_json_to_stdout(&doc_nodes)
  } else {
    doc_nodes.retain(|doc_node| doc_node.kind != doc::DocNodeKind::Import);
    let details = if let Some(filter) = maybe_filter {
      let nodes =
        doc::find_nodes_by_name_recursively(doc_nodes, filter.clone());
      if nodes.is_empty() {
        eprintln!("Node {} was not found!", filter);
        std::process::exit(1);
      }
      format!(
        "{}",
        doc::DocPrinter::new(&nodes, colors::use_color(), private)
      )
    } else {
      format!(
        "{}",
        doc::DocPrinter::new(&doc_nodes, colors::use_color(), private)
      )
    };

    write_to_stdout_ignore_sigpipe(details.as_bytes()).map_err(ErrBox::from)
  }
}

async fn run_repl(flags: Flags) -> Result<(), ErrBox> {
  let main_module =
    ModuleSpecifier::resolve_url_or_path("./__$deno$repl.ts").unwrap();
  let global_state = GlobalState::new(flags)?;
  let mut worker = MainWorker::create(&global_state, main_module)?;
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
    MainWorker::create(&global_state.clone(), main_module.clone())?;
  if script == "-" {
    let mut source = Vec::new();
    std::io::stdin().read_to_end(&mut source)?;
    let main_module_url = main_module.as_url().to_owned();
    // Create a dummy source file.
    let source_file = SourceFile {
      filename: main_module_url.to_file_path().unwrap(),
      url: main_module_url,
      types_header: None,
      media_type: MediaType::TypeScript,
      source_code: source.into(),
    };
    // Save our fake file into file fetcher cache
    // to allow module access by TS compiler
    global_state
      .file_fetcher
      .save_source_file_in_cache(&main_module, source_file);
  };

  debug!("main_module {}", main_module);
  worker.execute_module(&main_module).await?;
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
  let mut worker = MainWorker::create(&global_state, main_module.clone())?;
  // Create a dummy source file.
  let source_file = SourceFile {
    filename: test_file_url.to_file_path().unwrap(),
    url: test_file_url,
    types_header: None,
    media_type: MediaType::TypeScript,
    source_code: TextDocument::new(
      test_file.clone().into_bytes(),
      Some("utf-8"),
    ),
  };
  // Save our fake file into file fetcher cache
  // to allow module access by TS compiler
  global_state
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
    let v8_flags_includes_help = v8_flags
      .iter()
      .any(|flag| flag == "-help" || flag == "--help");
    let v8_flags = once("UNUSED_BUT_NECESSARY_ARG0".to_owned())
      .chain(v8_flags.iter().cloned())
      .collect::<Vec<_>>();
    let unrecognized_v8_flags = v8_set_flags(v8_flags)
      .into_iter()
      .skip(1)
      .collect::<Vec<_>>();
    if !unrecognized_v8_flags.is_empty() {
      for f in unrecognized_v8_flags {
        eprintln!("error: V8 did not recognize flag '{}'", f);
      }
      eprintln!();
      eprintln!("For a list of V8 flags, use '--v8-flags=--help'");
      std::process::exit(1);
    }
    if v8_flags_includes_help {
      std::process::exit(0);
    }
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
      private,
    } => doc_command(flags, source_file, json, filter, private).boxed_local(),
    DenoSubcommand::Eval {
      print,
      code,
      as_typescript,
    } => eval_command(flags, code, as_typescript, print).boxed_local(),
    DenoSubcommand::Cache { files } => {
      cache_command(flags, files).boxed_local()
    }
    DenoSubcommand::Fmt {
      check,
      files,
      ignore,
    } => fmt::format(files, check, ignore).boxed_local(),
    DenoSubcommand::Info { file, json } => {
      info_command(flags, file, json).boxed_local()
    }
    DenoSubcommand::Install {
      module_url,
      args,
      name,
      root,
      force,
    } => {
      install_command(flags, module_url, args, name, root, force).boxed_local()
    }
    DenoSubcommand::Lint {
      files,
      rules,
      ignore,
      json,
    } => lint_command(flags, files, rules, ignore, json).boxed_local(),
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
      output,
      ca_file,
    } => {
      upgrade_command(dry_run, force, version, output, ca_file).boxed_local()
    }
    _ => unreachable!(),
  };

  let result = tokio_util::run_basic(fut);
  if let Err(err) = result {
    let msg = format!("{}: {}", colors::red_bold("error"), err.to_string(),);
    eprintln!("{}", msg);
    std::process::exit(1);
  }
}
