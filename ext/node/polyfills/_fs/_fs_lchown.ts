// Copyright 2018-2026 the Deno authors. MIT license.

import {
  type CallbackWithError,
  makeCallback,
} from "ext:deno_node/_fs/_fs_common.ts";
import {
  getValidatedPathToString,
  kMaxUserId,
} from "ext:deno_node/internal/fs/utils.mjs";
import { validateInteger } from "ext:deno_node/internal/validators.mjs";
import type { Buffer } from "node:buffer";
import { promisify } from "ext:deno_node/internal/util.mjs";
import { op_node_lchown, op_node_lchown_sync } from "ext:core/ops";
import { primordials } from "ext:core/mod.js";

const { PromisePrototypeThen } = primordials;

/**
 * Asynchronously changes the owner and group
 * of a file, without following symlinks.
 */
export function lchown(
  path: string | Buffer | URL,
  uid: number,
  gid: number,
  callback: CallbackWithError,
) {
  callback = makeCallback(callback);
  path = getValidatedPathToString(path);
  validateInteger(uid, "uid", -1, kMaxUserId);
  validateInteger(gid, "gid", -1, kMaxUserId);

  PromisePrototypeThen(
    op_node_lchown(path, uid, gid),
    () => callback(null),
    callback,
  );
}

export const lchownPromise = promisify(lchown) as (
  path: string | Buffer | URL,
  uid: number,
  gid: number,
) => Promise<void>;

/**
 * Synchronously changes the owner and group
 * of a file, without following symlinks.
 */
export function lchownSync(
  path: string | Buffer | URL,
  uid: number,
  gid: number,
) {
  path = getValidatedPathToString(path);
  validateInteger(uid, "uid", -1, kMaxUserId);
  validateInteger(gid, "gid", -1, kMaxUserId);

  op_node_lchown_sync(path, uid, gid);
}