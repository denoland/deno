// Copyright 2018-2025 the Deno authors. MIT license.

// TODO(petamoriken): enable prefer-primordials for node polyfills
// deno-lint-ignore-file prefer-primordials

import {
  type CallbackWithError,
  makeCallback,
} from "ext:deno_node/_fs/_fs_common.ts";
import {
  getValidatedPath,
  kMaxUserId,
} from "ext:deno_node/internal/fs/utils.mjs";
import { validateInteger } from "ext:deno_node/internal/validators.mjs";
import type { Buffer } from "node:buffer";
import { promisify } from "ext:deno_node/internal/util.mjs";

/**
 * Asynchronously changes the owner and group
 * of a file.
 */
export function chown(
  path: string | Buffer | URL,
  uid: number,
  gid: number,
  callback: CallbackWithError,
) {
  callback = makeCallback(callback);
  path = getValidatedPath(path).toString();
  validateInteger(uid, "uid", -1, kMaxUserId);
  validateInteger(gid, "gid", -1, kMaxUserId);

  Deno.chown(path, uid, gid).then(
    () => callback(null),
    callback,
  );
}

export const chownPromise = promisify(chown) as (
  path: string | Buffer | URL,
  uid: number,
  gid: number,
) => Promise<void>;

/**
 * Synchronously changes the owner and group
 * of a file.
 */
export function chownSync(
  path: string | Buffer | URL,
  uid: number,
  gid: number,
) {
  path = getValidatedPath(path).toString();
  validateInteger(uid, "uid", -1, kMaxUserId);
  validateInteger(gid, "gid", -1, kMaxUserId);

  Deno.chownSync(path, uid, gid);
}
