// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
/// FLAGS

import { parseArgs } from "@std/cli/parse-args";
import { join, resolve, ROOT_PATH } from "../../../tools/util.js";

export const {
  json,
  wptreport,
  quiet,
  release,
  rebuild,
  ["--"]: rest,
  ["auto-config"]: autoConfig,
  ["inspect-brk"]: inspectBrk,
  ["no-ignore"]: noIgnore,
  binary,
} = parseArgs(Deno.args, {
  "--": true,
  boolean: ["quiet", "release", "no-interactive", "inspect-brk", "no-ignore"],
  string: ["json", "wptreport", "binary"],
});

export function denoBinary() {
  if (binary) {
    return resolve(binary);
  }
  return join(ROOT_PATH, `./target/${release ? "release" : "debug"}/deno`);
}

/// WPT TEST MANIFEST

export interface Manifest {
  items: {
    testharness: ManifestFolder;
  };
}
export interface ManifestFolder {
  [key: string]: ManifestFolder | ManifestTest;
}
export type ManifestTest = [
  hash: string,
  ...variations: ManifestTestVariation[],
];
export type ManifestTestVariation = [
  path: string,
  options: ManifestTestOptions,
];
export interface ManifestTestOptions {
  // deno-lint-ignore camelcase
  script_metadata: [string, string][];
}

const MANIFEST_PATH = join(ROOT_PATH, "./tests/wpt/runner/manifest.json");

export async function updateManifest() {
  const status = await runPy(
    [
      "wpt",
      "manifest",
      "--tests-root",
      ".",
      "-p",
      MANIFEST_PATH,
      ...(rebuild ? ["--rebuild"] : []),
    ],
    {},
  ).status;
  assert(status.success, "updating wpt manifest should succeed");
}

export function getManifest(): Manifest {
  const manifestText = Deno.readTextFileSync(MANIFEST_PATH);
  return JSON.parse(manifestText);
}

/// WPT TEST EXPECTATIONS

export const EXPECTATION_PATH = join(
  ROOT_PATH,
  "./tests/wpt/runner/expectation.json",
);

export interface Expectation {
  [key: string]: Expectation | boolean | string[] | { ignore: boolean };
}

export function getExpectation(): Expectation {
  const expectationText = Deno.readTextFileSync(EXPECTATION_PATH);
  return JSON.parse(expectationText);
}

export function saveExpectation(
  expectation: Expectation,
  path: string = EXPECTATION_PATH,
) {
  Deno.writeTextFileSync(
    path,
    JSON.stringify(expectation, undefined, "  ") + "\n",
  );
}

export function getExpectFailForCase(
  expectation: boolean | string[],
  caseName: string,
): boolean {
  if (noIgnore) return false;
  if (typeof expectation === "boolean") {
    return !expectation;
  }
  return expectation.includes(caseName);
}

/// UTILS

class AssertionError extends Error {
  name = "AssertionError";
  constructor(message: string) {
    super(message);
  }
}

export function assert(condition: unknown, message: string): asserts condition {
  if (!condition) {
    throw new AssertionError(message);
  }
}

export function runPy<T extends Omit<Deno.CommandOptions, "cwd">>(
  args: string[],
  options: T,
): Deno.ChildProcess {
  const cmd = Deno.build.os == "windows" ? "python.exe" : "python3";
  return new Deno.Command(cmd, {
    args,
    stdout: "inherit",
    stderr: "inherit",
    ...options,
    cwd: join(ROOT_PATH, "./tests/wpt/suite/"),
  }).spawn();
}

export async function runGitDiff(args: string[]): string {
  await new Deno.Command("git", {
    args: ["diff", ...args],
    stdout: "inherit",
    stderr: "inherit",
    cwd: ROOT_PATH,
  }).output();
}

export async function checkPy3Available() {
  const { success, stdout } = await runPy(["--version"], {
    stdout: "piped",
  }).output();
  assert(success, "failed to run python --version");
  const output = new TextDecoder().decode(stdout);
  assert(
    output.includes("Python 3.11"),
    `The ${
      Deno.build.os == "windows" ? "python.exe" : "python3"
    } in your path is not Python 3.11.x. See https://github.com/web-platform-tests/wpt/issues/44427 for more details.`,
  );
}

export async function cargoBuild() {
  if (binary) return;
  const { success } = await new Deno.Command("cargo", {
    args: ["build", ...(release ? ["--release"] : [])],
    cwd: ROOT_PATH,
    stdout: "inherit",
    stderr: "inherit",
  }).output();
  assert(success, "cargo build failed");
}

export function escapeLoneSurrogates(input: string): string;
export function escapeLoneSurrogates(input: string | null): string | null;
export function escapeLoneSurrogates(input: string | null): string | null {
  if (input === null) return null;
  return input.replace(
    /[\uD800-\uDFFF]/gu,
    (match) => `U+${match.charCodeAt(0).toString(16)}`,
  );
}

/// WPTREPORT

export async function generateRunInfo(): Promise<unknown> {
  const oses = {
    "windows": "win",
    "darwin": "mac",
    "linux": "linux",
    "freebsd": "freebsd",
    "openbsd": "openbsd",
  };
  const proc = await new Deno.Command("git", {
    args: ["rev-parse", "HEAD"],
    cwd: join(ROOT_PATH, "tests", "wpt", "suite"),
    stderr: "inherit",
  }).output();
  const revision = (new TextDecoder().decode(proc.stdout)).trim();
  const proc2 = await new Deno.Command(denoBinary(), {
    args: ["eval", "console.log(JSON.stringify(Deno.version))"],
    cwd: join(ROOT_PATH, "tests", "wpt", "suite"),
  }).output();
  const version = JSON.parse(new TextDecoder().decode(proc2.stdout));
  const runInfo = {
    "os": oses[Deno.build.os],
    "processor": Deno.build.arch,
    "version": "unknown",
    "os_version": "unknown",
    "bits": 64,
    "has_sandbox": true,
    "webrender": false,
    "automation": false,
    "linux_distro": "unknown",
    "revision": revision,
    "python_version": 3,
    "product": "deno",
    "debug": false,
    "browser_version": version.deno,
    "browser_channel": version.deno.includes("+") ? "canary" : "stable",
    "verify": false,
    "wasm": false,
    "headless": true,
  };
  return runInfo;
}
