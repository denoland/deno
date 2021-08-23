// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

mod ast;
use std::num::NonZeroUsize;
mod auth_tokens;
mod checksum;
mod colors;
mod config_file;
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
mod logger;
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
mod unix_util;
mod version;

use crate::file_fetcher::File;
use crate::file_watcher::ResolutionResult;
use crate::flags::DenoSubcommand;
use crate::flags::Flags;
use crate::fmt_errors::PrettyJsError;
use crate::media_type::MediaType;
use crate::module_graph::GraphBuilder;
use crate::module_graph::Module;
use crate::module_loader::CliModuleLoader;
use crate::program_state::ProgramState;
use crate::source_maps::apply_source_map;
use crate::specifier_handler::FetchHandler;
use crate::tools::installer::infer_name_from_url;
use deno_core::error::generic_error;
use deno_core::error::AnyError;
use deno_core::futures::future::FutureExt;
use deno_core::futures::Future;
use deno_core::located_script_name;
use deno_core::parking_lot::Mutex;
use deno_core::resolve_url_or_path;
use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_core::v8_set_flags;
use deno_core::JsRuntime;
use deno_core::ModuleSpecifier;
use deno_runtime::ops::worker_host::CreateWebWorkerCb;
use deno_runtime::permissions::Permissions;
use deno_runtime::web_worker::WebWorker;
use deno_runtime::web_worker::WebWorkerOptions;
use deno_runtime::worker::MainWorker;
use deno_runtime::worker::WorkerOptions;
use log::debug;
use log::info;
use std::collections::HashSet;
use std::env;
use std::io::Read;
use std::io::Write;
use std::iter::once;
use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Arc;
use tools::test_runner;

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
      enable_testing_features: program_state.flags.enable_testing_features,
      unsafely_ignore_certificate_errors: program_state
        .flags
        .unsafely_ignore_certificate_errors
        .clone(),
      root_cert_store: program_state.root_cert_store.clone(),
      user_agent: version::get_user_agent(),
      seed: program_state.flags.seed,
      module_loader,
      create_web_worker_cb,
      js_error_create_fn: Some(js_error_create_fn),
      use_deno_namespace: args.use_deno_namespace,
      worker_type: args.worker_type,
      maybe_inspector_server,
      runtime_version: version::deno(),
      ts_version: version::TYPESCRIPT.to_string(),
      no_color: !colors::use_color(),
      get_error_class_fn: Some(&crate::errors::get_error_class_name),
      blob_store: program_state.blob_store.clone(),
      broadcast_channel: program_state.broadcast_channel.clone(),
      shared_array_buffer_store: Some(
        program_state.shared_array_buffer_store.clone(),
      ),
      cpu_count: num_cpus::get(),
    };

    let (mut worker, external_handle) = WebWorker::from_options(
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
      js_runtime.sync_ops_cache();
    }
    worker.bootstrap(&options);

    (worker, external_handle)
  })
}

