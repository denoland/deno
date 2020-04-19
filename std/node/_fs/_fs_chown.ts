// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import { CallbackWithError } from "./_fs_common.ts";

/**
 * TODO: Also accept 'path' parameter as a Node polyfill Buffer or URL type once these
 * are implemented. See https://github.com/denoland/deno/issues/3403
 */
export function chown(
  path: string,
  uid: number,
  gid: number,
  callback: CallbackWithError
): void {
  Deno.chown(path, uid, gid)
    .then(() => callback())
    .catch(callback);
}

/**
 * TODO: Also accept 'path' parameter as a Node polyfill Buffer or URL type once these
 * are implemented. See https://github.com/denoland/deno/issues/3403
 */
export function chownSync(path: string, uid: number, gid: number): void {
  Deno.chownSync(path, uid, gid);
}
