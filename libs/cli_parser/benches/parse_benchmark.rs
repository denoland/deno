//! Benchmark comparing our custom parser against clap using the ACTUAL
//! Deno CLI command tree.
//!
//! The `build_clap_root()` function below is a faithful reproduction of the
//! `clap_root()` function from `cli/args/flags.rs` in the Deno codebase.
//! Simplifications:
//!   - `cstr!("...")` replaced with plain `"..."` (no ANSI color codes)
//!   - `DENO_VERSION_INFO.*` replaced with hardcoded strings
//!   - `clap_complete` related code (SubcommandCandidates, EnvCompleter) removed
//!   - `FalseyValueParser` replaced with `value_parser!(bool)`
//!   - `CommandExt` / `with_unstable_args` inlined directly
//!   - `deno_runtime::UNSTABLE_FEATURES` hardcoded
//!   - `help_subcommand()` omitted (clones subcommands, not part of parsing)
//!   - `ENV_VARIABLES_HELP` / `DENO_HELP` replaced with short strings
//!   - Custom value_parsers (SysDescriptor, flags_net::validator, etc.)
//!     replaced with simple string acceptance
//!   - `.defer()` calls preserved where they exist in the original

use clap::builder::styling::AnsiColor;
use clap::{value_parser, Arg, ArgAction, Command, ColorChoice, ValueHint};
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use std::num::{NonZeroU32, NonZeroU8, NonZeroUsize};

// ============================================================
// Heading constants (matching Deno)
// ============================================================
const DOC_HEADING: &str = "Documentation options";
const FMT_HEADING: &str = "Formatting options";
const COMPILE_HEADING: &str = "Compile options";
const LINT_HEADING: &str = "Linting options";
const TEST_HEADING: &str = "Testing options";
const UPGRADE_HEADING: &str = "Upgrade options";
const PUBLISH_HEADING: &str = "Publishing options";
const TYPE_CHECKING_HEADING: &str = "Type checking options";
const FILE_WATCHING_HEADING: &str = "File watching options";
const DEBUGGING_HEADING: &str = "Debugging options";
const DEPENDENCY_MANAGEMENT_HEADING: &str = "Dependency management options";
const UNSTABLE_HEADING: &str = "Unstable options";

// ============================================================
// Unstable feature flag names (from runtime/features/gen.rs)
// ============================================================
const UNSTABLE_FLAG_NAMES: &[(&str, &str, bool)] = &[
    ("unstable-bare-node-builtins", "Enable unstable bare node builtins feature", true),
    ("unstable-broadcast-channel", "Enable unstable BroadcastChannel API", false),
    ("unstable-bundle", "Enable unstable bundle runtime API", true),
    ("unstable-byonm", "", false),
    ("unstable-cron", "Enable unstable Deno.cron API", true),
    ("unstable-detect-cjs", "Treats ambiguous files as CommonJS in more cases", true),
    ("unstable-ffi", "Enable unstable FFI APIs", false),
    ("unstable-fs", "Enable unstable file system APIs", false),
    ("unstable-http", "Enable unstable HTTP APIs", false),
    ("unstable-kv", "Enable unstable KV APIs", true),
    ("unstable-lazy-dynamic-imports", "Lazily loads statically analyzable dynamic imports", true),
    ("unstable-lockfile-v5", "Enable unstable lockfile v5", true),
    ("unstable-net", "enable unstable net APIs", true),
    ("unstable-no-legacy-abort", "Enable abort signal without legacy behavior", true),
    ("unstable-node-globals", "Prefer Node.js globals over Deno globals", true),
    ("unstable-npm-lazy-caching", "Enable unstable lazy caching of npm dependencies", true),
    ("unstable-otel", "Enable unstable OpenTelemetry features", false),
    ("unstable-process", "Enable unstable process APIs", false),
    ("unstable-raw-imports", "Enable unstable bytes and text imports", true),
    ("unstable-sloppy-imports", "Enable unstable resolving of specifiers by extension probing", true),
    ("unstable-subdomain-wildcards", "Enable subdomain wildcards for --allow-net", false),
    ("unstable-temporal", "Enable unstable Temporal API", false),
    ("unstable-tsgo", "Enable unstable TypeScript Go integration", true),
    ("unstable-unsafe-proto", "Enable unsafe __proto__ support", true),
    ("unstable-vsock", "Enable unstable VSOCK APIs", false),
    ("unstable-webgpu", "Enable unstable WebGPU APIs", true),
    ("unstable-worker-options", "Enable unstable Web Worker APIs", true),
];

// ============================================================
// Shared arg builders (matching Deno's flags.rs)
// ============================================================

fn with_unstable_args(cmd: Command) -> Command {
    let mut next_order = 1000u32;
    let mut cmd = cmd.arg(
        Arg::new("unstable")
            .long("unstable")
            .help("The --unstable flag has been deprecated. Use granular --unstable-* flags instead")
            .action(ArgAction::SetTrue)
            .display_order({
                next_order += 1;
                next_order as usize
            }),
    );

    for &(flag_name, help_text, _show) in UNSTABLE_FLAG_NAMES {
        next_order += 1;
        let mut arg = Arg::new(flag_name)
            .long(flag_name)
            .help(help_text)
            .action(ArgAction::SetTrue)
            .value_parser(value_parser!(bool))
            .hide(true)
            .help_heading(UNSTABLE_HEADING)
            .display_order(next_order as usize);

        if flag_name == "unstable-sloppy-imports" {
            arg = arg.alias("sloppy-imports");
        }

        cmd = cmd.arg(arg);
    }

    cmd
}

fn no_check_arg() -> Arg {
    Arg::new("no-check")
        .num_args(0..=1)
        .require_equals(true)
        .value_name("NO_CHECK_TYPE")
        .long("no-check")
        .help("Skip type-checking")
        .help_heading(TYPE_CHECKING_HEADING)
}

fn check_arg(checks_local_by_default: bool) -> Arg {
    let arg = Arg::new("check")
        .conflicts_with("no-check")
        .long("check")
        .num_args(0..=1)
        .require_equals(true)
        .value_name("CHECK_TYPE")
        .help_heading(TYPE_CHECKING_HEADING);

    if checks_local_by_default {
        arg.help("Set type-checking behavior (type-checks local modules by default)")
    } else {
        arg.help("Enable type-checking (does not type-check by default)")
    }
}

fn import_map_arg() -> Arg {
    Arg::new("import-map")
        .long("import-map")
        .alias("importmap")
        .value_name("FILE")
        .help("Load import map file")
        .value_hint(ValueHint::FilePath)
        .help_heading(DEPENDENCY_MANAGEMENT_HEADING)
}

fn no_remote_arg() -> Arg {
    Arg::new("no-remote")
        .long("no-remote")
        .action(ArgAction::SetTrue)
        .help("Do not resolve remote modules")
        .help_heading(DEPENDENCY_MANAGEMENT_HEADING)
}

fn no_npm_arg() -> Arg {
    Arg::new("no-npm")
        .long("no-npm")
        .action(ArgAction::SetTrue)
        .help("Do not resolve npm modules")
        .help_heading(DEPENDENCY_MANAGEMENT_HEADING)
}

fn node_modules_dir_arg() -> Arg {
    Arg::new("node-modules-dir")
        .long("node-modules-dir")
        .num_args(0..=1)
        .default_missing_value("auto")
        .value_name("MODE")
        .require_equals(true)
        .help("Sets the node modules management mode for npm packages")
        .help_heading(DEPENDENCY_MANAGEMENT_HEADING)
}

fn vendor_arg() -> Arg {
    Arg::new("vendor")
        .long("vendor")
        .num_args(0..=1)
        .value_parser(value_parser!(bool))
        .default_missing_value("true")
        .require_equals(true)
        .help("Toggles local vendor folder usage")
        .help_heading(DEPENDENCY_MANAGEMENT_HEADING)
}

fn node_conditions_arg() -> Arg {
    Arg::new("conditions")
        .long("conditions")
        .help("Specify custom conditions for npm package exports")
        .use_value_delimiter(true)
        .action(ArgAction::Append)
}

fn config_arg() -> Arg {
    Arg::new("config")
        .short('c')
        .long("config")
        .value_name("FILE")
        .help("Configure different aspects of deno")
        .value_hint(ValueHint::FilePath)
}

fn no_config_arg() -> Arg {
    Arg::new("no-config")
        .long("no-config")
        .action(ArgAction::SetTrue)
        .help("Disable automatic loading of the configuration file")
        .conflicts_with("config")
}

fn reload_arg() -> Arg {
    Arg::new("reload")
        .short('r')
        .num_args(0..)
        .action(ArgAction::Append)
        .require_equals(true)
        .long("reload")
        .value_name("CACHE_BLOCKLIST")
        .help("Reload source code cache (recompile TypeScript)")
        .value_hint(ValueHint::FilePath)
        .help_heading(DEPENDENCY_MANAGEMENT_HEADING)
}

fn lock_args() -> [Arg; 3] {
    [
        Arg::new("lock")
            .long("lock")
            .value_name("FILE")
            .default_missing_value("./deno.lock")
            .help("Check the specified lock file")
            .num_args(0..=1)
            .value_parser(value_parser!(String))
            .value_hint(ValueHint::FilePath)
            .help_heading(DEPENDENCY_MANAGEMENT_HEADING),
        Arg::new("no-lock")
            .long("no-lock")
            .action(ArgAction::SetTrue)
            .help("Disable auto discovery of the lock file")
            .conflicts_with("lock")
            .help_heading(DEPENDENCY_MANAGEMENT_HEADING),
        Arg::new("frozen")
            .long("frozen")
            .alias("frozen-lockfile")
            .value_parser(value_parser!(bool))
            .value_name("BOOLEAN")
            .num_args(0..=1)
            .require_equals(true)
            .default_missing_value("true")
            .help("Error out if lockfile is out of date")
            .help_heading(DEPENDENCY_MANAGEMENT_HEADING),
    ]
}

fn ca_file_arg() -> Arg {
    Arg::new("cert")
        .long("cert")
        .value_name("FILE")
        .help("Load certificate authority from PEM encoded file")
        .value_hint(ValueHint::FilePath)
}

fn unsafely_ignore_certificate_errors_arg() -> Arg {
    Arg::new("unsafely-ignore-certificate-errors")
        .hide(true)
        .long("unsafely-ignore-certificate-errors")
        .num_args(0..)
        .use_value_delimiter(true)
        .require_equals(true)
        .value_name("HOSTNAMES")
        .help("DANGER: Disables verification of TLS certificates")
}

fn min_dep_age_arg() -> Arg {
    Arg::new("minimum-dependency-age")
        .long("minimum-dependency-age")
        .help("The age in minutes or duration for minimum dependency age")
}