pub fn create_main_worker(
  program_state: &Arc<ProgramState>,
  main_module: ModuleSpecifier,
  permissions: Permissions,
  maybe_op_init: Option<&dyn Fn(&mut JsRuntime)>,
) -> MainWorker {
  let module_loader = CliModuleLoader::new(program_state.clone());

  let global_state_ = program_state.clone();

  let js_error_create_fn = Rc::new(move |core_js_error| {
    let source_mapped_error =
      apply_source_map(&core_js_error, global_state_.clone());
    PrettyJsError::create(source_mapped_error)
  });

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
    enable_testing_features: program_state.flags.enable_testing_features,
    unsafely_ignore_certificate_errors: program_state
      .flags
      .unsafely_ignore_certificate_errors
      .clone(),
    root_cert_store: program_state.root_cert_store.clone(),
    user_agent: version::get_user_agent(),
    seed: program_state.flags.seed,
    js_error_create_fn: Some(js_error_create_fn),
    create_web_worker_cb,
    maybe_inspector_server,
    should_break_on_first_statement,
    module_loader,
    runtime_version: version::deno(),
    ts_version: version::TYPESCRIPT.to_string(),
    no_color: !colors::use_color(),
    get_error_class_fn: Some(&crate::errors::get_error_class_name),
    location: program_state.flags.location.clone(),
    origin_storage_dir: program_state.flags.location.clone().map(|loc| {
      program_state
        .dir
        .root
        .clone()
        // TODO(@crowlKats): change to origin_data for 2.0
        .join("location_data")
        .join(checksum::gen(&[loc.to_string().as_bytes()]))
    }),
    blob_store: program_state.blob_store.clone(),
    broadcast_channel: program_state.broadcast_channel.clone(),
    shared_array_buffer_store: Some(
      program_state.shared_array_buffer_store.clone(),
    ),
    cpu_count: num_cpus::get(),
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

    if let Some(op_init) = maybe_op_init {
      op_init(js_runtime);
    }

    js_runtime.sync_ops_cache();
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
  let mut writer = std::io::BufWriter::new(std::io::stdout());
  serde_json::to_writer_pretty(&mut writer, value)?;
  writeln!(&mut writer)?;
  Ok(())
}

fn print_cache_info(
  state: &Arc<ProgramState>,
  json: bool,
  location: Option<deno_core::url::Url>,
) -> Result<(), AnyError> {
  let deno_dir = &state.dir.root;
  let modules_cache = &state.file_fetcher.get_http_cache_location();
  let typescript_cache = &state.dir.gen_cache.location;
  let registry_cache =
    &state.dir.root.join(lsp::language_server::REGISTRIES_PATH);
  let mut origin_dir = state.dir.root.join("location_data");

  if let Some(location) = &location {
    origin_dir =
      origin_dir.join(&checksum::gen(&[location.to_string().as_bytes()]));
  }

  if json {
    let mut output = json!({
      "denoDir": deno_dir,
      "modulesCache": modules_cache,
      "typescriptCache": typescript_cache,
      "registryCache": registry_cache,
      "originStorage": origin_dir,
    });

    if location.is_some() {
      output["localStorage"] =
        serde_json::to_value(origin_dir.join("local_storage"))?;
    }

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
    println!("{} {:?}", colors::bold("Origin storage:"), origin_dir);
    if location.is_some() {
      println!(
        "{} {:?}",
        colors::bold("Local Storage:"),
        origin_dir.join("local_storage"),
      );
    }
    Ok(())
  }
}

pub fn get_types(unstable: bool) -> String {
  let mut types = vec![
    crate::tsc::DENO_NS_LIB,
    crate::tsc::DENO_CONSOLE_LIB,
    crate::tsc::DENO_URL_LIB,
    crate::tsc::DENO_WEB_LIB,
    crate::tsc::DENO_FETCH_LIB,
    crate::tsc::DENO_WEBGPU_LIB,
    crate::tsc::DENO_WEBSOCKET_LIB,
    crate::tsc::DENO_WEBSTORAGE_LIB,
    crate::tsc::DENO_CRYPTO_LIB,
    crate::tsc::DENO_BROADCAST_CHANNEL_LIB,
    crate::tsc::DENO_NET_LIB,
    crate::tsc::SHARED_GLOBALS_LIB,
    crate::tsc::WINDOW_LIB,
  ];

  if unstable {
    types.push(crate::tsc::UNSTABLE_NS_LIB);
    types.push(crate::tsc::DENO_NET_UNSTABLE_LIB);
    types.push(crate::tsc::DENO_HTTP_UNSTABLE_LIB);
  }

  types.join("\n")
}

