// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use clap::{App, AppSettings, Arg, ArgMatches, SubCommand};

// Creates vector of strings, Vec<String>
#[cfg(test)]
macro_rules! svec {
    ($($x:expr),*) => (vec![$($x.to_string()),*]);
}

#[cfg_attr(feature = "cargo-clippy", allow(stutter))]
#[derive(Clone, Debug, PartialEq, Default)]
pub struct DenoFlags {
  pub log_debug: bool,
  pub version: bool,
  pub reload: bool,
  pub allow_read: bool,
  pub allow_write: bool,
  pub allow_net: bool,
  pub allow_env: bool,
  pub allow_run: bool,
  pub allow_high_precision: bool,
  pub no_prompts: bool,
  pub v8_help: bool,
  pub v8_flags: Option<Vec<String>>,
}

static ENV_VARIABLES_HELP: &str = "ENVIRONMENT VARIABLES:
    DENO_DIR        Set deno's base directory
    NO_COLOR        Set to disable color";

pub fn create_cli_app<'a, 'b>() -> App<'a, 'b> {
  App::new("deno")
    .bin_name("deno")
    .global_settings(&[AppSettings::ColorNever])
    .settings(&[
      AppSettings::AllowExternalSubcommands,
      AppSettings::DisableVersion,
    ]).after_help(ENV_VARIABLES_HELP)
    .arg(
      Arg::with_name("allow-read")
        .long("allow-read")
        .help("Allow file system read access"),
    ).arg(
      Arg::with_name("allow-write")
        .long("allow-write")
        .help("Allow file system write access"),
    ).arg(
      Arg::with_name("allow-net")
        .long("allow-net")
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
      Arg::with_name("allow-high-precision")
        .long("allow-high-precision")
        .help("Allow high precision time measurement"),
    ).arg(
      Arg::with_name("allow-all")
        .short("A")
        .long("allow-all")
        .help("Allow all permissions"),
    ).arg(
      Arg::with_name("no-prompt")
        .long("no-prompt")
        .help("Do not use prompts"),
    ).arg(
      Arg::with_name("log-debug")
        .short("D")
        .long("log-debug")
        .help("Log debug output"),
    ).arg(
      Arg::with_name("reload")
        .short("r")
        .long("reload")
        .help("Reload source code cache (recompile TypeScript)"),
    ).arg(
      Arg::with_name("v8-options")
        .long("v8-options")
        .help("Print V8 command line options"),
    ).arg(
      Arg::with_name("v8-flags")
        .long("v8-flags")
        .takes_value(true)
        .use_delimiter(true)
        .require_equals(true)
        .help("Set V8 command line options"),
    ).subcommand(
      SubCommand::with_name("version")
        .setting(AppSettings::DisableVersion)
        .about("Print the version"),
    ).subcommand(
      SubCommand::with_name("fetch")
        .setting(AppSettings::DisableVersion)
        .about("Fetch the dependencies")
        .arg(Arg::with_name("file").takes_value(true).required(true)),
    ).subcommand(
      SubCommand::with_name("types")
        .setting(AppSettings::DisableVersion)
        .about("Print runtime TypeScript declarations"),
    ).subcommand(
      SubCommand::with_name("info")
        .setting(AppSettings::DisableVersion)
        .about("Show source file related info")
        .arg(Arg::with_name("file").takes_value(true).required(true)),
    ).subcommand(
      SubCommand::with_name("eval")
        .setting(AppSettings::DisableVersion)
        .about("Eval script")
        .arg(Arg::with_name("code").takes_value(true).required(true)),
    ).subcommand(
      SubCommand::with_name("fmt")
        .setting(AppSettings::DisableVersion)
        .about("Format files")
        .arg(
          Arg::with_name("files")
            .takes_value(true)
            .multiple(true)
            .required(true),
        ),
    ).subcommand(
      // this is a fake subcommand - it's used in conjunction with
      // AppSettings:AllowExternalSubcommand to treat it as an
      // entry point script
      SubCommand::with_name("<script>").about("Script to run"),
    )
}

