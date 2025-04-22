// Copyright 2018-2025 the Deno authors. MIT license.

mod args;
mod cache;
mod cdp;
mod emit;
mod factory;
mod file_fetcher;
mod graph_container;
mod graph_util;
mod http_util;
mod jsr;
mod lsp;
mod module_loader;
mod node;
mod npm;
mod ops;
mod registry;
mod resolver;
mod standalone;
mod task_runner;
mod tools;
mod tsc;
mod type_checker;
mod util;
mod worker;

pub mod sys {
  #[allow(clippy::disallowed_types)] // ok, definition
  pub type CliSys = sys_traits::impls::RealSys;
}

use std::env;
use std::future::Future;
use std::io::IsTerminal;
use std::ops::Deref;
use std::path::PathBuf;
use std::sync::Arc;

use args::TaskFlags;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_core::error::CoreError;
use deno_core::futures::FutureExt;
use deno_core::unsync::JoinHandle;
use deno_lib::util::result::any_and_jserrorbox_downcast_ref;
use deno_resolver::npm::ByonmResolvePkgFolderFromDenoReqError;
use deno_resolver::npm::ResolvePkgFolderFromDenoReqError;
use deno_runtime::fmt_errors::format_js_error;
use deno_runtime::tokio_util::create_and_run_current_thread_with_maybe_metrics;
use deno_runtime::WorkerExecutionMode;
pub use deno_runtime::UNSTABLE_GRANULAR_FLAGS;
use deno_telemetry::OtelConfig;
use deno_terminal::colors;
use factory::CliFactory;

const MODULE_NOT_FOUND: &str = "Module not found";
const UNSUPPORTED_SCHEME: &str = "Unsupported scheme";

use self::util::draw_thread::DrawThread;
use crate::args::flags_from_vec;
use crate::args::DenoSubcommand;
use crate::args::Flags;
use crate::util::display;
use crate::util::v8::get_v8_flags_from_env;
use crate::util::v8::init_v8_flags;

#[cfg(feature = "dhat-heap")]
#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

/// Ensures that all subcommands return an i32 exit code and an [`AnyError`] error type.
trait SubcommandOutput {
  fn output(self) -> Result<i32, AnyError>;
}

impl SubcommandOutput for Result<i32, AnyError> {
  fn output(self) -> Result<i32, AnyError> {
    self
  }
}

impl SubcommandOutput for Result<(), AnyError> {
  fn output(self) -> Result<i32, AnyError> {
    self.map(|_| 0)
  }
}

impl SubcommandOutput for Result<(), std::io::Error> {
  fn output(self) -> Result<i32, AnyError> {
    self.map(|_| 0).map_err(|e| e.into())
  }
}

/// Ensure that the subcommand runs in a task, rather than being directly executed. Since some of these
/// futures are very large, this prevents the stack from getting blown out from passing them by value up
/// the callchain (especially in debug mode when Rust doesn't have a chance to elide copies!).
#[inline(always)]
fn spawn_subcommand<F: Future<Output = T> + 'static, T: SubcommandOutput>(
  f: F,
) -> JoinHandle<Result<i32, AnyError>> {
  // the boxed_local() is important in order to get windows to not blow the stack in debug
  deno_core::unsync::spawn(
    async move { f.map(|r| r.output()).await }.boxed_local(),
  )
}

