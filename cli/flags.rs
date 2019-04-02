// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use clap::{App, AppSettings, Arg, ArgMatches};
use crate::ansi;
use deno::v8_set_flags;

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
  pub no_prompts: bool,
  pub types: bool,
  pub prefetch: bool,
  pub info: bool,
  pub fmt: bool,
}

/// Checks provided arguments for known options and sets appropriate Deno flags
/// for them. Unknown options are returned for further use.
/// Note:
///
/// 1. This assumes that privileged flags do not accept parameters deno --foo bar.
/// This assumption is currently valid. But if it were to change in the future,
/// this parsing technique would need to be modified. I think we want to keep the
/// privileged flags minimal - so having this restriction is maybe a good thing.
///
/// 2. Misspelled flags will be forwarded to user code - e.g. --allow-ne would
/// not cause an error. I also think this is ok because missing any of the
/// privileged flags is not destructive. Userland flag parsing would catch these
/// errors.
fn set_recognized_flags(matches: ArgMatches, flags: &mut DenoFlags) {
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
  if matches.is_present("allow-all") {
    flags.allow_read = true;
    flags.allow_env = true;
    flags.allow_net = true;
    flags.allow_run = true;
    flags.allow_read = true;
    flags.allow_write = true;
  }
  if matches.is_present("no-prompt") {
    flags.no_prompts = true;
  }
  if matches.is_present("types") {
    flags.types = true;
  }
  if matches.is_present("prefetch") {
    flags.prefetch = true;
  }
  if matches.is_present("info") {
    flags.info = true;
  }
  if matches.is_present("fmt") {
    flags.fmt = true;
  }
}

#[cfg_attr(feature = "cargo-clippy", allow(stutter))]
pub fn set_flags(
  args: Vec<String>,
) -> Result<(DenoFlags, Vec<String>), String> {
  // TODO: all flags passed after "--" are swallowed by v8_set_flags
  // eg. deno --allow-net ./test.ts -- --title foobar
  // args === ["deno", "--allow-net" "./test.ts"]
  let args = v8_set_flags(args);

  let mut app_settings: Vec<AppSettings> = vec![];

  if ansi::use_color() {
    app_settings.extend(vec![AppSettings::ColorAuto, AppSettings::ColoredHelp]);
  } else {
    app_settings.extend(vec![AppSettings::ColorNever]);
  }

  let clap_app = App::new("deno")
    .global_settings(&app_settings[..])
//    .arg(
//      Arg::with_name("version")
//        .short("v")
//        .long("version")
//        .help("Print the version"),
//    )
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
      Arg::with_name("types")
        .long("types")
        .help("Print runtime TypeScript declarations"),
    ).arg(
      Arg::with_name("prefetch")
        .long("prefetch")
        .help("Prefetch the dependencies"),
    ).arg(
      Arg::with_name("info")
        .long("info")
        .help("Show source file related info"),
    ).arg(Arg::with_name("fmt").long("fmt").help("Format code"))
    .arg(Arg::with_name("entry_point").required(false).index(1))
    .arg(
      Arg::with_name("rest")
        .required(false)
        .multiple(true)
        .index(2),
    );

  let matches = clap_app.get_matches_from(args);

  // TODO(bartomieju): compatibility with old "opts" approach - to be refactored
  let mut rest: Vec<String> = vec![String::from("deno")];

  if matches.is_present("entry_point") {
    let main_module = matches.value_of("entry_point").unwrap().to_string();
    rest.extend(vec![main_module]);
  }

  if matches.is_present("rest") {
    let vals: Vec<String> = matches
      .values_of("rest")
      .unwrap()
      .map(String::from)
      .collect();
    rest.extend(vals);
  }
  // TODO: end
  let mut flags = DenoFlags::default();
  set_recognized_flags(matches, &mut flags);
  Ok((flags, rest))
}

#[test]
fn test_set_flags_1() {
  let (flags, rest) = set_flags(svec!["deno", "--version"]).unwrap();
  assert_eq!(rest, svec!["deno"]);
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
  let (flags, rest) =
    set_flags(svec!["deno", "-r", "-D", "script.ts"]).unwrap();
  assert_eq!(rest, svec!["deno", "script.ts"]);
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
  let (flags, rest) =
    set_flags(svec!["deno", "-r", "script.ts", "--allow-write"]).unwrap();
  assert_eq!(rest, svec!["deno", "script.ts"]);
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
  let (flags, rest) =
    set_flags(svec!["deno", "-Dr", "script.ts", "--allow-write"]).unwrap();
  assert_eq!(rest, svec!["deno", "script.ts"]);
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
  let (flags, rest) = set_flags(svec!["deno", "--types"]).unwrap();
  assert_eq!(rest, svec!["deno"]);
  assert_eq!(
    flags,
    DenoFlags {
      types: true,
      ..DenoFlags::default()
    }
  )
}

#[test]
fn test_set_flags_6() {
  let (flags, rest) =
    set_flags(svec!["deno", "gist.ts", "--title", "X", "--allow-net"]).unwrap();
  assert_eq!(rest, svec!["deno", "gist.ts", "--title", "X"]);
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
  let (flags, rest) =
    set_flags(svec!["deno", "gist.ts", "--allow-all"]).unwrap();
  assert_eq!(rest, svec!["deno", "gist.ts"]);
  assert_eq!(
    flags,
    DenoFlags {
      allow_net: true,
      allow_env: true,
      allow_run: true,
      allow_read: true,
      allow_write: true,
      ..DenoFlags::default()
    }
  )
}

#[test]
fn test_set_flags_8() {
  let (flags, rest) =
    set_flags(svec!["deno", "gist.ts", "--allow-read"]).unwrap();
  assert_eq!(rest, svec!["deno", "gist.ts"]);
  assert_eq!(
    flags,
    DenoFlags {
      allow_read: true,
      ..DenoFlags::default()
    }
  )
}
