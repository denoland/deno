// Copyright 2018-2026 the Deno authors. MIT license.

// deno-lint-ignore-file no-console

import $ from "jsr:@david/dax@^0.42.0";
import { bold, cyan, dim, green, yellow } from "jsr:@std/fmt@^1/colors";

// ---------------------------------------------------------------------------
// Command registry
// ---------------------------------------------------------------------------

interface Command {
  /** One-line summary shown in the top-level help listing. */
  description: string;
  /**
   * Detailed help text shown when the user runs `./x <command> --help`.
   * Should be thorough enough for LLMs and AI agents to understand
   * what the command does, when to use it, and what it invokes under
   * the hood.
   */
  help: string;
  fn: () => Promise<void>;
}

function buildCommands(root: ReturnType<typeof $.path>): Record<string, Command> {
  const fmtCmd: Command = {
    description: "Format all code (JS/TS/Rust/etc. via dprint)",
    help: `Formats the entire codebase using the project's formatting tool chain.
This runs dprint (configured in dprint.json) which handles JavaScript,
TypeScript, JSON, JSONC, Markdown, TOML, and Rust formatting.

You should run this before committing any changes. It is also included
in the './x verify' pre-commit workflow.

Under the hood:
  deno run -A tools/format.js`,
    async fn() {
      $.logStep("Formatting code...");
      await $`deno run -A tools/format.js`.cwd(root);
      $.logStep("Formatting complete.");
    },
  };

  const lintJsCmd: Command = {
    description: "Lint JavaScript/TypeScript only (skip Rust/clippy)",
    help: `Runs linting only on JavaScript and TypeScript files, skipping Rust
clippy. This is significantly faster than './x lint' when you have
not modified any Rust code.

Under the hood:
  deno run -A tools/lint.js --js`,
    async fn() {
      $.logStep("Linting JavaScript/TypeScript...");
      await $`deno run -A tools/lint.js --js`.cwd(root);
      $.logStep("JS lint complete.");
    },
  };

  return {
    "setup": {
      description: "Initial setup: build deno and test_server",
      help: `Sets up the development environment by compiling both the main 'deno'
binary and the 'test_server' binary used by the test suite.

Run this once after cloning the repository, or after pulling changes that
modify Rust code, to ensure you have working binaries for development and
testing.

Under the hood:
  cargo build --bin deno --bin test_server`,
      async fn() {
        $.logStep("Setting up development environment...");
        $.logStep("Building deno and test_server...");
        await $`cargo build --bin deno --bin test_server`.cwd(root);
        $.logStep("Setup complete.");
      },
    },
    "build": {
      description: "Build the deno binary (debug mode)",
      help: `Compiles the main 'deno' binary in debug mode (unoptimized, with
debug symbols). The resulting binary is placed at ./target/debug/deno.

Use this during normal development iteration. For a release-optimized
build, run 'cargo build --release' directly.

If you only need to check for compilation errors without producing a
binary, use './x check' instead — it is significantly faster because
it skips the linking step.

Under the hood:
  cargo build --bin deno`,
      async fn() {
        $.logStep("Building Deno...");
        await $`cargo build --bin deno`.cwd(root);
        $.logStep("Build complete.");
      },
    },
    "check": {
      description: "Fast compile check (no linking)",
      help: `Runs 'cargo check' on the entire workspace to verify that all Rust
code compiles without errors. This is faster than './x build' because
it skips code generation and linking — no binary is produced.

Use this for rapid feedback while editing Rust code. It catches type
errors, borrow-checker issues, and missing imports without the overhead
of a full build.

Under the hood:
  cargo check`,
      async fn() {
        $.logStep("Checking (no linking)...");
        await $`cargo check`.cwd(root);
        $.logStep("Check complete.");
      },
    },
    "test-unit": {
      description: "Run Deno runtime unit tests",
      help: `Runs the Deno runtime unit tests. These are JavaScript/TypeScript tests
that exercise Deno's built-in APIs (e.g. Deno.readFile, Deno.serve,
Web API implementations) by running them inside the Deno runtime itself.

The test files live under tests/unit/ and are compiled into the
'unit_tests' crate.

Under the hood:
  cargo test -p unit_tests --test unit`,
      async fn() {
        $.logStep("Running unit tests...");
        await $`cargo test -p unit_tests --test unit`.cwd(root);
        $.logStep("Unit tests complete.");
      },
    },
    "test-node": {
      description: "Run Node.js API unit tests",
      help: `Runs unit tests for Deno's Node.js compatibility layer. These tests
verify that Deno's implementations of Node.js built-in modules (fs,
path, http, crypto, etc.) behave correctly.

The test files live under tests/unit_node/ and are compiled into the
'unit_node_tests' crate.

Under the hood:
  cargo test -p unit_node_tests --test unit_node`,
      async fn() {
        $.logStep("Running Node.js unit tests...");
        await $`cargo test -p unit_node_tests --test unit_node`.cwd(root);
        $.logStep("Node.js unit tests complete.");
      },
    },
    "test-compat": {
      description: "Run Node.js compatibility tests",
      help: `Runs the Node.js compatibility test suite. These tests use actual
Node.js test cases (ported or adapted) to verify that Deno's node:*
module implementations match Node.js behavior.

The test runner lives in tests/node_compat/runner/.

Under the hood:
  deno task --cwd tests/node_compat/runner test`,
      async fn() {
        $.logStep("Running Node.js compatibility tests...");
        await $`deno task --cwd tests/node_compat/runner test`.cwd(root);
        $.logStep("Node.js compatibility tests complete.");
      },
    },
    "test-spec": {
      description: "Run spec (integration) tests",
      help: `Runs the spec integration tests. These are the primary integration tests
for Deno's CLI — each test defines CLI commands to execute and asserts
on their stdout/stderr output.

Spec tests live under tests/specs/. Each test directory contains a
'__test__.jsonc' file describing the commands to run and expected output
(using wildcards like [WILDCARD], [WILDLINE], etc.).

Use this to verify end-to-end CLI behavior after making changes to
subcommands, flag parsing, error messages, or module resolution.

Under the hood:
  cargo test -p specs_tests --test specs`,
      async fn() {
        $.logStep("Running spec tests...");
        await $`cargo test -p specs_tests --test specs`.cwd(root);
        $.logStep("Spec tests complete.");
      },
    },
    "fmt": fmtCmd,
    "lint": {
      description: "Lint all code (JS/TS + Rust)",
      help: `Runs the full lint suite across the entire codebase, including both
JavaScript/TypeScript linting (via deno lint and dlint) and Rust
linting (via clippy).

Use this when you have changed both JS/TS and Rust code. If you only
changed JS/TS files, './x lint-js' is faster.

Under the hood:
  deno run -A tools/lint.js`,
      async fn() {
        $.logStep("Linting code...");
        await $`deno run -A tools/lint.js`.cwd(root);
        $.logStep("Linting complete.");
      },
    },
    "lint-js": lintJsCmd,
    "verify": {
      description: "Pre-commit verification (fmt + lint-js)",
      help: `Runs the recommended pre-commit checks: formats all code, then lints
JavaScript/TypeScript. This is the minimum verification you should do
before committing changes that only touch JS/TS files.

If you have also changed Rust code, you should additionally run
'./x lint' (which includes clippy) and './x check' to catch Rust
compilation errors.

Equivalent to running:
  ./x fmt
  ./x lint-js`,
      async fn() {
        $.logStep("Running pre-commit verification...");
        await fmtCmd.fn();
        await lintJsCmd.fn();
        $.logStep("Verification complete.");
      },
    },
  };
}

