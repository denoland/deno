// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use clap::App;
use clap::AppSettings;
use clap::Arg;
use clap::ArgMatches;
use clap::ArgSettings;
use clap::SubCommand;
use log::Level;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::str::FromStr;

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
    private: bool,
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
    files: Vec<PathBuf>,
    ignore: Vec<PathBuf>,
  },
  Info {
    json: bool,
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
    files: Vec<PathBuf>,
    ignore: Vec<PathBuf>,
    rules: bool,
    json: bool,
  },
  Repl,
  Run {
    script: String,
  },
  Test {
    no_run: bool,
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
    output: Option<PathBuf>,
    ca_file: Option<String>,
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
  pub allow_read: bool,
  pub allow_run: bool,
  pub allow_write: bool,
  pub cache_blocklist: Vec<String>,
  pub ca_file: Option<String>,
  pub cached_only: bool,
  pub config_path: Option<String>,
  pub coverage: bool,
  pub ignore: Vec<PathBuf>,
  pub import_map_path: Option<String>,
  pub inspect: Option<SocketAddr>,
  pub inspect_brk: Option<SocketAddr>,
  pub lock: Option<PathBuf>,
  pub lock_write: bool,
  pub log_level: Option<Level>,
  pub net_allowlist: Vec<String>,
  pub no_check: bool,
  pub no_prompts: bool,
  pub no_remote: bool,
  pub read_allowlist: Vec<PathBuf>,
  pub reload: bool,
  pub repl: bool,
  pub seed: Option<u64>,
  pub unstable: bool,
  pub v8_flags: Option<Vec<String>>,
  pub version: bool,
  pub watch: bool,
  pub write_allowlist: Vec<PathBuf>,
}

fn join_paths(allowlist: &[PathBuf], d: &str) -> String {
  allowlist
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

    if !self.read_allowlist.is_empty() {
      let s = format!("--allow-read={}", join_paths(&self.read_allowlist, ","));
      args.push(s);
    }

    if self.allow_read {
      args.push("--allow-read".to_string());
    }

    if !self.write_allowlist.is_empty() {
      let s =
        format!("--allow-write={}", join_paths(&self.write_allowlist, ","));
      args.push(s);
    }

    if self.allow_write {
      args.push("--allow-write".to_string());
    }

    if !self.net_allowlist.is_empty() {
      let s = format!("--allow-net={}", self.net_allowlist.join(","));
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

    if self.allow_hrtime {
      args.push("--allow-hrtime".to_string());
    }

    args
  }
}

static ENV_VARIABLES_HELP: &str = "ENVIRONMENT VARIABLES:
    DENO_DIR             Set the cache directory
    DENO_INSTALL_ROOT    Set deno install's output directory
                         (defaults to $HOME/.deno/bin)
    DENO_CERT            Load certificate authority from PEM encoded file
    NO_COLOR             Set to disable color
    HTTP_PROXY           Proxy address for HTTP requests
                         (module downloads, fetch)
    HTTPS_PROXY          Proxy address for HTTPS requests
                         (module downloads, fetch)
    NO_PROXY             Comma-separated list of hosts which do not use a proxy
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
    "{} ({}, {})\nv8 {}\ntypescript {}",
    crate::version::deno(),
    env!("PROFILE"),
    env!("TARGET"),
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
  let version = crate::version::deno();
  let app = clap_root(&*version);
  let matches = app.get_matches_from_safe(args)?;

  let mut flags = Flags::default();

  if matches.is_present("unstable") {
    flags.unstable = true;
  }
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

