// Copyright 2018-2026 the Deno authors. MIT license.

// TODO(petamoriken): enable prefer-primordials for node polyfills
// deno-lint-ignore-file prefer-primordials

import {
  type CallbackWithError,
  makeCallback,
} from "ext:deno_node/_fs/_fs_common.ts";
import { kMaxUserId } from "ext:deno_node/internal/fs/utils.mjs";
import { validateInteger } from "ext:deno_node/internal/validators.mjs";
import { op_fs_fchown_async, op_fs_fchown_sync } from "ext:core/ops";
import { promisify } from "ext:deno_node/internal/util.mjs";
import { getRid } from "ext:deno_node/internal/fs/fd_map.ts";

/**
 * Changes the owner and group of a file.
 */
export function fchown(
  fd: number,
  uid: number,
  gid: number,
  callback: CallbackWithError,
) {
  validateInteger(fd, "fd", 0, 2147483647);
  validateInteger(uid, "uid", -1, kMaxUserId);
  validateInteger(gid, "gid", -1, kMaxUserId);
  callback = makeCallback(callback);

  op_fs_fchown_async(getRid(fd), uid, gid).then(
    () => callback(null),
    callback,
  );
}
/**
 * Changes the owner and group of a file.
 */
export function fchownSync(
  fd: number,
  uid: number,
  gid: number,
) {
  validateInteger(fd, "fd", 0, 2147483647);
  validateInteger(uid, "uid", -1, kMaxUserId);
  validateInteger(gid, "gid", -1, kMaxUserId);

  op_fs_fchown_sync(getRid(fd), uid, gid);
}

export const fchownPromise = promisify(fchown) as (
  fd: number,
  uid: number,
  gid: number,
) => Promise<void>;
