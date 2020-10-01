// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

#![deny(warnings)]

#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate log;

mod ast;
mod checksum;
pub mod colors;
mod coverage;
pub mod deno_dir;
pub mod diagnostics;
mod diff;
mod disk_cache;
pub mod errors;
mod file_fetcher;
mod file_watcher;
pub mod flags;
mod flags_allow_net;
mod fmt;
pub mod fmt_errors;
mod fs;
pub mod global_state;
mod global_timer;
mod graph;
pub mod http_cache;
mod http_util;
mod import_map;
mod info;
mod inspector;
pub mod installer;
mod js;
mod lint;
mod lockfile;
mod media_type;
mod metrics;
mod module_graph;
mod op_fetch_asset;
pub mod ops;
pub mod permissions;
mod repl;
pub mod resolve_addr;
pub mod signal;
pub mod source_maps;
mod specifier_handler;
pub mod state;
mod test_runner;
mod text_encoding;
mod tokio_util;
mod tsc;
mod tsc_config;
mod upgrade;
pub mod version;
pub mod worker;

use crate::coverage::CoverageCollector;
use crate::coverage::PrettyCoverageReporter;
use crate::file_fetcher::SourceFile;
use crate::file_fetcher::SourceFileFetcher;
use crate::file_fetcher::TextDocument;
use crate::fs as deno_fs;
use crate::global_state::GlobalState;
use crate::inspector::InspectorSession;
use crate::media_type::MediaType;
use crate::permissions::Permissions;
use crate::worker::MainWorker;
use deno_core::error::AnyError;
use deno_core::futures::future::FutureExt;
use deno_core::futures::Future;
use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_core::url::Url;
use deno_core::v8_set_flags;
use deno_core::ModuleSpecifier;
use deno_doc as doc;
use deno_doc::parser::DocFileLoader;
use flags::DenoSubcommand;
use flags::Flags;
use global_state::exit_unstable;
use log::Level;
use log::LevelFilter;
use std::env;
use std::io::Read;
use std::io::Write;
use std::iter::once;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use upgrade::upgrade_command;

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

fn write_json_to_stdout<T>(value: &T) -> Result<(), AnyError>
where
  T: ?Sized + serde::ser::Serialize,
{
  let writer = std::io::BufWriter::new(std::io::stdout());
  serde_json::to_writer_pretty(writer, value).map_err(AnyError::from)
}

fn print_cache_info(
  state: &Arc<GlobalState>,
  json: bool,
) -> Result<(), AnyError> {
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

fn get_types(unstable: bool) -> String {
  let mut types = format!(
    "{}\n{}\n{}\n{}\n{}",
    crate::js::DENO_NS_LIB,
    crate::js::DENO_WEB_LIB,
    crate::js::DENO_FETCH_LIB,
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
) -> Result<(), AnyError> {
  if json && !flags.unstable {
    exit_unstable("--json");
  }
  let global_state = GlobalState::new(flags)?;
  // If it was just "deno info" print location of caches and exit
  if file.is_none() {
    print_cache_info(&global_state, json)
  } else {
    let main_module = ModuleSpecifier::resolve_url_or_path(&file.unwrap())?;
    let info =
      info::ModuleDepInfo::new(&global_state, main_module.clone()).await?;

    if json {
      write_json_to_stdout(&json!(info))
    } else {
      write_to_stdout_ignore_sigpipe(format!("{}", info).as_bytes())
        .map_err(AnyError::from)
    }
  }
}

async fn install_command(
  flags: Flags,
  module_url: String,
  args: Vec<String>,
  name: Option<String>,
  root: Option<PathBuf>,
  force: bool,
) -> Result<(), AnyError> {
  let global_state = GlobalState::new(flags.clone())?;
  let main_module = ModuleSpecifier::resolve_url_or_path(&module_url)?;
  let mut worker = MainWorker::new(&global_state, main_module.clone());
  // First, fetch and compile the module; this step ensures that the module exists.
  worker.preload_module(&main_module).await?;
  installer::install(flags, &module_url, args, name, root, force)
}

async fn lint_command(
  flags: Flags,
  files: Vec<String>,
  list_rules: bool,
  ignore: Vec<String>,
  json: bool,
) -> Result<(), AnyError> {
  if !flags.unstable {
    exit_unstable("lint");
  }

  if list_rules {
    lint::print_rules_list();
    return Ok(());
  }

  lint::lint_files(files, ignore, json).await
}

async fn cache_command(
  flags: Flags,
  files: Vec<String>,
) -> Result<(), AnyError> {
  let main_module =
    ModuleSpecifier::resolve_url_or_path("./$deno$cache.ts").unwrap();
  let global_state = GlobalState::new(flags)?;
  let mut worker = MainWorker::new(&global_state, main_module.clone());

  for file in files {
    let specifier = ModuleSpecifier::resolve_url_or_path(&file)?;
    // TODO(bartlomieju): don't use `preload_module` in favor of calling "GlobalState::prepare_module_load()"
    // explicitly? Seems wasteful to create multiple worker just to run TS compiler
    worker.preload_module(&specifier).await.map(|_| ())?;
  }

  Ok(())
}

async fn eval_command(
  flags: Flags,
  code: String,
  as_typescript: bool,
  print: bool,
) -> Result<(), AnyError> {
  // Force TypeScript compile.
  let main_module =
    ModuleSpecifier::resolve_url_or_path("./$deno$eval.ts").unwrap();
  let global_state = GlobalState::new(flags)?;
  let mut worker = MainWorker::new(&global_state, main_module.clone());
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
) -> Result<(), AnyError> {
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
      colors::gray(&info::human_size(output_len as f64))
    );
  } else {
    println!("{}", output);
  }
  Ok(())
}

