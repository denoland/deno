// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.

/**
 * Test whether or not the given path exists by checking with the file system
 */
export async function exists(filePath: string): Promise<boolean> {
  return Deno.lstat(filePath)
    .then((): boolean => true)
    .catch((): boolean => false);
}

/**
 * Test whether or not the given path exists by checking with the file system
 */
export function existsSync(filePath: string): boolean {
  try {
    Deno.lstatSync(filePath);
    return true;
  } catch {
    return false;
  }
}