async fn run_subcommand(flags: Arc<Flags>) -> Result<i32, AnyError> {
  let handle = match flags.subcommand.clone() {
    DenoSubcommand::Add(add_flags) => spawn_subcommand(async {
      tools::pm::add(flags, add_flags, tools::pm::AddCommandName::Add).await
    }),
    DenoSubcommand::Remove(remove_flags) => spawn_subcommand(async {
      tools::pm::remove(flags, remove_flags).await
    }),
    DenoSubcommand::Bench(bench_flags) => spawn_subcommand(async {
      if bench_flags.watch.is_some() {
        tools::bench::run_benchmarks_with_watch(flags, bench_flags).boxed_local().await
      } else {
        tools::bench::run_benchmarks(flags, bench_flags).await
      }
    }),
    DenoSubcommand::Bundle => exit_with_message("⚠️ `deno bundle` was removed in Deno 2.\n\nSee the Deno 1.x to 2.x Migration Guide for migration instructions: https://docs.deno.com/runtime/manual/advanced/migrate_deprecations", 1),
    DenoSubcommand::Doc(doc_flags) => {
      spawn_subcommand(async { tools::doc::doc(flags, doc_flags).await })
    }
    DenoSubcommand::Eval(eval_flags) => spawn_subcommand(async {
      tools::run::eval_command(flags, eval_flags).await
    }),
    DenoSubcommand::Cache(cache_flags) => spawn_subcommand(async move {
      tools::installer::install_from_entrypoints(flags, &cache_flags.files).await
    }),
    DenoSubcommand::Check(check_flags) => spawn_subcommand(async move {
      tools::check::check(flags, check_flags).await
    }),
    DenoSubcommand::Clean(clean_flags) => spawn_subcommand(async move {
      tools::clean::clean(flags, clean_flags).await
    }),
    DenoSubcommand::Compile(compile_flags) => spawn_subcommand(async {
      if compile_flags.eszip {
        tools::compile::compile_eszip(flags, compile_flags).boxed_local().await
      } else {
        tools::compile::compile(flags, compile_flags).await
      }
    }),
    DenoSubcommand::Coverage(coverage_flags) => spawn_subcommand(async move {
      let reporter = crate::tools::coverage::reporter::create(coverage_flags.r#type.clone());
      tools::coverage::cover_files(
        flags,
        coverage_flags.files.include,
        coverage_flags.files.ignore,
        coverage_flags.include,
        coverage_flags.exclude,
        coverage_flags.output,
        &[&*reporter]
      )
    }),
    DenoSubcommand::Fmt(fmt_flags) => {
      spawn_subcommand(
        async move { tools::fmt::format(flags, fmt_flags).await },
      )
    }
    DenoSubcommand::Init(init_flags) => {
      spawn_subcommand(async {
        tools::init::init_project(init_flags).await
      })
    }
    DenoSubcommand::Info(info_flags) => {
      spawn_subcommand(async { tools::info::info(flags, info_flags).await })
    }
    DenoSubcommand::Install(install_flags) => spawn_subcommand(async {
      tools::installer::install_command(flags, install_flags).await
    }),
    DenoSubcommand::JSONReference(json_reference) => spawn_subcommand(async move {
      display::write_to_stdout_ignore_sigpipe(&deno_core::serde_json::to_vec_pretty(&json_reference.json).unwrap())
    }),
    DenoSubcommand::Jupyter(jupyter_flags) => spawn_subcommand(async {
      tools::jupyter::kernel(flags, jupyter_flags).await
    }),
    DenoSubcommand::Uninstall(uninstall_flags) => spawn_subcommand(async {
      tools::installer::uninstall(flags, uninstall_flags).await
    }),
    DenoSubcommand::Lsp => spawn_subcommand(async move {
      if std::io::stderr().is_terminal() {
        log::warn!(
          "{} command is intended to be run by text editors and IDEs and shouldn't be run manually.

  Visit https://docs.deno.com/runtime/getting_started/setup_your_environment/ for instruction
  how to setup your favorite text editor.

  Press Ctrl+C to exit.
        ", colors::cyan("deno lsp"));
      }
      let factory = CliFactory::from_flags(flags.clone());
      lsp::start(Arc::new(factory.lockfile_npm_package_info_provider()?)).await
    }),
    DenoSubcommand::Lint(lint_flags) => spawn_subcommand(async {
      if lint_flags.rules {
        tools::lint::print_rules_list(
          lint_flags.json,
          lint_flags.maybe_rules_tags,
        );
        Ok(())
      } else {
        tools::lint::lint(flags, lint_flags).await
      }
    }),
    DenoSubcommand::Outdated(update_flags) => {
      spawn_subcommand(async move {
        tools::pm::outdated(flags, update_flags).await
      })
    }
    DenoSubcommand::Repl(repl_flags) => {
      spawn_subcommand(async move { tools::repl::run(flags, repl_flags).await })
    }
    DenoSubcommand::Run(run_flags) => spawn_subcommand(async move {
      if run_flags.is_stdin() {
        // these futures are boxed to prevent stack overflows on Windows
        tools::run::run_from_stdin(flags.clone()).boxed_local().await
      } else if flags.eszip {
        tools::run::run_eszip(flags, run_flags).boxed_local().await
      } else {
        let result = tools::run::run_script(WorkerExecutionMode::Run, flags.clone(), run_flags.watch).await;
        match result {
          Ok(v) => Ok(v),
          Err(script_err) => {
            if let Some(worker::CreateCustomWorkerError::ResolvePkgFolderFromDenoReq(ResolvePkgFolderFromDenoReqError::Byonm(ByonmResolvePkgFolderFromDenoReqError::UnmatchedReq(_)))) = any_and_jserrorbox_downcast_ref::<worker::CreateCustomWorkerError>(&script_err) {
              if flags.node_modules_dir.is_none() {
                let mut flags = flags.deref().clone();
                let watch = match &flags.subcommand {
                  DenoSubcommand::Run(run_flags) => run_flags.watch.clone(),
                  _ => unreachable!(),
                };
                flags.node_modules_dir = Some(deno_config::deno_json::NodeModulesDirMode::None);
                // use the current lockfile, but don't write it out
                if flags.frozen_lockfile.is_none() {
                  flags.internal.lockfile_skip_write = true;
                }
                return tools::run::run_script(WorkerExecutionMode::Run, Arc::new(flags), watch).boxed_local().await;
              }
            }
            let script_err_msg = script_err.to_string();
            if script_err_msg.starts_with(MODULE_NOT_FOUND) || script_err_msg.starts_with(UNSUPPORTED_SCHEME) {
              if run_flags.bare {
                let mut cmd = args::clap_root();
                cmd.build();
                let command_names = cmd.get_subcommands().map(|command| command.get_name()).collect::<Vec<_>>();
                let suggestions = args::did_you_mean(&run_flags.script, command_names);
                if !suggestions.is_empty() {
                  let mut error = clap::error::Error::<clap::error::DefaultFormatter>::new(clap::error::ErrorKind::InvalidSubcommand).with_cmd(&cmd);
                  error.insert(
                    clap::error::ContextKind::SuggestedSubcommand,
                    clap::error::ContextValue::Strings(suggestions),
                  );

                  Err(error.into())
                } else {
                  Err(script_err)
                }
              } else {
                let mut new_flags = flags.deref().clone();
                let task_flags = TaskFlags {
                  cwd: None,
                  task: Some(run_flags.script.clone()),
                  is_run: true,
                  recursive: false,
                  filter: None,
                  eval: false,
                };
                new_flags.subcommand = DenoSubcommand::Task(task_flags.clone());
                let result = tools::task::execute_script(Arc::new(new_flags), task_flags.clone()).await;
                match result {
                  Ok(v) => Ok(v),
                  Err(_) => {
                    // Return script error for backwards compatibility.
                    Err(script_err)
                  }
                }
              }
            } else {
              Err(script_err)
            }
          }
        }
      }
    }),
    DenoSubcommand::Serve(serve_flags) => spawn_subcommand(async move {
      tools::serve::serve(flags, serve_flags).await
    }),
    DenoSubcommand::Task(task_flags) => spawn_subcommand(async {
      tools::task::execute_script(flags, task_flags).await
    }),
    DenoSubcommand::Test(test_flags) => {
      spawn_subcommand(async {
        if let Some(ref coverage_dir) = test_flags.coverage_dir {
          if !test_flags.coverage_raw_data_only || test_flags.clean {
            // Keeps coverage_dir contents only when --coverage-raw-data-only is set and --clean is not set
            let _ = std::fs::remove_dir_all(coverage_dir);
          }
          std::fs::create_dir_all(coverage_dir)
            .with_context(|| format!("Failed creating: {coverage_dir}"))?;
          // this is set in order to ensure spawned processes use the same
          // coverage directory
          env::set_var(
            "DENO_UNSTABLE_COVERAGE_DIR",
            PathBuf::from(coverage_dir).canonicalize()?,
          );
        }

        if test_flags.watch.is_some() {
          tools::test::run_tests_with_watch(flags, test_flags).await
        } else {
          tools::test::run_tests(flags, test_flags).await
        }
      })
    }
    DenoSubcommand::Completions(completions_flags) => {
      spawn_subcommand(async move {
        display::write_to_stdout_ignore_sigpipe(&completions_flags.buf)
      })
    }
    DenoSubcommand::Types => spawn_subcommand(async move {
      let types = tsc::get_types_declaration_file_text();
      display::write_to_stdout_ignore_sigpipe(types.as_bytes())
    }),
    #[cfg(feature = "upgrade")]
    DenoSubcommand::Upgrade(upgrade_flags) => spawn_subcommand(async {
      tools::upgrade::upgrade(flags, upgrade_flags).await
    }),
    #[cfg(not(feature = "upgrade"))]
    DenoSubcommand::Upgrade(_) => exit_with_message(
      "This deno was built without the \"upgrade\" feature. Please upgrade using the installation method originally used to install Deno.",
      1,
    ),
    DenoSubcommand::Vendor => exit_with_message("⚠️ `deno vendor` was removed in Deno 2.\n\nSee the Deno 1.x to 2.x Migration Guide for migration instructions: https://docs.deno.com/runtime/manual/advanced/migrate_deprecations", 1),
    DenoSubcommand::Publish(publish_flags) => spawn_subcommand(async {
      tools::publish::publish(flags, publish_flags).await
    }),
    DenoSubcommand::Help(help_flags) => spawn_subcommand(async move {
      use std::io::Write;

      let mut stream = anstream::AutoStream::new(std::io::stdout(), if colors::use_color() {
        anstream::ColorChoice::Auto
      } else {
        anstream::ColorChoice::Never
      });

      match stream.write_all(help_flags.help.ansi().to_string().as_bytes()) {
        Ok(()) => Ok(()),
        Err(e) => match e.kind() {
          std::io::ErrorKind::BrokenPipe => Ok(()),
          _ => Err(e),
        },
      }
    }),
  };

  handle.await?
}

#[allow(clippy::print_stderr)]
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
    eprintln!("Version: {}", deno_lib::version::DENO_VERSION_INFO.deno);
    eprintln!("Args: {:?}", env::args().collect::<Vec<_>>());
    eprintln!();

    // Panic traces are not supported for custom/development builds.
    #[cfg(feature = "panic-trace")]
    {
      let info = &deno_lib::version::DENO_VERSION_INFO;
      let version =
        if info.release_channel == deno_lib::shared::ReleaseChannel::Canary {
          format!("{}+{}", deno_lib::version::DENO_VERSION, info.git_hash)
        } else {
          info.deno.to_string()
        };

      let trace = deno_panic::trace();
      eprintln!("View stack trace at:");
      eprintln!(
        "https://panic.deno.com/v{}/{}/{}",
        version,
        env!("TARGET"),
        trace
      );
    }

    orig_hook(panic_info);
    deno_runtime::exit(1);
  }));
}