async fn doc_command(
  flags: Flags,
  source_file: Option<String>,
  json: bool,
  maybe_filter: Option<String>,
  private: bool,
) -> Result<(), AnyError> {
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
    let syntax = ast::get_syntax(&MediaType::Dts);
    doc_parser.parse_source(
      "lib.deno.d.ts",
      syntax,
      get_types(flags.unstable).as_str(),
    )
  } else {
    let path = PathBuf::from(&source_file);
    let media_type = MediaType::from(&path);
    let syntax = ast::get_syntax(&media_type);
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

    write_to_stdout_ignore_sigpipe(details.as_bytes()).map_err(AnyError::from)
  }
}

async fn run_repl(flags: Flags) -> Result<(), AnyError> {
  let main_module =
    ModuleSpecifier::resolve_url_or_path("./$deno$repl.ts").unwrap();
  let global_state = GlobalState::new(flags)?;
  let mut worker = MainWorker::new(&global_state, main_module.clone());
  (&mut *worker).await?;

  let inspector = worker
    .inspector
    .as_mut()
    .expect("Inspector is not created.");

  let inspector_session = InspectorSession::new(&mut **inspector);
  let repl = repl::run(&global_state, inspector_session);

  tokio::pin!(repl);

  loop {
    tokio::select! {
      result = &mut repl => {
          return result;
      }
      _ = &mut *worker => {}
    }
  }
}

async fn run_from_stdin(flags: Flags) -> Result<(), AnyError> {
  let global_state = GlobalState::new(flags.clone())?;
  let main_module =
    ModuleSpecifier::resolve_url_or_path("./$deno$stdin.ts").unwrap();
  let mut worker = MainWorker::new(&global_state.clone(), main_module.clone());

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

  debug!("main_module {}", main_module);
  worker.execute_module(&main_module).await?;
  worker.execute("window.dispatchEvent(new Event('load'))")?;
  (&mut *worker).await?;
  worker.execute("window.dispatchEvent(new Event('unload'))")?;
  Ok(())
}

