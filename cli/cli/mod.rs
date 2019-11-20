// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use crate::flags::DenoFlags;
use clap::App;
use clap::AppSettings;
use clap::Arg;
use clap::Shell;
use std;
use std::str::FromStr;

mod args;
mod bundle;
mod completions;
mod eval;
mod fetch;
mod fmt;
mod info;
mod install;
mod run;
mod test;
mod types;
pub mod util;
mod xeval;

macro_rules! std_url {
  ($x:expr) => {
    concat!("https://deno.land/std@v0.23.0/", $x)
  };
}

/// Used for `deno fmt <files>...` subcommand
pub(crate) const PRETTIER_URL: &str = std_url!("prettier/main.ts");
/// Used for `deno install...` subcommand
pub(crate) const INSTALLER_URL: &str = std_url!("installer/mod.ts");
/// Used for `deno test...` subcommand
pub(crate) const TEST_RUNNER_URL: &str = std_url!("testing/runner.ts");
/// Used for `deno xeval...` subcommand
pub(crate) const XEVAL_URL: &str = std_url!("xeval/mod.ts");

static ENV_VARIABLES_HELP: &str = "ENVIRONMENT VARIABLES:
    DENO_DIR        Set deno's base directory
    NO_COLOR        Set to disable color
    HTTP_PROXY      Set proxy address for HTTP requests (module downloads, fetch)
    HTTPS_PROXY     Set proxy address for HTTPS requests (module downloads, fetch)";

static LONG_ABOUT: &str = "A secure JavaScript and TypeScript runtime

Docs: https://deno.land/manual.html
Modules: https://deno.land/x/
Bugs: https://github.com/denoland/deno/issues

To run the REPL:

  deno

To execute a sandboxed script:

  deno https://deno.land/std/examples/welcome.ts

To evaluate code from the command line:

  deno eval \"console.log(30933 + 404)\"

To get help on the another subcommands (run in this case):

  deno help run";

#[allow(dead_code)]
pub fn create_cli_app<'a, 'b>() -> App<'a, 'b> {
  let app = App::new("deno")
    .bin_name("deno")
    .global_settings(&[
      AppSettings::ColorNever,
      AppSettings::UnifiedHelpMessage,
      AppSettings::DisableVersion,
      AppSettings::DisableHelpSubcommand,
    ])
    .settings(&[AppSettings::AllowExternalSubcommands])
    .after_help(ENV_VARIABLES_HELP)
    .long_about(LONG_ABOUT)
    .arg(
      Arg::with_name("version")
        .short("v")
        .long("version")
        .help("Print the version"),
    );

  // Assigns the same arguments as for "deno run"
  let app = run::bootstrap_run(app);
  // You should be able to run "deno" or "deno script.ts", so main "app" doesn't
  // require subcommand to be given (which we use as a script name)
  let app = app.unset_setting(AppSettings::SubcommandRequired);

  app
    .subcommand(run::run_subcommand())
    .subcommand(test::test_subcommand())
    .subcommand(info::info_subcommand())
    .subcommand(bundle::bundle_subcommand())
    .subcommand(fetch::fetch_subcommand())
    .subcommand(types::types_subcommand())
    .subcommand(eval::eval_subcommand())
    .subcommand(fmt::fmt_subcommand())
    .subcommand(xeval::xeval_subcommand())
    .subcommand(completions::completions_subcommand())
    .subcommand(install::install_subcommand())
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
}

pub fn flags_from_vec(
  args: Vec<String>,
) -> (DenoFlags, DenoSubcommand, Vec<String>) {
  let cli_app = create_cli_app();
  let matches = cli_app.get_matches_from(args);
  let mut argv: Vec<String> = vec!["deno".to_string()];
  let mut flags = DenoFlags::default();

  if matches.is_present("version") {
    flags.version = true;
    return (flags, DenoSubcommand::Version, argv);
  }

  let subcommand = match matches.clone().subcommand() {
    ("bundle", Some(bundle_match)) => {
      bundle::parse(&mut flags, &mut argv, bundle_match)
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
      eval::parse(&mut flags, &mut argv, eval_match)
    }
    ("fetch", Some(fetch_match)) => {
      fetch::parse(&mut flags, &mut argv, fetch_match)
    }
    ("fmt", Some(fmt_match)) => fmt::parse(&mut flags, &mut argv, fmt_match),
    ("info", Some(info_match)) => {
      info::parse(&mut flags, &mut argv, info_match)
    }
    ("install", Some(install_match)) => {
      install::parse(&mut flags, &mut argv, install_match)
    }
    ("test", Some(test_match)) => {
      test::parse(&mut flags, &mut argv, test_match)
    }
    ("types", Some(_)) => DenoSubcommand::Types,
    ("run", Some(run_match)) => run::parse(&mut flags, &mut argv, run_match),
    ("xeval", Some(xeval_match)) => {
      xeval::parse(&mut flags, &mut argv, xeval_match)
    }
    (_script, Some(_script_match)) => {
      run::parse(&mut flags, &mut argv, &matches.clone())
    }
    _ => run::parse_repl(&mut flags, &mut argv, &matches.clone()),
  };

  (flags, subcommand, argv)
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::fs as deno_fs;
  use log::Level;

  // Creates vector of strings, Vec<String>
  macro_rules! svec {
      ($($x:expr),*) => (vec![$($x.to_string()),*]);
  }

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
      flags_from_vec(svec!["deno", "run", "-r", "script.ts"]);
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
  fn test_flags_from_vec_5() {
    let (flags, subcommand, argv) =
      flags_from_vec(svec!["deno", "run", "--v8-options", "script.ts"]);
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
      "run",
      "--v8-flags=--expose-gc,--gc-stats=1",
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
        "--write",
        "--config",
        "auto",
        "--ignore-path",
        "auto"
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
        ..DenoFlags::default()
      }
    );
    assert_eq!(subcommand, DenoSubcommand::Run);
    assert_eq!(
      argv,
      svec![
        "deno",
        XEVAL_URL,
        "--delim",
        " ",
        "--replvar",
        "val",
        "console.log(val)"
      ]
    );
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
      svec![
        "deno",
        PRETTIER_URL,
        "script_1.ts",
        "script_2.ts",
        "--config",
        "auto",
        "--ignore-path",
        "auto"
      ]
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
      "run",
      "-r",
      "--allow-net",
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
      flags_from_vec(svec!["deno", "run", "--seed", "250", "script.ts"]);
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
      "run",
      "--seed",
      "250",
      "--v8-flags=--expose-gc",
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

  #[test]
  fn test_flags_from_vec_37() {
    let (flags, subcommand, argv) = flags_from_vec(svec![
      "deno",
      "--allow-net=deno.land,:8000,:4545",
      "script.ts"
    ]);
    assert_eq!(
      flags,
      DenoFlags {
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
    assert_eq!(subcommand, DenoSubcommand::Run);
    assert_eq!(argv, svec!["deno", "script.ts"])
  }

  #[test]
  fn test_flags_from_vec_38() {
    let (flags, subcommand, argv) = flags_from_vec(svec![
      "deno",
      "--lock-write",
      "--lock=lock.json",
      "script.ts"
    ]);
    assert_eq!(
      flags,
      DenoFlags {
        lock_write: true,
        lock: Some("lock.json".to_string()),
        ..DenoFlags::default()
      }
    );
    assert_eq!(subcommand, DenoSubcommand::Run);
    assert_eq!(argv, svec!["deno", "script.ts"])
  }

  #[test]
  fn test_flags_from_vec_39() {
    let (flags, subcommand, argv) = flags_from_vec(svec![
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
      ]
    );
  }
}
