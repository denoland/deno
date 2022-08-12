// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

mod args;
mod auth_tokens;
mod cache;
mod cdp;
mod checksum;
mod compat;
mod deno_dir;
mod diagnostics;
mod diff;
mod display;
mod emit;
mod errors;
mod file_fetcher;
mod file_watcher;
mod fmt_errors;
mod fs_util;
mod graph_util;
mod http_cache;
mod http_util;
mod lockfile;
mod logger;
mod lsp;
mod module_loader;
#[allow(unused)]
mod npm;
mod ops;
mod proc_state;
mod resolver;
mod standalone;
mod text_encoding;
mod tools;
mod tsc;
mod unix_util;
mod version;
mod windows_util;
mod worker;

use crate::args::flags_from_vec;
use crate::args::BenchFlags;
use crate::args::BundleFlags;
use crate::args::CacheFlags;
use crate::args::CheckFlags;
use crate::args::CompileFlags;
use crate::args::CompletionsFlags;
use crate::args::CoverageFlags;
use crate::args::DenoSubcommand;
use crate::args::DocFlags;
use crate::args::EvalFlags;
use crate::args::Flags;
use crate::args::FmtFlags;
use crate::args::InfoFlags;
use crate::args::InstallFlags;
use crate::args::LintFlags;
use crate::args::ReplFlags;
use crate::args::RunFlags;
use crate::args::TaskFlags;
use crate::args::TestFlags;
use crate::args::TypeCheckMode;
use crate::args::UninstallFlags;
use crate::args::UpgradeFlags;
use crate::args::VendorFlags;
use crate::cache::TypeCheckCache;
use crate::emit::TsConfigType;
use crate::file_fetcher::File;
use crate::file_watcher::ResolutionResult;
use crate::fmt_errors::format_js_error;
use crate::graph_util::graph_lock_or_exit;
use crate::graph_util::graph_valid;
use crate::proc_state::ProcState;
use crate::resolver::ImportMapResolver;
use crate::resolver::JsxResolver;

use args::CliOptions;
use deno_ast::MediaType;
use deno_core::error::generic_error;
use deno_core::error::AnyError;
use deno_core::error::JsError;
use deno_core::futures::future::FutureExt;
use deno_core::futures::Future;
use deno_core::parking_lot::RwLock;
use deno_core::resolve_url_or_path;
use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_core::v8_set_flags;
use deno_core::ModuleSpecifier;
use deno_runtime::colors;
use deno_runtime::permissions::Permissions;
use deno_runtime::tokio_util::run_local;
use log::debug;
use log::info;
use std::env;
use std::io::Read;
use std::io::Write;
use std::iter::once;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use worker::create_main_worker;

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
  state: &ProcState,
  json: bool,
  location: Option<&deno_core::url::Url>,
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

  let local_storage_dir = origin_dir.join("local_storage");

  if json {
    let mut output = json!({
      "denoDir": deno_dir,
      "modulesCache": modules_cache,
      "typescriptCache": typescript_cache,
      "registryCache": registry_cache,
      "originStorage": origin_dir,
    });

    if location.is_some() {
      output["localStorage"] = serde_json::to_value(local_storage_dir)?;
    }

    write_json_to_stdout(&output)
  } else {
    println!(
      "{} {}",
      colors::bold("DENO_DIR location:"),
      deno_dir.display()
    );
    println!(
      "{} {}",
      colors::bold("Remote modules cache:"),
      modules_cache.display()
    );
    println!(
      "{} {}",
      colors::bold("Emitted modules cache:"),
      typescript_cache.display()
    );
    println!(
      "{} {}",
      colors::bold("Language server registries cache:"),
      registry_cache.display(),
    );
    println!(
      "{} {}",
      colors::bold("Origin storage:"),
      origin_dir.display()
    );
    if location.is_some() {
      println!(
        "{} {}",
        colors::bold("Local Storage:"),
        local_storage_dir.display(),
      );
    }
    Ok(())
  }
}