fn cached_only_arg() -> Arg {
    Arg::new("cached-only")
        .long("cached-only")
        .action(ArgAction::SetTrue)
        .help("Require that remote dependencies are already cached")
        .help_heading(DEPENDENCY_MANAGEMENT_HEADING)
}

fn location_arg() -> Arg {
    Arg::new("location")
        .long("location")
        .value_name("HREF")
        .help("Value of globalThis.location used by some web APIs")
        .value_hint(ValueHint::Url)
}

fn v8_flags_arg() -> Arg {
    Arg::new("v8-flags")
        .long("v8-flags")
        .num_args(..)
        .use_value_delimiter(true)
        .require_equals(true)
        .value_name("V8_FLAGS")
        .help("To see a list of all available flags use --v8-flags=--help")
}

fn seed_arg() -> Arg {
    Arg::new("seed")
        .long("seed")
        .value_name("NUMBER")
        .help("Set the random number generator seed")
        .value_parser(value_parser!(u64))
}

fn enable_testing_features_arg() -> Arg {
    Arg::new("enable-testing-features-do-not-use")
        .long("enable-testing-features-do-not-use")
        .help("INTERNAL: Enable internal features used during integration testing")
        .action(ArgAction::SetTrue)
        .hide(true)
}

fn trace_ops_arg() -> Arg {
    Arg::new("trace-ops")
        .long("trace-ops")
        .num_args(0..)
        .use_value_delimiter(true)
        .require_equals(true)
        .value_name("OPS")
        .help("Trace low-level op calls")
        .hide(true)
}

fn eszip_arg() -> Arg {
    Arg::new("eszip-internal-do-not-use")
        .hide(true)
        .long("eszip-internal-do-not-use")
        .action(ArgAction::SetTrue)
}

fn preload_arg() -> Arg {
    Arg::new("preload")
        .long("preload")
        .alias("import")
        .value_name("FILE")
        .action(ArgAction::Append)
        .help("A list of files that will be executed before the main module")
        .value_hint(ValueHint::FilePath)
}

fn require_arg() -> Arg {
    Arg::new("require")
        .long("require")
        .value_name("FILE")
        .action(ArgAction::Append)
        .help("A list of CommonJS modules that will be executed before the main module")
        .value_hint(ValueHint::FilePath)
}

fn executable_ext_arg() -> Arg {
    Arg::new("ext")
        .long("ext")
        .help("Set content type of the supplied file")
        .value_parser(["ts", "tsx", "js", "jsx", "mts", "mjs", "cts", "cjs"])
}

fn env_file_arg() -> Arg {
    Arg::new("env-file")
        .long("env-file")
        .alias("env")
        .value_name("FILE")
        .help("Load environment variables from local file")
        .value_hint(ValueHint::FilePath)
        .default_missing_value(".env")
        .require_equals(true)
        .num_args(0..=1)
        .action(ArgAction::Append)
}

fn no_code_cache_arg() -> Arg {
    Arg::new("no-code-cache")
        .long("no-code-cache")
        .help("Disable V8 code cache feature")
        .action(ArgAction::SetTrue)
}

fn watch_arg(takes_files: bool) -> Arg {
    let arg = Arg::new("watch")
        .long("watch")
        .help_heading(FILE_WATCHING_HEADING);

    if takes_files {
        arg.value_name("FILES")
            .num_args(0..)
            .action(ArgAction::Append)
            .require_equals(true)
            .help("Watch for file changes and restart process automatically")
            .value_hint(ValueHint::AnyPath)
    } else {
        arg.action(ArgAction::SetTrue)
            .help("Watch for file changes and restart process automatically")
    }
}

fn hmr_arg(takes_files: bool) -> Arg {
    let arg = Arg::new("hmr")
        .long("watch-hmr")
        .alias("unstable-hmr")
        .help("Watch for file changes and hot replace modules")
        .conflicts_with("watch")
        .help_heading(FILE_WATCHING_HEADING);

    if takes_files {
        arg.value_name("FILES")
            .num_args(0..)
            .action(ArgAction::Append)
            .require_equals(true)
            .help("Watch for file changes and restart process automatically (HMR)")
            .value_hint(ValueHint::AnyPath)
    } else {
        arg.action(ArgAction::SetTrue)
            .help("Watch for file changes and restart process automatically (HMR)")
    }
}

fn watch_exclude_arg() -> Arg {
    Arg::new("watch-exclude")
        .long("watch-exclude")
        .help("Exclude provided files/patterns from watch mode")
        .value_name("FILES")
        .num_args(0..)
        .action(ArgAction::Append)
        .require_equals(true)
        .value_hint(ValueHint::AnyPath)
        .help_heading(FILE_WATCHING_HEADING)
}

fn no_clear_screen_arg() -> Arg {
    Arg::new("no-clear-screen")
        .requires("watch")
        .long("no-clear-screen")
        .action(ArgAction::SetTrue)
        .help("Do not clear terminal screen when under watch mode")
        .help_heading(FILE_WATCHING_HEADING)
}

fn coverage_arg() -> Arg {
    Arg::new("coverage")
        .long("coverage")
        .value_name("DIR")
        .num_args(0..=1)
        .require_equals(true)
        .default_missing_value("coverage")
        .conflicts_with("inspect")
        .conflicts_with("inspect-wait")
        .conflicts_with("inspect-brk")
        .help("Collect coverage profile data into DIR")
        .value_hint(ValueHint::AnyPath)
}

fn tunnel_arg() -> Arg {
    Arg::new("tunnel")
        .long("tunnel")
        .alias("connected")
        .short('t')
        .num_args(0..=1)
        .help("Execute tasks with a tunnel to Deno Deploy")
        .require_equals(true)
        .action(ArgAction::SetTrue)
}

fn allow_scripts_arg() -> Arg {
    Arg::new("allow-scripts")
        .long("allow-scripts")
        .num_args(0..)
        .action(ArgAction::Append)
        .require_equals(true)
        .value_name("PACKAGE")
        .help("Allow running npm lifecycle scripts for the given packages")
}

fn script_arg() -> Arg {
    Arg::new("script_arg")
        .num_args(0..)
        .action(ArgAction::Append)
        .default_value_ifs([
            ("v8-flags", "--help", Some("_")),
            ("v8-flags", "-help", Some("_")),
        ])
        .help("Script arg")
        .value_name("SCRIPT_ARG")
        .value_hint(ValueHint::FilePath)
}

fn permit_no_files_arg() -> Arg {
    Arg::new("permit-no-files")
        .long("permit-no-files")
        .help("Don't return an error code if no files were found")
        .action(ArgAction::SetTrue)
}

fn parallel_arg(descr: &str) -> Arg {
    Arg::new("parallel")
        .long("parallel")
        .help(format!(
            "Run {descr} in parallel. Parallelism defaults to the number of available CPUs"
        ))
        .action(ArgAction::SetTrue)
}

fn lockfile_only_arg() -> Arg {
    Arg::new("lockfile-only")
        .long("lockfile-only")
        .action(ArgAction::SetTrue)
        .help("Install only updating the lockfile")
}

fn add_dev_arg() -> Arg {
    Arg::new("dev")
        .long("dev")
        .short('D')
        .help("Add the package as a dev dependency")
        .action(ArgAction::SetTrue)
}

fn default_registry_args() -> [Arg; 2] {
    [
        Arg::new("npm")
            .long("npm")
            .help("assume unprefixed package names are npm packages")
            .action(ArgAction::SetTrue)
            .conflicts_with("jsr"),
        Arg::new("jsr")
            .long("jsr")
            .help("assume unprefixed package names are jsr packages")
            .action(ArgAction::SetTrue)
            .conflicts_with("npm"),
    ]
}

fn allow_import_arg() -> Arg {
    Arg::new("allow-import")
        .long("allow-import")
        .short('I')
        .num_args(0..)
        .use_value_delimiter(true)
        .require_equals(true)
        .value_name("IP_OR_HOSTNAME")
        .help("Allow importing from remote hosts")
}

fn deny_import_arg() -> Arg {
    Arg::new("deny-import")
        .long("deny-import")
        .num_args(0..)
        .use_value_delimiter(true)
        .require_equals(true)
        .value_name("IP_OR_HOSTNAME")
        .help("Deny importing from remote hosts")
}

fn inspect_publish_uid_arg() -> Arg {
    Arg::new("inspect-publish-uid")
        .long("inspect-publish-uid")
        .value_name("VALUE")
        .require_equals(true)
        .hide(true)
}

fn update_and_outdated_args() -> [Arg; 6] {
    [
        Arg::new("filters")
            .num_args(0..)
            .action(ArgAction::Append)
            .help("Filters selecting which packages to act on"),
        Arg::new("latest")
            .long("latest")
            .action(ArgAction::SetTrue)
            .help("Consider the latest version, regardless of semver constraints")
            .conflicts_with("compatible"),
        Arg::new("compatible")
            .long("compatible")
            .action(ArgAction::SetTrue)
            .help("Only consider versions that satisfy semver requirements"),
        Arg::new("recursive")
            .long("recursive")
            .short('r')
            .action(ArgAction::SetTrue)
            .help("Include all workspace members"),
        min_dep_age_arg(),
        lockfile_only_arg(),
    ]
}

// ============================================================
// Compound arg builders (matching Deno's compose pattern)
// ============================================================

fn compile_args_without_check_args(app: Command) -> Command {
    app.arg(import_map_arg())
        .arg(no_remote_arg())
        .arg(no_npm_arg())
        .arg(node_modules_dir_arg())
        .arg(vendor_arg())
        .arg(node_conditions_arg())
        .arg(config_arg())
        .arg(no_config_arg())
        .arg(reload_arg())
        .args(lock_args())
        .arg(ca_file_arg())
        .arg(unsafely_ignore_certificate_errors_arg())
        .arg(min_dep_age_arg())
}

fn compile_args(app: Command) -> Command {
    compile_args_without_check_args(app.arg(no_check_arg()))
}

