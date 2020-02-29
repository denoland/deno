// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { StatResponse } from "./stat.ts";
import { build } from "./build.ts";

/** A FileInfo describes a file and is returned by `stat`, `lstat`,
 * `statSync`, `lstatSync`. A list of FileInfo is returned by `readdir`,
 * `readdirSync`. */
export interface FileInfo {
  /** The size of the file, in bytes. */
  length: number;
  /** The last modification time of the file. This corresponds to the `mtime`
   * field from `stat` on Linux/Mac OS and `ftLastWriteTime` on Windows. This
   * may not be available on all platforms. */
  modified: number | null;
  /** The last time either the file or its metadata was modified. This corresponds
   * to the `ctime` field from `stat` on Unix. Updated whenever `modified` is, and
   * also when the file is chown/chmod/renamed/moved. Unix only. */
  anyModified: number | null;
  /** The last access time of the file. This corresponds to the `atime`
   * field from `stat` on Unix and `ftLastAccessTime` on Windows. This may not
   * be available on all platforms. */
  accessed: number | null;
  /** The last access time of the file. This corresponds to the `birthtime`
   * field from `stat` on Mac/BSD and `ftCreationTime` on Windows. This may not
   * be available on all platforms. */
  created: number | null;
  /** The file or directory name. */
  name: string | null;
  /** ID of the device containing the file.
   *
   * _Linux/Mac OS only._ */
  dev: number | null;
  /** Inode number.
   *
   * _Linux/Mac OS only._ */
  ino: number | null;
  /** **UNSTABLE**: Match behavior with Go on windows for `perm`.
   *
   * The underlying raw `st_mode` bits that contain the standard Unix
   * permissions for this file/directory (masked to `0o7777`). */
  perm: number | null;
  /** Type of file this is info for. */
  type: FileType | null;
  /** Number of hard links pointing to this file.
   *
   * _Linux/Mac OS only._ */
  nlink: number | null;
  /** User ID of the owner of this file.
   *
   * _Linux/Mac OS only._ */
  uid: number | null;
  /** User ID of the owner of this file.
   *
   * _Linux/Mac OS only._ */
  gid: number | null;
  /** Device ID of this file.
   *
   * _Linux/Mac OS only._ */
  rdev: number | null;
  /** Blocksize for filesystem I/O.
   *
   * _Linux/Mac OS only._ */
  blksize: number | null;
  /** Number of blocks allocated to the file, in 512-byte units.
   *
   * _Linux/Mac OS only._ */
  blocks: number | null;
  /** Returns whether this is info for a regular file. This result is mutually
   * exclusive to `FileInfo.isDirectory` and `FileInfo.isSymlink`. */
  isFile(): boolean;
  /** Returns whether this is info for a regular directory. This result is
   * mutually exclusive to `FileInfo.isFile` and `FileInfo.isSymlink`. */
  isDirectory(): boolean;
  /** Returns whether this is info for a symlink. This result is
   * mutually exclusive to `FileInfo.isFile` and `FileInfo.isDirectory`. */
  isSymlink(): boolean;
}

// File types (from st_mode >> 12)
export enum FileType {
  TYPE_UNKNOWN = 14, // BSD "whiteout"
  TYPE_REGULAR = 8,
  TYPE_DIRECTORY = 4,
  TYPE_SYMLINK = 10,
  TYPE_SOCKET = 12,
  TYPE_FIFO = 1,
  TYPE_CHARDEV = 2,
  TYPE_BLKDEV = 6
}

// @internal
export class FileInfoImpl implements FileInfo {
  length: number;
  modified: number | null;
  anyModified: number | null;
  accessed: number | null;
  created: number | null;
  name: string | null;

  dev: number | null;
  ino: number | null;
  perm: number | null;
  type: FileType | null;
  nlink: number | null;
  uid: number | null;
  gid: number | null;
  rdev: number | null;
  blksize: number | null;
  blocks: number | null;

  /* @internal */
  constructor(private _res: StatResponse) {
    const isUnix = build.os === "mac" || build.os === "linux";
    const modified = this._res.modified;
    const accessed = this._res.accessed;
    const created = this._res.created;
    const name = this._res.name;
    // Unix only
    const {
      ctime,
      dev,
      ino,
      mode,
      nlink,
      uid,
      gid,
      rdev,
      blksize,
      blocks
    } = this._res;

    this.length = this._res.size;
    this.modified = modified ? modified : null;
    this.accessed = accessed ? accessed : null;
    this.created = created ? created : null;
    this.name = name ? name : null;
    // Only non-null if on Unix
    this.anyModified = isUnix ? ctime : null;
    this.dev = isUnix ? dev : null;
    this.ino = isUnix ? ino : null;
    this.perm = isUnix ? mode & 0o7777 : null;
    this.type = isUnix
      ? mode >> 12
      : this._res.isFile
      ? FileType.TYPE_REGULAR
      : this._res.isSymlink
      ? FileType.TYPE_SYMLINK
      : this._res.isDir
      ? FileType.TYPE_DIRECTORY
      : null;
    this.nlink = isUnix ? nlink : null;
    this.uid = isUnix ? uid : null;
    this.gid = isUnix ? gid : null;
    this.rdev = isUnix ? rdev : null;
    this.blksize = isUnix ? blksize : null;
    this.blocks = isUnix ? blocks : null;
  }

  isFile(): boolean {
    return this.type == FileType.TYPE_REGULAR; // this._res.isFile;
  }

  isDirectory(): boolean {
    return this.type == FileType.TYPE_DIRECTORY; // this._res.isDir;
  }

  isSymlink(): boolean {
    return this.type == FileType.TYPE_SYMLINK; // this._res.isSymlink;
  }
}