pub fn get_types(unstable: bool) -> String {
  let mut types = vec![
    tsc::DENO_NS_LIB,
    tsc::DENO_CONSOLE_LIB,
    tsc::DENO_URL_LIB,
    tsc::DENO_WEB_LIB,
    tsc::DENO_FETCH_LIB,
    tsc::DENO_WEBGPU_LIB,
    tsc::DENO_WEBSOCKET_LIB,
    tsc::DENO_WEBSTORAGE_LIB,
    tsc::DENO_CRYPTO_LIB,
    tsc::DENO_BROADCAST_CHANNEL_LIB,
    tsc::DENO_NET_LIB,
    tsc::SHARED_GLOBALS_LIB,
    tsc::WINDOW_LIB,
  ];

  if unstable {
    types.push(tsc::UNSTABLE_NS_LIB);
  }

  types.join("\n")
}

async fn compile_command(
  flags: Flags,
  compile_flags: CompileFlags,
) -> Result<i32, AnyError> {
  let debug = flags.log_level == Some(log::Level::Debug);

  let run_flags = tools::standalone::compile_to_runtime_flags(
    &flags,
    compile_flags.args.clone(),
  )?;

  let module_specifier = resolve_url_or_path(&compile_flags.source_file)?;
  let ps = ProcState::build(flags).await?;
  let deno_dir = &ps.dir;

  let output_path =
    tools::standalone::resolve_compile_executable_output_path(&compile_flags)?;

  let graph = Arc::try_unwrap(
    create_graph_and_maybe_check(module_specifier.clone(), &ps, debug).await?,
  )
  .map_err(|_| {
    generic_error("There should only be one reference to ModuleGraph")
  })?;

  graph.valid().unwrap();

  let eszip = eszip::EszipV2::from_graph(graph, Default::default())?;

  info!(
    "{} {}",
    colors::green("Compile"),
    module_specifier.to_string()
  );

  // Select base binary based on target
  let original_binary =
    tools::standalone::get_base_binary(deno_dir, compile_flags.target.clone())
      .await?;

  let final_bin = tools::standalone::create_standalone_binary(
    original_binary,
    eszip,
    module_specifier.clone(),
    run_flags,
    ps,
  )
  .await?;

  info!("{} {}", colors::green("Emit"), output_path.display());

  tools::standalone::write_standalone_binary(output_path, final_bin).await?;

  Ok(0)
}

async fn info_command(
  flags: Flags,
  info_flags: InfoFlags,
) -> Result<i32, AnyError> {
  let ps = ProcState::build(flags).await?;
  if let Some(specifier) = info_flags.file {
    let specifier = resolve_url_or_path(&specifier)?;
    let graph = ps
      .create_graph(vec![(specifier, deno_graph::ModuleKind::Esm)])
      .await?;

    if info_flags.json {
      write_json_to_stdout(&json!(graph))?;
    } else {
      write_to_stdout_ignore_sigpipe(graph.to_string().as_bytes())?;
    }
  } else {
    // If it was just "deno info" print location of caches and exit
    print_cache_info(&ps, info_flags.json, ps.options.location_flag())?;
  }
  Ok(0)
}

async fn install_command(
  flags: Flags,
  install_flags: InstallFlags,
) -> Result<i32, AnyError> {
  let mut preload_flags = flags.clone();
  preload_flags.inspect = None;
  preload_flags.inspect_brk = None;
  let permissions =
    Permissions::from_options(&preload_flags.permissions_options())?;
  let ps = ProcState::build(preload_flags).await?;
  let main_module = resolve_url_or_path(&install_flags.module_url)?;
  let mut worker = create_main_worker(
    &ps,
    main_module,
    permissions,
    vec![],
    Default::default(),
  );
  // First, fetch and compile the module; this step ensures that the module exists.
  worker.preload_main_module().await?;
  tools::installer::install(flags, install_flags)?;
  Ok(0)
}

async fn uninstall_command(
  uninstall_flags: UninstallFlags,
) -> Result<i32, AnyError> {
  tools::installer::uninstall(uninstall_flags.name, uninstall_flags.root)?;
  Ok(0)
}

async fn lsp_command() -> Result<i32, AnyError> {
  lsp::start().await?;
  Ok(0)
}

async fn lint_command(
  flags: Flags,
  lint_flags: LintFlags,
) -> Result<i32, AnyError> {
  if lint_flags.rules {
    tools::lint::print_rules_list(lint_flags.json);
    return Ok(0);
  }

  tools::lint::lint(flags, lint_flags).await?;
  Ok(0)
}

async fn cache_command(
  flags: Flags,
  cache_flags: CacheFlags,
) -> Result<i32, AnyError> {
  let ps = ProcState::build(flags).await?;
  load_and_type_check(&ps, &cache_flags.files).await?;
  ps.cache_module_emits()?;
  Ok(0)
}

