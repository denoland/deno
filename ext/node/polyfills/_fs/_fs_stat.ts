// Copyright 2018-2025 the Deno authors. MIT license.

// TODO(petamoriken): enable prefer-primordials for node polyfills
// deno-lint-ignore-file prefer-primordials

import { denoErrorToNodeError } from "ext:deno_node/internal/errors.ts";
import { promisify } from "ext:deno_node/internal/util.mjs";
import { primordials } from "ext:core/mod.js";
import { getValidatedPathToString } from "ext:deno_node/internal/fs/utils.mjs";
import { makeCallback } from "ext:deno_node/_fs/_fs_common.ts";
import { isWindows } from "ext:deno_node/_util/os.ts";

const { ObjectCreate, ObjectAssign } = primordials;

export type statOptions = {
  bigint: boolean;
  throwIfNoEntry?: boolean;
};

interface IStats {
  /** ID of the device containing the file.
   *
   * _Linux/Mac OS only._ */
  dev: number | null;
  /** Inode number.
   *
   * _Linux/Mac OS only._ */
  ino: number | null;
  /** **UNSTABLE**: Match behavior with Go on Windows for `mode`.
   *
   * The underlying raw `st_mode` bits that contain the standard Unix
   * permissions for this file/directory. */
  mode: number | null;
  /** Number of hard links pointing to this file.
   *
   * _Linux/Mac OS only._ */
  nlink: number | null;
  /** User ID of the owner of this file.
   *
   * _Linux/Mac OS only._ */
  uid: number | null;
  /** Group ID of the owner of this file.
   *
   * _Linux/Mac OS only._ */
  gid: number | null;
  /** Device ID of this file.
   *
   * _Linux/Mac OS only._ */
  rdev: number | null;
  /** The size of the file, in bytes. */
  size: number;
  /** Blocksize for filesystem I/O.
   *
   * _Linux/Mac OS only._ */
  blksize: number | null;
  /** Number of blocks allocated to the file, in 512-byte units.
   *
   * _Linux/Mac OS only._ */
  blocks: number | null;
  /** The last modification time of the file. This corresponds to the `mtime`
   * field from `stat` on Linux/Mac OS and `ftLastWriteTime` on Windows. This
   * may not be available on all platforms. */
  mtime: Date | null;
  /** The last access time of the file. This corresponds to the `atime`
   * field from `stat` on Unix and `ftLastAccessTime` on Windows. This may not
   * be available on all platforms. */
  atime: Date | null;
  /** The creation time of the file. This corresponds to the `birthtime`
   * field from `stat` on Mac/BSD and `ftCreationTime` on Windows. This may
   * not be available on all platforms. */
  birthtime: Date | null;
  /** change time */
  ctime: Date | null;
  /** atime in milliseconds */
  atimeMs: number | null;
  /** atime in milliseconds */
  mtimeMs: number | null;
  /** atime in milliseconds */
  ctimeMs: number | null;
  /** atime in milliseconds */
  birthtimeMs: number | null;
  isBlockDevice: () => boolean;
  isCharacterDevice: () => boolean;
  isDirectory: () => boolean;
  isFIFO: () => boolean;
  isFile: () => boolean;
  isSocket: () => boolean;
  isSymbolicLink: () => boolean;
}

class StatsBase {
  constructor(
    dev,
    mode,
    nlink,
    uid,
    gid,
    rdev,
    blksize,
    ino,
    size,
    blocks,
  ) {
    this.dev = dev;
    this.mode = mode;
    this.nlink = nlink;
    this.uid = uid;
    this.gid = gid;
    this.rdev = rdev;
    this.blksize = blksize;
    this.ino = ino;
    this.size = size;
    this.blocks = blocks;
  }

  isFile() {
    return false;
  }
  isDirectory() {
    return false;
  }
  isSymbolicLink() {
    return false;
  }
  isBlockDevice() {
    return false;
  }
  isFIFO() {
    return false;
  }
  isCharacterDevice() {
    return false;
  }
  isSocket() {
    return false;
  }
}

// The Date constructor performs Math.floor() to the timestamp.
// https://www.ecma-international.org/ecma-262/#sec-timeclip
// Since there may be a precision loss when the timestamp is
// converted to a floating point number, we manually round
// the timestamp here before passing it to Date().
function dateFromMs(ms) {
  return new Date(Number(ms) + 0.5);
}

export class Stats extends StatsBase {
  constructor(
    dev,
    mode,
    nlink,
    uid,
    gid,
    rdev,
    blksize,
    ino,
    size,
    blocks,
    atimeMs,
    mtimeMs,
    ctimeMs,
    birthtimeMs,
  ) {
    super(
      dev,
      mode,
      nlink,
      uid,
      gid,
      rdev,
      blksize,
      ino,
      size,
      blocks,
    );
    this.atimeMs = atimeMs;
    this.mtimeMs = mtimeMs;
    this.ctimeMs = ctimeMs;
    this.birthtimeMs = birthtimeMs;
    this.atime = dateFromMs(atimeMs);
    this.mtime = dateFromMs(mtimeMs);
    this.ctime = dateFromMs(ctimeMs);
    this.birthtime = dateFromMs(birthtimeMs);
  }
}

