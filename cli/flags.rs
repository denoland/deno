// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use clap::AppSettings;
use clap::ArgSettings;
use clap::Clap;
use clap::IntoApp;
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
  Compile {
    source_file: String,
    output: Option<PathBuf>,
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
    canary: bool,
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
  pub allow_plugin: bool,
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
    *crate::version::DENO,
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
  let app: Opt = Opt::try_parse_from(args)?;

  let mut flags = Flags::default();

  if app.unstable {
    flags.unstable = true;
  }
  if let Some(log_level) = app.log_level {
    flags.log_level = match log_level.as_str() {
      "debug" => Some(Level::Debug),
      "info" => Some(Level::Info),
      _ => unreachable!(),
    };
  }
  if app.quiet {
    flags.log_level = Some(Level::Error);
  }

  let mut clap_app: clap::App = Opt::into_app();

  if let Some(subcommand) = app.subcommand {
    match subcommand {
      Subcommand::Bundle(m) => bundle_parse(&mut flags, m),
      Subcommand::Cache(m) => cache_parse(&mut flags, m),
      Subcommand::Compile(m) => compile_parse(&mut flags, m),
      Subcommand::Completions(m) => {
        completions_parse(&mut flags, m, &mut clap_app)
      }
      Subcommand::Doc(m) => doc_parse(&mut flags, m),
      Subcommand::Eval(m) => eval_parse(&mut flags, m),
      Subcommand::Fmt(m) => fmt_parse(&mut flags, m),
      Subcommand::Info(m) => info_parse(&mut flags, m),
      Subcommand::Install(m) => install_parse(&mut flags, m),
      Subcommand::Lint(m) => lint_parse(&mut flags, m),
      Subcommand::Repl(m) => repl_parse(&mut flags, m),
      Subcommand::Run(m) => run_parse(&mut flags, m),
      Subcommand::Test(m) => test_parse(&mut flags, m, app.quiet),
      Subcommand::Types(m) => types_parse(&mut flags, m),
      Subcommand::Upgrade(m) => upgrade_parse(&mut flags, m),
    }
  } else {
    repl_parse(
      &mut flags,
      ReplSubcommand {
        runtime: RuntimeArgs {
          inspect: None,
          inspect_brk: None,
          cached_only: false,
          v8_flags: vec![],
          seed: None,
          compilation: CompilationArgs {
            no_remote: false,
            config: None,
            no_check: false,
            lock: None,
            lock_write: false,
            import_map: ImportMapArg { import_map: None },
            reload: ReloadArg { reload: None },
            ca_file: CAFileArg { cert: None },
          },
        },
      },
    );
  }

  Ok(flags)
}

#[derive(Clap, Debug, Clone)]
#[clap(
  name = "deno",
  version = crate::version::DENO.as_str(),
  long_version = LONG_VERSION.as_str(),
  max_term_width = 0,
  global_setting = AppSettings::UnifiedHelpMessage,
  global_setting = AppSettings::ColorNever,
  global_setting = AppSettings::VersionlessSubcommands,
  after_help = ENV_VARIABLES_HELP,
  long_about = DENO_HELP,
)]
struct Opt {
  /// Enable unstable features and APIs
  #[clap(long, global = true)]
  unstable: bool,

  /// Set log level
  #[clap(long, short = 'L', global = true, possible_values = &["debug", "info"])]
  log_level: Option<String>,

  /// Suppress diagnostic output
  ///
  /// By default, subcommands print human-readable diagnostic messages to stderr.
  /// If the flag is set, restrict these messages to errors.
  #[clap(long, short, global = true)]
  quiet: bool,

  #[clap(subcommand)]
  subcommand: Option<Subcommand>,
}

#[derive(Clap, Debug, Clone)]
enum Subcommand {
  Bundle(BundleSubcommand),
  Cache(CacheSubcommand),
  Compile(CompileSubcommand),
  Completions(CompletionsSubcommand),
  Doc(DocSubcommand),
  Eval(EvalSubcommand),
  Fmt(FmtSubcommand),
  Info(InfoSubcommand),
  Install(InstallSubcommand),
  Lint(LintSubcommand),
  Repl(ReplSubcommand),
  Run(RunSubcommand),
  Test(TestSubcommand),
  Types(TypesSubcommand),
  Upgrade(UpgradeSubcommand),
}

