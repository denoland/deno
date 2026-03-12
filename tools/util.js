// Copyright 2018-2026 the Deno authors. MIT license.

// deno-lint-ignore-file no-console

import {
  dirname,
  extname,
  fromFileUrl,
  join,
  resolve,
  SEPARATOR,
  toFileUrl,
} from "@std/path";
export { dirname, extname, fromFileUrl, join, resolve, SEPARATOR, toFileUrl };
export { existsSync, expandGlobSync, walk } from "@std/fs";
export { TextLineStream } from "@std/streams/text-line-stream";
export { delay } from "@std/async/delay";
export { parse as parseJSONC } from "@std/jsonc/parse";
import { createHash } from "node:crypto";

// [toolName] --version output
const versions = {
  "dlint": "dlint 0.73.0",
};

const compressed = new Set(["ld64.lld", "rcodesign"]);

export const ROOT_PATH = dirname(dirname(fromFileUrl(import.meta.url)));

async function getFilesFromGit(baseDir, args) {
  const { success, stdout } = await new Deno.Command("git", {
    stderr: "inherit",
    args,
  }).output();
  const output = new TextDecoder().decode(stdout);
  if (!success) {
    throw new Error("gitLsFiles failed");
  }

  const files = output
    .split("\0")
    .filter((line) => line.length > 0)
    .map((filePath) => {
      try {
        return Deno.realPathSync(join(baseDir, filePath));
      } catch {
        return null;
      }
    })
    .filter((filePath) => filePath !== null);

  return files;
}

export function gitLsFiles(baseDir, patterns) {
  baseDir = Deno.realPathSync(baseDir);
  const cmd = [
    "-C",
    baseDir,
    "ls-files",
    "-z",
    "--exclude-standard",
    "--cached",
    "--modified",
    "--others",
    "--",
    ...patterns,
  ];
  return getFilesFromGit(baseDir, cmd);
}

/** List all files staged for commit */
function gitStaged(baseDir, patterns) {
  baseDir = Deno.realPathSync(baseDir);
  const cmd = [
    "-C",
    baseDir,
    "diff",
    "--staged",
    "--diff-filter=ACMR",
    "--name-only",
    "-z",
    "--",
    ...patterns,
  ];
  return getFilesFromGit(baseDir, cmd);
}

/**
 *  Recursively list all files in (a subdirectory of) a git worktree.
 *    * Optionally, glob patterns may be specified to e.g. only list files with a
 *      certain extension.
 *    * Untracked files are included, unless they're listed in .gitignore.
 *    * Directory names themselves are not listed (but the files inside are).
 *    * Submodules and their contents are ignored entirely.
 *    * This function fails if the query matches no files.
 *
 * If --staged argument was provided when program is run
 * only staged sources will be returned.
 */
export async function getSources(baseDir, patterns) {
  const stagedOnly = Deno.args.includes("--staged");

  if (stagedOnly) {
    return await gitStaged(baseDir, patterns);
  } else {
    return await gitLsFiles(baseDir, patterns);
  }
}

export function buildMode() {
  if (Deno.args.includes("--release")) {
    return "release";
  }

  return "debug";
}

export function buildPath() {
  return join(ROOT_PATH, "target", buildMode());
}

const platformDirName = {
  "windows": "win",
  "darwin": "mac",
  "linux": "linux64",
}[Deno.build.os];

const executableSuffix = Deno.build.os === "windows" ? ".exe" : "";

async function sanityCheckPrebuiltFile(toolPath) {
  const stat = await Deno.stat(toolPath);
  if (stat.size < PREBUILT_MINIMUM_SIZE) {
    throw new Error(
      `File size ${stat.size} is less than expected minimum file size ${PREBUILT_MINIMUM_SIZE}`,
    );
  }
  const file = await Deno.open(toolPath, { read: true });
  const buffer = new Uint8Array(1024);
  let n = 0;
  while (n < 1024) {
    n += await file.read(buffer.subarray(n));
  }

  // Mac: OK
  if (buffer[0] == 0xcf && buffer[1] == 0xfa) {
    return;
  }

  // Windows OK
  if (buffer[0] == "M".charCodeAt(0) && buffer[1] == "Z".charCodeAt(0)) {
    return;
  }

  // Linux OK
  if (
    buffer[0] == 0x7f && buffer[1] == "E".charCodeAt(0) &&
    buffer[2] == "L".charCodeAt(0) && buffer[3] == "F".charCodeAt(0)
  ) {
    return;
  }

  throw new Error(`Invalid executable (header was ${buffer.subarray(0, 16)}`);
}

