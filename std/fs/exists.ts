// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
/**
 * Test whether or not the given path exists by checking with the file system
 */
export async function exists(filePath: string | URL): Promise<boolean> {
  try {
    await Deno.lstat(filePath);
    return true;
  } catch (err) {
    if (err instanceof Deno.errors.NotFound) {
      return false;
    }

    throw err;
  }
}

/**
 * Test whether or not the given path exists by checking with the file system
 */
export function existsSync(filePath: string | URL): boolean {
  try {
    Deno.lstatSync(filePath);
    return true;
  } catch (err) {
    if (err instanceof Deno.errors.NotFound) {
      return false;
    }
    throw err;
  }
}