/// Parse ArgMatches into internal DenoFlags structure.
/// This method should not make any side effects.
#[cfg_attr(feature = "cargo-clippy", allow(stutter))]
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
  if matches.is_present("allow-read") {
    flags.allow_read = true;
  }
  if matches.is_present("allow-write") {
    flags.allow_write = true;
  }
  if matches.is_present("allow-net") {
    flags.allow_net = true;
  }
  if matches.is_present("allow-env") {
    flags.allow_env = true;
  }
  if matches.is_present("allow-run") {
    flags.allow_run = true;
  }
  if matches.is_present("allow-high-precision") {
    flags.allow_high_precision = true;
  }
  if matches.is_present("allow-all") {
    flags.allow_read = true;
    flags.allow_env = true;
    flags.allow_net = true;
    flags.allow_run = true;
    flags.allow_read = true;
    flags.allow_write = true;
    flags.allow_high_precision = true;
  }
  if matches.is_present("no-prompt") {
    flags.no_prompts = true;
  }
  if matches.is_present("v8-options") {
    flags.v8_help = true;
  }
  if matches.is_present("v8-flags") {
    let v8_flags: Vec<String> = matches
      .values_of("v8-flags")
      .unwrap()
      .map(String::from)
      .collect();

    flags.v8_flags = Some(v8_flags);
  }

  flags
}

#[cfg(test)]
mod tests {
  use super::*;

  fn flags_from_vec(args: Vec<String>) -> DenoFlags {
    let cli_app = create_cli_app();
    let matches = cli_app.get_matches_from(args);
    parse_flags(matches)
  }

  #[test]
  fn test_set_flags_1() {
    let flags = flags_from_vec(svec!["deno", "version"]);
    assert_eq!(
      flags,
      DenoFlags {
        version: true,
        ..DenoFlags::default()
      }
    );
  }

  #[test]
  fn test_set_flags_2() {
    let flags = flags_from_vec(svec!["deno", "-r", "-D", "script.ts"]);
    assert_eq!(
      flags,
      DenoFlags {
        log_debug: true,
        reload: true,
        ..DenoFlags::default()
      }
    );
  }

  #[test]
  fn test_set_flags_3() {
    let flags =
      flags_from_vec(svec!["deno", "-r", "--allow-write", "script.ts"]);
    assert_eq!(
      flags,
      DenoFlags {
        reload: true,
        allow_write: true,
        ..DenoFlags::default()
      }
    );
  }

  #[test]
  fn test_set_flags_4() {
    let flags =
      flags_from_vec(svec!["deno", "-Dr", "--allow-write", "script.ts"]);
    assert_eq!(
      flags,
      DenoFlags {
        log_debug: true,
        reload: true,
        allow_write: true,
        ..DenoFlags::default()
      }
    );
  }

  #[test]
  fn test_set_flags_5() {
    let flags = flags_from_vec(svec!["deno", "--v8-options"]);
    assert_eq!(
      flags,
      DenoFlags {
        v8_help: true,
        ..DenoFlags::default()
      }
    );

    let flags =
      flags_from_vec(svec!["deno", "--v8-flags=--expose-gc,--gc-stats=1"]);
    assert_eq!(
      flags,
      DenoFlags {
        v8_flags: Some(svec!["--expose-gc", "--gc-stats=1"]),
        ..DenoFlags::default()
      }
    );
  }

  #[test]
  fn test_set_flags_6() {
    let flags =
      flags_from_vec(svec!["deno", "--allow-net", "gist.ts", "--title", "X"]);
    assert_eq!(
      flags,
      DenoFlags {
        allow_net: true,
        ..DenoFlags::default()
      }
    )
  }

  #[test]
  fn test_set_flags_7() {
    let flags = flags_from_vec(svec!["deno", "--allow-all", "gist.ts"]);
    assert_eq!(
      flags,
      DenoFlags {
        allow_net: true,
        allow_env: true,
        allow_run: true,
        allow_read: true,
        allow_write: true,
        allow_high_precision: true,
        ..DenoFlags::default()
      }
    )
  }

  #[test]
  fn test_set_flags_8() {
    let flags = flags_from_vec(svec!["deno", "--allow-read", "gist.ts"]);
    assert_eq!(
      flags,
      DenoFlags {
        allow_read: true,
        ..DenoFlags::default()
      }
    )
  }

  #[test]
  fn test_set_flags_9() {
    let flags =
      flags_from_vec(svec!["deno", "--allow-high-precision", "script.ts"]);
    assert_eq!(
      flags,
      DenoFlags {
        allow_high_precision: true,
        ..DenoFlags::default()
      }
    )
  }

  #[test]
  fn test_set_flags_10() {
    // notice that flags passed after script name will not
    // be parsed to DenoFlags but instead forwarded to
    // script args as Deno.args
    let flags = flags_from_vec(svec![
      "deno",
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
    )
  }
}