/// Bundle module and dependencies into single file
#[derive(Clap, Debug, Clone)]
#[clap(long_about = "Output a single JavaScript file with all dependencies.
  deno bundle https://deno.land/std/examples/colors.ts colors.bundle.js

If no output file is given, the output is written to standard output:
  deno bundle https://deno.land/std/examples/colors.ts")]
struct BundleSubcommand {
  source_file: String,

  #[clap(parse(from_os_str))]
  out_file: Option<PathBuf>,

  #[clap(flatten)]
  watch: WatchArg,

  #[clap(flatten)]
  compilation: CompilationArgs,
}

/// Cache the dependencies
#[derive(Clap, Debug, Clone)]
#[clap(long_about = "Cache and compile remote dependencies recursively.

Download and compile a module with all of its static dependencies and save them
in the local cache, without running any code:
  deno cache https://deno.land/std/http/file_server.ts

Future runs of this module will trigger no downloads or compilation unless
--reload is specified.")]
struct CacheSubcommand {
  #[clap(required = true)]
  file: Vec<String>,

  #[clap(flatten)]
  compilation: CompilationArgs,
}

/// Compiles the given script into a self contained executable.
///
///   deno compile --unstable https://deno.land/std/http/file_server.ts
///   deno compile --unstable --output /usr/local/bin/color_util https://deno.land/std/examples/colors.ts
/// The executable name is inferred by default:
///   - Attempt to take the file stem of the URL path. The above example would
///     become 'file_server'.
///   - If the file stem is something generic like 'main', 'mod', 'index' or 'cli',
///     and the path has no parent, take the file name of the parent path. Otherwise
///     settle with the generic name.
///   - If the resulting name has an '@...' suffix, strip it.
/// Cross compiling binaries for different platforms is not currently possible.
#[derive(Clap, Debug, Clone)]
struct CompileSubcommand {
  source_file: String,

  /// Output file (defaults to $PWD/<inferred-name>)
  #[clap(long, short, parse(from_os_str))]
  output: Option<PathBuf>,

  #[clap(flatten)]
  compilaton: CompilationArgs,
}

#[derive(Clap, Debug, Clone)]
#[allow(clippy::enum_variant_names)]
enum Shell {
  Bash,
  Fish,
  #[clap(name = "powershell")]
  PowerShell,
  Zsh,
}

/// Generate shell completions
#[derive(Clap, Debug, Clone)]
#[clap(
  setting = AppSettings::DisableHelpSubcommand,
  long_about = "Output shell completion script to standard output.
  deno completions bash > /usr/local/etc/bash_completion.d/deno.bash
  source /usr/local/etc/bash_completion.d/deno.bash"
)]
struct CompletionsSubcommand {
  #[clap(arg_enum)]
  shell: Shell,
}

// TODO(nayeemrmn): Make `--builtin` a proper option. Blocked by
// https://github.com/clap-rs/clap/issues/1794. Currently `--builtin` is
// just a possible value of `source_file` so leading hyphens must be
// enabled.
/// Show documentation for a module.
///
/// Output documentation to standard output:
///     deno doc ./path/to/module.ts
///
/// Output private documentation to standard output:
///     deno doc --private ./path/to/module.ts
///
/// Output documentation in JSON format:
///     deno doc --json ./path/to/module.ts
///
/// Target a specific symbol:
///     deno doc ./path/to/module.ts MyClass.someField
///
/// Show documentation for runtime built-ins:
///     deno doc
///     deno doc --builtin Deno.Listener
#[derive(Clap, Debug, Clone)]
#[clap(setting = AppSettings::AllowLeadingHyphen)]
struct DocSubcommand {
  /// Output documentation in JSON format
  #[clap(long)]
  json: bool,

  /// Output private documentation
  #[clap(long)]
  private: bool,

  source_file: Option<String>,

  /// Dot separated path to symbol
  #[clap(conflicts_with = "json")]
  filter: Option<String>,

