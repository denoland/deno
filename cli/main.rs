// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

mod args;
mod auth_tokens;
mod cache;
mod cdp;
mod emit;
mod errors;
mod factory;
mod file_fetcher;
mod graph_container;
mod graph_util;
mod http_util;
mod js;
mod jsr;
mod lsp;
mod module_loader;
mod napi;
mod node;
mod npm;
mod ops;
mod resolver;
mod standalone;
mod tools;
mod tsc;
mod util;
mod version;
mod worker;

use crate::args::flags_from_vec;
use crate::args::DenoSubcommand;
use crate::args::Flags;
use crate::args::DENO_FUTURE;
use crate::graph_container::ModuleGraphContainer;
use crate::util::display;
use crate::util::v8::get_v8_flags_from_env;
use crate::util::v8::init_v8_flags;

use deno_runtime::WorkerExecutionMode;
pub use deno_runtime::UNSTABLE_GRANULAR_FLAGS;

use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_core::error::JsError;
use deno_core::futures::FutureExt;
use deno_core::unsync::JoinHandle;
use deno_npm::resolution::SnapshotFromLockfileError;
use deno_runtime::fmt_errors::format_js_error;
use deno_runtime::tokio_util::create_and_run_current_thread_with_maybe_metrics;
use deno_terminal::colors;
use factory::CliFactory;
use std::env;
use std::future::Future;
use std::path::PathBuf;

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

