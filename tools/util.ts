// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { join } from "../js/deps/https/deno.land/x/std/fs/path/mod.ts";

const { platform, lstatSync, readDirSync } = Deno;

export interface FindOptions {
  skip?: string[];
  depth?: number;
}

/**
 * Finds files of the give extensions under the given paths recursively.
 * @param dirs directories
 * @param exts extensions
 * @param skip patterns to ignore
 * @param depth depth to find
 */
export function findFiles(
  dirs: string[],
  exts: string[],
  { skip = [], depth = 20 }: FindOptions = {}
) {
  return findFilesWalk(dirs, depth).filter(
    path =>
      exts.some(ext => path.endsWith(ext)) &&
      skip.every(pattern => !path.includes(pattern))
  );
}

function findFilesWalk(paths: string[], depth: number) {
  if (depth < 0) {
    return [];
  }

  const foundPaths = paths.map(path =>
    lstatSync(path).isDirectory()
      ? findFilesWalk(readDirSync(path).map(f => f.path), depth - 1)
      : path
  );

  return [].concat(...foundPaths);
}

export const executableSuffix = platform.os === "win" ? ".exe" : "";

/** Returns true if the path exists. */
export function existsSync(path: string): boolean {
  try {
    lstatSync(path);
  } catch (e) {
    return false;
  }
  return true;
}

/**
 * Looks up the available deno path with the priority
 * of release -> debug -> global
 */
export function lookupDenoPath(): string {
  const denoExe = "deno" + executableSuffix;
  const releaseExe = join("target", "release", denoExe);
  const debugExe = join("target", "debug", denoExe);

  if (existsSync(releaseExe)) {
    return releaseExe;
  } else if (existsSync(debugExe)) {
    return debugExe;
  }

  return denoExe;
}
