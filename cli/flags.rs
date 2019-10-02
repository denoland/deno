// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use crate::fs as deno_fs;
use clap::App;
use clap::AppSettings;
use clap::Arg;
use clap::ArgMatches;
use clap::Shell;
use clap::SubCommand;
use deno::ModuleSpecifier;
use log::Level;
use std;
use std::str;
use std::str::FromStr;

macro_rules! std_url {
  ($x:expr) => {
    concat!("https://deno.land/std@17a214b/", $x)
  };
}

/// Used for `deno fmt <files>...` subcommand
const PRETTIER_URL: &str = std_url!("prettier/main.ts");
/// Used for `deno install...` subcommand
const INSTALLER_URL: &str = std_url!("installer/mod.ts");
/// Used for `deno test...` subcommand
const TEST_RUNNER_URL: &str = std_url!("testing/runner.ts");

// Creates vector of strings, Vec<String>
macro_rules! svec {
    ($($x:expr),*) => (vec![$($x.to_string()),*]);
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct DenoFlags {
  pub log_level: Option<Level>,
  pub version: bool,
  pub reload: bool,
  /// When the `--config`/`-c` flag is used to pass the name, this will be set
  /// the path passed on the command line, otherwise `None`.
  pub config_path: Option<String>,
  /// When the `--importmap` flag is used to pass the name, this will be set
  /// the path passed on the command line, otherwise `None`.
  pub import_map_path: Option<String>,
  pub allow_read: bool,
  pub read_whitelist: Vec<String>,
  pub allow_write: bool,
  pub write_whitelist: Vec<String>,
  pub allow_net: bool,
  pub net_whitelist: Vec<String>,
  pub allow_env: bool,
  pub allow_run: bool,
  pub allow_hrtime: bool,
  pub no_prompts: bool,
  pub no_fetch: bool,
  pub seed: Option<u64>,
  pub v8_flags: Option<Vec<String>>,
  pub xeval_replvar: Option<String>,
  pub xeval_delim: Option<String>,
  // Use tokio::runtime::current_thread
  pub current_thread: bool,
}

static ENV_VARIABLES_HELP: &str = "ENVIRONMENT VARIABLES:
    DENO_DIR        Set deno's base directory
    NO_COLOR        Set to disable color
    HTTP_PROXY      Set proxy address for HTTP requests (module downloads, fetch)
    HTTPS_PROXY     Set proxy address for HTTPS requests (module downloads, fetch)";

fn add_run_args<'a, 'b>(app: App<'a, 'b>) -> App<'a, 'b> {
  app
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
      Arg::with_name("no-prompt")
        .long("no-prompt")
        .help("Do not use prompts"),
    )
    .arg(
      Arg::with_name("no-fetch")
        .long("no-fetch")
        .help("Do not download remote modules"),
    )
}

