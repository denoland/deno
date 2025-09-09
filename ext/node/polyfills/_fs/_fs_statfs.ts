// Copyright 2018-2025 the Deno authors. MIT license.

import { BigInt } from "ext:deno_node/internal/primordials.mjs";
import { op_node_statfs } from "ext:core/ops";
import { promisify } from "ext:deno_node/internal/util.mjs";
import type { Buffer } from "node:buffer";
import { getValidatedPathToString } from "ext:deno_node/internal/fs/utils.mjs";
import { makeCallback } from "ext:deno_node/_fs/_fs_common.ts";
import { denoErrorToNodeError } from "ext:deno_node/internal/errors.ts";

type StatFsCallback<T> = (err: Error | null, stats?: StatFs<T>) => void;

type StatFsOptions = {
  bigint?: boolean;
};

class StatFs<T> {
  type: T;
  bsize: T;
  blocks: T;
  bfree: T;
  bavail: T;
  files: T;
  ffree: T;
  constructor(
    type: T,
    bsize: T,
    blocks: T,
    bfree: T,
    bavail: T,
    files: T,
    ffree: T,
  ) {
    this.type = type;
    this.bsize = bsize;
    this.blocks = blocks;
    this.bfree = bfree;
    this.bavail = bavail;
    this.files = files;
    this.ffree = ffree;
  }
}

export function statfs(
  path: string | Buffer | URL,
  callback: StatFsCallback<number>,
): void;
export function statfs(
  path: string | Buffer | URL,
  options: { bigint?: false },
  callback: StatFsCallback<number>,
): void;
export function statfs(
  path: string | Buffer | URL,
  options: { bigint: true },
  callback: StatFsCallback<bigint>,
): void;
export function statfs(
  path: string | Buffer | URL,
  options: StatFsOptions | StatFsCallback<number> | undefined,
  callback?: StatFsCallback<number> | StatFsCallback<bigint>,
): void {
  if (typeof options === "function") {
    callback = options;
    options = undefined;
  }
  // @ts-expect-error callback type is known to be valid
  callback = makeCallback(callback);
  path = getValidatedPathToString(path);
  try {
    // TODO(Tango992): Implement async op
    const res = statfsSync(path, options);
    callback(null, res);
  } catch (err) {
    callback(err as Error);
  }
}

export function statfsSync(
  path: string | Buffer | URL,
  options?: { bigint?: false },
): StatFs<number>;
export function statfsSync(
  path: string | Buffer | URL,
  options: { bigint: true },
): StatFs<bigint>;
export function statfsSync(
  path: string | Buffer | URL,
  options?: StatFsOptions,
): StatFs<number> | StatFs<bigint> {
  path = getValidatedPathToString(path);
  const bigint = typeof options?.bigint === "boolean" ? options.bigint : false;

  try {
    const statFs = op_node_statfs(
      path,
      bigint,
    );
    return new StatFs(
      bigint ? BigInt(statFs.type) : statFs.type,
      bigint ? BigInt(statFs.bsize) : statFs.bsize,
      bigint ? BigInt(statFs.blocks) : statFs.blocks,
      bigint ? BigInt(statFs.bfree) : statFs.bfree,
      bigint ? BigInt(statFs.bavail) : statFs.bavail,
      bigint ? BigInt(statFs.files) : statFs.files,
      bigint ? BigInt(statFs.ffree) : statFs.ffree,
    );
  } catch (err) {
    throw denoErrorToNodeError(err as Error, {
      syscall: "statfs",
      path,
    });
  }
}

export const statfsPromise = promisify(statfs);
