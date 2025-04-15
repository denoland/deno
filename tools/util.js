// Copyright 2018-2025 the Deno authors. MIT license.

// deno-lint-ignore-file no-console

import {
  dirname,
  extname,
  fromFileUrl,
  join,
  resolve,
  toFileUrl,
} from "@std/path";
import { wait } from "https://deno.land/x/wait@0.1.13/mod.ts";
export { dirname, extname, fromFileUrl, join, resolve, toFileUrl };
export { existsSync, expandGlobSync, walk } from "@std/fs";
export { TextLineStream } from "@std/streams/text-line-stream";
export { delay } from "@std/async/delay";
export { parse as parseJSONC } from "@std/jsonc/parse";

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

function gitLsFiles(baseDir, patterns) {
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
  const spinner = wait({
    text: "Downloading prebuilt tool: " + toolName,
    interval: 1000,
  }).start();
  const toolPath = getPrebuiltToolPath(toolName);
  const tempFile = `${toolPath}.temp`;

  try {
    await Deno.mkdir(PREBUILT_TOOL_DIR, { recursive: true });

    let url = `${downloadUrl}/${toolName}${executableSuffix}`;
    if (compressed.has(toolName)) {
      url += ".gz";
    }

    const resp = await fetch(url);
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
    spinner.text = `Checking prebuilt tool: ${toolName}`;
    await sanityCheckPrebuiltFile(tempFile);
    if (!await verifyVersion(toolName, tempFile)) {
      throw new Error(
        "Didn't get the correct version of the tool after downloading.",
      );
    }
    spinner.text = `Successfully downloaded: ${toolName}`;
    try {
      // necessary on Windows it seems
      await Deno.remove(toolPath);
    } catch {
      // ignore
    }
    await Deno.rename(tempFile, toolPath);
  } catch (e) {
    spinner.fail();
    downloadDeferred.reject(e);
    throw e;
  }

  spinner.succeed();
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
