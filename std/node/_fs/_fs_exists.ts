// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

type ExitsCallback = (exists: boolean) => void;

/**
 * TODO: Also accept 'path' parameter as a Node polyfill Buffer or URL type once these
 * are implemented. See https://github.com/denoland/deno/issues/3403
 * Deprecated in node api
 */
export function exists(path: string, callback: ExitsCallback): void {
  Deno.lstat(path)
    .then(() => {
      callback(true);
    })
    .catch(() => callback(false));
}

/**
 * TODO: Also accept 'path' parameter as a Node polyfill Buffer or URL type once these
 * are implemented. See https://github.com/denoland/deno/issues/3403
 */
export function existsSync(path: string): boolean {
  try {
    Deno.lstatSync(path);
    return true;
  } catch (err) {
    if (err instanceof Deno.errors.NotFound) {
      return false;
    }
    throw err;
  }
}
