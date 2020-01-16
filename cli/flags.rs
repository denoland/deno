// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use crate::fs::resolve_from_cwd;
use clap::App;
use clap::AppSettings;
use clap::Arg;
use clap::ArgMatches;
use clap::SubCommand;
use log::Level;
use std::collections::HashSet;

/// Creates vector of strings, Vec<String>
macro_rules! svec {
    ($($x:expr),*) => (vec![$($x.to_string()),*]);
}
/// Creates HashSet<String> from string literals
macro_rules! sset {
  ($($x:expr),*) => {{
    let _v = svec![$($x.to_string()),*];
    let hash_set: HashSet<String> = _v.iter().cloned().collect();
    hash_set
  }}
}

macro_rules! std_url {
  ($x:expr) => {
    concat!("https://deno.land/std@v0.29.0/", $x)
  };
}

/// Used for `deno fmt <files>...` subcommand
const PRETTIER_URL: &str = std_url!("prettier/main.ts");
/// Used for `deno install...` subcommand
const INSTALLER_URL: &str = std_url!("installer/mod.ts");
/// Used for `deno test...` subcommand
const TEST_RUNNER_URL: &str = std_url!("testing/runner.ts");

#[derive(Clone, Debug, PartialEq)]
pub enum DenoSubcommand {
  Bundle,
  Completions,
  Eval,
  Fetch,
  Help,
  Info,
  Install,
  Repl,
  Run,
  Types,
}

impl Default for DenoSubcommand {
  fn default() -> DenoSubcommand {
    DenoSubcommand::Run
  }
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct DenoFlags {
  /// Vector of CLI arguments - these are user script arguments, all Deno
  /// specific flags are removed.
  pub argv: Vec<String>,
  pub subcommand: DenoSubcommand,

  pub log_level: Option<Level>,
  pub version: bool,
  pub reload: bool,
  pub config_path: Option<String>,
  pub import_map_path: Option<String>,
  pub allow_read: bool,
  pub read_whitelist: Vec<String>,
  pub cache_blacklist: Vec<String>,
  pub allow_write: bool,
  pub write_whitelist: Vec<String>,
  pub allow_net: bool,
  pub net_whitelist: Vec<String>,
  pub allow_env: bool,
  pub allow_run: bool,
  pub allow_plugin: bool,
  pub allow_hrtime: bool,
  pub no_prompts: bool,
  pub no_remote: bool,
  pub cached_only: bool,
  pub seed: Option<u64>,
  pub v8_flags: Option<Vec<String>>,
  // Use tokio::runtime::current_thread
  pub current_thread: bool,

  pub bundle_output: Option<String>,

  pub lock: Option<String>,
  pub lock_write: bool,
}

static ENV_VARIABLES_HELP: &str = "ENVIRONMENT VARIABLES:
    DENO_DIR       Set deno's base directory
    NO_COLOR       Set to disable color
    HTTP_PROXY     Proxy address for HTTP requests (module downloads, fetch)
    HTTPS_PROXY    Same but for HTTPS";

static DENO_HELP: &str = "A secure JavaScript and TypeScript runtime

Docs: https://deno.land/std/manual.md
Modules: https://deno.land/x/
Bugs: https://github.com/denoland/deno/issues

To run the REPL supply no arguments:

  deno

To evaluate code from the command line:

  deno eval \"console.log(30933 + 404)\"

To execute a script:

  deno https://deno.land/std/examples/welcome.ts

The default subcommand is 'run'. The above is equivalent to

  deno run https://deno.land/std/examples/welcome.ts

See 'deno help run' for run specific flags.";

lazy_static! {
  static ref LONG_VERSION: String = format!(
    "{}\nv8 {}\ntypescript {}",
    crate::version::DENO,
    crate::version::v8(),
    crate::version::TYPESCRIPT
  );
}

/// Main entry point for parsing deno's command line flags.
/// Exits the process on error.
pub fn flags_from_vec(args: Vec<String>) -> DenoFlags {
  match flags_from_vec_safe(args) {
    Ok(flags) => flags,
    Err(err) => err.exit(),
  }
}

/// Same as flags_from_vec but does not exit on error.
pub fn flags_from_vec_safe(args: Vec<String>) -> clap::Result<DenoFlags> {
  let args0 = args[0].clone();
  let args = arg_hacks(args);
  let app = clap_root();
  let matches = app.get_matches_from_safe(args)?;

  let mut flags = DenoFlags::default();

  flags.argv.push(args0);

  if matches.is_present("log-level") {
    flags.log_level = match matches.value_of("log-level").unwrap() {
      "debug" => Some(Level::Debug),
      "info" => Some(Level::Info),
      _ => unreachable!(),
    };
  }

  if let Some(m) = matches.subcommand_matches("run") {
    run_parse(&mut flags, m);
  } else if let Some(m) = matches.subcommand_matches("fmt") {
    fmt_parse(&mut flags, m);
  } else if let Some(m) = matches.subcommand_matches("types") {
    types_parse(&mut flags, m);
  } else if let Some(m) = matches.subcommand_matches("fetch") {
    fetch_parse(&mut flags, m);
  } else if let Some(m) = matches.subcommand_matches("info") {
    info_parse(&mut flags, m);
  } else if let Some(m) = matches.subcommand_matches("eval") {
    eval_parse(&mut flags, m);
  } else if let Some(m) = matches.subcommand_matches("repl") {
    repl_parse(&mut flags, m);
  } else if let Some(m) = matches.subcommand_matches("bundle") {
    bundle_parse(&mut flags, m);
  } else if let Some(m) = matches.subcommand_matches("install") {
    install_parse(&mut flags, m);
  } else if let Some(m) = matches.subcommand_matches("completions") {
    completions_parse(&mut flags, m);
  } else if let Some(m) = matches.subcommand_matches("test") {
    test_parse(&mut flags, m);
  } else {
    unimplemented!();
  }

  Ok(flags)
}

fn clap_root<'a, 'b>() -> App<'a, 'b> {
  clap::App::new("deno")
    .bin_name("deno")
    .global_settings(&[
      AppSettings::UnifiedHelpMessage,
      AppSettings::ColorNever,
      AppSettings::VersionlessSubcommands,
    ])
    // Disable clap's auto-detection of terminal width
    .set_term_width(0)
    // Disable each subcommand having its own version.
    .version(crate::version::DENO)
    .long_version(LONG_VERSION.as_str())
    .arg(
      Arg::with_name("log-level")
        .short("L")
        .long("log-level")
        .help("Set log level")
        .takes_value(true)
        .possible_values(&["debug", "info"])
        .global(true),
    )
    .subcommand(bundle_subcommand())
    .subcommand(completions_subcommand())
    .subcommand(eval_subcommand())
    .subcommand(fetch_subcommand())
    .subcommand(fmt_subcommand())
    .subcommand(info_subcommand())
    .subcommand(install_subcommand())
    .subcommand(repl_subcommand())
    .subcommand(run_subcommand())
    .subcommand(test_subcommand())
    .subcommand(types_subcommand())
    .long_about(DENO_HELP)
    .after_help(ENV_VARIABLES_HELP)
}

fn types_parse(flags: &mut DenoFlags, _matches: &clap::ArgMatches) {
  flags.subcommand = DenoSubcommand::Types;
}

fn fmt_parse(flags: &mut DenoFlags, matches: &clap::ArgMatches) {
  flags.subcommand = DenoSubcommand::Run;
  flags.allow_read = true;
  flags.allow_write = true;
  flags.argv.push(PRETTIER_URL.to_string());

  let files: Vec<String> = matches
    .values_of("files")
    .unwrap()
    .map(String::from)
    .collect();
  flags.argv.extend(files);

  if !matches.is_present("stdout") {
    // `deno fmt` writes to the files by default
    flags.argv.push("--write".to_string());
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
        flags.argv.push(format!("--{}", keyword));
      } else {
        if keyword == "prettierrc" {
          flags.argv.push("--config".to_string());
        } else {
          flags.argv.push(format!("--{}", keyword));
        }
        flags
          .argv
          .push(matches.value_of(keyword).unwrap().to_string());
      }
    }
  }
}

