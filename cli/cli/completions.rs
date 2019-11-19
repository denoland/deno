// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use clap::App;
use clap::AppSettings;
use clap::Arg;
use clap::Shell;
use clap::SubCommand;

pub fn completions_subcommand<'a, 'b>() -> App<'a, 'b> {
  SubCommand::with_name("completions")
    .settings(&[AppSettings::DisableHelpSubcommand])
    .about("Generate shell completions")
    .long_about(
      "Output shell completion script to standard output.

Example:

  deno completions bash > /usr/local/etc/bash_completion.d/deno.bash
  source /usr/local/etc/bash_completion.d/deno.bash",
    )
    .arg(
      Arg::with_name("shell")
        .possible_values(&Shell::variants())
        .required(true),
    )
}
