#!/usr/bin/env -S deno run --allow-all --ext=ts
// Copyright 2018-2026 the Deno authors. MIT license.

// deno-lint-ignore-file no-console

/**
 * x - Developer CLI for contributing to Deno
 *
 * Inspired by Servo's mach tool, this script provides a unified
 * interface for common development tasks like building, testing, and more.
 *
 * Usage:
 *   ./x <command> [options]
 *
 * Run `./x --help` for more information.
 */

import $ from "jsr:@david/dax@^0.42.0";

const root = $.path(import.meta.dirname!);

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

async function setup() {
  $.logStep("Setting up development environment...");
  $.logStep("Building deno and test_server...");
  await $`cargo build --bin deno --bin test_server`.cwd(root);
  $.logStep("Setup complete.");
}

async function build() {
  $.logStep("Building Deno...");
  await $`cargo build --bin deno`.cwd(root);
  $.logStep("Build complete.");
}

async function check() {
  $.logStep("Checking (no linking)...");
  await $`cargo check`.cwd(root);
  $.logStep("Check complete.");
}

async function testUnit() {
  $.logStep("Running unit tests...");
  await $`cargo test -p unit_tests --test unit`.cwd(root);
  $.logStep("Unit tests complete.");
}

async function testNodeUnit() {
  $.logStep("Running Node.js unit tests...");
  await $`cargo test -p unit_node_tests --test unit_node`.cwd(root);
  $.logStep("Node.js unit tests complete.");
}

async function testNodeCompat() {
  $.logStep("Running Node.js compatibility tests...");
  await $`deno task --cwd tests/node_compat/runner test`.cwd(root);
  $.logStep("Node.js compatibility tests complete.");
}

async function testSpec() {
  $.logStep("Running spec tests...");
  await $`cargo test -p specs_tests --test specs`.cwd(root);
  $.logStep("Spec tests complete.");
}

async function fmt() {
  $.logStep("Formatting code...");
  await $`deno run -A tools/format.js`.cwd(root);
  $.logStep("Formatting complete.");
}

async function lint() {
  $.logStep("Linting code...");
  await $`deno run -A tools/lint.js`.cwd(root);
  $.logStep("Linting complete.");
}

async function lintJs() {
  $.logStep("Linting JavaScript/TypeScript...");
  await $`deno run -A tools/lint.js --js`.cwd(root);
  $.logStep("JS lint complete.");
}

async function verify() {
  $.logStep("Running pre-commit verification...");
  await fmt();
  await lintJs();
  $.logStep("Verification complete.");
}

// ---------------------------------------------------------------------------
// Command registry
// ---------------------------------------------------------------------------

const COMMANDS: Record<
  string,
  { description: string; fn: () => Promise<void> }
> = {
  "setup": {
    description: "Initial setup: build deno and test_server",
    fn: setup,
  },
  "build": {
    description: "Build the deno binary",
    fn: build,
  },
  "check": {
    description: "Fast compile check (no linking)",
    fn: check,
  },
  "test-unit": {
    description: "Run unit tests (cargo test -p unit_tests)",
    fn: testUnit,
  },
  "test-node": {
    description: "Run Node.js unit tests",
    fn: testNodeUnit,
  },
  "test-compat": {
    description: "Run Node.js compatibility tests",
    fn: testNodeCompat,
  },
  "test-spec": {
    description: "Run spec (integration) tests",
    fn: testSpec,
  },
  "fmt": {
    description: "Format the code (dprint)",
    fn: fmt,
  },
  "lint": {
    description: "Lint code (JS + Rust)",
    fn: lint,
  },
  "lint-js": {
    description: "Lint JavaScript/TypeScript only",
    fn: lintJs,
  },
  "verify": {
    description: "Pre-commit verification (fmt + lint-js)",
    fn: verify,
  },
};

// ---------------------------------------------------------------------------
// CLI
// ---------------------------------------------------------------------------

const BOLD = "\x1b[1m";
const RESET = "\x1b[0m";
const GREEN = "\x1b[32m";
const CYAN = "\x1b[36m";
const DIM = "\x1b[2m";
const YELLOW = "\x1b[33m";

function printHelp() {
  console.log();
  console.log(
    `  ${BOLD}${CYAN}x${RESET} ${DIM}-${RESET} Developer CLI for contributing to Deno`,
  );
  console.log();
  console.log(`  ${BOLD}USAGE${RESET}`);
  console.log(
    `    ${DIM}$${RESET} ./x ${GREEN}build${RESET}          ${DIM}# build the deno binary${RESET}`,
  );
  console.log(
    `    ${DIM}$${RESET} ./x ${GREEN}test-spec${RESET}      ${DIM}# run spec integration tests${RESET}`,
  );
  console.log(
    `    ${DIM}$${RESET} ./x ${GREEN}fmt${RESET}            ${DIM}# format the codebase${RESET}`,
  );
  console.log();
  console.log(`  ${BOLD}COMMANDS${RESET}`);
  for (const [name, cmd] of Object.entries(COMMANDS)) {
    console.log(`    ${GREEN}${name.padEnd(20)}${RESET} ${cmd.description}`);
  }
  console.log();
  console.log(`  ${BOLD}OPTIONS${RESET}`);
  console.log(
    `    ${YELLOW}--help, -h${RESET}           Show this help message`,
  );
  console.log();
}

async function main() {
  const args = Deno.args;

  if (
    args.length === 0 || args[0] === "--help" || args[0] === "-h" ||
    args[0] === "help"
  ) {
    printHelp();
    return;
  }

  const subcommand = args[0];
  const cmd = COMMANDS[subcommand];

  if (!cmd) {
    $.logError(`Unknown command '${subcommand}'.`);
    console.log();
    printHelp();
    Deno.exit(1);
  }

  await cmd.fn();
}

await main();