fn install_parse(flags: &mut DenoFlags, matches: &clap::ArgMatches) {
  flags.subcommand = DenoSubcommand::Run;
  flags.allow_read = true;
  flags.allow_write = true;
  flags.allow_net = true;
  flags.allow_env = true;
  flags.allow_run = true;
  flags.argv.push(INSTALLER_URL.to_string());

  if matches.is_present("dir") {
    let install_dir = matches.value_of("dir").unwrap();
    flags.argv.push("--dir".to_string());
    println!("dir {}", install_dir);
    flags.argv.push(install_dir.to_string());
  } else {
    println!("no dir");
  }

  let exe_name = matches.value_of("exe_name").unwrap();
  flags.argv.push(String::from(exe_name));

  let cmd = matches.values_of("cmd").unwrap();
  for arg in cmd {
    flags.argv.push(String::from(arg));
  }
}

fn bundle_parse(flags: &mut DenoFlags, matches: &clap::ArgMatches) {
  flags.subcommand = DenoSubcommand::Bundle;
  let source_file: &str = matches.value_of("source_file").unwrap();
  flags.argv.push(source_file.into());
  if let Some(out_file) = matches.value_of("out_file") {
    flags.allow_write = true;
    flags.bundle_output = Some(out_file.to_string());
  }
}

fn completions_parse(flags: &mut DenoFlags, matches: &clap::ArgMatches) {
  flags.subcommand = DenoSubcommand::Completions;
  let shell: &str = matches.value_of("shell").unwrap();
  let mut buf: Vec<u8> = vec![];
  use std::str::FromStr;
  clap_root().gen_completions_to(
    "deno",
    clap::Shell::from_str(shell).unwrap(),
    &mut buf,
  );
  // TODO(ry) This flags module should only be for parsing flags, not actually
  // acting upon the flags. Although this print is innocent, it breaks the
  // model. The print should be moved out.
  print!("{}", std::str::from_utf8(&buf).unwrap());
}

fn repl_parse(flags: &mut DenoFlags, matches: &clap::ArgMatches) {
  v8_flags_arg_parse(flags, matches);
  flags.subcommand = DenoSubcommand::Repl;
  flags.allow_net = true;
  flags.allow_env = true;
  flags.allow_run = true;
  flags.allow_read = true;
  flags.allow_write = true;
  flags.allow_plugin = true;
  flags.allow_hrtime = true;
}

fn eval_parse(flags: &mut DenoFlags, matches: &clap::ArgMatches) {
  flags.subcommand = DenoSubcommand::Eval;
  flags.allow_net = true;
  flags.allow_env = true;
  flags.allow_run = true;
  flags.allow_read = true;
  flags.allow_write = true;
  flags.allow_plugin = true;
  flags.allow_hrtime = true;
  let code: &str = matches.value_of("code").unwrap();
  flags.argv.extend(vec![code.to_string()]);
}

fn info_parse(flags: &mut DenoFlags, matches: &clap::ArgMatches) {
  flags.subcommand = DenoSubcommand::Info;
  if let Some(file) = matches.value_of("file") {
    flags.argv.push(file.into());
  }
}

fn fetch_parse(flags: &mut DenoFlags, matches: &clap::ArgMatches) {
  flags.subcommand = DenoSubcommand::Fetch;
  reload_arg_parse(flags, matches);
  lock_args_parse(flags, matches);
  importmap_arg_parse(flags, matches);
  config_arg_parse(flags, matches);
  no_remote_arg_parse(flags, matches);
  if let Some(file) = matches.value_of("file") {
    flags.argv.push(file.into());
  }
}

fn lock_args_parse(flags: &mut DenoFlags, matches: &clap::ArgMatches) {
  if matches.is_present("lock") {
    let lockfile = matches.value_of("lock").unwrap();
    flags.lock = Some(lockfile.to_string());
  }
  if matches.is_present("lock-write") {
    flags.lock_write = true;
  }
}

fn resolve_fs_whitelist(whitelist: &[String]) -> Vec<String> {
  whitelist
    .iter()
    .map(|raw_path| {
      resolve_from_cwd(&raw_path)
        .unwrap()
        .0
        .to_str()
        .unwrap()
        .to_owned()
    })
    .collect::<Vec<_>>()
}

// Shared between the run and test subcommands. They both take similar options.
fn run_test_args_parse(flags: &mut DenoFlags, matches: &clap::ArgMatches) {
  reload_arg_parse(flags, matches);
  lock_args_parse(flags, matches);
  importmap_arg_parse(flags, matches);
  config_arg_parse(flags, matches);
  v8_flags_arg_parse(flags, matches);
  no_remote_arg_parse(flags, matches);

  if matches.is_present("allow-read") {
    if matches.value_of("allow-read").is_some() {
      let read_wl = matches.values_of("allow-read").unwrap();
      let raw_read_whitelist: Vec<String> =
        read_wl.map(std::string::ToString::to_string).collect();
      flags.read_whitelist = resolve_fs_whitelist(&raw_read_whitelist);
      debug!("read whitelist: {:#?}", &flags.read_whitelist);
    } else {
      flags.allow_read = true;
    }
  }
  if matches.is_present("allow-write") {
    if matches.value_of("allow-write").is_some() {
      let write_wl = matches.values_of("allow-write").unwrap();
      let raw_write_whitelist: Vec<String> =
        write_wl.map(std::string::ToString::to_string).collect();
      flags.write_whitelist =
        resolve_fs_whitelist(raw_write_whitelist.as_slice());
      debug!("write whitelist: {:#?}", &flags.write_whitelist);
    } else {
      flags.allow_write = true;
    }
  }
  if matches.is_present("allow-net") {
    if matches.value_of("allow-net").is_some() {
      let net_wl = matches.values_of("allow-net").unwrap();
      let raw_net_whitelist =
        net_wl.map(std::string::ToString::to_string).collect();
      flags.net_whitelist = resolve_hosts(raw_net_whitelist);
      debug!("net whitelist: {:#?}", &flags.net_whitelist);
    } else {
      flags.allow_net = true;
    }
  }
  if matches.is_present("allow-env") {
    flags.allow_env = true;
  }
  if matches.is_present("allow-run") {
    flags.allow_run = true;
  }
  if matches.is_present("allow-plugin") {
    flags.allow_plugin = true;
  }
  if matches.is_present("allow-hrtime") {
    flags.allow_hrtime = true;
  }
  if matches.is_present("allow-all") {
    flags.allow_read = true;
    flags.allow_env = true;
    flags.allow_net = true;
    flags.allow_run = true;
    flags.allow_read = true;
    flags.allow_write = true;
    flags.allow_plugin = true;
    flags.allow_hrtime = true;
  }
  if matches.is_present("cached-only") {
    flags.cached_only = true;
  }

  if matches.is_present("current-thread") {
    flags.current_thread = true;
  }

  if matches.is_present("seed") {
    let seed_string = matches.value_of("seed").unwrap();
    let seed = seed_string.parse::<u64>().unwrap();
    flags.seed = Some(seed);

    let v8_seed_flag = format!("--random-seed={}", seed);

    match flags.v8_flags {
      Some(ref mut v8_flags) => {
        v8_flags.push(v8_seed_flag);
      }
      None => {
        flags.v8_flags = Some(svec![v8_seed_flag]);
      }
    }
  }
}