  #[clap(flatten)]
  import_map: ImportMapArg,

  #[clap(flatten)]
  reload: ReloadArg,
}

/// Eval script
#[derive(Clap, Debug, Clone)]
#[clap(long_about = "Evaluate JavaScript from the command line.
  deno eval \"console.log('hello world')\"

To evaluate as TypeScript:
  deno eval -T \"const v: string = 'hello'; console.log(v)\"

This command has implicit access to all permissions (--allow-all).")]
struct EvalSubcommand {
  /// Treat eval input as TypeScript
  #[clap(long, short = 'T')]
  ts: bool,

  /// print result to stdout
  #[clap(long, short)]
  print: bool,

  /// Code arg
  #[clap(required = true, value_name = "CODE_ARG")]
  code_arg: Vec<String>,

  #[clap(flatten)]
  runtime: RuntimeArgs,
}

/// Format source files
#[derive(Clap, Debug, Clone)]
#[clap(long_about = "Auto-format JavaScript/TypeScript source code.
  deno fmt
  deno fmt myfile1.ts myfile2.ts
  deno fmt --check

Format stdin and write to stdout:
  cat file.ts | deno fmt -

Ignore formatting code by preceding it with an ignore comment:
  // deno-fmt-ignore

Ignore formatting a file by adding an ignore comment at the top of the file:
  // deno-fmt-ignore-file")]
struct FmtSubcommand {
  /// Check if the source files are formatted
  #[clap(long)]
  check: bool,

  /// Ignore formatting particular source files. Use with --unstable
  #[clap(
    long,
    use_delimiter = true,
    require_equals = true,
    parse(from_os_str)
  )]
  ignore: Vec<PathBuf>,

  #[clap(parse(from_os_str))]
  files: Vec<PathBuf>,

  #[clap(flatten)]
  watch: WatchArg,
}

/// Show info about cache or info related to source file
#[derive(Clap, Debug, Clone)]
#[clap(long_about = "Information about a module or the cache directories.

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
TypeScript compiler cache: Subdirectory containing TS compiler output.")]
struct InfoSubcommand {
  file: Option<String>,

  // TODO(lucacasonato): remove for 2.0
  /// Skip type checking modules
  #[clap(long, hidden = true)]
  #[allow(dead_code)]
  no_check: bool,

  /// Outputs the information in JSON format
  #[clap(long)]
  json: bool,

  #[clap(flatten)]
  import_map: ImportMapArg,

  #[clap(flatten)]
  ca_file: CAFileArg,

  // duplicate arg: requires
  /// Reload source code cache (recompile TypeScript)
  ///
  /// --reload
  ///   Reload everything
  /// --reload=https://deno.land/std
  ///   Reload only standard modules
  /// --reload=https://deno.land/std/fs/utils.ts,https://deno.land/std/fmt/colors.ts
  ///   Reloads specific modules
  #[clap(
    long,
    short,
    use_delimiter = true,
    require_equals = true,
    value_name = "CACHE_BLOCKLIST",
    requires = "file"
  )]
  reload: Option<Vec<String>>,
}

/// Install script as an executable
#[derive(Clap, Debug, Clone)]
#[clap(
  setting = AppSettings::TrailingVarArg,
  long_about = "Installs a script as an executable in the installation root's bin directory.
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

These must be added to the path manually if required."
)]
struct InstallSubcommand {
  #[clap(required = true, allow_hyphen_values = true)]
  cmd: Vec<String>,

  /// Executable file name
  #[clap(long, short)]
  name: Option<String>,

  /// Installation root
  #[clap(long, parse(from_os_str))]
  root: Option<PathBuf>,

  /// Forcefully overwrite existing installation
  #[clap(long, short)]
  force: bool,

  #[clap(flatten)]
  runtime: RuntimeArgs,

  #[clap(flatten)]
  permissions: PermissionArgs,
}

/// Lint source files
#[derive(Clap, Debug, Clone)]
#[clap(long_about = "Lint JavaScript/TypeScript source code.
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
")]
struct LintSubcommand {
  /// List available rules
  #[clap(long)]
  rules: bool,

