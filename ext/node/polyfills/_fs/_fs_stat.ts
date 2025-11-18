// Copyright 2018-2025 the Deno authors. MIT license.

import { denoErrorToNodeError } from "ext:deno_node/internal/errors.ts";
import { promisify } from "ext:deno_node/internal/util.mjs";
import { primordials } from "ext:core/mod.js";
import {
  BigIntStats,
  getValidatedPathToString,
  Stats,
} from "ext:deno_node/internal/fs/utils.mjs";
import { makeCallback } from "ext:deno_node/_fs/_fs_common.ts";
import { isWindows } from "ext:deno_node/_util/os.ts";
import { Buffer } from "node:buffer";

export { BigIntStats, Stats };

const {
  BigInt,
  Date,
  DatePrototypeGetTime,
  ErrorPrototype,
  ObjectDefineProperties,
  ObjectPrototypeIsPrototypeOf,
  PromisePrototypeThen,
} = primordials;

export type statOptions = {
  bigint: boolean;
  throwIfNoEntry?: boolean;
};

export function convertFileInfoToStats(origin: Deno.FileInfo): Stats {
  const atime = origin.atime ?? new Date(0);
  const birthtime = origin.birthtime ?? new Date(0);
  const ctime = origin.ctime ?? new Date(0);
  const mtime = origin.mtime ?? new Date(0);

  const stats = new Stats(
    origin.dev,
    origin.mode || 0,
    origin.nlink || 0,
    isWindows ? 0 : origin.uid,
    isWindows ? 0 : origin.gid,
    isWindows ? 0 : origin.rdev,
    // https://github.com/nodejs/node/blob/591ba692bfe30408e6a67397e7d18bfa1b9c3561/deps/uv/src/win/fs.c#L1929-L1930
    isWindows ? 4096 : origin.blksize,
    origin.ino || 0,
    origin.size,
    origin.blocks || 0,
    DatePrototypeGetTime(atime),
    DatePrototypeGetTime(mtime),
    DatePrototypeGetTime(ctime),
    DatePrototypeGetTime(birthtime),
  );

  return defineExtraProps(stats, origin);
}

function toBigInt(number?: number | null): bigint {
  if (number === null || number === undefined) return 0n;
  return BigInt(number);
}

export function convertFileInfoToBigIntStats(
  origin: Deno.FileInfo,
): BigIntStats {
  const atime = origin.atime ?? new Date(0);
  const birthtime = origin.birthtime ?? new Date(0);
  const ctime = origin.ctime ?? new Date(0);
  const mtime = origin.mtime ?? new Date(0);

  const bigIntStats = new BigIntStats(
    toBigInt(origin.dev),
    toBigInt(origin.mode),
    toBigInt(origin.nlink),
    toBigInt(origin.uid),
    toBigInt(origin.gid),
    toBigInt(origin.rdev),
    // https://github.com/nodejs/node/blob/591ba692bfe30408e6a67397e7d18bfa1b9c3561/deps/uv/src/win/fs.c#L1929-L1930
    isWindows ? 4096n : toBigInt(origin.blksize),
    toBigInt(origin.ino),
    toBigInt(origin.size),
    toBigInt(origin.blocks),
    BigInt(DatePrototypeGetTime(atime)) * 1000000n,
    BigInt(DatePrototypeGetTime(mtime)) * 1000000n,
    BigInt(DatePrototypeGetTime(ctime)) * 1000000n,
    BigInt(DatePrototypeGetTime(birthtime)) * 1000000n,
  );

  return defineExtraProps(bigIntStats, origin);
}

