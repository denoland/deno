// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { StatResponse } from "./ops/fs/stat.ts";
import { build } from "./build.ts";

export interface FileInfo {
  size: number;
  modified: number | null;
  accessed: number | null;
  created: number | null;
  name: string | null;
  dev: number | null;
  ino: number | null;
  mode: number | null;
  type: FileType | null;
  nlink: number | null;
  uid: number | null;
  gid: number | null;
  rdev: number | null;
  blksize: number | null;
  blocks: number | null;
  isFile(): boolean;
  isDirectory(): boolean;
  isSymlink(): boolean;
}

// File types (from st_mode & ~0o7777)
export enum FileType {
  TYPE_UNKNOWN = 0, // whiteouts, doors, ports
  TYPE_REGULAR = 8 << 12,
  TYPE_DIRECTORY = 4 << 12,
  TYPE_SYMLINK = 10 << 12,
  TYPE_FIFO = 1 << 12,
  TYPE_CHARDEV = 2 << 12,
  TYPE_BLKDEV = 6 << 12,
  TYPE_SOCKET = 12 << 12
}

// @internal
export class FileInfoImpl implements FileInfo {
  size: number;
  modified: number | null;
  accessed: number | null;
  created: number | null;
  name: string | null;

  dev: number | null;
  ino: number | null;
  mode: number | null;
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

    this.size = this._res.size;
    this.modified = modified ? modified : null;
    this.accessed = accessed ? accessed : null;
    this.created = created ? created : null;
    this.name = name ? name : null;
    // Only non-null if on Unix
    this.dev = isUnix ? dev : null;
    this.ino = isUnix ? ino : null;
    this.mode = isUnix ? mode & 0o7777 : null;
    this.type = isUnix
      ? mode & ~0o7777
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
