// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use clap::App;
use clap::AppSettings;
use clap::Arg;
use clap::ArgMatches;
use clap::SubCommand;
use log::Level;
use std::net::SocketAddr;
use std::path::PathBuf;

/// Creates vector of strings, Vec<String>
macro_rules! svec {
    ($($x:expr),*) => (vec![$($x.to_string()),*]);
}

#[derive(Clone, Debug, PartialEq)]
pub enum DenoSubcommand {
  Bundle {
    source_file: String,
    out_file: Option<PathBuf>,
  },
  Completions {
    buf: Box<[u8]>,
  },
  Doc {
    json: bool,
    source_file: Option<String>,
    filter: Option<String>,
  },
  Eval {
    print: bool,
    code: String,
    as_typescript: bool,
  },
  Cache {
    files: Vec<String>,
  },
  Fmt {
    check: bool,
    files: Vec<String>,
  },
  Help,
  Info {
    file: Option<String>,
  },
  Install {
    module_url: String,
    args: Vec<String>,
    name: Option<String>,
    root: Option<PathBuf>,
    force: bool,
  },
  Lint {
    files: Vec<String>,
  },
  Repl,
  Run {
    script: String,
  },
  Test {
    fail_fast: bool,
    quiet: bool,
    allow_none: bool,
    include: Option<Vec<String>>,
    filter: Option<String>,
  },
  Types,
  Upgrade {
    dry_run: bool,
    force: bool,
    version: Option<String>,
  },
}