fn permission_args(app: Command) -> Command {
    app.arg(
        Arg::new("allow-all")
            .short('A')
            .long("allow-all")
            .conflicts_with("allow-read")
            .conflicts_with("allow-write")
            .conflicts_with("allow-net")
            .conflicts_with("allow-env")
            .conflicts_with("allow-run")
            .conflicts_with("allow-sys")
            .conflicts_with("allow-ffi")
            .conflicts_with("allow-import")
            .conflicts_with("permission-set")
            .help("Allow all permissions")
            .action(ArgAction::Count)
            .hide(true),
    )
    .arg(
        Arg::new("permission-set")
            .long("permission-set")
            .action(ArgAction::Set)
            .num_args(0..=1)
            .require_equals(true)
            .default_missing_value("")
            .short('P')
            .hide(true),
    )
    .arg(
        Arg::new("allow-read")
            .long("allow-read")
            .short('R')
            .num_args(0..)
            .action(ArgAction::Append)
            .require_equals(true)
            .value_name("PATH")
            .value_hint(ValueHint::AnyPath)
            .hide(true),
    )
    .arg(
        Arg::new("deny-read")
            .long("deny-read")
            .num_args(0..)
            .action(ArgAction::Append)
            .require_equals(true)
            .value_name("PATH")
            .value_hint(ValueHint::AnyPath)
            .hide(true),
    )
    .arg(
        Arg::new("ignore-read")
            .long("ignore-read")
            .num_args(0..)
            .action(ArgAction::Append)
            .require_equals(true)
            .value_name("PATH")
            .value_hint(ValueHint::AnyPath)
            .hide(true),
    )
    .arg(
        Arg::new("allow-write")
            .long("allow-write")
            .short('W')
            .num_args(0..)
            .action(ArgAction::Append)
            .require_equals(true)
            .value_name("PATH")
            .value_hint(ValueHint::AnyPath)
            .hide(true),
    )
    .arg(
        Arg::new("deny-write")
            .long("deny-write")
            .num_args(0..)
            .action(ArgAction::Append)
            .require_equals(true)
            .value_name("PATH")
            .value_hint(ValueHint::AnyPath)
            .hide(true),
    )
    .arg(
        Arg::new("allow-net")
            .long("allow-net")
            .short('N')
            .num_args(0..)
            .use_value_delimiter(true)
            .require_equals(true)
            .value_name("IP_OR_HOSTNAME")
            .hide(true),
    )
    .arg(
        Arg::new("deny-net")
            .long("deny-net")
            .num_args(0..)
            .use_value_delimiter(true)
            .require_equals(true)
            .value_name("IP_OR_HOSTNAME")
            .hide(true),
    )
    .arg(
        Arg::new("allow-env")
            .long("allow-env")
            .short('E')
            .num_args(0..)
            .use_value_delimiter(true)
            .require_equals(true)
            .value_name("VARIABLE_NAME")
            .hide(true),
    )
    .arg(
        Arg::new("deny-env")
            .long("deny-env")
            .num_args(0..)
            .use_value_delimiter(true)
            .require_equals(true)
            .value_name("VARIABLE_NAME")
            .hide(true),
    )
    .arg(
        Arg::new("ignore-env")
            .long("ignore-env")
            .num_args(0..)
            .use_value_delimiter(true)
            .require_equals(true)
            .value_name("VARIABLE_NAME")
            .hide(true),
    )
    .arg(
        Arg::new("allow-sys")
            .long("allow-sys")
            .short('S')
            .num_args(0..)
            .use_value_delimiter(true)
            .require_equals(true)
            .value_name("API_NAME")
            .hide(true),
    )
    .arg(
        Arg::new("deny-sys")
            .long("deny-sys")
            .num_args(0..)
            .use_value_delimiter(true)
            .require_equals(true)
            .value_name("API_NAME")
            .hide(true),
    )
    .arg(
        Arg::new("allow-run")
            .long("allow-run")
            .num_args(0..)
            .use_value_delimiter(true)
            .require_equals(true)
            .value_name("PROGRAM_NAME")
            .hide(true),
    )
    .arg(
        Arg::new("deny-run")
            .long("deny-run")
            .num_args(0..)
            .use_value_delimiter(true)
            .require_equals(true)
            .value_name("PROGRAM_NAME")
            .hide(true),
    )
    .arg(
        Arg::new("allow-ffi")
            .long("allow-ffi")
            .num_args(0..)
            .action(ArgAction::Append)
            .require_equals(true)
            .value_name("PATH")
            .value_hint(ValueHint::AnyPath)
            .hide(true),
    )
    .arg(
        Arg::new("deny-ffi")
            .long("deny-ffi")
            .num_args(0..)
            .action(ArgAction::Append)
            .require_equals(true)
            .value_name("PATH")
            .value_hint(ValueHint::AnyPath)
            .hide(true),
    )
    .arg(
        Arg::new("allow-hrtime")
            .long("allow-hrtime")
            .action(ArgAction::SetTrue)
            .hide(true),
    )
    .arg(
        Arg::new("deny-hrtime")
            .long("deny-hrtime")
            .action(ArgAction::SetTrue)
            .hide(true),
    )
    .arg(
        Arg::new("no-prompt")
            .long("no-prompt")
            .action(ArgAction::SetTrue)
            .hide(true),
    )
    .arg(allow_import_arg().hide(true))
    .arg(deny_import_arg().hide(true))
}

fn inspect_args(app: Command) -> Command {
    app.arg(
        Arg::new("inspect")
            .long("inspect")
            .value_name("HOST_PORT")
            .default_missing_value("127.0.0.1:9229")
            .help("Activate inspector on host:port [default: 127.0.0.1:9229]")
            .num_args(0..=1)
            .require_equals(true)
            .help_heading(DEBUGGING_HEADING),
    )
    .arg(
        Arg::new("inspect-brk")
            .long("inspect-brk")
            .value_name("HOST_PORT")
            .default_missing_value("127.0.0.1:9229")
            .help("Activate inspector on host:port, wait for debugger to connect and break at the start of user script")
            .num_args(0..=1)
            .require_equals(true)
            .help_heading(DEBUGGING_HEADING),
    )
    .arg(
        Arg::new("inspect-wait")
            .long("inspect-wait")
            .value_name("HOST_PORT")
            .default_missing_value("127.0.0.1:9229")
            .help("Activate inspector on host:port and wait for debugger to connect before running user code")
            .num_args(0..=1)
            .require_equals(true)
            .help_heading(DEBUGGING_HEADING),
    )
    .arg(inspect_publish_uid_arg())
}

fn cpu_prof_args(cmd: Command) -> Command {
    cmd.arg(
        Arg::new("cpu-prof")
            .long("cpu-prof")
            .help("Start the V8 CPU profiler on startup")
            .action(ArgAction::SetTrue),
    )
    .arg(
        Arg::new("cpu-prof-dir")
            .long("cpu-prof-dir")
            .value_name("DIR")
            .help("Directory where the V8 CPU profiles will be written")
            .value_hint(ValueHint::DirPath)
            .value_parser(value_parser!(String)),
    )
    .arg(
        Arg::new("cpu-prof-name")
            .long("cpu-prof-name")
            .value_name("NAME")
            .help("Filename for the CPU profile")
            .value_parser(value_parser!(String)),
    )
    .arg(
        Arg::new("cpu-prof-interval")
            .long("cpu-prof-interval")
            .value_name("MICROSECONDS")
            .help("Sampling interval in microseconds for CPU profiling")
            .value_parser(value_parser!(u32)),
    )
    .arg(
        Arg::new("cpu-prof-md")
            .long("cpu-prof-md")
            .help("Generate a human-readable markdown report alongside the CPU profile")
            .action(ArgAction::SetTrue),
    )
    .arg(
        Arg::new("cpu-prof-flamegraph")
            .long("cpu-prof-flamegraph")
            .help("Generate an SVG flamegraph alongside the CPU profile")
            .action(ArgAction::SetTrue),
    )
}

fn runtime_misc_args(app: Command) -> Command {
    app.arg(cached_only_arg())
        .arg(location_arg())
        .arg(v8_flags_arg())
        .arg(seed_arg())
        .arg(enable_testing_features_arg())
        .arg(trace_ops_arg())
        .arg(eszip_arg())
        .arg(preload_arg())
        .arg(require_arg())
}

fn runtime_args(
    app: Command,
    include_perms: bool,
    include_inspector: bool,
    include_allow_scripts: bool,
) -> Command {
    let app = compile_args(app);
    let app = if include_perms {
        permission_args(app)
    } else {
        app
    };
    let app = if include_inspector {
        inspect_args(app)
    } else {
        app
    };
    let app = if include_allow_scripts {
        app.arg(allow_scripts_arg())
    } else {
        app
    };
    runtime_misc_args(app)
}

// ============================================================
// run_args — applied to both the root command and the run subcommand
// ============================================================

fn run_args(command: Command, top_level: bool) -> Command {
    cpu_prof_args(
        runtime_args(command, true, true, true)
            .arg(check_arg(false))
            .arg(watch_arg(true))
            .arg(hmr_arg(true))
            .arg(watch_exclude_arg())
            .arg(no_clear_screen_arg())
            .arg(executable_ext_arg())
            .arg(if top_level {
                script_arg().trailing_var_arg(true).hide(true)
            } else {
                script_arg().trailing_var_arg(true)
            })
            .arg(env_file_arg())
            .arg(no_code_cache_arg())
            .arg(coverage_arg()),
    )
    .arg(tunnel_arg())
}

// ============================================================
// Subcommand builders (matching Deno's flags.rs)
// ============================================================

fn run_subcommand() -> Command {
    run_args(
        with_unstable_args(
            Command::new("run").about("Run a JavaScript or TypeScript program, or a task or script"),
        ),
        false,
    )
}

fn serve_subcommand() -> Command {
    cpu_prof_args(
        runtime_args(
            with_unstable_args(
                Command::new("serve").about("Run a server defined in a main module"),
            ),
            true,
            true,
            true,
        )
        .arg(
            Arg::new("port")
                .long("port")
                .help("The TCP port to serve on")
                .value_parser(value_parser!(u16)),
        )
        .arg(
            Arg::new("host")
                .long("host")
                .help("The TCP address to serve on, defaulting to 0.0.0.0"),
        )
        .arg(
            Arg::new("open")
                .long("open")
                .help("Open the browser on the address that the server is running on")
                .action(ArgAction::SetTrue),
        )
        .arg(parallel_arg("multiple server workers"))
        .arg(check_arg(false))
        .arg(watch_arg(true))
        .arg(hmr_arg(true))
        .arg(watch_exclude_arg())
        .arg(no_clear_screen_arg())
        .arg(executable_ext_arg())
        .arg(
            script_arg()
                .required_unless_present_any(["help", "v8-flags"])
                .trailing_var_arg(true),
        )
        .arg(env_file_arg())
        .arg(no_code_cache_arg()),
    )
    .arg(tunnel_arg())
}

fn add_subcommand() -> Command {
    with_unstable_args(Command::new("add").about("Add dependencies to your configuration file"))
        .defer(|cmd| {
            cmd.arg(
                Arg::new("packages")
                    .help("List of packages to add")
                    .required_unless_present("help")
                    .num_args(1..)
                    .action(ArgAction::Append),
            )
            .arg(add_dev_arg())
            .arg(allow_scripts_arg())
            .args(lock_args())
            .arg(lockfile_only_arg())
            .args(default_registry_args())
            .arg(
                Arg::new("save-exact")
                    .long("save-exact")
                    .alias("exact")
                    .help("Save exact version without the caret (^)")
                    .action(ArgAction::SetTrue),
            )
        })
}

