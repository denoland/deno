// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

mod args;
mod auth_tokens;
mod cache;
mod deno_std;
mod emit;
mod errors;
mod file_fetcher;
mod graph_util;
mod http_util;
mod js;
mod lsp;
mod module_loader;
mod napi;
mod node;
mod npm;
mod ops;
mod proc_state;
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
use crate::proc_state::ProcState;
use crate::resolver::CliResolver;
use crate::util::display;

use args::CliOptions;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_core::error::JsError;
use deno_core::v8_set_flags;
use deno_runtime::colors;
use deno_runtime::fmt_errors::format_js_error;
use deno_runtime::tokio_util::run_local;
use std::env;
use std::iter::once;
use std::path::PathBuf;

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

async fn run_subcommand(flags: Flags) -> Result<i32, AnyError> {
  match flags.subcommand.clone() {
    DenoSubcommand::Bench(bench_flags) => {
      if flags.watch.is_some() {
        tools::bench::run_benchmarks_with_watch(flags, bench_flags).await?;
      } else {
        tools::bench::run_benchmarks(flags, bench_flags).await?;
      }
      Ok(0)
    }
    DenoSubcommand::Bundle(bundle_flags) => {
      tools::bundle::bundle(flags, bundle_flags).await?;
      Ok(0)
    }
    DenoSubcommand::Doc(doc_flags) => {
      tools::doc::print_docs(flags, doc_flags).await?;
      Ok(0)
    }
    DenoSubcommand::Eval(eval_flags) => {
      tools::run::eval_command(flags, eval_flags).await
    }
    DenoSubcommand::Cache(cache_flags) => {
      let ps = ProcState::build(flags).await?;
      ps.load_and_type_check_files(&cache_flags.files).await?;
      ps.cache_module_emits()?;
      Ok(0)
    }
    DenoSubcommand::Check(check_flags) => {
      let ps = ProcState::build(flags).await?;
      ps.load_and_type_check_files(&check_flags.files).await?;
      Ok(0)
    }
    DenoSubcommand::Compile(compile_flags) => {
      tools::standalone::compile(flags, compile_flags).await?;
      Ok(0)
    }
    DenoSubcommand::Coverage(coverage_flags) => {
      tools::coverage::cover_files(flags, coverage_flags).await?;
      Ok(0)
    }
    DenoSubcommand::Fmt(fmt_flags) => {
      let config = CliOptions::from_flags(flags)?;

      if fmt_flags.files.len() == 1
        && fmt_flags.files[0].to_string_lossy() == "-"
      {
        let maybe_fmt_config = config.to_fmt_config()?;
        tools::fmt::format_stdin(
          fmt_flags,
          maybe_fmt_config.map(|c| c.options).unwrap_or_default(),
        )?;
      } else {
        tools::fmt::format(&config, fmt_flags).await?;
      }
      Ok(0)
    }
    DenoSubcommand::Init(init_flags) => {
      tools::init::init_project(init_flags).await?;
      Ok(0)
    }
    DenoSubcommand::Info(info_flags) => {
      tools::info::info(flags, info_flags).await?;
      Ok(0)
    }
    DenoSubcommand::Install(install_flags) => {
      tools::installer::install_command(flags, install_flags).await?;
      Ok(0)
    }
    DenoSubcommand::Uninstall(uninstall_flags) => {
      tools::installer::uninstall(uninstall_flags.name, uninstall_flags.root)?;
      Ok(0)
    }
    DenoSubcommand::Lsp => {
      lsp::start().await?;
      Ok(0)
    }
    DenoSubcommand::Lint(lint_flags) => {
      if lint_flags.rules {
        tools::lint::print_rules_list(lint_flags.json);
      } else {
        tools::lint::lint(flags, lint_flags).await?;
      }
      Ok(0)
    }
    DenoSubcommand::Repl(repl_flags) => {
      tools::repl::run(flags, repl_flags).await
    }
    DenoSubcommand::Run(run_flags) => {
      if run_flags.is_stdin() {
        tools::run::run_from_stdin(flags).await
      } else {
        tools::run::run_script(flags, run_flags).await
      }
    }
    DenoSubcommand::Task(task_flags) => {
      tools::task::execute_script(flags, task_flags).await
    }
    DenoSubcommand::Test(test_flags) => {
      if let Some(ref coverage_dir) = flags.coverage_dir {
        std::fs::create_dir_all(coverage_dir)
          .with_context(|| format!("Failed creating: {}", coverage_dir))?;
        // this is set in order to ensure spawned processes use the same
        // coverage directory
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
    DenoSubcommand::Completions(completions_flags) => {
      display::write_to_stdout_ignore_sigpipe(&completions_flags.buf)?;
      Ok(0)
    }
    DenoSubcommand::Types => {
      let types = tsc::get_types_declaration_file_text(flags.unstable);
      display::write_to_stdout_ignore_sigpipe(types.as_bytes())?;
      Ok(0)
    }
    DenoSubcommand::Upgrade(upgrade_flags) => {
      tools::upgrade::upgrade(flags, upgrade_flags).await?;
      Ok(0)
    }
    DenoSubcommand::Vendor(vendor_flags) => {
      tools::vendor::vendor(flags, vendor_flags).await?;
      Ok(0)
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
      let mut error_string = format!("{:?}", error);
      let mut error_code = 1;

      if let Some(e) = error.downcast_ref::<JsError>() {
        error_string = format_js_error(e);
      } else if let Some(e) = error.downcast_ref::<args::LockfileError>() {
        error_string = e.to_string();
        error_code = 10;
      }

      eprintln!(
        "{}: {}",
        colors::red_bold("error"),
        error_string.trim_start_matches("error: ")
      );
      std::process::exit(error_code);
    }
  }
}

pub fn main() {
  setup_panic_hook();

  util::unix::raise_fd_limit();
  util::windows::ensure_stdio_open();
  #[cfg(windows)]
  colors::enable_ansi(); // For Windows 10
  deno_runtime::permissions::set_prompt_callbacks(
    Box::new(util::draw_thread::DrawThread::hide),
    Box::new(util::draw_thread::DrawThread::show),
  );

  let args: Vec<String> = env::args().collect();

  let future = async move {
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
      init_v8_flags(&flags.v8_flags);
    }

    util::logger::init(flags.log_level);

    run_subcommand(flags).await
  };

  let exit_code = unwrap_or_exit(run_local(future));

  std::process::exit(exit_code);
}