  /// Ignore linting particular source files
  #[clap(
    long,
    use_delimiter = true,
    require_equals = true,
    parse(from_os_str),
    requires = "unstable"
  )]
  ignore: Vec<PathBuf>,

  /// Output lint result in JSON format
  #[clap(long)]
  json: bool,

  #[clap(parse(from_os_str))]
  files: Vec<PathBuf>,
}

/// Read Eval Print Loop
#[derive(Clap, Debug, Clone)]
struct ReplSubcommand {
  #[clap(flatten)]
  runtime: RuntimeArgs,
}

/// Run a program given a filename or url to the module. Use '-' as a filename to read from stdin.
#[derive(Clap, Debug, Clone)]
#[clap(
  setting = AppSettings::TrailingVarArg,
  long_about = "Run a program given a filename or url to the module.

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
)]
struct RunSubcommand {
  // duplicate arg: required
  // NOTE: these defaults are provided
  // so `deno run --v8-flags=--help` works
  // without specifying file to run.
  /// Script arg
  #[clap(required = true, value_name = "SCRIPT_ARG", setting = ArgSettings::AllowEmptyValues, default_value_ifs = &[
    ("v8-flags", Some("--help"), "_"),
    ("v8-flags", Some("-help"), "_"),
  ])]
  script_arg: Vec<String>,

  // duplicate arg: conflicts
  /// Watch for file changes and restart process automatically
  #[clap(
    long,
    requires = "unstable",
    conflicts_with = "inspect",
    conflicts_with = "inspect-brk",
    long_about = "Watch for file changes and restart process automatically.
Only local files from entry point module graph are watched."
  )]
  watch: bool,

  #[clap(flatten)]
  runtime: RuntimeArgs,

  #[clap(flatten)]
  permissions: PermissionArgs,
}

/// Run tests
#[derive(Clap, Debug, Clone)]
#[clap(
  setting = AppSettings::TrailingVarArg,
  long_about = "Run tests using Deno's built-in test runner.

Evaluate the given modules, run all tests declared with 'Deno.test()' and
report results to standard output:
  deno test src/fetch_test.ts src/signal_test.ts

Directory arguments are expanded to all contained files matching the glob
{*_,*.,}test.{js,mjs,ts,jsx,tsx}:
  deno test src/"
)]
struct TestSubcommand {
  /// Cache test modules, but don't run tests
  #[clap(long, requires = "unstable")]
  no_run: bool,

  /// Stop on first error
  #[clap(long, alias = "failfast")]
  fail_fast: bool,

  /// Don't return error code if no test files are found
  #[clap(long)]
  allow_none: bool,

  /// Run tests with this string or pattern in the test name
  #[clap(long, allow_hyphen_values = true)]
  filter: Option<String>,

  /// Collect coverage information
  #[clap(
    long,
    requires = "unstable",
    conflicts_with = "inspect",
    conflicts_with = "inspect"
  )]
  coverage: bool,

  /// List of file names to run
  files: Vec<String>,

  // duplicate arg: last
  // NOTE: these defaults are provided
  // so `deno run --v8-flags=--help` works
  // without specifying file to run.
  /// Script arg
  #[clap(value_name = "SCRIPT_ARG", last = true, setting = ArgSettings::AllowEmptyValues, default_value_ifs = &[
    ("v8-flags", Some("--help"), "_"),
    ("v8-flags", Some("-help"), "_"),
  ])]
  script_arg: Vec<String>,

  #[clap(flatten)]
  runtime: RuntimeArgs,

  #[clap(flatten)]
  permissions: PermissionArgs,
}

/// Print runtime TypeScript declarations.
#[derive(Clap, Debug, Clone)]
#[clap(long_about = "Print runtime TypeScript declarations.
  deno types > lib.deno.d.ts

The declaration file could be saved and used for typing information.")]
struct TypesSubcommand {}

/// Upgrade deno executable to given version
#[derive(Clap, Debug, Clone)]
#[clap(long_about = "Upgrade deno executable to the given version.
Defaults to latest.

The version is downloaded from
https://github.com/denoland/deno/releases
and is used to replace the current executable.

