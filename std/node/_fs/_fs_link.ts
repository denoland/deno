// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import type { CallbackWithError } from "./_fs_common.ts";
import { fromFileUrl } from "../path.ts";

/**
 * TODO: Also accept 'path' parameter as a Node polyfill Buffer type once these
 * are implemented. See https://github.com/denoland/deno/issues/3403
 */
export function link(
  existingPath: string | URL,
  newPath: string | URL,
  callback: CallbackWithError,
): void {
  existingPath = existingPath instanceof URL
    ? fromFileUrl(existingPath)
    : existingPath;
  newPath = newPath instanceof URL ? fromFileUrl(newPath) : newPath;

  Deno.link(existingPath, newPath)
    .then(() => callback())
    .catch(callback);
}

/**
 * TODO: Also accept 'path' parameter as a Node polyfill Buffer type once these
 * are implemented. See https://github.com/denoland/deno/issues/3403
 */
export function linkSync(
  existingPath: string | URL,
  newPath: string | URL,
): void {
  existingPath = existingPath instanceof URL
    ? fromFileUrl(existingPath)
    : existingPath;
  newPath = newPath instanceof URL ? fromFileUrl(newPath) : newPath;

  Deno.linkSync(existingPath, newPath);
}