fn approve_scripts_subcommand() -> Command {
    with_unstable_args(
        Command::new("approve-scripts").about("Approve npm lifecycle scripts for installed dependencies"),
    )
    .alias("approve-builds")
    .defer(|cmd| {
        cmd.arg(
            Arg::new("packages")
                .help("Packages to approve (npm specifiers)")
                .num_args(0..)
                .action(ArgAction::Append),
        )
        .arg(lockfile_only_arg())
    })
}

fn audit_subcommand() -> Command {
    with_unstable_args(
        Command::new("audit").about("Audit currently installed dependencies"),
    )
    .defer(|cmd| {
        cmd.args(lock_args())
            .arg(
                Arg::new("level")
                    .long("level")
                    .alias("audit-level")
                    .alias("severity")
                    .help("Only show advisories with severity greater or equal to the one specified")
                    .value_parser(["low", "moderate", "high", "critical"]),
            )
            .arg(
                Arg::new("ignore-unfixable")
                    .long("ignore-unfixable")
                    .help("Ignore advisories that don't have any actions to resolve them")
                    .action(ArgAction::SetTrue),
            )
            .arg(
                Arg::new("socket")
                    .long("socket")
                    .help("Check against socket.dev vulnerability database")
                    .action(ArgAction::SetTrue),
            )
            .arg(
                Arg::new("ignore-registry-errors")
                    .long("ignore-registry-errors")
                    .help("Return exit code 0 if remote service(s) responds with an error")
                    .action(ArgAction::SetTrue),
            )
            .arg(
                Arg::new("ignore")
                    .long("ignore")
                    .help("Ignore advisories matching the given CVE IDs")
                    .action(ArgAction::Append)
                    .value_name("CVE"),
            )
    })
}

fn remove_subcommand() -> Command {
    with_unstable_args(
        Command::new("remove").about("Remove dependencies from the configuration file"),
    )
    .defer(|cmd| {
        cmd.arg(
            Arg::new("packages")
                .help("List of packages to remove")
                .required_unless_present("help")
                .num_args(1..)
                .action(ArgAction::Append),
        )
        .args(lock_args())
        .arg(lockfile_only_arg())
    })
}

fn bench_subcommand() -> Command {
    with_unstable_args(
        Command::new("bench").about("Run benchmarks using Deno's built-in bench tool"),
    )
    .defer(|cmd| {
        runtime_args(cmd, true, false, true)
            .arg(check_arg(true))
            .arg(
                Arg::new("json")
                    .long("json")
                    .action(ArgAction::SetTrue)
                    .help("UNSTABLE: Output benchmark result in JSON format"),
            )
            .arg(
                Arg::new("ignore")
                    .long("ignore")
                    .num_args(1..)
                    .action(ArgAction::Append)
                    .require_equals(true)
                    .help("Ignore files"),
            )
            .arg(
                Arg::new("filter")
                    .long("filter")
                    .allow_hyphen_values(true)
                    .help("Run benchmarks with this string or regexp pattern in the bench name"),
            )
            .arg(
                Arg::new("files")
                    .help("List of file names to run")
                    .num_args(..)
                    .action(ArgAction::Append),
            )
            .arg(
                Arg::new("no-run")
                    .long("no-run")
                    .help("Cache bench modules, but don't run benchmarks")
                    .action(ArgAction::SetTrue),
            )
            .arg(permit_no_files_arg())
            .arg(watch_arg(false))
            .arg(watch_exclude_arg())
            .arg(no_clear_screen_arg())
            .arg(script_arg().last(true))
            .arg(env_file_arg())
            .arg(executable_ext_arg())
    })
}

fn bundle_subcommand() -> Command {
    with_unstable_args(
        Command::new("bundle")
            .about("Output a single JavaScript file with all dependencies"),
    )
    .defer(|cmd| {
        compile_args(cmd)
            .arg(check_arg(false))
            .arg(
                Arg::new("file")
                    .num_args(1..)
                    .required_unless_present("help")
                    .value_hint(ValueHint::FilePath),
            )
            .arg(
                Arg::new("output")
                    .long("output")
                    .short('o')
                    .help("Output path")
                    .num_args(1)
                    .value_parser(value_parser!(String))
                    .value_hint(ValueHint::FilePath),
            )
            .arg(
                Arg::new("outdir")
                    .long("outdir")
                    .help("Output directory for bundled files")
                    .num_args(1)
                    .value_parser(value_parser!(String))
                    .value_hint(ValueHint::DirPath),
            )
            .arg(
                Arg::new("external")
                    .long("external")
                    .action(ArgAction::Append)
                    .num_args(1)
                    .value_parser(value_parser!(String)),
            )
            .arg(
                Arg::new("format")
                    .long("format")
                    .num_args(1)
                    .value_parser(["esm", "cjs", "iife"])
                    .default_value("esm"),
            )
            .arg(
                Arg::new("packages")
                    .long("packages")
                    .help("How to handle packages")
                    .num_args(1)
                    .value_parser(["bundle", "external"])
                    .default_value("bundle"),
            )
            .arg(
                Arg::new("minify")
                    .long("minify")
                    .help("Minify the output")
                    .action(ArgAction::SetTrue),
            )
            .arg(
                Arg::new("keep-names")
                    .long("keep-names")
                    .help("Keep function and class names")
                    .action(ArgAction::SetTrue),
            )
            .arg(
                Arg::new("code-splitting")
                    .long("code-splitting")
                    .help("Enable code splitting")
                    .action(ArgAction::SetTrue),
            )
            .arg(
                Arg::new("inline-imports")
                    .long("inline-imports")
                    .help("Whether to inline imported modules into the importing file")
                    .require_equals(true)
                    .default_value("true")
                    .default_missing_value("true")
                    .value_parser(value_parser!(bool))
                    .num_args(0..=1)
                    .action(ArgAction::Set),
            )
            .arg(
                Arg::new("sourcemap")
                    .long("sourcemap")
                    .help("Generate source map")
                    .require_equals(true)
                    .default_missing_value("linked")
                    .value_parser(["linked", "inline", "external"])
                    .num_args(0..=1)
                    .action(ArgAction::Set),
            )
            .arg(
                Arg::new("watch")
                    .long("watch")
                    .help("Watch and rebuild on changes")
                    .action(ArgAction::SetTrue),
            )
            .arg(
                Arg::new("platform")
                    .long("platform")
                    .help("Platform to bundle for")
                    .num_args(1)
                    .value_parser(["browser", "deno"])
                    .default_value("deno"),
            )
            .arg(allow_scripts_arg())
            .arg(allow_import_arg())
            .arg(deny_import_arg())
    })
}

fn cache_subcommand() -> Command {
    with_unstable_args(
        Command::new("cache").about("Cache and compile remote dependencies"),
    )
    .hide(true)
    .defer(|cmd| {
        compile_args(cmd)
            .arg(check_arg(false))
            .arg(
                Arg::new("file")
                    .num_args(1..)
                    .required_unless_present("help")
                    .value_hint(ValueHint::FilePath),
            )
            .arg(allow_scripts_arg())
            .arg(allow_import_arg())
            .arg(deny_import_arg())
            .arg(env_file_arg())
    })
}

fn clean_subcommand() -> Command {
    with_unstable_args(
        Command::new("clean").about("Remove the cache directory ($DENO_DIR)"),
    )
    .defer(|cmd| {
        cmd.arg(
            Arg::new("except-paths")
                .required_if_eq("except", "true")
                .num_args(1..)
                .value_hint(ValueHint::FilePath),
        )
        .arg(
            Arg::new("except")
                .long("except")
                .short('e')
                .help("Retain cache data needed by the given files")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("dry-run")
                .long("dry-run")
                .action(ArgAction::SetTrue)
                .help("Show what would be removed without performing any actions")
                .requires("except"),
        )
        .arg(node_modules_dir_arg().requires("except"))
        .arg(vendor_arg().requires("except"))
    })
}

fn check_subcommand() -> Command {
    with_unstable_args(
        Command::new("check").about("Download and type-check without execution"),
    )
    .defer(|cmd| {
        compile_args_without_check_args(cmd)
            .arg(no_code_cache_arg())
            .arg(
                Arg::new("all")
                    .long("all")
                    .help("Type-check all code, including remote modules and npm packages")
                    .action(ArgAction::SetTrue)
                    .conflicts_with("no-remote"),
            )
            .arg(
                Arg::new("remote")
                    .long("remote")
                    .help("Type-check all modules, including remote ones")
                    .action(ArgAction::SetTrue)
                    .conflicts_with("no-remote")
                    .hide(true),
            )
            .arg(
                Arg::new("doc")
                    .long("doc")
                    .help("Type-check code blocks in JSDoc as well as actual code")
                    .action(ArgAction::SetTrue),
            )
            .arg(
                Arg::new("doc-only")
                    .long("doc-only")
                    .help("Type-check code blocks in JSDoc and Markdown only")
                    .action(ArgAction::SetTrue)
                    .conflicts_with("doc"),
            )
            .arg(
                Arg::new("check-js")
                    .long("check-js")
                    .help("Enable type-checking of JavaScript files")
                    .action(ArgAction::SetTrue),
            )
            .arg(
                Arg::new("file")
                    .num_args(1..)
                    .value_hint(ValueHint::FilePath),
            )
            .arg(allow_import_arg())
            .arg(deny_import_arg())
            .arg(v8_flags_arg())
    })
}

fn compile_subcommand() -> Command {
    with_unstable_args(
        Command::new("compile")
            .about("Compiles the given script into a self contained executable"),
    )
    .defer(|cmd| {
        runtime_args(cmd, true, false, true)
            .arg(check_arg(true))
            .arg(
                Arg::new("include")
                    .long("include")
                    .help("Includes an additional module or file/directory in the compiled executable")
                    .action(ArgAction::Append)
                    .value_hint(ValueHint::FilePath)
                    .help_heading(COMPILE_HEADING),
            )
            .arg(
                Arg::new("exclude")
                    .long("exclude")
                    .help("Excludes a file/directory in the compiled executable")
                    .action(ArgAction::Append)
                    .value_hint(ValueHint::FilePath)
                    .help_heading(COMPILE_HEADING),
            )
            .arg(
                Arg::new("output")
                    .long("output")
                    .short('o')
                    .value_parser(value_parser!(String))
                    .help("Output file")
                    .value_hint(ValueHint::FilePath)
                    .help_heading(COMPILE_HEADING),
            )
            .arg(
                Arg::new("target")
                    .long("target")
                    .help("Target OS architecture")
                    .value_parser([
                        "x86_64-unknown-linux-gnu",
                        "aarch64-unknown-linux-gnu",
                        "x86_64-pc-windows-msvc",
                        "x86_64-apple-darwin",
                        "aarch64-apple-darwin",
                    ])
                    .help_heading(COMPILE_HEADING),
            )
            .arg(no_code_cache_arg())
            .arg(
                Arg::new("no-terminal")
                    .long("no-terminal")
                    .help("Hide terminal on Windows")
                    .action(ArgAction::SetTrue)
                    .help_heading(COMPILE_HEADING),
            )
            .arg(
                Arg::new("icon")
                    .long("icon")
                    .help("Set the icon of the executable on Windows (.ico)")
                    .value_parser(value_parser!(String))
                    .help_heading(COMPILE_HEADING),
            )
            .arg(
                Arg::new("self-extracting")
                    .long("self-extracting")
                    .help("Create a self-extracting binary")
                    .action(ArgAction::SetTrue)
                    .help_heading(COMPILE_HEADING),
            )
            .arg(executable_ext_arg())
            .arg(env_file_arg())
            .arg(
                script_arg()
                    .required_unless_present("help")
                    .trailing_var_arg(true),
            )
    })
}