If you want to not replace the current Deno executable but instead download an
update to a different location, use the --output flag
  deno upgrade --output $HOME/my_deno")]
struct UpgradeSubcommand {
  /// The version to upgrade to
  #[clap(long)]
  version: Option<String>,

  /// The path to output the updated version to
  #[clap(long, parse(from_os_str))]
  output: Option<PathBuf>,

  /// Perform all checks without replacing old exe
  #[clap(long)]
  dry_run: bool,

  /// Replace current exe even if not out-of-date
  #[clap(long, short)]
  force: bool,

  /// Upgrade to canary builds
  #[clap(long)]
  canary: bool,

  #[clap(flatten)]
  ca_file: CAFileArg,
}

#[derive(Clap, Debug, Clone)]
struct CompilationArgs {
  /// Do not resolve remote modules
  #[clap(long)]
  no_remote: bool,

  /// Load tsconfig.json configuration file
  #[clap(long, short, value_name = "FILE")]
  config: Option<String>,

  /// Skip type checking modules
  #[clap(long)]
  no_check: bool,

  /// Check the specified lock file
  #[clap(long, value_name = "FILE", parse(from_os_str))]
  lock: Option<PathBuf>,

  /// Write lock file (use with --lock)
  #[clap(long, requires = "lock")]
  lock_write: bool,

  #[clap(flatten)]
  import_map: ImportMapArg,

  #[clap(flatten)]
  reload: ReloadArg,

  #[clap(flatten)]
  ca_file: CAFileArg,
}

#[derive(Clap, Debug, Clone)]
struct ImportMapArg {
  /// UNSTABLE: Load import map file
  #[clap(
    long,
    alias = "importmap",
    value_name = "FILE",
    requires = "unstable",
    long_about = "UNSTABLE:
Load import map file
Docs: https://deno.land/manual/linking_to_external_code/import_maps
Specification: https://wicg.github.io/import-maps/
Examples: https://github.com/WICG/import-maps#the-import-map"
  )]
  import_map: Option<String>,
}

#[derive(Clap, Debug, Clone)]
struct ReloadArg {
  /// Reload source code cache (recompile TypeScript)
  ///
  /// --reload
  ///   Reload everything
  /// --reload=https://deno.land/std
  ///   Reload only standard modules
  /// --reload=https://deno.land/std/fs/utils.ts,https://deno.land/std/fmt/colors.ts
  ///   Reloads specific modules
  #[clap(
    long,
    short,
    use_delimiter = true,
    require_equals = true,
    value_name = "CACHE_BLOCKLIST"
  )]
  reload: Option<Vec<String>>,
}

#[derive(Clap, Debug, Clone)]
struct CAFileArg {
  /// Load certificate authority from PEM encoded file
  #[clap(long, value_name = "FILE")]
  cert: Option<String>,
}

#[derive(Clap, Debug, Clone)]
struct PermissionArgs {
  /// Allow file system read access
  #[clap(
    long,
    use_delimiter = true,
    require_equals = true,
    parse(from_os_str)
  )]
  allow_read: Option<Vec<PathBuf>>,

  /// Allow file system write access
  #[clap(
    long,
    use_delimiter = true,
    require_equals = true,
    parse(from_os_str)
  )]
  allow_write: Option<Vec<PathBuf>>,

  /// Allow network access
  #[clap(long, use_delimiter = true, require_equals = true, validator = crate::flags_allow_net::validator)]
  allow_net: Option<Vec<String>>,

  /// Allow environment access
  #[clap(long)]
  allow_env: bool,

  /// Allow running subprocesses
  #[clap(long)]
  allow_run: bool,

  /// Allow loading plugins
  #[clap(long)]
  allow_plugin: bool,

  /// Allow high resolution time measurement
  #[clap(long)]
  allow_hrtime: bool,

  /// Allow all permissions
  #[clap(long, short = 'A')]
  allow_all: bool,
}

#[derive(Clap, Debug, Clone)]
struct RuntimeArgs {
  /// activate inspector on host:port (default: 127.0.0.1:9229)
  #[clap(long, value_name = "HOST:PORT", require_equals = true, validator = inspect_arg_validate)]
  inspect: Option<Option<String>>,

