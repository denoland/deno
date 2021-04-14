// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

#![deny(warnings)]

mod ast;
mod auth_tokens;
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
mod lockfile;
mod lsp;
mod media_type;
mod module_graph;
mod module_loader;
mod ops;
mod program_state;
mod source_maps;
mod specifier_handler;
mod standalone;
mod text_encoding;
mod tokio_util;
mod tools;
mod tsc;
mod tsc_config;
mod unix_util;
mod version;

use crate::file_fetcher::File;
use crate::file_watcher::ModuleResolutionResult;
use crate::flags::DenoSubcommand;
use crate::flags::Flags;
use crate::fmt_errors::PrettyJsError;
use crate::media_type::MediaType;
use crate::module_loader::CliModuleLoader;
use crate::program_state::ProgramState;
use crate::source_maps::apply_source_map;
use crate::specifier_handler::FetchHandler;
use crate::tools::installer::infer_name_from_url;
use deno_core::error::generic_error;
use deno_core::error::AnyError;
use deno_core::futures::future::FutureExt;
use deno_core::futures::Future;
use deno_core::resolve_url_or_path;
use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_core::v8_set_flags;
use deno_core::ModuleSpecifier;
use deno_runtime::ops::worker_host::CreateWebWorkerCb;
use deno_runtime::permissions::Permissions;
use deno_runtime::web_worker::WebWorker;
use deno_runtime::web_worker::WebWorkerOptions;
use deno_runtime::worker::MainWorker;
use deno_runtime::worker::WorkerOptions;
use log::debug;
use log::info;
use log::Level;
use log::LevelFilter;
use std::env;
use std::io::Read;
use std::io::Write;
use std::iter::once;
use std::path::PathBuf;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::Mutex;

fn create_web_worker_callback(
  program_state: Arc<ProgramState>,
) -> Arc<CreateWebWorkerCb> {
  Arc::new(move |args| {
    let global_state_ = program_state.clone();
    let js_error_create_fn = Rc::new(move |core_js_error| {
      let source_mapped_error =
        apply_source_map(&core_js_error, global_state_.clone());
      PrettyJsError::create(source_mapped_error)
    });

    let attach_inspector = program_state.maybe_inspector_server.is_some()
      || program_state.coverage_dir.is_some();
    let maybe_inspector_server = program_state.maybe_inspector_server.clone();

    let module_loader = CliModuleLoader::new_for_worker(
      program_state.clone(),
      args.parent_permissions.clone(),
    );
    let create_web_worker_cb =
      create_web_worker_callback(program_state.clone());

    let options = WebWorkerOptions {
      args: program_state.flags.argv.clone(),
      apply_source_maps: true,
      debug_flag: program_state
        .flags
        .log_level
        .map_or(false, |l| l == log::Level::Debug),
      unstable: program_state.flags.unstable,
      ca_data: program_state.ca_data.clone(),
      user_agent: version::get_user_agent(),
      seed: program_state.flags.seed,
      module_loader,
      create_web_worker_cb,
      js_error_create_fn: Some(js_error_create_fn),
      use_deno_namespace: args.use_deno_namespace,
      attach_inspector,
      maybe_inspector_server,
      runtime_version: version::deno(),
      ts_version: version::TYPESCRIPT.to_string(),
      no_color: !colors::use_color(),
      get_error_class_fn: Some(&crate::errors::get_error_class_name),
      blob_url_store: program_state.blob_url_store.clone(),
    };

    let mut worker = WebWorker::from_options(
      args.name,
      args.permissions,
      args.main_module,
      args.worker_id,
      &options,
    );

    // This block registers additional ops and state that
    // are only available in the CLI
    {
      let js_runtime = &mut worker.js_runtime;
      js_runtime
        .op_state()
        .borrow_mut()
        .put::<Arc<ProgramState>>(program_state.clone());
      // Applies source maps - works in conjuction with `js_error_create_fn`
      // above
      ops::errors::init(js_runtime);
      if args.use_deno_namespace {
        ops::runtime_compiler::init(js_runtime);
      }
    }
    worker.bootstrap(&options);

    worker
  })
}