fn completions_subcommand() -> Command {
    with_unstable_args(
        Command::new("completions").about("Output shell completion script to standard output"),
    )
    .defer(|cmd| {
        cmd.disable_help_subcommand(true)
            .arg(
                Arg::new("shell")
                    .value_parser(["bash", "fish", "powershell", "zsh", "fig"])
                    .required_unless_present("help"),
            )
            .arg(
                Arg::new("dynamic")
                    .long("dynamic")
                    .action(ArgAction::SetTrue)
                    .help("Generate dynamic completions for the given shell"),
            )
    })
}

fn coverage_subcommand() -> Command {
    with_unstable_args(
        Command::new("coverage").about("Print coverage reports from coverage profiles"),
    )
    .defer(|cmd| {
        cmd.arg(
            Arg::new("ignore")
                .long("ignore")
                .num_args(1..)
                .action(ArgAction::Append)
                .require_equals(true)
                .help("Ignore coverage files")
                .value_hint(ValueHint::AnyPath),
        )
        .arg(
            Arg::new("include")
                .long("include")
                .num_args(1..)
                .action(ArgAction::Append)
                .value_name("regex")
                .require_equals(true)
                .default_value(r"^file:")
                .help("Include source files in the report"),
        )
        .arg(
            Arg::new("exclude")
                .long("exclude")
                .num_args(1..)
                .action(ArgAction::Append)
                .value_name("regex")
                .require_equals(true)
                .default_value(r"test\.(js|mjs|ts|jsx|tsx)$")
                .help("Exclude source files from the report"),
        )
        .arg(
            Arg::new("lcov")
                .long("lcov")
                .help("Output coverage report in lcov format")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("output")
                .requires("lcov")
                .long("output")
                .value_parser(value_parser!(String))
                .help("Exports the coverage report in lcov format to the given file")
                .require_equals(true)
                .value_hint(ValueHint::FilePath),
        )
        .arg(
            Arg::new("html")
                .long("html")
                .help("Output coverage report in HTML format in the given directory")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("detailed")
                .long("detailed")
                .help("Output coverage report in detailed format in the terminal")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("files")
                .num_args(0..)
                .action(ArgAction::Append)
                .value_hint(ValueHint::AnyPath),
        )
    })
}

fn deploy_subcommand() -> Command {
    Command::new("deploy").arg(
        Arg::new("args")
            .num_args(0..)
            .action(ArgAction::Append)
            .trailing_var_arg(true)
            .allow_hyphen_values(true),
    )
}

fn sandbox_subcommand() -> Command {
    Command::new("sandbox").arg(
        Arg::new("args")
            .num_args(0..)
            .action(ArgAction::Append)
            .trailing_var_arg(true)
            .allow_hyphen_values(true),
    )
}

fn doc_subcommand() -> Command {
    with_unstable_args(
        Command::new("doc").about("Show documentation for a module"),
    )
    .defer(|cmd| {
        cmd.arg(import_map_arg())
            .arg(reload_arg())
            .args(lock_args())
            .arg(no_npm_arg())
            .arg(no_remote_arg())
            .arg(allow_import_arg())
            .arg(deny_import_arg())
            .arg(
                Arg::new("json")
                    .long("json")
                    .help("Output documentation in JSON format")
                    .action(ArgAction::SetTrue)
                    .help_heading(DOC_HEADING),
            )
            .arg(
                Arg::new("html")
                    .long("html")
                    .help("Output documentation in HTML format")
                    .action(ArgAction::SetTrue)
                    .display_order(1000)
                    .conflicts_with("json")
                    .help_heading(DOC_HEADING),
            )
            .arg(
                Arg::new("name")
                    .long("name")
                    .help("The name that will be used in the docs")
                    .action(ArgAction::Set)
                    .require_equals(true)
                    .help_heading(DOC_HEADING),
            )
            .arg(
                Arg::new("category-docs")
                    .long("category-docs")
                    .help("Path to a JSON file keyed by category")
                    .requires("html")
                    .action(ArgAction::Set)
                    .require_equals(true)
                    .help_heading(DOC_HEADING),
            )
            .arg(
                Arg::new("symbol-redirect-map")
                    .long("symbol-redirect-map")
                    .help("Path to a JSON file keyed by file, with an inner map of symbol to an external link")
                    .requires("html")
                    .action(ArgAction::Set)
                    .require_equals(true)
                    .help_heading(DOC_HEADING),
            )
            .arg(
                Arg::new("strip-trailing-html")
                    .long("strip-trailing-html")
                    .help("Remove trailing .html from various links")
                    .requires("html")
                    .action(ArgAction::SetTrue)
                    .help_heading(DOC_HEADING),
            )
            .arg(
                Arg::new("default-symbol-map")
                    .long("default-symbol-map")
                    .help("Uses the provided mapping of default name to wanted name for usage blocks")
                    .requires("html")
                    .action(ArgAction::Set)
                    .require_equals(true)
                    .help_heading(DOC_HEADING),
            )
            .arg(
                Arg::new("output")
                    .long("output")
                    .help("Directory for HTML documentation output")
                    .action(ArgAction::Set)
                    .require_equals(true)
                    .value_hint(ValueHint::DirPath)
                    .value_parser(value_parser!(String))
                    .help_heading(DOC_HEADING),
            )
            .arg(
                Arg::new("private")
                    .long("private")
                    .help("Output private documentation")
                    .action(ArgAction::SetTrue)
                    .help_heading(DOC_HEADING),
            )
            .arg(
                Arg::new("filter")
                    .long("filter")
                    .help("Dot separated path to symbol")
                    .conflicts_with("json")
                    .conflicts_with("lint")
                    .conflicts_with("html")
                    .help_heading(DOC_HEADING),
            )
            .arg(
                Arg::new("lint")
                    .long("lint")
                    .help("Output documentation diagnostics")
                    .action(ArgAction::SetTrue)
                    .help_heading(DOC_HEADING),
            )
            .allow_hyphen_values(true)
            .arg(
                Arg::new("source_file")
                    .num_args(1..)
                    .action(ArgAction::Append)
                    .value_hint(ValueHint::FilePath)
                    .required_if_eq_any([("html", "true"), ("lint", "true")]),
            )
    })
}

fn eval_subcommand() -> Command {
    with_unstable_args(
        Command::new("eval").about("Evaluate JavaScript from the command line"),
    )
    .defer(|cmd| {
        cpu_prof_args(
            runtime_args(cmd, false, true, true)
                .arg(check_arg(false))
                .arg(executable_ext_arg())
                .arg(
                    Arg::new("print")
                        .long("print")
                        .short('p')
                        .help("print result to stdout")
                        .action(ArgAction::SetTrue),
                )
                .arg(
                    Arg::new("code_arg")
                        .num_args(1..)
                        .action(ArgAction::Append)
                        .help("Code to evaluate")
                        .value_name("CODE_ARG")
                        .required_unless_present("help"),
                )
                .arg(env_file_arg()),
        )
    })
}

fn fmt_subcommand() -> Command {
    with_unstable_args(
        Command::new("fmt").about("Auto-format various file types"),
    )
    .defer(|cmd| {
        cmd.arg(config_arg())
            .arg(no_config_arg())
            .arg(
                Arg::new("check")
                    .long("check")
                    .help("Check if the source files are formatted")
                    .num_args(0)
                    .help_heading(FMT_HEADING),
            )
            .arg(
                Arg::new("fail-fast")
                    .long("fail-fast")
                    .alias("failfast")
                    .help("Stop checking files on first format error")
                    .num_args(0)
                    .requires("check")
                    .help_heading(FMT_HEADING),
            )
            .arg(
                Arg::new("ext")
                    .long("ext")
                    .help("Set content type of the supplied file")
                    .value_parser([
                        "ts", "tsx", "js", "jsx", "mts", "mjs", "cts", "cjs", "md", "json",
                        "jsonc", "css", "scss", "sass", "less", "html", "svelte", "vue", "astro",
                        "yml", "yaml", "ipynb", "sql", "vto", "njk",
                    ])
                    .help_heading(FMT_HEADING)
                    .requires("files"),
            )
            .arg(
                Arg::new("ignore")
                    .long("ignore")
                    .num_args(1..)
                    .action(ArgAction::Append)
                    .require_equals(true)
                    .help("Ignore formatting particular source files")
                    .value_hint(ValueHint::AnyPath)
                    .help_heading(FMT_HEADING),
            )
            .arg(
                Arg::new("files")
                    .num_args(1..)
                    .action(ArgAction::Append)
                    .value_hint(ValueHint::AnyPath),
            )
            .arg(permit_no_files_arg())
            .arg(watch_arg(false))
            .arg(watch_exclude_arg())
            .arg(no_clear_screen_arg())
            .arg(
                Arg::new("use-tabs")
                    .long("use-tabs")
                    .alias("options-use-tabs")
                    .num_args(0..=1)
                    .value_parser(value_parser!(bool))
                    .default_missing_value("true")
                    .require_equals(true)
                    .help("Use tabs instead of spaces for indentation [default: false]")
                    .help_heading(FMT_HEADING),
            )
            .arg(
                Arg::new("line-width")
                    .long("line-width")
                    .alias("options-line-width")
                    .help("Define maximum line width [default: 80]")
                    .value_parser(value_parser!(NonZeroU32))
                    .help_heading(FMT_HEADING),
            )
            .arg(
                Arg::new("indent-width")
                    .long("indent-width")
                    .alias("options-indent-width")
                    .help("Define indentation width [default: 2]")
                    .value_parser(value_parser!(NonZeroU8))
                    .help_heading(FMT_HEADING),
            )
            .arg(
                Arg::new("single-quote")
                    .long("single-quote")
                    .alias("options-single-quote")
                    .num_args(0..=1)
                    .value_parser(value_parser!(bool))
                    .default_missing_value("true")
                    .require_equals(true)
                    .help("Use single quotes [default: false]")
                    .help_heading(FMT_HEADING),
            )
            .arg(
                Arg::new("prose-wrap")
                    .long("prose-wrap")
                    .alias("options-prose-wrap")
                    .value_parser(["always", "never", "preserve"])
                    .help("Define how prose should be wrapped [default: always]")
                    .help_heading(FMT_HEADING),
            )
            .arg(
                Arg::new("no-semicolons")
                    .long("no-semicolons")
                    .alias("options-no-semicolons")
                    .num_args(0..=1)
                    .value_parser(value_parser!(bool))
                    .default_missing_value("true")
                    .require_equals(true)
                    .help("Don't use semicolons except where necessary [default: false]")
                    .help_heading(FMT_HEADING),
            )
            .arg(
                Arg::new("unstable-css")
                    .long("unstable-css")
                    .help("Enable formatting CSS, SCSS, Sass and Less files")
                    .value_parser(value_parser!(bool))
                    .action(ArgAction::SetTrue)
                    .help_heading(FMT_HEADING)
                    .hide(true),
            )
            .arg(
                Arg::new("unstable-html")
                    .long("unstable-html")
                    .help("Enable formatting HTML files")
                    .value_parser(value_parser!(bool))
                    .action(ArgAction::SetTrue)
                    .help_heading(FMT_HEADING)
                    .hide(true),
            )
            .arg(
                Arg::new("unstable-component")
                    .long("unstable-component")
                    .help("Enable formatting Svelte, Vue, Astro and Angular files")
                    .value_parser(value_parser!(bool))
                    .action(ArgAction::SetTrue)
                    .help_heading(FMT_HEADING),
            )
            .arg(
                Arg::new("unstable-yaml")
                    .long("unstable-yaml")
                    .help("Enable formatting YAML files")
                    .value_parser(value_parser!(bool))
                    .action(ArgAction::SetTrue)
                    .help_heading(FMT_HEADING)
                    .hide(true),
            )
            .arg(
                Arg::new("unstable-sql")
                    .long("unstable-sql")
                    .help("Enable formatting SQL files")
                    .value_parser(value_parser!(bool))
                    .action(ArgAction::SetTrue)
                    .help_heading(FMT_HEADING),
            )
    })
}