export interface IBigIntStats {
  /** ID of the device containing the file.
   *
   * _Linux/Mac OS only._ */
  dev: bigint | null;
  /** Inode number.
   *
   * _Linux/Mac OS only._ */
  ino: bigint | null;
  /** **UNSTABLE**: Match behavior with Go on Windows for `mode`.
   *
   * The underlying raw `st_mode` bits that contain the standard Unix
   * permissions for this file/directory. */
  mode: bigint | null;
  /** Number of hard links pointing to this file.
   *
   * _Linux/Mac OS only._ */
  nlink: bigint | null;
  /** User ID of the owner of this file.
   *
   * _Linux/Mac OS only._ */
  uid: bigint | null;
  /** Group ID of the owner of this file.
   *
   * _Linux/Mac OS only._ */
  gid: bigint | null;
  /** Device ID of this file.
   *
   * _Linux/Mac OS only._ */
  rdev: bigint | null;
  /** The size of the file, in bytes. */
  size: bigint;
  /** Blocksize for filesystem I/O.
   *
   * _Linux/Mac OS only._ */
  blksize: bigint | null;
  /** Number of blocks allocated to the file, in 512-byte units.
   *
   * _Linux/Mac OS only._ */
  blocks: bigint | null;
  /** The last modification time of the file. This corresponds to the `mtime`
   * field from `stat` on Linux/Mac OS and `ftLastWriteTime` on Windows. This
   * may not be available on all platforms. */
  mtime: Date | null;
  /** The last access time of the file. This corresponds to the `atime`
   * field from `stat` on Unix and `ftLastAccessTime` on Windows. This may not
   * be available on all platforms. */
  atime: Date | null;
  /** The creation time of the file. This corresponds to the `birthtime`
   * field from `stat` on Mac/BSD and `ftCreationTime` on Windows. This may
   * not be available on all platforms. */
  birthtime: Date | null;
  /** change time */
  ctime: Date | null;
  /** atime in milliseconds */
  atimeMs: bigint | null;
  /** atime in milliseconds */
  mtimeMs: bigint | null;
  /** atime in milliseconds */
  ctimeMs: bigint | null;
  /** atime in nanoseconds */
  birthtimeMs: bigint | null;
  /** atime in nanoseconds */
  atimeNs: bigint | null;
  /** atime in nanoseconds */
  mtimeNs: bigint | null;
  /** atime in nanoseconds */
  ctimeNs: bigint | null;
  /** atime in nanoseconds */
  birthtimeNs: bigint | null;
  isBlockDevice: () => boolean;
  isCharacterDevice: () => boolean;
  isDirectory: () => boolean;
  isFIFO: () => boolean;
  isFile: () => boolean;
  isSocket: () => boolean;
  isSymbolicLink: () => boolean;
}

export class BigIntStats {}

export function convertFileInfoToStats(origin: Deno.FileInfo): Stats {
  const stats = ObjectCreate(Stats.prototype);
  const atime = origin.atime ?? new Date(0);
  const birthtime = origin.birthtime ?? new Date(0);
  const ctime = origin.ctime ?? new Date(0);
  const mtime = origin.mtime ?? new Date(0);
  ObjectAssign(stats, {
    dev: origin.dev,
    ino: origin.ino || 0,
    mode: origin.mode || 0,
    nlink: origin.nlink || 0,
    uid: isWindows ? 0 : origin.uid,
    gid: isWindows ? 0 : origin.gid,
    rdev: isWindows ? 0 : origin.rdev,
    size: origin.size,
    // https://github.com/nodejs/node/blob/591ba692bfe30408e6a67397e7d18bfa1b9c3561/deps/uv/src/win/fs.c#L1929-L1930
    blksize: isWindows ? 4096 : origin.blksize,
    blocks: origin.blocks || 0,
    mtime,
    atime,
    birthtime,
    mtimeMs: BigInt(mtime.getTime()),
    atimeMs: BigInt(atime.getTime()),
    birthtimeMs: BigInt(birthtime.getTime()),
    isFile: () => origin.isFile,
    isDirectory: () => origin.isDirectory,
    isSymbolicLink: () => origin.isSymlink,
    isBlockDevice: () => isWindows ? false : origin.isBlockDevice,
    isFIFO: () => isWindows ? false : origin.isFifo,
    isCharacterDevice: () => isWindows ? false : origin.isCharDevice,
    isSocket: () => isWindows ? false : origin.isSocket,
    ctime,
    ctimeMs: BigInt(ctime.getTime()),
  });

  return stats;
}

function toBigInt(number?: number | null) {
  if (number === null || number === undefined) return null;
  return BigInt(number);
}

