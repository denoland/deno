// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate log;

pub mod colors;
pub mod errors;
pub mod flags;
pub mod flags_allow_net;
pub mod fs_util;
pub mod http_util;
pub mod js;
pub mod metrics;
pub mod ops;
pub mod permissions;
pub mod program_state;
pub mod resolve_addr;
pub mod signal;
pub mod text_encoding;
pub mod tokio_util;
pub mod version;
pub mod web_worker;
pub mod worker;

#[cfg(feature = "tools")]
pub mod ast;
#[cfg(feature = "tools")]
pub mod checksum;
#[cfg(feature = "tools")]
pub mod deno_dir;
#[cfg(feature = "tools")]
pub mod diagnostics;
#[cfg(feature = "tools")]
pub mod diff;
#[cfg(feature = "tools")]
pub mod disk_cache;
#[cfg(feature = "tools")]
pub mod file_fetcher;
#[cfg(feature = "tools")]
pub mod file_watcher;
#[cfg(feature = "tools")]
pub mod fmt_errors;
#[cfg(feature = "tools")]
pub mod http_cache;
#[cfg(feature = "tools")]
pub mod import_map;
#[cfg(feature = "tools")]
pub mod info;
#[cfg(feature = "tools")]
pub mod inspector;
#[cfg(feature = "tools")]
pub mod lockfile;
#[cfg(feature = "tools")]
pub mod media_type;
#[cfg(feature = "tools")]
pub mod module_graph;
#[cfg(feature = "tools")]
pub mod module_loader;
#[cfg(feature = "tools")]
pub mod source_maps;
#[cfg(feature = "tools")]
pub mod specifier_handler;
#[cfg(feature = "tools")]
pub mod tools;
#[cfg(feature = "tools")]
pub mod tsc;
#[cfg(feature = "tools")]
pub mod tsc_config;

// Compiled only for "lite" binary
#[cfg(not(feature = "tools"))]
pub mod fs_module_loader;

use crate::permissions::Permissions;
use crate::program_state::ProgramState;
use crate::worker::MainWorker;
use deno_core::error::generic_error;
use deno_core::error::AnyError;
use deno_core::futures::future::Future;
use deno_core::futures::future::FutureExt;
use deno_core::ModuleSpecifier;
use flags::Flags;
use std::io::Read;
use std::io::Write;
use std::pin::Pin;

#[cfg(feature = "tools")]
use {
  crate::file_fetcher::File, crate::file_fetcher::FileFetcher,
  crate::file_watcher::ModuleResolutionResult, crate::import_map::ImportMap,
  crate::media_type::MediaType, crate::specifier_handler::FetchHandler,
  deno_core::serde_json, deno_core::serde_json::json, deno_doc as doc,
  deno_doc::parser::DocFileLoader, program_state::exit_unstable,
  std::cell::RefCell, std::path::PathBuf, std::rc::Rc, std::sync::Arc,
};

#[cfg(feature = "tools")]
pub fn write_to_stdout_ignore_sigpipe(
  bytes: &[u8],
) -> Result<(), std::io::Error> {
  use std::io::ErrorKind;

  match std::io::stdout().write_all(bytes) {
    Ok(()) => Ok(()),
    Err(e) => match e.kind() {
      ErrorKind::BrokenPipe => Ok(()),
      _ => Err(e),
    },
  }
}

#[cfg(feature = "tools")]
pub fn write_json_to_stdout<T>(value: &T) -> Result<(), AnyError>
where
  T: ?Sized + serde::ser::Serialize,
{
  let writer = std::io::BufWriter::new(std::io::stdout());
  serde_json::to_writer_pretty(writer, value).map_err(AnyError::from)
}

#[cfg(feature = "tools")]
pub fn print_cache_info(
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

#[cfg(feature = "tools")]
pub fn get_types(unstable: bool) -> String {
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

#[cfg(feature = "tools")]
pub async fn info_command(
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

#[cfg(feature = "tools")]
pub async fn install_command(
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

#[cfg(feature = "tools")]
pub async fn lint_command(
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

#[cfg(feature = "tools")]
pub async fn cache_command(
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

#[cfg(feature = "tools")]
pub async fn eval_command(
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

#[cfg(feature = "tools")]
pub async fn bundle_command(
  flags: Flags,
  source_file: String,
  out_file: Option<PathBuf>,
) -> Result<(), AnyError> {
  let debug = flags.log_level == Some(log::Level::Debug);

  let module_resolver = || {
    let flags = flags.clone();
    let source_file1 = source_file.clone();
    let source_file2 = source_file.clone();
    async move {
      let module_specifier =
        ModuleSpecifier::resolve_url_or_path(&source_file1)?;

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
    .map(move |result| match result {
      Ok((paths_to_watch, module_graph)) => ModuleResolutionResult::Success {
        paths_to_watch,
        module_info: module_graph,
      },
      Err(e) => ModuleResolutionResult::Fail {
        source_path: PathBuf::from(source_file2),
        error: e,
      },
    })
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
    let module_graph = match module_resolver().await {
      ModuleResolutionResult::Fail { error, .. } => return Err(error),
      ModuleResolutionResult::Success { module_info, .. } => module_info,
    };
    operation(module_graph).await?;
  }

  Ok(())
}

#[cfg(feature = "tools")]
pub struct DocLoader {
  fetcher: FileFetcher,
  maybe_import_map: Option<ImportMap>,
}

#[cfg(feature = "tools")]
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

#[cfg(feature = "tools")]
pub async fn doc_command(
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

#[cfg(feature = "tools")]
pub async fn format_command(
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

#[cfg(feature = "tools")]
pub async fn run_repl(flags: Flags) -> Result<(), AnyError> {
  let main_module =
    ModuleSpecifier::resolve_url_or_path("./$deno$repl.ts").unwrap();
  let permissions = Permissions::from_flags(&flags);
  let program_state = ProgramState::new(flags)?;
  let mut worker =
    MainWorker::new(&program_state, main_module.clone(), permissions);
  worker.run_event_loop().await?;

  tools::repl::run(&program_state, worker).await
}

#[cfg(feature = "tools")]
pub async fn run_from_stdin(flags: Flags) -> Result<(), AnyError> {
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

#[cfg(feature = "tools")]
pub async fn run_with_watch(
  flags: Flags,
  script: String,
) -> Result<(), AnyError> {
  let module_resolver = || {
    let script1 = script.clone();
    let script2 = script.clone();
    let flags = flags.clone();
    async move {
      let main_module = ModuleSpecifier::resolve_url_or_path(&script1)?;
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
    .map(move |result| match result {
      Ok((paths_to_watch, module_info)) => ModuleResolutionResult::Success {
        paths_to_watch,
        module_info,
      },
      Err(e) => ModuleResolutionResult::Fail {
        source_path: PathBuf::from(script2),
        error: e,
      },
    })
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

pub async fn run_command(flags: Flags, script: String) -> Result<(), AnyError> {
  // Read script content from stdin
  #[cfg(feature = "tools")]
  if script == "-" {
    return run_from_stdin(flags).await;
  }

  if flags.watch {
    #[cfg(feature = "tools")]
    return run_with_watch(flags, script).await;
    #[cfg(not(feature = "tools"))]
    unimplemented!()
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

#[cfg(feature = "tools")]
pub async fn test_command(
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