fn clap_root<'a, 'b>(version: &'b str) -> App<'a, 'b> {
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
    .version(version)
    .long_version(LONG_VERSION.as_str())
    .arg(
      Arg::with_name("unstable")
        .long("unstable")
        .help("Enable unstable features and APIs")
        .global(true),
    )
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

fn types_parse(flags: &mut Flags, _matches: &clap::ArgMatches) {
  flags.subcommand = DenoSubcommand::Types;
}

fn fmt_parse(flags: &mut Flags, matches: &clap::ArgMatches) {
  flags.watch = matches.is_present("watch");
  let files = match matches.values_of("files") {
    Some(f) => f.map(PathBuf::from).collect(),
    None => vec![],
  };
  let ignore = match matches.values_of("ignore") {
    Some(f) => f.map(PathBuf::from).collect(),
    None => vec![],
  };
  flags.subcommand = DenoSubcommand::Fmt {
    check: matches.is_present("check"),
    files,
    ignore,
  }
}

fn install_parse(flags: &mut Flags, matches: &clap::ArgMatches) {
  runtime_args_parse(flags, matches, true);

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
  compile_args_parse(flags, matches);

  let source_file = matches.value_of("source_file").unwrap().to_string();

  let out_file = if let Some(out_file) = matches.value_of("out_file") {
    flags.allow_write = true;
    Some(PathBuf::from(out_file))
  } else {
    None
  };

  flags.watch = matches.is_present("watch");

  flags.subcommand = DenoSubcommand::Bundle {
    source_file,
    out_file,
  };
}

fn completions_parse(flags: &mut Flags, matches: &clap::ArgMatches) {
  let shell: &str = matches.value_of("shell").unwrap();
  let mut buf: Vec<u8> = vec![];
  clap_root(&*crate::version::deno()).gen_completions_to(
    "deno",
    clap::Shell::from_str(shell).unwrap(),
    &mut buf,
  );

  flags.subcommand = DenoSubcommand::Completions {
    buf: buf.into_boxed_slice(),
  };
}

fn repl_parse(flags: &mut Flags, matches: &clap::ArgMatches) {
  runtime_args_parse(flags, matches, false);
  flags.repl = true;
  flags.subcommand = DenoSubcommand::Repl;
  flags.allow_net = true;
  flags.allow_env = true;
  flags.allow_run = true;
  flags.allow_read = true;
  flags.allow_write = true;
  flags.allow_hrtime = true;
}

fn eval_parse(flags: &mut Flags, matches: &clap::ArgMatches) {
  runtime_args_parse(flags, matches, false);
  flags.allow_net = true;
  flags.allow_env = true;
  flags.allow_run = true;
  flags.allow_read = true;
  flags.allow_write = true;
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
  reload_arg_parse(flags, matches);
  import_map_arg_parse(flags, matches);
  ca_file_arg_parse(flags, matches);
  let json = matches.is_present("json");
  flags.subcommand = DenoSubcommand::Info {
    file: matches.value_of("file").map(|f| f.to_string()),
    json,
  };
}

fn cache_parse(flags: &mut Flags, matches: &clap::ArgMatches) {
  compile_args_parse(flags, matches);
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
    flags.lock = Some(PathBuf::from(lockfile));
  }
  if matches.is_present("lock-write") {
    flags.lock_write = true;
  }
}

fn compile_args<'a, 'b>(app: App<'a, 'b>) -> App<'a, 'b> {
  app
    .arg(import_map_arg())
    .arg(no_remote_arg())
    .arg(config_arg())
    .arg(no_check_arg())
    .arg(reload_arg())
    .arg(lock_arg())
    .arg(lock_write_arg())
    .arg(ca_file_arg())
}

fn compile_args_parse(flags: &mut Flags, matches: &clap::ArgMatches) {
  import_map_arg_parse(flags, matches);
  no_remote_arg_parse(flags, matches);
  config_arg_parse(flags, matches);
  no_check_arg_parse(flags, matches);
  reload_arg_parse(flags, matches);
  lock_args_parse(flags, matches);
  ca_file_arg_parse(flags, matches);
}

fn runtime_args<'a, 'b>(app: App<'a, 'b>, include_perms: bool) -> App<'a, 'b> {
  let app = inspect_args(compile_args(app));
  let app = if include_perms {
    permission_args(app)
  } else {
    app
  };
  app
    .arg(cached_only_arg())
    .arg(v8_flags_arg())
    .arg(seed_arg())
}

fn runtime_args_parse(
  flags: &mut Flags,
  matches: &clap::ArgMatches,
  include_perms: bool,
) {
  compile_args_parse(flags, matches);
  cached_only_arg_parse(flags, matches);
  if include_perms {
    permission_args_parse(flags, matches);
  }
  v8_flags_arg_parse(flags, matches);
  seed_arg_parse(flags, matches);
  inspect_arg_parse(flags, matches);
}

fn run_parse(flags: &mut Flags, matches: &clap::ArgMatches) {
  runtime_args_parse(flags, matches, true);

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

  flags.watch = matches.is_present("watch");
  flags.subcommand = DenoSubcommand::Run { script };
}

fn test_parse(flags: &mut Flags, matches: &clap::ArgMatches) {
  runtime_args_parse(flags, matches, true);

  let no_run = matches.is_present("no-run");
  let fail_fast = matches.is_present("fail-fast");
  let allow_none = matches.is_present("allow-none");
  let quiet = matches.is_present("quiet");
  let filter = matches.value_of("filter").map(String::from);
  let coverage = matches.is_present("coverage");

  if coverage {
    flags.coverage = true;
  }

  if matches.is_present("script_arg") {
    let script_arg: Vec<String> = matches
      .values_of("script_arg")
      .unwrap()
      .map(String::from)
      .collect();

    for v in script_arg {
      flags.argv.push(v);
    }
  }

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
    no_run,
    fail_fast,
    quiet,
    include,
    filter,
    allow_none,
  };
}

fn upgrade_parse(flags: &mut Flags, matches: &clap::ArgMatches) {
  ca_file_arg_parse(flags, matches);

  let dry_run = matches.is_present("dry-run");
  let force = matches.is_present("force");
  let version = matches.value_of("version").map(|s| s.to_string());
  let output = if matches.is_present("output") {
    let install_root = matches.value_of("output").unwrap();
    Some(PathBuf::from(install_root))
  } else {
    None
  };
  let ca_file = matches.value_of("cert").map(|s| s.to_string());
  flags.subcommand = DenoSubcommand::Upgrade {
    dry_run,
    force,
    version,
    output,
    ca_file,
  };
}

