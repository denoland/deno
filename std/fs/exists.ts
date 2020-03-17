// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
const { lstat, lstatSync } = Deno;
/**
 * Test whether or not the given path exists by checking with the file system
 */
export function exists(filePath: string): Promise<boolean> {
  return lstat(filePath)
    .then((): boolean => true)
    .catch((err: Error): boolean => {
      if (err instanceof Deno.errors.NotFound) {
        return false;
      }

      throw err;
    });
}

/**
 * Test whether or not the given path exists by checking with the file system
 */
export function existsSync(filePath: string): Promise<boolean> {
  try {
    lstatSync(filePath);
    return Promise.resolve(true);
  } catch (err) {
    if (err instanceof Deno.errors.NotFound) {
      return Promise.resolve(false);
    }
    throw err;
  }
}
