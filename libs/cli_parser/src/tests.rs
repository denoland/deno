use pretty_assertions::assert_eq;

use crate::*;

/// Helper: create Vec<String> from string literals.
macro_rules! svec {
    ($($x:expr),* $(,)?) => {
        vec![$($x.to_string()),*]
    };
}

// ---- Static command definitions for tests ----
// These mirror a simplified version of Deno's CLI structure.

const GLOBAL_ARGS: &[ArgDef] = &[
    ArgDef::new("help")
        .short('h')
        .long("help")
        .set_true()
        .global(),
    ArgDef::new("version")
        .short('V')
        .long("version")
        .set_true()
        .short_aliases(&['v']),
    ArgDef::new("log-level")
        .short('L')
        .long("log-level")
        .action(ArgAction::Set)
        .num_args(NumArgs::Exact(1))
        .global(),
    ArgDef::new("quiet")
        .short('q')
        .long("quiet")
        .set_true()
        .global(),
];

const PERMISSION_ARGS: &[ArgDef] = &[
    ArgDef::new("allow-all")
        .short('A')
        .long("allow-all")
        .set_true(),
    ArgDef::new("allow-read")
        .short('R')
        .long("allow-read")
        .action(ArgAction::Append)
        .num_args(NumArgs::ZeroOrMore)
        .require_equals()
        .value_delimiter(','),
    ArgDef::new("deny-read")
        .long("deny-read")
        .action(ArgAction::Append)
        .num_args(NumArgs::ZeroOrMore)
        .require_equals()
        .value_delimiter(','),
    ArgDef::new("allow-write")
        .short('W')
        .long("allow-write")
        .action(ArgAction::Append)
        .num_args(NumArgs::ZeroOrMore)
        .require_equals()
        .value_delimiter(','),
    ArgDef::new("deny-write")
        .long("deny-write")
        .action(ArgAction::Append)
        .num_args(NumArgs::ZeroOrMore)
        .require_equals()
        .value_delimiter(','),
    ArgDef::new("allow-net")
        .short('N')
        .long("allow-net")
        .action(ArgAction::Append)
        .num_args(NumArgs::ZeroOrMore)
        .require_equals()
        .value_delimiter(','),
    ArgDef::new("deny-net")
        .long("deny-net")
        .action(ArgAction::Append)
        .num_args(NumArgs::ZeroOrMore)
        .require_equals()
        .value_delimiter(','),
    ArgDef::new("allow-env")
        .short('E')
        .long("allow-env")
        .action(ArgAction::Append)
        .num_args(NumArgs::ZeroOrMore)
        .require_equals()
        .value_delimiter(','),
    ArgDef::new("deny-env")
        .long("deny-env")
        .action(ArgAction::Append)
        .num_args(NumArgs::ZeroOrMore)
        .require_equals()
        .value_delimiter(','),
    ArgDef::new("allow-run")
        .long("allow-run")
        .action(ArgAction::Append)
        .num_args(NumArgs::ZeroOrMore)
        .require_equals()
        .value_delimiter(','),
    ArgDef::new("deny-run")
        .long("deny-run")
        .action(ArgAction::Append)
        .num_args(NumArgs::ZeroOrMore)
        .require_equals()
        .value_delimiter(','),
    ArgDef::new("allow-sys")
        .short('S')
        .long("allow-sys")
        .action(ArgAction::Append)
        .num_args(NumArgs::ZeroOrMore)
        .require_equals()
        .value_delimiter(','),
    ArgDef::new("deny-sys")
        .long("deny-sys")
        .action(ArgAction::Append)
        .num_args(NumArgs::ZeroOrMore)
        .require_equals()
        .value_delimiter(','),
    ArgDef::new("allow-ffi")
        .long("allow-ffi")
        .action(ArgAction::Append)
        .num_args(NumArgs::ZeroOrMore)
        .require_equals()
        .value_delimiter(','),
    ArgDef::new("deny-ffi")
        .long("deny-ffi")
        .action(ArgAction::Append)
        .num_args(NumArgs::ZeroOrMore)
        .require_equals()
        .value_delimiter(','),
    ArgDef::new("allow-import")
        .short('I')
        .long("allow-import")
        .action(ArgAction::Append)
        .num_args(NumArgs::ZeroOrMore)
        .require_equals()
        .value_delimiter(','),
    ArgDef::new("deny-import")
        .long("deny-import")
        .action(ArgAction::Append)
        .num_args(NumArgs::ZeroOrMore)
        .require_equals()
        .value_delimiter(','),
    ArgDef::new("no-prompt")
        .long("no-prompt")
        .set_true(),
];

const COMPILE_ARGS: &[ArgDef] = &[
    ArgDef::new("no-check")
        .long("no-check")
        .action(ArgAction::Set)
        .num_args(NumArgs::Optional)
        .require_equals(),
    ArgDef::new("import-map")
        .long("import-map")
        .action(ArgAction::Set)
        .num_args(NumArgs::Exact(1))
        .value_name("FILE"),
    ArgDef::new("no-remote")
        .long("no-remote")
        .set_true(),
    ArgDef::new("no-npm")
        .long("no-npm")
        .set_true(),
    ArgDef::new("node-modules-dir")
        .long("node-modules-dir")
        .action(ArgAction::Set)
        .num_args(NumArgs::Optional)
        .require_equals(),
    ArgDef::new("config")
        .short('c')
        .long("config")
        .action(ArgAction::Set)
        .num_args(NumArgs::Exact(1))
        .value_name("FILE"),
    ArgDef::new("no-config")
        .long("no-config")
        .set_true(),
    ArgDef::new("reload")
        .short('r')
        .long("reload")
        .action(ArgAction::Append)
        .num_args(NumArgs::ZeroOrMore)
        .require_equals()
        .value_delimiter(','),
    ArgDef::new("lock")
        .long("lock")
        .action(ArgAction::Set)
        .num_args(NumArgs::Optional)
        .require_equals(),
    ArgDef::new("no-lock")
        .long("no-lock")
        .set_true(),
    ArgDef::new("cert")
        .long("cert")
        .action(ArgAction::Set)
        .num_args(NumArgs::Exact(1))
        .value_name("FILE"),
];

