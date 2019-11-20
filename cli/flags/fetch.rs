// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use super::args;
use super::DenoSubcommand;
use crate::flags::DenoFlags;
use clap::App;
use clap::Arg;
use clap::ArgMatches;
use clap::SubCommand;

pub fn fetch_subcommand<'a, 'b>() -> App<'a, 'b> {
  let subcmd = SubCommand::with_name("fetch")
    .about("Fetch the dependencies")
    .long_about(
      "Fetch and compile remote dependencies recursively.

Downloads all statically imported scripts and save them in local
cache, without running the code. No future import network requests
would be made unless --reload is specified.

  # Downloads all dependencies
  deno fetch https://deno.land/std/http/file_server.ts

  # Once cached, static imports no longer send network requests
  deno run -A https://deno.land/std/http/file_server.ts",
    )
    .arg(Arg::with_name("file").takes_value(true).required(true));

  subcmd
    .arg(args::log_level())
    .arg(args::reload())
    .arg(args::no_fetch())
    .args(&args::configuration())
    .arg(args::lock())
    .arg(args::lock_write())
}

pub fn parse(
  flags: &mut DenoFlags,
  argv: &mut Vec<String>,
  matches: &ArgMatches,
) -> DenoSubcommand {
  args::parse_log_level(flags, matches);
  args::parse_reload(flags, matches);
  args::parse_no_fetch(flags, matches);
  args::parse_configuration(flags, matches);
  args::parse_lock_args(flags, matches);
  let file: &str = matches.value_of("file").unwrap();
  argv.extend(vec![file.to_string()]);
  DenoSubcommand::Fetch
}
