// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { join } from "../path/mod.ts";
const { readdir, readdirSync, mkdir, mkdirSync, remove, removeSync } = Deno;
/**
 * Ensures that a directory is empty.
 * Deletes directory contents if the directory is not empty.
 * If the directory does not exist, it is created.
 * The directory itself is not deleted.
 * Requires the `--allow-read` and `--alow-write` flag.
 */
export async function emptyDir(dir: string): Promise<void> {
  try {
    const items = await readdir(dir);

    while (items.length) {
      const item = items.shift();
      if (item && item.name) {
        const filepath = join(dir, item.name);
        await remove(filepath, { recursive: true });
      }
    }
  } catch (err) {
    if (!(err instanceof Deno.errors.NotFound)) {
      throw err;
    }

    // if not exist. then create it
    await mkdir(dir, { recursive: true });
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
  try {
    const items = readdirSync(dir);

    // if directory already exist. then remove it's child item.
    while (items.length) {
      const item = items.shift();
      if (item && item.name) {
        const filepath = join(dir, item.name);
        removeSync(filepath, { recursive: true });
      }
    }
  } catch (err) {
    if (!(err instanceof Deno.errors.NotFound)) {
      throw err;
    }
    // if not exist. then create it
    mkdirSync(dir, { recursive: true });
    return;
  }
}