const RUN_ARGS: &[ArgDef] = &[
    ArgDef::new("script_arg")
        .positional()
        .action(ArgAction::Set)
        .num_args(NumArgs::Exact(1)),
    ArgDef::new("check")
        .long("check")
        .action(ArgAction::Set)
        .num_args(NumArgs::Optional)
        .require_equals(),
    ArgDef::new("watch")
        .long("watch")
        .action(ArgAction::Append)
        .num_args(NumArgs::ZeroOrMore)
        .require_equals()
        .value_delimiter(','),
    ArgDef::new("hmr")
        .long("hmr")
        .action(ArgAction::Append)
        .num_args(NumArgs::ZeroOrMore)
        .require_equals()
        .value_delimiter(','),
    ArgDef::new("watch-exclude")
        .long("watch-exclude")
        .action(ArgAction::Append)
        .num_args(NumArgs::ZeroOrMore)
        .require_equals()
        .value_delimiter(','),
    ArgDef::new("no-clear-screen")
        .long("no-clear-screen")
        .set_true(),
    ArgDef::new("ext")
        .long("ext")
        .action(ArgAction::Set)
        .num_args(NumArgs::Exact(1)),
    ArgDef::new("env-file")
        .long("env-file")
        .action(ArgAction::Append)
        .num_args(NumArgs::Optional)
        .require_equals(),
    ArgDef::new("no-code-cache")
        .long("no-code-cache")
        .set_true(),
    // inspect args
    ArgDef::new("inspect")
        .long("inspect")
        .action(ArgAction::Set)
        .num_args(NumArgs::Optional)
        .require_equals(),
    ArgDef::new("inspect-brk")
        .long("inspect-brk")
        .action(ArgAction::Set)
        .num_args(NumArgs::Optional)
        .require_equals(),
    ArgDef::new("inspect-wait")
        .long("inspect-wait")
        .action(ArgAction::Set)
        .num_args(NumArgs::Optional)
        .require_equals(),
    // runtime misc
    ArgDef::new("cached-only")
        .long("cached-only")
        .set_true(),
    ArgDef::new("location")
        .long("location")
        .action(ArgAction::Set)
        .num_args(NumArgs::Exact(1)),
    ArgDef::new("v8-flags")
        .long("v8-flags")
        .action(ArgAction::Append)
        .num_args(NumArgs::ZeroOrMore)
        .require_equals()
        .value_delimiter(','),
    ArgDef::new("seed")
        .long("seed")
        .action(ArgAction::Set)
        .num_args(NumArgs::Exact(1)),
    ArgDef::new("enable-testing-features")
        .long("enable-testing-features")
        .set_true()
        .hidden(),
    ArgDef::new("coverage")
        .long("coverage")
        .action(ArgAction::Set)
        .num_args(NumArgs::Optional)
        .require_equals(),
];

const EVAL_ARGS: &[ArgDef] = &[
    ArgDef::new("code_arg")
        .positional()
        .action(ArgAction::Set)
        .num_args(NumArgs::Exact(1)),
    ArgDef::new("print")
        .short('p')
        .long("print")
        .set_true(),
    ArgDef::new("ext")
        .long("ext")
        .action(ArgAction::Set)
        .num_args(NumArgs::Exact(1)),
    // inspect args
    ArgDef::new("inspect")
        .long("inspect")
        .action(ArgAction::Set)
        .num_args(NumArgs::Optional)
        .require_equals(),
    ArgDef::new("inspect-brk")
        .long("inspect-brk")
        .action(ArgAction::Set)
        .num_args(NumArgs::Optional)
        .require_equals(),
    ArgDef::new("inspect-wait")
        .long("inspect-wait")
        .action(ArgAction::Set)
        .num_args(NumArgs::Optional)
        .require_equals(),
    ArgDef::new("v8-flags")
        .long("v8-flags")
        .action(ArgAction::Append)
        .num_args(NumArgs::ZeroOrMore)
        .require_equals()
        .value_delimiter(','),
    ArgDef::new("cached-only")
        .long("cached-only")
        .set_true(),
    ArgDef::new("location")
        .long("location")
        .action(ArgAction::Set)
        .num_args(NumArgs::Exact(1)),
    ArgDef::new("seed")
        .long("seed")
        .action(ArgAction::Set)
        .num_args(NumArgs::Exact(1)),
];

const FMT_ARGS: &[ArgDef] = &[
    ArgDef::new("files")
        .positional()
        .action(ArgAction::Append)
        .num_args(NumArgs::ZeroOrMore),
    ArgDef::new("check")
        .long("check")
        .set_true(),
    ArgDef::new("watch")
        .long("watch")
        .action(ArgAction::Append)
        .num_args(NumArgs::ZeroOrMore)
        .require_equals()
        .value_delimiter(','),
    ArgDef::new("watch-exclude")
        .long("watch-exclude")
        .action(ArgAction::Append)
        .num_args(NumArgs::ZeroOrMore)
        .require_equals()
        .value_delimiter(','),
    ArgDef::new("no-clear-screen")
        .long("no-clear-screen")
        .set_true(),
    ArgDef::new("ext")
        .long("ext")
        .action(ArgAction::Set)
        .num_args(NumArgs::Exact(1)),
    ArgDef::new("ignore")
        .long("ignore")
        .action(ArgAction::Append)
        .num_args(NumArgs::ZeroOrMore)
        .require_equals()
        .value_delimiter(','),
    ArgDef::new("use-tabs")
        .long("use-tabs")
        .action(ArgAction::Set)
        .num_args(NumArgs::Optional)
        .require_equals(),
    ArgDef::new("line-width")
        .long("line-width")
        .action(ArgAction::Set)
        .num_args(NumArgs::Exact(1)),
    ArgDef::new("indent-width")
        .long("indent-width")
        .action(ArgAction::Set)
        .num_args(NumArgs::Exact(1)),
    ArgDef::new("single-quote")
        .long("single-quote")
        .action(ArgAction::Set)
        .num_args(NumArgs::Optional)
        .require_equals(),
    ArgDef::new("prose-wrap")
        .long("prose-wrap")
        .action(ArgAction::Set)
        .num_args(NumArgs::Exact(1)),
    ArgDef::new("no-semicolons")
        .long("no-semicolons")
        .action(ArgAction::Set)
        .num_args(NumArgs::Optional)
        .require_equals(),
    ArgDef::new("config")
        .short('c')
        .long("config")
        .action(ArgAction::Set)
        .num_args(NumArgs::Exact(1)),
    ArgDef::new("no-config")
        .long("no-config")
        .set_true(),
];