async fn check_command(
  flags: Flags,
  check_flags: CheckFlags,
) -> Result<i32, AnyError> {
  let ps = ProcState::build(flags).await?;
  load_and_type_check(&ps, &check_flags.files).await?;
  Ok(0)
}

async fn load_and_type_check(
  ps: &ProcState,
  files: &Vec<String>,
) -> Result<(), AnyError> {
  let lib = ps.options.ts_type_lib_window();

  for file in files {
    let specifier = resolve_url_or_path(file)?;
    ps.prepare_module_load(
      vec![specifier],
      false,
      lib,
      Permissions::allow_all(),
      Permissions::allow_all(),
      false,
    )
    .await?;
  }

  Ok(())
}

async fn eval_command(
  flags: Flags,
  eval_flags: EvalFlags,
) -> Result<i32, AnyError> {
  // deno_graph works off of extensions for local files to determine the media
  // type, and so our "fake" specifier needs to have the proper extension.
  let main_module =
    resolve_url_or_path(&format!("./$deno$eval.{}", eval_flags.ext))?;
  let permissions = Permissions::from_options(&flags.permissions_options())?;
  let ps = ProcState::build(flags).await?;
  let mut worker = create_main_worker(
    &ps,
    main_module.clone(),
    permissions,
    vec![],
    Default::default(),
  );
  // Create a dummy source file.
  let source_code = if eval_flags.print {
    format!("console.log({})", eval_flags.code)
  } else {
    eval_flags.code
  }
  .into_bytes();

  let file = File {
    local: main_module.clone().to_file_path().unwrap(),
    maybe_types: None,
    media_type: MediaType::Unknown,
    source: String::from_utf8(source_code)?.into(),
    specifier: main_module.clone(),
    maybe_headers: None,
  };

  // Save our fake file into file fetcher cache
  // to allow module access by TS compiler.
  ps.file_fetcher.insert_cached(file);
  let exit_code = worker.run().await?;
  Ok(exit_code)
}

async fn create_graph_and_maybe_check(
  root: ModuleSpecifier,
  ps: &ProcState,
  debug: bool,
) -> Result<Arc<deno_graph::ModuleGraph>, AnyError> {
  let mut cache = cache::FetchCacher::new(
    ps.emit_cache.clone(),
    ps.file_fetcher.clone(),
    Permissions::allow_all(),
    Permissions::allow_all(),
  );
  let maybe_locker = lockfile::as_maybe_locker(ps.lockfile.clone());
  let maybe_imports = ps.options.to_maybe_imports()?;
  let maybe_import_map_resolver =
    ps.maybe_import_map.clone().map(ImportMapResolver::new);
  let maybe_jsx_resolver = ps
    .options
    .to_maybe_jsx_import_source_module()
    .map(|im| JsxResolver::new(im, maybe_import_map_resolver.clone()));
  let maybe_resolver = if maybe_jsx_resolver.is_some() {
    maybe_jsx_resolver.as_ref().map(|jr| jr.as_resolver())
  } else {
    maybe_import_map_resolver
      .as_ref()
      .map(|im| im.as_resolver())
  };
  let graph = Arc::new(
    deno_graph::create_graph(
      vec![(root, deno_graph::ModuleKind::Esm)],
      false,
      maybe_imports,
      &mut cache,
      maybe_resolver,
      maybe_locker,
      None,
      None,
    )
    .await,
  );

  let check_js = ps.options.check_js();
  graph_valid(
    &graph,
    ps.options.type_check_mode() != TypeCheckMode::None,
    check_js,
  )?;
  graph_lock_or_exit(&graph);

  if ps.options.type_check_mode() != TypeCheckMode::None {
    let ts_config_result =
      ps.options.resolve_ts_config_for_emit(TsConfigType::Check {
        lib: ps.options.ts_type_lib_window(),
      })?;
    if let Some(ignored_options) = ts_config_result.maybe_ignored_options {
      eprintln!("{}", ignored_options);
    }
    let maybe_config_specifier = ps.options.maybe_config_file_specifier();
    let cache = TypeCheckCache::new(&ps.dir.type_checking_cache_db_file_path());
    let check_result = emit::check(
      &graph.roots,
      Arc::new(RwLock::new(graph.as_ref().into())),
      &cache,
      emit::CheckOptions {
        type_check_mode: ps.options.type_check_mode(),
        debug,
        maybe_config_specifier,
        ts_config: ts_config_result.ts_config,
        log_checks: true,
        reload: ps.options.reload_flag(),
      },
    )?;
    debug!("{}", check_result.stats);
    if !check_result.diagnostics.is_empty() {
      return Err(check_result.diagnostics.into());
    }
  }

  Ok(graph)
}

