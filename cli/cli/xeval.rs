// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use super::args;
use super::DenoSubcommand;
use super::XEVAL_URL;
use crate::flags::DenoFlags;
use clap::App;
use clap::Arg;
use clap::ArgMatches;
use clap::SubCommand;

pub fn xeval_subcommand<'a, 'b>() -> App<'a, 'b> {
  SubCommand::with_name("xeval")
    .about("Eval a script on text segments from stdin")
    .long_about(
      "Eval a script on lines from stdin

Read from standard input and eval code on each whitespace-delimited
string chunks.

-I/--replvar optionally sets variable name for input to be used in eval.
Otherwise '$' will be used as default variable name.

This command has implicit access to all permissions (equivalent to deno run --allow-all)

Print all the usernames in /etc/passwd:

  cat /etc/passwd | deno xeval \"a = $.split(':'); if (a) console.log(a[0])\"

A complicated way to print the current git branch:

  git branch | deno xeval -I 'line' \"if (line.startsWith('*')) console.log(line.slice(2))\"

Demonstrates breaking the input up by space delimiter instead of by lines:

  cat LICENSE | deno xeval -d \" \" \"if ($ === 'MIT') console.log('MIT licensed')\"",
    ).arg(args::log_level())
    .arg(args::reload()).arg(
    Arg::with_name("replvar")
      .long("replvar")
      .short("I")
      .help("Set variable name to be used in eval, defaults to $")
      .takes_value(true),
  ).arg(
    Arg::with_name("delim")
      .long("delim")
      .short("d")
      .help("Set delimiter, defaults to newline")
      .takes_value(true),
  ).arg(Arg::with_name("code").takes_value(true).required(true))
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
  argv.push(XEVAL_URL.to_string());

  if matches.is_present("delim") {
    let delim = matches.value_of("delim").unwrap();
    argv.push("--delim".to_string());
    argv.push(delim.to_string());
  }

  if matches.is_present("replvar") {
    let replvar = matches.value_of("replvar").unwrap();
    argv.push("--replvar".to_string());
    argv.push(replvar.to_string());
  }

  let code: &str = matches.value_of("code").unwrap();
  argv.push(code.to_string());

  DenoSubcommand::Run
}
