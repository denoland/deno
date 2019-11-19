// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use clap::App;
use clap::SubCommand;

pub fn types_subcommand<'a, 'b>() -> App<'a, 'b> {
  SubCommand::with_name("types")
    .about("Print runtime TypeScript declarations")
    .long_about(
      "Print runtime TypeScript declarations.

  deno types > lib.deno_runtime.d.ts

The declaration file could be saved and used for typing information.",
    )
}