fn bundle_module_graph(
  graph: &deno_graph::ModuleGraph,
  ps: &ProcState,
) -> Result<deno_emit::BundleEmit, AnyError> {
  info!("{} {}", colors::green("Bundle"), graph.roots[0].0);

  let ts_config_result = ps
    .options
    .resolve_ts_config_for_emit(TsConfigType::Bundle)?;
  if ps.options.type_check_mode() == TypeCheckMode::None {
    if let Some(ignored_options) = ts_config_result.maybe_ignored_options {
      eprintln!("{}", ignored_options);
    }
  }

  deno_emit::bundle_graph(
    graph,
    deno_emit::BundleOptions {
      bundle_type: deno_emit::BundleType::Module,
      emit_options: ts_config_result.ts_config.into(),
      emit_ignore_directives: true,
    },
  )
}

async fn bundle_command(
  flags: Flags,
  bundle_flags: BundleFlags,
) -> Result<i32, AnyError> {
  let debug = flags.log_level == Some(log::Level::Debug);
  let cli_options = Arc::new(CliOptions::from_flags(flags)?);
  let resolver = |_| {
    let cli_options = cli_options.clone();
    let source_file1 = bundle_flags.source_file.clone();
    let source_file2 = bundle_flags.source_file.clone();
    async move {
      let module_specifier = resolve_url_or_path(&source_file1)?;

      debug!(">>>>> bundle START");
      let ps = ProcState::from_options(cli_options).await?;

      let graph =
        create_graph_and_maybe_check(module_specifier, &ps, debug).await?;

      let mut paths_to_watch: Vec<PathBuf> = graph
        .specifiers()
        .iter()
        .filter_map(|(_, r)| {
          r.as_ref().ok().and_then(|(s, _, _)| s.to_file_path().ok())
        })
        .collect();

      if let Ok(Some(import_map_path)) = ps
        .options
        .resolve_import_map_specifier()
        .map(|ms| ms.and_then(|ref s| s.to_file_path().ok()))
      {
        paths_to_watch.push(import_map_path);
      }

      Ok((paths_to_watch, graph, ps))
    }
    .map(move |result| match result {
      Ok((paths_to_watch, graph, ps)) => ResolutionResult::Restart {
        paths_to_watch,
        result: Ok((ps, graph)),
      },
      Err(e) => ResolutionResult::Restart {
        paths_to_watch: vec![PathBuf::from(source_file2)],
        result: Err(e),
      },
    })
  };

  let operation = |(ps, graph): (ProcState, Arc<deno_graph::ModuleGraph>)| {
    let out_file = bundle_flags.out_file.clone();
    async move {
      let bundle_output = bundle_module_graph(graph.as_ref(), &ps)?;
      debug!(">>>>> bundle END");

      if let Some(out_file) = out_file.as_ref() {
        let output_bytes = bundle_output.code.as_bytes();
        let output_len = output_bytes.len();
        fs_util::write_file(out_file, output_bytes, 0o644)?;
        info!(
          "{} {:?} ({})",
          colors::green("Emit"),
          out_file,
          colors::gray(display::human_size(output_len as f64))
        );
        if let Some(bundle_map) = bundle_output.maybe_map {
          let map_bytes = bundle_map.as_bytes();
          let map_len = map_bytes.len();
          let ext = if let Some(curr_ext) = out_file.extension() {
            format!("{}.map", curr_ext.to_string_lossy())
          } else {
            "map".to_string()
          };
          let map_out_file = out_file.with_extension(ext);
          fs_util::write_file(&map_out_file, map_bytes, 0o644)?;
          info!(
            "{} {:?} ({})",
            colors::green("Emit"),
            map_out_file,
            colors::gray(display::human_size(map_len as f64))
          );
        }
      } else {
        println!("{}", bundle_output.code);
      }

      Ok(())
    }
  };

  if cli_options.watch_paths().is_some() {
    file_watcher::watch_func(
      resolver,
      operation,
      file_watcher::PrintConfig {
        job_name: "Bundle".to_string(),
        clear_screen: !cli_options.no_clear_screen(),
      },
    )
    .await?;
  } else {
    let module_graph =
      if let ResolutionResult::Restart { result, .. } = resolver(None).await {
        result?
      } else {
        unreachable!();
      };
    operation(module_graph).await?;
  }

  Ok(0)
}

