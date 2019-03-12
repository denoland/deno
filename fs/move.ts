// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import * as path from "./path/mod.ts";
import { exists, existsSync } from "./exists.ts";

interface MoveOptions {
  overwrite?: boolean;
}

function isSrcSubdir(src: string, dest: string): boolean {
  const srcArray = src.split(path.sep);
  const destArray = dest.split(path.sep);

  return srcArray.reduce((acc, current, i) => {
    return acc && destArray[i] === current;
  }, true);
}

/**
 * Moves a file or directory
 * @export
 * @param {string} src
 * @param {string} dest
 * @param {MoveOptions} [options]
 * @returns {Promise<void>}
 */
export async function move(
  src: string,
  dest: string,
  options?: MoveOptions
): Promise<void> {
  src = path.resolve(src);
  dest = path.resolve(dest);

  const srcStat = await Deno.stat(src);

  if (srcStat.isDirectory() && isSrcSubdir(src, dest)) {
    throw new Error(
      `Cannot move '${src}' to a subdirectory of itself, '${dest}'.`
    );
  }

  if (options && options.overwrite) {
    await Deno.remove(dest, { recursive: true });
    await Deno.rename(src, dest);
  } else {
    if (await exists(dest)) {
      throw new Error("dest already exists.");
    }
    await Deno.rename(src, dest);
  }

  return;
}

/**
 * Moves a file or directory
 * @export
 * @param {string} src
 * @param {string} dest
 * @param {MoveOptions} [options]
 * @returns {void}
 */
export function moveSync(
  src: string,
  dest: string,
  options?: MoveOptions
): void {
  src = path.resolve(src);
  dest = path.resolve(dest);

  const srcStat = Deno.statSync(src);

  if (srcStat.isDirectory() && isSrcSubdir(src, dest)) {
    throw new Error(
      `Cannot move '${src}' to a subdirectory of itself, '${dest}'.`
    );
  }

  if (options && options.overwrite) {
    Deno.removeSync(dest, { recursive: true });
    Deno.renameSync(src, dest);
  } else {
    if (existsSync(dest)) {
      throw new Error("dest already exists.");
    }
    Deno.renameSync(src, dest);
  }
}