fn doc_parse(flags: &mut Flags, matches: &clap::ArgMatches) {
  import_map_arg_parse(flags, matches);
  reload_arg_parse(flags, matches);

  let source_file = matches.value_of("source_file").map(String::from);
  let private = matches.is_present("private");
  let json = matches.is_present("json");
  let filter = matches.value_of("filter").map(String::from);
  flags.subcommand = DenoSubcommand::Doc {
    source_file,
    json,
    filter,
    private,
  };
}

fn lint_parse(flags: &mut Flags, matches: &clap::ArgMatches) {
  let files = match matches.values_of("files") {
    Some(f) => f.map(PathBuf::from).collect(),
    None => vec![],
  };
  let ignore = match matches.values_of("ignore") {
    Some(f) => f.map(PathBuf::from).collect(),
    None => vec![],
  };
  let rules = matches.is_present("rules");
  let json = matches.is_present("json");
  flags.subcommand = DenoSubcommand::Lint {
    files,
    rules,
    ignore,
    json,
  };
}

fn types_subcommand<'a, 'b>() -> App<'a, 'b> {
  SubCommand::with_name("types")
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
        .help("Check if the source files are formatted")
        .takes_value(false),
    )
    .arg(
      Arg::with_name("ignore")
        .long("ignore")
        .takes_value(true)
        .use_delimiter(true)
        .require_equals(true)
        .help("Ignore formatting particular source files. Use with --unstable"),
    )
    .arg(
      Arg::with_name("files")
        .takes_value(true)
        .multiple(true)
        .required(false),
    )
    .arg(watch_arg())
}

fn repl_subcommand<'a, 'b>() -> App<'a, 'b> {
  runtime_args(SubCommand::with_name("repl"), false)
    .about("Read Eval Print Loop")
}

fn install_subcommand<'a, 'b>() -> App<'a, 'b> {
  runtime_args(SubCommand::with_name("install"), true)
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
  - If the resulting name has an '@...' suffix, strip it.

To change the installation root, use --root:
  deno install --allow-net --allow-read --root /usr/local https://deno.land/std/http/file_server.ts

The installation root is determined, in order of precedence:
  - --root option
  - DENO_INSTALL_ROOT environment variable
  - $HOME/.deno

These must be added to the path manually if required.")
}

fn bundle_subcommand<'a, 'b>() -> App<'a, 'b> {
  compile_args(SubCommand::with_name("bundle"))
    .arg(
      Arg::with_name("source_file")
        .takes_value(true)
        .required(true),
    )
    .arg(Arg::with_name("out_file").takes_value(true).required(false))
    .arg(watch_arg())
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
  runtime_args(SubCommand::with_name("eval"), false)
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
    .arg(reload_arg().requires("file"))
    .arg(ca_file_arg())
    // TODO(lucacasonato): remove for 2.0
    .arg(no_check_arg().hidden(true))
    .arg(import_map_arg())
    .arg(
      Arg::with_name("json")
        .long("json")
        .help("Outputs the information in JSON format")
        .takes_value(false),
    )
}

fn cache_subcommand<'a, 'b>() -> App<'a, 'b> {
  compile_args(SubCommand::with_name("cache"))
    .arg(
      Arg::with_name("file")
        .takes_value(true)
        .required(true)
        .min_values(1),
    )
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
and is used to replace the current executable.

If you want to not replace the current Deno executable but instead download an
update to a different location, use the --output flag
  deno upgrade --output $HOME/my_deno",
    )
    .arg(
      Arg::with_name("version")
        .long("version")
        .help("The version to upgrade to")
        .takes_value(true),
    )
    .arg(
      Arg::with_name("output")
        .long("output")
        .help("The path to output the updated version to")
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
    .arg(ca_file_arg())
}

