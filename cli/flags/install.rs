// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use super::args;
use super::DenoSubcommand;
use super::INSTALLER_URL;
use crate::flags::DenoFlags;
use clap::App;
use clap::AppSettings;
use clap::Arg;
use clap::ArgMatches;
use clap::SubCommand;

pub fn install_subcommand<'a, 'b>() -> App<'a, 'b> {
  SubCommand::with_name("install")
    .settings(&[
      AppSettings::DisableHelpSubcommand,
      AppSettings::AllowExternalSubcommands,
      AppSettings::SubcommandRequired,
    ])
    .about("Install script as executable")
    .long_about(
      "Automatically downloads deno_installer dependencies on first run.

Default installation directory is $HOME/.deno/bin and it must be added to the path manually.

  deno install file_server https://deno.land/std/http/file_server.ts --allow-net --allow-read

  deno install colors https://deno.land/std/examples/colors.ts

To change installation directory use -d/--dir flag

  deno install -d /usr/local/bin file_server https://deno.land/std/http/file_server.ts --allow-net --allow-read",
    ).arg(args::log_level())
    .arg(args::reload()).arg(
    Arg::with_name("dir")
      .long("dir")
      .short("d")
      .help("Installation directory (defaults to $HOME/.deno/bin)")
      .takes_value(true)
  ).arg(
    Arg::with_name("exe_name")
      .help("Executable name")
      .required(true),
  ).subcommand(
    // this is a fake subcommand - it's used in conjunction with
    // AppSettings:AllowExternalSubcommand to treat it as an
    // entry point script
    SubCommand::with_name("[SCRIPT]").about("Script URL"),
  )
}

pub fn parse(
  flags: &mut DenoFlags,
  argv: &mut Vec<String>,
  matches: &ArgMatches,
) -> DenoSubcommand {
  args::parse_log_level(flags, matches);
  args::parse_reload(flags, matches);
  flags.allow_read = true;
  flags.allow_write = true;
  flags.allow_net = true;
  flags.allow_env = true;
  flags.allow_run = true;
  argv.push(INSTALLER_URL.to_string());

  if matches.is_present("dir") {
    let install_dir = matches.value_of("dir").unwrap();
    argv.push("--dir".to_string());
    argv.push(install_dir.to_string());
  }

  let exe_name: &str = matches.value_of("exe_name").unwrap();
  argv.push(exe_name.to_string());

  match matches.subcommand() {
    (script_url, Some(script_match)) => {
      argv.push(script_url.to_string());
      if script_match.is_present("") {
        let flags: Vec<String> = script_match
          .values_of("")
          .unwrap()
          .map(String::from)
          .collect();
        argv.extend(flags);
      }
      DenoSubcommand::Run
    }
    _ => unreachable!(),
  }
}
