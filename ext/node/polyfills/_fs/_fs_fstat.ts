// Copyright 2018-2026 the Deno authors. MIT license.

// TODO(petamoriken): enable prefer-primordials for node polyfills
// deno-lint-ignore-file prefer-primordials

import {
  CFISBIS,
  type statCallback,
  type statCallbackBigInt,
  type statOptions,
} from "ext:deno_node/internal/fs/stat_utils.ts";
import { BigIntStats, Stats } from "ext:deno_node/internal/fs/utils.mjs";
import { denoErrorToNodeError } from "ext:deno_node/internal/errors.ts";
import { getValidatedFd } from "ext:deno_node/internal/fs/utils.mjs";
import { op_node_fs_fstat, op_node_fs_fstat_sync } from "ext:core/ops";

// deno-lint-ignore no-explicit-any
function nodeFsStatToFileInfo(stat: any) {
  return {
    isFile: stat.isFile,
    isDirectory: stat.isDirectory,
    isSymlink: stat.isSymlink,
    size: stat.size,
    mtime: stat.mtimeMs != null ? new Date(stat.mtimeMs) : null,
    atime: stat.atimeMs != null ? new Date(stat.atimeMs) : null,
    birthtime: stat.birthtimeMs != null ? new Date(stat.birthtimeMs) : null,
    ctime: stat.ctimeMs != null ? new Date(stat.ctimeMs) : null,
    dev: stat.dev,
    ino: stat.ino ?? 0,
    mode: stat.mode,
    nlink: stat.nlink ?? 0,
    uid: stat.uid,
    gid: stat.gid,
    rdev: stat.rdev,
    blksize: stat.blksize,
    blocks: stat.blocks ?? 0,
    isBlockDevice: stat.isBlockDevice,
    isCharDevice: stat.isCharDevice,
    isFifo: stat.isFifo,
    isSocket: stat.isSocket,
  };
}

export function fstat(fd: number, callback: statCallback): void;
export function fstat(
  fd: number,
  options: { bigint: false },
  callback: statCallback,
): void;
export function fstat(
  fd: number,
  options: { bigint: true },
  callback: statCallbackBigInt,
): void;
export function fstat(
  fd: number,
  optionsOrCallback: statCallback | statCallbackBigInt | statOptions,
  maybeCallback?: statCallback | statCallbackBigInt,
) {
  fd = getValidatedFd(fd);
  const callback =
    (typeof optionsOrCallback === "function"
      ? optionsOrCallback
      : maybeCallback) as (
        ...args: [Error] | [null, BigIntStats | Stats]
      ) => void;
  const options = typeof optionsOrCallback === "object"
    ? optionsOrCallback
    : { bigint: false };

  if (!callback) throw new Error("No callback function supplied");

  op_node_fs_fstat(fd).then(
    (stat) =>
      callback(null, CFISBIS(nodeFsStatToFileInfo(stat), options.bigint)),
    (err) => callback(denoErrorToNodeError(err, { syscall: "fstat" })),
  );
}

export function fstatSync(fd: number): Stats;
export function fstatSync(
  fd: number,
  options: { bigint: false },
): Stats;
export function fstatSync(
  fd: number,
  options: { bigint: true },
): BigIntStats;
export function fstatSync(
  fd: number,
  options?: statOptions,
): Stats | BigIntStats {
  fd = getValidatedFd(fd);
  try {
    const stat = op_node_fs_fstat_sync(fd);
    return CFISBIS(nodeFsStatToFileInfo(stat), options?.bigint || false);
  } catch (err) {
    throw denoErrorToNodeError(err, { syscall: "fstat" });
  }
}

export function fstatPromise(fd: number): Promise<Stats>;
export function fstatPromise(
  fd: number,
  options: { bigint: false },
): Promise<Stats>;
export function fstatPromise(
  fd: number,
  options: { bigint: true },
): Promise<BigIntStats>;
export function fstatPromise(
  fd: number,
  options?: statOptions,
): Stats | BigIntStats {
  return new Promise((resolve, reject) => {
    fstat(fd, options, (err, stats) => {
      if (err) reject(err);
      else resolve(stats);
    });
  });
}