// ---------------------------------------------------------------------------
// CLI
// ---------------------------------------------------------------------------

function printHelp(COMMANDS: Record<string, Command>) {
  console.log();
  console.log(`  ${bold(cyan("x"))} ${dim("-")} Developer CLI for contributing to Deno`);
  console.log();
  console.log(`  ${bold("USAGE")}`);
  console.log(`    ${dim("$")} ./x ${green("<command>")} [options]`);
  console.log();
  console.log(`  ${bold("EXAMPLES")}`);
  console.log(`    ${dim("$")} ./x ${green("build")}          ${dim("# build the deno binary")}`);
  console.log(`    ${dim("$")} ./x ${green("test-spec")}      ${dim("# run spec integration tests")}`);
  console.log(`    ${dim("$")} ./x ${green("fmt")}            ${dim("# format the codebase")}`);
  console.log(`    ${dim("$")} ./x ${green("build --help")}   ${dim("# show detailed help for a command")}`);
  console.log();
  console.log(`  ${bold("COMMANDS")}`);
  for (const [name, cmd] of Object.entries(COMMANDS)) {
    console.log(`    ${green(name.padEnd(20))} ${cmd.description}`);
  }
  console.log();
  console.log(`  ${bold("OPTIONS")}`);
  console.log(`    ${yellow("--help, -h")}           Show this help message`);
  console.log();
  console.log(`  Run ${cyan("./x <command> --help")} for detailed information about a specific command.`);
  console.log();
}

function printCommandHelp(name: string, cmd: Command) {
  console.log();
  console.log(`  ${bold(cyan("x"))} ${bold(green(name))} ${dim("-")} ${cmd.description}`);
  console.log();
  for (const line of cmd.help.split("\n")) {
    console.log(`  ${line}`);
  }
  console.log();
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

export async function main(dirname: string) {
  const root = $.path(dirname);
  const COMMANDS = buildCommands(root);
  const args = Deno.args;

  if (
    args.length === 0 || args[0] === "--help" || args[0] === "-h" ||
    args[0] === "help"
  ) {
    printHelp(COMMANDS);
    Deno.exit(0);
  }

  const subcommand = args[0];
  const cmd = COMMANDS[subcommand];

  if (!cmd) {
    $.logError(`Unknown command '${subcommand}'.`);
    console.log();
    printHelp(COMMANDS);
    Deno.exit(1);
  }

  if (args.includes("--help") || args.includes("-h")) {
    printCommandHelp(subcommand, cmd);
    Deno.exit(0);
  }

  await cmd.fn();
}