async fn run_subcommand(flags: Flags) -> Result<i32, AnyError> {
  let handle = match flags.subcommand.clone() {
    DenoSubcommand::Add(add_flags) => spawn_subcommand(async {
      tools::registry::add(flags, add_flags).await
    }),
    DenoSubcommand::Bench(bench_flags) => spawn_subcommand(async {
      if bench_flags.watch.is_some() {
        tools::bench::run_benchmarks_with_watch(flags, bench_flags).await
      } else {
        tools::bench::run_benchmarks(flags, bench_flags).await
      }
    }),
    DenoSubcommand::Bundle(bundle_flags) => spawn_subcommand(async {
      tools::bundle::bundle(flags, bundle_flags).await
    }),
    DenoSubcommand::Doc(doc_flags) => {
      spawn_subcommand(async { tools::doc::doc(flags, doc_flags).await })
    }
    DenoSubcommand::Eval(eval_flags) => spawn_subcommand(async {
      tools::run::eval_command(flags, eval_flags).await
    }),
    DenoSubcommand::Cache(cache_flags) => spawn_subcommand(async move {
      let factory = CliFactory::from_flags(flags)?;
      let emitter = factory.emitter()?;
      let main_graph_container =
        factory.main_module_graph_container().await?;
      main_graph_container
        .load_and_type_check_files(&cache_flags.files)
        .await?;
      emitter.cache_module_emits(&main_graph_container.graph()).await
    }),
    DenoSubcommand::Check(check_flags) => spawn_subcommand(async move {
      let factory = CliFactory::from_flags(flags)?;
      let main_graph_container =
        factory.main_module_graph_container().await?;
      main_graph_container
        .load_and_type_check_files(&check_flags.files)
        .await
    }),
    DenoSubcommand::Compile(compile_flags) => spawn_subcommand(async {
      tools::compile::compile(flags, compile_flags).await
    }),
    DenoSubcommand::Coverage(coverage_flags) => spawn_subcommand(async {
      tools::coverage::cover_files(flags, coverage_flags).await
    }),
    DenoSubcommand::Fmt(fmt_flags) => {
      spawn_subcommand(
        async move { tools::fmt::format(flags, fmt_flags).await },
      )
    }
    DenoSubcommand::Init(init_flags) => {
      spawn_subcommand(async {
        // make compiler happy since init_project is sync
        tokio::task::yield_now().await;
        tools::init::init_project(init_flags)
      })
    }
    DenoSubcommand::Info(info_flags) => {
      spawn_subcommand(async { tools::info::info(flags, info_flags).await })
    }
    DenoSubcommand::Install(install_flags) => spawn_subcommand(async {
      tools::installer::install_command(flags, install_flags).await
    }),
    DenoSubcommand::Jupyter(jupyter_flags) => spawn_subcommand(async {
      tools::jupyter::kernel(flags, jupyter_flags).await
    }),
    DenoSubcommand::Uninstall(uninstall_flags) => spawn_subcommand(async {
      tools::installer::uninstall(uninstall_flags)
    }),
    DenoSubcommand::Lsp => spawn_subcommand(async { lsp::start().await }),
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
    DenoSubcommand::Repl(repl_flags) => {
      spawn_subcommand(async move { tools::repl::run(flags, repl_flags).await })
    }
    DenoSubcommand::Run(run_flags) => spawn_subcommand(async move {
      if run_flags.is_stdin() {
        tools::run::run_from_stdin(flags).await
      } else if flags.eszip {
        tools::run::run_eszip(flags).await
      } else {
        tools::run::run_script(WorkerExecutionMode::Run, flags, run_flags.watch).await
      }
    }),
    DenoSubcommand::Serve(serve_flags) => spawn_subcommand(async move {
      tools::run::run_script(WorkerExecutionMode::Serve, flags, serve_flags.watch).await
    }),
    DenoSubcommand::Task(task_flags) => spawn_subcommand(async {
      tools::task::execute_script(flags, task_flags).await
    }),
    DenoSubcommand::Test(test_flags) => {
      spawn_subcommand(async {
        if let Some(ref coverage_dir) = test_flags.coverage_dir {
          if test_flags.clean {
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
    DenoSubcommand::Vendor(vendor_flags) => spawn_subcommand(async {
      tools::vendor::vendor(flags, vendor_flags).await
    }),
    // TODO:
    DenoSubcommand::Publish(publish_flags) => spawn_subcommand(async {
      tools::registry::publish(flags, publish_flags).await
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
    eprintln!("Version: {}", version::deno());
    eprintln!("Args: {:?}", env::args().collect::<Vec<_>>());
    eprintln!();
    orig_hook(panic_info);
    std::process::exit(1);
  }));
}

#[allow(clippy::print_stderr)]
fn exit_with_message(message: &str, code: i32) -> ! {
  eprintln!(
    "{}: {}",
    colors::red_bold("error"),
    message.trim_start_matches("error: ")
  );
  std::process::exit(code);
}

fn exit_for_error(error: AnyError) -> ! {
  let mut error_string = format!("{error:?}");
  let mut error_code = 1;

  if let Some(e) = error.downcast_ref::<JsError>() {
    error_string = format_js_error(e);
  } else if let Some(SnapshotFromLockfileError::IntegrityCheckFailed(e)) =
    error.downcast_ref::<SnapshotFromLockfileError>()
  {
    error_string = e.to_string();
    error_code = 10;
  }

  exit_with_message(&error_string, error_code);
}

#[allow(clippy::print_stderr)]
pub(crate) fn unstable_exit_cb(feature: &str, api_name: &str) {
  eprintln!(
    "Unstable API '{api_name}'. The `--unstable-{}` flag must be provided.",
    feature
  );
  std::process::exit(70);
}

// TODO(bartlomieju): remove when `--unstable` flag is removed.
#[allow(clippy::print_stderr)]
pub(crate) fn unstable_warn_cb(feature: &str, api_name: &str) {
  eprintln!(
    "⚠️  {}",
    colors::yellow(format!(
      "The `{}` API was used with `--unstable` flag. The `--unstable` flag is deprecated and will be removed in Deno 2.0. Use granular `--unstable-{}` instead.\nLearn more at: https://docs.deno.com/runtime/manual/tools/unstable_flags",
      api_name, feature
    ))
  );
}

pub fn main() {
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
    run_subcommand(flags).await
  };

  match create_and_run_current_thread_with_maybe_metrics(future) {
    Ok(exit_code) => std::process::exit(exit_code),
    Err(err) => exit_for_error(err),
  }
}

fn resolve_flags_and_init(
  args: Vec<std::ffi::OsString>,
) -> Result<Flags, AnyError> {
  let flags = match flags_from_vec(args) {
    Ok(flags) => flags,
    Err(err @ clap::Error { .. })
      if err.kind() == clap::error::ErrorKind::DisplayHelp
        || err.kind() == clap::error::ErrorKind::DisplayVersion =>
    {
      err.print().unwrap();
      std::process::exit(0);
    }
    Err(err) => exit_for_error(AnyError::from(err)),
  };

  // TODO(bartlomieju): remove when `--unstable` flag is removed.
  if flags.unstable_config.legacy_flag_enabled {
    #[allow(clippy::print_stderr)]
    if matches!(flags.subcommand, DenoSubcommand::Check(_)) {
      // can't use log crate because that's not setup yet
      eprintln!(
        "⚠️  {}",
        colors::yellow(
          "The `--unstable` flag is not needed for `deno check` anymore."
        )
      );
    } else {
      eprintln!(
        "⚠️  {}",
        colors::yellow(
          "The `--unstable` flag is deprecated and will be removed in Deno 2.0. Use granular `--unstable-*` flags instead.\nLearn more at: https://docs.deno.com/runtime/manual/tools/unstable_flags"
        )
      );
    }
  }

  let default_v8_flags = match flags.subcommand {
    // Using same default as VSCode:
    // https://github.com/microsoft/vscode/blob/48d4ba271686e8072fc6674137415bc80d936bc7/extensions/typescript-language-features/src/configuration/configuration.ts#L213-L214
    DenoSubcommand::Lsp => vec!["--max-old-space-size=3072".to_string()],
    _ => {
      if *DENO_FUTURE {
        // deno_ast removes TypeScript `assert` keywords, so this flag only affects JavaScript
        // TODO(petamoriken): Need to check TypeScript `assert` keywords in deno_ast
        vec!["--no-harmony-import-assertions".to_string()]
      } else {
        // If we're still in v1.X version we want to support import assertions.
        // V8 12.6 unshipped the support by default, so force it by passing a
        // flag.
        vec!["--harmony-import-assertions".to_string()]
      }
    }
  };

  init_v8_flags(&default_v8_flags, &flags.v8_flags, get_v8_flags_from_env());
  deno_core::JsRuntime::init_platform(None);
  util::logger::init(flags.log_level);

  Ok(flags)
}