pub fn create_cli_app<'a, 'b>() -> App<'a, 'b> {
  add_run_args(App::new("deno"))
    .bin_name("deno")
    .global_settings(&[AppSettings::ColorNever, AppSettings::UnifiedHelpMessage, AppSettings::DisableVersion])
    .settings(&[AppSettings::AllowExternalSubcommands])
    .after_help(ENV_VARIABLES_HELP)
    .long_about("A secure runtime for JavaScript and TypeScript built with V8, Rust, and Tokio.

Docs: https://deno.land/manual.html
Modules: https://github.com/denoland/deno_std
Bugs: https://github.com/denoland/deno/issues

To run the REPL:

  deno

To execute a sandboxed script:

  deno https://deno.land/welcome.ts

To evaluate code from the command line:

  deno eval \"console.log(30933 + 404)\"

To get help on the another subcommands (run in this case):

  deno help run")
    .arg(
      Arg::with_name("version")
        .short("v")
        .long("version")
        .help("Print the version"),
    )
    .arg(
      Arg::with_name("log-level")
        .short("L")
        .long("log-level")
        .help("Set log level")
        .takes_value(true)
        .possible_values(&["debug", "info"])
        .global(true),
    ).arg(
      Arg::with_name("reload")
        .short("r")
        .long("reload")
        .help("Reload source code cache (recompile TypeScript)")
        .global(true),
    ).arg(
      Arg::with_name("config")
        .short("c")
        .long("config")
        .value_name("FILE")
        .help("Load compiler configuration file")
        .takes_value(true)
        .global(true),
    )
    .arg(
      Arg::with_name("current-thread")
        .long("current-thread")
        .global(true)
        .help("Use tokio::runtime::current_thread"),
    ).arg(
      Arg::with_name("importmap")
        .long("importmap")
        .value_name("FILE")
        .help("Load import map file")
        .long_help(
          "Load import map file
Specification: https://wicg.github.io/import-maps/
Examples: https://github.com/WICG/import-maps#the-import-map",
        )
        .takes_value(true)
        .global(true),
    ).arg(
      Arg::with_name("seed")
        .long("seed")
        .value_name("NUMBER")
        .help("Seed Math.random()")
        .takes_value(true)
        .validator(|val: String| {
          match val.parse::<u64>() {
            Ok(_) => Ok(()),
            Err(_) => Err("Seed should be a number".to_string())
          }
        })
        .global(true),
    ).arg(
      Arg::with_name("v8-options")
        .long("v8-options")
        .help("Print V8 command line options")
        .global(true),
    ).arg(
      Arg::with_name("v8-flags")
        .long("v8-flags")
        .takes_value(true)
        .use_delimiter(true)
        .require_equals(true)
        .help("Set V8 command line options")
        .global(true),
    ).subcommand(
      SubCommand::with_name("version")
        .about("Print the version")
        .long_about("Print current version of Deno.

Includes versions of Deno, V8 JavaScript Engine, and the TypeScript
compiler.",
        ),
    ).subcommand(
      SubCommand::with_name("bundle")
        .about("Bundle module and dependencies into single file")
        .long_about(
          "Output a single JavaScript file with all dependencies

Example:

  deno bundle https://deno.land/std/examples/colors.ts"
        )
          .arg(Arg::with_name("source_file").takes_value(true).required(true))
          .arg(Arg::with_name("out_file").takes_value(true).required(false)),
    ).subcommand(
      SubCommand::with_name("fetch")
        .about("Fetch the dependencies")
        .long_about(
          "Fetch and compile remote dependencies recursively.

Downloads all statically imported scripts and save them in local
cache, without running the code. No future import network requests
would be made unless --reload is specified.

  # Downloads all dependencies
  deno fetch https://deno.land/std/http/file_server.ts

  # Once cached, static imports no longer send network requests
  deno run -A https://deno.land/std/http/file_server.ts",
        ).arg(Arg::with_name("file").takes_value(true).required(true)),
    ).subcommand(
      SubCommand::with_name("types")
        .about("Print runtime TypeScript declarations")
        .long_about("Print runtime TypeScript declarations.

  deno types > lib.deno_runtime.d.ts

The declaration file could be saved and used for typing information.",
        ),
    ).subcommand(
      SubCommand::with_name("info")
        .about("Show info about cache or info related to source file")
        .long_about("Show info about cache or info related to source file.

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
        ).arg(Arg::with_name("file").takes_value(true).required(false)),
    ).subcommand(
      SubCommand::with_name("eval")
        .about("Eval script")
        .long_about(
          "Evaluate provided script.

This command has implicit access to all permissions (equivalent to deno run --allow-all)

  deno eval \"console.log('hello world')\"",
        ).arg(Arg::with_name("code").takes_value(true).required(true)),
    ).subcommand(
      SubCommand::with_name("fmt")
        .about("Format files")
        .long_about(
"Auto-format JavaScript/TypeScript source code using Prettier

Automatically downloads Prettier dependencies on first run.

  deno fmt myfile1.ts myfile2.ts",
        ).arg(
          Arg::with_name("stdout")
            .long("stdout")
            .help("Output formated code to stdout")
            .takes_value(false),
        ).arg(
          Arg::with_name("files")
            .takes_value(true)
            .multiple(true)
            .required(true),
        ),
    ).subcommand(
      add_run_args(SubCommand::with_name("test"))
        .about("Run tests")
        .long_about(
"Run tests using test runner

Automatically downloads test runner on first run.

  deno test **/*_test.ts **/test.ts",
        ).arg(
          Arg::with_name("failfast")
            .short("f")
            .long("failfast")
            .help("Stop on first error")
            .takes_value(false),
        ).arg(
          Arg::with_name("quiet")
            .short("q")
            .long("quiet")
            .help("Don't show output from test cases")
            .takes_value(false)
        ).arg(
          Arg::with_name("exclude")
            .short("e")
            .long("exclude")
            .help("List of file names to exclude from run")
            .takes_value(true)
            .multiple(true)
        ).arg(
          Arg::with_name("files")
            .help("List of file names to run")
            .takes_value(true)
            .multiple(true)
        ),
    ).subcommand(
      add_run_args(SubCommand::with_name("run"))
        .settings(&[
          AppSettings::AllowExternalSubcommands,
          AppSettings::DisableHelpSubcommand,
          AppSettings::SubcommandRequired,
        ]).about("Run a program given a filename or url to the source code")
        .long_about(
          "Run a program given a filename or url to the source code.

By default all programs are run in sandbox without access to disk, network or
ability to spawn subprocesses.

  deno run https://deno.land/welcome.ts

  # run program with permission to read from disk and listen to network
  deno run --allow-net --allow-read https://deno.land/std/http/file_server.ts

  # run program with permission to read whitelist files from disk and listen to network
  deno run --allow-net --allow-read=$(pwd) https://deno.land/std/http/file_server.ts

  # run program with all permissions
  deno run -A https://deno.land/std/http/file_server.ts",
        ).subcommand(
          // this is a fake subcommand - it's used in conjunction with
          // AppSettings:AllowExternalSubcommand to treat it as an
          // entry point script
          SubCommand::with_name("[SCRIPT]").about("Script to run"),
        ),
    ).subcommand(
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
        ).arg(
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
        ).arg(Arg::with_name("code").takes_value(true).required(true)),
    ).subcommand(
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
        ).arg(
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
        ),
    ).subcommand(
      SubCommand::with_name("completions")
        .settings(&[
          AppSettings::DisableHelpSubcommand,
        ]).about("Generate shell completions")
        .long_about(
"Output shell completion script to standard output.

Example:

  deno completions bash > /usr/local/etc/bash_completion.d/deno.bash
  source /usr/local/etc/bash_completion.d/deno.bash")
        .arg(
          Arg::with_name("shell")
          .possible_values(&Shell::variants())
          .required(true),
        ),
  ).subcommand(
      // this is a fake subcommand - it's used in conjunction with
      // AppSettings:AllowExternalSubcommand to treat it as an
      // entry point script
      SubCommand::with_name("[SCRIPT]").about("Script to run"),
    )
}

/// Convert paths supplied into full path.
/// If a path is invalid, we print out a warning
/// and ignore this path in the output.
fn resolve_paths(paths: Vec<String>) -> Vec<String> {
  let mut out: Vec<String> = vec![];
  for pathstr in paths.iter() {
    let result = deno_fs::resolve_from_cwd(pathstr);
    if result.is_err() {
      eprintln!("Unrecognized path to whitelist: {}", pathstr);
      continue;
    }
    let mut full_path = result.unwrap().1;
    // Remove trailing slash.
    if full_path.len() > 1 && full_path.ends_with('/') {
      full_path.pop();
    }
    out.push(full_path);
  }
  out
}

