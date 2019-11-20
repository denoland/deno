// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use super::args;
use super::DenoSubcommand;
use crate::cli::create_cli_app;
use crate::flags::DenoFlags;
use clap::App;
use clap::AppSettings;
use clap::ArgMatches;
use clap::SubCommand;

pub fn bootstrap_run<'a, 'b>(app: App<'a, 'b>) -> App<'a, 'b> {
  app
    .settings(&[
      AppSettings::AllowExternalSubcommands,
      AppSettings::DisableHelpSubcommand,
      AppSettings::SubcommandRequired,
    ]).about("Run a program given a filename or url to the source code")
    .long_about(
      "Run a program given a filename or url to the source code.

By default all programs are run in sandbox without access to disk, network or
ability to spawn subprocesses.

  deno run https://deno.land/welcome.ts

  # run program with permission to read from disk and listen to network
  deno run --allow-net --allow-read https://deno.land/std/http/file_server.ts

  # run program with permission to read whitelist files from disk and listen to network
  deno run --allow-net --allow-read=$(pwd) https://deno.land/std/http/file_server.ts

  # run program with all permissions
  deno run -A https://deno.land/std/http/file_server.ts",
    )
    .arg(args::log_level())
    .arg(args::reload())
    .args(&args::permissions())
    .args(&args::runtime())
    .args(&args::configuration())
    .arg(args::lock())
    .arg(args::lock_write())
    .subcommand(
      // this is a fake subcommand - it's used in conjunction with
      // AppSettings:AllowExternalSubcommand to treat it as an
      // entry point script
      SubCommand::with_name("[SCRIPT]").about("Script to run"),
    )
}

pub fn run_subcommand<'a, 'b>() -> App<'a, 'b> {
  let subcmd = SubCommand::with_name("run");
  bootstrap_run(subcmd)
}

/// Parse vector or arguments as DenoFlags.
///
/// This is very specialized utility that parses arguments passed after script URL.
///
/// Only dash (eg. `-r`) and double dash (eg. `--reload`) arguments are supported.
/// Arguments recognized as DenoFlags will be eaten.
/// Parsing stops after double dash `--` argument.
///
/// NOTE: this method ignores `-h/--help` and `-v/--version` flags.
fn parse_script_args(
  flags: &mut DenoFlags,
  argv: &mut Vec<String>,
  args: Vec<String>,
) {
  let mut seen_double_dash = false;

  // we have to iterate and parse argument one by one because clap returns error on any
  // unrecognized argument
  for arg in args.iter() {
    if seen_double_dash {
      argv.push(arg.to_string());
      continue;
    }

    if arg == "--" {
      seen_double_dash = true;
      argv.push(arg.to_string());
      continue;
    }

    if !arg.starts_with('-') || arg == "-" {
      argv.push(arg.to_string());
      continue;
    }

    let cli_app = create_cli_app();
    // `get_matches_from_safe` returns error for `-h/-v` flags
    let matches =
      cli_app.get_matches_from_safe(vec!["deno".to_string(), arg.to_string()]);

    if let Ok(m) = matches {
      args::parse_log_level(flags, &m);
      args::parse_reload(flags, &m);
      args::parse_permissions(flags, &m);
      args::parse_runtime(flags, &m);
      args::parse_configuration(flags, &m);
      args::parse_lock_args(flags, &m);
    } else {
      argv.push(arg.to_string());
    }
  }
}

pub fn parse(
  flags: &mut DenoFlags,
  argv: &mut Vec<String>,
  matches: &ArgMatches,
) -> DenoSubcommand {
  args::parse_log_level(flags, matches);
  args::parse_reload(flags, matches);
  args::parse_permissions(flags, matches);
  args::parse_runtime(flags, matches);
  args::parse_configuration(flags, matches);
  args::parse_lock_args(flags, matches);
  match matches.subcommand() {
    (script, Some(script_match)) => {
      argv.extend(vec![script.to_string()]);
      // check if there are any extra arguments that should
      // be passed to script
      if script_match.is_present("") {
        let script_args: Vec<String> = script_match
          .values_of("")
          .unwrap()
          .map(String::from)
          .collect();

        parse_script_args(flags, argv, script_args);
      }
      DenoSubcommand::Run
    }
    _ => unreachable!(),
  }
}
