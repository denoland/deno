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

export type statCallback = (err: Error | undefined, stat: Stats) => any;

export function convertFileInfoToStats(origin: Deno.FileInfo) {
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

export function stat(
  path: string | URL,
  options: statOptions,
  callback: statCallback
): void;
export function stat(path: string | URL, callback: statCallback): void;
export function stat(
  path: string | URL,
  optionsOrCallback: statCallback | statOptions,
  maybeCallback?: statCallback
) {
  const callback =
    typeof optionsOrCallback === "function"
      ? optionsOrCallback
      : maybeCallback || (() => {});
  // const options =
  //  typeof optionsOrCallback === "object" ? optionsOrCallback : null;
  Deno.stat(path)
    .then((stat) => callback(undefined, convertFileInfoToStats(stat)))
    // @ts-ignore
    .catch((err) => callback(err, null));
}

export function statSync(path: string | URL, options?: statOptions): Stats {
  const origin = Deno.statSync(path);
  return convertFileInfoToStats(origin);
}