/// Parse ArgMatches into internal DenoFlags structure.
/// This method should not make any side effects.
pub fn parse_flags(
  matches: &ArgMatches,
  maybe_flags: Option<DenoFlags>,
) -> DenoFlags {
  let mut flags = maybe_flags.unwrap_or_default();

  if matches.is_present("current-thread") {
    flags.current_thread = true;
  }
  if matches.is_present("log-level") {
    flags.log_level = match matches.value_of("log-level").unwrap() {
      "debug" => Some(Level::Debug),
      "info" => Some(Level::Info),
      _ => unreachable!(),
    };
  }
  if matches.is_present("version") {
    flags.version = true;
  }
  if matches.is_present("reload") {
    flags.reload = true;
  }
  flags.config_path = matches.value_of("config").map(ToOwned::to_owned);
  if matches.is_present("v8-options") {
    let v8_flags = svec!["deno", "--help"];
    flags.v8_flags = Some(v8_flags);
  }
  if matches.is_present("v8-flags") {
    let mut v8_flags: Vec<String> = matches
      .values_of("v8-flags")
      .unwrap()
      .map(String::from)
      .collect();

    v8_flags.insert(0, "deno".to_string());
    flags.v8_flags = Some(v8_flags);
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
        flags.v8_flags = Some(svec!["deno", v8_seed_flag]);
      }
    }
  }

  flags = parse_run_args(flags, matches);
  // flags specific to "run" subcommand
  if let Some(run_matches) = matches.subcommand_matches("run") {
    flags = parse_run_args(flags.clone(), run_matches);
  }
  // flags specific to "test" subcommand
  if let Some(test_matches) = matches.subcommand_matches("test") {
    flags = parse_run_args(flags.clone(), test_matches);
  }

  flags
}

