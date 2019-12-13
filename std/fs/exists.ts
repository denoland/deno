// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
const { lstat, lstatSync, DenoError, ErrorKind } = Deno;
/**
 * Test whether or not the given path exists by checking with the file system
 */
export async function exists(filePath: string): Promise<boolean> {
  return lstat(filePath)
    .then((): boolean => true)
    .catch((err: Error): boolean => {
      if (err instanceof DenoError) {
        if (err.kind === ErrorKind.NotFound) {
          return false;
        }
      }

      throw err;
    });
}

/**
 * Test whether or not the given path exists by checking with the file system
 */
export function existsSync(filePath: string): boolean {
  try {
    lstatSync(filePath);
    return true;
  } catch (err) {
    if (err instanceof DenoError) {
      if (err.kind === ErrorKind.NotFound) {
        return false;
      }
    }
    throw err;
  }
}