const defineExtraProps = <T extends Stats | BigIntStats>(
  stats: T,
  origin: Deno.FileInfo,
): T => {
  ObjectDefineProperties(stats, {
    isDirectory: {
      __proto__: null,
      value: () => origin.isDirectory,
      writable: true,
      configurable: true,
    },
    isFile: {
      __proto__: null,
      value: () => origin.isFile,
      writable: true,
      configurable: true,
    },
    isSymbolicLink: {
      __proto__: null,
      value: () => origin.isSymlink,
      writable: true,
      configurable: true,
    },
    isBlockDevice: {
      __proto__: null,
      value: () => isWindows ? false : origin.isBlockDevice,
      writable: true,
      configurable: true,
    },
    isFIFO: {
      __proto__: null,
      value: () => isWindows ? false : origin.isFifo,
      writable: true,
      configurable: true,
    },
    isCharacterDevice: {
      __proto__: null,
      value: () => isWindows ? false : origin.isCharDevice,
      writable: true,
      configurable: true,
    },
    isSocket: {
      __proto__: null,
      value: () => isWindows ? false : origin.isSocket,
      writable: true,
      configurable: true,
    },
  });
  return stats;
};

// shortcut for Convert File Info to Stats or BigIntStats
export function CFISBIS(fileInfo: Deno.FileInfo, bigInt: false): Stats;
export function CFISBIS(fileInfo: Deno.FileInfo, bigInt: true): BigIntStats;
export function CFISBIS(
  fileInfo: Deno.FileInfo,
  bigInt: boolean,
): Stats | BigIntStats {
  if (bigInt) return convertFileInfoToBigIntStats(fileInfo);
  return convertFileInfoToStats(fileInfo);
}

export type statCallbackBigInt = (
  err: Error | null,
  stat?: BigIntStats,
) => void;

export type statCallback = (err: Error | null, stat?: Stats) => void;

const defaultOptions = { __proto__: null, bigint: false };
const defaultSyncOptions = {
  __proto__: null,
  bigint: false,
  throwIfNoEntry: true,
};

export function stat(path: string | Buffer | URL, callback: statCallback): void;
export function stat(
  path: string | Buffer | URL,
  options: { bigint: false },
  callback: statCallback,
): void;
export function stat(
  path: string | Buffer | URL,
  options: { bigint: true },
  callback: statCallbackBigInt,
): void;
export function stat(
  path: string | Buffer | URL,
  options: statCallback | statCallbackBigInt | statOptions = defaultOptions,
  callback?: statCallback | statCallbackBigInt,
) {
  if (typeof options === "function") {
    callback = options;
    options = defaultOptions;
  }
  callback = makeCallback(callback);
  path = getValidatedPathToString(path);

  PromisePrototypeThen(
    Deno.stat(path),
    (stat) => callback(null, CFISBIS(stat, options.bigint)),
    (err) =>
      callback(
        denoErrorToNodeError(err, { syscall: "stat", path }),
      ),
  );
}

export const statPromise = promisify(stat) as (
  & ((path: string | Buffer | URL) => Promise<Stats>)
  & ((
    path: string | Buffer | URL,
    options: { bigint: false },
  ) => Promise<Stats>)
  & ((
    path: string | Buffer | URL,
    options: { bigint: true },
  ) => Promise<BigIntStats>)
);

export function statSync(path: string | Buffer | URL): Stats;
export function statSync(
  path: string | Buffer | URL,
  options: { bigint: false; throwIfNoEntry: true },
): Stats;
export function statSync(
  path: string | Buffer | URL,
  options: { bigint: false; throwIfNoEntry: false },
): Stats | undefined;
export function statSync(
  path: string | Buffer | URL,
  options: { bigint: true; throwIfNoEntry: true },
): BigIntStats;
export function statSync(
  path: string | Buffer | URL,
  options: { bigint: true; throwIfNoEntry: false },
): BigIntStats | undefined;
export function statSync(
  path: string | Buffer | URL,
  options: statOptions = defaultSyncOptions,
): Stats | BigIntStats | undefined {
  path = getValidatedPathToString(path);

  try {
    const origin = Deno.statSync(path);
    return CFISBIS(origin, options.bigint);
  } catch (err) {
    if (
      options?.throwIfNoEntry === false &&
      ObjectPrototypeIsPrototypeOf(Deno.errors.NotFound.prototype, err)
    ) {
      return;
    }
    if (ObjectPrototypeIsPrototypeOf(ErrorPrototype, err)) {
      throw denoErrorToNodeError(err as Error, {
        syscall: "stat",
        path,
      });
    } else {
      throw err;
    }
  }
}