/// Parse permission specific matches Args and assign to DenoFlags.
/// This method is required because multiple subcommands use permission args.
fn parse_run_args(mut flags: DenoFlags, matches: &ArgMatches) -> DenoFlags {
  if matches.is_present("allow-read") {
    if matches.value_of("allow-read").is_some() {
      let read_wl = matches.values_of("allow-read").unwrap();
      let raw_read_whitelist: Vec<String> =
        read_wl.map(std::string::ToString::to_string).collect();
      flags.read_whitelist = resolve_paths(raw_read_whitelist);
      debug!("read whitelist: {:#?}", &flags.read_whitelist);
    } else {
      flags.allow_read = true;
    }
  }
  if matches.is_present("allow-write") {
    if matches.value_of("allow-write").is_some() {
      let write_wl = matches.values_of("allow-write").unwrap();
      let raw_write_whitelist =
        write_wl.map(std::string::ToString::to_string).collect();
      flags.write_whitelist = resolve_paths(raw_write_whitelist);
      debug!("write whitelist: {:#?}", &flags.write_whitelist);
    } else {
      flags.allow_write = true;
    }
  }
  if matches.is_present("allow-net") {
    if matches.value_of("allow-net").is_some() {
      let net_wl = matches.values_of("allow-net").unwrap();
      flags.net_whitelist =
        net_wl.map(std::string::ToString::to_string).collect();
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
    flags.allow_hrtime = true;
  }
  if matches.is_present("no-prompt") {
    flags.no_prompts = true;
  }
  if matches.is_present("no-fetch") {
    flags.no_fetch = true;
  }
  flags.import_map_path = matches.value_of("importmap").map(ToOwned::to_owned);

  flags
}

/// Parse vector or arguments as DenoFlags.
///
/// This is very specialized utility that parses arguments passed after script URL.
///
/// Only dash (eg. `-r`) and double dash (eg. `--reload`) arguments are supported.
/// Arguments recognized as DenoFlags will be eaten.
/// Parsing stops after double dash `--` argument.
///
/// NOTE: this method ignores `-h/--help` and `-v/--version` flags.
fn parse_script_args(
  args: Vec<String>,
  mut flags: DenoFlags,
) -> (Vec<String>, DenoFlags) {
  let mut argv = vec![];
  let mut seen_double_dash = false;

  // we have to iterate and parse argument one by one because clap returns error on any
  // unrecognized argument
  for arg in args.iter() {
    if seen_double_dash {
      argv.push(arg.to_string());
      continue;
    }

    if arg == "--" {
      seen_double_dash = true;
      argv.push(arg.to_string());
      continue;
    }

    if !arg.starts_with('-') || arg == "-" {
      argv.push(arg.to_string());
      continue;
    }

    let cli_app = create_cli_app();
    // `get_matches_from_safe` returns error for `-h/-v` flags
    let matches =
      cli_app.get_matches_from_safe(vec!["deno".to_string(), arg.to_string()]);

    if matches.is_ok() {
      flags = parse_flags(&matches.unwrap(), Some(flags));
    } else {
      argv.push(arg.to_string());
    }
  }

  (argv, flags)
}

/// These are currently handled subcommands.
/// There is no "Help" subcommand because it's handled by `clap::App` itself.
#[derive(Debug, PartialEq)]
pub enum DenoSubcommand {
  Bundle,
  Completions,
  Eval,
  Fetch,
  Info,
  Repl,
  Run,
  Types,
  Version,
  Xeval,
}

fn get_default_bundle_filename(source_file: &str) -> String {
  let specifier = ModuleSpecifier::resolve_url_or_path(source_file).unwrap();
  let path_segments = specifier.as_url().path_segments().unwrap();
  let file_name = path_segments.filter(|s| !s.is_empty()).last().unwrap();
  let file_stem = file_name.trim_end_matches(".ts").trim_end_matches(".js");
  format!("{}.bundle.js", file_stem)
}

#[test]
fn test_get_default_bundle_filename() {
  assert_eq!(get_default_bundle_filename("blah.ts"), "blah.bundle.js");
  assert_eq!(
    get_default_bundle_filename("http://example.com/blah.ts"),
    "blah.bundle.js"
  );
  assert_eq!(get_default_bundle_filename("blah.js"), "blah.bundle.js");
  assert_eq!(
    get_default_bundle_filename("http://example.com/blah.js"),
    "blah.bundle.js"
  );
  assert_eq!(
    get_default_bundle_filename("http://zombo.com/stuff/"),
    "stuff.bundle.js"
  );
}

pub fn flags_from_vec(
  args: Vec<String>,
) -> (DenoFlags, DenoSubcommand, Vec<String>) {
  let cli_app = create_cli_app();
  let matches = cli_app.get_matches_from(args);
  let mut argv: Vec<String> = vec!["deno".to_string()];
  let mut flags = parse_flags(&matches.clone(), None);

  if flags.version {
    return (flags, DenoSubcommand::Version, argv);
  }

  let subcommand = match matches.subcommand() {
    ("bundle", Some(bundle_match)) => {
      flags.allow_write = true;
      let source_file: &str = bundle_match.value_of("source_file").unwrap();
      let out_file = bundle_match
        .value_of("out_file")
        .map(String::from)
        .unwrap_or_else(|| get_default_bundle_filename(source_file));
      argv.extend(vec![source_file.to_string(), out_file.to_string()]);
      DenoSubcommand::Bundle
    }
    ("completions", Some(completions_match)) => {
      let shell: &str = completions_match.value_of("shell").unwrap();
      let mut buf: Vec<u8> = vec![];
      create_cli_app().gen_completions_to(
        "deno",
        Shell::from_str(shell).unwrap(),
        &mut buf,
      );
      print!("{}", std::str::from_utf8(&buf).unwrap());
      DenoSubcommand::Completions
    }
    ("eval", Some(eval_match)) => {
      flags.allow_net = true;
      flags.allow_env = true;
      flags.allow_run = true;
      flags.allow_read = true;
      flags.allow_write = true;
      flags.allow_hrtime = true;
      let code: &str = eval_match.value_of("code").unwrap();
      argv.extend(vec![code.to_string()]);
      DenoSubcommand::Eval
    }
    ("fetch", Some(fetch_match)) => {
      let file: &str = fetch_match.value_of("file").unwrap();
      argv.extend(vec![file.to_string()]);
      DenoSubcommand::Fetch
    }
    ("fmt", Some(fmt_match)) => {
      flags.allow_read = true;
      flags.allow_write = true;
      argv.push(PRETTIER_URL.to_string());

      let files: Vec<String> = fmt_match
        .values_of("files")
        .unwrap()
        .map(String::from)
        .collect();
      argv.extend(files);

      if !fmt_match.is_present("stdout") {
        // `deno fmt` writes to the files by default
        argv.push("--write".to_string());
      }

      DenoSubcommand::Run
    }
    ("info", Some(info_match)) => {
      if info_match.is_present("file") {
        argv.push(info_match.value_of("file").unwrap().to_string());
      }
      DenoSubcommand::Info
    }
    ("install", Some(install_match)) => {
      flags.allow_read = true;
      flags.allow_write = true;
      flags.allow_net = true;
      flags.allow_env = true;
      flags.allow_run = true;
      argv.push(INSTALLER_URL.to_string());

      if install_match.is_present("dir") {
        let install_dir = install_match.value_of("dir").unwrap();
        argv.push("--dir".to_string());
        argv.push(install_dir.to_string());
      }

      let exe_name: &str = install_match.value_of("exe_name").unwrap();
      argv.push(exe_name.to_string());

      match install_match.subcommand() {
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
    ("test", Some(test_match)) => {
      flags.allow_read = true;
      argv.push(TEST_RUNNER_URL.to_string());

      if test_match.is_present("quiet") {
        argv.push("--quiet".to_string());
      }

      if test_match.is_present("failfast") {
        argv.push("--failfast".to_string());
      }

      if test_match.is_present("exclude") {
        argv.push("--exclude".to_string());
        let exclude: Vec<String> = test_match
          .values_of("exclude")
          .unwrap()
          .map(String::from)
          .collect();
        argv.extend(exclude);
      }

      if test_match.is_present("files") {
        argv.push("--".to_string());
        let files: Vec<String> = test_match
          .values_of("files")
          .unwrap()
          .map(String::from)
          .collect();
        argv.extend(files);
      }

      DenoSubcommand::Run
    }
    ("types", Some(_)) => DenoSubcommand::Types,
    ("run", Some(run_match)) => {
      match run_match.subcommand() {
        (script, Some(script_match)) => {
          argv.extend(vec![script.to_string()]);
          // check if there are any extra arguments that should
          // be passed to script
          if script_match.is_present("") {
            let script_args: Vec<String> = script_match
              .values_of("")
              .unwrap()
              .map(String::from)
              .collect();

            let (script_args, flags_) = parse_script_args(script_args, flags);
            flags = flags_;
            argv.extend(script_args);
          }
          DenoSubcommand::Run
        }
        _ => unreachable!(),
      }
    }
    ("xeval", Some(eval_match)) => {
      flags.allow_net = true;
      flags.allow_env = true;
      flags.allow_run = true;
      flags.allow_read = true;
      flags.allow_write = true;
      flags.allow_hrtime = true;
      let code: &str = eval_match.value_of("code").unwrap();
      flags.xeval_replvar =
        Some(eval_match.value_of("replvar").unwrap_or("$").to_owned());
      // Currently clap never escapes string,
      // So -d "\n" won't expand to newline.
      // Instead, do -d $'\n'
      flags.xeval_delim = eval_match.value_of("delim").map(String::from);
      argv.extend(vec![code.to_string()]);
      DenoSubcommand::Xeval
    }
    (script, Some(script_match)) => {
      argv.extend(vec![script.to_string()]);
      // check if there are any extra arguments that should
      // be passed to script
      if script_match.is_present("") {
        let script_args: Vec<String> = script_match
          .values_of("")
          .unwrap()
          .map(String::from)
          .collect();

        let (script_args, flags_) = parse_script_args(script_args, flags);
        flags = flags_;
        argv.extend(script_args);
      }
      DenoSubcommand::Run
    }
    _ => {
      flags.allow_net = true;
      flags.allow_env = true;
      flags.allow_run = true;
      flags.allow_read = true;
      flags.allow_write = true;
      flags.allow_hrtime = true;
      DenoSubcommand::Repl
    }
  };

  (flags, subcommand, argv)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_flags_from_vec_1() {
    let (flags, subcommand, argv) = flags_from_vec(svec!["deno", "version"]);
    assert_eq!(
      flags,
      DenoFlags {
        version: true,
        ..DenoFlags::default()
      }
    );
    assert_eq!(subcommand, DenoSubcommand::Version);
    assert_eq!(argv, svec!["deno"]);

    let (flags, subcommand, argv) = flags_from_vec(svec!["deno", "--version"]);
    assert_eq!(
      flags,
      DenoFlags {
        version: true,
        ..DenoFlags::default()
      }
    );
    assert_eq!(subcommand, DenoSubcommand::Version);
    assert_eq!(argv, svec!["deno"]);

    let (flags, subcommand, argv) = flags_from_vec(svec!["deno", "-v"]);
    assert_eq!(
      flags,
      DenoFlags {
        version: true,
        ..DenoFlags::default()
      }
    );
    assert_eq!(subcommand, DenoSubcommand::Version);
    assert_eq!(argv, svec!["deno"]);
  }

  #[test]
  fn test_flags_from_vec_2() {
    let (flags, subcommand, argv) =
      flags_from_vec(svec!["deno", "-r", "run", "script.ts"]);
    assert_eq!(
      flags,
      DenoFlags {
        reload: true,
        ..DenoFlags::default()
      }
    );
    assert_eq!(subcommand, DenoSubcommand::Run);
    assert_eq!(argv, svec!["deno", "script.ts"]);
  }

  #[test]
  fn test_flags_from_vec_3() {
    let (flags, subcommand, argv) =
      flags_from_vec(svec!["deno", "run", "-r", "--allow-write", "script.ts"]);
    assert_eq!(
      flags,
      DenoFlags {
        reload: true,
        allow_write: true,
        ..DenoFlags::default()
      }
    );
    assert_eq!(subcommand, DenoSubcommand::Run);
    assert_eq!(argv, svec!["deno", "script.ts"]);
  }

  #[test]
  fn test_flags_from_vec_4() {
    let (flags, subcommand, argv) =
      flags_from_vec(svec!["deno", "-r", "run", "--allow-write", "script.ts"]);
    assert_eq!(
      flags,
      DenoFlags {
        reload: true,
        allow_write: true,
        ..DenoFlags::default()
      }
    );
    assert_eq!(subcommand, DenoSubcommand::Run);
    assert_eq!(argv, svec!["deno", "script.ts"]);
  }

  #[test]
  fn test_flags_from_vec_5() {
    let (flags, subcommand, argv) =
      flags_from_vec(svec!["deno", "--v8-options", "run", "script.ts"]);
    assert_eq!(
      flags,
      DenoFlags {
        v8_flags: Some(svec!["deno", "--help"]),
        ..DenoFlags::default()
      }
    );
    assert_eq!(subcommand, DenoSubcommand::Run);
    assert_eq!(argv, svec!["deno", "script.ts"]);

    let (flags, subcommand, argv) = flags_from_vec(svec![
      "deno",
      "--v8-flags=--expose-gc,--gc-stats=1",
      "run",
      "script.ts"
    ]);
    assert_eq!(
      flags,
      DenoFlags {
        v8_flags: Some(svec!["deno", "--expose-gc", "--gc-stats=1"]),
        ..DenoFlags::default()
      }
    );
    assert_eq!(subcommand, DenoSubcommand::Run);
    assert_eq!(argv, svec!["deno", "script.ts"]);
  }

  #[test]
  fn test_flags_from_vec_6() {
    let (flags, subcommand, argv) = flags_from_vec(svec![
      "deno",
      "run",
      "--allow-net",
      "gist.ts",
      "--title",
      "X"
    ]);
    assert_eq!(
      flags,
      DenoFlags {
        allow_net: true,
        ..DenoFlags::default()
      }
    );
    assert_eq!(subcommand, DenoSubcommand::Run);
    assert_eq!(argv, svec!["deno", "gist.ts", "--title", "X"]);
  }

  #[test]
  fn test_flags_from_vec_7() {
    let (flags, subcommand, argv) =
      flags_from_vec(svec!["deno", "run", "--allow-all", "gist.ts"]);
    assert_eq!(
      flags,
      DenoFlags {
        allow_net: true,
        allow_env: true,
        allow_run: true,
        allow_read: true,
        allow_write: true,
        allow_hrtime: true,
        ..DenoFlags::default()
      }
    );
    assert_eq!(subcommand, DenoSubcommand::Run);
    assert_eq!(argv, svec!["deno", "gist.ts"]);
  }

  #[test]
  fn test_flags_from_vec_8() {
    let (flags, subcommand, argv) =
      flags_from_vec(svec!["deno", "run", "--allow-read", "gist.ts"]);
    assert_eq!(
      flags,
      DenoFlags {
        allow_read: true,
        ..DenoFlags::default()
      }
    );
    assert_eq!(subcommand, DenoSubcommand::Run);
    assert_eq!(argv, svec!["deno", "gist.ts"]);
  }

  #[test]
  fn test_flags_from_vec_9() {
    let (flags, subcommand, argv) =
      flags_from_vec(svec!["deno", "run", "--allow-hrtime", "script.ts"]);
    assert_eq!(
      flags,
      DenoFlags {
        allow_hrtime: true,
        ..DenoFlags::default()
      }
    );
    assert_eq!(subcommand, DenoSubcommand::Run);
    assert_eq!(argv, svec!["deno", "script.ts"]);
  }

  #[test]
  fn test_flags_from_vec_10() {
    // notice that flags passed after double dash will not
    // be parsed to DenoFlags but instead forwarded to
    // script args as Deno.args
    let (flags, subcommand, argv) = flags_from_vec(svec![
      "deno",
      "run",
      "--allow-write",
      "script.ts",
      "--",
      "-D",
      "--allow-net"
    ]);
    assert_eq!(
      flags,
      DenoFlags {
        allow_write: true,
        ..DenoFlags::default()
      }
    );
    assert_eq!(subcommand, DenoSubcommand::Run);
    assert_eq!(argv, svec!["deno", "script.ts", "--", "-D", "--allow-net"]);
  }

  #[test]
  fn test_flags_from_vec_11() {
    let (flags, subcommand, argv) =
      flags_from_vec(svec!["deno", "fmt", "script_1.ts", "script_2.ts"]);
    assert_eq!(
      flags,
      DenoFlags {
        allow_write: true,
        allow_read: true,
        ..DenoFlags::default()
      }
    );
    assert_eq!(subcommand, DenoSubcommand::Run);
    assert_eq!(
      argv,
      svec![
        "deno",
        PRETTIER_URL,
        "script_1.ts",
        "script_2.ts",
        "--write"
      ]
    );
  }

  #[test]
  fn test_flags_from_vec_12() {
    let (flags, subcommand, argv) = flags_from_vec(svec!["deno", "types"]);
    assert_eq!(flags, DenoFlags::default());
    assert_eq!(subcommand, DenoSubcommand::Types);
    assert_eq!(argv, svec!["deno"]);
  }

  #[test]
  fn test_flags_from_vec_13() {
    let (flags, subcommand, argv) =
      flags_from_vec(svec!["deno", "fetch", "script.ts"]);
    assert_eq!(flags, DenoFlags::default());
    assert_eq!(subcommand, DenoSubcommand::Fetch);
    assert_eq!(argv, svec!["deno", "script.ts"]);
  }

  #[test]
  fn test_flags_from_vec_14() {
    let (flags, subcommand, argv) =
      flags_from_vec(svec!["deno", "info", "script.ts"]);
    assert_eq!(flags, DenoFlags::default());
    assert_eq!(subcommand, DenoSubcommand::Info);
    assert_eq!(argv, svec!["deno", "script.ts"]);

    let (flags, subcommand, argv) = flags_from_vec(svec!["deno", "info"]);
    assert_eq!(flags, DenoFlags::default());
    assert_eq!(subcommand, DenoSubcommand::Info);
    assert_eq!(argv, svec!["deno"]);
  }

  #[test]
  fn test_flags_from_vec_15() {
    let (flags, subcommand, argv) =
      flags_from_vec(svec!["deno", "run", "-c", "tsconfig.json", "script.ts"]);
    assert_eq!(
      flags,
      DenoFlags {
        config_path: Some("tsconfig.json".to_owned()),
        ..DenoFlags::default()
      }
    );
    assert_eq!(subcommand, DenoSubcommand::Run);
    assert_eq!(argv, svec!["deno", "script.ts"]);
  }

  #[test]
  fn test_flags_from_vec_16() {
    let (flags, subcommand, argv) =
      flags_from_vec(svec!["deno", "eval", "'console.log(\"hello\")'"]);
    assert_eq!(
      flags,
      DenoFlags {
        allow_net: true,
        allow_env: true,
        allow_run: true,
        allow_read: true,
        allow_write: true,
        allow_hrtime: true,
        ..DenoFlags::default()
      }
    );
    assert_eq!(subcommand, DenoSubcommand::Eval);
    assert_eq!(argv, svec!["deno", "'console.log(\"hello\")'"]);
  }

  #[test]
  fn test_flags_from_vec_17() {
    let (flags, subcommand, argv) = flags_from_vec(svec!["deno"]);
    assert_eq!(
      flags,
      DenoFlags {
        allow_net: true,
        allow_env: true,
        allow_run: true,
        allow_read: true,
        allow_write: true,
        allow_hrtime: true,
        ..DenoFlags::default()
      }
    );
    assert_eq!(subcommand, DenoSubcommand::Repl);
    assert_eq!(argv, svec!["deno"]);
  }

  #[test]
  fn test_flags_from_vec_18() {
    let (flags, subcommand, argv) = flags_from_vec(svec![
      "deno",
      "xeval",
      "-I",
      "val",
      "-d",
      " ",
      "console.log(val)"
    ]);
    assert_eq!(
      flags,
      DenoFlags {
        allow_net: true,
        allow_env: true,
        allow_run: true,
        allow_read: true,
        allow_write: true,
        allow_hrtime: true,
        xeval_replvar: Some("val".to_owned()),
        xeval_delim: Some(" ".to_owned()),
        ..DenoFlags::default()
      }
    );
    assert_eq!(subcommand, DenoSubcommand::Xeval);
    assert_eq!(argv, svec!["deno", "console.log(val)"]);
  }

  #[test]
  fn test_flags_from_vec_19() {
    use tempfile::TempDir;
    let temp_dir = TempDir::new().expect("tempdir fail");
    let (_, temp_dir_path) =
      deno_fs::resolve_from_cwd(temp_dir.path().to_str().unwrap()).unwrap();

    let (flags, subcommand, argv) = flags_from_vec(svec![
      "deno",
      "run",
      format!("--allow-read={}", &temp_dir_path),
      "script.ts"
    ]);
    assert_eq!(
      flags,
      DenoFlags {
        allow_read: false,
        read_whitelist: svec![&temp_dir_path],
        ..DenoFlags::default()
      }
    );
    assert_eq!(subcommand, DenoSubcommand::Run);
    assert_eq!(argv, svec!["deno", "script.ts"]);
  }

  #[test]
  fn test_flags_from_vec_20() {
    use tempfile::TempDir;
    let temp_dir = TempDir::new().expect("tempdir fail");
    let (_, temp_dir_path) =
      deno_fs::resolve_from_cwd(temp_dir.path().to_str().unwrap()).unwrap();

    let (flags, subcommand, argv) = flags_from_vec(svec![
      "deno",
      "run",
      format!("--allow-write={}", &temp_dir_path),
      "script.ts"
    ]);
    assert_eq!(
      flags,
      DenoFlags {
        allow_write: false,
        write_whitelist: svec![&temp_dir_path],
        ..DenoFlags::default()
      }
    );
    assert_eq!(subcommand, DenoSubcommand::Run);
    assert_eq!(argv, svec!["deno", "script.ts"]);
  }

  #[test]
  fn test_flags_from_vec_21() {
    let (flags, subcommand, argv) = flags_from_vec(svec![
      "deno",
      "run",
      "--allow-net=127.0.0.1",
      "script.ts"
    ]);
    assert_eq!(
      flags,
      DenoFlags {
        allow_net: false,
        net_whitelist: svec!["127.0.0.1"],
        ..DenoFlags::default()
      }
    );
    assert_eq!(subcommand, DenoSubcommand::Run);
    assert_eq!(argv, svec!["deno", "script.ts"]);
  }

  #[test]
  fn test_flags_from_vec_22() {
    let (flags, subcommand, argv) = flags_from_vec(svec![
      "deno",
      "fmt",
      "--stdout",
      "script_1.ts",
      "script_2.ts"
    ]);
    assert_eq!(
      flags,
      DenoFlags {
        allow_write: true,
        allow_read: true,
        ..DenoFlags::default()
      }
    );
    assert_eq!(subcommand, DenoSubcommand::Run);
    assert_eq!(
      argv,
      svec!["deno", PRETTIER_URL, "script_1.ts", "script_2.ts"]
    );
  }

  #[test]
  fn test_flags_from_vec_23() {
    let (flags, subcommand, argv) = flags_from_vec(svec!["deno", "script.ts"]);
    assert_eq!(flags, DenoFlags::default());
    assert_eq!(subcommand, DenoSubcommand::Run);
    assert_eq!(argv, svec!["deno", "script.ts"]);
  }

  #[test]
  fn test_flags_from_vec_24() {
    let (flags, subcommand, argv) =
      flags_from_vec(svec!["deno", "--allow-net", "--allow-read", "script.ts"]);
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
  }

  #[test]
  fn test_flags_from_vec_25() {
    let (flags, subcommand, argv) = flags_from_vec(svec![
      "deno",
      "-r",
      "--allow-net",
      "run",
      "--allow-read",
      "script.ts"
    ]);
    assert_eq!(
      flags,
      DenoFlags {
        reload: true,
        allow_net: true,
        allow_read: true,
        ..DenoFlags::default()
      }
    );
    assert_eq!(subcommand, DenoSubcommand::Run);
    assert_eq!(argv, svec!["deno", "script.ts"]);
  }

  #[test]
  fn test_flags_from_vec_26() {
    let (flags, subcommand, argv) =
      flags_from_vec(svec!["deno", "bundle", "source.ts", "bundle.js"]);
    assert_eq!(
      flags,
      DenoFlags {
        allow_write: true,
        ..DenoFlags::default()
      }
    );
    assert_eq!(subcommand, DenoSubcommand::Bundle);
    assert_eq!(argv, svec!["deno", "source.ts", "bundle.js"])
  }

  #[test]
  fn test_flags_from_vec_27() {
    let (flags, subcommand, argv) = flags_from_vec(svec![
      "deno",
      "run",
      "--importmap=importmap.json",
      "script.ts"
    ]);
    assert_eq!(
      flags,
      DenoFlags {
        import_map_path: Some("importmap.json".to_owned()),
        ..DenoFlags::default()
      }
    );
    assert_eq!(subcommand, DenoSubcommand::Run);
    assert_eq!(argv, svec!["deno", "script.ts"]);

    let (flags, subcommand, argv) =
      flags_from_vec(svec!["deno", "--importmap=importmap.json", "script.ts"]);
    assert_eq!(
      flags,
      DenoFlags {
        import_map_path: Some("importmap.json".to_owned()),
        ..DenoFlags::default()
      }
    );
    assert_eq!(subcommand, DenoSubcommand::Run);
    assert_eq!(argv, svec!["deno", "script.ts"]);

    let (flags, subcommand, argv) = flags_from_vec(svec![
      "deno",
      "fetch",
      "--importmap=importmap.json",
      "script.ts"
    ]);
    assert_eq!(
      flags,
      DenoFlags {
        import_map_path: Some("importmap.json".to_owned()),
        ..DenoFlags::default()
      }
    );
    assert_eq!(subcommand, DenoSubcommand::Fetch);
    assert_eq!(argv, svec!["deno", "script.ts"]);
  }

  #[test]
  fn test_flags_from_vec_28() {
    let (flags, subcommand, argv) =
      flags_from_vec(svec!["deno", "--seed", "250", "run", "script.ts"]);
    assert_eq!(
      flags,
      DenoFlags {
        seed: Some(250 as u64),
        v8_flags: Some(svec!["deno", "--random-seed=250"]),
        ..DenoFlags::default()
      }
    );
    assert_eq!(subcommand, DenoSubcommand::Run);
    assert_eq!(argv, svec!["deno", "script.ts"]);
  }

  #[test]
  fn test_flags_from_vec_29() {
    let (flags, subcommand, argv) = flags_from_vec(svec![
      "deno",
      "--seed",
      "250",
      "--v8-flags=--expose-gc",
      "run",
      "script.ts"
    ]);
    assert_eq!(
      flags,
      DenoFlags {
        seed: Some(250 as u64),
        v8_flags: Some(svec!["deno", "--expose-gc", "--random-seed=250"]),
        ..DenoFlags::default()
      }
    );
    assert_eq!(subcommand, DenoSubcommand::Run);
    assert_eq!(argv, svec!["deno", "script.ts"]);
  }

  #[test]
  fn test_flags_from_vec_30() {
    let (flags, subcommand, argv) = flags_from_vec(svec![
      "deno",
      "install",
      "deno_colors",
      "https://deno.land/std/examples/colors.ts"
    ]);
    assert_eq!(
      flags,
      DenoFlags {
        allow_write: true,
        allow_net: true,
        allow_read: true,
        allow_env: true,
        allow_run: true,
        ..DenoFlags::default()
      }
    );
    assert_eq!(subcommand, DenoSubcommand::Run);
    assert_eq!(
      argv,
      svec![
        "deno",
        INSTALLER_URL,
        "deno_colors",
        "https://deno.land/std/examples/colors.ts"
      ]
    );

    let (flags, subcommand, argv) = flags_from_vec(svec![
      "deno",
      "install",
      "file_server",
      "https://deno.land/std/http/file_server.ts",
      "--allow-net",
      "--allow-read"
    ]);
    assert_eq!(
      flags,
      DenoFlags {
        allow_write: true,
        allow_net: true,
        allow_read: true,
        allow_env: true,
        allow_run: true,
        ..DenoFlags::default()
      }
    );
    assert_eq!(subcommand, DenoSubcommand::Run);
    assert_eq!(
      argv,
      svec![
        "deno",
        INSTALLER_URL,
        "file_server",
        "https://deno.land/std/http/file_server.ts",
        "--allow-net",
        "--allow-read"
      ]
    );

    let (flags, subcommand, argv) = flags_from_vec(svec![
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
      flags,
      DenoFlags {
        allow_write: true,
        allow_net: true,
        allow_read: true,
        allow_env: true,
        allow_run: true,
        ..DenoFlags::default()
      }
    );
    assert_eq!(subcommand, DenoSubcommand::Run);
    assert_eq!(
      argv,
      svec![
        "deno",
        INSTALLER_URL,
        "--dir",
        "/usr/local/bin",
        "file_server",
        "https://deno.land/std/http/file_server.ts",
        "--allow-net",
        "--allow-read"
      ]
    );
  }

  #[test]
  fn test_flags_from_vec_31() {
    let (flags, subcommand, argv) =
      flags_from_vec(svec!["deno", "--log-level=debug", "script.ts"]);
    assert_eq!(
      flags,
      DenoFlags {
        log_level: Some(Level::Debug),
        ..DenoFlags::default()
      }
    );
    assert_eq!(subcommand, DenoSubcommand::Run);
    assert_eq!(argv, svec!["deno", "script.ts"])
  }

  #[test]
  fn test_flags_from_vec_32() {
    let (flags, subcommand, argv) =
      flags_from_vec(svec!["deno", "completions", "bash"]);
    assert_eq!(flags, DenoFlags::default());
    assert_eq!(subcommand, DenoSubcommand::Completions);
    assert_eq!(argv, svec!["deno"])
  }

  #[test]
  fn test_flags_from_vec_33() {
    let (flags, subcommand, argv) =
      flags_from_vec(svec!["deno", "script.ts", "--allow-read", "--allow-net"]);
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

    let (flags, subcommand, argv) = flags_from_vec(svec![
      "deno",
      "--allow-read",
      "run",
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
      flags_from_vec(svec!["deno", "script.ts", "foo", "bar"]);
    assert_eq!(flags, DenoFlags::default());
    assert_eq!(subcommand, DenoSubcommand::Run);
    assert_eq!(argv, svec!["deno", "script.ts", "foo", "bar"]);

    let (flags, subcommand, argv) =
      flags_from_vec(svec!["deno", "script.ts", "-"]);
    assert_eq!(flags, DenoFlags::default());
    assert_eq!(subcommand, DenoSubcommand::Run);
    assert_eq!(argv, svec!["deno", "script.ts", "-"]);

    let (flags, subcommand, argv) =
      flags_from_vec(svec!["deno", "script.ts", "-", "foo", "bar"]);
    assert_eq!(flags, DenoFlags::default());
    assert_eq!(subcommand, DenoSubcommand::Run);
    assert_eq!(argv, svec!["deno", "script.ts", "-", "foo", "bar"]);
  }

  #[test]
  fn test_flags_from_vec_34() {
    let (flags, subcommand, argv) =
      flags_from_vec(svec!["deno", "--no-fetch", "script.ts"]);
    assert_eq!(
      flags,
      DenoFlags {
        no_fetch: true,
        ..DenoFlags::default()
      }
    );
    assert_eq!(subcommand, DenoSubcommand::Run);
    assert_eq!(argv, svec!["deno", "script.ts"])
  }

  #[test]
  fn test_flags_from_vec_35() {
    let (flags, subcommand, argv) =
      flags_from_vec(svec!["deno", "--current-thread", "script.ts"]);
    assert_eq!(
      flags,
      DenoFlags {
        current_thread: true,
        ..DenoFlags::default()
      }
    );
    assert_eq!(subcommand, DenoSubcommand::Run);
    assert_eq!(argv, svec!["deno", "script.ts"])
  }

  #[test]
  fn test_flags_from_vec_36() {
    let (flags, subcommand, argv) = flags_from_vec(svec![
      "deno",
      "test",
      "--exclude",
      "some_dir/",
      "**/*_test.ts"
    ]);
    assert_eq!(
      flags,
      DenoFlags {
        allow_read: true,
        ..DenoFlags::default()
      }
    );
    assert_eq!(subcommand, DenoSubcommand::Run);
    assert_eq!(
      argv,
      svec![
        "deno",
        TEST_RUNNER_URL,
        "--exclude",
        "some_dir/",
        "**/*_test.ts"
      ]
    )
  }
}
