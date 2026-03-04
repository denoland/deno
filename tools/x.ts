#!/usr/bin/env -S deno run --allow-all --config=tests/config/deno.json
// Copyright 2018-2026 the Deno authors. MIT license.

// deno-lint-ignore-file no-console

/**
 * x.ts - Developer CLI for contributing to Deno
 *
 * Inspired by Servo's mach tool, this script provides a unified
 * interface for common development tasks like building, testing, and more.
 *
 * Usage:
 *   ./tools/x.ts <subcommand> [options]
 *
 * Run `./tools/x.ts --help` for more information.
 */

import { ROOT_PATH } from "./util.js";

const BOLD = "\x1b[1m";
const RESET = "\x1b[0m";
const GREEN = "\x1b[32m";
const RED = "\x1b[31m";
const YELLOW = "\x1b[33m";

const HELP_TEXT = `${BOLD}x.ts - Developer CLI for contributing to Deno${RESET}

Inspired by Servo's mach tool. Provides common commands for building,
testing, and contributing to the Deno project.

${BOLD}USAGE:${RESET}
    ./tools/x.ts <subcommand> [options]

${BOLD}SUBCOMMANDS:${RESET}
    ${GREEN}build${RESET}           Build the Deno project using Cargo
    ${GREEN}test${RESET}            Run tests
    ${GREEN}help${RESET}            Show this help message

${BOLD}TEST TARGETS:${RESET}
    ${GREEN}test node_unit${RESET}   Build first, then run Node.js unit tests

${BOLD}OPTIONS:${RESET}
    --help, -h      Show this help message

${BOLD}EXAMPLES:${RESET}
    ./tools/x.ts build           # Build the project
    ./tools/x.ts test node_unit  # Build and run Node.js unit tests
`;

/** Run a command, streaming output to the terminal. Returns the exit code. */
async function runCommand(
  command: string,
  args: string[],
  label: string,
): Promise<number> {
  console.log(`${YELLOW}>>> ${label}${RESET}`);
  console.log(`${YELLOW}>>> Running: ${command} ${args.join(" ")}${RESET}\n`);

  const cmd = new Deno.Command(command, {
    args,
    cwd: ROOT_PATH,
    stdout: "inherit",
    stderr: "inherit",
  });

  const { code } = await cmd.output();

  if (code !== 0) {
    console.error(`\n${RED}>>> Command failed with exit code ${code}${RESET}`);
  } else {
    console.log(`\n${GREEN}>>> Done: ${label}${RESET}`);
  }

  return code;
}

async function build(): Promise<number> {
  return await runCommand("cargo", ["build"], "Building Deno");
}

async function testNodeUnit(): Promise<number> {
  // First, build the project
  const buildCode = await build();
  if (buildCode !== 0) {
    console.error(`${RED}Build failed, skipping tests.${RESET}`);
    return buildCode;
  }

  // Then, run the node unit tests
  return await runCommand(
    "cargo",
    ["test", "node_unit"],
    "Running Node.js unit tests",
  );
}

function printHelp(): void {
  console.log(HELP_TEXT);
}

async function main(): Promise<number> {
  const args = Deno.args;

  if (args.length === 0 || args[0] === "--help" || args[0] === "-h" ||
    args[0] === "help") {
    printHelp();
    return 0;
  }

  const subcommand = args[0];

  switch (subcommand) {
    case "build":
      return await build();

    case "test": {
      const target = args[1];
      if (!target) {
        console.error(
          `${RED}Error: 'test' subcommand requires a target.${RESET}`,
        );
        console.error(`\nAvailable test targets:`);
        console.error(`  node_unit  - Run Node.js unit tests`);
        return 1;
      }
      if (target === "node_unit") {
        return await testNodeUnit();
      }
      console.error(
        `${RED}Error: Unknown test target '${target}'.${RESET}`,
      );
      console.error(`\nAvailable test targets:`);
      console.error(`  node_unit  - Run Node.js unit tests`);
      return 1;
    }

    default:
      console.error(
        `${RED}Error: Unknown subcommand '${subcommand}'.${RESET}`,
      );
      console.error(`Run './tools/x.ts --help' for usage information.`);
      return 1;
  }
}

Deno.exit(await main());