const TEST_ARGS: &[ArgDef] = &[
    ArgDef::new("files")
        .positional()
        .action(ArgAction::Append)
        .num_args(NumArgs::ZeroOrMore),
    ArgDef::new("doc")
        .long("doc")
        .set_true(),
    ArgDef::new("no-run")
        .long("no-run")
        .set_true(),
    ArgDef::new("coverage")
        .long("coverage")
        .action(ArgAction::Set)
        .num_args(NumArgs::Optional)
        .require_equals(),
    ArgDef::new("clean")
        .long("clean")
        .set_true(),
    ArgDef::new("fail-fast")
        .long("fail-fast")
        .action(ArgAction::Set)
        .num_args(NumArgs::Optional)
        .require_equals(),
    ArgDef::new("filter")
        .long("filter")
        .action(ArgAction::Set)
        .num_args(NumArgs::Exact(1)),
    ArgDef::new("shuffle")
        .long("shuffle")
        .action(ArgAction::Set)
        .num_args(NumArgs::Optional)
        .require_equals(),
    ArgDef::new("parallel")
        .long("parallel")
        .set_true(),
    ArgDef::new("trace-leaks")
        .long("trace-leaks")
        .set_true(),
    ArgDef::new("watch")
        .long("watch")
        .action(ArgAction::Append)
        .num_args(NumArgs::ZeroOrMore)
        .require_equals()
        .value_delimiter(','),
    ArgDef::new("watch-exclude")
        .long("watch-exclude")
        .action(ArgAction::Append)
        .num_args(NumArgs::ZeroOrMore)
        .require_equals()
        .value_delimiter(','),
    ArgDef::new("no-clear-screen")
        .long("no-clear-screen")
        .set_true(),
    ArgDef::new("reporter")
        .long("reporter")
        .action(ArgAction::Set)
        .num_args(NumArgs::Exact(1)),
    ArgDef::new("junit-path")
        .long("junit-path")
        .action(ArgAction::Set)
        .num_args(NumArgs::Exact(1)),
    ArgDef::new("hide-stacktraces")
        .long("hide-stacktraces")
        .set_true(),
    ArgDef::new("ignore")
        .long("ignore")
        .action(ArgAction::Append)
        .num_args(NumArgs::ZeroOrMore)
        .require_equals()
        .value_delimiter(','),
    ArgDef::new("enable-testing-features")
        .long("enable-testing-features")
        .set_true()
        .hidden(),
    // compile/runtime args needed by test
    ArgDef::new("no-check")
        .long("no-check")
        .action(ArgAction::Set)
        .num_args(NumArgs::Optional)
        .require_equals(),
    ArgDef::new("config")
        .short('c')
        .long("config")
        .action(ArgAction::Set)
        .num_args(NumArgs::Exact(1)),
    ArgDef::new("no-config")
        .long("no-config")
        .set_true(),
    ArgDef::new("reload")
        .short('r')
        .long("reload")
        .action(ArgAction::Append)
        .num_args(NumArgs::ZeroOrMore)
        .require_equals()
        .value_delimiter(','),
    ArgDef::new("seed")
        .long("seed")
        .action(ArgAction::Set)
        .num_args(NumArgs::Exact(1)),
    ArgDef::new("v8-flags")
        .long("v8-flags")
        .action(ArgAction::Append)
        .num_args(NumArgs::ZeroOrMore)
        .require_equals()
        .value_delimiter(','),
    ArgDef::new("inspect")
        .long("inspect")
        .action(ArgAction::Set)
        .num_args(NumArgs::Optional)
        .require_equals(),
    ArgDef::new("inspect-brk")
        .long("inspect-brk")
        .action(ArgAction::Set)
        .num_args(NumArgs::Optional)
        .require_equals(),
];

const UPGRADE_ARGS: &[ArgDef] = &[
    ArgDef::new("dry-run")
        .long("dry-run")
        .set_true(),
    ArgDef::new("force")
        .short('f')
        .long("force")
        .set_true(),
    ArgDef::new("canary")
        .long("canary")
        .set_true(),
    ArgDef::new("release-candidate")
        .long("release-candidate")
        .long_aliases(&["rc"])
        .set_true(),
    ArgDef::new("version")
        .long("version")
        .action(ArgAction::Set)
        .num_args(NumArgs::Exact(1)),
    ArgDef::new("output")
        .short('o')
        .long("output")
        .action(ArgAction::Set)
        .num_args(NumArgs::Exact(1)),
    ArgDef::new("version-or-hash-or-channel")
        .positional()
        .action(ArgAction::Set)
        .num_args(NumArgs::Optional),
];

