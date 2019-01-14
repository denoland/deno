// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { lstatSync, readDirSync } from "deno";

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