export async function getPrebuilt(toolName) {
  const toolPath = getPrebuiltToolPath(toolName);
  try {
    await sanityCheckPrebuiltFile(toolPath);
    const versionOk = await verifyVersion(toolName, toolPath);
    if (!versionOk) {
      throw new Error("Version mismatch");
    }
  } catch {
    await downloadPrebuilt(toolName);
  }

  return toolPath;
}

const PREBUILT_PATH = join(ROOT_PATH, "third_party", "prebuilt");
const PREBUILT_TOOL_DIR = join(PREBUILT_PATH, platformDirName);
const PREBUILT_MINIMUM_SIZE = 16 * 1024;
const DOWNLOAD_TASKS = {};

export function getPrebuiltToolPath(toolName) {
  return join(PREBUILT_TOOL_DIR, toolName + executableSuffix);
}

const commitId = "aa25a37b0f2bdadc83e99e625e8a074d56d1febd";
const downloadUrl =
  `https://raw.githubusercontent.com/denoland/deno_third_party/${commitId}/prebuilt/${platformDirName}`;

export async function downloadPrebuilt(toolName) {
  // Ensure only one download per tool happens at a time
  if (DOWNLOAD_TASKS[toolName]) {
    return await DOWNLOAD_TASKS[toolName].promise;
  }

  const downloadDeferred = DOWNLOAD_TASKS[toolName] = Promise.withResolvers();
  console.error("Downloading prebuilt tool:", toolName);
  const toolPath = getPrebuiltToolPath(toolName);
  const tempFile = `${toolPath}.temp`;

  try {
    await Deno.mkdir(PREBUILT_TOOL_DIR, { recursive: true });

    let url = `${downloadUrl}/${toolName}${executableSuffix}`;
    if (compressed.has(toolName)) {
      url += ".gz";
    }

    const headers = new Headers();
    if (Deno.env.has("GITHUB_TOKEN")) {
      headers.append("authorization", `Bearer ${Deno.env.get("GITHUB_TOKEN")}`);
    }

    const resp = await fetch(url, { headers });
    if (!resp.ok) {
      throw new Error(`Non-successful response from ${url}: ${resp.status}`);
    }

    const file = await Deno.open(tempFile, {
      create: true,
      write: true,
      mode: 0o755,
    });

    if (compressed.has(toolName)) {
      await resp.body.pipeThrough(new DecompressionStream("gzip")).pipeTo(
        file.writable,
      );
    } else {
      await resp.body.pipeTo(file.writable);
    }
    console.error("Checking prebuilt tool:", toolName);
    await sanityCheckPrebuiltFile(tempFile);
    if (!await verifyVersion(toolName, tempFile)) {
      throw new Error(
        "Didn't get the correct version of the tool after downloading.",
      );
    }
    console.error("Successfully downloaded:", toolName);
    try {
      // necessary on Windows it seems
      await Deno.remove(toolPath);
    } catch {
      // ignore
    }
    await Deno.rename(tempFile, toolPath);
  } catch (e) {
    downloadDeferred.reject(e);
    throw e;
  }

  downloadDeferred.resolve(null);
}

export async function verifyVersion(toolName, toolPath) {
  const requiredVersion = versions[toolName];
  if (!requiredVersion) {
    return true;
  }

  try {
    const cmd = new Deno.Command(toolPath, {
      args: ["--version"],
      stdout: "piped",
      stderr: "inherit",
    });
    const output = await cmd.output();
    const version = new TextDecoder().decode(output.stdout).trim();
    return version == requiredVersion;
  } catch (e) {
    console.error(e);
    return false;
  }
}

/// INPUT HASHING

/** A streaming hasher for computing a combined hash of multiple inputs.
 * Mirrors the Rust InputHasher API in tests/util/lib/hash.rs. */