  /// activate inspector on host:port and break at start of user script
  #[clap(long, value_name = "HOST:PORT", require_equals = true, validator = inspect_arg_validate)]
  inspect_brk: Option<Option<String>>,

  /// Require that remote dependencies are already cached
  #[clap(long)]
  cached_only: bool,

  /// Set V8 command line options (for help: --v8-flags=--help)
  #[clap(long, use_delimiter = true, require_equals = true)]
  v8_flags: Vec<String>,

  /// Seed Math.random()
  #[clap(long, value_name = "NUMBER", validator = |val: &str| match val.parse::<u64>() {
    Ok(_) => Ok(()),
    Err(_) => Err("Seed should be a number".to_string()),
  })]
  seed: Option<u64>,

  #[clap(flatten)]
  compilation: CompilationArgs,
}

#[derive(Clap, Debug, Clone)]
struct WatchArg {
  /// Watch for file changes and restart process automatically
  #[clap(
    long,
    requires = "unstable",
    long_about = "Watch for file changes and restart process automatically.
Only local files from entry point module graph are watched."
  )]
  watch: bool,
}

fn bundle_parse(flags: &mut Flags, matches: BundleSubcommand) {
  compile_args_parse(flags, matches.compilation);

  flags.allow_write = matches.out_file.is_some();

  flags.watch = matches.watch.watch;

  flags.subcommand = DenoSubcommand::Bundle {
    source_file: matches.source_file,
    out_file: matches.out_file,
  };
}

fn cache_parse(flags: &mut Flags, matches: CacheSubcommand) {
  compile_args_parse(flags, matches.compilation);

  flags.subcommand = DenoSubcommand::Cache {
    files: matches.file,
  };
}

fn compile_parse(flags: &mut Flags, matches: CompileSubcommand) {
  compile_args_parse(flags, matches.compilaton);

  flags.subcommand = DenoSubcommand::Compile {
    source_file: matches.source_file,
    output: matches.output,
  }
}

fn completions_parse(
  flags: &mut Flags,
  matches: CompletionsSubcommand,
  mut app: &mut clap::App,
) {
  use clap_generate::generators::{Bash, Fish, PowerShell, Zsh};

  let mut buf: Vec<u8> = vec![];

  let name = "deno";

  match matches.shell {
    Shell::Bash => clap_generate::generate::<Bash, _>(&mut app, name, &mut buf),
    Shell::Fish => clap_generate::generate::<Fish, _>(&mut app, name, &mut buf),
    Shell::PowerShell => {
      clap_generate::generate::<PowerShell, _>(&mut app, name, &mut buf)
    }
    Shell::Zsh => clap_generate::generate::<Zsh, _>(&mut app, name, &mut buf),
  }

  flags.subcommand = DenoSubcommand::Completions {
    buf: buf.into_boxed_slice(),
  };
}

fn doc_parse(flags: &mut Flags, matches: DocSubcommand) {
  import_map_arg_parse(flags, matches.import_map);
  reload_arg_parse(flags, matches.reload.reload);

  flags.subcommand = DenoSubcommand::Doc {
    source_file: matches.source_file,
    json: matches.json,
    filter: matches.filter,
    private: matches.private,
  };
}

fn eval_parse(flags: &mut Flags, matches: EvalSubcommand) {
  runtime_args_parse(flags, matches.runtime);

  flags.allow_net = true;
  flags.allow_env = true;
  flags.allow_run = true;
  flags.allow_read = true;
  flags.allow_write = true;
  flags.allow_plugin = true;
  flags.allow_hrtime = true;

  let mut code = matches.code_arg;
  let code_args = code.split_off(1);
  let code = code[0].clone();
  for v in code_args {
    flags.argv.push(v);
  }

  flags.subcommand = DenoSubcommand::Eval {
    print: matches.print,
    code,
    as_typescript: matches.ts,
  }
}

fn fmt_parse(flags: &mut Flags, matches: FmtSubcommand) {
  flags.watch = matches.watch.watch;

  flags.subcommand = DenoSubcommand::Fmt {
    check: matches.check,
    files: matches.files,
    ignore: matches.ignore,
  }
}

