// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import {
  dirname,
  fromFileUrl,
  join,
  resolve,
  toFileUrl,
} from "../test_util/std/path/mod.ts";
import { wait } from "https://deno.land/x/wait@0.1.13/mod.ts";
export { dirname, fromFileUrl, join, resolve, toFileUrl };
export { existsSync, walk } from "../test_util/std/fs/mod.ts";
export { TextLineStream } from "../test_util/std/streams/text_line_stream.ts";
export { delay } from "../test_util/std/async/delay.ts";

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

const platformDirName = {
  "windows": "win",
  "darwin": "mac",
  "linux": "linux64",
}[Deno.build.os];

const executableSuffix = Deno.build.os === "windows" ? ".exe" : "";

export async function getPrebuilt(toolName) {
  const toolPath = getPrebuiltToolPath(toolName);
  try {
    await Deno.stat(toolPath);
  } catch (e) {
    await downloadPrebuilt(toolName);
  }
  return toolPath;
}

const PREBUILT_PATH = join(ROOT_PATH, "third_party", "prebuilt");

export function getPrebuiltToolPath(toolName) {
  return join(PREBUILT_PATH, platformDirName, toolName + executableSuffix);
}

const downloadUrl =
  `https://raw.githubusercontent.com/denoland/deno_third_party/master/prebuilt/${platformDirName}`;

export async function downloadPrebuilt(toolName) {
  const spinner = wait("Downloading prebuilt tool: " + toolName).start();
  const toolPath = join(
    PREBUILT_PATH,
    platformDirName,
    toolName + executableSuffix,
  );

  try {
    await Deno.mkdir(join(PREBUILT_PATH, platformDirName), { recursive: true });

    const url = `${downloadUrl}/${toolName}${executableSuffix}`;

    const resp = await fetch(url);
    const file = await Deno.open(toolPath, {
      create: true,
      write: true,
      mode: 0o755,
    });

    await resp.body.pipeTo(file.writable);
  } catch (e) {
    spinner.fail();
    throw e;
  }

  spinner.succeed();
}