fn init_subcommand() -> Command {
    with_unstable_args(
        Command::new("init").about("Scaffolds a basic Deno project"),
    )
    .defer(|cmd| {
        cmd.arg(
            Arg::new("args")
                .num_args(0..)
                .action(ArgAction::Append)
                .value_name("DIRECTORY OR PACKAGE")
                .trailing_var_arg(true),
        )
        .arg(
            Arg::new("npm")
                .long("npm")
                .help("Generate a npm create-* project")
                .conflicts_with_all(["lib", "serve", "empty", "jsr"])
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("jsr")
                .long("jsr")
                .help("Generate a project from a JSR package")
                .conflicts_with_all(["lib", "serve", "empty", "npm"])
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("lib")
                .long("lib")
                .help("Generate an example library project")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("serve")
                .long("serve")
                .help("Generate an example project for deno serve")
                .conflicts_with("lib")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("empty")
                .long("empty")
                .help("Generate a minimal project with just main.ts and deno.json")
                .conflicts_with_all(["lib", "serve", "npm", "jsr"])
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("yes")
                .short('y')
                .long("yes")
                .help("Bypass the prompt and run with full permissions")
                .action(ArgAction::SetTrue),
        )
    })
}

fn create_subcommand() -> Command {
    with_unstable_args(
        Command::new("create").about("Scaffolds a project from a package"),
    )
    .defer(|cmd| {
        cmd.arg(
            Arg::new("package")
                .required_unless_present("help")
                .value_name("PACKAGE"),
        )
        .arg(
            Arg::new("package_args")
                .num_args(0..)
                .action(ArgAction::Append)
                .value_name("ARGS")
                .last(true),
        )
        .arg(
            Arg::new("npm")
                .long("npm")
                .help("Treat unprefixed package names as npm packages")
                .conflicts_with("jsr")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("jsr")
                .long("jsr")
                .help("Treat unprefixed package names as JSR packages")
                .conflicts_with("npm")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("yes")
                .short('y')
                .long("yes")
                .help("Bypass the prompt and run with full permissions")
                .action(ArgAction::SetTrue),
        )
    })
}

fn info_subcommand() -> Command {
    with_unstable_args(
        Command::new("info").about("Show information about a module or the cache directories"),
    )
    .defer(|cmd| {
        cmd.arg(Arg::new("file").value_hint(ValueHint::FilePath))
            .arg(reload_arg().requires("file"))
            .arg(ca_file_arg())
            .arg(unsafely_ignore_certificate_errors_arg())
            .arg(
                location_arg()
                    .conflicts_with("file")
                    .help("Show files used for origin bound APIs"),
            )
            .arg(no_check_arg().hide(true))
            .arg(no_config_arg())
            .arg(no_remote_arg())
            .arg(no_npm_arg())
            .args(lock_args())
            .arg(config_arg())
            .arg(import_map_arg())
            .arg(node_modules_dir_arg())
            .arg(vendor_arg())
            .arg(
                Arg::new("json")
                    .long("json")
                    .help("UNSTABLE: Outputs the information in JSON format")
                    .action(ArgAction::SetTrue),
            )
            .arg(allow_import_arg())
            .arg(deny_import_arg())
    })
}

fn install_subcommand() -> Command {
    with_unstable_args(
        Command::new("install")
            .about("Installs dependencies either in the local project or globally to a bin directory")
            .visible_alias("i"),
    )
    .defer(|cmd| {
        permission_args(runtime_args(cmd, false, true, false))
            .arg(check_arg(true))
            .arg(allow_scripts_arg())
            .arg(
                Arg::new("cmd")
                    .required_if_eq("global", "true")
                    .required_if_eq("entrypoint", "true")
                    .num_args(1..)
                    .value_hint(ValueHint::FilePath),
            )
            .arg(script_arg().last(true))
            .arg(
                Arg::new("name")
                    .long("name")
                    .short('n')
                    .requires("global")
                    .help("Executable file name"),
            )
            .arg(
                Arg::new("root")
                    .long("root")
                    .requires("global")
                    .help("Installation root")
                    .value_hint(ValueHint::DirPath),
            )
            .arg(
                Arg::new("force")
                    .long("force")
                    .requires("global")
                    .short('f')
                    .help("Forcefully overwrite existing installation")
                    .action(ArgAction::SetTrue),
            )
            .arg(
                Arg::new("compile")
                    .long("compile")
                    .requires("global")
                    .help("Install the script as a compiled executable")
                    .action(ArgAction::SetTrue),
            )
            .arg(
                Arg::new("global")
                    .long("global")
                    .short('g')
                    .help("Install a package or script as a globally available executable")
                    .action(ArgAction::SetTrue),
            )
            .arg(
                Arg::new("entrypoint")
                    .long("entrypoint")
                    .short('e')
                    .conflicts_with("global")
                    .action(ArgAction::SetTrue)
                    .help("Install dependents of the specified entrypoint(s)"),
            )
            .arg(env_file_arg())
            .arg(
                add_dev_arg()
                    .conflicts_with("entrypoint")
                    .conflicts_with("global"),
            )
            .args(
                default_registry_args()
                    .into_iter()
                    .map(|arg| arg.conflicts_with("entrypoint").conflicts_with("global")),
            )
            .arg(
                Arg::new("save-exact")
                    .long("save-exact")
                    .alias("exact")
                    .help("Save exact version without the caret (^)")
                    .action(ArgAction::SetTrue)
                    .conflicts_with("entrypoint")
                    .conflicts_with("global"),
            )
            .arg(lockfile_only_arg().conflicts_with("global"))
    })
}

fn json_reference_subcommand() -> Command {
    Command::new("json_reference").hide(true)
}

fn jupyter_subcommand() -> Command {
    with_unstable_args(
        Command::new("jupyter").about("Deno kernel for Jupyter notebooks"),
    )
    .arg(
        Arg::new("install")
            .long("install")
            .help("Install a kernelspec")
            .conflicts_with("kernel")
            .action(ArgAction::SetTrue),
    )
    .arg(
        Arg::new("name")
            .long("name")
            .short('n')
            .help("Set a name for the kernel (defaults to 'deno')")
            .value_parser(value_parser!(String))
            .conflicts_with("kernel"),
    )
    .arg(
        Arg::new("display")
            .long("display")
            .short('d')
            .help("Set a display name for the kernel (defaults to 'Deno')")
            .value_parser(value_parser!(String))
            .requires("install"),
    )
    .arg(
        Arg::new("force")
            .long("force")
            .help("Force installation of a kernel")
            .requires("install")
            .action(ArgAction::SetTrue),
    )
    .arg(
        Arg::new("kernel")
            .long("kernel")
            .help("Start the kernel")
            .conflicts_with("install")
            .requires("conn")
            .action(ArgAction::SetTrue),
    )
    .arg(
        Arg::new("conn")
            .long("conn")
            .help("Path to JSON file describing connection parameters")
            .value_parser(value_parser!(String))
            .value_hint(ValueHint::FilePath)
            .conflicts_with("install"),
    )
}

fn lsp_subcommand() -> Command {
    Command::new("lsp").about("The deno lsp subcommand provides a way for code editors and IDEs to interact with Deno")
}

fn lint_subcommand() -> Command {
    with_unstable_args(
        Command::new("lint").about("Lint JavaScript/TypeScript source code"),
    )
    .defer(|cmd| {
        cmd.arg(
            Arg::new("fix")
                .long("fix")
                .help("Fix any linting errors for rules that support it")
                .action(ArgAction::SetTrue)
                .help_heading(LINT_HEADING),
        )
        .arg(
            Arg::new("ext")
                .long("ext")
                .require_equals(true)
                .value_name("EXT")
                .help("Specify the file extension to lint when reading from stdin"),
        )
        .arg(
            Arg::new("rules")
                .long("rules")
                .help("List available rules")
                .action(ArgAction::SetTrue)
                .help_heading(LINT_HEADING),
        )
        .arg(
            Arg::new("rules-tags")
                .long("rules-tags")
                .require_equals(true)
                .num_args(1..)
                .action(ArgAction::Append)
                .use_value_delimiter(true)
                .help("Use set of rules with a tag")
                .help_heading(LINT_HEADING),
        )
        .arg(
            Arg::new("rules-include")
                .long("rules-include")
                .require_equals(true)
                .num_args(1..)
                .use_value_delimiter(true)
                .conflicts_with("rules")
                .help("Include lint rules")
                .help_heading(LINT_HEADING),
        )
        .arg(
            Arg::new("rules-exclude")
                .long("rules-exclude")
                .require_equals(true)
                .num_args(1..)
                .use_value_delimiter(true)
                .conflicts_with("rules")
                .help("Exclude lint rules")
                .help_heading(LINT_HEADING),
        )
        .arg(no_config_arg())
        .arg(config_arg())
        .arg(
            Arg::new("ignore")
                .long("ignore")
                .num_args(1..)
                .action(ArgAction::Append)
                .require_equals(true)
                .help("Ignore linting particular source files")
                .value_hint(ValueHint::AnyPath)
                .help_heading(LINT_HEADING),
        )
        .arg(
            Arg::new("json")
                .long("json")
                .help("Output lint result in JSON format")
                .action(ArgAction::SetTrue)
                .help_heading(LINT_HEADING),
        )
        .arg(
            Arg::new("compact")
                .long("compact")
                .help("Output lint result in compact format")
                .action(ArgAction::SetTrue)
                .conflicts_with("json")
                .help_heading(LINT_HEADING),
        )
        .arg(
            Arg::new("files")
                .num_args(1..)
                .action(ArgAction::Append)
                .value_hint(ValueHint::AnyPath),
        )
        .arg(permit_no_files_arg())
        .arg(watch_arg(false))
        .arg(watch_exclude_arg())
        .arg(no_clear_screen_arg())
        .arg(allow_import_arg())
        .arg(deny_import_arg())
    })
}