pub fn create_main_worker(
  program_state: &Arc<ProgramState>,
  main_module: ModuleSpecifier,
  permissions: Permissions,
) -> MainWorker {
  let module_loader = CliModuleLoader::new(program_state.clone());

  let global_state_ = program_state.clone();

  let js_error_create_fn = Rc::new(move |core_js_error| {
    let source_mapped_error =
      apply_source_map(&core_js_error, global_state_.clone());
    PrettyJsError::create(source_mapped_error)
  });

  let attach_inspector = program_state.maybe_inspector_server.is_some()
    || program_state.flags.repl
    || program_state.coverage_dir.is_some();
  let maybe_inspector_server = program_state.maybe_inspector_server.clone();
  let should_break_on_first_statement =
    program_state.flags.inspect_brk.is_some();

  let create_web_worker_cb = create_web_worker_callback(program_state.clone());

  let options = WorkerOptions {
    apply_source_maps: true,
    args: program_state.flags.argv.clone(),
    debug_flag: program_state
      .flags
      .log_level
      .map_or(false, |l| l == log::Level::Debug),
    unstable: program_state.flags.unstable,
    ca_data: program_state.ca_data.clone(),
    user_agent: version::get_user_agent(),
    seed: program_state.flags.seed,
    js_error_create_fn: Some(js_error_create_fn),
    create_web_worker_cb,
    attach_inspector,
    maybe_inspector_server,
    should_break_on_first_statement,
    module_loader,
    runtime_version: version::deno(),
    ts_version: version::TYPESCRIPT.to_string(),
    no_color: !colors::use_color(),
    get_error_class_fn: Some(&crate::errors::get_error_class_name),
    location: program_state.flags.location.clone(),
    blob_url_store: program_state.blob_url_store.clone(),
  };

  let mut worker = MainWorker::from_options(main_module, permissions, &options);

  // This block registers additional ops and state that
  // are only available in the CLI
  {
    let js_runtime = &mut worker.js_runtime;
    js_runtime
      .op_state()
      .borrow_mut()
      .put::<Arc<ProgramState>>(program_state.clone());
    // Applies source maps - works in conjuction with `js_error_create_fn`
    // above
    ops::errors::init(js_runtime);
    ops::runtime_compiler::init(js_runtime);
  }
  worker.bootstrap(&options);

  worker
}

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

pub fn write_json_to_stdout<T>(value: &T) -> Result<(), AnyError>
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
  let registry_cache =
    &state.dir.root.join(lsp::language_server::REGISTRIES_PATH);
  if json {
    let output = json!({
        "denoDir": deno_dir,
        "modulesCache": modules_cache,
        "typescriptCache": typescript_cache,
        "registryCache": registry_cache,
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
      colors::bold("Emitted modules cache:"),
      typescript_cache
    );
    println!(
      "{} {:?}",
      colors::bold("Language server registries cache:"),
      registry_cache,
    );
    Ok(())
  }
}

pub fn get_types(unstable: bool) -> String {
  let mut types = format!(
    "{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}",
    crate::tsc::DENO_NS_LIB,
    crate::tsc::DENO_CONSOLE_LIB,
    crate::tsc::DENO_URL_LIB,
    crate::tsc::DENO_WEB_LIB,
    crate::tsc::DENO_FILE_LIB,
    crate::tsc::DENO_FETCH_LIB,
    crate::tsc::DENO_WEBGPU_LIB,
    crate::tsc::DENO_WEBSOCKET_LIB,
    crate::tsc::DENO_CRYPTO_LIB,
    crate::tsc::SHARED_GLOBALS_LIB,
    crate::tsc::WINDOW_LIB,
  );

  if unstable {
    types.push_str(&format!("\n{}", crate::tsc::UNSTABLE_NS_LIB,));
  }

  types
}

async fn compile_command(
  flags: Flags,
  source_file: String,
  output: Option<PathBuf>,
  args: Vec<String>,
  target: Option<String>,
  lite: bool,
) -> Result<(), AnyError> {
  let debug = flags.log_level == Some(log::Level::Debug);

  let run_flags =
    tools::standalone::compile_to_runtime_flags(flags.clone(), args)?;

  let module_specifier = resolve_url_or_path(&source_file)?;
  let program_state = ProgramState::build(flags.clone()).await?;
  let deno_dir = &program_state.dir;

  let output = output.or_else(|| {
    infer_name_from_url(&module_specifier).map(PathBuf::from)
  }).ok_or_else(|| generic_error(
    "An executable name was not provided. One could not be inferred from the URL. Aborting.",
  ))?;

  let module_graph = create_module_graph_and_maybe_check(
    module_specifier.clone(),
    program_state.clone(),
    debug,
  )
  .await?;

  info!(
    "{} {}",
    colors::green("Bundle"),
    module_specifier.to_string()
  );
  let bundle_str = bundle_module_graph(module_graph, flags, debug)?;

  info!(
    "{} {}",
    colors::green("Compile"),
    module_specifier.to_string()
  );

  // Select base binary based on `target` and `lite` arguments
  let original_binary =
    tools::standalone::get_base_binary(deno_dir, target.clone(), lite).await?;

  let final_bin = tools::standalone::create_standalone_binary(
    original_binary,
    bundle_str,
    run_flags,
  )?;

  info!("{} {}", colors::green("Emit"), output.display());

  tools::standalone::write_standalone_binary(output.clone(), target, final_bin)
    .await?;

  Ok(())
}

