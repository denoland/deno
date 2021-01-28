export type statOptions = {
  bigint: boolean;
};

export type Stats = {
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
};

export type BigIntStats = {
  /** ID of the device containing the file.
   *
   * _Linux/Mac OS only._ */
  dev: BigInt | null;
  /** Inode number.
   *
   * _Linux/Mac OS only._ */
  ino: BigInt | null;
  /** **UNSTABLE**: Match behavior with Go on Windows for `mode`.
   *
   * The underlying raw `st_mode` bits that contain the standard Unix
   * permissions for this file/directory. */
  mode: BigInt | null;
  /** Number of hard links pointing to this file.
   *
   * _Linux/Mac OS only._ */
  nlink: BigInt | null;
  /** User ID of the owner of this file.
   *
   * _Linux/Mac OS only._ */
  uid: BigInt | null;
  /** Group ID of the owner of this file.
   *
   * _Linux/Mac OS only._ */
  gid: BigInt | null;
  /** Device ID of this file.
   *
   * _Linux/Mac OS only._ */
  rdev: BigInt | null;
  /** The size of the file, in bytes. */
  size: BigInt;
  /** Blocksize for filesystem I/O.
   *
   * _Linux/Mac OS only._ */
  blksize: BigInt | null;
  /** Number of blocks allocated to the file, in 512-byte units.
   *
   * _Linux/Mac OS only._ */
  blocks: BigInt | null;
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
  atimeMs: BigInt | null;
  /** atime in milliseconds */
  mtimeMs: BigInt | null;
  /** atime in milliseconds */
  ctimeMs: BigInt | null;
  /** atime in nanoseconds */
  birthtimeMs: BigInt | null;
  /** atime in nanoseconds */
  atimeNs: BigInt | null;
  /** atime in nanoseconds */
  mtimeNs: BigInt | null;
  /** atime in nanoseconds */
  ctimeNs: BigInt | null;
  /** atime in nanoseconds */
  birthtimeNs: BigInt | null;
  isBlockDevice: () => boolean;
  isCharacterDevice: () => boolean;
  isDirectory: () => boolean;
  isFIFO: () => boolean;
  isFile: () => boolean;
  isSocket: () => boolean;
  isSymbolicLink: () => boolean;
};

export function convertFileInfoToStats(origin: Deno.FileInfo): Stats {
  return {
    dev: origin.dev,
    ino: origin.ino,
    mode: origin.mode,
    nlink: origin.nlink,
    uid: origin.uid,
    gid: origin.gid,
    rdev: origin.rdev,
    size: origin.size,
    blksize: origin.blksize,
    blocks: origin.blocks,
    mtime: origin.mtime,
    atime: origin.atime,
    birthtime: origin.birthtime,
    mtimeMs: origin.mtime?.getTime() || null,
    atimeMs: origin.atime?.getTime() || null,
    birthtimeMs: origin.birthtime?.getTime() || null,
    isFile: () => origin.isFile,
    isDirectory: () => origin.isDirectory,
    isSymbolicLink: () => origin.isSymlink,
    // not sure about those
    isBlockDevice: () => false,
    isFIFO: () => false,
    isCharacterDevice: () => false,
    isSocket: () => false,
    ctime: origin.mtime,
    ctimeMs: origin.mtime?.getTime() || null,
  };
}

function toBigInt(number?: number | null) {
  if (number === null || number === undefined) return null;
  return BigInt(number);
}

export function convertFileInfoToBigIntStats(
  origin: Deno.FileInfo,
): BigIntStats {
  return {
    dev: toBigInt(origin.dev),
    ino: toBigInt(origin.ino),
    mode: toBigInt(origin.mode),
    nlink: toBigInt(origin.nlink),
    uid: toBigInt(origin.uid),
    gid: toBigInt(origin.gid),
    rdev: toBigInt(origin.rdev),
    size: toBigInt(origin.size) || 0n,
    blksize: toBigInt(origin.blksize),
    blocks: toBigInt(origin.blocks),
    mtime: origin.mtime,
    atime: origin.atime,
    birthtime: origin.birthtime,
    mtimeMs: origin.mtime ? BigInt(origin.mtime.getTime()) : null,
    atimeMs: origin.atime ? BigInt(origin.atime.getTime()) : null,
    birthtimeMs: origin.birthtime ? BigInt(origin.birthtime.getTime()) : null,
    mtimeNs: origin.mtime ? BigInt(origin.mtime.getTime()) * 1000000n : null,
    atimeNs: origin.atime ? BigInt(origin.atime.getTime()) * 1000000n : null,
    birthtimeNs: origin.birthtime
      ? BigInt(origin.birthtime.getTime()) * 1000000n
      : null,
    isFile: () => origin.isFile,
    isDirectory: () => origin.isDirectory,
    isSymbolicLink: () => origin.isSymlink,
    // not sure about those
    isBlockDevice: () => false,
    isFIFO: () => false,
    isCharacterDevice: () => false,
    isSocket: () => false,
    ctime: origin.mtime,
    ctimeMs: origin.mtime ? BigInt(origin.mtime.getTime()) : null,
    ctimeNs: origin.mtime ? BigInt(origin.mtime.getTime()) * 1000000n : null,
  };
}

// shortcut for Convert File Info to Stats or BigIntStats
export function CFISBIS(fileInfo: Deno.FileInfo, bigInt: boolean) {
  if (bigInt) return convertFileInfoToBigIntStats(fileInfo);
  return convertFileInfoToStats(fileInfo);
}

export type statCallbackBigInt = (
  err: Error | null,
  stat: BigIntStats,
) => void;

export type statCallback = (err: Error | null, stat: Stats) => void;

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
  optionsOrCallback: statCallback | statCallbackBigInt | statOptions,
  maybeCallback?: statCallback | statCallbackBigInt,
) {
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

  Deno.stat(path).then(
    (stat) => callback(null, CFISBIS(stat, options.bigint)),
    (err) => callback(err),
  );
}

export function statSync(path: string | URL): Stats;
export function statSync(path: string | URL, options: { bigint: false }): Stats;
export function statSync(
  path: string | URL,
  options: { bigint: true },
): BigIntStats;
export function statSync(
  path: string | URL,
  options: statOptions = { bigint: false },
): Stats | BigIntStats {
  const origin = Deno.statSync(path);
  return CFISBIS(origin, options.bigint);
}
