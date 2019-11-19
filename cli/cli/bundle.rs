// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use super::args;
use super::DenoSubcommand;
use crate::flags::DenoFlags;
use clap::App;
use clap::Arg;
use clap::ArgMatches;
use clap::SubCommand;

pub fn bundle_subcommand<'a, 'b>() -> App<'a, 'b> {
  let subcmd = SubCommand::with_name("bundle")
    .about("Bundle module and dependencies into single file")
    .long_about(
      "Output a single JavaScript file with all dependencies.

If a out_file argument is omitted, the output of the bundle will be sent to
standard out.

Example:

  deno bundle https://deno.land/std/examples/colors.ts

  deno bundle https://deno.land/std/examples/colors.ts colors.bundle.js",
    )
    .arg(
      Arg::with_name("source_file")
        .takes_value(true)
        .required(true),
    )
    .arg(Arg::with_name("out_file").takes_value(true).required(false));

  subcmd
    .arg(args::log_level())
    .arg(args::reload())
    .args(&args::configuration())
    .arg(args::lock())
}

pub fn parse(
  flags: &mut DenoFlags,
  argv: &mut Vec<String>,
  matches: &ArgMatches,
) -> DenoSubcommand {
  args::parse_log_level(flags, matches);
  args::parse_reload(flags, matches);
  args::parse_configuration(flags, matches);
  args::parse_lock_args(flags, matches);
  flags.allow_write = true;
  let source_file: &str = matches.value_of("source_file").unwrap();
  let out_file = matches.value_of("out_file").map(String::from);
  match out_file {
    Some(out_file) => {
      argv.extend(vec![source_file.to_string(), out_file.to_string()])
    }
    _ => argv.extend(vec![source_file.to_string()]),
  }
  DenoSubcommand::Bundle
}
