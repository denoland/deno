// Copyright 2018-2026 the Deno authors. MIT license.

import { core, primordials } from "ext:core/mod.js";
import { BigIntStats, Stats } from "ext:deno_node/internal/fs/utils.mjs";
const { isWindows } = core.loadExtScript("ext:deno_node/_util/os.ts");

const {
  BigInt,
  Date,
  DatePrototypeGetTime,
  ObjectDefineProperties,
} = primordials;

export type statOptions = {
  bigint: boolean;
  throwIfNoEntry?: boolean;
};

export type statCallbackBigInt = (
  err: Error | null,
  stat?: BigIntStats,
) => void;

export type statCallback = (err: Error | null, stat?: Stats) => void;

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
    isWindows ? 4096 : origin.blksize,
    origin.ino || 0,
    origin.size,
    origin.blocks || 0,
    DatePrototypeGetTime(atime),
    DatePrototypeGetTime(mtime),
    DatePrototypeGetTime(ctime),
    DatePrototypeGetTime(birthtime),
  );

  return defineStatExtraProps(stats, origin);
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
    isWindows ? 4096n : toBigInt(origin.blksize),
    toBigInt(origin.ino),
    toBigInt(origin.size),
    toBigInt(origin.blocks),
    BigInt(DatePrototypeGetTime(atime)) * 1000000n,
    BigInt(DatePrototypeGetTime(mtime)) * 1000000n,
    BigInt(DatePrototypeGetTime(ctime)) * 1000000n,
    BigInt(DatePrototypeGetTime(birthtime)) * 1000000n,
  );

  return defineStatExtraProps(bigIntStats, origin);
}

const defineStatExtraProps = <T extends Stats | BigIntStats>(
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

export function CFISBIS(fileInfo: Deno.FileInfo, bigInt: false): Stats;
export function CFISBIS(fileInfo: Deno.FileInfo, bigInt: true): BigIntStats;
export function CFISBIS(
  fileInfo: Deno.FileInfo,
  bigInt: boolean,
): Stats | BigIntStats {
  if (bigInt) return convertFileInfoToBigIntStats(fileInfo);
  return convertFileInfoToStats(fileInfo);
}
