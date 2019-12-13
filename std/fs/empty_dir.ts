// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { join } from "../path/mod.ts";
const {
  readDir,
  readDirSync,
  mkdir,
  mkdirSync,
  remove,
  removeSync,
  ErrorKind
} = Deno;
/**
 * Ensures that a directory is empty.
 * Deletes directory contents if the directory is not empty.
 * If the directory does not exist, it is created.
 * The directory itself is not deleted.
 * Requires the `--allow-read` and `--alow-write` flag.
 */
export async function emptyDir(dir: string): Promise<void> {
  let items: Deno.FileInfo[] = [];
  try {
    items = await readDir(dir);
  } catch (err) {
    if ((err as Deno.DenoError<Deno.ErrorKind>).kind !== ErrorKind.NotFound) {
      throw err;
    }

    // if not exist. then create it
    await mkdir(dir, true);
    return;
  }

  // if directory already exist. then remove it's child item.
  while (items.length) {
    const item = items.shift();
    if (item && item.name) {
      const fn = join(dir, item.name);
      await remove(fn, { recursive: true });
    }
  }
}

/**
 * Ensures that a directory is empty.
 * Deletes directory contents if the directory is not empty.
 * If the directory does not exist, it is created.
 * The directory itself is not deleted.
 * Requires the `--allow-read` and `--alow-write` flag.
 */
export function emptyDirSync(dir: string): void {
  let items: Deno.FileInfo[] = [];
  try {
    items = readDirSync(dir);
  } catch (err) {
    if ((err as Deno.DenoError<Deno.ErrorKind>).kind !== ErrorKind.NotFound) {
      throw err;
    }
    // if not exist. then create it
    mkdirSync(dir, true);
    return;
  }

  // if directory already exist. then remove it's child item.
  while (items.length) {
    const item = items.shift();
    if (item && item.name) {
      const fn = join(dir, item.name);
      removeSync(fn, { recursive: true });
    }
  }
}