fn run_parse(flags: &mut DenoFlags, matches: &clap::ArgMatches) {
  flags.subcommand = DenoSubcommand::Run;
  script_arg_parse(flags, matches);
  run_test_args_parse(flags, matches);
}

fn test_parse(flags: &mut DenoFlags, matches: &clap::ArgMatches) {
  flags.subcommand = DenoSubcommand::Run;
  flags.allow_read = true;

  flags.argv.push(TEST_RUNNER_URL.to_string());

  run_test_args_parse(flags, matches);

  if matches.is_present("quiet") {
    flags.argv.push("--quiet".to_string());
  }

  if matches.is_present("failfast") {
    flags.argv.push("--failfast".to_string());
  }

  if matches.is_present("exclude") {
    flags.argv.push("--exclude".to_string());
    let exclude: Vec<String> = matches
      .values_of("exclude")
      .unwrap()
      .map(String::from)
      .collect();
    flags.argv.extend(exclude);
  }

  if matches.is_present("files") {
    flags.argv.push("--".to_string());
    let files: Vec<String> = matches
      .values_of("files")
      .unwrap()
      .map(String::from)
      .collect();
    flags.argv.extend(files);
  }
}

fn types_subcommand<'a, 'b>() -> App<'a, 'b> {
  SubCommand::with_name("types")
    .about("Print runtime TypeScript declarations")
    .long_about(
      "Print runtime TypeScript declarations.

  deno types > lib.deno_runtime.d.ts

The declaration file could be saved and used for typing information.",
    )
}