export class InputHasher {
  #hash;

  constructor() {
    this.#hash = createHash("sha256");
  }

  /** Create a hasher pre-seeded with the current CLI args. */
  static newWithCliArgs() {
    const hasher = new InputHasher();
    for (const arg of Deno.args) {
      hasher.writeSync(arg);
    }
    return hasher;
  }

  /** Write raw string data into the hash. */
  writeSync(data) {
    this.#hash.update(data);
  }

  /** Hash a single file's contents (streamed). Skips if file doesn't exist. */
  async hashFile(path) {
    try {
      const file = await Deno.open(path);
      for await (const chunk of file.readable) {
        this.#hash.update(chunk);
      }
    } catch {
      // skip if file doesn't exist
    }
    return this;
  }

  /** Recursively hash all file contents in a directory (sorted for
   * determinism). Skips if directory doesn't exist. */
  async hashDir(path) {
    const entries = [];
    collectEntriesRecursive(path, entries);
    entries.sort();
    for (const entryPath of entries) {
      // hash the relative path for determinism
      if (entryPath.startsWith(path)) {
        this.writeSync(entryPath.slice(path.length));
      }
      try {
        const file = await Deno.open(entryPath);
        for await (const chunk of file.readable) {
          this.#hash.update(chunk);
        }
      } catch {
        // skip unreadable files
      }
    }
    return this;
  }

  /** Finalize the hash and return a hex string. */
  finish() {
    return this.#hash.digest("hex");
  }
}

function collectEntriesRecursive(dir, out) {
  let entries;
  try {
    entries = Deno.readDirSync(dir);
  } catch {
    return;
  }
  for (const entry of entries) {
    const fullPath = join(dir, entry.name);
    if (entry.isDirectory) {
      collectEntriesRecursive(fullPath, out);
    } else {
      out.push(fullPath);
    }
  }
}

const HASHY_URL = "https://hashy.deno.deno.net";

/**
 * Check if tests can be skipped on CI by comparing input hashes
 * against the hashy service.
 *
 * `name` is used for the hash key and log messages (e.g. "wpt").
 * `configure` receives an InputHasher to add whatever files/dirs are relevant.
 *
 * Returns `{ skip: boolean, commit: () => Promise<void> }`.
 * Call `commit()` after tests pass to mark the hash as known-good.
 */
export async function checkCiHash(name, configure) {
  const noop = { skip: false, commit: async () => {} };
  if (!Deno.env.get("CI")) {
    return noop;
  }

  const start = performance.now();

  const hasher = InputHasher.newWithCliArgs();
  await configure(hasher);
  const hash = await hasher.finish();
  const key = `${name}_${hash}`;

  const elapsed = Math.round(performance.now() - start);
  console.log(`ci hash took ${elapsed}ms`);

  const commitFn = async () => {
    try {
      await fetch(`${HASHY_URL}/hashes/${key}`, {
        method: "PUT",
        signal: AbortSignal.timeout(5000),
      });
      console.log(`hashy: committed hash ${key}`);
    } catch {
      console.log(`hashy: failed to commit hash ${key}`);
    }
  };

  // On main/tag builds, always run tests but still commit on success
  // to seed the cache for PR builds.
  if (isMainOrTag()) {
    console.log(
      `hashy: main/tag build, running tests (will commit on success)`,
    );
    return { skip: false, commit: commitFn };
  }

  try {
    const res = await fetch(`${HASHY_URL}/hashes/${key}`, {
      signal: AbortSignal.timeout(5000),
    });
    if (res.ok) {
      console.log(`hashy: ${name} hash found (${key}), skipping`);
      return { skip: true, commit: async () => {} };
    }
  } catch {
    // service unreachable — run tests
    console.log(`hashy: failed to check hash, running tests`);
    return noop;
  }

  console.log(`hashy: ${name} hash not found (${key}), will run tests`);
  return { skip: false, commit: commitFn };
}

function isMainOrTag() {
  const ref = Deno.env.get("GITHUB_REF") ?? "";
  return ref === "refs/heads/main" || ref.startsWith("refs/tags/");
}