async fn doc_command(
  flags: Flags,
  doc_flags: DocFlags,
) -> Result<i32, AnyError> {
  tools::doc::print_docs(flags, doc_flags).await?;
  Ok(0)
}

async fn format_command(
  flags: Flags,
  fmt_flags: FmtFlags,
) -> Result<i32, AnyError> {
  let config = CliOptions::from_flags(flags)?;

  if fmt_flags.files.len() == 1 && fmt_flags.files[0].to_string_lossy() == "-" {
    let maybe_fmt_config = config.to_fmt_config()?;
    tools::fmt::format_stdin(
      fmt_flags,
      maybe_fmt_config.map(|c| c.options).unwrap_or_default(),
    )?;
    return Ok(0);
  }

  tools::fmt::format(&config, fmt_flags).await?;
  Ok(0)
}

async fn repl_command(
  flags: Flags,
  repl_flags: ReplFlags,
) -> Result<i32, AnyError> {
  let main_module = resolve_url_or_path("./$deno$repl.ts").unwrap();
  let ps = ProcState::build(flags).await?;
  let mut worker = create_main_worker(
    &ps,
    main_module.clone(),
    Permissions::from_options(&ps.options.permissions_options())?,
    vec![],
    Default::default(),
  );
  worker.setup_repl().await?;
  tools::repl::run(
    &ps,
    worker.into_main_worker(),
    repl_flags.eval_files,
    repl_flags.eval,
  )
  .await
}

async fn run_from_stdin(flags: Flags) -> Result<i32, AnyError> {
  let ps = ProcState::build(flags).await?;
  let main_module = resolve_url_or_path("./$deno$stdin.ts").unwrap();
  let mut worker = create_main_worker(
    &ps.clone(),
    main_module.clone(),
    Permissions::from_options(&ps.options.permissions_options())?,
    vec![],
    Default::default(),
  );

  let mut source = Vec::new();
  std::io::stdin().read_to_end(&mut source)?;
  // Create a dummy source file.
  let source_file = File {
    local: main_module.clone().to_file_path().unwrap(),
    maybe_types: None,
    media_type: MediaType::TypeScript,
    source: String::from_utf8(source)?.into(),
    specifier: main_module.clone(),
    maybe_headers: None,
  };
  // Save our fake file into file fetcher cache
  // to allow module access by TS compiler
  ps.file_fetcher.insert_cached(source_file);

  let exit_code = worker.run().await?;
  Ok(exit_code)
}

// TODO(bartlomieju): this function is not handling `exit_code` set by the runtime
// code properly.
async fn run_with_watch(flags: Flags, script: String) -> Result<i32, AnyError> {
  let flags = Arc::new(flags);
  let main_module = resolve_url_or_path(&script)?;
  let (sender, receiver) = tokio::sync::mpsc::unbounded_channel();

  let operation = |(sender, main_module): (
    tokio::sync::mpsc::UnboundedSender<Vec<PathBuf>>,
    ModuleSpecifier,
  )| {
    let flags = flags.clone();
    let permissions = Permissions::from_options(&flags.permissions_options())?;
    Ok(async move {
      let ps =
        ProcState::build_for_file_watcher((*flags).clone(), sender.clone())
          .await?;
      let worker = create_main_worker(
        &ps,
        main_module.clone(),
        permissions,
        vec![],
        Default::default(),
      );
      worker.run_for_watcher().await?;

      Ok(())
    })
  };

  file_watcher::watch_func2(
    receiver,
    operation,
    (sender, main_module),
    file_watcher::PrintConfig {
      job_name: "Process".to_string(),
      clear_screen: !flags.no_clear_screen,
    },
  )
  .await?;

  Ok(0)
}