fn doc_subcommand<'a, 'b>() -> App<'a, 'b> {
  SubCommand::with_name("doc")
    .about("Show documentation for a module")
    .long_about(
      "Show documentation for a module.

Output documentation to standard output:
    deno doc ./path/to/module.ts

Output private documentation to standard output:
    deno doc --private ./path/to/module.ts

Output documentation in JSON format:
    deno doc --json ./path/to/module.ts

Target a specific symbol:
    deno doc ./path/to/module.ts MyClass.someField

Show documentation for runtime built-ins:
    deno doc
    deno doc --builtin Deno.Listener",
    )
    .arg(import_map_arg())
    .arg(reload_arg())
    .arg(
      Arg::with_name("json")
        .long("json")
        .help("Output documentation in JSON format")
        .takes_value(false),
    )
    .arg(
      Arg::with_name("private")
        .long("private")
        .help("Output private documentation")
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
        .help("Dot separated path to symbol")
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
  deno lint --unstable
  deno lint --unstable myfile1.ts myfile2.js

Print result as JSON:
  deno lint --unstable --json

Read from stdin:
  cat file.ts | deno lint --unstable -
  cat file.ts | deno lint --unstable --json -

List available rules:
  deno lint --unstable --rules

Ignore diagnostics on the next line by preceding it with an ignore comment and
rule name:
  // deno-lint-ignore no-explicit-any

  // deno-lint-ignore require-await no-empty

Names of rules to ignore must be specified after ignore comment.

Ignore linting a file by adding an ignore comment at the top of the file:
  // deno-lint-ignore-file
",
    )
    .arg(
      Arg::with_name("rules")
        .long("rules")
        .help("List available rules"),
    )
    .arg(
      Arg::with_name("ignore")
        .long("ignore")
        .requires("unstable")
        .takes_value(true)
        .use_delimiter(true)
        .require_equals(true)
        .help("Ignore linting particular source files"),
    )
    .arg(
      Arg::with_name("json")
        .long("json")
        .help("Output lint result in JSON format")
        .takes_value(false),
    )
    .arg(
      Arg::with_name("files")
        .takes_value(true)
        .multiple(true)
        .required(false),
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
        .help("Allow network access")
        .validator(crate::flags_allow_net::validator),
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
}

fn run_subcommand<'a, 'b>() -> App<'a, 'b> {
  runtime_args(SubCommand::with_name("run"), true)
    .arg(watch_arg())
    .setting(AppSettings::TrailingVarArg)
    .arg(
        script_arg()
        .required(true)
    )
    .about("Run a program given a filename or url to the module. Use '-' as a filename to read from stdin.")
    .long_about(
	  "Run a program given a filename or url to the module.

By default all programs are run in sandbox without access to disk, network or
ability to spawn subprocesses.
  deno run https://deno.land/std/examples/welcome.ts

Grant all permissions:
  deno run -A https://deno.land/std/http/file_server.ts

Grant permission to read from disk and listen to network:
  deno run --allow-read --allow-net https://deno.land/std/http/file_server.ts

Grant permission to read allow-listed files from disk:
  deno run --allow-read=/etc https://deno.land/std/http/file_server.ts

Deno allows specifying the filename '-' to read the file from stdin.
  curl https://deno.land/std/examples/welcome.ts | target/debug/deno run -",
    )
}

fn test_subcommand<'a, 'b>() -> App<'a, 'b> {
  runtime_args(SubCommand::with_name("test"), true)
    .setting(AppSettings::TrailingVarArg)
    .arg(
      Arg::with_name("no-run")
        .long("no-run")
        .help("Cache test modules, but don't run tests")
        .takes_value(false)
        .requires("unstable"),
    )
    .arg(
      Arg::with_name("fail-fast")
        .long("fail-fast")
        .alias("failfast")
        .help("Stop on first error")
        .takes_value(false),
    )
    .arg(
      Arg::with_name("allow-none")
        .long("allow-none")
        .help("Don't return error code if no test files are found")
        .takes_value(false),
    )
    .arg(
      Arg::with_name("filter")
        .set(ArgSettings::AllowLeadingHyphen)
        .long("filter")
        .takes_value(true)
        .help("Run tests with this string or pattern in the test name"),
    )
    .arg(
      Arg::with_name("coverage")
        .long("coverage")
        .takes_value(false)
        .requires("unstable")
        .conflicts_with("inspect")
        .conflicts_with("inspect-brk")
        .help("Collect coverage information"),
    )
    .arg(
      Arg::with_name("files")
        .help("List of file names to run")
        .takes_value(true)
        .multiple(true),
    )
    .arg(script_arg().last(true))
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
    // NOTE: these defaults are provided
    // so `deno run --v8-flags=--help` works
    // without specifying file to run.
    .default_value_ifs(&[
      ("v8-flags", Some("--help"), "_"),
      ("v8-flags", Some("-help"), "_"),
    ])
    .help("Script arg")
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
    .requires("lock")
    .help("Write lock file (use with --lock)")
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
    .value_name("CACHE_BLOCKLIST")
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
    let raw_cache_blocklist: Vec<String> =
      cache_bl.map(ToString::to_string).collect();
    if raw_cache_blocklist.is_empty() {
      flags.reload = true;
    } else {
      flags.cache_blocklist = resolve_urls(raw_cache_blocklist);
      debug!("cache blocklist: {:#?}", &flags.cache_blocklist);
      flags.reload = false;
    }
  }
}

