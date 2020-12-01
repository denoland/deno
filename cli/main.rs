// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

extern crate lazy_static;
extern crate log;

use deno::flags::DenoSubcommand;
use deno::flags::Flags;
use deno::*;
use deno_core::error::AnyError;
use deno_core::futures::future::Future;
use deno_core::futures::future::FutureExt;
use deno_core::v8_set_flags;
use log::Level;
use log::LevelFilter;
use std::env;
use std::io::Write;
use std::iter::once;
use std::pin::Pin;

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

#[cfg(feature = "tools")]
fn get_subcommand(
  flags: Flags,
) -> Pin<Box<dyn Future<Output = Result<(), AnyError>>>> {
  match flags.clone().subcommand {
    DenoSubcommand::Run { script } => run_command(flags, script).boxed_local(),
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
    DenoSubcommand::Compile {
      source_file,
      output,
    } => compile_command(flags, source_file, output).boxed_local(),
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

#[cfg(not(feature = "tools"))]
fn get_subcommand(
  flags: Flags,
) -> Pin<Box<dyn Future<Output = Result<(), AnyError>>>> {
  use deno_core::error::generic_error;

  match flags.clone().subcommand {
    DenoSubcommand::Run { script } => run_command(flags, script).boxed_local(),
    _ => async { Err(generic_error("Toolchain not compiled")) }.boxed_local(),
  }
}

pub fn main() {
  #[cfg(windows)]
  colors::enable_ansi(); // For Windows 10

  let args: Vec<String> = env::args().collect();
  if let Err(err) = deno::standalone::try_run_standalone_binary(args.clone()) {
    eprintln!("{}: {}", colors::red_bold("error"), err.to_string());
    std::process::exit(1);
  }

  let flags = flags::flags_from_vec(args);
  if let Some(ref v8_flags) = flags.v8_flags {
    init_v8_flags(v8_flags);
  }
  init_logger(flags.log_level);
  let subcommand_fut = get_subcommand(flags);
  let result = tokio_util::run_basic(subcommand_fut);
  if let Err(err) = result {
    eprintln!("{}: {}", colors::red_bold("error"), err.to_string());
    std::process::exit(1);
  }
}