impl Default for DenoSubcommand {
  fn default() -> DenoSubcommand {
    DenoSubcommand::Repl
  }
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Flags {
  /// Vector of CLI arguments - these are user script arguments, all Deno
  /// specific flags are removed.
  pub argv: Vec<String>,
  pub subcommand: DenoSubcommand,

  pub allow_env: bool,
  pub allow_hrtime: bool,
  pub allow_net: bool,
  pub allow_plugin: bool,
  pub allow_read: bool,
  pub allow_run: bool,
  pub allow_write: bool,
  pub cache_blacklist: Vec<String>,
  pub ca_file: Option<String>,
  pub cached_only: bool,
  pub config_path: Option<String>,
  pub import_map_path: Option<String>,
  pub inspect: Option<SocketAddr>,
  pub inspect_brk: Option<SocketAddr>,
  pub lock: Option<String>,
  pub lock_write: bool,
  pub log_level: Option<Level>,
  pub net_whitelist: Vec<String>,
  pub no_prompts: bool,
  pub no_remote: bool,
  pub read_whitelist: Vec<PathBuf>,
  pub reload: bool,
  pub seed: Option<u64>,
  pub unstable: bool,
  pub v8_flags: Option<Vec<String>>,
  pub version: bool,
  pub write_whitelist: Vec<PathBuf>,
}

fn join_paths(whitelist: &[PathBuf], d: &str) -> String {
  whitelist
    .iter()
    .map(|path| path.to_str().unwrap().to_string())
    .collect::<Vec<String>>()
    .join(d)
}

impl Flags {
  /// Return list of permission arguments that are equivalent
  /// to the ones used to create `self`.
  pub fn to_permission_args(&self) -> Vec<String> {
    let mut args = vec![];

    if !self.read_whitelist.is_empty() {
      let s = format!("--allow-read={}", join_paths(&self.read_whitelist, ","));
      args.push(s);
    }

    if self.allow_read {
      args.push("--allow-read".to_string());
    }

    if !self.write_whitelist.is_empty() {
      let s =
        format!("--allow-write={}", join_paths(&self.write_whitelist, ","));
      args.push(s);
    }

    if self.allow_write {
      args.push("--allow-write".to_string());
    }

    if !self.net_whitelist.is_empty() {
      let s = format!("--allow-net={}", self.net_whitelist.join(","));
      args.push(s);
    }

    if self.allow_net {
      args.push("--allow-net".to_string());
    }

    if self.allow_env {
      args.push("--allow-env".to_string());
    }

    if self.allow_run {
      args.push("--allow-run".to_string());
    }

    if self.allow_plugin {
      args.push("--allow-plugin".to_string());
    }

    if self.allow_hrtime {
      args.push("--allow-hrtime".to_string());
    }

    args
  }
}

static ENV_VARIABLES_HELP: &str = "ENVIRONMENT VARIABLES:
    DENO_DIR             Set deno's base directory (defaults to $HOME/.deno)
    DENO_INSTALL_ROOT    Set deno install's output directory
                         (defaults to $HOME/.deno/bin)
    NO_COLOR             Set to disable color
    HTTP_PROXY           Proxy address for HTTP requests
                         (module downloads, fetch)
    HTTPS_PROXY          Proxy address for HTTPS requests
                         (module downloads, fetch)";

static DENO_HELP: &str = "A secure JavaScript and TypeScript runtime

Docs: https://deno.land/manual
Modules: https://deno.land/std/ https://deno.land/x/
Bugs: https://github.com/denoland/deno/issues

To start the REPL:
  deno

To execute a script:
  deno run https://deno.land/std/examples/welcome.ts

To evaluate code in the shell:
  deno eval \"console.log(30933 + 404)\"
";

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
pub fn flags_from_vec(args: Vec<String>) -> Flags {
  match flags_from_vec_safe(args) {
    Ok(flags) => flags,
    Err(err) => err.exit(),
  }
}

/// Same as flags_from_vec but does not exit on error.
pub fn flags_from_vec_safe(args: Vec<String>) -> clap::Result<Flags> {
  let app = clap_root();
  let matches = app.get_matches_from_safe(args)?;

  let mut flags = Flags::default();

  if matches.is_present("log-level") {
    flags.log_level = match matches.value_of("log-level").unwrap() {
      "debug" => Some(Level::Debug),
      "info" => Some(Level::Info),
      _ => unreachable!(),
    };
  }
  if matches.is_present("quiet") {
    flags.log_level = Some(Level::Error);
  }

  if let Some(m) = matches.subcommand_matches("run") {
    run_parse(&mut flags, m);
  } else if let Some(m) = matches.subcommand_matches("fmt") {
    fmt_parse(&mut flags, m);
  } else if let Some(m) = matches.subcommand_matches("types") {
    types_parse(&mut flags, m);
  } else if let Some(m) = matches.subcommand_matches("cache") {
    cache_parse(&mut flags, m);
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
  } else if let Some(m) = matches.subcommand_matches("upgrade") {
    upgrade_parse(&mut flags, m);
  } else if let Some(m) = matches.subcommand_matches("doc") {
    doc_parse(&mut flags, m);
  } else if let Some(m) = matches.subcommand_matches("lint") {
    lint_parse(&mut flags, m);
  } else {
    repl_parse(&mut flags, &matches);
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
    .arg(
      Arg::with_name("quiet")
        .short("q")
        .long("quiet")
        .help("Suppress diagnostic output")
        .long_help(
          "Suppress diagnostic output
By default, subcommands print human-readable diagnostic messages to stderr.
If the flag is set, restrict these messages to errors.",
        )
        .global(true),
    )
    .subcommand(bundle_subcommand())
    .subcommand(cache_subcommand())
    .subcommand(completions_subcommand())
    .subcommand(doc_subcommand())
    .subcommand(eval_subcommand())
    .subcommand(fmt_subcommand())
    .subcommand(info_subcommand())
    .subcommand(install_subcommand())
    .subcommand(lint_subcommand())
    .subcommand(repl_subcommand())
    .subcommand(run_subcommand())
    .subcommand(test_subcommand())
    .subcommand(types_subcommand())
    .subcommand(upgrade_subcommand())
    .long_about(DENO_HELP)
    .after_help(ENV_VARIABLES_HELP)
}

fn types_parse(flags: &mut Flags, matches: &clap::ArgMatches) {
  unstable_arg_parse(flags, matches);
  flags.subcommand = DenoSubcommand::Types;
}

fn fmt_parse(flags: &mut Flags, matches: &clap::ArgMatches) {
  let files = match matches.values_of("files") {
    Some(f) => f.map(String::from).collect(),
    None => vec![],
  };
  flags.subcommand = DenoSubcommand::Fmt {
    check: matches.is_present("check"),
    files,
  }
}

fn install_parse(flags: &mut Flags, matches: &clap::ArgMatches) {
  permission_args_parse(flags, matches);
  ca_file_arg_parse(flags, matches);
  unstable_arg_parse(flags, matches);

  let root = if matches.is_present("root") {
    let install_root = matches.value_of("root").unwrap();
    Some(PathBuf::from(install_root))
  } else {
    None
  };

  let force = matches.is_present("force");
  let name = matches.value_of("name").map(|s| s.to_string());
  let cmd_values = matches.values_of("cmd").unwrap();
  let mut cmd = vec![];
  for value in cmd_values {
    cmd.push(value.to_string());
  }

  let module_url = cmd[0].to_string();
  let args = cmd[1..].to_vec();

  flags.subcommand = DenoSubcommand::Install {
    name,
    module_url,
    args,
    root,
    force,
  };
}

fn bundle_parse(flags: &mut Flags, matches: &clap::ArgMatches) {
  ca_file_arg_parse(flags, matches);
  config_arg_parse(flags, matches);
  importmap_arg_parse(flags, matches);
  unstable_arg_parse(flags, matches);

  let source_file = matches.value_of("source_file").unwrap().to_string();

  let out_file = if let Some(out_file) = matches.value_of("out_file") {
    flags.allow_write = true;
    Some(PathBuf::from(out_file))
  } else {
    None
  };

  flags.subcommand = DenoSubcommand::Bundle {
    source_file,
    out_file,
  };
}

fn completions_parse(flags: &mut Flags, matches: &clap::ArgMatches) {
  let shell: &str = matches.value_of("shell").unwrap();
  let mut buf: Vec<u8> = vec![];
  use std::str::FromStr;
  clap_root().gen_completions_to(
    "deno",
    clap::Shell::from_str(shell).unwrap(),
    &mut buf,
  );

  flags.subcommand = DenoSubcommand::Completions {
    buf: buf.into_boxed_slice(),
  };
}

fn repl_parse(flags: &mut Flags, matches: &clap::ArgMatches) {
  v8_flags_arg_parse(flags, matches);
  ca_file_arg_parse(flags, matches);
  inspect_arg_parse(flags, matches);
  unstable_arg_parse(flags, matches);
  flags.subcommand = DenoSubcommand::Repl;
  flags.allow_net = true;
  flags.allow_env = true;
  flags.allow_run = true;
  flags.allow_read = true;
  flags.allow_write = true;
  flags.allow_plugin = true;
  flags.allow_hrtime = true;
}

fn eval_parse(flags: &mut Flags, matches: &clap::ArgMatches) {
  v8_flags_arg_parse(flags, matches);
  ca_file_arg_parse(flags, matches);
  inspect_arg_parse(flags, matches);
  unstable_arg_parse(flags, matches);
  flags.allow_net = true;
  flags.allow_env = true;
  flags.allow_run = true;
  flags.allow_read = true;
  flags.allow_write = true;
  flags.allow_plugin = true;
  flags.allow_hrtime = true;
  let code = matches.value_of("code").unwrap().to_string();
  let as_typescript = matches.is_present("ts");
  let print = matches.is_present("print");
  flags.subcommand = DenoSubcommand::Eval {
    print,
    code,
    as_typescript,
  }
}

fn info_parse(flags: &mut Flags, matches: &clap::ArgMatches) {
  ca_file_arg_parse(flags, matches);
  unstable_arg_parse(flags, matches);

  flags.subcommand = DenoSubcommand::Info {
    file: matches.value_of("file").map(|f| f.to_string()),
  };
}

fn cache_parse(flags: &mut Flags, matches: &clap::ArgMatches) {
  reload_arg_parse(flags, matches);
  lock_args_parse(flags, matches);
  importmap_arg_parse(flags, matches);
  config_arg_parse(flags, matches);
  no_remote_arg_parse(flags, matches);
  ca_file_arg_parse(flags, matches);
  unstable_arg_parse(flags, matches);
  let files = matches
    .values_of("file")
    .unwrap()
    .map(String::from)
    .collect();
  flags.subcommand = DenoSubcommand::Cache { files };
}

fn lock_args_parse(flags: &mut Flags, matches: &clap::ArgMatches) {
  if matches.is_present("lock") {
    let lockfile = matches.value_of("lock").unwrap();
    flags.lock = Some(lockfile.to_string());
  }
  if matches.is_present("lock-write") {
    flags.lock_write = true;
  }
}

// Shared between the run and test subcommands. They both take similar options.
fn run_test_args_parse(flags: &mut Flags, matches: &clap::ArgMatches) {
  reload_arg_parse(flags, matches);
  lock_args_parse(flags, matches);
  importmap_arg_parse(flags, matches);
  config_arg_parse(flags, matches);
  v8_flags_arg_parse(flags, matches);
  no_remote_arg_parse(flags, matches);
  permission_args_parse(flags, matches);
  ca_file_arg_parse(flags, matches);
  inspect_arg_parse(flags, matches);
  unstable_arg_parse(flags, matches);

  if matches.is_present("cached-only") {
    flags.cached_only = true;
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

fn run_parse(flags: &mut Flags, matches: &clap::ArgMatches) {
  run_test_args_parse(flags, matches);

  let mut script: Vec<String> = matches
    .values_of("script_arg")
    .unwrap()
    .map(String::from)
    .collect();
  assert!(!script.is_empty());
  let script_args = script.split_off(1);
  let script = script[0].to_string();
  for v in script_args {
    flags.argv.push(v);
  }

  flags.subcommand = DenoSubcommand::Run { script };
}

fn test_parse(flags: &mut Flags, matches: &clap::ArgMatches) {
  run_test_args_parse(flags, matches);

  let failfast = matches.is_present("failfast");
  let allow_none = matches.is_present("allow_none");
  let quiet = matches.is_present("quiet");
  let filter = matches.value_of("filter").map(String::from);
  let include = if matches.is_present("files") {
    let files: Vec<String> = matches
      .values_of("files")
      .unwrap()
      .map(String::from)
      .collect();
    Some(files)
  } else {
    None
  };

  flags.subcommand = DenoSubcommand::Test {
    fail_fast: failfast,
    quiet,
    include,
    filter,
    allow_none,
  };
}

fn upgrade_parse(flags: &mut Flags, matches: &clap::ArgMatches) {
  let dry_run = matches.is_present("dry-run");
  let force = matches.is_present("force");
  let version = matches.value_of("version").map(|s| s.to_string());
  flags.subcommand = DenoSubcommand::Upgrade {
    dry_run,
    force,
    version,
  };
}

fn doc_parse(flags: &mut Flags, matches: &clap::ArgMatches) {
  reload_arg_parse(flags, matches);
  unstable_arg_parse(flags, matches);

  let source_file = matches.value_of("source_file").map(String::from);
  let json = matches.is_present("json");
  let filter = matches.value_of("filter").map(String::from);
  flags.subcommand = DenoSubcommand::Doc {
    source_file,
    json,
    filter,
  };
}

fn lint_parse(flags: &mut Flags, matches: &clap::ArgMatches) {
  unstable_arg_parse(flags, matches);
  let files = matches
    .values_of("files")
    .unwrap()
    .map(String::from)
    .collect();
  flags.subcommand = DenoSubcommand::Lint { files };
}

fn types_subcommand<'a, 'b>() -> App<'a, 'b> {
  SubCommand::with_name("types")
    .arg(unstable_arg())
    .about("Print runtime TypeScript declarations")
    .long_about(
      "Print runtime TypeScript declarations.
  deno types > lib.deno.d.ts

The declaration file could be saved and used for typing information.",
    )
}

fn fmt_subcommand<'a, 'b>() -> App<'a, 'b> {
  SubCommand::with_name("fmt")
    .about("Format source files")
    .long_about(
      "Auto-format JavaScript/TypeScript source code.
  deno fmt
  deno fmt myfile1.ts myfile2.ts
  deno fmt --check

Format stdin and write to stdout:
  cat file.ts | deno fmt -

Ignore formatting code by preceding it with an ignore comment:
  // deno-fmt-ignore

Ignore formatting a file by adding an ignore comment at the top of the file:
  // deno-fmt-ignore-file",
    )
    .arg(
      Arg::with_name("check")
        .long("check")
        .help("Check if the source files are formatted.")
        .takes_value(false),
    )
    .arg(
      Arg::with_name("files")
        .takes_value(true)
        .multiple(true)
        .required(false),
    )
}

fn repl_subcommand<'a, 'b>() -> App<'a, 'b> {
  inspect_args(SubCommand::with_name("repl"))
    .about("Read Eval Print Loop")
    .arg(v8_flags_arg())
    .arg(ca_file_arg())
    .arg(unstable_arg())
}

fn install_subcommand<'a, 'b>() -> App<'a, 'b> {
  permission_args(SubCommand::with_name("install"))
        .setting(AppSettings::TrailingVarArg)
        .arg(
          Arg::with_name("cmd")
            .required(true)
            .multiple(true)
            .allow_hyphen_values(true))
        .arg(
          Arg::with_name("name")
          .long("name")
          .short("n")
          .help("Executable file name")
          .takes_value(true)
          .required(false))
        .arg(
          Arg::with_name("root")
            .long("root")
            .help("Installation root")
            .takes_value(true)
            .multiple(false))
        .arg(
          Arg::with_name("force")
            .long("force")
            .short("f")
            .help("Forcefully overwrite existing installation")
            .takes_value(false))
        .arg(ca_file_arg())
        .arg(unstable_arg())
        .about("Install script as an executable")
        .long_about(
"Installs a script as an executable in the installation root's bin directory.
  deno install --allow-net --allow-read https://deno.land/std/http/file_server.ts
  deno install https://deno.land/std/examples/colors.ts

To change the executable name, use -n/--name:
  deno install --allow-net --allow-read -n serve https://deno.land/std/http/file_server.ts

The executable name is inferred by default:
  - Attempt to take the file stem of the URL path. The above example would
    become 'file_server'.
  - If the file stem is something generic like 'main', 'mod', 'index' or 'cli',
    and the path has no parent, take the file name of the parent path. Otherwise
    settle with the generic name.

To change the installation root, use --root:
  deno install --allow-net --allow-read --root /usr/local https://deno.land/std/http/file_server.ts

The installation root is determined, in order of precedence:
  - --root option
  - DENO_INSTALL_ROOT environment variable
  - $HOME/.deno

These must be added to the path manually if required.")
}

fn bundle_subcommand<'a, 'b>() -> App<'a, 'b> {
  SubCommand::with_name("bundle")
    .arg(
      Arg::with_name("source_file")
        .takes_value(true)
        .required(true),
    )
    .arg(Arg::with_name("out_file").takes_value(true).required(false))
    .arg(ca_file_arg())
    .arg(importmap_arg())
    .arg(unstable_arg())
    .arg(config_arg())
    .about("Bundle module and dependencies into single file")
    .long_about(
      "Output a single JavaScript file with all dependencies.
  deno bundle https://deno.land/std/examples/colors.ts colors.bundle.js

If no output file is given, the output is written to standard output:
  deno bundle https://deno.land/std/examples/colors.ts",
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
  deno completions bash > /usr/local/etc/bash_completion.d/deno.bash
  source /usr/local/etc/bash_completion.d/deno.bash",
    )
}

fn eval_subcommand<'a, 'b>() -> App<'a, 'b> {
  inspect_args(SubCommand::with_name("eval"))
    .arg(ca_file_arg())
    .arg(unstable_arg())
    .about("Eval script")
    .long_about(
      "Evaluate JavaScript from the command line.
  deno eval \"console.log('hello world')\"

To evaluate as TypeScript:
  deno eval -T \"const v: string = 'hello'; console.log(v)\"

This command has implicit access to all permissions (--allow-all).",
    )
    .arg(
      Arg::with_name("ts")
        .long("ts")
        .short("T")
        .help("Treat eval input as TypeScript")
        .takes_value(false)
        .multiple(false),
    )
    .arg(
      Arg::with_name("print")
        .long("print")
        .short("p")
        .help("print result to stdout")
        .takes_value(false)
        .multiple(false),
    )
    .arg(Arg::with_name("code").takes_value(true).required(true))
    .arg(v8_flags_arg())
}

fn info_subcommand<'a, 'b>() -> App<'a, 'b> {
  SubCommand::with_name("info")
    .about("Show info about cache or info related to source file")
    .long_about(
      "Information about a module or the cache directories.

Get information about a module:
  deno info https://deno.land/std/http/file_server.ts

The following information is shown:

local: Local path of the file.
type: JavaScript, TypeScript, or JSON.
compiled: Local path of compiled source code. (TypeScript only.)
map: Local path of source map. (TypeScript only.)
deps: Dependency tree of the source file.

Without any additional arguments, 'deno info' shows:

DENO_DIR: Directory containing Deno-managed files.
Remote modules cache: Subdirectory containing downloaded remote modules.
TypeScript compiler cache: Subdirectory containing TS compiler output.",
    )
    .arg(Arg::with_name("file").takes_value(true).required(false))
    .arg(ca_file_arg())
    .arg(unstable_arg())
}

fn cache_subcommand<'a, 'b>() -> App<'a, 'b> {
  SubCommand::with_name("cache")
    .arg(reload_arg())
    .arg(lock_arg())
    .arg(lock_write_arg())
    .arg(importmap_arg())
    .arg(unstable_arg())
    .arg(config_arg())
    .arg(no_remote_arg())
    .arg(
      Arg::with_name("file")
        .takes_value(true)
        .required(true)
        .min_values(1),
    )
    .arg(ca_file_arg())
    .about("Cache the dependencies")
    .long_about(
      "Cache and compile remote dependencies recursively.

Download and compile a module with all of its static dependencies and save them
in the local cache, without running any code:
  deno cache https://deno.land/std/http/file_server.ts

Future runs of this module will trigger no downloads or compilation unless
--reload is specified.",
    )
}

fn upgrade_subcommand<'a, 'b>() -> App<'a, 'b> {
  SubCommand::with_name("upgrade")
    .about("Upgrade deno executable to given version")
    .long_about(
      "Upgrade deno executable to the given version.
Defaults to latest.

The version is downloaded from
https://github.com/denoland/deno/releases
and is used to replace the current executable.",
    )
    .arg(
      Arg::with_name("version")
        .long("version")
        .help("The version to upgrade to")
        .takes_value(true),
    )
    .arg(
      Arg::with_name("dry-run")
        .long("dry-run")
        .help("Perform all checks without replacing old exe"),
    )
    .arg(
      Arg::with_name("force")
        .long("force")
        .short("f")
        .help("Replace current exe even if not out-of-date"),
    )
}

fn doc_subcommand<'a, 'b>() -> App<'a, 'b> {
  SubCommand::with_name("doc")
    .arg(unstable_arg())
    .about("Show documentation for a module")
    .long_about(
      "Show documentation for a module.

Output documentation to standard output:
    deno doc ./path/to/module.ts

Output documentation in JSON format:
    deno doc --json ./path/to/module.ts

Target a specific symbol:
    deno doc ./path/to/module.ts MyClass.someField

Show documentation for runtime built-ins:
    deno doc
    deno doc --builtin Deno.Listener",
    )
    .arg(reload_arg())
    .arg(
      Arg::with_name("json")
        .long("json")
        .help("Output documentation in JSON format.")
        .takes_value(false),
    )
    // TODO(nayeemrmn): Make `--builtin` a proper option. Blocked by
    // https://github.com/clap-rs/clap/issues/1794. Currently `--builtin` is
    // just a possible value of `source_file` so leading hyphens must be
    // enabled.
    .setting(clap::AppSettings::AllowLeadingHyphen)
    .arg(Arg::with_name("source_file").takes_value(true))
    .arg(
      Arg::with_name("filter")
        .help("Dot separated path to symbol.")
        .takes_value(true)
        .required(false)
        .conflicts_with("json")
        .conflicts_with("pretty"),
    )
}

fn lint_subcommand<'a, 'b>() -> App<'a, 'b> {
  SubCommand::with_name("lint")
    .about("Lint source files")
    .long_about(
      "Lint JavaScript/TypeScript source code.
  deno lint myfile1.ts myfile2.js

Ignore diagnostics on next line preceding it with an ignore comment and code:
  // deno-lint-ignore no-explicit-any",
    )
    .arg(unstable_arg())
    .arg(
      Arg::with_name("files")
        .takes_value(true)
        .required(true)
        .min_values(1),
    )
}

fn permission_args<'a, 'b>(app: App<'a, 'b>) -> App<'a, 'b> {
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
}

fn run_test_args<'a, 'b>(app: App<'a, 'b>) -> App<'a, 'b> {
  permission_args(inspect_args(app))
    .arg(importmap_arg())
    .arg(unstable_arg())
    .arg(reload_arg())
    .arg(config_arg())
    .arg(lock_arg())
    .arg(lock_write_arg())
    .arg(no_remote_arg())
    .arg(v8_flags_arg())
    .arg(ca_file_arg())
    .arg(
      Arg::with_name("cached-only")
        .long("cached-only")
        .help("Require that remote dependencies are already cached"),
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
    .about("Run a program given a filename or url to the module")
    .long_about(
      "Run a program given a filename or url to the module.

By default all programs are run in sandbox without access to disk, network or
ability to spawn subprocesses.
  deno run https://deno.land/std/examples/welcome.ts

Grant all permissions:
  deno run -A https://deno.land/std/http/file_server.ts

Grant permission to read from disk and listen to network:
  deno run --allow-read --allow-net https://deno.land/std/http/file_server.ts

Grant permission to read whitelisted files from disk:
  deno run --allow-read=/etc https://deno.land/std/http/file_server.ts",
    )
}

fn test_subcommand<'a, 'b>() -> App<'a, 'b> {
  run_test_args(SubCommand::with_name("test"))
    .arg(
      Arg::with_name("failfast")
        .long("failfast")
        .help("Stop on first error")
        .takes_value(false),
    )
    .arg(
      Arg::with_name("allow_none")
        .long("allow-none")
        .help("Don't return error code if no test files are found")
        .takes_value(false),
    )
    .arg(
      Arg::with_name("filter")
        .long("filter")
        .takes_value(true)
        .help("A pattern to filter the tests to run by"),
    )
    .arg(
      Arg::with_name("files")
        .help("List of file names to run")
        .takes_value(true)
        .multiple(true),
    )
    .about("Run tests")
    .long_about(
      "Run tests using Deno's built-in test runner.

Evaluate the given modules, run all tests declared with 'Deno.test()' and
report results to standard output:
  deno test src/fetch_test.ts src/signal_test.ts

Directory arguments are expanded to all contained files matching the glob
{*_,*.,}test.{js,mjs,ts,jsx,tsx}:
  deno test src/",
    )
}

fn script_arg<'a, 'b>() -> Arg<'a, 'b> {
  Arg::with_name("script_arg")
    .multiple(true)
    .required(true)
    .help("script args")
    .value_name("SCRIPT_ARG")
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

fn config_arg_parse(flags: &mut Flags, matches: &ArgMatches) {
  flags.config_path = matches.value_of("config").map(ToOwned::to_owned);
}

fn ca_file_arg<'a, 'b>() -> Arg<'a, 'b> {
  Arg::with_name("cert")
    .long("cert")
    .value_name("FILE")
    .help("Load certificate authority from PEM encoded file")
    .takes_value(true)
}

fn ca_file_arg_parse(flags: &mut Flags, matches: &clap::ArgMatches) {
  flags.ca_file = matches.value_of("cert").map(ToOwned::to_owned);
}

fn unstable_arg<'a, 'b>() -> Arg<'a, 'b> {
  Arg::with_name("unstable")
    .long("unstable")
    .help("Enable unstable APIs")
}

fn unstable_arg_parse(flags: &mut Flags, matches: &clap::ArgMatches) {
  if matches.is_present("unstable") {
    flags.unstable = true;
  }
}

fn inspect_args<'a, 'b>(app: App<'a, 'b>) -> App<'a, 'b> {
  app
    .arg(
      Arg::with_name("inspect")
        .long("inspect")
        .value_name("HOST:PORT")
        .help("activate inspector on host:port (default: 127.0.0.1:9229)")
        .min_values(0)
        .max_values(1)
        .require_equals(true)
        .takes_value(true)
        .validator(inspect_arg_validate),
    )
    .arg(
      Arg::with_name("inspect-brk")
        .long("inspect-brk")
        .value_name("HOST:PORT")
        .help(
          "activate inspector on host:port and break at start of user script",
        )
        .min_values(0)
        .max_values(1)
        .require_equals(true)
        .takes_value(true)
        .validator(inspect_arg_validate),
    )
}

fn inspect_arg_validate(val: String) -> Result<(), String> {
  match val.parse::<SocketAddr>() {
    Ok(_) => Ok(()),
    Err(e) => Err(e.to_string()),
  }
}

fn inspect_arg_parse(flags: &mut Flags, matches: &clap::ArgMatches) {
  let default = || "127.0.0.1:9229".parse::<SocketAddr>().unwrap();
  flags.inspect = if matches.is_present("inspect") {
    if let Some(host) = matches.value_of("inspect") {
      Some(host.parse().unwrap())
    } else {
      Some(default())
    }
  } else {
    None
  };
  flags.inspect_brk = if matches.is_present("inspect-brk") {
    if let Some(host) = matches.value_of("inspect-brk") {
      Some(host.parse().unwrap())
    } else {
      Some(default())
    }
  } else {
    None
  };
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

fn reload_arg_parse(flags: &mut Flags, matches: &ArgMatches) {
  if let Some(cache_bl) = matches.values_of("reload") {
    let raw_cache_blacklist: Vec<String> =
      cache_bl.map(std::string::ToString::to_string).collect();
    if raw_cache_blacklist.is_empty() {
      flags.reload = true;
    } else {
      flags.cache_blacklist = resolve_urls(raw_cache_blacklist);
      debug!("cache blacklist: {:#?}", &flags.cache_blacklist);
      flags.reload = false;
    }
  }
}

fn importmap_arg<'a, 'b>() -> Arg<'a, 'b> {
  Arg::with_name("importmap")
    .long("importmap")
    .value_name("FILE")
    .help("UNSTABLE: Load import map file")
    .long_help(
      "UNSTABLE:
Load import map file
Docs: https://deno.land/manual/linking_to_external_code/import_maps
Specification: https://wicg.github.io/import-maps/
Examples: https://github.com/WICG/import-maps#the-import-map",
    )
    .takes_value(true)
}

fn importmap_arg_parse(flags: &mut Flags, matches: &clap::ArgMatches) {
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

fn v8_flags_arg_parse(flags: &mut Flags, matches: &ArgMatches) {
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

fn no_remote_arg_parse(flags: &mut Flags, matches: &clap::ArgMatches) {
  if matches.is_present("no-remote") {
    flags.no_remote = true;
  }
}

fn permission_args_parse(flags: &mut Flags, matches: &clap::ArgMatches) {
  if let Some(read_wl) = matches.values_of("allow-read") {
    let read_whitelist: Vec<PathBuf> = read_wl.map(PathBuf::from).collect();

    if read_whitelist.is_empty() {
      flags.allow_read = true;
    } else {
      flags.read_whitelist = read_whitelist;
    }
  }

  if let Some(write_wl) = matches.values_of("allow-write") {
    let write_whitelist: Vec<PathBuf> = write_wl.map(PathBuf::from).collect();

    if write_whitelist.is_empty() {
      flags.allow_write = true;
    } else {
      flags.write_whitelist = write_whitelist;
    }
  }

  if let Some(net_wl) = matches.values_of("allow-net") {
    let raw_net_whitelist: Vec<String> =
      net_wl.map(std::string::ToString::to_string).collect();
    if raw_net_whitelist.is_empty() {
      flags.allow_net = true;
    } else {
      flags.net_whitelist = resolve_hosts(raw_net_whitelist);
      debug!("net whitelist: {:#?}", &flags.net_whitelist);
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

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn upgrade() {
    let r =
      flags_from_vec_safe(svec!["deno", "upgrade", "--dry-run", "--force"]);
    let flags = r.unwrap();
    assert_eq!(
      flags,
      Flags {
        subcommand: DenoSubcommand::Upgrade {
          force: true,
          dry_run: true,
          version: None
        },
        ..Flags::default()
      }
    );
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
      Flags {
        subcommand: DenoSubcommand::Run {
          script: "script.ts".to_string(),
        },
        reload: true,
        ..Flags::default()
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
      Flags {
        reload: true,
        subcommand: DenoSubcommand::Run {
          script: "script.ts".to_string(),
        },
        allow_write: true,
        ..Flags::default()
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
      Flags {
        subcommand: DenoSubcommand::Run {
          script: "script.ts".to_string(),
        },
        v8_flags: Some(svec!["--help"]),
        ..Flags::default()
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
      Flags {
        subcommand: DenoSubcommand::Run {
          script: "script.ts".to_string(),
        },
        v8_flags: Some(svec!["--expose-gc", "--gc-stats=1"]),
        ..Flags::default()
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
      Flags {
        subcommand: DenoSubcommand::Run {
          script: "gist.ts".to_string(),
        },
        argv: svec!["--title", "X"],
        allow_net: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn allow_all() {
    let r = flags_from_vec_safe(svec!["deno", "run", "--allow-all", "gist.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run {
          script: "gist.ts".to_string(),
        },
        allow_net: true,
        allow_env: true,
        allow_run: true,
        allow_read: true,
        allow_write: true,
        allow_plugin: true,
        allow_hrtime: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn allow_read() {
    let r =
      flags_from_vec_safe(svec!["deno", "run", "--allow-read", "gist.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run {
          script: "gist.ts".to_string(),
        },
        allow_read: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn allow_hrtime() {
    let r =
      flags_from_vec_safe(svec!["deno", "run", "--allow-hrtime", "gist.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run {
          script: "gist.ts".to_string(),
        },
        allow_hrtime: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn double_hyphen() {
    // notice that flags passed after double dash will not
    // be parsed to Flags but instead forwarded to
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
      Flags {
        subcommand: DenoSubcommand::Run {
          script: "script.ts".to_string(),
        },
        argv: svec!["--", "-D", "--allow-net"],
        allow_write: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn fmt() {
    let r =
      flags_from_vec_safe(svec!["deno", "fmt", "script_1.ts", "script_2.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Fmt {
          check: false,
          files: vec!["script_1.ts".to_string(), "script_2.ts".to_string()]
        },
        ..Flags::default()
      }
    );

    let r = flags_from_vec_safe(svec!["deno", "fmt", "--check"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Fmt {
          check: true,
          files: vec![],
        },
        ..Flags::default()
      }
    );

    let r = flags_from_vec_safe(svec!["deno", "fmt"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Fmt {
          check: false,
          files: vec![],
        },
        ..Flags::default()
      }
    );
  }

  #[test]
  fn types() {
    let r = flags_from_vec_safe(svec!["deno", "types"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Types,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn types_unstable() {
    let r = flags_from_vec_safe(svec!["deno", "types", "--unstable"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        unstable: true,
        subcommand: DenoSubcommand::Types,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn cache() {
    let r = flags_from_vec_safe(svec!["deno", "cache", "script.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Cache {
          files: svec!["script.ts"],
        },
        ..Flags::default()
      }
    );
  }

  #[test]
  fn cache_unstable() {
    let r =
      flags_from_vec_safe(svec!["deno", "cache", "--unstable", "script.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        unstable: true,
        subcommand: DenoSubcommand::Cache {
          files: svec!["script.ts"],
        },
        ..Flags::default()
      }
    );
  }

  #[test]
  fn info() {
    let r = flags_from_vec_safe(svec!["deno", "info", "script.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Info {
          file: Some("script.ts".to_string()),
        },
        ..Flags::default()
      }
    );

    let r = flags_from_vec_safe(svec!["deno", "info"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Info { file: None },
        ..Flags::default()
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
      Flags {
        subcommand: DenoSubcommand::Run {
          script: "script.ts".to_string(),
        },
        config_path: Some("tsconfig.json".to_owned()),
        ..Flags::default()
      }
    );
  }

  #[test]
  fn eval() {
    let r =
      flags_from_vec_safe(svec!["deno", "eval", "'console.log(\"hello\")'"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Eval {
          print: false,
          code: "'console.log(\"hello\")'".to_string(),
          as_typescript: false,
        },
        allow_net: true,
        allow_env: true,
        allow_run: true,
        allow_read: true,
        allow_write: true,
        allow_plugin: true,
        allow_hrtime: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn eval_p() {
    let r = flags_from_vec_safe(svec!["deno", "eval", "-p", "1+2"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Eval {
          print: true,
          code: "1+2".to_string(),
          as_typescript: false,
        },
        allow_net: true,
        allow_env: true,
        allow_run: true,
        allow_read: true,
        allow_write: true,
        allow_plugin: true,
        allow_hrtime: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn eval_unstable() {
    let r = flags_from_vec_safe(svec![
      "deno",
      "eval",
      "--unstable",
      "'console.log(\"hello\")'"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        unstable: true,
        subcommand: DenoSubcommand::Eval {
          print: false,
          code: "'console.log(\"hello\")'".to_string(),
          as_typescript: false,
        },
        allow_net: true,
        allow_env: true,
        allow_run: true,
        allow_read: true,
        allow_write: true,
        allow_plugin: true,
        allow_hrtime: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn eval_typescript() {
    let r = flags_from_vec_safe(svec![
      "deno",
      "eval",
      "-T",
      "'console.log(\"hello\")'"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Eval {
          print: false,
          code: "'console.log(\"hello\")'".to_string(),
          as_typescript: true,
        },
        allow_net: true,
        allow_env: true,
        allow_run: true,
        allow_read: true,
        allow_write: true,
        allow_plugin: true,
        allow_hrtime: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn eval_with_v8_flags() {
    let r =
      flags_from_vec_safe(svec!["deno", "eval", "--v8-flags=--help", "42"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Eval {
          print: false,
          code: "42".to_string(),
          as_typescript: false,
        },
        v8_flags: Some(svec!["--help"]),
        allow_net: true,
        allow_env: true,
        allow_run: true,
        allow_read: true,
        allow_write: true,
        allow_plugin: true,
        allow_hrtime: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn repl() {
    let r = flags_from_vec_safe(svec!["deno"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Repl,
        allow_net: true,
        allow_env: true,
        allow_run: true,
        allow_read: true,
        allow_write: true,
        allow_plugin: true,
        allow_hrtime: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn repl_unstable() {
    let r = flags_from_vec_safe(svec!["deno", "repl", "--unstable"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        unstable: true,
        subcommand: DenoSubcommand::Repl,
        allow_net: true,
        allow_env: true,
        allow_run: true,
        allow_read: true,
        allow_write: true,
        allow_plugin: true,
        allow_hrtime: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn allow_read_whitelist() {
    use tempfile::TempDir;
    let temp_dir = TempDir::new().expect("tempdir fail").path().to_path_buf();

    let r = flags_from_vec_safe(svec![
      "deno",
      "run",
      format!("--allow-read=.,{}", temp_dir.to_str().unwrap()),
      "script.ts"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        allow_read: false,
        read_whitelist: vec![PathBuf::from("."), temp_dir],
        subcommand: DenoSubcommand::Run {
          script: "script.ts".to_string(),
        },
        ..Flags::default()
      }
    );
  }

  #[test]
  fn allow_write_whitelist() {
    use tempfile::TempDir;
    let temp_dir = TempDir::new().expect("tempdir fail").path().to_path_buf();

    let r = flags_from_vec_safe(svec![
      "deno",
      "run",
      format!("--allow-write=.,{}", temp_dir.to_str().unwrap()),
      "script.ts"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        allow_write: false,
        write_whitelist: vec![PathBuf::from("."), temp_dir],
        subcommand: DenoSubcommand::Run {
          script: "script.ts".to_string(),
        },
        ..Flags::default()
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
      Flags {
        subcommand: DenoSubcommand::Run {
          script: "script.ts".to_string(),
        },
        allow_net: false,
        net_whitelist: svec!["127.0.0.1"],
        ..Flags::default()
      }
    );
  }

  #[test]
  fn bundle() {
    let r = flags_from_vec_safe(svec!["deno", "bundle", "source.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Bundle {
          source_file: "source.ts".to_string(),
          out_file: None,
        },
        ..Flags::default()
      }
    );
  }

  #[test]
  fn bundle_unstable() {
    let r =
      flags_from_vec_safe(svec!["deno", "bundle", "--unstable", "source.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        unstable: true,
        subcommand: DenoSubcommand::Bundle {
          source_file: "source.ts".to_string(),
          out_file: None,
        },
        ..Flags::default()
      }
    );
  }

  #[test]
  fn bundle_with_config() {
    let r = flags_from_vec_safe(svec![
      "deno",
      "bundle",
      "--config",
      "tsconfig.json",
      "source.ts",
      "bundle.js"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Bundle {
          source_file: "source.ts".to_string(),
          out_file: Some(PathBuf::from("bundle.js")),
        },
        allow_write: true,
        config_path: Some("tsconfig.json".to_owned()),
        ..Flags::default()
      }
    );
  }

  #[test]
  fn bundle_with_output() {
    let r =
      flags_from_vec_safe(svec!["deno", "bundle", "source.ts", "bundle.js"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Bundle {
          source_file: "source.ts".to_string(),
          out_file: Some(PathBuf::from("bundle.js")),
        },
        allow_write: true,
        ..Flags::default()
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
      Flags {
        subcommand: DenoSubcommand::Run {
          script: "script.ts".to_string(),
        },
        import_map_path: Some("importmap.json".to_owned()),
        ..Flags::default()
      }
    );
  }

  #[test]
  fn cache_importmap() {
    let r = flags_from_vec_safe(svec![
      "deno",
      "cache",
      "--importmap=importmap.json",
      "script.ts"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Cache {
          files: svec!["script.ts"],
        },
        import_map_path: Some("importmap.json".to_owned()),
        ..Flags::default()
      }
    );
  }

  #[test]
  fn cache_multiple() {
    let r =
      flags_from_vec_safe(svec!["deno", "cache", "script.ts", "script_two.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Cache {
          files: svec!["script.ts", "script_two.ts"],
        },
        ..Flags::default()
      }
    );
  }

  #[test]
  fn run_seed() {
    let r =
      flags_from_vec_safe(svec!["deno", "run", "--seed", "250", "script.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run {
          script: "script.ts".to_string(),
        },
        seed: Some(250 as u64),
        v8_flags: Some(svec!["--random-seed=250"]),
        ..Flags::default()
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
      Flags {
        subcommand: DenoSubcommand::Run {
          script: "script.ts".to_string(),
        },
        seed: Some(250 as u64),
        v8_flags: Some(svec!["--expose-gc", "--random-seed=250"]),
        ..Flags::default()
      }
    );
  }

  #[test]
  fn install() {
    let r = flags_from_vec_safe(svec![
      "deno",
      "install",
      "https://deno.land/std/examples/colors.ts"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Install {
          name: None,
          module_url: "https://deno.land/std/examples/colors.ts".to_string(),
          args: vec![],
          root: None,
          force: false,
        },
        ..Flags::default()
      }
    );
  }

  #[test]
  fn install_unstable() {
    let r = flags_from_vec_safe(svec![
      "deno",
      "install",
      "--unstable",
      "https://deno.land/std/examples/colors.ts"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        unstable: true,
        subcommand: DenoSubcommand::Install {
          name: None,
          module_url: "https://deno.land/std/examples/colors.ts".to_string(),
          args: svec![],
          root: None,
          force: false,
        },
        ..Flags::default()
      }
    );
  }

  #[test]
  fn install_with_args() {
    let r = flags_from_vec_safe(svec![
      "deno",
      "install",
      "--allow-net",
      "--allow-read",
      "-n",
      "file_server",
      "https://deno.land/std/http/file_server.ts"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Install {
          name: Some("file_server".to_string()),
          module_url: "https://deno.land/std/http/file_server.ts".to_string(),
          args: vec![],
          root: None,
          force: false,
        },
        allow_net: true,
        allow_read: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn install_with_args_and_dir_and_force() {
    let r = flags_from_vec_safe(svec![
      "deno",
      "install",
      "--root",
      "/usr/local",
      "-f",
      "--allow-net",
      "--allow-read",
      "-n",
      "file_server",
      "https://deno.land/std/http/file_server.ts",
      "arg1",
      "arg2"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Install {
          name: Some("file_server".to_string()),
          module_url: "https://deno.land/std/http/file_server.ts".to_string(),
          args: svec!["arg1", "arg2"],
          root: Some(PathBuf::from("/usr/local")),
          force: true,
        },
        allow_net: true,
        allow_read: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn log_level() {
    let r = flags_from_vec_safe(svec![
      "deno",
      "run",
      "--log-level=debug",
      "script.ts"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run {
          script: "script.ts".to_string(),
        },
        log_level: Some(Level::Debug),
        ..Flags::default()
      }
    );
  }

  #[test]
  fn quiet() {
    let r = flags_from_vec_safe(svec!["deno", "run", "-q", "script.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run {
          script: "script.ts".to_string(),
        },
        log_level: Some(Level::Error),
        ..Flags::default()
      }
    );
  }

  #[test]
  fn completions() {
    let r = flags_from_vec_safe(svec!["deno", "completions", "bash"]).unwrap();

    match r.subcommand {
      DenoSubcommand::Completions { buf } => assert!(!buf.is_empty()),
      _ => unreachable!(),
    }
  }

  /* TODO(ry) Fix this test
  #[test]
  fn test_flags_from_vec_33() {
    let (flags, subcommand, argv) =
      flags_from_vec_safe(svec!["deno", "script.ts", "--allow-read", "--allow-net"]);
    assert_eq!(
      flags,
      Flags {
        allow_net: true,
        allow_read: true,
        ..Flags::default()
      }
    );
    assert_eq!(subcommand, DenoSubcommand::Run);
    assert_eq!(argv, svec!["script.ts"]);

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
      Flags {
        allow_net: true,
        allow_read: true,
        reload: true,
        ..Flags::default()
      }
    );
    assert_eq!(subcommand, DenoSubcommand::Run);
    assert_eq!(argv, svec!["deno", "script.ts", "--help", "--foo", "bar"]);

    let (flags, subcommand, argv) =
      flags_from_vec_safe(svec!["deno""script.ts", "foo", "bar"]);
    assert_eq!(flags, Flags::default());
    assert_eq!(subcommand, DenoSubcommand::Run);
  assert_eq!(argv, svec!["script.ts", "foo", "bar"]);

    let (flags, subcommand, argv) =
      flags_from_vec_safe(svec!["deno""script.ts", "-"]);
    assert_eq!(flags, Flags::default());
    assert_eq!(subcommand, DenoSubcommand::Run);
    assert_eq!(argv, svec!["script.ts", "-"]);

    let (flags, subcommand, argv) =
      flags_from_vec_safe(svec!["deno""script.ts", "-", "foo", "bar"]);
    assert_eq!(flags, Flags::default());
    assert_eq!(subcommand, DenoSubcommand::Run);
    assert_eq!(argv, svec!["script.ts", "-", "foo", "bar"]);
  }
  */

  #[test]
  fn no_remote() {
    let r =
      flags_from_vec_safe(svec!["deno", "run", "--no-remote", "script.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run {
          script: "script.ts".to_string(),
        },
        no_remote: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn cached_only() {
    let r =
      flags_from_vec_safe(svec!["deno", "run", "--cached-only", "script.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run {
          script: "script.ts".to_string(),
        },
        cached_only: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn allow_net_whitelist_with_ports() {
    let r = flags_from_vec_safe(svec![
      "deno",
      "run",
      "--allow-net=deno.land,:8000,:4545",
      "script.ts"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run {
          script: "script.ts".to_string(),
        },
        net_whitelist: svec![
          "deno.land",
          "0.0.0.0:8000",
          "127.0.0.1:8000",
          "localhost:8000",
          "0.0.0.0:4545",
          "127.0.0.1:4545",
          "localhost:4545"
        ],
        ..Flags::default()
      }
    );
  }

  #[test]
  fn lock_write() {
    let r = flags_from_vec_safe(svec![
      "deno",
      "run",
      "--lock-write",
      "--lock=lock.json",
      "script.ts"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run {
          script: "script.ts".to_string(),
        },
        lock_write: true,
        lock: Some("lock.json".to_string()),
        ..Flags::default()
      }
    );
  }

  #[test]
  fn test_with_allow_net() {
    let r = flags_from_vec_safe(svec![
      "deno",
      "test",
      "--allow-net",
      "--allow-none",
      "dir1/",
      "dir2/"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Test {
          fail_fast: false,
          filter: None,
          allow_none: true,
          quiet: false,
          include: Some(svec!["dir1/", "dir2/"]),
        },
        allow_net: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn test_filter() {
    let r = flags_from_vec_safe(svec!["deno", "test", "--filter=foo", "dir1"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Test {
          fail_fast: false,
          allow_none: false,
          quiet: false,
          filter: Some("foo".to_string()),
          include: Some(svec!["dir1"]),
        },
        ..Flags::default()
      }
    );
  }

  #[test]
  fn run_with_cafile() {
    let r = flags_from_vec_safe(svec![
      "deno",
      "run",
      "--cert",
      "example.crt",
      "script.ts"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run {
          script: "script.ts".to_string(),
        },
        ca_file: Some("example.crt".to_owned()),
        ..Flags::default()
      }
    );
  }

  #[test]
  fn bundle_with_cafile() {
    let r = flags_from_vec_safe(svec![
      "deno",
      "bundle",
      "--cert",
      "example.crt",
      "source.ts"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Bundle {
          source_file: "source.ts".to_string(),
          out_file: None,
        },
        ca_file: Some("example.crt".to_owned()),
        ..Flags::default()
      }
    );
  }

  #[test]
  fn eval_with_cafile() {
    let r = flags_from_vec_safe(svec![
      "deno",
      "eval",
      "--cert",
      "example.crt",
      "console.log('hello world')"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Eval {
          print: false,
          code: "console.log('hello world')".to_string(),
          as_typescript: false,
        },
        ca_file: Some("example.crt".to_owned()),
        allow_net: true,
        allow_env: true,
        allow_run: true,
        allow_read: true,
        allow_write: true,
        allow_plugin: true,
        allow_hrtime: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn eval_with_inspect() {
    let r = flags_from_vec_safe(svec![
      "deno",
      "eval",
      "--inspect",
      "const foo = 'bar'"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Eval {
          print: false,
          code: "const foo = 'bar'".to_string(),
          as_typescript: false,
        },
        inspect: Some("127.0.0.1:9229".parse().unwrap()),
        allow_net: true,
        allow_env: true,
        allow_run: true,
        allow_read: true,
        allow_write: true,
        allow_plugin: true,
        allow_hrtime: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn cache_with_cafile() {
    let r = flags_from_vec_safe(svec![
      "deno",
      "cache",
      "--cert",
      "example.crt",
      "script.ts",
      "script_two.ts"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Cache {
          files: svec!["script.ts", "script_two.ts"],
        },
        ca_file: Some("example.crt".to_owned()),
        ..Flags::default()
      }
    );
  }

  #[test]
  fn info_with_cafile() {
    let r = flags_from_vec_safe(svec![
      "deno",
      "info",
      "--cert",
      "example.crt",
      "https://example.com"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Info {
          file: Some("https://example.com".to_string()),
        },
        ca_file: Some("example.crt".to_owned()),
        ..Flags::default()
      }
    );
  }

  #[test]
  fn install_with_cafile() {
    let r = flags_from_vec_safe(svec![
      "deno",
      "install",
      "--cert",
      "example.crt",
      "-n",
      "deno_colors",
      "https://deno.land/std/examples/colors.ts"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Install {
          name: Some("deno_colors".to_string()),
          module_url: "https://deno.land/std/examples/colors.ts".to_string(),
          args: vec![],
          root: None,
          force: false,
        },
        ca_file: Some("example.crt".to_owned()),
        ..Flags::default()
      }
    );
  }

  #[test]
  fn repl_with_cafile() {
    let r = flags_from_vec_safe(svec!["deno", "repl", "--cert", "example.crt"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Repl {},
        ca_file: Some("example.crt".to_owned()),
        allow_read: true,
        allow_write: true,
        allow_net: true,
        allow_env: true,
        allow_run: true,
        allow_plugin: true,
        allow_hrtime: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn repl_with_inspect() {
    let r = flags_from_vec_safe(svec!["deno", "repl", "--inspect"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Repl {},
        inspect: Some("127.0.0.1:9229".parse().unwrap()),
        allow_read: true,
        allow_write: true,
        allow_net: true,
        allow_env: true,
        allow_run: true,
        allow_plugin: true,
        allow_hrtime: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn doc() {
    let r =
      flags_from_vec_safe(svec!["deno", "doc", "--json", "path/to/module.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Doc {
          json: true,
          source_file: Some("path/to/module.ts".to_string()),
          filter: None,
        },
        ..Flags::default()
      }
    );

    let r = flags_from_vec_safe(svec![
      "deno",
      "doc",
      "path/to/module.ts",
      "SomeClass.someField"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Doc {
          json: false,
          source_file: Some("path/to/module.ts".to_string()),
          filter: Some("SomeClass.someField".to_string()),
        },
        ..Flags::default()
      }
    );

    let r = flags_from_vec_safe(svec!["deno", "doc"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Doc {
          json: false,
          source_file: None,
          filter: None,
        },
        ..Flags::default()
      }
    );

    let r =
      flags_from_vec_safe(svec!["deno", "doc", "--builtin", "Deno.Listener"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Doc {
          json: false,
          source_file: Some("--builtin".to_string()),
          filter: Some("Deno.Listener".to_string()),
        },
        ..Flags::default()
      }
    );
  }

  #[test]
  fn inspect_default_host() {
    let r = flags_from_vec_safe(svec!["deno", "run", "--inspect", "foo.js"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run {
          script: "foo.js".to_string(),
        },
        inspect: Some("127.0.0.1:9229".parse().unwrap()),
        ..Flags::default()
      }
    );
  }
}