const LINT_ARGS: &[ArgDef] = &[
    ArgDef::new("files")
        .positional()
        .action(ArgAction::Append)
        .num_args(NumArgs::ZeroOrMore),
    ArgDef::new("rules")
        .long("rules")
        .set_true(),
    ArgDef::new("fix")
        .long("fix")
        .set_true(),
    ArgDef::new("rules-tags")
        .long("rules-tags")
        .action(ArgAction::Append)
        .num_args(NumArgs::ZeroOrMore)
        .require_equals()
        .value_delimiter(','),
    ArgDef::new("rules-include")
        .long("rules-include")
        .action(ArgAction::Append)
        .num_args(NumArgs::ZeroOrMore)
        .require_equals()
        .value_delimiter(','),
    ArgDef::new("rules-exclude")
        .long("rules-exclude")
        .action(ArgAction::Append)
        .num_args(NumArgs::ZeroOrMore)
        .require_equals()
        .value_delimiter(','),
    ArgDef::new("json")
        .long("json")
        .set_true(),
    ArgDef::new("compact")
        .long("compact")
        .set_true(),
    ArgDef::new("ignore")
        .long("ignore")
        .action(ArgAction::Append)
        .num_args(NumArgs::ZeroOrMore)
        .require_equals()
        .value_delimiter(','),
    ArgDef::new("watch")
        .long("watch")
        .action(ArgAction::Append)
        .num_args(NumArgs::ZeroOrMore)
        .require_equals()
        .value_delimiter(','),
    ArgDef::new("watch-exclude")
        .long("watch-exclude")
        .action(ArgAction::Append)
        .num_args(NumArgs::ZeroOrMore)
        .require_equals()
        .value_delimiter(','),
    ArgDef::new("no-clear-screen")
        .long("no-clear-screen")
        .set_true(),
    ArgDef::new("config")
        .short('c')
        .long("config")
        .action(ArgAction::Set)
        .num_args(NumArgs::Exact(1)),
    ArgDef::new("no-config")
        .long("no-config")
        .set_true(),
];

const DEPLOY_ARGS: &[ArgDef] = &[];

const SERVE_ARGS: &[ArgDef] = &[
    ArgDef::new("script_arg")
        .positional()
        .action(ArgAction::Set)
        .num_args(NumArgs::Exact(1)),
    ArgDef::new("port")
        .long("port")
        .action(ArgAction::Set)
        .num_args(NumArgs::Exact(1))
        .default_value("8000"),
    ArgDef::new("host")
        .long("host")
        .action(ArgAction::Set)
        .num_args(NumArgs::Exact(1))
        .default_value("0.0.0.0"),
    ArgDef::new("parallel")
        .long("parallel")
        .set_true(),
    ArgDef::new("watch")
        .long("watch")
        .action(ArgAction::Append)
        .num_args(NumArgs::ZeroOrMore)
        .require_equals()
        .value_delimiter(','),
    ArgDef::new("hmr")
        .long("hmr")
        .action(ArgAction::Append)
        .num_args(NumArgs::ZeroOrMore)
        .require_equals()
        .value_delimiter(','),
    ArgDef::new("watch-exclude")
        .long("watch-exclude")
        .action(ArgAction::Append)
        .num_args(NumArgs::ZeroOrMore)
        .require_equals()
        .value_delimiter(','),
    ArgDef::new("no-clear-screen")
        .long("no-clear-screen")
        .set_true(),
    ArgDef::new("check")
        .long("check")
        .action(ArgAction::Set)
        .num_args(NumArgs::Optional)
        .require_equals(),
    ArgDef::new("inspect")
        .long("inspect")
        .action(ArgAction::Set)
        .num_args(NumArgs::Optional)
        .require_equals(),
    ArgDef::new("inspect-brk")
        .long("inspect-brk")
        .action(ArgAction::Set)
        .num_args(NumArgs::Optional)
        .require_equals(),
    ArgDef::new("inspect-wait")
        .long("inspect-wait")
        .action(ArgAction::Set)
        .num_args(NumArgs::Optional)
        .require_equals(),
    // compile args
    ArgDef::new("no-check")
        .long("no-check")
        .action(ArgAction::Set)
        .num_args(NumArgs::Optional)
        .require_equals(),
    ArgDef::new("config")
        .short('c')
        .long("config")
        .action(ArgAction::Set)
        .num_args(NumArgs::Exact(1)),
    ArgDef::new("no-config")
        .long("no-config")
        .set_true(),
    ArgDef::new("reload")
        .short('r')
        .long("reload")
        .action(ArgAction::Append)
        .num_args(NumArgs::ZeroOrMore)
        .require_equals()
        .value_delimiter(','),
    ArgDef::new("lock")
        .long("lock")
        .action(ArgAction::Set)
        .num_args(NumArgs::Optional)
        .require_equals(),
    ArgDef::new("no-lock")
        .long("no-lock")
        .set_true(),
    ArgDef::new("cert")
        .long("cert")
        .action(ArgAction::Set)
        .num_args(NumArgs::Exact(1)),
    ArgDef::new("env-file")
        .long("env-file")
        .action(ArgAction::Append)
        .num_args(NumArgs::Optional)
        .require_equals(),
    ArgDef::new("no-code-cache")
        .long("no-code-cache")
        .set_true(),
    // runtime misc
    ArgDef::new("cached-only")
        .long("cached-only")
        .set_true(),
    ArgDef::new("location")
        .long("location")
        .action(ArgAction::Set)
        .num_args(NumArgs::Exact(1)),
    ArgDef::new("v8-flags")
        .long("v8-flags")
        .action(ArgAction::Append)
        .num_args(NumArgs::ZeroOrMore)
        .require_equals()
        .value_delimiter(','),
    ArgDef::new("seed")
        .long("seed")
        .action(ArgAction::Set)
        .num_args(NumArgs::Exact(1)),
];