async fn compile_command(
  flags: Flags,
  source_file: String,
  output: Option<PathBuf>,
  args: Vec<String>,
  target: Option<String>,
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
  let bundle_str =
    bundle_module_graph(module_graph, program_state.clone(), flags, debug)?;

  info!(
    "{} {}",
    colors::green("Compile"),
    module_specifier.to_string()
  );

  // Select base binary based on target
  let original_binary =
    tools::standalone::get_base_binary(deno_dir, target.clone()).await?;

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
  let location = flags.location.clone();
  let program_state = ProgramState::build(flags).await?;
  if let Some(specifier) = maybe_specifier {
    let specifier = resolve_url_or_path(&specifier)?;
    let handler = Arc::new(Mutex::new(specifier_handler::FetchHandler::new(
      &program_state,
      // info accesses dynamically imported modules just for their information
      // so we allow access to all of them.
      Permissions::allow_all(),
      Permissions::allow_all(),
    )?));
    let mut builder = module_graph::GraphBuilder::new(
      handler,
      program_state.maybe_import_map.clone(),
      program_state.lockfile.clone(),
    );
    builder.add(&specifier, false).await?;
    builder
      .analyze_config_file(&program_state.maybe_config_file)
      .await?;
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
    print_cache_info(&program_state, json, location)
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
    create_main_worker(&program_state, main_module.clone(), permissions, None);
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
    create_main_worker(&program_state, main_module.clone(), permissions, None);
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
  worker.execute_script(
    &located_script_name!(),
    "window.dispatchEvent(new Event('load'))",
  )?;
  worker.run_event_loop(false).await?;
  worker.execute_script(
    &located_script_name!(),
    "window.dispatchEvent(new Event('unload'))",
  )?;
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
    Permissions::allow_all(),
  )?));
  let mut builder = module_graph::GraphBuilder::new(
    handler,
    program_state.maybe_import_map.clone(),
    program_state.lockfile.clone(),
  );
  builder.add(&module_specifier, false).await?;
  builder
    .analyze_config_file(&program_state.maybe_config_file)
    .await?;
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
        maybe_config_file: program_state.maybe_config_file.clone(),
        reload: program_state.flags.reload,
        ..Default::default()
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
  program_state: Arc<ProgramState>,
  flags: Flags,
  debug: bool,
) -> Result<String, AnyError> {
  let (bundle, stats, maybe_ignored_options) =
    module_graph.bundle(module_graph::BundleOptions {
      debug,
      maybe_config_file: program_state.maybe_config_file.clone(),
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

  let resolver = |_| {
    let flags = flags.clone();
    let source_file1 = source_file.clone();
    let source_file2 = source_file.clone();
    async move {
      let module_specifier = resolve_url_or_path(&source_file1)?;

      debug!(">>>>> bundle START");
      let program_state = ProgramState::build(flags.clone()).await?;

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

      Ok((paths_to_watch, module_graph, program_state))
    }
    .map(move |result| match result {
      Ok((paths_to_watch, module_graph, program_state)) => {
        ResolutionResult::Restart {
          paths_to_watch,
          result: Ok((program_state, module_graph)),
        }
      }
      Err(e) => ResolutionResult::Restart {
        paths_to_watch: vec![PathBuf::from(source_file2)],
        result: Err(e),
      },
    })
  };

  let operation = |(program_state, module_graph): (
    Arc<ProgramState>,
    module_graph::Graph,
  )| {
    let flags = flags.clone();
    let out_file = out_file.clone();
    async move {
      info!("{} {}", colors::green("Bundle"), module_graph.info()?.root);

      let output =
        bundle_module_graph(module_graph, program_state, flags, debug)?;

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
  };

  if flags.watch {
    file_watcher::watch_func(resolver, operation, "Bundle").await?;
  } else {
    let module_graph =
      if let ResolutionResult::Restart { result, .. } = resolver(None).await {
        result?
      } else {
        unreachable!();
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

async fn run_repl(
  flags: Flags,
  maybe_eval: Option<String>,
) -> Result<(), AnyError> {
  let main_module = resolve_url_or_path("./$deno$repl.ts").unwrap();
  let permissions = Permissions::from_options(&flags.clone().into());
  let program_state = ProgramState::build(flags).await?;
  let mut worker =
    create_main_worker(&program_state, main_module.clone(), permissions, None);
  worker.run_event_loop(false).await?;

  tools::repl::run(&program_state, worker, maybe_eval).await
}

async fn run_from_stdin(flags: Flags) -> Result<(), AnyError> {
  let program_state = ProgramState::build(flags.clone()).await?;
  let permissions = Permissions::from_options(&flags.clone().into());
  let main_module = resolve_url_or_path("./$deno$stdin.ts").unwrap();
  let mut worker = create_main_worker(
    &program_state.clone(),
    main_module.clone(),
    permissions,
    None,
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
  worker.execute_script(
    &located_script_name!(),
    "window.dispatchEvent(new Event('load'))",
  )?;
  worker.run_event_loop(false).await?;
  worker.execute_script(
    &located_script_name!(),
    "window.dispatchEvent(new Event('unload'))",
  )?;
  Ok(())
}

async fn run_with_watch(flags: Flags, script: String) -> Result<(), AnyError> {
  let resolver = |_| {
    let script1 = script.clone();
    let script2 = script.clone();
    let flags = flags.clone();
    async move {
      let main_module = resolve_url_or_path(&script1)?;
      let program_state = ProgramState::build(flags).await?;
      let handler = Arc::new(Mutex::new(FetchHandler::new(
        &program_state,
        Permissions::allow_all(),
        Permissions::allow_all(),
      )?));
      let mut builder = module_graph::GraphBuilder::new(
        handler,
        program_state.maybe_import_map.clone(),
        program_state.lockfile.clone(),
      );
      builder.add(&main_module, false).await?;
      builder
        .analyze_config_file(&program_state.maybe_config_file)
        .await?;
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

      Ok((paths_to_watch, main_module, program_state))
    }
    .map(move |result| match result {
      Ok((paths_to_watch, module_info, program_state)) => {
        ResolutionResult::Restart {
          paths_to_watch,
          result: Ok((program_state, module_info)),
        }
      }
      Err(e) => ResolutionResult::Restart {
        paths_to_watch: vec![PathBuf::from(script2)],
        result: Err(e),
      },
    })
  };

  let operation =
    |(program_state, main_module): (Arc<ProgramState>, ModuleSpecifier)| {
      let flags = flags.clone();
      let permissions = Permissions::from_options(&flags.into());
      async move {
        let main_module = main_module.clone();
        let mut worker = create_main_worker(
          &program_state,
          main_module.clone(),
          permissions,
          None,
        );
        debug!("main_module {}", main_module);
        worker.execute_module(&main_module).await?;
        worker.execute_script(
          &located_script_name!(),
          "window.dispatchEvent(new Event('load'))",
        )?;
        worker.run_event_loop(false).await?;
        worker.execute_script(
          &located_script_name!(),
          "window.dispatchEvent(new Event('unload'))",
        )?;
        Ok(())
      }
    };

  file_watcher::watch_func(resolver, operation, "Process").await
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
    create_main_worker(&program_state, main_module.clone(), permissions, None);

  let mut maybe_coverage_collector =
    if let Some(ref coverage_dir) = program_state.coverage_dir {
      let session = worker.create_inspector_session().await;

      let coverage_dir = PathBuf::from(coverage_dir);
      let mut coverage_collector =
        tools::coverage::CoverageCollector::new(coverage_dir, session);
      worker
        .with_event_loop(coverage_collector.start_collecting().boxed_local())
        .await?;
      Some(coverage_collector)
    } else {
      None
    };

  debug!("main_module {}", main_module);
  worker.execute_module(&main_module).await?;
  worker.execute_script(
    &located_script_name!(),
    "window.dispatchEvent(new Event('load'))",
  )?;
  worker
    .run_event_loop(maybe_coverage_collector.is_none())
    .await?;
  worker.execute_script(
    &located_script_name!(),
    "window.dispatchEvent(new Event('unload'))",
  )?;

  if let Some(coverage_collector) = maybe_coverage_collector.as_mut() {
    worker
      .with_event_loop(coverage_collector.stop_collecting().boxed_local())
      .await?;
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
    return Err(generic_error("No matching coverage profiles found"));
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

#[allow(clippy::too_many_arguments)]
async fn test_command(
  flags: Flags,
  include: Option<Vec<String>>,
  no_run: bool,
  doc: bool,
  fail_fast: Option<NonZeroUsize>,
  quiet: bool,
  allow_none: bool,
  filter: Option<String>,
  shuffle: Option<u64>,
  concurrent_jobs: NonZeroUsize,
) -> Result<(), AnyError> {
  if let Some(ref coverage_dir) = flags.coverage_dir {
    std::fs::create_dir_all(&coverage_dir)?;
    env::set_var(
      "DENO_UNSTABLE_COVERAGE_DIR",
      PathBuf::from(coverage_dir).canonicalize()?,
    );
  }

  // TODO(caspervonb) move this chunk into tools::test_runner.

  let program_state = ProgramState::build(flags.clone()).await?;

  let include = include.unwrap_or_else(|| vec![".".to_string()]);

  let permissions = Permissions::from_options(&flags.clone().into());
  let lib = if flags.unstable {
    module_graph::TypeLib::UnstableDenoWindow
  } else {
    module_graph::TypeLib::DenoWindow
  };

  if flags.watch {
    let handler = Arc::new(Mutex::new(FetchHandler::new(
      &program_state,
      Permissions::allow_all(),
      Permissions::allow_all(),
    )?));

    let paths_to_watch: Vec<_> = include.iter().map(PathBuf::from).collect();

    // TODO(caspervonb) clean this up.
    let resolver = |changed: Option<Vec<PathBuf>>| {
      let test_modules_result = if doc {
        fs_util::collect_specifiers(
          include.clone(),
          fs_util::is_supported_test_ext,
        )
      } else {
        fs_util::collect_specifiers(
          include.clone(),
          fs_util::is_supported_test_path,
        )
      };

      let paths_to_watch = paths_to_watch.clone();
      let paths_to_watch_clone = paths_to_watch.clone();

      let handler = handler.clone();
      let program_state = program_state.clone();
      let files_changed = changed.is_some();
      async move {
        let test_modules = test_modules_result?;

        let mut paths_to_watch = paths_to_watch_clone;
        let mut modules_to_reload = if files_changed {
          Vec::new()
        } else {
          test_modules
            .iter()
            .filter_map(|url| deno_core::resolve_url(url.as_str()).ok())
            .collect()
        };

        let mut builder = GraphBuilder::new(
          handler,
          program_state.maybe_import_map.clone(),
          program_state.lockfile.clone(),
        );
        for specifier in test_modules.iter() {
          builder.add(specifier, false).await?;
        }
        builder
          .analyze_config_file(&program_state.maybe_config_file)
          .await?;
        let graph = builder.get_graph();

        for specifier in test_modules {
          fn get_dependencies<'a>(
            graph: &'a module_graph::Graph,
            module: &'a Module,
            // This needs to be accessible to skip getting dependencies if they're already there,
            // otherwise this will cause a stack overflow with circular dependencies
            output: &mut HashSet<&'a ModuleSpecifier>,
          ) -> Result<(), AnyError> {
            for dep in module.dependencies.values() {
              if let Some(specifier) = &dep.maybe_code {
                if !output.contains(specifier) {
                  output.insert(specifier);

                  get_dependencies(
                    graph,
                    graph.get_specifier(specifier)?,
                    output,
                  )?;
                }
              }
              if let Some(specifier) = &dep.maybe_type {
                if !output.contains(specifier) {
                  output.insert(specifier);

                  get_dependencies(
                    graph,
                    graph.get_specifier(specifier)?,
                    output,
                  )?;
                }
              }
            }

            Ok(())
          }

          // This test module and all it's dependencies
          let mut modules = HashSet::new();
          modules.insert(&specifier);
          get_dependencies(
            &graph,
            graph.get_specifier(&specifier)?,
            &mut modules,
          )?;

          paths_to_watch.extend(
            modules
              .iter()
              .filter_map(|specifier| specifier.to_file_path().ok()),
          );

          if let Some(changed) = &changed {
            for path in changed.iter().filter_map(|path| {
              deno_core::resolve_url_or_path(&path.to_string_lossy()).ok()
            }) {
              if modules.contains(&&path) {
                modules_to_reload.push(specifier);
                break;
              }
            }
          }
        }

        Ok((paths_to_watch, modules_to_reload))
      }
      .map(move |result| {
        if files_changed
          && matches!(result, Ok((_, ref modules)) if modules.is_empty())
        {
          ResolutionResult::Ignore
        } else {
          match result {
            Ok((paths_to_watch, modules_to_reload)) => {
              ResolutionResult::Restart {
                paths_to_watch,
                result: Ok(modules_to_reload),
              }
            }
            Err(e) => ResolutionResult::Restart {
              paths_to_watch,
              result: Err(e),
            },
          }
        }
      })
    };

    let operation = |modules_to_reload: Vec<ModuleSpecifier>| {
      let filter = filter.clone();
      let include = include.clone();
      let lib = lib.clone();
      let permissions = permissions.clone();
      let program_state = program_state.clone();

      async move {
        let doc_modules = if doc {
          fs_util::collect_specifiers(
            include.clone(),
            fs_util::is_supported_test_ext,
          )?
        } else {
          Vec::new()
        };

        let doc_modules_to_reload = doc_modules
          .iter()
          .filter(|specifier| modules_to_reload.contains(specifier))
          .cloned()
          .collect();

        let test_modules = fs_util::collect_specifiers(
          include.clone(),
          fs_util::is_supported_test_path,
        )?;

        let test_modules_to_reload = test_modules
          .iter()
          .filter(|specifier| modules_to_reload.contains(specifier))
          .cloned()
          .collect();

        test_runner::run_tests(
          program_state.clone(),
          permissions.clone(),
          lib.clone(),
          doc_modules_to_reload,
          test_modules_to_reload,
          no_run,
          fail_fast,
          quiet,
          true,
          filter.clone(),
          shuffle,
          concurrent_jobs,
        )
        .await?;

        Ok(())
      }
    };

    file_watcher::watch_func(resolver, operation, "Test").await?;
  } else {
    let doc_modules = if doc {
      fs_util::collect_specifiers(
        include.clone(),
        fs_util::is_supported_test_ext,
      )?
    } else {
      Vec::new()
    };

    let test_modules = fs_util::collect_specifiers(
      include.clone(),
      fs_util::is_supported_test_path,
    )?;

    test_runner::run_tests(
      program_state.clone(),
      permissions,
      lib,
      doc_modules,
      test_modules,
      no_run,
      fail_fast,
      quiet,
      allow_none,
      filter,
      shuffle,
      concurrent_jobs,
    )
    .await?;
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
      target,
    } => {
      compile_command(flags, source_file, output, args, target).boxed_local()
    }
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
    DenoSubcommand::Repl { eval } => run_repl(flags, eval).boxed_local(),
    DenoSubcommand::Run { script } => run_command(flags, script).boxed_local(),
    DenoSubcommand::Test {
      no_run,
      doc,
      fail_fast,
      quiet,
      include,
      allow_none,
      filter,
      shuffle,
      concurrent_jobs,
    } => test_command(
      flags,
      include,
      no_run,
      doc,
      fail_fast,
      quiet,
      allow_none,
      filter,
      shuffle,
      concurrent_jobs,
    )
    .boxed_local(),
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

  logger::init(flags.log_level);

  unwrap_or_exit(tokio_util::run_basic(get_subcommand(flags)));
}
