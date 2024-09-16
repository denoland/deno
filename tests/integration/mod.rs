// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

#![allow(clippy::print_stdout)]
#![allow(clippy::print_stderr)]

// These files have `_tests.rs` suffix to make it easier to tell which file is
// the test (ex. `lint_tests.rs`) and which is the implementation (ex. `lint.rs`)
// when both are open, especially for two tabs in VS Code

#[path = "bench_tests.rs"]
mod bench;
#[path = "cache_tests.rs"]
mod cache;
#[path = "check_tests.rs"]
mod check;
#[path = "compile_tests.rs"]
mod compile;
#[path = "coverage_tests.rs"]
mod coverage;
#[path = "eval_tests.rs"]
mod eval;
#[path = "flags_tests.rs"]
mod flags;
#[path = "fmt_tests.rs"]
mod fmt;
#[path = "init_tests.rs"]
mod init;
#[path = "inspector_tests.rs"]
mod inspector;
#[path = "install_tests.rs"]
mod install;
#[path = "js_unit_tests.rs"]
mod js_unit_tests;
#[path = "jsr_tests.rs"]
mod jsr;
#[path = "jupyter_tests.rs"]
mod jupyter;
#[path = "lsp_tests.rs"]
mod lsp;
#[path = "node_unit_tests.rs"]
mod node_unit_tests;
#[path = "npm_tests.rs"]
mod npm;
#[path = "pm_tests.rs"]
mod pm;
#[path = "publish_tests.rs"]
mod publish;

#[path = "repl_tests.rs"]
mod repl;
#[path = "run_tests.rs"]
mod run;
#[path = "serve_tests.rs"]
mod serve;
#[path = "shared_library_tests.rs"]
mod shared_library_tests;
#[path = "task_tests.rs"]
mod task;
#[path = "test_tests.rs"]
mod test;
#[path = "upgrade_tests.rs"]
mod upgrade;
#[path = "watcher_tests.rs"]
mod watcher;