fn info_parse(flags: &mut Flags, matches: InfoSubcommand) {
  reload_arg_parse(flags, matches.reload);
  import_map_arg_parse(flags, matches.import_map);
  ca_file_arg_parse(flags, matches.ca_file);

  flags.subcommand = DenoSubcommand::Info {
    file: matches.file,
    json: matches.json,
  };
}

fn install_parse(flags: &mut Flags, matches: InstallSubcommand) {
  runtime_args_parse(flags, matches.runtime);
  permission_args_parse(flags, matches.permissions);

  let cmd = matches.cmd;

  flags.subcommand = DenoSubcommand::Install {
    name: matches.name,
    module_url: cmd[0].clone(),
    args: cmd[1..].to_vec(),
    root: matches.root,
    force: matches.force,
  };
}

fn lint_parse(flags: &mut Flags, matches: LintSubcommand) {
  flags.subcommand = DenoSubcommand::Lint {
    files: matches.files,
    rules: matches.rules,
    ignore: matches.ignore,
    json: matches.json,
  };
}

fn repl_parse(flags: &mut Flags, matches: ReplSubcommand) {
  runtime_args_parse(flags, matches.runtime);

  flags.repl = true;
  flags.subcommand = DenoSubcommand::Repl;
  flags.allow_net = true;
  flags.allow_env = true;
  flags.allow_run = true;
  flags.allow_read = true;
  flags.allow_write = true;
  flags.allow_plugin = true;
  flags.allow_hrtime = true;
}

fn run_parse(flags: &mut Flags, matches: RunSubcommand) {
  runtime_args_parse(flags, matches.runtime);
  permission_args_parse(flags, matches.permissions);

  flags.watch = matches.watch;

  let mut script = matches.script_arg;
  assert!(!script.is_empty());
  let script_args = script.split_off(1);
  let script = script[0].to_string();
  for v in script_args {
    flags.argv.push(v);
  }

  flags.subcommand = DenoSubcommand::Run { script };
}

fn test_parse(flags: &mut Flags, matches: TestSubcommand, quiet: bool) {
  runtime_args_parse(flags, matches.runtime);
  permission_args_parse(flags, matches.permissions);

  flags.coverage = matches.coverage;

  if !matches.script_arg.is_empty() {
    flags.argv.extend_from_slice(&*matches.script_arg);
  }

  let include = if matches.files.is_empty() {
    None
  } else {
    Some(matches.files)
  };

  flags.subcommand = DenoSubcommand::Test {
    no_run: matches.no_run,
    fail_fast: matches.fail_fast,
    quiet,
    include,
    filter: matches.filter,
    allow_none: matches.allow_none,
  };
}

fn types_parse(flags: &mut Flags, _matches: TypesSubcommand) {
  flags.subcommand = DenoSubcommand::Types;
}

fn upgrade_parse(flags: &mut Flags, matches: UpgradeSubcommand) {
  ca_file_arg_parse(flags, matches.ca_file.clone());

  flags.subcommand = DenoSubcommand::Upgrade {
    dry_run: matches.dry_run,
    force: matches.force,
    canary: matches.canary,
    version: matches.version,
    output: matches.output,
    ca_file: matches.ca_file.cert,
  };
}

fn compile_args_parse(flags: &mut Flags, matches: CompilationArgs) {
  import_map_arg_parse(flags, matches.import_map);
  reload_arg_parse(flags, matches.reload.reload);
  ca_file_arg_parse(flags, matches.ca_file);

  flags.no_remote = matches.no_remote;
  flags.config_path = matches.config;
  flags.no_check = matches.no_check;
  flags.lock = matches.lock;
  flags.lock_write = matches.lock_write;
}

