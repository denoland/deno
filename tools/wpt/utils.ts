/// FLAGS

import { parse } from "https://deno.land/std@0.84.0/flags/mod.ts";

export const { json, quiet, release, ["--"]: rest } = parse(Deno.args, {
  "--": true,
  boolean: ["quiet", "release"],
  string: ["json"],
});

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
  ...variations: ManifestTestVariation[]
];
export type ManifestTestVariation = [
  path: string,
  options: ManifestTestOptions
];
export interface ManifestTestOptions {}

export async function updateManifest() {
  const proc = runPy(
    [
      "wpt",
      "manifest",
      "--tests-root",
      ".",
      "-p",
      "../../tools/wpt/manifest.json",
    ],
    {}
  );
  const status = await proc.status();
  assert(status.success, "updating wpt manifest should succeed");
}

export function getManifest(): Manifest {
  const manifestText = Deno.readTextFileSync("./tools/wpt/manifest.json");
  return JSON.parse(manifestText);
}

/// WPT TEST EXPECTATIONS

export interface Expectation {
  [key: string]: Expectation | boolean | string[];
}

export function getExpectations(): Expectation {
  const expectationText = Deno.readTextFileSync("./tools/wpt/expectation.json");
  return JSON.parse(expectationText);
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
  options: Omit<Omit<Deno.RunOptions, "cmd">, "cwd">
): Deno.Process {
  const cmd = Deno.build.os == "windows" ? "python.exe" : "python3";
  return Deno.run({
    cmd: [cmd, ...args],
    cwd: "./test_util/wpt/",
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
    } in your path is not is not Python 3.`
  );
}