export function convertFileInfoToBigIntStats(
  origin: Deno.FileInfo,
): BigIntStats {
  const stats = ObjectCreate(BigIntStats.prototype);
  const atime = origin.atime ?? new Date(0);
  const birthtime = origin.birthtime ?? new Date(0);
  const ctime = origin.ctime ?? new Date(0);
  const mtime = origin.mtime ?? new Date(0);
  ObjectAssign(stats, {
    dev: toBigInt(origin.dev),
    ino: toBigInt(origin.ino) || 0n,
    mode: toBigInt(origin.mode) || 0n,
    nlink: toBigInt(origin.nlink) || 0n,
    uid: isWindows ? 0n : toBigInt(origin.uid),
    gid: isWindows ? 0n : toBigInt(origin.gid),
    rdev: isWindows ? 0n : toBigInt(origin.rdev),
    size: toBigInt(origin.size) || 0n,
    // https://github.com/nodejs/node/blob/591ba692bfe30408e6a67397e7d18bfa1b9c3561/deps/uv/src/win/fs.c#L1929-L1930
    blksize: isWindows ? 4096n : toBigInt(origin.blksize),
    blocks: toBigInt(origin.blocks) || 0n,
    mtime,
    atime,
    birthtime,
    mtimeMs: BigInt(mtime.getTime()),
    atimeMs: BigInt(atime.getTime()),
    birthtimeMs: BigInt(birthtime.getTime()),
    mtimeNs: BigInt(mtime.getTime()) * 1000000n,
    atimeNs: BigInt(atime.getTime()) * 1000000n,
    birthtimeNs: BigInt(birthtime.getTime()) * 1000000n,
    isFile: () => origin.isFile,
    isDirectory: () => origin.isDirectory,
    isSymbolicLink: () => origin.isSymlink,
    isBlockDevice: () => isWindows ? false : origin.isBlockDevice,
    isFIFO: () => isWindows ? false : origin.isFifo,
    isCharacterDevice: () => isWindows ? false : origin.isCharDevice,
    isSocket: () => isWindows ? false : origin.isSocket,
    ctime,
    ctimeMs: BigInt(ctime.getTime()),
    ctimeNs: BigInt(ctime.getTime()) * 1000000n,
  });
  return stats;
}

// shortcut for Convert File Info to Stats or BigIntStats
export function CFISBIS(fileInfo: Deno.FileInfo, bigInt: boolean) {
  if (bigInt) return convertFileInfoToBigIntStats(fileInfo);
  return convertFileInfoToStats(fileInfo);
}

export type statCallbackBigInt = (
  err: Error | null,
  stat?: BigIntStats,
) => void;

export type statCallback = (err: Error | null, stat?: Stats) => void;

const defaultOptions = { __proto__: null, bigint: false };

export function stat(path: string | URL, callback: statCallback): void;
export function stat(
  path: string | URL,
  options: { bigint: false },
  callback: statCallback,
): void;
export function stat(
  path: string | URL,
  options: { bigint: true },
  callback: statCallbackBigInt,
): void;
export function stat(
  path: string | URL,
  options: statCallback | statCallbackBigInt | statOptions = defaultOptions,
  callback?: statCallback | statCallbackBigInt,
) {
  if (typeof options === "function") {
    callback = options;
    options = defaultOptions;
  }
  callback = makeCallback(callback);
  path = getValidatedPathToString(path);

  Deno.stat(path).then(
    (stat) => callback(null, CFISBIS(stat, options.bigint)),
    (err) =>
      callback(
        denoErrorToNodeError(err, { syscall: "stat", path }),
      ),
  );
}

export const statPromise = promisify(stat) as (
  & ((path: string | URL) => Promise<Stats>)
  & ((path: string | URL, options: { bigint: false }) => Promise<Stats>)
  & ((path: string | URL, options: { bigint: true }) => Promise<BigIntStats>)
);

export function statSync(path: string | URL): Stats;
export function statSync(
  path: string | URL,
  options: { bigint: false; throwIfNoEntry: true },
): Stats;
export function statSync(
  path: string | URL,
  options: { bigint: false; throwIfNoEntry: false },
): Stats | undefined;
export function statSync(
  path: string | URL,
  options: { bigint: true; throwIfNoEntry: true },
): BigIntStats;
export function statSync(
  path: string | URL,
  options: { bigint: true; throwIfNoEntry: false },
): BigIntStats | undefined;
export function statSync(
  path: string | URL,
  options: statOptions = { ...defaultOptions, throwIfNoEntry: true },
): Stats | BigIntStats | undefined {
  path = getValidatedPathToString(path);

  try {
    const origin = Deno.statSync(path);
    return CFISBIS(origin, options.bigint);
  } catch (err) {
    if (
      options?.throwIfNoEntry === false &&
      err instanceof Deno.errors.NotFound
    ) {
      return;
    }
    if (err instanceof Error) {
      throw denoErrorToNodeError(err, {
        syscall: "stat",
        path,
      });
    } else {
      throw err;
    }
  }
}