async fn run_with_watch(flags: Flags, script: String) -> Result<(), AnyError> {
  let main_module = ModuleSpecifier::resolve_url_or_path(&script)?;
  let global_state = GlobalState::new(flags.clone())?;

  let mut module_graph_loader = module_graph::ModuleGraphLoader::new(
    global_state.file_fetcher.clone(),
    global_state.maybe_import_map.clone(),
    Permissions::allow_all(),
    false,
    false,
  );
  module_graph_loader.add_to_graph(&main_module, None).await?;
  let module_graph = module_graph_loader.get_graph();

  // Find all local files in graph
  let mut paths_to_watch: Vec<PathBuf> = module_graph
    .values()
    .map(|f| Url::parse(&f.url).unwrap())
    .filter(|url| url.scheme() == "file")
    .map(|url| url.to_file_path().unwrap())
    .collect();

  if let Some(import_map) = global_state.flags.import_map_path.clone() {
    paths_to_watch.push(
      Url::parse(&format!("file://{}", &import_map))?
        .to_file_path()
        .unwrap(),
    );
  }

  // FIXME(bartlomieju): new file watcher is created on after each restart
  file_watcher::watch_func(&paths_to_watch, move || {
    // FIXME(bartlomieju): GlobalState must be created on each restart - otherwise file fetcher
    // will use cached source files
    let gs = GlobalState::new(flags.clone()).unwrap();
    let main_module = main_module.clone();
    async move {
      let mut worker = MainWorker::new(&gs, main_module.clone());
      debug!("main_module {}", main_module);
      worker.execute_module(&main_module).await?;
      worker.execute("window.dispatchEvent(new Event('load'))")?;
      (&mut *worker).await?;
      worker.execute("window.dispatchEvent(new Event('unload'))")?;
      Ok(())
    }
    .boxed_local()
  })
  .await
}

async fn run_command(flags: Flags, script: String) -> Result<(), AnyError> {
  // Read script content from stdin
  if script == "-" {
    return run_from_stdin(flags).await;
  }

  if flags.watch {
    return run_with_watch(flags, script).await;
  }

  let main_module = ModuleSpecifier::resolve_url_or_path(&script)?;
  let global_state = GlobalState::new(flags.clone())?;
  let mut worker = MainWorker::new(&global_state, main_module.clone());
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
) -> Result<(), AnyError> {
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

  let test_file_path = cwd.join("$deno$test.ts");
  let test_file_url =
    Url::from_file_path(&test_file_path).expect("Should be valid file url");
  let test_file = test_runner::render_test_file(
    test_modules.clone(),
    fail_fast,
    quiet,
    filter,
  );
  let main_module =
    ModuleSpecifier::resolve_url(&test_file_url.to_string()).unwrap();
  let mut worker = MainWorker::new(&global_state, main_module.clone());
  // Create a dummy source file.
  let source_file = SourceFile {
    filename: test_file_url.to_file_path().unwrap(),
    url: test_file_url.clone(),
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

  let mut maybe_coverage_collector = if flags.coverage {
    let inspector = worker
      .inspector
      .as_mut()
      .expect("Inspector is not created.");

    let mut coverage_collector = CoverageCollector::new(&mut **inspector);
    coverage_collector.start_collecting().await?;

    Some(coverage_collector)
  } else {
    None
  };

  let execute_result = worker.execute_module(&main_module).await;
  execute_result?;
  worker.execute("window.dispatchEvent(new Event('load'))")?;
  (&mut *worker).await?;
  worker.execute("window.dispatchEvent(new Event('unload'))")?;
  (&mut *worker).await?;

  if let Some(coverage_collector) = maybe_coverage_collector.as_mut() {
    let coverages = coverage_collector.collect().await?;
    coverage_collector.stop_collecting().await?;

    let filtered_coverages =
      coverage::filter_script_coverages(coverages, test_file_url, test_modules);

    let mut coverage_reporter = PrettyCoverageReporter::new(quiet);
    for coverage in filtered_coverages {
      coverage_reporter.visit_coverage(&coverage);
    }
  }

  Ok(())
}

pub fn main() {
  #[cfg(windows)]
  colors::enable_ansi(); // For Windows 10

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
  env_logger::Builder::from_env(
    env_logger::Env::default()
      .default_filter_or(log_level.to_level_filter().to_string()),
  )
  // https://github.com/denoland/deno/issues/6641
  .filter_module("rustyline", LevelFilter::Off)
  .format(|buf, record| {
    let mut target = record.target().to_string();
    if let Some(line_no) = record.line() {
      target.push(':');
      target.push_str(&line_no.to_string());
    }
    if record.level() <= Level::Info {
      // Print ERROR, WARN, INFO logs as they are
      writeln!(buf, "{}", record.args())
    } else {
      // Add prefix to DEBUG or TRACE logs
      writeln!(
        buf,
        "{} RS - {} - {}",
        record.level(),
        target,
        record.args()
      )
    }
  })
  .init();

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
