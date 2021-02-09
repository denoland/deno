/// FLAGS

import { parse } from "https://deno.land/std@0.84.0/flags/mod.ts";
import { join, ROOT_PATH } from "../util.js";

export const {
  json,
  quiet,
  release,
  rebuild,
  ["--"]: rest,
  ["auto-config"]: autoConfig,
} = parse(Deno.args, {
  "--": true,
  boolean: ["quiet", "release", "no-interactive"],
  string: ["json"],
});

/// PAGE ROOT

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
  name?: string;
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

export function generateTestExpectations(filter: string[]) {
  const manifest = getManifest();

  function walk(folder: ManifestFolder, prefix: string): Expectation {
    const expectation: Expectation = {};
    for (const key in folder) {
      const path = `${prefix}/${key}`;
      const entry = folder[key];
      if (Array.isArray(entry)) {
        if (!filter.find((filter) => path.startsWith(filter))) continue;
        if (key.endsWith(".js")) {
          expectation[key] = false;
        }
      } else {
        if (!filter.find((filter) => `${path}/`.startsWith(filter))) continue;
        expectation[key] = walk(entry, path);
      }
    }
    for (const key in expectation) {
      const entry = expectation[key];
      if (typeof entry === "object") {
        if (Object.keys(expectation[key]).length === 0) {
          delete expectation[key];
        }
      }
    }
    return expectation;
  }

  return walk(manifest.items.testharness, "");
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
  const proc = Deno.run({
    cmd: ["cargo", "build", ...(release ? ["--release"] : [])],
    cwd: ROOT_PATH,
  });
  const status = await proc.status();
  proc.close();
  assert(status.success, "cargo build failed");
}
