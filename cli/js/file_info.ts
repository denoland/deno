// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { StatResponse } from "./ops/fs/stat.ts";
import { build } from "./build.ts";

export interface FileInfo {
  len: number;
  modified: number | null;
  accessed: number | null;
  created: number | null;
  name: string | null;
  dev: number | null;
  ino: number | null;
  mode: number | null;
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

// @internal
export class FileInfoImpl implements FileInfo {
  private readonly _isFile: boolean;
  private readonly _isSymlink: boolean;
  len: number;
  modified: number | null;
  accessed: number | null;
  created: number | null;
  name: string | null;

  dev: number | null;
  ino: number | null;
  mode: number | null;
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

    this._isFile = this._res.isFile;
    this._isSymlink = this._res.isSymlink;
    this.len = this._res.len;
    this.modified = modified ? modified : null;
    this.accessed = accessed ? accessed : null;
    this.created = created ? created : null;
    this.name = name ? name : null;
    // Only non-null if on Unix
    this.dev = isUnix ? dev : null;
    this.ino = isUnix ? ino : null;
    this.mode = isUnix ? mode : null;
    this.nlink = isUnix ? nlink : null;
    this.uid = isUnix ? uid : null;
    this.gid = isUnix ? gid : null;
    this.rdev = isUnix ? rdev : null;
    this.blksize = isUnix ? blksize : null;
    this.blocks = isUnix ? blocks : null;
  }

  isFile(): boolean {
    return this._isFile;
  }

  isDirectory(): boolean {
    return !this._isFile && !this._isSymlink;
  }

  isSymlink(): boolean {
    return this._isSymlink;
  }
}
