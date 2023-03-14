// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { fromFileUrl } from "ext:deno_node/path.ts";

type ExistsCallback = (exists: boolean) => void;

/**
 * TODO: Also accept 'path' parameter as a Node polyfill Buffer type once these
 * are implemented. See https://github.com/denoland/deno/issues/3403
 * Deprecated in node api
 */
export function exists(path: string | URL, callback: ExistsCallback) {
  path = path instanceof URL ? fromFileUrl(path) : path;
  Deno.lstat(path).then(() => callback(true), () => callback(false));
}

// The callback of fs.exists doesn't have standard callback signature.
// We need to provide special implementation for promisify.
// See https://github.com/nodejs/node/pull/13316
const kCustomPromisifiedSymbol = Symbol.for("nodejs.util.promisify.custom");
Object.defineProperty(exists, kCustomPromisifiedSymbol, {
  value: (path: string | URL) => {
    return new Promise((resolve) => {
      exists(path, (exists) => resolve(exists));
    });
  },
});

/**
 * TODO: Also accept 'path' parameter as a Node polyfill Buffer or URL type once these
 * are implemented. See https://github.com/denoland/deno/issues/3403
 */
export function existsSync(path: string | URL): boolean {
  path = path instanceof URL ? fromFileUrl(path) : path;
  try {
    Deno.lstatSync(path);
    return true;
  } catch (_err) {
    return false;
  }
}
