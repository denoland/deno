// Copyright 2018-2025 the Deno authors. MIT license.

import { parse } from "@std/toml";

export interface SingleFileConfig {
  flaky?: boolean;
  windows?: boolean;
  darwin?: boolean;
  linux?: boolean;
  /** Optional reason for ignoring the test */
  reason?: string;
}

type Config = {
  tests: Record<string, SingleFileConfig>;
};

export const configFile = await Deno.readTextFile(
  new URL("./config.toml", import.meta.url),
).then(parse) as Config;

/** Checks if the test file uses `node:test` module */
export function usesNodeTestModule(testSource: string): boolean {
  return testSource.includes("'node:test'");
}

export const RUN_ARGS = [
  "-A",
  "--quiet",
  "--unstable-unsafe-proto",
  "--unstable-bare-node-builtins",
];

export const TEST_ARGS = [
  "test",
  ...RUN_ARGS,
  "--no-check",
  "--unstable-detect-cjs",
];

/** Parses the special "Flags:"" syntax in Node.js test files */
export function parseFlags(source: string): string[] {
  const line = /^\/\/ Flags: (.+)$/um.exec(source);
  if (line == null) return [];
  return line[1].split(" ");
}