fn repl_subcommand() -> Command {
    with_unstable_args(
        Command::new("repl").about("Starts a read-eval-print-loop"),
    )
    .defer(|cmd| {
        let cmd = compile_args_without_check_args(cmd);
        let cmd = inspect_args(cmd);
        let cmd = permission_args(cmd);
        let cmd = runtime_misc_args(cmd);

        cmd.arg(
            Arg::new("eval-file")
                .long("eval-file")
                .num_args(1..)
                .action(ArgAction::Append)
                .use_value_delimiter(true)
                .help("Evaluates the provided file(s) as scripts when the REPL starts")
                .value_hint(ValueHint::AnyPath),
        )
        .arg(
            Arg::new("eval")
                .long("eval")
                .help("Evaluates the provided code when the REPL starts")
                .value_name("code"),
        )
        .arg(
            Arg::new("json")
                .long("json")
                .action(ArgAction::SetTrue)
                .hide(true),
        )
    })
    .arg(env_file_arg())
    .arg(
        Arg::new("args")
            .num_args(0..)
            .action(ArgAction::Append)
            .value_name("ARGS")
            .last(true),
    )
}

fn task_subcommand() -> Command {
    with_unstable_args(
        Command::new("task").about("Run a task defined in the configuration file"),
    )
    .defer(|cmd| {
        cmd.allow_external_subcommands(true)
            .subcommand_value_name("TASK")
            .arg(config_arg())
            .args(lock_args())
            .arg(
                Arg::new("cwd")
                    .long("cwd")
                    .value_name("DIR")
                    .help("Specify the directory to run the task in")
                    .value_hint(ValueHint::DirPath),
            )
            .arg(
                Arg::new("recursive")
                    .long("recursive")
                    .short('r')
                    .help("Run the task in all projects in the workspace")
                    .action(ArgAction::SetTrue),
            )
            .arg(
                Arg::new("filter")
                    .long("filter")
                    .short('f')
                    .help("Filter members of the workspace by name")
                    .value_parser(value_parser!(String)),
            )
            .arg(
                Arg::new("eval")
                    .long("eval")
                    .help("Evaluate the passed value as if it was a task in a configuration file")
                    .action(ArgAction::SetTrue),
            )
            .arg(node_modules_dir_arg())
            .arg(tunnel_arg())
    })
}

fn test_subcommand() -> Command {
    with_unstable_args(
        Command::new("test").about("Run tests using Deno's built-in test runner"),
    )
    .defer(|cmd| {
        runtime_args(cmd, true, true, true)
            .arg(check_arg(true))
            .arg(
                Arg::new("ignore")
                    .long("ignore")
                    .num_args(1..)
                    .action(ArgAction::Append)
                    .require_equals(true)
                    .help("Ignore files")
                    .value_hint(ValueHint::AnyPath),
            )
            .arg(
                Arg::new("no-run")
                    .long("no-run")
                    .help("Cache test modules, but don't run tests")
                    .action(ArgAction::SetTrue)
                    .help_heading(TEST_HEADING),
            )
            .arg(
                Arg::new("trace-leaks")
                    .long("trace-leaks")
                    .help("Enable tracing of leaks")
                    .action(ArgAction::SetTrue)
                    .help_heading(TEST_HEADING),
            )
            .arg(
                Arg::new("doc")
                    .long("doc")
                    .help("Evaluate code blocks in JSDoc and Markdown")
                    .action(ArgAction::SetTrue)
                    .help_heading(TEST_HEADING),
            )
            .arg(
                Arg::new("fail-fast")
                    .long("fail-fast")
                    .alias("failfast")
                    .help("Stop after N errors. Defaults to stopping after first failure")
                    .num_args(0..=1)
                    .require_equals(true)
                    .value_name("N")
                    .value_parser(value_parser!(NonZeroUsize))
                    .help_heading(TEST_HEADING),
            )
            .arg(permit_no_files_arg().help_heading(TEST_HEADING))
            .arg(
                Arg::new("filter")
                    .allow_hyphen_values(true)
                    .long("filter")
                    .help("Run tests with this string or regexp pattern in the test name")
                    .help_heading(TEST_HEADING),
            )
            .arg(
                Arg::new("shuffle")
                    .long("shuffle")
                    .value_name("NUMBER")
                    .help("Shuffle the order in which the tests are run")
                    .num_args(0..=1)
                    .require_equals(true)
                    .value_parser(value_parser!(u64))
                    .help_heading(TEST_HEADING),
            )
            .arg(
                Arg::new("coverage")
                    .long("coverage")
                    .value_name("DIR")
                    .num_args(0..=1)
                    .require_equals(true)
                    .default_missing_value("coverage")
                    .conflicts_with("inspect")
                    .conflicts_with("inspect-wait")
                    .conflicts_with("inspect-brk")
                    .help("Collect coverage profile data into DIR")
                    .help_heading(TEST_HEADING),
            )
            .arg(
                Arg::new("coverage-raw-data-only")
                    .long("coverage-raw-data-only")
                    .help("Only collect raw coverage data, without generating a report")
                    .action(ArgAction::SetTrue)
                    .help_heading(TEST_HEADING),
            )
            .arg(
                Arg::new("clean")
                    .long("clean")
                    .help("Empty the temporary coverage profile data directory before running tests")
                    .action(ArgAction::SetTrue)
                    .help_heading(TEST_HEADING),
            )
            .arg(parallel_arg("test modules"))
            .arg(
                Arg::new("files")
                    .help("List of file names to run")
                    .num_args(0..)
                    .action(ArgAction::Append)
                    .value_hint(ValueHint::AnyPath),
            )
            .arg(
                watch_arg(true)
                    .conflicts_with("no-run")
                    .conflicts_with("coverage"),
            )
            .arg(watch_exclude_arg())
            .arg(no_clear_screen_arg())
            .arg(script_arg().last(true))
            .arg(
                Arg::new("junit-path")
                    .long("junit-path")
                    .value_name("PATH")
                    .value_hint(ValueHint::FilePath)
                    .help("Write a JUnit XML test report to PATH")
                    .help_heading(TEST_HEADING),
            )
            .arg(
                Arg::new("reporter")
                    .long("reporter")
                    .help("Select reporter to use. Default to 'pretty'")
                    .value_parser(["pretty", "dot", "junit", "tap"])
                    .help_heading(TEST_HEADING),
            )
            .arg(
                Arg::new("hide-stacktraces")
                    .long("hide-stacktraces")
                    .help("Hide stack traces for errors in failure test results")
                    .action(ArgAction::SetTrue),
            )
            .arg(env_file_arg())
            .arg(executable_ext_arg())
    })
}

fn types_subcommand() -> Command {
    with_unstable_args(
        Command::new("types").about("Print runtime TypeScript declarations"),
    )
}

fn upgrade_subcommand() -> Command {
    with_unstable_args(
        Command::new("upgrade").about("Upgrade deno executable to the given version"),
    )
    .defer(|cmd| {
        cmd.arg(
            Arg::new("version")
                .long("version")
                .help("The version to upgrade to")
                .help_heading(UPGRADE_HEADING)
                .hide(true),
        )
        .arg(
            Arg::new("output")
                .long("output")
                .help("The path to output the updated version to")
                .value_parser(value_parser!(String))
                .value_hint(ValueHint::FilePath)
                .help_heading(UPGRADE_HEADING),
        )
        .arg(
            Arg::new("dry-run")
                .long("dry-run")
                .help("Perform all checks without replacing old exe")
                .action(ArgAction::SetTrue)
                .help_heading(UPGRADE_HEADING),
        )
        .arg(
            Arg::new("force")
                .long("force")
                .short('f')
                .help("Replace current exe even if not out-of-date")
                .action(ArgAction::SetTrue)
                .help_heading(UPGRADE_HEADING),
        )
        .arg(
            Arg::new("canary")
                .long("canary")
                .help("Upgrade to canary builds")
                .action(ArgAction::SetTrue)
                .help_heading(UPGRADE_HEADING)
                .hide(true),
        )
        .arg(
            Arg::new("release-candidate")
                .long("rc")
                .help("Upgrade to a release candidate")
                .conflicts_with_all(["canary", "version"])
                .action(ArgAction::SetTrue)
                .help_heading(UPGRADE_HEADING)
                .hide(true),
        )
        .arg(
            Arg::new("version-or-hash-or-channel")
                .help("Version, channel, commit hash, or pr 12345 to install from a PR")
                .value_name("VERSION")
                .action(ArgAction::Append)
                .trailing_var_arg(true),
        )
        .arg(
            Arg::new("checksum")
                .long("checksum")
                .help("Verify the downloaded archive against the provided SHA256 checksum")
                .value_parser(value_parser!(String))
                .help_heading(UPGRADE_HEADING),
        )
        .arg(ca_file_arg())
        .arg(unsafely_ignore_certificate_errors_arg())
    })
}

fn update_subcommand() -> Command {
    with_unstable_args(
        Command::new("update").about("Update outdated dependencies"),
    )
    .defer(|cmd| {
        cmd.args(update_and_outdated_args())
            .arg(
                Arg::new("interactive")
                    .long("interactive")
                    .short('i')
                    .action(ArgAction::SetTrue)
                    .help("Interactively select which dependencies to update"),
            )
            .args(lock_args())
    })
}