static TEST_ROOT: CommandDef = CommandDef {
    name: "deno",
    about: "A modern JavaScript and TypeScript runtime",
    aliases: &[],
    args: GLOBAL_ARGS,
    arg_groups: &[],
    subcommands: &[
        CommandDef {
            name: "run",
            about: "Run a JavaScript or TypeScript program",
            aliases: &[],
            args: RUN_ARGS,
            arg_groups: &[PERMISSION_ARGS, COMPILE_ARGS],
            subcommands: &[],
            default_subcommand: None,
            trailing_var_arg: true,
            passthrough: false,
        },
        CommandDef {
            name: "serve",
            about: "Run a server",
            aliases: &[],
            args: SERVE_ARGS,
            arg_groups: &[PERMISSION_ARGS],
            subcommands: &[],
            default_subcommand: None,
            trailing_var_arg: true,
            passthrough: false,
        },
        CommandDef {
            name: "eval",
            about: "Evaluate a script",
            aliases: &[],
            args: EVAL_ARGS,
            arg_groups: &[PERMISSION_ARGS, COMPILE_ARGS],
            subcommands: &[],
            default_subcommand: None,
            trailing_var_arg: true,
            passthrough: false,
        },
        CommandDef {
            name: "fmt",
            about: "Format source files",
            aliases: &[],
            args: FMT_ARGS,
            arg_groups: &[],
            subcommands: &[],
            default_subcommand: None,
            trailing_var_arg: false,
            passthrough: false,
        },
        CommandDef {
            name: "lint",
            about: "Lint source files",
            aliases: &[],
            args: LINT_ARGS,
            arg_groups: &[],
            subcommands: &[],
            default_subcommand: None,
            trailing_var_arg: false,
            passthrough: false,
        },
        CommandDef {
            name: "test",
            about: "Run tests",
            aliases: &[],
            args: TEST_ARGS,
            arg_groups: &[PERMISSION_ARGS],
            subcommands: &[],
            default_subcommand: None,
            trailing_var_arg: true,
            passthrough: false,
        },
        CommandDef {
            name: "upgrade",
            about: "Upgrade deno executable",
            aliases: &[],
            args: UPGRADE_ARGS,
            arg_groups: &[],
            subcommands: &[],
            default_subcommand: None,
            trailing_var_arg: false,
            passthrough: false,
        },
        CommandDef {
            name: "deploy",
            about: "Deploy to Deno Deploy",
            aliases: &[],
            args: DEPLOY_ARGS,
            arg_groups: &[],
            subcommands: &[],
            default_subcommand: None,
            trailing_var_arg: false,
            passthrough: true,
        },
    ],
    default_subcommand: Some("run"),
    trailing_var_arg: false,
    passthrough: false,
};

// ---- Tests ----

#[test]
fn basic_run() {
    let r = parse(&TEST_ROOT, &svec!["deno", "run", "script.ts"]).unwrap();
    assert_eq!(r.subcommand.as_deref(), Some("run"));
    assert_eq!(r.get_one("script_arg"), Some("script.ts"));
    assert!(!r.get_bool("allow-all"));
}

#[test]
fn run_with_allow_all() {
    let r = parse(&TEST_ROOT, &svec!["deno", "run", "-A", "script.ts"]).unwrap();
    assert_eq!(r.subcommand.as_deref(), Some("run"));
    assert!(r.get_bool("allow-all"));
    assert_eq!(r.get_one("script_arg"), Some("script.ts"));
}

#[test]
fn run_with_permission_values() {
    let r = parse(
        &TEST_ROOT,
        &svec!["deno", "run", "--allow-read=/etc,/var", "--allow-net=localhost:8080", "script.ts"],
    )
    .unwrap();
    assert_eq!(r.subcommand.as_deref(), Some("run"));
    assert_eq!(
        r.get_many("allow-read"),
        Some(vec!["/etc".to_string(), "/var".to_string()].as_slice())
    );
    assert_eq!(
        r.get_many("allow-net"),
        Some(vec!["localhost:8080".to_string()].as_slice())
    );
}

#[test]
fn run_with_bare_permission_flags() {
    // --allow-read without =value means "allow all reads"
    let r = parse(
        &TEST_ROOT,
        &svec!["deno", "run", "--allow-read", "--allow-write", "script.ts"],
    )
    .unwrap();
    assert!(r.get_bool("allow-read"));
    assert!(r.get_bool("allow-write"));
    // No values means unrestricted
    assert!(r.get_many("allow-read").unwrap().is_empty());
    assert_eq!(r.get_one("script_arg"), Some("script.ts"));
}

#[test]
fn run_trailing_args() {
    let r = parse(
        &TEST_ROOT,
        &svec!["deno", "run", "script.ts", "arg1", "--flag", "arg2"],
    )
    .unwrap();
    assert_eq!(r.get_one("script_arg"), Some("script.ts"));
    assert_eq!(r.trailing, vec!["arg1", "--flag", "arg2"]);
}

#[test]
fn run_double_dash_trailing() {
    let r = parse(
        &TEST_ROOT,
        &svec!["deno", "run", "script.ts", "--", "arg1", "--flag"],
    )
    .unwrap();
    assert_eq!(r.get_one("script_arg"), Some("script.ts"));
    // After script_arg is consumed, rest is trailing (including --)
    assert_eq!(r.trailing, vec!["--", "arg1", "--flag"]);
}

#[test]
fn global_flags_before_subcommand() {
    let r = parse(
        &TEST_ROOT,
        &svec!["deno", "--log-level", "debug", "--quiet", "run", "script.ts"],
    )
    .unwrap();
    assert_eq!(r.subcommand.as_deref(), Some("run"));
    assert_eq!(r.get_one("log-level"), Some("debug"));
    assert!(r.get_bool("quiet"));
    assert_eq!(r.get_one("script_arg"), Some("script.ts"));
}

