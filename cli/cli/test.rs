// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use super::args;
use super::DenoSubcommand;
use super::TEST_RUNNER_URL;
use crate::flags::DenoFlags;
use clap::App;
use clap::Arg;
use clap::ArgMatches;
use clap::SubCommand;

pub fn test_subcommand<'a, 'b>() -> App<'a, 'b> {
  let subcmd = SubCommand::with_name("test")
    .about("Run tests")
    .long_about(
      "Run tests using test runner

Automatically downloads test runner on first run.

  deno test **/*_test.ts **/test.ts",
    )
    .arg(
      Arg::with_name("failfast")
        .short("f")
        .long("failfast")
        .help("Stop on first error")
        .takes_value(false),
    )
    .arg(
      Arg::with_name("quiet")
        .short("q")
        .long("quiet")
        .help("Don't show output from test cases")
        .takes_value(false),
    )
    .arg(
      Arg::with_name("exclude")
        .short("e")
        .long("exclude")
        .help("List of file names to exclude from run")
        .takes_value(true)
        .multiple(true),
    )
    .arg(
      Arg::with_name("files")
        .help("List of file names to run")
        .takes_value(true)
        .multiple(true),
    );

  subcmd
    .arg(args::log_level())
    .arg(args::reload())
    .arg(args::no_fetch())
    .args(&args::permissions())
    .args(&args::runtime())
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
  args::parse_no_fetch(flags, matches);
  args::parse_permissions(flags, matches);
  args::parse_runtime(flags, matches);
  args::parse_configuration(flags, matches);
  args::parse_lock_args(flags, matches);
  flags.allow_read = true;
  argv.push(TEST_RUNNER_URL.to_string());

  if matches.is_present("quiet") {
    argv.push("--quiet".to_string());
  }

  if matches.is_present("failfast") {
    argv.push("--failfast".to_string());
  }

  if matches.is_present("exclude") {
    argv.push("--exclude".to_string());
    let exclude: Vec<String> = matches
      .values_of("exclude")
      .unwrap()
      .map(String::from)
      .collect();
    argv.extend(exclude);
  }

  if matches.is_present("files") {
    argv.push("--".to_string());
    let files: Vec<String> = matches
      .values_of("files")
      .unwrap()
      .map(String::from)
      .collect();
    argv.extend(files);
  }

  DenoSubcommand::Run
}