fn exit_with_message(message: &str, code: i32) -> ! {
  log::error!(
    "{}: {}",
    colors::red_bold("error"),
    message.trim_start_matches("error: ")
  );
  deno_runtime::exit(code);
}

fn exit_for_error(error: AnyError) -> ! {
  let mut error_string = format!("{error:?}");
  if let Some(CoreError::Js(e)) =
    any_and_jserrorbox_downcast_ref::<CoreError>(&error)
  {
    error_string = format_js_error(e);
  }

  exit_with_message(&error_string, 1);
}

pub(crate) fn unstable_exit_cb(feature: &str, api_name: &str) {
  log::error!(
    "Unstable API '{api_name}'. The `--unstable-{}` flag must be provided.",
    feature
  );
  deno_runtime::exit(70);
}

pub fn main() {
  #[cfg(feature = "dhat-heap")]
  let profiler = dhat::Profiler::new_heap();

  setup_panic_hook();

  util::unix::raise_fd_limit();
  util::windows::ensure_stdio_open();
  #[cfg(windows)]
  colors::enable_ansi(); // For Windows 10
  deno_runtime::deno_permissions::set_prompt_callbacks(
    Box::new(util::draw_thread::DrawThread::hide),
    Box::new(util::draw_thread::DrawThread::show),
  );

  let args: Vec<_> = env::args_os().collect();
  let future = async move {
    // NOTE(lucacasonato): due to new PKU feature introduced in V8 11.6 we need to
    // initialize the V8 platform on a parent thread of all threads that will spawn
    // V8 isolates.
    let flags = resolve_flags_and_init(args)?;
    run_subcommand(Arc::new(flags)).await
  };

  let result = create_and_run_current_thread_with_maybe_metrics(future);

  #[cfg(feature = "dhat-heap")]
  drop(profiler);

  match result {
    Ok(exit_code) => deno_runtime::exit(exit_code),
    Err(err) => exit_for_error(err),
  }
}

