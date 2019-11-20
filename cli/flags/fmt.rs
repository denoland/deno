// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use super::args;
use super::DenoSubcommand;
use super::PRETTIER_URL;
use crate::flags::DenoFlags;
use clap::App;
use clap::Arg;
use clap::ArgMatches;
use clap::SubCommand;

pub fn fmt_subcommand<'a, 'b>() -> App<'a, 'b> {
  SubCommand::with_name("fmt")
    .about("Format files")
    .long_about(
      "Auto-format JavaScript/TypeScript source code using Prettier

Automatically downloads Prettier dependencies on first run.

  deno fmt myfile1.ts myfile2.ts",
    ).arg(args::log_level())
    .arg(args::reload())
    .arg(
      Arg::with_name("check")
        .long("check")
        .help("Check if the source files are formatted.")
        .takes_value(false),
    )
    .arg(
      Arg::with_name("prettierrc")
        .long("prettierrc")
        .value_name("auto|disable|FILE")
        .help("Specify the configuration file of the prettier.
  auto: Auto detect prettier configuration file in current working dir.
  disable: Disable load configuration file.
  FILE: Load specified prettier configuration file. support .json/.toml/.js/.ts file
 ")
        .takes_value(true)
        .require_equals(true)
        .default_value("auto")
    )
    .arg(
      Arg::with_name("ignore-path")
        .long("ignore-path")
        .value_name("auto|disable|FILE")
        .help("Path to a file containing patterns that describe files to ignore.
  auto: Auto detect .pretierignore file in current working dir.
  disable: Disable load .prettierignore file.
  FILE: Load specified prettier ignore file.
 ")
        .takes_value(true)
        .require_equals(true)
        .default_value("auto")
    )
    .arg(
      Arg::with_name("stdout")
        .long("stdout")
        .help("Output formated code to stdout")
        .takes_value(false),
    )
    .arg(
      Arg::with_name("print-width")
        .long("print-width")
        .value_name("int")
        .help("Specify the line length that the printer will wrap on.")
        .takes_value(true)
        .require_equals(true)
    )
    .arg(
      Arg::with_name("tab-width")
        .long("tab-width")
        .value_name("int")
        .help("Specify the number of spaces per indentation-level.")
        .takes_value(true)
        .require_equals(true)
    )
    .arg(
      Arg::with_name("use-tabs")
        .long("use-tabs")
        .help("Indent lines with tabs instead of spaces.")
        .takes_value(false)
    )
    .arg(
      Arg::with_name("no-semi")
        .long("no-semi")
        .help("Print semicolons at the ends of statements.")
        .takes_value(false)
    )
    .arg(
      Arg::with_name("single-quote")
        .long("single-quote")
        .help("Use single quotes instead of double quotes.")
        .takes_value(false)
    )
    .arg(
      Arg::with_name("quote-props")
        .long("quote-props")
        .value_name("as-needed|consistent|preserve")
        .help("Change when properties in objects are quoted.")
        .takes_value(true)
        .possible_values(&["as-needed", "consistent", "preserve"])
        .require_equals(true)
    )
    .arg(
      Arg::with_name("jsx-single-quote")
        .long("jsx-single-quote")
        .help("Use single quotes instead of double quotes in JSX.")
        .takes_value(false)
    )
    .arg(
      Arg::with_name("jsx-bracket-same-line")
        .long("jsx-bracket-same-line")
        .help(
          "Put the > of a multi-line JSX element at the end of the last line
instead of being alone on the next line (does not apply to self closing elements)."
        )
        .takes_value(false)
    )
    .arg(
      Arg::with_name("trailing-comma")
        .long("trailing-comma")
        .help("Print trailing commas wherever possible when multi-line.")
        .takes_value(false)
    )
    .arg(
      Arg::with_name("no-bracket-spacing")
        .long("no-bracket-spacing")
        .help("Print spaces between brackets in object literals.")
        .takes_value(false)
    )
    .arg(
      Arg::with_name("arrow-parens")
        .long("arrow-parens")
        .value_name("avoid|always")
        .help("Include parentheses around a sole arrow function parameter.")
        .takes_value(true)
        .possible_values(&["avoid", "always"])
        .require_equals(true)
    )
    .arg(
      Arg::with_name("prose-wrap")
        .long("prose-wrap")
        .value_name("always|never|preserve")
        .help("How to wrap prose.")
        .takes_value(true)
        .possible_values(&["always", "never", "preserve"])
        .require_equals(true)
    )
    .arg(
      Arg::with_name("end-of-line")
        .long("end-of-line")
        .value_name("auto|lf|crlf|cr")
        .help("Which end of line characters to apply.")
        .takes_value(true)
        .possible_values(&["auto", "lf", "crlf", "cr"])
        .require_equals(true)
    )
    .arg(
      Arg::with_name("files")
        .takes_value(true)
        .multiple(true)
        .required(true),
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
  argv.push(PRETTIER_URL.to_string());

  let files: Vec<String> = matches
    .values_of("files")
    .unwrap()
    .map(String::from)
    .collect();
  argv.extend(files);

  if !matches.is_present("stdout") {
    // `deno fmt` writes to the files by default
    argv.push("--write".to_string());
  }

  let prettier_flags = [
    ["0", "check"],
    ["1", "prettierrc"],
    ["1", "ignore-path"],
    ["1", "print-width"],
    ["1", "tab-width"],
    ["0", "use-tabs"],
    ["0", "no-semi"],
    ["0", "single-quote"],
    ["1", "quote-props"],
    ["0", "jsx-single-quote"],
    ["0", "jsx-bracket-same-line"],
    ["0", "trailing-comma"],
    ["0", "no-bracket-spacing"],
    ["1", "arrow-parens"],
    ["1", "prose-wrap"],
    ["1", "end-of-line"],
  ];

  for opt in &prettier_flags {
    let t = opt[0];
    let keyword = opt[1];

    if matches.is_present(&keyword) {
      if t == "0" {
        argv.push(format!("--{}", keyword));
      } else {
        if keyword == "prettierrc" {
          argv.push("--config".to_string());
        } else {
          argv.push(format!("--{}", keyword));
        }
        argv.push(matches.value_of(keyword).unwrap().to_string());
      }
    }
  }

  DenoSubcommand::Run
}