#[test]
fn global_flags_after_subcommand() {
    let r = parse(
        &TEST_ROOT,
        &svec!["deno", "run", "--log-level", "debug", "--quiet", "script.ts"],
    )
    .unwrap();
    assert_eq!(r.subcommand.as_deref(), Some("run"));
    assert_eq!(r.get_one("log-level"), Some("debug"));
    assert!(r.get_bool("quiet"));
}

#[test]
fn upgrade_subcommand() {
    let r = parse(
        &TEST_ROOT,
        &svec!["deno", "upgrade", "--dry-run", "--force"],
    )
    .unwrap();
    assert_eq!(r.subcommand.as_deref(), Some("upgrade"));
    assert!(r.get_bool("dry-run"));
    assert!(r.get_bool("force"));
}

#[test]
fn upgrade_with_positional() {
    let r = parse(
        &TEST_ROOT,
        &svec!["deno", "upgrade", "canary"],
    )
    .unwrap();
    assert_eq!(r.subcommand.as_deref(), Some("upgrade"));
    assert_eq!(r.get_one("version-or-hash-or-channel"), Some("canary"));
}

#[test]
fn run_reload() {
    let r = parse(
        &TEST_ROOT,
        &svec!["deno", "run", "--reload", "script.ts"],
    )
    .unwrap();
    assert!(r.get_bool("reload"));
    assert_eq!(r.get_one("script_arg"), Some("script.ts"));
}

#[test]
fn run_reload_with_values() {
    let r = parse(
        &TEST_ROOT,
        &svec!["deno", "run", "--reload=http://example.com,http://foo.com", "script.ts"],
    )
    .unwrap();
    assert!(r.get_bool("reload"));
    assert_eq!(
        r.get_many("reload"),
        Some(vec!["http://example.com".to_string(), "http://foo.com".to_string()].as_slice())
    );
}

#[test]
fn run_watch() {
    let r = parse(
        &TEST_ROOT,
        &svec!["deno", "run", "--watch", "script.ts"],
    )
    .unwrap();
    assert!(r.get_bool("watch"));
    assert_eq!(r.get_one("script_arg"), Some("script.ts"));
}

#[test]
fn run_watch_with_paths() {
    let r = parse(
        &TEST_ROOT,
        &svec!["deno", "run", "--watch=src/,lib/", "script.ts"],
    )
    .unwrap();
    assert!(r.get_bool("watch"));
    assert_eq!(
        r.get_many("watch"),
        Some(vec!["src/".to_string(), "lib/".to_string()].as_slice())
    );
}

#[test]
fn run_v8_flags() {
    let r = parse(
        &TEST_ROOT,
        &svec!["deno", "run", "--v8-flags=--max-old-space-size=4096,--expose-gc", "script.ts"],
    )
    .unwrap();
    assert_eq!(
        r.get_many("v8-flags"),
        Some(vec!["--max-old-space-size=4096".to_string(), "--expose-gc".to_string()].as_slice())
    );
}

#[test]
fn run_seed() {
    let r = parse(
        &TEST_ROOT,
        &svec!["deno", "run", "--seed", "42", "script.ts"],
    )
    .unwrap();
    assert_eq!(r.get_one("seed"), Some("42"));
}

#[test]
fn run_config() {
    let r = parse(
        &TEST_ROOT,
        &svec!["deno", "run", "--config", "deno.json", "script.ts"],
    )
    .unwrap();
    assert_eq!(r.get_one("config"), Some("deno.json"));
}

#[test]
fn run_config_short() {
    let r = parse(
        &TEST_ROOT,
        &svec!["deno", "run", "-c", "deno.json", "script.ts"],
    )
    .unwrap();
    assert_eq!(r.get_one("config"), Some("deno.json"));
}

#[test]
fn fmt_basic() {
    let r = parse(
        &TEST_ROOT,
        &svec!["deno", "fmt", "file1.ts", "file2.ts"],
    )
    .unwrap();
    assert_eq!(r.subcommand.as_deref(), Some("fmt"));
    assert_eq!(
        r.get_many("files"),
        Some(vec!["file1.ts".to_string(), "file2.ts".to_string()].as_slice())
    );
}

#[test]
fn fmt_check() {
    let r = parse(
        &TEST_ROOT,
        &svec!["deno", "fmt", "--check", "file.ts"],
    )
    .unwrap();
    assert!(r.get_bool("check"));
}

#[test]
fn fmt_options() {
    let r = parse(
        &TEST_ROOT,
        &svec![
            "deno", "fmt",
            "--use-tabs",
            "--line-width", "120",
            "--indent-width", "4",
            "--single-quote",
            "--prose-wrap", "always",
            "--no-semicolons",
        ],
    )
    .unwrap();
    assert!(r.get_bool("use-tabs"));
    assert_eq!(r.get_one("line-width"), Some("120"));
    assert_eq!(r.get_one("indent-width"), Some("4"));
    assert!(r.get_bool("single-quote"));
    assert_eq!(r.get_one("prose-wrap"), Some("always"));
    assert!(r.get_bool("no-semicolons"));
}

#[test]
fn lint_basic() {
    let r = parse(
        &TEST_ROOT,
        &svec!["deno", "lint", "file.ts"],
    )
    .unwrap();
    assert_eq!(r.subcommand.as_deref(), Some("lint"));
    assert_eq!(
        r.get_many("files"),
        Some(vec!["file.ts".to_string()].as_slice())
    );
}

#[test]
fn lint_rules_include() {
    let r = parse(
        &TEST_ROOT,
        &svec!["deno", "lint", "--rules-include=no-var,no-eval"],
    )
    .unwrap();
    assert_eq!(
        r.get_many("rules-include"),
        Some(vec!["no-var".to_string(), "no-eval".to_string()].as_slice())
    );
}

#[test]
fn test_basic() {
    let r = parse(
        &TEST_ROOT,
        &svec!["deno", "test"],
    )
    .unwrap();
    assert_eq!(r.subcommand.as_deref(), Some("test"));
}

