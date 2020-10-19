// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import type { CallbackWithError } from "./_fs_common.ts";
import { fromFileUrl } from "../path.ts";

/**
 * TODO: Also accept 'path' parameter as a Node polyfill Buffer type once these
 * are implemented. See https://github.com/denoland/deno/issues/3403
 */
export function chown(
  path: string | URL,
  uid: number,
  gid: number,
  callback: CallbackWithError,
): void {
  path = path instanceof URL ? fromFileUrl(path) : path;

  Deno.chown(path, uid, gid)
    .then(() => callback())
    .catch(callback);
}

/**
 * TODO: Also accept 'path' parameter as a Node polyfill Buffer type once these
 * are implemented. See https://github.com/denoland/deno/issues/3403
 */
export function chownSync(path: string | URL, uid: number, gid: number): void {
  path = path instanceof URL ? fromFileUrl(path) : path;

  Deno.chownSync(path, uid, gid);
}