async fn run_command(
  flags: Flags,
  run_flags: RunFlags,
) -> Result<i32, AnyError> {
  // Read script content from stdin
  if run_flags.is_stdin() {
    return run_from_stdin(flags).await;
  }

  if flags.watch.is_some() {
    return run_with_watch(flags, run_flags.script).await;
  }

  // TODO(bartlomieju): it should not be resolved here if we're in compat mode
  // because it might be a bare specifier
  // TODO(bartlomieju): actually I think it will also fail if there's an import
  // map specified and bare specifier is used on the command line - this should
  // probably call `ProcState::resolve` instead
  let main_module = resolve_url_or_path(&run_flags.script)?;
  let ps = ProcState::build(flags).await?;
  let permissions =
    Permissions::from_options(&ps.options.permissions_options())?;
  let mut worker = create_main_worker(
    &ps,
    main_module.clone(),
    permissions,
    vec![],
    Default::default(),
  );

  let exit_code = worker.run().await?;
  Ok(exit_code)
}

async fn task_command(
  flags: Flags,
  task_flags: TaskFlags,
) -> Result<i32, AnyError> {
  tools::task::execute_script(flags, task_flags).await
}

async fn coverage_command(
  flags: Flags,
  coverage_flags: CoverageFlags,
) -> Result<i32, AnyError> {
  if coverage_flags.files.is_empty() {
    return Err(generic_error("No matching coverage profiles found"));
  }

  tools::coverage::cover_files(flags, coverage_flags).await?;
  Ok(0)
}

async fn bench_command(
  flags: Flags,
  bench_flags: BenchFlags,
) -> Result<i32, AnyError> {
  if flags.watch.is_some() {
    tools::bench::run_benchmarks_with_watch(flags, bench_flags).await?;
  } else {
    tools::bench::run_benchmarks(flags, bench_flags).await?;
  }

  Ok(0)
}

async fn test_command(
  flags: Flags,
  test_flags: TestFlags,
) -> Result<i32, AnyError> {
  if let Some(ref coverage_dir) = flags.coverage_dir {
    std::fs::create_dir_all(&coverage_dir)?;
    env::set_var(
      "DENO_UNSTABLE_COVERAGE_DIR",
      PathBuf::from(coverage_dir).canonicalize()?,
    );
  }

  if flags.watch.is_some() {
    tools::test::run_tests_with_watch(flags, test_flags).await?;
  } else {
    tools::test::run_tests(flags, test_flags).await?;
  }

  Ok(0)
}

async fn completions_command(
  _flags: Flags,
  completions_flags: CompletionsFlags,
) -> Result<i32, AnyError> {
  write_to_stdout_ignore_sigpipe(&completions_flags.buf)?;
  Ok(0)
}

async fn types_command(flags: Flags) -> Result<i32, AnyError> {
  let types = get_types(flags.unstable);
  write_to_stdout_ignore_sigpipe(types.as_bytes())?;
  Ok(0)
}

async fn upgrade_command(
  _flags: Flags,
  upgrade_flags: UpgradeFlags,
) -> Result<i32, AnyError> {
  tools::upgrade::upgrade(upgrade_flags).await?;
  Ok(0)
}

async fn vendor_command(
  flags: Flags,
  vendor_flags: VendorFlags,
) -> Result<i32, AnyError> {
  tools::vendor::vendor(flags, vendor_flags).await?;
  Ok(0)
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
) -> Pin<Box<dyn Future<Output = Result<i32, AnyError>>>> {
  match flags.subcommand.clone() {
    DenoSubcommand::Bench(bench_flags) => {
      bench_command(flags, bench_flags).boxed_local()
    }
    DenoSubcommand::Bundle(bundle_flags) => {
      bundle_command(flags, bundle_flags).boxed_local()
    }
    DenoSubcommand::Doc(doc_flags) => {
      doc_command(flags, doc_flags).boxed_local()
    }
    DenoSubcommand::Eval(eval_flags) => {
      eval_command(flags, eval_flags).boxed_local()
    }
    DenoSubcommand::Cache(cache_flags) => {
      cache_command(flags, cache_flags).boxed_local()
    }
    DenoSubcommand::Check(check_flags) => {
      check_command(flags, check_flags).boxed_local()
    }
    DenoSubcommand::Compile(compile_flags) => {
      compile_command(flags, compile_flags).boxed_local()
    }
    DenoSubcommand::Coverage(coverage_flags) => {
      coverage_command(flags, coverage_flags).boxed_local()
    }
    DenoSubcommand::Fmt(fmt_flags) => {
      format_command(flags, fmt_flags).boxed_local()
    }
    DenoSubcommand::Info(info_flags) => {
      info_command(flags, info_flags).boxed_local()
    }
    DenoSubcommand::Install(install_flags) => {
      install_command(flags, install_flags).boxed_local()
    }
    DenoSubcommand::Uninstall(uninstall_flags) => {
      uninstall_command(uninstall_flags).boxed_local()
    }
    DenoSubcommand::Lsp => lsp_command().boxed_local(),
    DenoSubcommand::Lint(lint_flags) => {
      lint_command(flags, lint_flags).boxed_local()
    }
    DenoSubcommand::Repl(repl_flags) => {
      repl_command(flags, repl_flags).boxed_local()
    }
    DenoSubcommand::Run(run_flags) => {
      run_command(flags, run_flags).boxed_local()
    }
    DenoSubcommand::Task(task_flags) => {
      task_command(flags, task_flags).boxed_local()
    }
    DenoSubcommand::Test(test_flags) => {
      test_command(flags, test_flags).boxed_local()
    }
    DenoSubcommand::Completions(completions_flags) => {
      completions_command(flags, completions_flags).boxed_local()
    }
    DenoSubcommand::Types => types_command(flags).boxed_local(),
    DenoSubcommand::Upgrade(upgrade_flags) => {
      upgrade_command(flags, upgrade_flags).boxed_local()
    }
    DenoSubcommand::Vendor(vendor_flags) => {
      vendor_command(flags, vendor_flags).boxed_local()
    }
  }
}