fn import_map_arg<'a, 'b>() -> Arg<'a, 'b> {
  Arg::with_name("import-map")
    .long("import-map")
    .alias("importmap")
    .value_name("FILE")
    .requires("unstable")
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

fn import_map_arg_parse(flags: &mut Flags, matches: &clap::ArgMatches) {
  flags.import_map_path = matches.value_of("import-map").map(ToOwned::to_owned);
}

fn v8_flags_arg<'a, 'b>() -> Arg<'a, 'b> {
  Arg::with_name("v8-flags")
    .long("v8-flags")
    .takes_value(true)
    .use_delimiter(true)
    .require_equals(true)
    .help("Set V8 command line options (for help: --v8-flags=--help)")
}

fn v8_flags_arg_parse(flags: &mut Flags, matches: &ArgMatches) {
  if let Some(v8_flags) = matches.values_of("v8-flags") {
    let s: Vec<String> = v8_flags.map(String::from).collect();
    flags.v8_flags = Some(s);
  }
}

fn watch_arg<'a, 'b>() -> Arg<'a, 'b> {
  Arg::with_name("watch")
    .requires("unstable")
    .long("watch")
    .conflicts_with("inspect")
    .conflicts_with("inspect-brk")
    .help("Watch for file changes and restart process automatically")
    .long_help(
      "Watch for file changes and restart process automatically.
Only local files from entry point module graph are watched.",
    )
}

fn seed_arg<'a, 'b>() -> Arg<'a, 'b> {
  Arg::with_name("seed")
    .long("seed")
    .value_name("NUMBER")
    .help("Seed Math.random()")
    .takes_value(true)
    .validator(|val: String| match val.parse::<u64>() {
      Ok(_) => Ok(()),
      Err(_) => Err("Seed should be a number".to_string()),
    })
}

fn seed_arg_parse(flags: &mut Flags, matches: &ArgMatches) {
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

fn cached_only_arg<'a, 'b>() -> Arg<'a, 'b> {
  Arg::with_name("cached-only")
    .long("cached-only")
    .help("Require that remote dependencies are already cached")
}

fn cached_only_arg_parse(flags: &mut Flags, matches: &ArgMatches) {
  if matches.is_present("cached-only") {
    flags.cached_only = true;
  }
}

fn no_check_arg<'a, 'b>() -> Arg<'a, 'b> {
  Arg::with_name("no-check")
    .long("no-check")
    .help("Skip type checking modules")
}

fn no_check_arg_parse(flags: &mut Flags, matches: &clap::ArgMatches) {
  if matches.is_present("no-check") {
    flags.no_check = true;
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
    let read_allowlist: Vec<PathBuf> = read_wl.map(PathBuf::from).collect();

    if read_allowlist.is_empty() {
      flags.allow_read = true;
    } else {
      flags.read_allowlist = read_allowlist;
    }
  }

  if let Some(write_wl) = matches.values_of("allow-write") {
    let write_allowlist: Vec<PathBuf> = write_wl.map(PathBuf::from).collect();

    if write_allowlist.is_empty() {
      flags.allow_write = true;
    } else {
      flags.write_allowlist = write_allowlist;
    }
  }

  if let Some(net_wl) = matches.values_of("allow-net") {
    let raw_net_allowlist: Vec<String> =
      net_wl.map(ToString::to_string).collect();
    if raw_net_allowlist.is_empty() {
      flags.allow_net = true;
    } else {
      flags.net_allowlist =
        crate::flags_allow_net::parse(raw_net_allowlist).unwrap();
      debug!("net allowlist: {:#?}", &flags.net_allowlist);
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
}

// TODO(ry) move this to utility module and add test.
/// Strips fragment part of URL. Panics on bad URL.
pub fn resolve_urls(urls: Vec<String>) -> Vec<String> {
  use deno_core::url::Url;
  let mut out: Vec<String> = vec![];
  for urlstr in urls.iter() {
    if let Ok(mut url) = Url::from_str(urlstr) {
      url.set_fragment(None);
      let mut full_url = String::from(url.as_str());
      if full_url.len() > 1 && full_url.ends_with('/') {
        full_url.pop();
      }
      out.push(full_url);
    } else {
      panic!("Bad Url: {}", urlstr);
    }
  }
  out
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn global_flags() {
    #[rustfmt::skip]
    let r = flags_from_vec_safe(svec!["deno", "--unstable", "--log-level", "debug", "--quiet", "run", "script.ts"]);
    let flags = r.unwrap();
    assert_eq!(
      flags,
      Flags {
        subcommand: DenoSubcommand::Run {
          script: "script.ts".to_string(),
        },
        unstable: true,
        log_level: Some(Level::Error),
        ..Flags::default()
      }
    );
    #[rustfmt::skip]
    let r2 = flags_from_vec_safe(svec!["deno", "run", "--unstable", "--log-level", "debug", "--quiet", "script.ts"]);
    let flags2 = r2.unwrap();
    assert_eq!(flags2, flags);
  }

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
          version: None,
          output: None,
          ca_file: None,
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
  fn run_watch() {
    let r = flags_from_vec_safe(svec![
      "deno",
      "run",
      "--unstable",
      "--watch",
      "script.ts"
    ]);
    let flags = r.unwrap();
    assert_eq!(
      flags,
      Flags {
        subcommand: DenoSubcommand::Run {
          script: "script.ts".to_string(),
        },
        watch: true,
        unstable: true,
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
    let r = flags_from_vec_safe(svec!["deno", "run", "--v8-flags=--help"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run {
          script: "_".to_string(),
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
          ignore: vec![],
          check: false,
          files: vec![
            PathBuf::from("script_1.ts"),
            PathBuf::from("script_2.ts")
          ],
        },
        ..Flags::default()
      }
    );

    let r = flags_from_vec_safe(svec!["deno", "fmt", "--check"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Fmt {
          ignore: vec![],
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
          ignore: vec![],
          check: false,
          files: vec![],
        },
        ..Flags::default()
      }
    );

    let r = flags_from_vec_safe(svec!["deno", "fmt", "--watch", "--unstable"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Fmt {
          ignore: vec![],
          check: false,
          files: vec![],
        },
        watch: true,
        unstable: true,
        ..Flags::default()
      }
    );

    let r = flags_from_vec_safe(svec![
      "deno",
      "fmt",
      "--check",
      "--watch",
      "--unstable",
      "foo.ts",
      "--ignore=bar.js"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Fmt {
          ignore: vec![PathBuf::from("bar.js")],
          check: true,
          files: vec![PathBuf::from("foo.ts")],
        },
        watch: true,
        unstable: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn lint() {
    let r = flags_from_vec_safe(svec![
      "deno",
      "lint",
      "--unstable",
      "script_1.ts",
      "script_2.ts"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Lint {
          files: vec![
            PathBuf::from("script_1.ts"),
            PathBuf::from("script_2.ts")
          ],
          rules: false,
          json: false,
          ignore: vec![],
        },
        unstable: true,
        ..Flags::default()
      }
    );

    let r = flags_from_vec_safe(svec![
      "deno",
      "lint",
      "--unstable",
      "--ignore=script_1.ts,script_2.ts"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Lint {
          files: vec![],
          rules: false,
          json: false,
          ignore: vec![
            PathBuf::from("script_1.ts"),
            PathBuf::from("script_2.ts")
          ],
        },
        unstable: true,
        ..Flags::default()
      }
    );

    let r = flags_from_vec_safe(svec!["deno", "lint", "--unstable", "--rules"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Lint {
          files: vec![],
          rules: true,
          json: false,
          ignore: vec![],
        },
        unstable: true,
        ..Flags::default()
      }
    );

    let r = flags_from_vec_safe(svec![
      "deno",
      "lint",
      "--unstable",
      "--json",
      "script_1.ts"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Lint {
          files: vec![PathBuf::from("script_1.ts")],
          rules: false,
          json: true,
          ignore: vec![],
        },
        unstable: true,
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
  fn info() {
    let r = flags_from_vec_safe(svec!["deno", "info", "script.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Info {
          json: false,
          file: Some("script.ts".to_string()),
        },
        ..Flags::default()
      }
    );

    let r = flags_from_vec_safe(svec!["deno", "info", "--reload", "script.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Info {
          json: false,
          file: Some("script.ts".to_string()),
        },
        reload: true,
        ..Flags::default()
      }
    );

    let r = flags_from_vec_safe(svec!["deno", "info", "--json", "script.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Info {
          json: true,
          file: Some("script.ts".to_string()),
        },
        ..Flags::default()
      }
    );

    let r = flags_from_vec_safe(svec!["deno", "info"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Info {
          json: false,
          file: None
        },
        ..Flags::default()
      }
    );

    let r = flags_from_vec_safe(svec!["deno", "info", "--json"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Info {
          json: true,
          file: None
        },
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
        allow_hrtime: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn eval_with_flags() {
    #[rustfmt::skip]
    let r = flags_from_vec_safe(svec!["deno", "eval", "--unstable", "--import-map", "import_map.json", "--no-remote", "--config", "tsconfig.json", "--no-check", "--reload", "--lock", "lock.json", "--lock-write", "--cert", "example.crt", "--cached-only", "--v8-flags=--help", "--seed", "1", "--inspect=127.0.0.1:9229", "42"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Eval {
          print: false,
          code: "42".to_string(),
          as_typescript: false,
        },
        unstable: true,
        import_map_path: Some("import_map.json".to_string()),
        no_remote: true,
        config_path: Some("tsconfig.json".to_string()),
        no_check: true,
        reload: true,
        lock: Some(PathBuf::from("lock.json")),
        lock_write: true,
        ca_file: Some("example.crt".to_string()),
        cached_only: true,
        v8_flags: Some(svec!["--help", "--random-seed=1"]),
        seed: Some(1),
        inspect: Some("127.0.0.1:9229".parse().unwrap()),
        allow_net: true,
        allow_env: true,
        allow_run: true,
        allow_read: true,
        allow_write: true,
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
        repl: true,
        subcommand: DenoSubcommand::Repl,
        allow_net: true,
        allow_env: true,
        allow_run: true,
        allow_read: true,
        allow_write: true,
        allow_hrtime: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn repl_with_flags() {
    #[rustfmt::skip]
    let r = flags_from_vec_safe(svec!["deno", "repl", "--unstable", "--import-map", "import_map.json", "--no-remote", "--config", "tsconfig.json", "--no-check", "--reload", "--lock", "lock.json", "--lock-write", "--cert", "example.crt", "--cached-only", "--v8-flags=--help", "--seed", "1", "--inspect=127.0.0.1:9229"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        repl: true,
        subcommand: DenoSubcommand::Repl,
        unstable: true,
        import_map_path: Some("import_map.json".to_string()),
        no_remote: true,
        config_path: Some("tsconfig.json".to_string()),
        no_check: true,
        reload: true,
        lock: Some(PathBuf::from("lock.json")),
        lock_write: true,
        ca_file: Some("example.crt".to_string()),
        cached_only: true,
        v8_flags: Some(svec!["--help", "--random-seed=1"]),
        seed: Some(1),
        inspect: Some("127.0.0.1:9229".parse().unwrap()),
        allow_net: true,
        allow_env: true,
        allow_run: true,
        allow_read: true,
        allow_write: true,
        allow_hrtime: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn allow_read_allowlist() {
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
        read_allowlist: vec![PathBuf::from("."), temp_dir],
        subcommand: DenoSubcommand::Run {
          script: "script.ts".to_string(),
        },
        ..Flags::default()
      }
    );
  }

  #[test]
  fn allow_write_allowlist() {
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
        write_allowlist: vec![PathBuf::from("."), temp_dir],
        subcommand: DenoSubcommand::Run {
          script: "script.ts".to_string(),
        },
        ..Flags::default()
      }
    );
  }

  #[test]
  fn allow_net_allowlist() {
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
        net_allowlist: svec!["127.0.0.1"],
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
  fn bundle_with_config() {
    let r = flags_from_vec_safe(svec![
      "deno",
      "bundle",
      "--no-remote",
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
        no_remote: true,
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
  fn bundle_with_lock() {
    let r = flags_from_vec_safe(svec![
      "deno",
      "bundle",
      "--lock-write",
      "--lock=lock.json",
      "source.ts"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Bundle {
          source_file: "source.ts".to_string(),
          out_file: None,
        },
        lock_write: true,
        lock: Some(PathBuf::from("lock.json")),
        ..Flags::default()
      }
    );
  }

  #[test]
  fn bundle_with_reload() {
    let r =
      flags_from_vec_safe(svec!["deno", "bundle", "--reload", "source.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        reload: true,
        subcommand: DenoSubcommand::Bundle {
          source_file: "source.ts".to_string(),
          out_file: None,
        },
        ..Flags::default()
      }
    );
  }

  #[test]
  fn bundle_nocheck() {
    let r =
      flags_from_vec_safe(svec!["deno", "bundle", "--no-check", "script.ts"])
        .unwrap();
    assert_eq!(
      r,
      Flags {
        subcommand: DenoSubcommand::Bundle {
          source_file: "script.ts".to_string(),
          out_file: None,
        },
        no_check: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn bundle_watch() {
    let r = flags_from_vec_safe(svec![
      "deno",
      "bundle",
      "--watch",
      "--unstable",
      "source.ts"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Bundle {
          source_file: "source.ts".to_string(),
          out_file: None,
        },
        watch: true,
        unstable: true,
        ..Flags::default()
      }
    )
  }

  #[test]
  fn run_import_map() {
    let r = flags_from_vec_safe(svec![
      "deno",
      "run",
      "--unstable",
      "--import-map=import_map.json",
      "script.ts"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run {
          script: "script.ts".to_string(),
        },
        unstable: true,
        import_map_path: Some("import_map.json".to_owned()),
        ..Flags::default()
      }
    );
  }

  #[test]
  fn info_import_map() {
    let r = flags_from_vec_safe(svec![
      "deno",
      "info",
      "--unstable",
      "--import-map=import_map.json",
      "script.ts"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Info {
          file: Some("script.ts".to_string()),
          json: false,
        },
        unstable: true,
        import_map_path: Some("import_map.json".to_owned()),
        ..Flags::default()
      }
    );
  }

  #[test]
  fn cache_import_map() {
    let r = flags_from_vec_safe(svec![
      "deno",
      "cache",
      "--unstable",
      "--import-map=import_map.json",
      "script.ts"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Cache {
          files: svec!["script.ts"],
        },
        unstable: true,
        import_map_path: Some("import_map.json".to_owned()),
        ..Flags::default()
      }
    );
  }

  #[test]
  fn doc_import_map() {
    let r = flags_from_vec_safe(svec![
      "deno",
      "doc",
      "--unstable",
      "--import-map=import_map.json",
      "script.ts"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Doc {
          source_file: Some("script.ts".to_owned()),
          private: false,
          json: false,
          filter: None,
        },
        unstable: true,
        import_map_path: Some("import_map.json".to_owned()),
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
  fn install_with_flags() {
    #[rustfmt::skip]
    let r = flags_from_vec_safe(svec!["deno", "install", "--unstable", "--import-map", "import_map.json", "--no-remote", "--config", "tsconfig.json", "--no-check", "--reload", "--lock", "lock.json", "--lock-write", "--cert", "example.crt", "--cached-only", "--allow-read", "--allow-net", "--v8-flags=--help", "--seed", "1", "--inspect=127.0.0.1:9229", "--name", "file_server", "--root", "/foo", "--force", "https://deno.land/std/http/file_server.ts", "foo", "bar"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Install {
          name: Some("file_server".to_string()),
          module_url: "https://deno.land/std/http/file_server.ts".to_string(),
          args: svec!["foo", "bar"],
          root: Some(PathBuf::from("/foo")),
          force: true,
        },
        unstable: true,
        import_map_path: Some("import_map.json".to_string()),
        no_remote: true,
        config_path: Some("tsconfig.json".to_string()),
        no_check: true,
        reload: true,
        lock: Some(PathBuf::from("lock.json")),
        lock_write: true,
        ca_file: Some("example.crt".to_string()),
        cached_only: true,
        v8_flags: Some(svec!["--help", "--random-seed=1"]),
        seed: Some(1),
        inspect: Some("127.0.0.1:9229".parse().unwrap()),
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

  #[test]
  fn run_with_args() {
    let r = flags_from_vec_safe(svec![
      "deno",
      "run",
      "script.ts",
      "--allow-read",
      "--allow-net"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run {
          script: "script.ts".to_string(),
        },
        argv: svec!["--allow-read", "--allow-net"],
        ..Flags::default()
      }
    );
    let r = flags_from_vec_safe(svec![
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
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run {
          script: "script.ts".to_string(),
        },
        allow_read: true,
        argv: svec!["--allow-net", "-r", "--help", "--foo", "bar"],
        ..Flags::default()
      }
    );

    let r =
      flags_from_vec_safe(svec!["deno", "run", "script.ts", "foo", "bar"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run {
          script: "script.ts".to_string(),
        },
        argv: svec!["foo", "bar"],
        ..Flags::default()
      }
    );
    let r = flags_from_vec_safe(svec!["deno", "run", "script.ts", "-"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run {
          script: "script.ts".to_string(),
        },
        argv: svec!["-"],
        ..Flags::default()
      }
    );

    let r =
      flags_from_vec_safe(svec!["deno", "run", "script.ts", "-", "foo", "bar"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run {
          script: "script.ts".to_string(),
        },
        argv: svec!["-", "foo", "bar"],
        ..Flags::default()
      }
    );
  }

  #[test]
  fn no_check() {
    let r =
      flags_from_vec_safe(svec!["deno", "run", "--no-check", "script.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run {
          script: "script.ts".to_string(),
        },
        no_check: true,
        ..Flags::default()
      }
    );
  }

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
  fn allow_net_allowlist_with_ports() {
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
        net_allowlist: svec![
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
  fn allow_net_allowlist_with_ipv6_address() {
    let r = flags_from_vec_safe(svec![
      "deno",
      "run",
      "--allow-net=deno.land,deno.land:80,::,127.0.0.1,[::1],1.2.3.4:5678,:5678,[::1]:8080",
      "script.ts"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run {
          script: "script.ts".to_string(),
        },
        net_allowlist: svec![
          "deno.land",
          "deno.land:80",
          "::",
          "127.0.0.1",
          "[::1]",
          "1.2.3.4:5678",
          "0.0.0.0:5678",
          "127.0.0.1:5678",
          "localhost:5678",
          "[::1]:8080"
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
        lock: Some(PathBuf::from("lock.json")),
        ..Flags::default()
      }
    );
  }

  #[test]
  fn test_with_flags() {
    #[rustfmt::skip]
    let r = flags_from_vec_safe(svec!["deno", "test", "--unstable", "--no-run", "--filter", "- foo", "--coverage", "--allow-net", "--allow-none", "dir1/", "dir2/", "--", "arg1", "arg2"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Test {
          no_run: true,
          fail_fast: false,
          filter: Some("- foo".to_string()),
          allow_none: true,
          quiet: false,
          include: Some(svec!["dir1/", "dir2/"]),
        },
        unstable: true,
        coverage: true,
        allow_net: true,
        argv: svec!["arg1", "arg2"],
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
  fn upgrade_with_ca_file() {
    let r =
      flags_from_vec_safe(svec!["deno", "upgrade", "--cert", "example.crt"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Upgrade {
          force: false,
          dry_run: false,
          version: None,
          output: None,
          ca_file: Some("example.crt".to_owned()),
        },
        ca_file: Some("example.crt".to_owned()),
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
          json: false,
          file: Some("https://example.com".to_string()),
        },
        ca_file: Some("example.crt".to_owned()),
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
          private: false,
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
          private: false,
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
          private: false,
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
          private: false,
          json: false,
          source_file: Some("--builtin".to_string()),
          filter: Some("Deno.Listener".to_string()),
        },
        ..Flags::default()
      }
    );

    let r = flags_from_vec_safe(svec![
      "deno",
      "doc",
      "--private",
      "path/to/module.js"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Doc {
          private: true,
          json: false,
          source_file: Some("path/to/module.js".to_string()),
          filter: None,
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
