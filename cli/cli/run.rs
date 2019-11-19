// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use super::args;
use super::DenoSubcommand;
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

        argv.extend(script_args);
      }
      DenoSubcommand::Run
    }
    _ => unreachable!(),
  }
}