fn setup_panic_hook() {
  // This function does two things inside of the panic hook:
  // - Tokio does not exit the process when a task panics, so we define a custom
  //   panic hook to implement this behaviour.
  // - We print a message to stderr to indicate that this is a bug in Deno, and
  //   should be reported to us.
  let orig_hook = std::panic::take_hook();
  std::panic::set_hook(Box::new(move |panic_info| {
    eprintln!("\n============================================================");
    eprintln!("Deno has panicked. This is a bug in Deno. Please report this");
    eprintln!("at https://github.com/denoland/deno/issues/new.");
    eprintln!("If you can reliably reproduce this panic, include the");
    eprintln!("reproduction steps and re-run with the RUST_BACKTRACE=1 env");
    eprintln!("var set and include the backtrace in your report.");
    eprintln!();
    eprintln!("Platform: {} {}", env::consts::OS, env::consts::ARCH);
    eprintln!("Version: {}", version::deno());
    eprintln!("Args: {:?}", env::args().collect::<Vec<_>>());
    eprintln!();
    orig_hook(panic_info);
    std::process::exit(1);
  }));
}

fn unwrap_or_exit<T>(result: Result<T, AnyError>) -> T {
  match result {
    Ok(value) => value,
    Err(error) => {
      let error_string = match error.downcast_ref::<JsError>() {
        Some(e) => format_js_error(e),
        None => format!("{:?}", error),
      };
      eprintln!(
        "{}: {}",
        colors::red_bold("error"),
        error_string.trim_start_matches("error: ")
      );
      std::process::exit(1);
    }
  }
}

pub fn main() {
  setup_panic_hook();

  unix_util::raise_fd_limit();
  windows_util::ensure_stdio_open();
  #[cfg(windows)]
  colors::enable_ansi(); // For Windows 10

  let args: Vec<String> = env::args().collect();

  let exit_code = async move {
    let standalone_res =
      match standalone::extract_standalone(args.clone()).await {
        Ok(Some((metadata, eszip))) => standalone::run(eszip, metadata).await,
        Ok(None) => Ok(()),
        Err(err) => Err(err),
      };
    // TODO(bartlomieju): doesn't handle exit code set by the runtime properly
    unwrap_or_exit(standalone_res);

    let flags = match flags_from_vec(args) {
      Ok(flags) => flags,
      Err(err @ clap::Error { .. })
        if err.kind() == clap::ErrorKind::DisplayHelp
          || err.kind() == clap::ErrorKind::DisplayVersion =>
      {
        err.print().unwrap();
        std::process::exit(0);
      }
      Err(err) => unwrap_or_exit(Err(AnyError::from(err))),
    };
    if !flags.v8_flags.is_empty() {
      init_v8_flags(&*flags.v8_flags);
    }

    logger::init(flags.log_level);

    let exit_code = get_subcommand(flags).await;

    exit_code
  };

  let exit_code = unwrap_or_exit(run_local(exit_code));

  std::process::exit(exit_code);
}
