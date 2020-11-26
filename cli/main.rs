// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

#![deny(warnings)]

#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate log;

mod ast;
mod checksum;
mod colors;
mod deno_dir;
mod diagnostics;
mod diff;
mod disk_cache;
mod errors;
mod file_fetcher;
mod file_watcher;
mod flags;
mod flags_allow_net;
mod fmt_errors;
mod fs_util;
mod http_cache;
mod http_util;
mod import_map;
mod info;
mod inspector;
mod js;
mod lockfile;
mod media_type;
mod metrics;
mod module_graph;
mod module_loader;
mod ops;
mod permissions;
mod program_state;
mod resolve_addr;
mod signal;
mod source_maps;
mod specifier_handler;
mod text_encoding;
mod tokio_util;
mod tools;
mod tsc;
mod tsc_config;
mod version;
mod web_worker;
mod worker;

use crate::file_fetcher::File;
use crate::file_fetcher::FileFetcher;
use crate::media_type::MediaType;
use crate::permissions::Permissions;
use crate::program_state::ProgramState;
use crate::specifier_handler::FetchHandler;
use crate::worker::MainWorker;
use deno_core::error::generic_error;
use deno_core::error::AnyError;
use deno_core::futures::future::FutureExt;
use deno_core::futures::Future;
use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_core::v8_set_flags;
use deno_core::ModuleSpecifier;
use deno_doc as doc;
use deno_doc::parser::DocFileLoader;
use flags::DenoSubcommand;
use flags::Flags;
use import_map::ImportMap;
use log::Level;
use log::LevelFilter;
use program_state::exit_unstable;
use std::cell::RefCell;
use std::env;
use std::io::Read;
use std::io::Write;
use std::iter::once;
use std::path::PathBuf;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Arc;

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
  state: &Arc<ProgramState>,
  json: bool,
) -> Result<(), AnyError> {
  let deno_dir = &state.dir.root;
  let modules_cache = &state.file_fetcher.get_http_cache_location();
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
  maybe_specifier: Option<String>,
  json: bool,
) -> Result<(), AnyError> {
  if json && !flags.unstable {
    exit_unstable("--json");
  }
  let program_state = ProgramState::new(flags)?;
  if let Some(specifier) = maybe_specifier {
    let specifier = ModuleSpecifier::resolve_url_or_path(&specifier)?;
    let handler = Rc::new(RefCell::new(specifier_handler::FetchHandler::new(
      &program_state,
      // info accesses dynamically imported modules just for their information
      // so we allow access to all of them.
      Permissions::allow_all(),
    )?));
    let mut builder = module_graph::GraphBuilder::new(
      handler,
      program_state.maybe_import_map.clone(),
      program_state.lockfile.clone(),
    );
    builder.add(&specifier, false).await?;
    let graph = builder.get_graph();
    let info = graph.info()?;

    if json {
      write_json_to_stdout(&json!(info))?;
    } else {
      write_to_stdout_ignore_sigpipe(info.to_string().as_bytes())?;
    }
    Ok(())
  } else {
    // If it was just "deno info" print location of caches and exit
    print_cache_info(&program_state, json)
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
  let mut preload_flags = flags.clone();
  preload_flags.inspect = None;
  preload_flags.inspect_brk = None;
  let permissions = Permissions::from_flags(&preload_flags);
  let program_state = ProgramState::new(preload_flags)?;
  let main_module = ModuleSpecifier::resolve_url_or_path(&module_url)?;
  let mut worker =
    MainWorker::new(&program_state, main_module.clone(), permissions);
  // First, fetch and compile the module; this step ensures that the module exists.
  worker.preload_module(&main_module).await?;
  tools::installer::install(flags, &module_url, args, name, root, force)
}

async fn lint_command(
  flags: Flags,
  files: Vec<PathBuf>,
  list_rules: bool,
  ignore: Vec<PathBuf>,
  json: bool,
) -> Result<(), AnyError> {
  if !flags.unstable {
    exit_unstable("lint");
  }

  if list_rules {
    tools::lint::print_rules_list(json);
    return Ok(());
  }

  tools::lint::lint_files(files, ignore, json).await
}

async fn cache_command(
  flags: Flags,
  files: Vec<String>,
) -> Result<(), AnyError> {
  let lib = if flags.unstable {
    module_graph::TypeLib::UnstableDenoWindow
  } else {
    module_graph::TypeLib::DenoWindow
  };
  let program_state = ProgramState::new(flags)?;

  for file in files {
    let specifier = ModuleSpecifier::resolve_url_or_path(&file)?;
    program_state
      .prepare_module_load(
        specifier,
        lib.clone(),
        Permissions::allow_all(),
        false,
        program_state.maybe_import_map.clone(),
      )
      .await?;
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
  let permissions = Permissions::from_flags(&flags);
  let program_state = ProgramState::new(flags)?;
  let mut worker =
    MainWorker::new(&program_state, main_module.clone(), permissions);
  let main_module_url = main_module.as_url().to_owned();
  // Create a dummy source file.
  let source_code = if print {
    format!("console.log({})", code)
  } else {
    code
  }
  .into_bytes();

  let file = File {
    local: main_module_url.to_file_path().unwrap(),
    maybe_types: None,
    media_type: if as_typescript {
      MediaType::TypeScript
    } else {
      MediaType::JavaScript
    },
    source: String::from_utf8(source_code)?,
    specifier: ModuleSpecifier::from(main_module_url),
  };

  // Save our fake file into file fetcher cache
  // to allow module access by TS compiler.
  program_state.file_fetcher.insert_cached(file);
  debug!("main_module {}", &main_module);
  worker.execute_module(&main_module).await?;
  worker.execute("window.dispatchEvent(new Event('load'))")?;
  worker.run_event_loop().await?;
  worker.execute("window.dispatchEvent(new Event('unload'))")?;
  Ok(())
}

async fn bundle_command(
  flags: Flags,
  source_file: String,
  out_file: Option<PathBuf>,
) -> Result<(), AnyError> {
  let debug = flags.log_level == Some(log::Level::Debug);

  let module_resolver = || {
    let flags = flags.clone();
    let source_file = source_file.clone();
    async move {
      let module_specifier =
        ModuleSpecifier::resolve_url_or_path(&source_file)?;

      debug!(">>>>> bundle START");
      let program_state = ProgramState::new(flags.clone())?;

      info!(
        "{} {}",
        colors::green("Bundle"),
        module_specifier.to_string()
      );

      let handler = Rc::new(RefCell::new(FetchHandler::new(
        &program_state,
        // when bundling, dynamic imports are only access for their type safety,
        // therefore we will allow the graph to access any module.
        Permissions::allow_all(),
      )?));
      let mut builder = module_graph::GraphBuilder::new(
        handler,
        program_state.maybe_import_map.clone(),
        program_state.lockfile.clone(),
      );
      builder.add(&module_specifier, false).await?;
      let module_graph = builder.get_graph();

      if !flags.no_check {
        // TODO(@kitsonk) support bundling for workers
        let lib = if flags.unstable {
          module_graph::TypeLib::UnstableDenoWindow
        } else {
          module_graph::TypeLib::DenoWindow
        };
        let result_info =
          module_graph.clone().check(module_graph::CheckOptions {
            debug,
            emit: false,
            lib,
            maybe_config_path: flags.config_path.clone(),
            reload: flags.reload,
          })?;

        debug!("{}", result_info.stats);
        if let Some(ignored_options) = result_info.maybe_ignored_options {
          eprintln!("{}", ignored_options);
        }
        if !result_info.diagnostics.is_empty() {
          return Err(generic_error(result_info.diagnostics.to_string()));
        }
      }

      let mut paths_to_watch: Vec<PathBuf> = module_graph
        .get_modules()
        .iter()
        .filter_map(|specifier| specifier.as_url().to_file_path().ok())
        .collect();

      if let Some(import_map) = program_state.flags.import_map_path.as_ref() {
        paths_to_watch
          .push(fs_util::resolve_from_cwd(std::path::Path::new(import_map))?);
      }

      Ok((paths_to_watch, module_graph))
    }
    .boxed_local()
  };

  let operation = |module_graph: module_graph::Graph| {
    let flags = flags.clone();
    let out_file = out_file.clone();
    async move {
      let (output, stats, maybe_ignored_options) =
        module_graph.bundle(module_graph::BundleOptions {
          debug,
          maybe_config_path: flags.config_path,
        })?;

      match maybe_ignored_options {
        Some(ignored_options) if flags.no_check => {
          eprintln!("{}", ignored_options);
        }
        _ => {}
      }
      debug!("{}", stats);

      debug!(">>>>> bundle END");

      if let Some(out_file) = out_file.as_ref() {
        let output_bytes = output.as_bytes();
        let output_len = output_bytes.len();
        fs_util::write_file(out_file, output_bytes, 0o644)?;
        info!(
          "{} {:?} ({})",
          colors::green("Emit"),
          out_file,
          colors::gray(&info::human_size(output_len as f64))
        );
      } else {
        println!("{}", output);
      }

      Ok(())
    }
    .boxed_local()
  };

  if flags.watch {
    file_watcher::watch_func_with_module_resolution(
      module_resolver,
      operation,
      "Bundle",
    )
    .await?;
  } else {
    let (_, module_graph) = module_resolver().await?;
    operation(module_graph).await?;
  }

  Ok(())
}

struct DocLoader {
  fetcher: FileFetcher,
  maybe_import_map: Option<ImportMap>,
}

impl DocFileLoader for DocLoader {
  fn resolve(
    &self,
    specifier: &str,
    referrer: &str,
  ) -> Result<String, doc::DocError> {
    let maybe_resolved =
      if let Some(import_map) = self.maybe_import_map.as_ref() {
        import_map
          .resolve(specifier, referrer)
          .map_err(|e| doc::DocError::Resolve(e.to_string()))?
      } else {
        None
      };

    let resolved_specifier = if let Some(resolved) = maybe_resolved {
      resolved
    } else {
      ModuleSpecifier::resolve_import(specifier, referrer)
        .map_err(|e| doc::DocError::Resolve(e.to_string()))?
    };

    Ok(resolved_specifier.to_string())
  }

  fn load_source_code(
    &self,
    specifier: &str,
  ) -> Pin<Box<dyn Future<Output = Result<String, doc::DocError>>>> {
    let fetcher = self.fetcher.clone();
    let specifier = ModuleSpecifier::resolve_url_or_path(specifier)
      .expect("Expected valid specifier");
    async move {
      let source_file = fetcher
        .fetch(&specifier, &Permissions::allow_all())
        .await
        .map_err(|e| {
          doc::DocError::Io(std::io::Error::new(
            std::io::ErrorKind::Other,
            e.to_string(),
          ))
        })?;
      Ok(source_file.source)
    }
    .boxed_local()
  }
}

async fn doc_command(
  flags: Flags,
  source_file: Option<String>,
  json: bool,
  maybe_filter: Option<String>,
  private: bool,
) -> Result<(), AnyError> {
  let program_state = ProgramState::new(flags.clone())?;
  let source_file = source_file.unwrap_or_else(|| "--builtin".to_string());

  let loader = Box::new(DocLoader {
    fetcher: program_state.file_fetcher.clone(),
    maybe_import_map: program_state.maybe_import_map.clone(),
  });
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

async fn format_command(
  flags: Flags,
  args: Vec<PathBuf>,
  ignore: Vec<PathBuf>,
  check: bool,
) -> Result<(), AnyError> {
  if args.len() == 1 && args[0].to_string_lossy() == "-" {
    return tools::fmt::format_stdin(check);
  }

  tools::fmt::format(args, ignore, check, flags.watch).await?;
  Ok(())
}

async fn run_repl(flags: Flags) -> Result<(), AnyError> {
  let main_module =
    ModuleSpecifier::resolve_url_or_path("./$deno$repl.ts").unwrap();
  let permissions = Permissions::from_flags(&flags);
  let program_state = ProgramState::new(flags)?;
  let mut worker =
    MainWorker::new(&program_state, main_module.clone(), permissions);
  worker.run_event_loop().await?;

  tools::repl::run(&program_state, worker).await
}

async fn run_from_stdin(flags: Flags) -> Result<(), AnyError> {
  let program_state = ProgramState::new(flags.clone())?;
  let permissions = Permissions::from_flags(&flags);
  let main_module =
    ModuleSpecifier::resolve_url_or_path("./$deno$stdin.ts").unwrap();
  let mut worker =
    MainWorker::new(&program_state.clone(), main_module.clone(), permissions);

  let mut source = Vec::new();
  std::io::stdin().read_to_end(&mut source)?;
  let main_module_url = main_module.as_url().to_owned();
  // Create a dummy source file.
  let source_file = File {
    local: main_module_url.to_file_path().unwrap(),
    maybe_types: None,
    media_type: MediaType::TypeScript,
    source: String::from_utf8(source)?,
    specifier: main_module.clone(),
  };
  // Save our fake file into file fetcher cache
  // to allow module access by TS compiler
  program_state.file_fetcher.insert_cached(source_file);

  debug!("main_module {}", main_module);
  worker.execute_module(&main_module).await?;
  worker.execute("window.dispatchEvent(new Event('load'))")?;
  worker.run_event_loop().await?;
  worker.execute("window.dispatchEvent(new Event('unload'))")?;
  Ok(())
}

async fn run_with_watch(flags: Flags, script: String) -> Result<(), AnyError> {
  let module_resolver = || {
    let script = script.clone();
    let flags = flags.clone();
    async move {
      let main_module = ModuleSpecifier::resolve_url_or_path(&script)?;
      let program_state = ProgramState::new(flags)?;
      let handler = Rc::new(RefCell::new(FetchHandler::new(
        &program_state,
        Permissions::allow_all(),
      )?));
      let mut builder = module_graph::GraphBuilder::new(
        handler,
        program_state.maybe_import_map.clone(),
        program_state.lockfile.clone(),
      );
      builder.add(&main_module, false).await?;
      let module_graph = builder.get_graph();

      // Find all local files in graph
      let mut paths_to_watch: Vec<PathBuf> = module_graph
        .get_modules()
        .iter()
        .filter_map(|specifier| specifier.as_url().to_file_path().ok())
        .collect();

      if let Some(import_map) = program_state.flags.import_map_path.as_ref() {
        paths_to_watch
          .push(fs_util::resolve_from_cwd(std::path::Path::new(import_map))?);
      }

      Ok((paths_to_watch, main_module))
    }
    .boxed_local()
  };

  let operation = |main_module: ModuleSpecifier| {
    let flags = flags.clone();
    let permissions = Permissions::from_flags(&flags);
    async move {
      let main_module = main_module.clone();
      let program_state = ProgramState::new(flags)?;
      let mut worker =
        MainWorker::new(&program_state, main_module.clone(), permissions);
      debug!("main_module {}", main_module);
      worker.execute_module(&main_module).await?;
      worker.execute("window.dispatchEvent(new Event('load'))")?;
      worker.run_event_loop().await?;
      worker.execute("window.dispatchEvent(new Event('unload'))")?;
      Ok(())
    }
    .boxed_local()
  };

  file_watcher::watch_func_with_module_resolution(
    module_resolver,
    operation,
    "Process",
  )
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
  let program_state = ProgramState::new(flags.clone())?;
  let permissions = Permissions::from_flags(&flags);
  let mut worker =
    MainWorker::new(&program_state, main_module.clone(), permissions);
  debug!("main_module {}", main_module);
  worker.execute_module(&main_module).await?;
  worker.execute("window.dispatchEvent(new Event('load'))")?;
  worker.run_event_loop().await?;
  worker.execute("window.dispatchEvent(new Event('unload'))")?;
  Ok(())
}

async fn test_command(
  flags: Flags,
  include: Option<Vec<String>>,
  no_run: bool,
  fail_fast: bool,
  quiet: bool,
  allow_none: bool,
  filter: Option<String>,
) -> Result<(), AnyError> {
  let program_state = ProgramState::new(flags.clone())?;
  let permissions = Permissions::from_flags(&flags);
  let cwd = std::env::current_dir().expect("No current directory");
  let include = include.unwrap_or_else(|| vec![".".to_string()]);
  let test_modules =
    tools::test_runner::prepare_test_modules_urls(include, &cwd)?;

  if test_modules.is_empty() {
    println!("No matching test modules found");
    if !allow_none {
      std::process::exit(1);
    }
    return Ok(());
  }
  let main_module = ModuleSpecifier::resolve_path("$deno$test.ts")?;
  // Create a dummy source file.
  let source_file = File {
    local: main_module.as_url().to_file_path().unwrap(),
    maybe_types: None,
    media_type: MediaType::TypeScript,
    source: tools::test_runner::render_test_file(
      test_modules.clone(),
      fail_fast,
      quiet,
      filter,
    ),
    specifier: main_module.clone(),
  };
  // Save our fake file into file fetcher cache
  // to allow module access by TS compiler
  program_state.file_fetcher.insert_cached(source_file);

  if no_run {
    let lib = if flags.unstable {
      module_graph::TypeLib::UnstableDenoWindow
    } else {
      module_graph::TypeLib::DenoWindow
    };
    program_state
      .prepare_module_load(
        main_module.clone(),
        lib,
        Permissions::allow_all(),
        false,
        program_state.maybe_import_map.clone(),
      )
      .await?;
    return Ok(());
  }

  let mut worker =
    MainWorker::new(&program_state, main_module.clone(), permissions);

  let mut maybe_coverage_collector = if flags.coverage {
    let session = worker.create_inspector_session();
    let mut coverage_collector =
      tools::coverage::CoverageCollector::new(session);
    coverage_collector.start_collecting().await?;

    Some(coverage_collector)
  } else {
    None
  };

  let execute_result = worker.execute_module(&main_module).await;
  execute_result?;
  worker.execute("window.dispatchEvent(new Event('load'))")?;
  worker.run_event_loop().await?;
  worker.execute("window.dispatchEvent(new Event('unload'))")?;
  worker.run_event_loop().await?;

  if let Some(coverage_collector) = maybe_coverage_collector.as_mut() {
    let coverages = coverage_collector.collect().await?;
    coverage_collector.stop_collecting().await?;

    let filtered_coverages = tools::coverage::filter_script_coverages(
      coverages,
      main_module.as_url().clone(),
      test_modules,
    );

    let mut coverage_reporter =
      tools::coverage::PrettyCoverageReporter::new(quiet);
    for coverage in filtered_coverages {
      coverage_reporter.visit_coverage(&coverage);
    }
  }

  Ok(())
}

fn init_v8_flags(v8_flags: &[String]) {
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
    eprintln!("\nFor a list of V8 flags, use '--v8-flags=--help'");
    std::process::exit(1);
  }
  if v8_flags_includes_help {
    std::process::exit(0);
  }
}

fn init_logger(maybe_level: Option<Level>) {
  let log_level = match maybe_level {
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
}

fn get_subcommand(
  flags: Flags,
) -> Pin<Box<dyn Future<Output = Result<(), AnyError>>>> {
  match flags.clone().subcommand {
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
    } => format_command(flags, files, ignore, check).boxed_local(),
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
      no_run,
      fail_fast,
      quiet,
      include,
      allow_none,
      filter,
    } => {
      test_command(flags, include, no_run, fail_fast, quiet, allow_none, filter)
        .boxed_local()
    }
    DenoSubcommand::Completions { buf } => {
      if let Err(e) = write_to_stdout_ignore_sigpipe(&buf) {
        eprintln!("{}", e);
        std::process::exit(1);
      }
      std::process::exit(0);
    }
    DenoSubcommand::Types => {
      let types = get_types(flags.unstable);
      if let Err(e) = write_to_stdout_ignore_sigpipe(types.as_bytes()) {
        eprintln!("{}", e);
        std::process::exit(1);
      }
      std::process::exit(0);
    }
    DenoSubcommand::Upgrade {
      force,
      dry_run,
      canary,
      version,
      output,
      ca_file,
    } => tools::upgrade::upgrade_command(
      dry_run, force, canary, version, output, ca_file,
    )
    .boxed_local(),
  }
}

pub fn main() {
  #[cfg(windows)]
  colors::enable_ansi(); // For Windows 10

  let args: Vec<String> = env::args().collect();
  let flags = flags::flags_from_vec(args);

  if let Some(ref v8_flags) = flags.v8_flags {
    init_v8_flags(v8_flags);
  }
  init_logger(flags.log_level);

  let subcommand_future = get_subcommand(flags);
  let result = tokio_util::run_basic(subcommand_future);
  if let Err(err) = result {
    eprintln!("{}: {}", colors::red_bold("error"), err.to_string());
    std::process::exit(1);
  }
}
