// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use clap::{App, AppSettings, Arg, ArgMatches, SubCommand};
use crate::deno_dir;

// Creates vector of strings, Vec<String>
macro_rules! svec {
    ($($x:expr),*) => (vec![$($x.to_string()),*]);
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct DenoFlags {
  pub log_debug: bool,
  pub version: bool,
  pub reload: bool,
  /// When the `--config`/`-c` flag is used to pass the name, this will be set
  /// the path passed on the command line, otherwise `None`.
  pub config_path: Option<String>,
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
  pub v8_flags: Option<Vec<String>>,
  pub xeval_replvar: Option<String>,
  pub xeval_delim: Option<String>,
}

static ENV_VARIABLES_HELP: &str = "ENVIRONMENT VARIABLES:
    DENO_DIR        Set deno's base directory
    NO_COLOR        Set to disable color";

pub fn create_cli_app<'a, 'b>() -> App<'a, 'b> {
  App::new("deno")
    .bin_name("deno")
    .global_settings(&[AppSettings::ColorNever])
    .settings(&[AppSettings::DisableVersion])
    .after_help(ENV_VARIABLES_HELP)
    .long_about("A secure runtime for JavaScript and TypeScript built with V8, Rust, and Tokio.

Docs: https://deno.land/manual.html
Modules: https://github.com/denoland/deno_std
Bugs: https://github.com/denoland/deno/issues

To run the REPL:

  deno

To execute a sandboxed script:

  deno run https://deno.land/welcome.ts

To evaluate code from the command line:

  deno eval \"console.log(30933 + 404)\"

To get help on the another subcommands (run in this case):

  deno help run")
    .arg(
      Arg::with_name("log-debug")
        .short("D")
        .long("log-debug")
        .help("Log debug output")
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
        .setting(AppSettings::DisableVersion)
        .about("Print the version")
        .long_about("Print current version of Deno.

Includes versions of Deno, V8 JavaScript Engine, and the TypeScript
compiler.",
        ),
    ).subcommand(
      SubCommand::with_name("fetch")
        .setting(AppSettings::DisableVersion)
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
        .setting(AppSettings::DisableVersion)
        .about("Print runtime TypeScript declarations")
        .long_about("Print runtime TypeScript declarations.

  deno types > lib.deno_runtime.d.ts

The declaration file could be saved and used for typing information.",
        ),
    ).subcommand(
      SubCommand::with_name("info")
        .setting(AppSettings::DisableVersion)
        .about("Show source file related info")
        .long_about("Show source file related info.

  deno info https://deno.land/std@v0.6/http/file_server.ts

The following information is shown:

  local:    Local path of the file.
  type:     JavaScript, TypeScript, or JSON.
  compiled: TypeScript only. shown local path of compiled source code.
  map:      TypeScript only. shown local path of source map.
  deps:     Dependency tree of the source file.",
        ).arg(Arg::with_name("file").takes_value(true).required(true)),
    ).subcommand(
      SubCommand::with_name("eval")
        .setting(AppSettings::DisableVersion)
        .about("Eval script")
        .long_about(
          "Evaluate provided script.

This command has implicit access to all permissions (equivalent to deno run --allow-all)

  deno eval \"console.log('hello world')\"",
        ).arg(Arg::with_name("code").takes_value(true).required(true)),
    ).subcommand(
      SubCommand::with_name("fmt")
        .setting(AppSettings::DisableVersion)
        .about("Format files")
        .long_about(
"Auto-format JavaScript/TypeScript source code using Prettier

Automatically downloads Prettier dependencies on first run.

  deno fmt --write myfile1.ts myfile2.ts",
        ).arg(
          Arg::with_name("files")
            .takes_value(true)
            .multiple(true)
            .required(true),
        ),
    ).subcommand(
      SubCommand::with_name("run")
        .settings(&[
          AppSettings::AllowExternalSubcommands,
          AppSettings::DisableHelpSubcommand,
          AppSettings::DisableVersion,
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
        ).arg(
          Arg::with_name("allow-read")
        .long("allow-read")
        .min_values(0)
        .takes_value(true)
        .use_delimiter(true)
        .require_equals(true)
        .help("Allow file system read access"),
    ).arg(
      Arg::with_name("allow-write")
        .long("allow-write")
        .min_values(0)
        .takes_value(true)
        .use_delimiter(true)
        .require_equals(true)
        .help("Allow file system write access"),
    ).arg(
      Arg::with_name("allow-net")
        .long("allow-net")
        .min_values(0)
        .takes_value(true)
        .use_delimiter(true)
        .require_equals(true)
        .help("Allow network access"),
    ).arg(
          Arg::with_name("allow-env")
            .long("allow-env")
            .help("Allow environment access"),
        ).arg(
          Arg::with_name("allow-run")
            .long("allow-run")
            .help("Allow running subprocesses"),
        ).arg(
          Arg::with_name("allow-hrtime")
            .long("allow-hrtime")
            .help("Allow high resolution time measurement"),
        ).arg(
          Arg::with_name("allow-all")
            .short("A")
            .long("allow-all")
            .help("Allow all permissions"),
        ).arg(
          Arg::with_name("no-prompt")
            .long("no-prompt")
            .help("Do not use prompts"),
        ).subcommand(
          // this is a fake subcommand - it's used in conjunction with
          // AppSettings:AllowExternalSubcommand to treat it as an
          // entry point script
          SubCommand::with_name("<script>").about("Script to run"),
        ),
    ).subcommand(
    SubCommand::with_name("xeval")
        .setting(AppSettings::DisableVersion)
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
    )
}

/// Convert paths supplied into full path.
/// If a path is invalid, we print out a warning
/// and ignore this path in the output.
fn resolve_paths(paths: Vec<String>) -> Vec<String> {
  let mut out: Vec<String> = vec![];
  for pathstr in paths.iter() {
    let result = deno_dir::resolve_path(pathstr);
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
pub fn parse_flags(matches: ArgMatches) -> DenoFlags {
  let mut flags = DenoFlags::default();

  if matches.is_present("log-debug") {
    flags.log_debug = true;
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

  // flags specific to "run" subcommand
  if let Some(run_matches) = matches.subcommand_matches("run") {
    if run_matches.is_present("allow-read") {
      if run_matches.value_of("allow-read").is_some() {
        let read_wl = run_matches.values_of("allow-read").unwrap();
        let raw_read_whitelist: Vec<String> =
          read_wl.map(std::string::ToString::to_string).collect();
        flags.read_whitelist = resolve_paths(raw_read_whitelist);
        debug!("read whitelist: {:#?}", &flags.read_whitelist);
      } else {
        flags.allow_read = true;
      }
    }
    if run_matches.is_present("allow-write") {
      if run_matches.value_of("allow-write").is_some() {
        let write_wl = run_matches.values_of("allow-write").unwrap();
        let raw_write_whitelist =
          write_wl.map(std::string::ToString::to_string).collect();
        flags.write_whitelist = resolve_paths(raw_write_whitelist);
        debug!("write whitelist: {:#?}", &flags.write_whitelist);
      } else {
        flags.allow_write = true;
      }
    }
    if run_matches.is_present("allow-net") {
      if run_matches.value_of("allow-net").is_some() {
        let net_wl = run_matches.values_of("allow-net").unwrap();
        flags.net_whitelist =
          net_wl.map(std::string::ToString::to_string).collect();
        debug!("net whitelist: {:#?}", &flags.net_whitelist);
      } else {
        flags.allow_net = true;
      }
    }
    if run_matches.is_present("allow-env") {
      flags.allow_env = true;
    }
    if run_matches.is_present("allow-run") {
      flags.allow_run = true;
    }
    if run_matches.is_present("allow-hrtime") {
      flags.allow_hrtime = true;
    }
    if run_matches.is_present("allow-all") {
      flags.allow_read = true;
      flags.allow_env = true;
      flags.allow_net = true;
      flags.allow_run = true;
      flags.allow_read = true;
      flags.allow_write = true;
      flags.allow_hrtime = true;
    }
    if run_matches.is_present("no-prompt") {
      flags.no_prompts = true;
    }
  }

  flags
}

/// Used for `deno fmt <files>...` subcommand
const PRETTIER_URL: &str = "https://deno.land/std@v0.5.0/prettier/main.ts";

/// These are currently handled subcommands.
/// There is no "Help" subcommand because it's handled by `clap::App` itself.
#[derive(Debug, PartialEq)]
pub enum DenoSubcommand {
  Eval,
  Fetch,
  Info,
  Repl,
  Run,
  Types,
  Version,
  Xeval,
}

pub fn flags_from_vec(
  args: Vec<String>,
) -> (DenoFlags, DenoSubcommand, Vec<String>) {
  let cli_app = create_cli_app();
  let matches = cli_app.get_matches_from(args);
  let mut argv: Vec<String> = vec!["deno".to_string()];
  let mut flags = parse_flags(matches.clone());

  let subcommand = match matches.subcommand() {
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

      // `deno fmt` writes to the files by default
      argv.push("--write".to_string());

      DenoSubcommand::Run
    }
    ("info", Some(info_match)) => {
      let file: &str = info_match.value_of("file").unwrap();
      argv.extend(vec![file.to_string()]);
      DenoSubcommand::Info
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
    ("version", Some(_)) => DenoSubcommand::Version,
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
  }

  #[test]
  fn test_flags_from_vec_2() {
    let (flags, subcommand, argv) =
      flags_from_vec(svec!["deno", "-r", "-D", "run", "script.ts"]);
    assert_eq!(
      flags,
      DenoFlags {
        log_debug: true,
        reload: true,
        ..DenoFlags::default()
      }
    );
    assert_eq!(subcommand, DenoSubcommand::Run);
    assert_eq!(argv, svec!["deno", "script.ts"]);
  }

  #[test]
  fn test_flags_from_vec_3() {
    let (flags, subcommand, argv) = flags_from_vec(svec![
      "deno",
      "run",
      "-r",
      "-D",
      "--allow-write",
      "script.ts"
    ]);
    assert_eq!(
      flags,
      DenoFlags {
        reload: true,
        log_debug: true,
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
      flags_from_vec(svec!["deno", "-Dr", "run", "--allow-write", "script.ts"]);
    assert_eq!(
      flags,
      DenoFlags {
        log_debug: true,
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
    // notice that flags passed after script name will not
    // be parsed to DenoFlags but instead forwarded to
    // script args as Deno.args
    let (flags, subcommand, argv) = flags_from_vec(svec![
      "deno",
      "run",
      "--allow-write",
      "script.ts",
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
    assert_eq!(argv, svec!["deno", "script.ts", "-D", "--allow-net"]);
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
      deno_dir::resolve_path(temp_dir.path().to_str().unwrap()).unwrap();

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
      deno_dir::resolve_path(temp_dir.path().to_str().unwrap()).unwrap();

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
}