async fn info_command(
  flags: Flags,
  maybe_specifier: Option<String>,
  json: bool,
) -> Result<(), AnyError> {
  let program_state = ProgramState::build(flags).await?;
  if let Some(specifier) = maybe_specifier {
    let specifier = resolve_url_or_path(&specifier)?;
    let handler = Arc::new(Mutex::new(specifier_handler::FetchHandler::new(
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
      write_json_to_stdout(&json!(info))
    } else {
      write_to_stdout_ignore_sigpipe(info.to_string().as_bytes())
        .map_err(|err| err.into())
    }
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
  let permissions = Permissions::from_options(&preload_flags.clone().into());
  let program_state = ProgramState::build(preload_flags).await?;
  let main_module = resolve_url_or_path(&module_url)?;
  let mut worker =
    create_main_worker(&program_state, main_module.clone(), permissions);
  // First, fetch and compile the module; this step ensures that the module exists.
  worker.preload_module(&main_module).await?;
  tools::installer::install(flags, &module_url, args, name, root, force)
}

async fn lsp_command() -> Result<(), AnyError> {
  lsp::start().await
}

async fn lint_command(
  _flags: Flags,
  files: Vec<PathBuf>,
  list_rules: bool,
  ignore: Vec<PathBuf>,
  json: bool,
) -> Result<(), AnyError> {
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
  let program_state = ProgramState::build(flags).await?;

  for file in files {
    let specifier = resolve_url_or_path(&file)?;
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
  ext: String,
  print: bool,
) -> Result<(), AnyError> {
  // Force TypeScript compile.
  let main_module = resolve_url_or_path("./$deno$eval.ts").unwrap();
  let permissions = Permissions::from_options(&flags.clone().into());
  let program_state = ProgramState::build(flags).await?;
  let mut worker =
    create_main_worker(&program_state, main_module.clone(), permissions);
  // Create a dummy source file.
  let source_code = if print {
    format!("console.log({})", code)
  } else {
    code
  }
  .into_bytes();

  let file = File {
    local: main_module.clone().to_file_path().unwrap(),
    maybe_types: None,
    media_type: if ext.as_str() == "ts" {
      MediaType::TypeScript
    } else if ext.as_str() == "tsx" {
      MediaType::Tsx
    } else if ext.as_str() == "js" {
      MediaType::JavaScript
    } else {
      MediaType::Jsx
    },
    source: String::from_utf8(source_code)?,
    specifier: main_module.clone(),
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

async fn create_module_graph_and_maybe_check(
  module_specifier: ModuleSpecifier,
  program_state: Arc<ProgramState>,
  debug: bool,
) -> Result<module_graph::Graph, AnyError> {
  let handler = Arc::new(Mutex::new(FetchHandler::new(
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

  if !program_state.flags.no_check {
    // TODO(@kitsonk) support bundling for workers
    let lib = if program_state.flags.unstable {
      module_graph::TypeLib::UnstableDenoWindow
    } else {
      module_graph::TypeLib::DenoWindow
    };
    let result_info =
      module_graph.clone().check(module_graph::CheckOptions {
        debug,
        emit: false,
        lib,
        maybe_config_path: program_state.flags.config_path.clone(),
        reload: program_state.flags.reload,
      })?;

    debug!("{}", result_info.stats);
    if let Some(ignored_options) = result_info.maybe_ignored_options {
      eprintln!("{}", ignored_options);
    }
    if !result_info.diagnostics.is_empty() {
      return Err(generic_error(result_info.diagnostics.to_string()));
    }
  }

  Ok(module_graph)
}

fn bundle_module_graph(
  module_graph: module_graph::Graph,
  flags: Flags,
  debug: bool,
) -> Result<String, AnyError> {
  let (bundle, stats, maybe_ignored_options) =
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
  Ok(bundle)
}

async fn bundle_command(
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
      let module_specifier = resolve_url_or_path(&source_file1)?;

      debug!(">>>>> bundle START");
      let program_state = ProgramState::build(flags.clone()).await?;

      info!(
        "{} {}",
        colors::green("Bundle"),
        module_specifier.to_string()
      );

      let module_graph = create_module_graph_and_maybe_check(
        module_specifier,
        program_state.clone(),
        debug,
      )
      .await?;

      let mut paths_to_watch: Vec<PathBuf> = module_graph
        .get_modules()
        .iter()
        .filter_map(|specifier| specifier.to_file_path().ok())
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
      let output = bundle_module_graph(module_graph, flags, debug)?;

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

async fn doc_command(
  flags: Flags,
  source_file: Option<String>,
  json: bool,
  maybe_filter: Option<String>,
  private: bool,
) -> Result<(), AnyError> {
  tools::doc::print_docs(flags, source_file, json, maybe_filter, private).await
}

async fn format_command(
  flags: Flags,
  args: Vec<PathBuf>,
  ignore: Vec<PathBuf>,
  check: bool,
  ext: String,
) -> Result<(), AnyError> {
  if args.len() == 1 && args[0].to_string_lossy() == "-" {
    return tools::fmt::format_stdin(check, ext);
  }

  tools::fmt::format(args, ignore, check, flags.watch).await?;
  Ok(())
}

async fn run_repl(flags: Flags) -> Result<(), AnyError> {
  let main_module = resolve_url_or_path("./$deno$repl.ts").unwrap();
  let permissions = Permissions::from_options(&flags.clone().into());
  let program_state = ProgramState::build(flags).await?;
  let mut worker =
    create_main_worker(&program_state, main_module.clone(), permissions);
  worker.run_event_loop().await?;

  tools::repl::run(&program_state, worker).await
}

async fn run_from_stdin(flags: Flags) -> Result<(), AnyError> {
  let program_state = ProgramState::build(flags.clone()).await?;
  let permissions = Permissions::from_options(&flags.clone().into());
  let main_module = resolve_url_or_path("./$deno$stdin.ts").unwrap();
  let mut worker = create_main_worker(
    &program_state.clone(),
    main_module.clone(),
    permissions,
  );

  let mut source = Vec::new();
  std::io::stdin().read_to_end(&mut source)?;
  // Create a dummy source file.
  let source_file = File {
    local: main_module.clone().to_file_path().unwrap(),
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
    let script1 = script.clone();
    let script2 = script.clone();
    let flags = flags.clone();
    async move {
      let main_module = resolve_url_or_path(&script1)?;
      let program_state = ProgramState::build(flags).await?;
      let handler = Arc::new(Mutex::new(FetchHandler::new(
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
        .filter_map(|specifier| specifier.to_file_path().ok())
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
    let permissions = Permissions::from_options(&flags.clone().into());
    async move {
      let main_module = main_module.clone();
      let program_state = ProgramState::build(flags).await?;
      let mut worker =
        create_main_worker(&program_state, main_module.clone(), permissions);
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

  let main_module = resolve_url_or_path(&script)?;
  let program_state = ProgramState::build(flags.clone()).await?;
  let permissions = Permissions::from_options(&flags.clone().into());
  let mut worker =
    create_main_worker(&program_state, main_module.clone(), permissions);

  let mut maybe_coverage_collector =
    if let Some(ref coverage_dir) = program_state.coverage_dir {
      let session = worker.create_inspector_session();

      let coverage_dir = PathBuf::from(coverage_dir);
      let mut coverage_collector =
        tools::coverage::CoverageCollector::new(coverage_dir, session);
      coverage_collector.start_collecting().await?;

      Some(coverage_collector)
    } else {
      None
    };

  debug!("main_module {}", main_module);
  worker.execute_module(&main_module).await?;
  worker.execute("window.dispatchEvent(new Event('load'))")?;
  worker.run_event_loop().await?;
  worker.execute("window.dispatchEvent(new Event('unload'))")?;

  if let Some(coverage_collector) = maybe_coverage_collector.as_mut() {
    coverage_collector.stop_collecting().await?;
  }

  Ok(())
}

async fn coverage_command(
  flags: Flags,
  files: Vec<PathBuf>,
  ignore: Vec<PathBuf>,
  include: Vec<String>,
  exclude: Vec<String>,
  lcov: bool,
) -> Result<(), AnyError> {
  if files.is_empty() {
    println!("No matching coverage profiles found");
    std::process::exit(1);
  }

  tools::coverage::cover_files(
    flags.clone(),
    files,
    ignore,
    include,
    exclude,
    lcov,
  )
  .await
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
  let program_state = ProgramState::build(flags.clone()).await?;
  let permissions = Permissions::from_options(&flags.clone().into());
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
  let main_module = deno_core::resolve_path("$deno$test.ts")?;
  // Create a dummy source file.
  let source_file = File {
    local: main_module.to_file_path().unwrap(),
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
    create_main_worker(&program_state, main_module.clone(), permissions);

  if let Some(ref coverage_dir) = flags.coverage_dir {
    env::set_var("DENO_UNSTABLE_COVERAGE_DIR", coverage_dir);
  }

  let mut maybe_coverage_collector =
    if let Some(ref coverage_dir) = program_state.coverage_dir {
      let session = worker.create_inspector_session();
      let coverage_dir = PathBuf::from(coverage_dir);
      let mut coverage_collector =
        tools::coverage::CoverageCollector::new(coverage_dir, session);
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
    coverage_collector.stop_collecting().await?;
  }

  Ok(())
}

fn init_v8_flags(v8_flags: &[String]) {
  let v8_flags_includes_help = v8_flags
    .iter()
    .any(|flag| flag == "-help" || flag == "--help");
  // Keep in sync with `standalone.rs`.
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
  // wgpu backend crates (gfx_backend), have a lot of useless INFO and WARN logs
  .filter_module("gfx", LevelFilter::Error)
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
    DenoSubcommand::Eval { print, code, ext } => {
      eval_command(flags, code, ext, print).boxed_local()
    }
    DenoSubcommand::Cache { files } => {
      cache_command(flags, files).boxed_local()
    }
    DenoSubcommand::Compile {
      source_file,
      output,
      args,
      lite,
      target,
    } => compile_command(flags, source_file, output, args, target, lite)
      .boxed_local(),
    DenoSubcommand::Coverage {
      files,
      ignore,
      include,
      exclude,
      lcov,
    } => coverage_command(flags, files, ignore, include, exclude, lcov)
      .boxed_local(),
    DenoSubcommand::Fmt {
      check,
      files,
      ignore,
      ext,
    } => format_command(flags, files, ignore, check, ext).boxed_local(),
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
    DenoSubcommand::Lsp => lsp_command().boxed_local(),
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

fn unwrap_or_exit<T>(result: Result<T, AnyError>) -> T {
  match result {
    Ok(value) => value,
    Err(error) => {
      eprintln!("{}: {:?}", colors::red_bold("error"), error);
      std::process::exit(1);
    }
  }
}

pub fn main() {
  #[cfg(windows)]
  colors::enable_ansi(); // For Windows 10
  unix_util::raise_fd_limit();

  let args: Vec<String> = env::args().collect();
  let standalone_res = match standalone::extract_standalone(args.clone()) {
    Ok(Some((metadata, bundle))) => {
      tokio_util::run_basic(standalone::run(bundle, metadata))
    }
    Ok(None) => Ok(()),
    Err(err) => Err(err),
  };
  if let Err(err) = standalone_res {
    eprintln!("{}: {}", colors::red_bold("error"), err.to_string());
    std::process::exit(1);
  }

  let flags = match flags::flags_from_vec(args) {
    Ok(flags) => flags,
    Err(err @ clap::Error { .. })
      if err.kind == clap::ErrorKind::HelpDisplayed
        || err.kind == clap::ErrorKind::VersionDisplayed =>
    {
      err.write_to(&mut std::io::stdout()).unwrap();
      std::io::stdout().write_all(b"\n").unwrap();
      std::process::exit(0);
    }
    Err(err) => unwrap_or_exit(Err(AnyError::from(err))),
  };
  if !flags.v8_flags.is_empty() {
    init_v8_flags(&*flags.v8_flags);
  }
  init_logger(flags.log_level);

  unwrap_or_exit(tokio_util::run_basic(get_subcommand(flags)));
}