fn outdated_subcommand() -> Command {
    with_unstable_args(
        Command::new("outdated").about("Find and update outdated dependencies"),
    )
    .defer(|cmd| {
        cmd.args(update_and_outdated_args())
            .arg(
                Arg::new("interactive")
                    .long("interactive")
                    .short('i')
                    .requires("update")
                    .action(ArgAction::SetTrue)
                    .help("Interactively select which dependencies to update"),
            )
            .args(lock_args())
            .arg(
                Arg::new("update")
                    .long("update")
                    .short('u')
                    .action(ArgAction::SetTrue)
                    .help("Update dependency versions"),
            )
    })
}

fn vendor_subcommand() -> Command {
    with_unstable_args(
        Command::new("vendor").about("deno vendor was removed in Deno 2"),
    )
    .hide(true)
}

fn uninstall_subcommand() -> Command {
    with_unstable_args(
        Command::new("uninstall").about("Uninstalls a dependency or an executable script"),
    )
    .defer(|cmd| {
        cmd.arg(Arg::new("name-or-package").required_unless_present("help"))
            .arg(
                Arg::new("root")
                    .long("root")
                    .help("Installation root")
                    .requires("global")
                    .value_hint(ValueHint::DirPath),
            )
            .arg(
                Arg::new("global")
                    .long("global")
                    .short('g')
                    .help("Remove globally installed package or module")
                    .action(ArgAction::SetTrue),
            )
            .arg(
                Arg::new("additional-packages")
                    .help("List of additional packages to remove")
                    .conflicts_with("global")
                    .num_args(1..)
                    .action(ArgAction::Append),
            )
            .args(lock_args())
            .arg(lockfile_only_arg())
    })
}

fn publish_subcommand() -> Command {
    with_unstable_args(
        Command::new("publish").about("Publish the current working directory's package or workspace to JSR"),
    )
    .defer(|cmd| {
        cmd.arg(
            Arg::new("token")
                .long("token")
                .help("The API token to use when publishing")
                .help_heading(PUBLISH_HEADING),
        )
        .arg(config_arg())
        .arg(no_config_arg())
        .arg(
            Arg::new("dry-run")
                .long("dry-run")
                .help("Prepare the package for publishing performing all checks without uploading")
                .action(ArgAction::SetTrue)
                .help_heading(PUBLISH_HEADING),
        )
        .arg(
            Arg::new("allow-slow-types")
                .long("allow-slow-types")
                .help("Allow publishing with slow types")
                .action(ArgAction::SetTrue)
                .help_heading(PUBLISH_HEADING),
        )
        .arg(
            Arg::new("allow-dirty")
                .long("allow-dirty")
                .help("Allow publishing if the repository has uncommitted changes")
                .action(ArgAction::SetTrue)
                .help_heading(PUBLISH_HEADING),
        )
        .arg(
            Arg::new("no-provenance")
                .long("no-provenance")
                .help("Disable provenance attestation")
                .action(ArgAction::SetTrue)
                .help_heading(PUBLISH_HEADING),
        )
        .arg(
            Arg::new("set-version")
                .long("set-version")
                .help("Set version for a package to be published")
                .value_name("VERSION")
                .help_heading(PUBLISH_HEADING),
        )
        .arg(check_arg(true))
        .arg(no_check_arg())
    })
}

fn x_subcommand() -> Command {
    with_unstable_args(
        Command::new("x").about("Execute a binary from npm or jsr, like npx"),
    )
    .defer(|cmd| {
        runtime_args(cmd, true, true, true)
            .arg(script_arg().trailing_var_arg(true))
            .arg(
                Arg::new("yes")
                    .long("yes")
                    .short('y')
                    .help("Assume confirmation for all prompts")
                    .action(ArgAction::SetTrue)
                    .conflicts_with("install-alias"),
            )
            .arg(check_arg(false))
            .arg(env_file_arg())
            .arg(
                Arg::new("install-alias")
                    .long("install-alias")
                    .help("Creates a dx alias")
                    .num_args(0..=1)
                    .default_missing_value("dx")
                    .action(ArgAction::Set)
                    .conflicts_with("script_arg"),
            )
    })
}

// ============================================================
// Root command builder — the full Deno clap tree
// ============================================================

fn build_clap_root() -> Command {
    run_args(with_unstable_args(Command::new("deno")), true)
        .next_line_help(false)
        .bin_name("deno")
        .styles(
            clap::builder::Styles::styled()
                .header(AnsiColor::Yellow.on_default())
                .usage(AnsiColor::White.on_default())
                .literal(AnsiColor::Green.on_default())
                .placeholder(AnsiColor::Green.on_default()),
        )
        .color(ColorChoice::Auto)
        .term_width(800)
        .version("2.0.0")
        .long_version("2.0.0 (stable, release, x86_64-apple-darwin)\nv8 12.0.0\ntypescript 5.6.2")
        .disable_version_flag(true)
        .disable_help_flag(true)
        .disable_help_subcommand(true)
        .arg(
            Arg::new("help")
                .short('h')
                .long("help")
                .action(ArgAction::Append)
                .num_args(0..=1)
                .require_equals(true)
                .value_name("CONTEXT")
                .value_parser(["unstable", "full"])
                .global(true),
        )
        .arg(
            Arg::new("version")
                .short('V')
                .short_alias('v')
                .long("version")
                .action(ArgAction::Version)
                .help("Print version"),
        )
        .arg(
            Arg::new("log-level")
                .short('L')
                .long("log-level")
                .help("Set log level")
                .hide(true)
                .value_parser(["trace", "debug", "info"])
                .global(true),
        )
        .arg(
            Arg::new("quiet")
                .short('q')
                .long("quiet")
                .help("Suppress diagnostic output")
                .action(ArgAction::SetTrue)
                .global(true),
        )
        .subcommand(run_subcommand())
        .subcommand(serve_subcommand())
        .defer(|cmd| {
            cmd.subcommand(add_subcommand())
                .subcommand(audit_subcommand())
                .subcommand(remove_subcommand())
                .subcommand(bench_subcommand())
                .subcommand(bundle_subcommand())
                .subcommand(cache_subcommand())
                .subcommand(check_subcommand())
                .subcommand(clean_subcommand())
                .subcommand(compile_subcommand())
                .subcommand(create_subcommand())
                .subcommand(completions_subcommand())
                .subcommand(coverage_subcommand())
                .subcommand(doc_subcommand())
                .subcommand(deploy_subcommand())
                .subcommand(sandbox_subcommand())
                .subcommand(eval_subcommand())
                .subcommand(fmt_subcommand())
                .subcommand(init_subcommand())
                .subcommand(info_subcommand())
                .subcommand(install_subcommand())
                .subcommand(json_reference_subcommand())
                .subcommand(jupyter_subcommand())
                .subcommand(approve_scripts_subcommand())
                .subcommand(uninstall_subcommand())
                .subcommand(outdated_subcommand())
                .subcommand(lsp_subcommand())
                .subcommand(lint_subcommand())
                .subcommand(publish_subcommand())
                .subcommand(repl_subcommand())
                .subcommand(task_subcommand())
                .subcommand(test_subcommand())
                .subcommand(types_subcommand())
                .subcommand(update_subcommand())
                .subcommand(upgrade_subcommand())
                .subcommand(vendor_subcommand())
                .subcommand(x_subcommand())
        })
        .next_line_help(false)
}

// ============================================================
// Benchmarks
// ============================================================

fn bench_parse(c: &mut Criterion) {
    let mut group = c.benchmark_group("parse");

    // Benchmark 1: "deno run script.ts" -- most common case
    let args_run = vec!["deno", "run", "script.ts"];

    group.bench_function("clap_run", |b| {
        b.iter(|| {
            let app = build_clap_root();
            let _ = app.try_get_matches_from(black_box(args_run.clone()));
        })
    });

    group.bench_function("custom_run", |b| {
        let args: Vec<String> = args_run.iter().map(|s| s.to_string()).collect();
        b.iter(|| {
            let _ = deno_cli_parser::parse(
                &deno_cli_parser::defs::DENO_ROOT,
                black_box(&args),
            );
        })
    });

    // Benchmark 2: "deno run -A script.ts" -- with permission flag
    let args_run_a = vec!["deno", "run", "-A", "script.ts"];

    group.bench_function("clap_run_allow_all", |b| {
        b.iter(|| {
            let app = build_clap_root();
            let _ = app.try_get_matches_from(black_box(args_run_a.clone()));
        })
    });

    group.bench_function("custom_run_allow_all", |b| {
        let args: Vec<String> = args_run_a.iter().map(|s| s.to_string()).collect();
        b.iter(|| {
            let _ = deno_cli_parser::parse(
                &deno_cli_parser::defs::DENO_ROOT,
                black_box(&args),
            );
        })
    });

    // Benchmark 3: "deno fmt file.ts" -- non-run subcommand (deferred)
    let args_fmt = vec!["deno", "fmt", "file.ts"];

    group.bench_function("clap_fmt", |b| {
        b.iter(|| {
            let app = build_clap_root();
            let _ = app.try_get_matches_from(black_box(args_fmt.clone()));
        })
    });

    group.bench_function("custom_fmt", |b| {
        let args: Vec<String> = args_fmt.iter().map(|s| s.to_string()).collect();
        b.iter(|| {
            let _ = deno_cli_parser::parse(
                &deno_cli_parser::defs::DENO_ROOT,
                black_box(&args),
            );
        })
    });

    // Benchmark 4: complex run with multiple flags
    let args_complex = vec![
        "deno",
        "run",
        "--allow-read=/tmp",
        "--allow-net=localhost:8080",
        "--config",
        "deno.json",
        "--v8-flags=--max-old-space-size=4096",
        "script.ts",
        "arg1",
        "arg2",
    ];

    group.bench_function("clap_complex", |b| {
        b.iter(|| {
            let app = build_clap_root();
            let _ = app.try_get_matches_from(black_box(args_complex.clone()));
        })
    });

    group.bench_function("custom_complex", |b| {
        let args: Vec<String> = args_complex.iter().map(|s| s.to_string()).collect();
        b.iter(|| {
            let _ = deno_cli_parser::parse(
                &deno_cli_parser::defs::DENO_ROOT,
                black_box(&args),
            );
        })
    });

    // Benchmark 5: Full pipeline including Flags conversion
    group.bench_function("custom_full_run", |b| {
        let args: Vec<String> = args_run.iter().map(|s| s.to_string()).collect();
        b.iter(|| {
            let _ = deno_cli_parser::convert::flags_from_vec(black_box(args.clone()));
        })
    });

    group.bench_function("custom_full_complex", |b| {
        let args: Vec<String> = args_complex.iter().map(|s| s.to_string()).collect();
        b.iter(|| {
            let _ = deno_cli_parser::convert::flags_from_vec(black_box(args.clone()));
        })
    });

    group.finish();
}

criterion_group!(benches, bench_parse);
criterion_main!(benches);