#[test]
fn test_with_filter() {
    let r = parse(
        &TEST_ROOT,
        &svec!["deno", "test", "--filter", "my_test"],
    )
    .unwrap();
    assert_eq!(r.get_one("filter"), Some("my_test"));
}

#[test]
fn test_fail_fast_no_value() {
    let r = parse(
        &TEST_ROOT,
        &svec!["deno", "test", "--fail-fast"],
    )
    .unwrap();
    assert!(r.get_bool("fail-fast"));
    assert_eq!(r.get_one("fail-fast"), None);
}

#[test]
fn test_fail_fast_with_value() {
    let r = parse(
        &TEST_ROOT,
        &svec!["deno", "test", "--fail-fast=3"],
    )
    .unwrap();
    assert!(r.get_bool("fail-fast"));
    assert_eq!(r.get_one("fail-fast"), Some("3"));
}

#[test]
fn test_shuffle_no_value() {
    let r = parse(
        &TEST_ROOT,
        &svec!["deno", "test", "--shuffle"],
    )
    .unwrap();
    assert!(r.get_bool("shuffle"));
}

#[test]
fn test_shuffle_with_value() {
    let r = parse(
        &TEST_ROOT,
        &svec!["deno", "test", "--shuffle=42"],
    )
    .unwrap();
    assert_eq!(r.get_one("shuffle"), Some("42"));
}

#[test]
fn test_reporter() {
    let r = parse(
        &TEST_ROOT,
        &svec!["deno", "test", "--reporter", "dot"],
    )
    .unwrap();
    assert_eq!(r.get_one("reporter"), Some("dot"));
}

#[test]
fn test_watch() {
    let r = parse(
        &TEST_ROOT,
        &svec!["deno", "test", "--watch"],
    )
    .unwrap();
    assert!(r.get_bool("watch"));
}

#[test]
fn test_watch_with_no_clear_screen() {
    let r = parse(
        &TEST_ROOT,
        &svec!["deno", "test", "--watch", "--no-clear-screen"],
    )
    .unwrap();
    assert!(r.get_bool("watch"));
    assert!(r.get_bool("no-clear-screen"));
}

#[test]
fn eval_basic() {
    let r = parse(
        &TEST_ROOT,
        &svec!["deno", "eval", "console.log('hello')"],
    )
    .unwrap();
    assert_eq!(r.subcommand.as_deref(), Some("eval"));
    assert_eq!(r.get_one("code_arg"), Some("console.log('hello')"));
}

#[test]
fn eval_print() {
    let r = parse(
        &TEST_ROOT,
        &svec!["deno", "eval", "-p", "1+1"],
    )
    .unwrap();
    assert!(r.get_bool("print"));
    assert_eq!(r.get_one("code_arg"), Some("1+1"));
}

#[test]
fn unknown_flag_error() {
    let err = parse(
        &TEST_ROOT,
        &svec!["deno", "run", "--unknown-flag", "script.ts"],
    )
    .unwrap_err();
    assert_eq!(err.kind, CliErrorKind::UnknownFlag);
    assert!(err.message.contains("--unknown-flag"));
}

#[test]
fn unknown_flag_suggestion() {
    let err = parse(
        &TEST_ROOT,
        &svec!["deno", "run", "--alow-read", "script.ts"],
    )
    .unwrap_err();
    assert_eq!(err.kind, CliErrorKind::UnknownFlag);
    assert!(err.suggestion.is_some());
    assert!(err.suggestion.unwrap().contains("--allow-read"));
}

#[test]
fn missing_value_error() {
    let err = parse(
        &TEST_ROOT,
        &svec!["deno", "run", "--seed"],
    )
    .unwrap_err();
    assert_eq!(err.kind, CliErrorKind::MissingValue);
}

#[test]
fn default_subcommand_run() {
    // `deno script.ts` parses with default subcommand args but subcommand is None
    // (the convert layer handles this as bare run)
    let r = parse(
        &TEST_ROOT,
        &svec!["deno", "script.ts"],
    )
    .unwrap();
    assert_eq!(r.subcommand.as_deref(), None);
    assert_eq!(r.get_one("script_arg"), Some("script.ts"));
}

#[test]
fn default_subcommand_with_flags() {
    // `deno -A script.ts` parses with default subcommand args but subcommand is None
    let r = parse(
        &TEST_ROOT,
        &svec!["deno", "-A", "script.ts"],
    )
    .unwrap();
    assert_eq!(r.subcommand.as_deref(), None);
    assert!(r.get_bool("allow-all"));
    assert_eq!(r.get_one("script_arg"), Some("script.ts"));
}

#[test]
fn default_subcommand_with_allow_read_values() {
    let r = parse(
        &TEST_ROOT,
        &svec!["deno", "--allow-read=/tmp", "script.ts"],
    )
    .unwrap();
    assert_eq!(r.subcommand.as_deref(), None);
    assert_eq!(
        r.get_many("allow-read"),
        Some(vec!["/tmp".to_string()].as_slice())
    );
}

#[test]
fn passthrough_subcommand() {
    let r = parse(
        &TEST_ROOT,
        &svec!["deno", "deploy", "--project=myapp", "--prod"],
    )
    .unwrap();
    assert_eq!(r.subcommand.as_deref(), Some("deploy"));
    assert_eq!(r.trailing, vec!["--project=myapp", "--prod"]);
}

#[test]
fn combined_short_flags() {
    let r = parse(
        &TEST_ROOT,
        &svec!["deno", "run", "-RWNE", "script.ts"],
    )
    .unwrap();
    assert!(r.get_bool("allow-read"));
    assert!(r.get_bool("allow-write"));
    assert!(r.get_bool("allow-net"));
    assert!(r.get_bool("allow-env"));
}

