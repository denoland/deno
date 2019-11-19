// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use super::args;
use super::DenoSubcommand;
use crate::flags::DenoFlags;
use clap::App;
use clap::Arg;
use clap::ArgMatches;
use clap::SubCommand;

pub fn info_subcommand<'a, 'b>() -> App<'a, 'b> {
  let subcmd = SubCommand::with_name("info")
    .about("Show info about cache or info related to source file")
    .long_about(
      "Show info about cache or info related to source file.

  deno info

The following information is shown:

  DENO_DIR:                  location of directory containing Deno-related files
  Remote modules cache:      location of directory containing remote modules
  TypeScript compiler cache: location of directory containing TS compiler output


  deno info https://deno.land/std@v0.11/http/file_server.ts

The following information is shown:

  local:    Local path of the file.
  type:     JavaScript, TypeScript, or JSON.
  compiled: TypeScript only. shown local path of compiled source code.
  map:      TypeScript only. shown local path of source map.
  deps:     Dependency tree of the source file.",
    )
    .arg(Arg::with_name("file").takes_value(true).required(false));

  subcmd
    .arg(args::reload())
    .args(&args::configuration())
    .arg(args::lock())
}

pub fn parse(
  flags: &mut DenoFlags,
  argv: &mut Vec<String>,
  matches: &ArgMatches,
) -> DenoSubcommand {
  args::parse_reload(flags, matches);
  args::parse_configuration(flags, matches);
  args::parse_lock_args(flags, matches);
  if matches.is_present("file") {
    argv.push(matches.value_of("file").unwrap().to_string());
  }
  DenoSubcommand::Info
}
