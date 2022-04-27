// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
import {
  dirname,
  fromFileUrl,
  join,
  resolve,
  toFileUrl,
} from "../test_util/std/path/mod.ts";
export { dirname, fromFileUrl, join, resolve, toFileUrl };
export { existsSync, walk } from "../test_util/std/fs/mod.ts";
export { TextLineStream } from "../test_util/std/streams/delimiter.ts";
export { delay } from "../test_util/std/async/delay.ts";

export const ROOT_PATH = dirname(dirname(fromFileUrl(import.meta.url)));

async function getFilesFromGit(baseDir, args) {
  const { status, stdout } = await Deno.spawn("git", {
    stderr: "inherit",
    args,
  });
  const output = new TextDecoder().decode(stdout);
  if (!status.success) {
    throw new Error("gitLsFiles failed");
  }

  const files = output.split("\0").filter((line) => line.length > 0).map(
    (filePath) => {
      return Deno.realPathSync(join(baseDir, filePath));
    },
  );

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

export function getPrebuiltToolPath(toolName) {
  const PREBUILT_PATH = join(ROOT_PATH, "third_party", "prebuilt");

  const platformDirName = {
    "windows": "win",
    "darwin": "mac",
    "linux": "linux64",
  }[Deno.build.os];
  const executableSuffix = Deno.build.os === "windows" ? ".exe" : "";
  return join(PREBUILT_PATH, platformDirName, toolName + executableSuffix);
}
