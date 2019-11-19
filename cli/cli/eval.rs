// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use super::args;
use super::DenoSubcommand;
use crate::flags::DenoFlags;
use clap::App;
use clap::Arg;
use clap::ArgMatches;
use clap::SubCommand;

pub fn eval_subcommand<'a, 'b>() -> App<'a, 'b> {
  SubCommand::with_name("eval")
    .about("Eval script")
    .long_about(
      "Evaluate provided script.

This command has implicit access to all permissions (equivalent to deno run --allow-all)

  deno eval \"console.log('hello world')\"",
    ).arg(Arg::with_name("code").takes_value(true).required(true)).arg(args::log_level())
    .arg(args::reload())
}

pub fn parse(
  flags: &mut DenoFlags,
  argv: &mut Vec<String>,
  matches: &ArgMatches,
) -> DenoSubcommand {
  args::parse_log_level(flags, matches);
  args::parse_reload(flags, matches);
  flags.allow_net = true;
  flags.allow_env = true;
  flags.allow_run = true;
  flags.allow_read = true;
  flags.allow_write = true;
  flags.allow_hrtime = true;
  let code: &str = matches.value_of("code").unwrap();
  argv.extend(vec![code.to_string()]);
  DenoSubcommand::Eval
}