fn fmt_subcommand<'a, 'b>() -> App<'a, 'b> {
  SubCommand::with_name("fmt")
        .about("Format files")
        .long_about(
"Auto-format JavaScript/TypeScript source code using Prettier

Automatically downloads Prettier dependencies on first run.

  deno fmt myfile1.ts myfile2.ts",
        )
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

fn repl_subcommand<'a, 'b>() -> App<'a, 'b> {
  SubCommand::with_name("repl")
    .about("Read Eval Print Loop")
    .arg(v8_flags_arg())
}

fn install_subcommand<'a, 'b>() -> App<'a, 'b> {
  SubCommand::with_name("install")
        .setting(AppSettings::TrailingVarArg)
        .arg(
          Arg::with_name("dir")
            .long("dir")
            .short("d")
            .help("Installation directory (defaults to $HOME/.deno/bin)")
            .takes_value(true)
            .multiple(false))
        .arg(
          Arg::with_name("exe_name")
            .required(true)
        )
        .arg(
          Arg::with_name("cmd")
            .required(true)
            .multiple(true)
            .allow_hyphen_values(true)
        )
        .about("Install script as executable")
        .long_about(
"Installs a script as executable. The default installation directory is
$HOME/.deno/bin and it must be added to the path manually.

  deno install file_server https://deno.land/std/http/file_server.ts --allow-net --allow-read

  deno install colors https://deno.land/std/examples/colors.ts

To change installation directory use -d/--dir flag

  deno install -d /usr/local/bin file_server https://deno.land/std/http/file_server.ts --allow-net --allow-read")
}

fn bundle_subcommand<'a, 'b>() -> App<'a, 'b> {
  SubCommand::with_name("bundle")
    .arg(
      Arg::with_name("source_file")
        .takes_value(true)
        .required(true),
    )
    .arg(Arg::with_name("out_file").takes_value(true).required(false))
    .about("Bundle module and dependencies into single file")
    .long_about(
      "Output a single JavaScript file with all dependencies.

If a out_file argument is omitted, the output of the bundle will be sent to
standard out. Examples:

  deno bundle https://deno.land/std/examples/colors.ts

  deno bundle https://deno.land/std/examples/colors.ts colors.bundle.js",
    )
}

fn completions_subcommand<'a, 'b>() -> App<'a, 'b> {
  SubCommand::with_name("completions")
    .setting(AppSettings::DisableHelpSubcommand)
    .arg(
      Arg::with_name("shell")
        .possible_values(&clap::Shell::variants())
        .required(true),
    )
    .about("Generate shell completions")
    .long_about(
      "Output shell completion script to standard output.

Example:

  deno completions bash > /usr/local/etc/bash_completion.d/deno.bash
  source /usr/local/etc/bash_completion.d/deno.bash",
    )
}

fn eval_subcommand<'a, 'b>() -> App<'a, 'b> {
  SubCommand::with_name("eval")
    .about("Eval script")
    .long_about(
      "Evaluate JavaScript from command-line

This command has implicit access to all permissions (--allow-all)

  deno eval \"console.log('hello world')\"",
    )
    .arg(Arg::with_name("code").takes_value(true).required(true))
}

fn info_subcommand<'a, 'b>() -> App<'a, 'b> {
  SubCommand::with_name("info")
    .about("Show info about cache or info related to source file")
    .long_about(
      "Information about source file and cache

Example: deno info https://deno.land/std/http/file_server.ts

The following information is shown:

local: Local path of the file.
type: JavaScript, TypeScript, or JSON.
compiled: Local path of compiled source code (TypeScript only)
map: Local path of source map (TypeScript only)
deps: Dependency tree of the source file.

Without any additional arguments 'deno info' shows:

DENO_DIR: directory containing Deno-related files
Remote modules cache: directory containing remote modules
TypeScript compiler cache: directory containing TS compiler output",
    )
    .arg(Arg::with_name("file").takes_value(true).required(false))
}

fn fetch_subcommand<'a, 'b>() -> App<'a, 'b> {
  SubCommand::with_name("fetch")
    .arg(reload_arg())
    .arg(lock_arg())
    .arg(lock_write_arg())
    .arg(importmap_arg())
    .arg(config_arg())
    .arg(no_remote_arg())
    .arg(Arg::with_name("file").takes_value(true).required(true))
    .about("Fetch the dependencies")
    .long_about(
      "Fetch and compile remote dependencies recursively.

Downloads all statically imported scripts and save them in local
cache, without running the code. No future import network requests
would be made unless --reload is specified.

Downloads all dependencies

  deno fetch https://deno.land/std/http/file_server.ts

Once cached, static imports no longer send network requests

  deno run -A https://deno.land/std/http/file_server.ts",
    )
}

fn run_test_args<'a, 'b>(app: App<'a, 'b>) -> App<'a, 'b> {
  app
    .arg(importmap_arg())
    .arg(reload_arg())
    .arg(config_arg())
    .arg(lock_arg())
    .arg(lock_write_arg())
    .arg(no_remote_arg())
    .arg(v8_flags_arg())
    .arg(
      Arg::with_name("allow-read")
        .long("allow-read")
        .min_values(0)
        .takes_value(true)
        .use_delimiter(true)
        .require_equals(true)
        .help("Allow file system read access"),
    )
    .arg(
      Arg::with_name("allow-write")
        .long("allow-write")
        .min_values(0)
        .takes_value(true)
        .use_delimiter(true)
        .require_equals(true)
        .help("Allow file system write access"),
    )
    .arg(
      Arg::with_name("allow-net")
        .long("allow-net")
        .min_values(0)
        .takes_value(true)
        .use_delimiter(true)
        .require_equals(true)
        .help("Allow network access"),
    )
    .arg(
      Arg::with_name("allow-env")
        .long("allow-env")
        .help("Allow environment access"),
    )
    .arg(
      Arg::with_name("allow-run")
        .long("allow-run")
        .help("Allow running subprocesses"),
    )
    .arg(
      Arg::with_name("allow-plugin")
        .long("allow-plugin")
        .help("Allow loading plugins"),
    )
    .arg(
      Arg::with_name("allow-hrtime")
        .long("allow-hrtime")
        .help("Allow high resolution time measurement"),
    )
    .arg(
      Arg::with_name("allow-all")
        .short("A")
        .long("allow-all")
        .help("Allow all permissions"),
    )
    .arg(
      Arg::with_name("cached-only")
        .long("cached-only")
        .help("Require that remote dependencies are already cached"),
    )
    .arg(
      Arg::with_name("current-thread")
        .long("current-thread")
        .help("Use tokio::runtime::current_thread"),
    )
    .arg(
      Arg::with_name("seed")
        .long("seed")
        .value_name("NUMBER")
        .help("Seed Math.random()")
        .takes_value(true)
        .validator(|val: String| match val.parse::<u64>() {
          Ok(_) => Ok(()),
          Err(_) => Err("Seed should be a number".to_string()),
        }),
    )
}

fn run_subcommand<'a, 'b>() -> App<'a, 'b> {
  run_test_args(SubCommand::with_name("run"))
    .setting(AppSettings::TrailingVarArg)
    .arg(script_arg())
    .about("Run a program given a filename or url to the source code")
    .long_about(
      "Run a program given a filename or url to the source code.

By default all programs are run in sandbox without access to disk, network or
ability to spawn subprocesses.

  deno run https://deno.land/std/examples/welcome.ts

With all permissions

  deno run -A https://deno.land/std/http/file_server.ts

With only permission to read from disk and listen to network

  deno run --allow-net --allow-read https://deno.land/std/http/file_server.ts

With only permission to read whitelist files from disk

  deno run --allow-read=/etc https://deno.land/std/http/file_server.ts",
    )
}

fn test_subcommand<'a, 'b>() -> App<'a, 'b> {
  run_test_args(SubCommand::with_name("test"))
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
        .takes_value(true),
    )
    .arg(
      Arg::with_name("files")
        .help("List of file names to run")
        .takes_value(true)
        .multiple(true),
    )
    .about("Run tests")
    .long_about(
      "Run tests using test runner

Searches the specified directories for all files that end in _test.ts or
_test.js and executes them.

  deno test src/",
    )
}

fn script_arg<'a, 'b>() -> Arg<'a, 'b> {
  Arg::with_name("script_arg")
    .multiple(true)
    .help("script args")
    .value_name("SCRIPT_ARG")
}

fn script_arg_parse(flags: &mut DenoFlags, matches: &ArgMatches) {
  if let Some(script_values) = matches.values_of("script_arg") {
    for v in script_values {
      flags.argv.push(String::from(v));
    }
  }
}

fn lock_arg<'a, 'b>() -> Arg<'a, 'b> {
  Arg::with_name("lock")
    .long("lock")
    .value_name("FILE")
    .help("Check the specified lock file")
    .takes_value(true)
}

fn lock_write_arg<'a, 'b>() -> Arg<'a, 'b> {
  Arg::with_name("lock-write")
    .long("lock-write")
    .help("Write lock file. Use with --lock.")
}

fn config_arg<'a, 'b>() -> Arg<'a, 'b> {
  Arg::with_name("config")
    .short("c")
    .long("config")
    .value_name("FILE")
    .help("Load tsconfig.json configuration file")
    .takes_value(true)
}

fn config_arg_parse(flags: &mut DenoFlags, matches: &ArgMatches) {
  flags.config_path = matches.value_of("config").map(ToOwned::to_owned);
}

fn reload_arg<'a, 'b>() -> Arg<'a, 'b> {
  Arg::with_name("reload")
    .short("r")
    .min_values(0)
    .takes_value(true)
    .use_delimiter(true)
    .require_equals(true)
    .long("reload")
    .help("Reload source code cache (recompile TypeScript)")
    .value_name("CACHE_BLACKLIST")
    .long_help(
      "Reload source code cache (recompile TypeScript)
--reload
  Reload everything
--reload=https://deno.land/std
  Reload only standard modules
--reload=https://deno.land/std/fs/utils.ts,https://deno.land/std/fmt/colors.ts
  Reloads specific modules",
    )
}

fn reload_arg_parse(flags: &mut DenoFlags, matches: &ArgMatches) {
  if matches.is_present("reload") {
    if matches.value_of("reload").is_some() {
      let cache_bl = matches.values_of("reload").unwrap();
      let raw_cache_blacklist: Vec<String> =
        cache_bl.map(std::string::ToString::to_string).collect();
      flags.cache_blacklist = resolve_urls(raw_cache_blacklist);
      debug!("cache blacklist: {:#?}", &flags.cache_blacklist);
      flags.reload = false;
    } else {
      flags.reload = true;
    }
  }
}

fn importmap_arg<'a, 'b>() -> Arg<'a, 'b> {
  Arg::with_name("importmap")
    .long("importmap")
    .value_name("FILE")
    .help("Load import map file")
    .long_help(
      "Load import map file
Docs: https://deno.land/std/manual.md#import-maps
Specification: https://wicg.github.io/import-maps/
Examples: https://github.com/WICG/import-maps#the-import-map",
    )
    .takes_value(true)
}

fn importmap_arg_parse(flags: &mut DenoFlags, matches: &clap::ArgMatches) {
  flags.import_map_path = matches.value_of("importmap").map(ToOwned::to_owned);
}

fn v8_flags_arg<'a, 'b>() -> Arg<'a, 'b> {
  Arg::with_name("v8-flags")
    .long("v8-flags")
    .takes_value(true)
    .use_delimiter(true)
    .require_equals(true)
    .help("Set V8 command line options. For help: --v8-flags=--help")
}

fn v8_flags_arg_parse(flags: &mut DenoFlags, matches: &ArgMatches) {
  if let Some(v8_flags) = matches.values_of("v8-flags") {
    let s: Vec<String> = v8_flags.map(String::from).collect();
    flags.v8_flags = Some(s);
  }
}

fn no_remote_arg<'a, 'b>() -> Arg<'a, 'b> {
  Arg::with_name("no-remote")
    .long("no-remote")
    .help("Do not resolve remote modules")
}

fn no_remote_arg_parse(flags: &mut DenoFlags, matches: &clap::ArgMatches) {
  if matches.is_present("no-remote") {
    flags.no_remote = true;
  }
}

// TODO(ry) move this to utility module and add test.
/// Strips fragment part of URL. Panics on bad URL.
pub fn resolve_urls(urls: Vec<String>) -> Vec<String> {
  use url::Url;
  let mut out: Vec<String> = vec![];
  for urlstr in urls.iter() {
    use std::str::FromStr;
    let result = Url::from_str(urlstr);
    if result.is_err() {
      panic!("Bad Url: {}", urlstr);
    }
    let mut url = result.unwrap();
    url.set_fragment(None);
    let mut full_url = String::from(url.as_str());
    if full_url.len() > 1 && full_url.ends_with('/') {
      full_url.pop();
    }
    out.push(full_url);
  }
  out
}

/// Expands "bare port" paths (eg. ":8080") into full paths with hosts. It
/// expands to such paths into 3 paths with following hosts: `0.0.0.0:port`,
/// `127.0.0.1:port` and `localhost:port`.
fn resolve_hosts(paths: Vec<String>) -> Vec<String> {
  let mut out: Vec<String> = vec![];
  for host_and_port in paths.iter() {
    let parts = host_and_port.split(':').collect::<Vec<&str>>();

    match parts.len() {
      // host only
      1 => {
        out.push(host_and_port.to_owned());
      }
      // host and port (NOTE: host might be empty string)
      2 => {
        let host = parts[0];
        let port = parts[1];

        if !host.is_empty() {
          out.push(host_and_port.to_owned());
          continue;
        }

        // we got bare port, let's add default hosts
        for host in ["0.0.0.0", "127.0.0.1", "localhost"].iter() {
          out.push(format!("{}:{}", host, port));
        }
      }
      _ => panic!("Bad host:port pair: {}", host_and_port),
    }
  }

  out
}

fn arg_hacks(mut args: Vec<String>) -> Vec<String> {
  // Hack #1 We want to default the subcommand to "run"
  // Clap does not let us have a default sub-command. But we want to allow users
  // to do "deno script.js" instead of "deno run script.js".
  // This function insert the "run" into the second position of the args.
  assert!(!args.is_empty());
  // Rational:
  // deno -> deno repl
  if args.len() == 1 {
    args.insert(1, "repl".to_string());
    return args;
  }
  let subcommands = sset![
    "bundle",
    "completions",
    "eval",
    "fetch",
    "fmt",
    "test",
    "info",
    "repl",
    "run",
    "types",
    "install",
    "help",
    "version"
  ];
  let modifier_flags = sset!["-h", "--help", "-V", "--version"];
  // deno [subcommand|behavior modifier flags] -> do nothing
  if subcommands.contains(&args[1]) || modifier_flags.contains(&args[1]) {
    return args;
  }
  // This is not perfect either, since originally we should also
  // support e.g. `-L debug` which `debug` would be treated as main module.
  // Instead `-L=debug` must be used
  let mut has_main_module = false;
  for arg in args.iter().skip(1) {
    if !arg.starts_with('-') {
      has_main_module = true;
      break;
    }
  }
  if has_main_module {
    // deno ...-[flags] NAME ... -> deno run ...-[flags] NAME ...
    args.insert(1, "run".to_string());
  } else {
    // deno ...-[flags] -> deno repl ...-[flags]
    args.insert(1, "repl".to_string());
  }
  args
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::env::current_dir;

  #[test]
  fn arg_hacks_test() {
    let args0 = arg_hacks(svec!["deno", "--version"]);
    assert_eq!(args0, ["deno", "--version"]);
    let args1 = arg_hacks(svec!["deno"]);
    assert_eq!(args1, ["deno", "repl"]);
    let args2 = arg_hacks(svec!["deno", "-L=debug", "-h"]);
    assert_eq!(args2, ["deno", "repl", "-L=debug", "-h"]);
    let args3 = arg_hacks(svec!["deno", "script.js"]);
    assert_eq!(args3, ["deno", "run", "script.js"]);
    let args4 = arg_hacks(svec!["deno", "-A", "script.js", "-L=info"]);
    assert_eq!(args4, ["deno", "run", "-A", "script.js", "-L=info"]);
  }

  #[test]
  fn version() {
    let r = flags_from_vec_safe(svec!["deno", "--version"]);
    assert_eq!(r.unwrap_err().kind, clap::ErrorKind::VersionDisplayed);
    let r = flags_from_vec_safe(svec!["deno", "-V"]);
    assert_eq!(r.unwrap_err().kind, clap::ErrorKind::VersionDisplayed);
  }

  #[test]
  fn run_reload() {
    let r = flags_from_vec_safe(svec!["deno", "run", "-r", "script.ts"]);
    let flags = r.unwrap();
    assert_eq!(
      flags,
      DenoFlags {
        subcommand: DenoSubcommand::Run,
        argv: svec!["deno", "script.ts"],
        reload: true,
        ..DenoFlags::default()
      }
    );
  }

  #[test]
  fn run_reload_allow_write() {
    let r = flags_from_vec_safe(svec![
      "deno",
      "run",
      "-r",
      "--allow-write",
      "script.ts"
    ]);
    assert_eq!(
      r.unwrap(),
      DenoFlags {
        reload: true,
        subcommand: DenoSubcommand::Run,
        argv: svec!["deno", "script.ts"],
        allow_write: true,
        ..DenoFlags::default()
      }
    );
  }

  #[test]
  fn run_v8_flags() {
    let r = flags_from_vec_safe(svec![
      "deno",
      "run",
      "--v8-flags=--help",
      "script.ts"
    ]);
    assert_eq!(
      r.unwrap(),
      DenoFlags {
        subcommand: DenoSubcommand::Run,
        argv: svec!["deno", "script.ts"],
        v8_flags: Some(svec!["--help"]),
        ..DenoFlags::default()
      }
    );

    let r = flags_from_vec_safe(svec![
      "deno",
      "run",
      "--v8-flags=--expose-gc,--gc-stats=1",
      "script.ts"
    ]);
    assert_eq!(
      r.unwrap(),
      DenoFlags {
        subcommand: DenoSubcommand::Run,
        argv: svec!["deno", "script.ts"],
        v8_flags: Some(svec!["--expose-gc", "--gc-stats=1"]),
        ..DenoFlags::default()
      }
    );
  }

  #[test]
  fn script_args() {
    let r = flags_from_vec_safe(svec![
      "deno",
      "run",
      "--allow-net",
      "gist.ts",
      "--title",
      "X"
    ]);
    assert_eq!(
      r.unwrap(),
      DenoFlags {
        subcommand: DenoSubcommand::Run,
        argv: svec!["deno", "gist.ts", "--title", "X"],
        allow_net: true,
        ..DenoFlags::default()
      }
    );
  }

  #[test]
  fn allow_all() {
    let r = flags_from_vec_safe(svec!["deno", "run", "--allow-all", "gist.ts"]);
    assert_eq!(
      r.unwrap(),
      DenoFlags {
        subcommand: DenoSubcommand::Run,
        argv: svec!["deno", "gist.ts"],
        allow_net: true,
        allow_env: true,
        allow_run: true,
        allow_read: true,
        allow_write: true,
        allow_plugin: true,
        allow_hrtime: true,
        ..DenoFlags::default()
      }
    );
  }

  #[test]
  fn allow_read() {
    let r =
      flags_from_vec_safe(svec!["deno", "run", "--allow-read", "gist.ts"]);
    assert_eq!(
      r.unwrap(),
      DenoFlags {
        subcommand: DenoSubcommand::Run,
        argv: svec!["deno", "gist.ts"],
        allow_read: true,
        ..DenoFlags::default()
      }
    );
  }

  #[test]
  fn allow_hrtime() {
    let r =
      flags_from_vec_safe(svec!["deno", "run", "--allow-hrtime", "gist.ts"]);
    assert_eq!(
      r.unwrap(),
      DenoFlags {
        subcommand: DenoSubcommand::Run,
        argv: svec!["deno", "gist.ts"],
        allow_hrtime: true,
        ..DenoFlags::default()
      }
    );
  }

  #[test]
  fn double_hyphen() {
    // notice that flags passed after double dash will not
    // be parsed to DenoFlags but instead forwarded to
    // script args as Deno.args
    let r = flags_from_vec_safe(svec![
      "deno",
      "run",
      "--allow-write",
      "script.ts",
      "--",
      "-D",
      "--allow-net"
    ]);
    assert_eq!(
      r.unwrap(),
      DenoFlags {
        subcommand: DenoSubcommand::Run,
        argv: svec!["deno", "script.ts", "--", "-D", "--allow-net"],
        allow_write: true,
        ..DenoFlags::default()
      }
    );
  }

  #[test]
  fn fmt() {
    let r =
      flags_from_vec_safe(svec!["deno", "fmt", "script_1.ts", "script_2.ts"]);
    assert_eq!(
      r.unwrap(),
      DenoFlags {
        subcommand: DenoSubcommand::Run,
        allow_write: true,
        allow_read: true,
        argv: svec![
          "deno",
          PRETTIER_URL,
          "script_1.ts",
          "script_2.ts",
          "--write",
          "--config",
          "auto",
          "--ignore-path",
          "auto"
        ],
        ..DenoFlags::default()
      }
    );
  }

  #[test]
  fn types() {
    let r = flags_from_vec_safe(svec!["deno", "types"]);
    assert_eq!(
      r.unwrap(),
      DenoFlags {
        subcommand: DenoSubcommand::Types,
        argv: svec!["deno"],
        ..DenoFlags::default()
      }
    );
  }

  #[test]
  fn fetch() {
    let r = flags_from_vec_safe(svec!["deno", "fetch", "script.ts"]);
    assert_eq!(
      r.unwrap(),
      DenoFlags {
        subcommand: DenoSubcommand::Fetch,
        argv: svec!["deno", "script.ts"],
        ..DenoFlags::default()
      }
    );
  }

  #[test]
  fn info() {
    let r = flags_from_vec_safe(svec!["deno", "info", "script.ts"]);
    assert_eq!(
      r.unwrap(),
      DenoFlags {
        subcommand: DenoSubcommand::Info,
        // TODO(ry) I'm not sure the argv values in this case make sense.
        // Nothing is being executed. Shouldn't argv be empty?
        argv: svec!["deno", "script.ts"],
        ..DenoFlags::default()
      }
    );

    let r = flags_from_vec_safe(svec!["deno", "info"]);
    assert_eq!(
      r.unwrap(),
      DenoFlags {
        subcommand: DenoSubcommand::Info,
        argv: svec!["deno"], // TODO(ry) Ditto argv unnecessary?
        ..DenoFlags::default()
      }
    );
  }

  #[test]
  fn tsconfig() {
    let r = flags_from_vec_safe(svec![
      "deno",
      "run",
      "-c",
      "tsconfig.json",
      "script.ts"
    ]);
    assert_eq!(
      r.unwrap(),
      DenoFlags {
        subcommand: DenoSubcommand::Run,
        argv: svec!["deno", "script.ts"],
        config_path: Some("tsconfig.json".to_owned()),
        ..DenoFlags::default()
      }
    );
  }

  #[test]
  fn eval() {
    let r =
      flags_from_vec_safe(svec!["deno", "eval", "'console.log(\"hello\")'"]);
    assert_eq!(
      r.unwrap(),
      DenoFlags {
        subcommand: DenoSubcommand::Eval,
        // TODO(ry) argv in this test seems odd and potentially not correct.
        argv: svec!["deno", "'console.log(\"hello\")'"],
        allow_net: true,
        allow_env: true,
        allow_run: true,
        allow_read: true,
        allow_write: true,
        allow_plugin: true,
        allow_hrtime: true,
        ..DenoFlags::default()
      }
    );
  }

  #[test]
  fn repl() {
    let r = flags_from_vec_safe(svec!["deno"]);
    assert_eq!(
      r.unwrap(),
      DenoFlags {
        subcommand: DenoSubcommand::Repl,
        argv: svec!["deno"],
        allow_net: true,
        allow_env: true,
        allow_run: true,
        allow_read: true,
        allow_write: true,
        allow_plugin: true,
        allow_hrtime: true,
        ..DenoFlags::default()
      }
    );
  }

  #[test]
  fn allow_read_whitelist() {
    use tempfile::TempDir;
    let temp_dir = TempDir::new().expect("tempdir fail");
    let temp_dir_path = temp_dir.path().to_str().unwrap();

    let r = flags_from_vec_safe(svec![
      "deno",
      "run",
      format!("--allow-read=.,{}", &temp_dir_path),
      "script.ts"
    ]);
    assert_eq!(
      r.unwrap(),
      DenoFlags {
        allow_read: false,
        read_whitelist: svec![
          current_dir().unwrap().to_str().unwrap().to_owned(),
          &temp_dir_path
        ],
        argv: svec!["deno", "script.ts"],
        subcommand: DenoSubcommand::Run,
        ..DenoFlags::default()
      }
    );
  }

  #[test]
  fn allow_write_whitelist() {
    use tempfile::TempDir;
    let temp_dir = TempDir::new().expect("tempdir fail");
    let temp_dir_path = temp_dir.path().to_str().unwrap();

    let r = flags_from_vec_safe(svec![
      "deno",
      "run",
      format!("--allow-write=.,{}", &temp_dir_path),
      "script.ts"
    ]);
    assert_eq!(
      r.unwrap(),
      DenoFlags {
        allow_write: false,
        write_whitelist: svec![
          current_dir().unwrap().to_str().unwrap().to_owned(),
          &temp_dir_path
        ],
        argv: svec!["deno", "script.ts"],
        subcommand: DenoSubcommand::Run,
        ..DenoFlags::default()
      }
    );
  }

  #[test]
  fn allow_net_whitelist() {
    let r = flags_from_vec_safe(svec![
      "deno",
      "run",
      "--allow-net=127.0.0.1",
      "script.ts"
    ]);
    assert_eq!(
      r.unwrap(),
      DenoFlags {
        subcommand: DenoSubcommand::Run,
        argv: svec!["deno", "script.ts"],
        allow_net: false,
        net_whitelist: svec!["127.0.0.1"],
        ..DenoFlags::default()
      }
    );
  }

  #[test]
  fn fmt_stdout() {
    let r = flags_from_vec_safe(svec![
      "deno",
      "fmt",
      "--stdout",
      "script_1.ts",
      "script_2.ts"
    ]);
    assert_eq!(
      r.unwrap(),
      DenoFlags {
        subcommand: DenoSubcommand::Run,
        argv: svec![
          "deno",
          PRETTIER_URL,
          "script_1.ts",
          "script_2.ts",
          "--config",
          "auto",
          "--ignore-path",
          "auto"
        ],
        allow_write: true,
        allow_read: true,
        ..DenoFlags::default()
      }
    );
  }

  #[test]
  fn default_to_run() {
    let r = flags_from_vec_safe(svec!["deno", "script.ts"]);
    assert_eq!(
      r.unwrap(),
      DenoFlags {
        subcommand: DenoSubcommand::Run,
        argv: svec!["deno", "script.ts"],
        ..DenoFlags::default()
      }
    );
  }

  #[test]
  fn default_to_run_with_permissions() {
    let r = flags_from_vec_safe(svec![
      "deno",
      "--allow-net",
      "--allow-read",
      "script.ts"
    ]);
    assert_eq!(
      r.unwrap(),
      DenoFlags {
        subcommand: DenoSubcommand::Run,
        argv: svec!["deno", "script.ts"],
        allow_net: true,
        allow_read: true,
        ..DenoFlags::default()
      }
    );
  }

  #[test]
  fn bundle() {
    let r = flags_from_vec_safe(svec!["deno", "bundle", "source.ts"]);
    assert_eq!(
      r.unwrap(),
      DenoFlags {
        subcommand: DenoSubcommand::Bundle,
        argv: svec!["deno", "source.ts"],
        bundle_output: None,
        ..DenoFlags::default()
      }
    );
  }

  #[test]
  fn bundle_with_output() {
    let r =
      flags_from_vec_safe(svec!["deno", "bundle", "source.ts", "bundle.js"]);
    assert_eq!(
      r.unwrap(),
      DenoFlags {
        subcommand: DenoSubcommand::Bundle,
        argv: svec!["deno", "source.ts"],
        bundle_output: Some("bundle.js".to_string()),
        allow_write: true,
        ..DenoFlags::default()
      }
    );
  }

  #[test]
  fn run_importmap() {
    let r = flags_from_vec_safe(svec![
      "deno",
      "run",
      "--importmap=importmap.json",
      "script.ts"
    ]);
    assert_eq!(
      r.unwrap(),
      DenoFlags {
        subcommand: DenoSubcommand::Run,
        argv: svec!["deno", "script.ts"],
        import_map_path: Some("importmap.json".to_owned()),
        ..DenoFlags::default()
      }
    );
  }

  #[test]
  fn default_to_run_importmap() {
    let r = flags_from_vec_safe(svec![
      "deno",
      "--importmap=importmap.json",
      "script.ts"
    ]);
    assert_eq!(
      r.unwrap(),
      DenoFlags {
        subcommand: DenoSubcommand::Run,
        argv: svec!["deno", "script.ts"],
        import_map_path: Some("importmap.json".to_owned()),
        ..DenoFlags::default()
      }
    );
  }

  #[test]
  fn fetch_importmap() {
    let r = flags_from_vec_safe(svec![
      "deno",
      "fetch",
      "--importmap=importmap.json",
      "script.ts"
    ]);
    assert_eq!(
      r.unwrap(),
      DenoFlags {
        subcommand: DenoSubcommand::Fetch,
        argv: svec!["deno", "script.ts"],
        import_map_path: Some("importmap.json".to_owned()),
        ..DenoFlags::default()
      }
    );
  }

  #[test]
  fn run_seed() {
    let r =
      flags_from_vec_safe(svec!["deno", "run", "--seed", "250", "script.ts"]);
    assert_eq!(
      r.unwrap(),
      DenoFlags {
        subcommand: DenoSubcommand::Run,
        argv: svec!["deno", "script.ts"],
        seed: Some(250 as u64),
        v8_flags: Some(svec!["--random-seed=250"]),
        ..DenoFlags::default()
      }
    );
  }

  #[test]
  fn run_seed_with_v8_flags() {
    let r = flags_from_vec_safe(svec![
      "deno",
      "run",
      "--seed",
      "250",
      "--v8-flags=--expose-gc",
      "script.ts"
    ]);
    assert_eq!(
      r.unwrap(),
      DenoFlags {
        subcommand: DenoSubcommand::Run,
        argv: svec!["deno", "script.ts"],
        seed: Some(250 as u64),
        v8_flags: Some(svec!["--expose-gc", "--random-seed=250"]),
        ..DenoFlags::default()
      }
    );
  }

  #[test]
  fn install() {
    let r = flags_from_vec_safe(svec![
      "deno",
      "install",
      "deno_colors",
      "https://deno.land/std/examples/colors.ts"
    ]);
    assert_eq!(
      r.unwrap(),
      DenoFlags {
        subcommand: DenoSubcommand::Run,
        argv: svec![
          "deno",
          INSTALLER_URL,
          "deno_colors",
          "https://deno.land/std/examples/colors.ts"
        ],
        allow_write: true,
        allow_net: true,
        allow_read: true,
        allow_env: true,
        allow_run: true,
        ..DenoFlags::default()
      }
    );
  }

  #[test]
  fn install_with_args() {
    let r = flags_from_vec_safe(svec![
      "deno",
      "install",
      "file_server",
      "https://deno.land/std/http/file_server.ts",
      "--allow-net",
      "--allow-read"
    ]);
    assert_eq!(
      r.unwrap(),
      DenoFlags {
        subcommand: DenoSubcommand::Run,
        argv: svec![
          "deno",
          INSTALLER_URL,
          "file_server",
          "https://deno.land/std/http/file_server.ts",
          "--allow-net",
          "--allow-read"
        ],
        allow_write: true,
        allow_net: true,
        allow_read: true,
        allow_env: true,
        allow_run: true,
        ..DenoFlags::default()
      }
    );
  }

  #[test]
  fn install_with_args_and_dir() {
    let r = flags_from_vec_safe(svec![
      "deno",
      "install",
      "-d",
      "/usr/local/bin",
      "file_server",
      "https://deno.land/std/http/file_server.ts",
      "--allow-net",
      "--allow-read"
    ]);
    assert_eq!(
      r.unwrap(),
      DenoFlags {
        subcommand: DenoSubcommand::Run,
        argv: svec![
          "deno",
          INSTALLER_URL,
          "--dir",
          "/usr/local/bin",
          "file_server",
          "https://deno.land/std/http/file_server.ts",
          "--allow-net",
          "--allow-read"
        ],
        allow_write: true,
        allow_net: true,
        allow_read: true,
        allow_env: true,
        allow_run: true,
        ..DenoFlags::default()
      }
    );
  }

  #[test]
  fn log_level() {
    let r =
      flags_from_vec_safe(svec!["deno", "--log-level=debug", "script.ts"]);
    assert_eq!(
      r.unwrap(),
      DenoFlags {
        subcommand: DenoSubcommand::Run,
        argv: svec!["deno", "script.ts"],
        log_level: Some(Level::Debug),
        ..DenoFlags::default()
      }
    );
  }

  #[test]
  fn completions() {
    let r = flags_from_vec_safe(svec!["deno", "completions", "bash"]);
    assert_eq!(
      r.unwrap(),
      DenoFlags {
        subcommand: DenoSubcommand::Completions,
        argv: svec!["deno"], // TODO(ry) argv doesn't make sense here. Make it Option.
        ..DenoFlags::default()
      }
    );
  }

  /* TODO(ry) Fix this test
  #[test]
  fn test_flags_from_vec_33() {
    let (flags, subcommand, argv) =
      flags_from_vec_safe(svec!["deno", "script.ts", "--allow-read", "--allow-net"]);
    assert_eq!(
      flags,
      DenoFlags {
        allow_net: true,
        allow_read: true,
        ..DenoFlags::default()
      }
    );
    assert_eq!(subcommand, DenoSubcommand::Run);
    assert_eq!(argv, svec!["deno", "script.ts"]);

    let (flags, subcommand, argv) = flags_from_vec_safe(svec![
      "deno",
      "run",
      "--allow-read",
      "script.ts",
      "--allow-net",
      "-r",
      "--help",
      "--foo",
      "bar"
    ]);
    assert_eq!(
      flags,
      DenoFlags {
        allow_net: true,
        allow_read: true,
        reload: true,
        ..DenoFlags::default()
      }
    );
    assert_eq!(subcommand, DenoSubcommand::Run);
    assert_eq!(argv, svec!["deno", "script.ts", "--help", "--foo", "bar"]);

    let (flags, subcommand, argv) =
      flags_from_vec_safe(svec!["deno", "script.ts", "foo", "bar"]);
    assert_eq!(flags, DenoFlags::default());
    assert_eq!(subcommand, DenoSubcommand::Run);
    assert_eq!(argv, svec!["deno", "script.ts", "foo", "bar"]);

    let (flags, subcommand, argv) =
      flags_from_vec_safe(svec!["deno", "script.ts", "-"]);
    assert_eq!(flags, DenoFlags::default());
    assert_eq!(subcommand, DenoSubcommand::Run);
    assert_eq!(argv, svec!["deno", "script.ts", "-"]);

    let (flags, subcommand, argv) =
      flags_from_vec_safe(svec!["deno", "script.ts", "-", "foo", "bar"]);
    assert_eq!(flags, DenoFlags::default());
    assert_eq!(subcommand, DenoSubcommand::Run);
    assert_eq!(argv, svec!["deno", "script.ts", "-", "foo", "bar"]);
  }
  */

  #[test]
  fn no_remote() {
    let r = flags_from_vec_safe(svec!["deno", "--no-remote", "script.ts"]);
    assert_eq!(
      r.unwrap(),
      DenoFlags {
        subcommand: DenoSubcommand::Run,
        argv: svec!["deno", "script.ts"],
        no_remote: true,
        ..DenoFlags::default()
      }
    );
  }

  #[test]
  fn cached_only() {
    let r = flags_from_vec_safe(svec!["deno", "--cached-only", "script.ts"]);
    assert_eq!(
      r.unwrap(),
      DenoFlags {
        subcommand: DenoSubcommand::Run,
        argv: svec!["deno", "script.ts"],
        cached_only: true,
        ..DenoFlags::default()
      }
    );
  }

  #[test]
  fn current_thread() {
    let r = flags_from_vec_safe(svec!["deno", "--current-thread", "script.ts"]);
    assert_eq!(
      r.unwrap(),
      DenoFlags {
        subcommand: DenoSubcommand::Run,
        argv: svec!["deno", "script.ts"],
        current_thread: true,
        ..DenoFlags::default()
      }
    );
  }

  #[test]
  fn allow_net_whitelist_with_ports() {
    let r = flags_from_vec_safe(svec![
      "deno",
      "--allow-net=deno.land,:8000,:4545",
      "script.ts"
    ]);
    assert_eq!(
      r.unwrap(),
      DenoFlags {
        subcommand: DenoSubcommand::Run,
        argv: svec!["deno", "script.ts"],
        net_whitelist: svec![
          "deno.land",
          "0.0.0.0:8000",
          "127.0.0.1:8000",
          "localhost:8000",
          "0.0.0.0:4545",
          "127.0.0.1:4545",
          "localhost:4545"
        ],
        ..DenoFlags::default()
      }
    );
  }

  #[test]
  fn lock_write() {
    let r = flags_from_vec_safe(svec![
      "deno",
      "--lock-write",
      "--lock=lock.json",
      "script.ts"
    ]);
    assert_eq!(
      r.unwrap(),
      DenoFlags {
        subcommand: DenoSubcommand::Run,
        argv: svec!["deno", "script.ts"],
        lock_write: true,
        lock: Some("lock.json".to_string()),
        ..DenoFlags::default()
      }
    );
  }

  #[test]
  fn fmt_args() {
    let r = flags_from_vec_safe(svec![
      "deno",
      "fmt",
      "--check",
      "--prettierrc=auto",
      "--print-width=100",
      "--tab-width=4",
      "--use-tabs",
      "--no-semi",
      "--single-quote",
      "--arrow-parens=always",
      "--prose-wrap=preserve",
      "--end-of-line=crlf",
      "--quote-props=preserve",
      "--jsx-single-quote",
      "--jsx-bracket-same-line",
      "--ignore-path=.prettier-ignore",
      "script.ts"
    ]);
    assert_eq!(
      r.unwrap(),
      DenoFlags {
        subcommand: DenoSubcommand::Run,
        argv: svec![
          "deno",
          PRETTIER_URL,
          "script.ts",
          "--write",
          "--check",
          "--config",
          "auto",
          "--ignore-path",
          ".prettier-ignore",
          "--print-width",
          "100",
          "--tab-width",
          "4",
          "--use-tabs",
          "--no-semi",
          "--single-quote",
          "--quote-props",
          "preserve",
          "--jsx-single-quote",
          "--jsx-bracket-same-line",
          "--arrow-parens",
          "always",
          "--prose-wrap",
          "preserve",
          "--end-of-line",
          "crlf"
        ],
        allow_write: true,
        allow_read: true,
        ..DenoFlags::default()
      }
    );
  }

  #[test]
  fn test_with_exclude() {
    let r = flags_from_vec_safe(svec![
      "deno",
      "test",
      "--exclude",
      "some_dir/",
      "dir1/",
      "dir2/"
    ]);
    assert_eq!(
      r.unwrap(),
      DenoFlags {
        subcommand: DenoSubcommand::Run,
        argv: svec![
          "deno",
          TEST_RUNNER_URL,
          "--exclude",
          "some_dir/",
          "--",
          "dir1/",
          "dir2/"
        ],
        allow_read: true,
        ..DenoFlags::default()
      }
    );
  }

  #[test]
  fn test_with_allow_net() {
    let r = flags_from_vec_safe(svec![
      "deno",
      "test",
      "--allow-net",
      "dir1/",
      "dir2/"
    ]);
    assert_eq!(
      r.unwrap(),
      DenoFlags {
        subcommand: DenoSubcommand::Run,
        argv: svec!["deno", TEST_RUNNER_URL, "--", "dir1/", "dir2/"],
        allow_read: true,
        allow_net: true,
        ..DenoFlags::default()
      }
    );
  }
}
