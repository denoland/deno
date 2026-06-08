// Copyright 2018-2026 the Deno authors. MIT license.

import type { CallbackWithError } from "ext:deno_node/_fs/_fs_common.ts";
import { type Buffer } from "node:buffer";
import { core, primordials } from "ext:core/mod.js";
import { op_node_lutimes, op_node_lutimes_sync } from "ext:core/ops";
const { promisify } = core.loadExtScript("ext:deno_node/internal/util.mjs");

const {
  Error,
  PromisePrototypeThen,
} = primordials;

type TimeLike = number | string | Date;
type PathLike = string | Buffer | URL;

export function lutimes(
  path: PathLike,
  atime: TimeLike,
  mtime: TimeLike,
  callback: CallbackWithError,
): void {
  if (!callback) {
    throw new Error("No callback function supplied");
  }
  // The op validates the path + atime/mtime synchronously (async(eager_throw)).
  PromisePrototypeThen(
    op_node_lutimes(path, atime, mtime),
    () => callback(null),
    callback,
  );
}

export function lutimesSync(
  path: PathLike,
  atime: TimeLike,
  mtime: TimeLike,
): void {
  op_node_lutimes_sync(path, atime, mtime);
}

export const lutimesPromise = promisify(lutimes) as (
  path: PathLike,
  atime: TimeLike,
  mtime: TimeLike,
) => Promise<void>;