fn runtime_args_parse(flags: &mut Flags, matches: RuntimeArgs) {
  compile_args_parse(flags, matches.compilation);

  flags.cached_only = matches.cached_only;

  if !matches.v8_flags.is_empty() {
    flags.v8_flags = Some(matches.v8_flags);
  }

  flags.seed = matches.seed;
  if let Some(seed) = matches.seed {
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

  let default = || "127.0.0.1:9229".parse::<SocketAddr>().unwrap();
  flags.inspect = matches
    .inspect
    .map(|inspect| inspect.map_or_else(default, |host| host.parse().unwrap()));
  flags.inspect_brk = matches
    .inspect_brk
    .map(|inspect| inspect.map_or_else(default, |host| host.parse().unwrap()));
}

fn permission_args_parse(flags: &mut Flags, matches: PermissionArgs) {
  if let Some(read_allowlist) = matches.allow_read {
    if read_allowlist.is_empty() {
      flags.allow_read = true;
    } else {
      flags.read_allowlist = read_allowlist;
    }
  }

  if let Some(write_allowlist) = matches.allow_write {
    if write_allowlist.is_empty() {
      flags.allow_write = true;
    } else {
      flags.write_allowlist = write_allowlist;
    }
  }

  if let Some(net_allowlist) = matches.allow_net {
    if net_allowlist.is_empty() {
      flags.allow_net = true;
    } else {
      flags.net_allowlist =
        crate::flags_allow_net::parse(net_allowlist).unwrap();
      debug!("net allowlist: {:#?}", &flags.net_allowlist);
    }
  }

  flags.allow_env = matches.allow_env;
  flags.allow_run = matches.allow_run;
  flags.allow_plugin = matches.allow_plugin;
  flags.allow_hrtime = matches.allow_hrtime;

  if matches.allow_all {
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

fn import_map_arg_parse(flags: &mut Flags, matches: ImportMapArg) {
  flags.import_map_path = matches.import_map;
}

fn ca_file_arg_parse(flags: &mut Flags, matches: CAFileArg) {
  flags.ca_file = matches.cert;
}

fn reload_arg_parse(flags: &mut Flags, reload: Option<Vec<String>>) {
  if let Some(cache_bl) = reload {
    if cache_bl.is_empty() {
      flags.reload = true;
    } else {
      flags.cache_blocklist = resolve_urls(cache_bl);
      debug!("cache blocklist: {:#?}", &flags.cache_blocklist);
      flags.reload = false;
    }
  }
}

fn inspect_arg_validate(val: &str) -> Result<(), String> {
  match val.parse::<SocketAddr>() {
    Ok(_) => Ok(()),
    Err(e) => Err(e.to_string()),
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
          canary: false,
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
    assert_eq!(r.unwrap_err().kind, clap::ErrorKind::DisplayVersion);
    let r = flags_from_vec_safe(svec!["deno", "-V"]);
    assert_eq!(r.unwrap_err().kind, clap::ErrorKind::DisplayVersion);
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
        allow_plugin: true,
        allow_hrtime: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn eval_args() {
    let r = flags_from_vec_safe(svec![
      "deno",
      "eval",
      "console.log(Deno.args)",
      "arg1",
      "arg2"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Eval {
          print: false,
          code: "console.log(Deno.args)".to_string(),
          as_typescript: false,
        },
        argv: svec!["arg1", "arg2"],
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
        repl: true,
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
        allow_plugin: true,
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
        seed: Some(250_u64),
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
        seed: Some(250_u64),
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
          canary: false,
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

  #[test]
  fn compile() {
    let r = flags_from_vec_safe(svec![
      "deno",
      "compile",
      "https://deno.land/std/examples/colors.ts"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Compile {
          source_file: "https://deno.land/std/examples/colors.ts".to_string(),
          output: None
        },
        ..Flags::default()
      }
    );
  }

  #[test]
  fn compile_with_flags() {
    #[rustfmt::skip]
    let r = flags_from_vec_safe(svec!["deno", "compile", "--unstable", "--import-map", "import_map.json", "--no-remote", "--config", "tsconfig.json", "--no-check", "--reload", "--lock", "lock.json", "--lock-write", "--cert", "example.crt", "--output", "colors", "https://deno.land/std/examples/colors.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Compile {
          source_file: "https://deno.land/std/examples/colors.ts".to_string(),
          output: Some(PathBuf::from("colors"))
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
        ..Flags::default()
      }
    );
  }
}