#[test]
fn upgrade_alias() {
    let r = parse(
        &TEST_ROOT,
        &svec!["deno", "upgrade", "--rc"],
    )
    .unwrap();
    assert!(r.get_bool("release-candidate"));
}

#[test]
fn serve_basic() {
    let r = parse(
        &TEST_ROOT,
        &svec!["deno", "serve", "--port", "3000", "server.ts"],
    )
    .unwrap();
    assert_eq!(r.subcommand.as_deref(), Some("serve"));
    assert_eq!(r.get_one("port"), Some("3000"));
    assert_eq!(r.get_one("script_arg"), Some("server.ts"));
}

#[test]
fn serve_with_permissions() {
    let r = parse(
        &TEST_ROOT,
        &svec!["deno", "serve", "-A", "--port", "8080", "server.ts"],
    )
    .unwrap();
    assert!(r.get_bool("allow-all"));
    assert_eq!(r.get_one("port"), Some("8080"));
}

#[test]
fn run_inspect() {
    let r = parse(
        &TEST_ROOT,
        &svec!["deno", "run", "--inspect", "script.ts"],
    )
    .unwrap();
    assert!(r.get_bool("inspect"));
}

#[test]
fn run_inspect_with_port() {
    let r = parse(
        &TEST_ROOT,
        &svec!["deno", "run", "--inspect=127.0.0.1:9229", "script.ts"],
    )
    .unwrap();
    assert_eq!(r.get_one("inspect"), Some("127.0.0.1:9229"));
}

#[test]
fn run_inspect_brk() {
    let r = parse(
        &TEST_ROOT,
        &svec!["deno", "run", "--inspect-brk", "script.ts"],
    )
    .unwrap();
    assert!(r.get_bool("inspect-brk"));
}

#[test]
fn no_check() {
    let r = parse(
        &TEST_ROOT,
        &svec!["deno", "run", "--no-check", "script.ts"],
    )
    .unwrap();
    assert!(r.get_bool("no-check"));
}

#[test]
fn no_check_remote() {
    let r = parse(
        &TEST_ROOT,
        &svec!["deno", "run", "--no-check=remote", "script.ts"],
    )
    .unwrap();
    assert_eq!(r.get_one("no-check"), Some("remote"));
}

#[test]
fn run_import_map() {
    let r = parse(
        &TEST_ROOT,
        &svec!["deno", "run", "--import-map", "import_map.json", "script.ts"],
    )
    .unwrap();
    assert_eq!(r.get_one("import-map"), Some("import_map.json"));
}

#[test]
fn run_no_remote() {
    let r = parse(
        &TEST_ROOT,
        &svec!["deno", "run", "--no-remote", "script.ts"],
    )
    .unwrap();
    assert!(r.get_bool("no-remote"));
}

#[test]
fn run_lock() {
    let r = parse(
        &TEST_ROOT,
        &svec!["deno", "run", "--lock=lock.json", "script.ts"],
    )
    .unwrap();
    assert_eq!(r.get_one("lock"), Some("lock.json"));
}

#[test]
fn run_lock_bare() {
    let r = parse(
        &TEST_ROOT,
        &svec!["deno", "run", "--lock", "script.ts"],
    )
    .unwrap();
    // --lock with require_equals and no = means present but no value
    assert!(r.get_bool("lock"));
    assert_eq!(r.get_one("lock"), None);
}

#[test]
fn run_no_lock() {
    let r = parse(
        &TEST_ROOT,
        &svec!["deno", "run", "--no-lock", "script.ts"],
    )
    .unwrap();
    assert!(r.get_bool("no-lock"));
}

#[test]
fn watch_exclude() {
    let r = parse(
        &TEST_ROOT,
        &svec!["deno", "run", "--watch", "--watch-exclude=node_modules/,dist/", "script.ts"],
    )
    .unwrap();
    assert!(r.get_bool("watch"));
    assert_eq!(
        r.get_many("watch-exclude"),
        Some(vec!["node_modules/".to_string(), "dist/".to_string()].as_slice())
    );
}

#[test]
fn test_with_allow_flags() {
    let r = parse(
        &TEST_ROOT,
        &svec!["deno", "test", "-A", "test_file.ts"],
    )
    .unwrap();
    assert_eq!(r.subcommand.as_deref(), Some("test"));
    assert!(r.get_bool("allow-all"));
    assert_eq!(
        r.get_many("files"),
        Some(vec!["test_file.ts".to_string()].as_slice())
    );
}

#[test]
fn run_env_file() {
    let r = parse(
        &TEST_ROOT,
        &svec!["deno", "run", "--env-file", "script.ts"],
    )
    .unwrap();
    assert!(r.get_bool("env-file"));
    assert_eq!(r.get_one("script_arg"), Some("script.ts"));
}

#[test]
fn run_env_file_with_value() {
    let r = parse(
        &TEST_ROOT,
        &svec!["deno", "run", "--env-file=.env.local", "script.ts"],
    )
    .unwrap();
    assert_eq!(r.get_one("env-file"), Some(".env.local"));
}

#[test]
fn run_coverage() {
    let r = parse(
        &TEST_ROOT,
        &svec!["deno", "run", "--coverage=./cov", "script.ts"],
    )
    .unwrap();
    assert_eq!(r.get_one("coverage"), Some("./cov"));
}

#[test]
fn run_no_config() {
    let r = parse(
        &TEST_ROOT,
        &svec!["deno", "run", "--no-config", "script.ts"],
    )
    .unwrap();
    assert!(r.get_bool("no-config"));
}

#[test]
fn levenshtein_basics() {
    // Verify the suggestion engine works
    let err = parse(
        &TEST_ROOT,
        &svec!["deno", "run", "--allow-raed", "script.ts"],
    )
    .unwrap_err();
    assert!(err.suggestion.as_ref().unwrap().contains("--allow-read"));
}
