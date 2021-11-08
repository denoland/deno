// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
/// FLAGS

import { parse } from "../../test_util/std/flags/mod.ts";
import { join, resolve, ROOT_PATH } from "../util.js";

export const {
  json,
  wptreport,
  quiet,
  release,
  rebuild,
  ["--"]: rest,
  ["auto-config"]: autoConfig,
  binary,
} = parse(Deno.args, {
  "--": true,
  boolean: ["quiet", "release", "no-interactive"],
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

const MANIFEST_PATH = join(ROOT_PATH, "./tools/wpt/manifest.json");

export async function updateManifest() {
  const proc = runPy(
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
  );
  const status = await proc.status();
  assert(status.success, "updating wpt manifest should succeed");
}

export function getManifest(): Manifest {
  const manifestText = Deno.readTextFileSync(MANIFEST_PATH);
  return JSON.parse(manifestText);
}

/// WPT TEST EXPECTATIONS

const EXPECTATION_PATH = join(ROOT_PATH, "./tools/wpt/expectation.json");

export interface Expectation {
  [key: string]: Expectation | boolean | string[];
}

export function getExpectation(): Expectation {
  const expectationText = Deno.readTextFileSync(EXPECTATION_PATH);
  return JSON.parse(expectationText);
}

export function saveExpectation(expectation: Expectation) {
  Deno.writeTextFileSync(
    EXPECTATION_PATH,
    JSON.stringify(expectation, undefined, "  "),
  );
}

export function getExpectFailForCase(
  expectation: boolean | string[],
  caseName: string,
): boolean {
  if (typeof expectation == "boolean") {
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

export function runPy(
  args: string[],
  options: Omit<Omit<Deno.RunOptions, "cmd">, "cwd">,
): Deno.Process {
  const cmd = Deno.build.os == "windows" ? "python.exe" : "python3";
  return Deno.run({
    cmd: [cmd, ...args],
    cwd: join(ROOT_PATH, "./test_util/wpt/"),
    ...options,
  });
}

export async function checkPy3Available() {
  const proc = runPy(["--version"], { stdout: "piped" });
  const status = await proc.status();
  assert(status.success, "failed to run python --version");
  const output = new TextDecoder().decode(await proc.output());
  assert(
    output.includes("Python 3."),
    `The ${
      Deno.build.os == "windows" ? "python.exe" : "python3"
    } in your path is not Python 3.`,
  );
}

export async function cargoBuild() {
  if (binary) return;
  const proc = Deno.run({
    cmd: ["cargo", "build", ...(release ? ["--release"] : [])],
    cwd: ROOT_PATH,
  });
  const status = await proc.status();
  proc.close();
  assert(status.success, "cargo build failed");
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
  };
  const proc = Deno.run({
    cmd: ["git", "rev-parse", "HEAD"],
    cwd: join(ROOT_PATH, "test_util", "wpt"),
    stdout: "piped",
  });
  await proc.status();
  const revision = (new TextDecoder().decode(await proc.output())).trim();
  proc.close();
  const proc2 = Deno.run({
    cmd: [denoBinary(), "eval", "console.log(JSON.stringify(Deno.version))"],
    cwd: join(ROOT_PATH, "test_util", "wpt"),
    stdout: "piped",
  });
  await proc2.status();
  const version = JSON.parse(new TextDecoder().decode(await proc2.output()));
  proc2.close();
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
