// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
/**
 * Ensures that a directory is empty.
 * Deletes directory contents if the directory is not empty.
 * If the directory does not exist, it is created.
 * The directory itself is not deleted.
 */
export async function emptyDir(dir: string): Promise<void> {
  let items: Deno.FileInfo[] = [];
  try {
    items = await Deno.readDir(dir);
  } catch {
    // if not exist. then create it
    await Deno.mkdir(dir, true);
    return;
  }
  while (items.length) {
    const item = items.shift();
    if (item && item.path) {
      await Deno.remove(item.path, { recursive: true });
    }
  }
}

/**
 * Ensures that a directory is empty.
 * Deletes directory contents if the directory is not empty.
 * If the directory does not exist, it is created.
 * The directory itself is not deleted.
 */
export function emptyDirSync(dir: string): void {
  let items: Deno.FileInfo[] = [];
  try {
    items = Deno.readDirSync(dir);
  } catch {
    // if not exist. then create it
    Deno.mkdirSync(dir, true);
    return;
  }
  while (items.length) {
    const item = items.shift();
    if (item && item.path) {
      Deno.removeSync(item.path, { recursive: true });
    }
  }
}
