// Copyright 2018-2025 the Deno authors. MIT license.

import { BigInt } from "ext:deno_node/internal/primordials.mjs";
import { op_node_statfs, op_node_statfs_sync } from "ext:core/ops";
import { promisify } from "ext:deno_node/internal/util.mjs";
import type { Buffer } from "node:buffer";
import { getValidatedPathToString } from "ext:deno_node/internal/fs/utils.mjs";
import { makeCallback } from "ext:deno_node/_fs/_fs_common.ts";
import { denoErrorToNodeError } from "ext:deno_node/internal/errors.ts";
import { primordials } from "ext:core/mod.js";

const { PromisePrototypeThen } = primordials;

type StatFsPromise = (
  path: string | Buffer | URL,
  options?: { bigint?: false },
) => Promise<StatFs<number>>;
type StatFsBigIntPromise = (
  path: string | Buffer | URL,
  options: { bigint: true },
) => Promise<StatFs<bigint>>;

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

type OpResult = {
  type: number;
  bsize: number;
  blocks: number;
  bfree: number;
  bavail: number;
  files: number;
  ffree: number;
};

function opResultToStatFs(result: OpResult, bigint: true): StatFs<bigint>;
function opResultToStatFs(
  result: OpResult,
  bigint: false,
): StatFs<number>;
function opResultToStatFs(
  result: OpResult,
  bigint: boolean,
): StatFs<bigint> | StatFs<number> {
  if (!bigint) {
    return new StatFs(
      result.type,
      result.bsize,
      result.blocks,
      result.bfree,
      result.bavail,
      result.files,
      result.ffree,
    );
  }
  return new StatFs(
    BigInt(result.type),
    BigInt(result.bsize),
    BigInt(result.blocks),
    BigInt(result.bfree),
    BigInt(result.bavail),
    BigInt(result.files),
    BigInt(result.ffree),
  );
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
  const bigint = typeof options?.bigint === "boolean" ? options.bigint : false;

  PromisePrototypeThen(
    op_node_statfs(path, bigint),
    (statFs) => {
      callback(
        null,
        opResultToStatFs(statFs, bigint),
      );
    },
    (err: Error) =>
      callback(denoErrorToNodeError(err, {
        syscall: "statfs",
        path,
      })),
  );
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
    const statFs = op_node_statfs_sync(
      path,
      bigint,
    );
    return opResultToStatFs(statFs, bigint);
  } catch (err) {
    throw denoErrorToNodeError(err as Error, {
      syscall: "statfs",
      path,
    });
  }
}

export const statfsPromise = promisify(statfs) as (
  StatFsPromise & StatFsBigIntPromise
);