fn resolve_flags_and_init(
  args: Vec<std::ffi::OsString>,
) -> Result<Flags, AnyError> {
  let flags = match flags_from_vec(args) {
    Ok(flags) => flags,
    Err(err @ clap::Error { .. })
      if err.kind() == clap::error::ErrorKind::DisplayVersion =>
    {
      // Ignore results to avoid BrokenPipe errors.
      init_logging(None, None);
      let _ = err.print();
      deno_runtime::exit(0);
    }
    Err(err) => {
      init_logging(None, None);
      exit_for_error(AnyError::from(err))
    }
  };

  let otel_config = flags.otel_config();
  deno_telemetry::init(
    deno_lib::version::otel_runtime_config(),
    otel_config.clone(),
  )?;
  init_logging(flags.log_level, Some(otel_config));

  // TODO(bartlomieju): remove in Deno v2.5 and hard error then.
  if flags.unstable_config.legacy_flag_enabled {
    log::warn!(
      "⚠️  {}",
      colors::yellow(
        "The `--unstable` flag has been removed in Deno 2.0. Use granular `--unstable-*` flags instead.\nLearn more at: https://docs.deno.com/runtime/manual/tools/unstable_flags"
      )
    );
  }

  let default_v8_flags = match flags.subcommand {
    DenoSubcommand::Lsp => vec![
      "--stack-size=1024".to_string(),
      "--js-explicit-resource-management".to_string(),
      // Using same default as VSCode:
      // https://github.com/microsoft/vscode/blob/48d4ba271686e8072fc6674137415bc80d936bc7/extensions/typescript-language-features/src/configuration/configuration.ts#L213-L214
      "--max-old-space-size=3072".to_string(),
    ],
    _ => {
      vec![
        "--stack-size=1024".to_string(),
        "--js-explicit-resource-management".to_string(),
        // TODO(bartlomieju): I think this can be removed as it's handled by `deno_core`
        // and its settings.
        // deno_ast removes TypeScript `assert` keywords, so this flag only affects JavaScript
        // TODO(petamoriken): Need to check TypeScript `assert` keywords in deno_ast
        "--no-harmony-import-assertions".to_string(),
      ]
    }
  };

  init_v8_flags(&default_v8_flags, &flags.v8_flags, get_v8_flags_from_env());
  // TODO(bartlomieju): remove last argument once Deploy no longer needs it
  deno_core::JsRuntime::init_platform(
    None, /* import assertions enabled */ false,
  );

  Ok(flags)
}

fn init_logging(
  maybe_level: Option<log::Level>,
  otel_config: Option<OtelConfig>,
) {
  deno_lib::util::logger::init(deno_lib::util::logger::InitLoggingOptions {
    maybe_level,
    otel_config,
    // it was considered to hold the draw thread's internal lock
    // across logging, but if outputting to stderr blocks then that
    // could potentially block other threads that access the draw
    // thread's state
    on_log_start: DrawThread::hide,
    on_log_end: DrawThread::show,
  })
}
